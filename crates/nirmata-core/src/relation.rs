use crate::time::Certainty;
use crate::{DomainError, EntityId, JsonObject, RelationId, WorldId, required, validate_version};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationDirection {
    Directed,
    Undirected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    id: RelationId,
    world_id: WorldId,
    source_entity_id: EntityId,
    target_entity_id: EntityId,
    kind: String,
    direction: RelationDirection,
    valid_from_tick: Option<i64>,
    valid_to_tick: Option<i64>,
    certainty: Certainty,
    source_reference: Option<String>,
    metadata_json: JsonObject,
    version: u64,
}

impl Relation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        source_entity_id: EntityId,
        target_entity_id: EntityId,
        kind: impl Into<String>,
        direction: RelationDirection,
        valid_from_tick: Option<i64>,
        valid_to_tick: Option<i64>,
        certainty: Certainty,
        source_reference: Option<String>,
        metadata_json: impl Into<String>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            RelationId::new(),
            world_id,
            source_entity_id,
            target_entity_id,
            kind,
            direction,
            valid_from_tick,
            valid_to_tick,
            certainty,
            source_reference,
            metadata_json,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: RelationId,
        world_id: WorldId,
        source_entity_id: EntityId,
        target_entity_id: EntityId,
        kind: impl Into<String>,
        direction: RelationDirection,
        valid_from_tick: Option<i64>,
        valid_to_tick: Option<i64>,
        certainty: Certainty,
        source_reference: Option<String>,
        metadata_json: impl Into<String>,
        version: u64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;
        if matches!(
            (valid_from_tick, valid_to_tick),
            (Some(start), Some(end)) if start > end
        ) {
            return Err(DomainError::InvalidPeriod);
        }

        Ok(Self {
            id,
            world_id,
            source_entity_id,
            target_entity_id,
            kind: required("kind", kind)?,
            direction,
            valid_from_tick,
            valid_to_tick,
            certainty,
            source_reference,
            metadata_json: JsonObject::new("metadata_json", metadata_json)?,
            version,
        })
    }

    pub fn id(&self) -> RelationId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn source_entity_id(&self) -> EntityId {
        self.source_entity_id
    }

    pub fn target_entity_id(&self) -> EntityId {
        self.target_entity_id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn direction(&self) -> RelationDirection {
        self.direction
    }

    pub fn valid_from_tick(&self) -> Option<i64> {
        self.valid_from_tick
    }

    pub fn valid_to_tick(&self) -> Option<i64> {
        self.valid_to_tick
    }

    pub fn certainty(&self) -> Certainty {
        self.certainty
    }

    pub fn source_reference(&self) -> Option<&str> {
        self.source_reference.as_deref()
    }

    pub fn metadata_json(&self) -> &JsonObject {
        &self.metadata_json
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn exactly_matches(&self, other: &Self) -> bool {
        self.world_id == other.world_id
            && self.kind == other.kind
            && self.direction == other.direction
            && self.valid_from_tick == other.valid_from_tick
            && self.valid_to_tick == other.valid_to_tick
            && self.certainty == other.certainty
            && self.source_reference == other.source_reference
            && self.metadata_json == other.metadata_json
            && match self.direction {
                RelationDirection::Directed => {
                    self.source_entity_id == other.source_entity_id
                        && self.target_entity_id == other.target_entity_id
                }
                RelationDirection::Undirected => {
                    (self.source_entity_id == other.source_entity_id
                        && self.target_entity_id == other.target_entity_id)
                        || (self.source_entity_id == other.target_entity_id
                            && self.target_entity_id == other.source_entity_id)
                }
            }
    }
}

#[cfg(test)]
#[path = "../tests/unit/relation/mod.rs"]
mod tests;
