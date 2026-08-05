use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_domain,
    invalid_value, map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    EntityId, EventId, GoalId, WorldId,
    document::ObjectRef,
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    time::{Certainty, EventTime, EventTimeKind, TimePrecision},
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::{path::Path, str::FromStr};

pub use nirmata_core::event::EventAggregate;

impl WorldStore {
    pub fn insert_event(&mut self, aggregate: &EventAggregate) -> Result<(), StoreError> {
        let event = aggregate.event();
        ensure_world(self, event.world_id())?;
        let version = stored_version(event.version())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        insert_event_in_tx(&transaction, &self.path, aggregate, version)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_event(&self, id: EventId) -> Result<Option<EventAggregate>, StoreError> {
        load_event(&self.connection, &self.path, id)
    }

    pub fn list_events(&self) -> Result<Vec<EventAggregate>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, kind, summary, body_md, time_kind, start_tick, end_tick,
                        time_precision, certainty, location_entity_id, version,
                        created_at_ms, updated_at_ms
                 FROM events ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let rows = statement
            .query_map([], raw_event_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        rows.into_iter()
            .map(|row| restore_event(&self.connection, &self.path, row))
            .collect()
    }

    pub fn update_event(
        &mut self,
        aggregate: &EventAggregate,
    ) -> Result<EventAggregate, StoreError> {
        let event = aggregate.event();
        ensure_world(self, event.world_id())?;
        let id = event.id().to_string();
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        update_event_in_tx(&transaction, &self.path, aggregate)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_event(event.id())?
            .ok_or(StoreError::ObjectNotFound {
                object: "event",
                id,
            })
    }
}

pub(crate) fn insert_event_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    aggregate: &EventAggregate,
    version: i64,
) -> Result<(), StoreError> {
    validate_links(aggregate)?;
    insert_event_row(transaction, aggregate.event(), version)
        .map_err(|error| map_database_error(path, error))?;
    insert_event_children(transaction, aggregate)
        .map_err(|error| map_database_error(path, error))?;
    crate::search::index_event(transaction, path, aggregate.event())?;
    Ok(())
}

pub(crate) fn update_event_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    aggregate: &EventAggregate,
) -> Result<(), StoreError> {
    let event = aggregate.event();
    validate_links(aggregate)?;
    let expected = expected_version(event.version())?;
    let id = event.id().to_string();
    let changed = update_event_row(transaction, event, expected)
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            transaction,
            path,
            "event",
            "SELECT EXISTS(SELECT 1 FROM events WHERE id = ?1)",
            id,
            event.version(),
        )?);
    }
    transaction
        .execute("DELETE FROM event_participants WHERE event_id = ?1", [&id])
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute("DELETE FROM event_goals WHERE event_id = ?1", [&id])
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute("DELETE FROM event_links WHERE source_event_id = ?1", [&id])
        .map_err(|error| map_database_error(path, error))?;
    insert_event_children(transaction, aggregate)
        .map_err(|error| map_database_error(path, error))?;
    crate::search::index_event(transaction, path, event)?;
    Ok(())
}

pub(crate) fn delete_event_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    world_id: WorldId,
    id: EventId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = transaction
        .execute(
            "DELETE FROM events WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            transaction,
            path,
            "event",
            "SELECT EXISTS(SELECT 1 FROM events WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    crate::search::remove_text_index_row(transaction, path, world_id, ObjectRef::Event(id))?;
    content::remove_object(transaction, path, world_id, ObjectRef::Event(id))?;
    Ok(())
}

fn validate_links(aggregate: &EventAggregate) -> Result<(), StoreError> {
    if aggregate
        .links()
        .iter()
        .any(|link| link.source_event_id() != aggregate.event().id())
    {
        return Err(StoreError::InvalidAggregate(
            "every event link must originate from the aggregate event".to_owned(),
        ));
    }
    Ok(())
}

fn insert_event_row(
    transaction: &Transaction<'_>,
    event: &Event,
    version: i64,
) -> rusqlite::Result<usize> {
    let time = event.time();
    transaction.execute(
        "INSERT INTO events (
            id, world_id, kind, summary, body_md, time_kind, start_tick, end_tick,
            time_precision, certainty, location_entity_id, version, created_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            event.id().to_string(),
            event.world_id().to_string(),
            event.kind(),
            event.summary(),
            event.body_md(),
            time_kind(time.kind()),
            time.start_tick(),
            time.end_tick(),
            time_precision(time.precision()),
            certainty(time.certainty()),
            event.location_entity_id().map(|id| id.to_string()),
            version,
            event.created_at_ms(),
            event.updated_at_ms(),
        ],
    )
}

fn update_event_row(
    transaction: &Transaction<'_>,
    event: &Event,
    expected: i64,
) -> rusqlite::Result<usize> {
    let time = event.time();
    transaction.execute(
        "UPDATE events
         SET kind = ?1, summary = ?2, body_md = ?3, time_kind = ?4, start_tick = ?5,
             end_tick = ?6, time_precision = ?7, certainty = ?8, location_entity_id = ?9,
             version = version + 1, updated_at_ms = ?10
         WHERE id = ?11 AND world_id = ?12 AND version = ?13",
        params![
            event.kind(),
            event.summary(),
            event.body_md(),
            time_kind(time.kind()),
            time.start_tick(),
            time.end_tick(),
            time_precision(time.precision()),
            certainty(time.certainty()),
            event.location_entity_id().map(|id| id.to_string()),
            event.updated_at_ms(),
            event.id().to_string(),
            event.world_id().to_string(),
            expected,
        ],
    )
}

fn insert_event_children(
    transaction: &Transaction<'_>,
    aggregate: &EventAggregate,
) -> rusqlite::Result<()> {
    let event = aggregate.event();
    for participant in event.participants() {
        transaction.execute(
            "INSERT INTO event_participants (world_id, event_id, entity_id, role, ordinal)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.world_id().to_string(),
                event.id().to_string(),
                participant.entity_id().to_string(),
                participant.role(),
                i64::from(participant.ordinal()),
            ],
        )?;
    }
    for goal_id in event.affected_goal_ids() {
        transaction.execute(
            "INSERT INTO event_goals (world_id, event_id, goal_id) VALUES (?1, ?2, ?3)",
            params![
                event.world_id().to_string(),
                event.id().to_string(),
                goal_id.to_string(),
            ],
        )?;
    }
    for link in aggregate.links() {
        transaction.execute(
            "INSERT INTO event_links (
                world_id, source_event_id, target_event_id, kind
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                event.world_id().to_string(),
                link.source_event_id().to_string(),
                link.target_event_id().to_string(),
                link_kind(link.kind()),
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn load_event(
    connection: &Connection,
    path: &Path,
    id: EventId,
) -> Result<Option<EventAggregate>, StoreError> {
    let row = connection
        .query_row(
            "SELECT id, world_id, kind, summary, body_md, time_kind, start_tick, end_tick,
                    time_precision, certainty, location_entity_id, version,
                    created_at_ms, updated_at_ms
             FROM events WHERE id = ?1",
            [id.to_string()],
            raw_event_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))?;
    row.map(|row| restore_event(connection, path, row))
        .transpose()
}

fn restore_event(
    connection: &Connection,
    path: &Path,
    row: RawEvent,
) -> Result<EventAggregate, StoreError> {
    let mut participant_statement = connection
        .prepare(
            "SELECT entity_id, role, ordinal
             FROM event_participants WHERE event_id = ?1 ORDER BY ordinal",
        )
        .map_err(|error| map_schema_error(path, error))?;
    let participants = participant_statement
        .query_map([row.id.to_string()], participant_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))?;

    let mut goal_statement = connection
        .prepare("SELECT goal_id FROM event_goals WHERE event_id = ?1 ORDER BY rowid")
        .map_err(|error| map_schema_error(path, error))?;
    let goals = goal_statement
        .query_map([row.id.to_string()], |row| {
            GoalId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))
        })
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))?;

    let mut link_statement = connection
        .prepare(
            "SELECT source_event_id, target_event_id, kind
             FROM event_links WHERE source_event_id = ?1 ORDER BY rowid",
        )
        .map_err(|error| map_schema_error(path, error))?;
    let links = link_statement
        .query_map([row.id.to_string()], link_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))?;

    let event = Event::restore(
        row.id,
        row.world_id,
        row.kind,
        row.summary,
        row.body_md,
        row.time,
        row.location_entity_id,
        participants,
        goals,
        row.version,
        row.created_at_ms,
        row.updated_at_ms,
    )
    .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    Ok(EventAggregate::new(event, links))
}

struct RawEvent {
    id: EventId,
    world_id: WorldId,
    kind: String,
    summary: String,
    body_md: String,
    time: EventTime,
    location_entity_id: Option<EntityId>,
    version: u64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

fn raw_event_from_row(row: &Row<'_>) -> rusqlite::Result<RawEvent> {
    let location = row
        .get::<_, Option<String>>(10)?
        .map(|value| EntityId::from_str(&value).map_err(|error| invalid_data(10, error)))
        .transpose()?;
    Ok(RawEvent {
        id: EventId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        world_id: WorldId::from_str(&row.get::<_, String>(1)?)
            .map_err(|error| invalid_data(1, error))?,
        kind: row.get(2)?,
        summary: row.get(3)?,
        body_md: row.get(4)?,
        time: EventTime::new(
            parse_time_kind(5, &row.get::<_, String>(5)?)?,
            row.get(6)?,
            row.get(7)?,
            parse_time_precision(8, &row.get::<_, String>(8)?)?,
            parse_certainty(9, &row.get::<_, String>(9)?)?,
        )
        .map_err(|error| invalid_domain(5, error))?,
        location_entity_id: location,
        version: u64::try_from(row.get::<_, i64>(11)?).map_err(|error| invalid_data(11, error))?,
        created_at_ms: row.get(12)?,
        updated_at_ms: row.get(13)?,
    })
}

fn participant_from_row(row: &Row<'_>) -> rusqlite::Result<EventParticipant> {
    let ordinal = u32::try_from(row.get::<_, i64>(2)?).map_err(|error| invalid_data(2, error))?;
    EventParticipant::new(
        EntityId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        row.get::<_, String>(1)?,
        ordinal,
    )
    .map_err(|error| invalid_domain(1, error))
}

fn link_from_row(row: &Row<'_>) -> rusqlite::Result<EventLink> {
    EventLink::new(
        EventId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        EventId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?,
        parse_link_kind(2, &row.get::<_, String>(2)?)?,
    )
    .map_err(|error| invalid_domain(0, error))
}

fn time_kind(value: EventTimeKind) -> &'static str {
    match value {
        EventTimeKind::Unknown => "unknown",
        EventTimeKind::Instant => "instant",
        EventTimeKind::Interval => "interval",
        EventTimeKind::Ongoing => "ongoing",
    }
}

fn parse_time_kind(index: usize, value: &str) -> rusqlite::Result<EventTimeKind> {
    match value {
        "unknown" => Ok(EventTimeKind::Unknown),
        "instant" => Ok(EventTimeKind::Instant),
        "interval" => Ok(EventTimeKind::Interval),
        "ongoing" => Ok(EventTimeKind::Ongoing),
        _ => Err(invalid_value(index, value)),
    }
}

fn time_precision(value: TimePrecision) -> &'static str {
    match value {
        TimePrecision::Exact => "exact",
        TimePrecision::Day => "day",
        TimePrecision::Month => "month",
        TimePrecision::Year => "year",
        TimePrecision::Era => "era",
        TimePrecision::Unknown => "unknown",
    }
}

fn parse_time_precision(index: usize, value: &str) -> rusqlite::Result<TimePrecision> {
    match value {
        "exact" => Ok(TimePrecision::Exact),
        "day" => Ok(TimePrecision::Day),
        "month" => Ok(TimePrecision::Month),
        "year" => Ok(TimePrecision::Year),
        "era" => Ok(TimePrecision::Era),
        "unknown" => Ok(TimePrecision::Unknown),
        _ => Err(invalid_value(index, value)),
    }
}

fn certainty(value: Certainty) -> &'static str {
    match value {
        Certainty::Certain => "certain",
        Certainty::Approximate => "approximate",
        Certainty::Uncertain => "uncertain",
        Certainty::ApproximateUncertain => "approximate_uncertain",
    }
}

fn parse_certainty(index: usize, value: &str) -> rusqlite::Result<Certainty> {
    match value {
        "certain" => Ok(Certainty::Certain),
        "approximate" => Ok(Certainty::Approximate),
        "uncertain" => Ok(Certainty::Uncertain),
        "approximate_uncertain" => Ok(Certainty::ApproximateUncertain),
        _ => Err(invalid_value(index, value)),
    }
}

fn link_kind(value: EventLinkKind) -> &'static str {
    match value {
        EventLinkKind::Enables => "enables",
        EventLinkKind::Causes => "causes",
        EventLinkKind::Motivates => "motivates",
        EventLinkKind::Prevents => "prevents",
        EventLinkKind::Terminates => "terminates",
        EventLinkKind::Reveals => "reveals",
    }
}

fn parse_link_kind(index: usize, value: &str) -> rusqlite::Result<EventLinkKind> {
    match value {
        "enables" => Ok(EventLinkKind::Enables),
        "causes" => Ok(EventLinkKind::Causes),
        "motivates" => Ok(EventLinkKind::Motivates),
        "prevents" => Ok(EventLinkKind::Prevents),
        "terminates" => Ok(EventLinkKind::Terminates),
        "reveals" => Ok(EventLinkKind::Reveals),
        _ => Err(invalid_value(index, value)),
    }
}
