use crate::time::EventTime;
use crate::{DomainError, EntityId, EventId, GoalId, WorldId, required, validate_version};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventParticipant {
    entity_id: EntityId,
    role: String,
    ordinal: u32,
}

impl EventParticipant {
    pub fn new(
        entity_id: EntityId,
        role: impl Into<String>,
        ordinal: u32,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            entity_id,
            role: required("role", role)?,
            ordinal,
        })
    }

    pub fn entity_id(&self) -> EntityId {
        self.entity_id
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Event {
    id: EventId,
    world_id: WorldId,
    kind: String,
    summary: String,
    body_md: String,
    time: EventTime,
    location_entity_id: Option<EntityId>,
    participants: Vec<EventParticipant>,
    affected_goal_ids: Vec<GoalId>,
    version: u64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl Event {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        kind: impl Into<String>,
        summary: impl Into<String>,
        body_md: impl Into<String>,
        time: EventTime,
        location_entity_id: Option<EntityId>,
        participants: Vec<EventParticipant>,
        affected_goal_ids: Vec<GoalId>,
        now_ms: i64,
    ) -> Result<Self, DomainError> {
        Self::restore(
            EventId::new(),
            world_id,
            kind,
            summary,
            body_md,
            time,
            location_entity_id,
            participants,
            affected_goal_ids,
            1,
            now_ms,
            now_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EventId,
        world_id: WorldId,
        kind: impl Into<String>,
        summary: impl Into<String>,
        body_md: impl Into<String>,
        time: EventTime,
        location_entity_id: Option<EntityId>,
        participants: Vec<EventParticipant>,
        affected_goal_ids: Vec<GoalId>,
        version: u64,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;
        validate_ordinals(&participants)?;
        if affected_goal_ids.iter().collect::<HashSet<_>>().len() != affected_goal_ids.len() {
            return Err(DomainError::DuplicateReference);
        }

        Ok(Self {
            id,
            world_id,
            kind: required("kind", kind)?,
            summary: summary.into(),
            body_md: body_md.into(),
            time,
            location_entity_id,
            participants,
            affected_goal_ids,
            version,
            created_at_ms,
            updated_at_ms,
        })
    }

    pub fn id(&self) -> EventId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn body_md(&self) -> &str {
        &self.body_md
    }

    pub fn time(&self) -> &EventTime {
        &self.time
    }

    pub fn location_entity_id(&self) -> Option<EntityId> {
        self.location_entity_id
    }

    pub fn participants(&self) -> &[EventParticipant] {
        &self.participants
    }

    pub fn affected_goal_ids(&self) -> &[GoalId] {
        &self.affected_goal_ids
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventAggregate {
    event: Event,
    links: Vec<EventLink>,
}

impl EventAggregate {
    pub fn new(event: Event, links: Vec<EventLink>) -> Self {
        Self { event, links }
    }

    pub fn event(&self) -> &Event {
        &self.event
    }

    pub fn links(&self) -> &[EventLink] {
        &self.links
    }

    pub fn into_parts(self) -> (Event, Vec<EventLink>) {
        (self.event, self.links)
    }
}

fn validate_ordinals(participants: &[EventParticipant]) -> Result<(), DomainError> {
    let mut ordinals = HashSet::with_capacity(participants.len());
    for participant in participants {
        if !ordinals.insert(participant.ordinal) {
            return Err(DomainError::DuplicateOrdinal(participant.ordinal));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLinkKind {
    Enables,
    Causes,
    Motivates,
    Prevents,
    Terminates,
    Reveals,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventLink {
    source_event_id: EventId,
    target_event_id: EventId,
    kind: EventLinkKind,
}

impl EventLink {
    pub fn new(
        source_event_id: EventId,
        target_event_id: EventId,
        kind: EventLinkKind,
    ) -> Result<Self, DomainError> {
        if source_event_id == target_event_id {
            return Err(DomainError::DuplicateReference);
        }
        Ok(Self {
            source_event_id,
            target_event_id,
            kind,
        })
    }

    pub fn source_event_id(&self) -> EventId {
        self.source_event_id
    }

    pub fn target_event_id(&self) -> EventId {
        self.target_event_id
    }

    pub fn kind(&self) -> EventLinkKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{Certainty, TimePrecision};

    #[test]
    fn rejects_duplicate_participant_ordinals_and_self_causality() {
        let entity = EntityId::new();
        let participants = vec![
            EventParticipant::new(entity, "actor", 0).expect("participant"),
            EventParticipant::new(entity, "witness", 0).expect("participant"),
        ];
        let event = Event::new(
            WorldId::new(),
            "arrival",
            "",
            "",
            EventTime::instant(1, TimePrecision::Exact, Certainty::Certain),
            None,
            participants,
            vec![],
            1,
        );
        assert_eq!(event, Err(DomainError::DuplicateOrdinal(0)));

        let event_id = EventId::new();
        assert_eq!(
            EventLink::new(event_id, event_id, EventLinkKind::Causes),
            Err(DomainError::DuplicateReference)
        );
    }
}
