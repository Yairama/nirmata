use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_domain,
    invalid_value, map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    EntityId, GoalId, Period, WorldId,
    document::ObjectRef,
    goal::{Goal, GoalStatus, GoalVisibility},
};
use rusqlite::{OptionalExtension, Row, params};
use std::str::FromStr;

impl WorldStore {
    pub fn insert_goal(&mut self, goal: &Goal) -> Result<(), StoreError> {
        ensure_world(self, goal.world_id())?;
        insert_goal_in_tx(&self.connection, &self.path, goal)?;
        Ok(())
    }

    pub fn get_goal(&self, id: GoalId) -> Result<Option<Goal>, StoreError> {
        load_goal(&self.connection, &self.path, id)
    }

    pub fn list_goals(&self) -> Result<Vec<Goal>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, holder_entity_id, desired_state_md, priority, status,
                        valid_from_tick, valid_to_tick, visibility, source, version
                 FROM goals ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([], goal_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn update_goal(&mut self, goal: &Goal) -> Result<Goal, StoreError> {
        ensure_world(self, goal.world_id())?;
        let id = goal.id().to_string();
        update_goal_in_tx(&self.connection, &self.path, goal)?;
        self.get_goal(goal.id())?
            .ok_or(StoreError::ObjectNotFound { object: "goal", id })
    }
}

pub(crate) fn insert_goal_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    goal: &Goal,
) -> Result<(), StoreError> {
    let period = goal.period();
    connection
        .execute(
            "INSERT INTO goals (
                id, world_id, holder_entity_id, desired_state_md, priority, status,
                valid_from_tick, valid_to_tick, visibility, source, version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                goal.id().to_string(),
                goal.world_id().to_string(),
                goal.holder_entity_id().to_string(),
                goal.desired_state_md(),
                goal.priority(),
                status(goal.status()),
                period.and_then(|value| value.start_tick()),
                period.and_then(|value| value.end_tick()),
                visibility(goal.visibility()),
                goal.source(),
                stored_version(goal.version())?,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(crate) fn update_goal_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    goal: &Goal,
) -> Result<(), StoreError> {
    let expected = expected_version(goal.version())?;
    let period = goal.period();
    let id = goal.id().to_string();
    let changed = connection
        .execute(
            "UPDATE goals
             SET holder_entity_id = ?1, desired_state_md = ?2, priority = ?3, status = ?4,
                 valid_from_tick = ?5, valid_to_tick = ?6, visibility = ?7, source = ?8,
                 version = version + 1
             WHERE id = ?9 AND world_id = ?10 AND version = ?11",
            params![
                goal.holder_entity_id().to_string(),
                goal.desired_state_md(),
                goal.priority(),
                status(goal.status()),
                period.and_then(|value| value.start_tick()),
                period.and_then(|value| value.end_tick()),
                visibility(goal.visibility()),
                goal.source(),
                id,
                goal.world_id().to_string(),
                expected,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "goal",
            "SELECT EXISTS(SELECT 1 FROM goals WHERE id = ?1)",
            id,
            goal.version(),
        )?);
    }
    Ok(())
}

pub(crate) fn delete_goal_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    world_id: WorldId,
    id: GoalId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = connection
        .execute(
            "DELETE FROM goals WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "goal",
            "SELECT EXISTS(SELECT 1 FROM goals WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    content::remove_object(connection, path, world_id, ObjectRef::Goal(id))?;
    Ok(())
}

pub(crate) fn load_goal(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    id: GoalId,
) -> Result<Option<Goal>, StoreError> {
    connection
        .query_row(
            "SELECT id, world_id, holder_entity_id, desired_state_md, priority, status,
                    valid_from_tick, valid_to_tick, visibility, source, version
             FROM goals WHERE id = ?1",
            [id.to_string()],
            goal_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))
}

fn goal_from_row(row: &Row<'_>) -> rusqlite::Result<Goal> {
    let start: Option<i64> = row.get(6)?;
    let end: Option<i64> = row.get(7)?;
    let period = if start.is_some() || end.is_some() {
        Some(Period::new(start, end).map_err(|error| invalid_domain(6, error))?)
    } else {
        None
    };
    Goal::restore(
        GoalId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        WorldId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?,
        EntityId::from_str(&row.get::<_, String>(2)?).map_err(|error| invalid_data(2, error))?,
        row.get::<_, String>(3)?,
        row.get(4)?,
        parse_status(5, &row.get::<_, String>(5)?)?,
        period,
        parse_visibility(8, &row.get::<_, String>(8)?)?,
        row.get(9)?,
        u64::try_from(row.get::<_, i64>(10)?).map_err(|error| invalid_data(10, error))?,
    )
    .map_err(|error| invalid_domain(0, error))
}

fn status(value: GoalStatus) -> &'static str {
    match value {
        GoalStatus::Active => "active",
        GoalStatus::Achieved => "achieved",
        GoalStatus::Abandoned => "abandoned",
        GoalStatus::Frustrated => "frustrated",
    }
}

fn parse_status(index: usize, value: &str) -> rusqlite::Result<GoalStatus> {
    match value {
        "active" => Ok(GoalStatus::Active),
        "achieved" => Ok(GoalStatus::Achieved),
        "abandoned" => Ok(GoalStatus::Abandoned),
        "frustrated" => Ok(GoalStatus::Frustrated),
        _ => Err(invalid_value(index, value)),
    }
}

fn visibility(value: GoalVisibility) -> &'static str {
    match value {
        GoalVisibility::Public => "public",
        GoalVisibility::Secret => "secret",
    }
}

fn parse_visibility(index: usize, value: &str) -> rusqlite::Result<GoalVisibility> {
    match value {
        "public" => Ok(GoalVisibility::Public),
        "secret" => Ok(GoalVisibility::Secret),
        _ => Err(invalid_value(index, value)),
    }
}
