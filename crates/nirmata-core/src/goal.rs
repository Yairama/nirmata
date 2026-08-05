use crate::{DomainError, EntityId, GoalId, Period, WorldId, required, validate_version};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Achieved,
    Abandoned,
    Frustrated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalVisibility {
    Public,
    Secret,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    id: GoalId,
    world_id: WorldId,
    holder_entity_id: EntityId,
    desired_state_md: String,
    priority: i32,
    status: GoalStatus,
    period: Option<Period>,
    visibility: GoalVisibility,
    source: Option<String>,
    version: u64,
}

impl Goal {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        holder_entity_id: EntityId,
        desired_state_md: impl Into<String>,
        priority: i32,
        status: GoalStatus,
        period: Option<Period>,
        visibility: GoalVisibility,
        source: Option<String>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            GoalId::new(),
            world_id,
            holder_entity_id,
            desired_state_md,
            priority,
            status,
            period,
            visibility,
            source,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: GoalId,
        world_id: WorldId,
        holder_entity_id: EntityId,
        desired_state_md: impl Into<String>,
        priority: i32,
        status: GoalStatus,
        period: Option<Period>,
        visibility: GoalVisibility,
        source: Option<String>,
        version: u64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;

        Ok(Self {
            id,
            world_id,
            holder_entity_id,
            desired_state_md: required("desired_state_md", desired_state_md)?,
            priority,
            status,
            period,
            visibility,
            source,
            version,
        })
    }

    pub fn id(&self) -> GoalId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn holder_entity_id(&self) -> EntityId {
        self.holder_entity_id
    }

    pub fn desired_state_md(&self) -> &str {
        &self.desired_state_md
    }

    pub fn priority(&self) -> i32 {
        self.priority
    }

    pub fn status(&self) -> GoalStatus {
        self.status
    }

    pub fn period(&self) -> Option<Period> {
        self.period
    }

    pub fn visibility(&self) -> GoalVisibility {
        self.visibility
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_desired_state_and_ordered_period() {
        assert_eq!(
            Goal::new(
                WorldId::new(),
                EntityId::new(),
                " ",
                1,
                GoalStatus::Active,
                None,
                GoalVisibility::Secret,
                None,
            ),
            Err(DomainError::EmptyField {
                field: "desired_state_md"
            })
        );
        assert_eq!(
            Period::new(Some(2), Some(1)),
            Err(DomainError::InvalidPeriod)
        );
    }
}
