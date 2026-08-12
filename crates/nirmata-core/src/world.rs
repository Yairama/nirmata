use crate::calendar::WorldCalendar;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, str::FromStr};
use uuid::Uuid;

pub const MAX_WORLD_NAME_CHARS: usize = 200;
pub const MAX_PREMISE_CHARS: usize = 100_000;
pub const MAX_EPOCH_LABEL_CHARS: usize = 200;

macro_rules! domain_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

domain_id!(WorldId);
domain_id!(RevisionId);
domain_id!(VariantId);
domain_id!(RuleId);
domain_id!(EntityId);
domain_id!(RelationId);
domain_id!(GoalId);
domain_id!(EventId);
domain_id!(ClaimId);
domain_id!(DocumentId);
domain_id!(ChangeSetId);
domain_id!(ChangeOperationId);
domain_id!(DecisionPointId);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct JsonObject(String);

impl JsonObject {
    pub fn new(field: &'static str, value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let parsed: serde_json::Value =
            serde_json::from_str(&value).map_err(|_| DomainError::InvalidJsonObject { field })?;
        if !parsed.is_object() {
            return Err(DomainError::InvalidJsonObject { field });
        }
        Ok(Self(value))
    }

    pub fn empty() -> Self {
        Self("{}".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&self.0)
            .is_ok_and(|object| object.is_empty())
    }
}

impl TryFrom<String> for JsonObject {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new("json", value)
    }
}

impl From<JsonObject> for String {
    fn from(value: JsonObject) -> Self {
        value.0
    }
}

impl Default for JsonObject {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Period {
    start_tick: Option<i64>,
    end_tick: Option<i64>,
}

impl Period {
    pub fn new(start_tick: Option<i64>, end_tick: Option<i64>) -> Result<Self, DomainError> {
        if matches!((start_tick, end_tick), (Some(start), Some(end)) if start > end) {
            return Err(DomainError::InvalidPeriod);
        }
        Ok(Self {
            start_tick,
            end_tick,
        })
    }

    pub fn start_tick(&self) -> Option<i64> {
        self.start_tick
    }

    pub fn end_tick(&self) -> Option<i64> {
        self.end_tick
    }

    pub fn is_ordered(&self) -> bool {
        !matches!(
            (self.start_tick, self.end_tick),
            (Some(start), Some(end)) if start > end
        )
    }

    pub fn overlaps(self, other: Self) -> bool {
        !matches!((self.end_tick, other.start_tick), (Some(end), Some(start)) if end < start)
            && !matches!((other.end_tick, self.start_tick), (Some(end), Some(start)) if end < start)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct World {
    id: WorldId,
    name: String,
    premise_md: String,
    epoch_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    calendar: Option<WorldCalendar>,
    current_revision: RevisionId,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl World {
    pub fn new(
        name: impl Into<String>,
        premise_md: impl Into<String>,
        epoch_label: impl Into<String>,
        now_ms: i64,
    ) -> Result<Self, DomainError> {
        Self::restore(
            WorldId::new(),
            name,
            premise_md,
            epoch_label,
            None,
            RevisionId::new(),
            now_ms,
            now_ms,
        )
    }

    pub fn restore(
        id: WorldId,
        name: impl Into<String>,
        premise_md: impl Into<String>,
        epoch_label: impl Into<String>,
        calendar: Option<WorldCalendar>,
        current_revision: RevisionId,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, DomainError> {
        let name = name.into().trim().to_owned();
        let premise_md = premise_md.into();
        let epoch_label = epoch_label.into();

        if name.is_empty() {
            return Err(DomainError::EmptyWorldName);
        }

        validate_length("name", &name, MAX_WORLD_NAME_CHARS)?;
        validate_length("premise_md", &premise_md, MAX_PREMISE_CHARS)?;
        validate_length("epoch_label", &epoch_label, MAX_EPOCH_LABEL_CHARS)?;

        Ok(Self {
            id,
            name,
            premise_md,
            epoch_label,
            calendar,
            current_revision,
            created_at_ms,
            updated_at_ms,
        })
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn premise_md(&self) -> &str {
        &self.premise_md
    }

    pub fn epoch_label(&self) -> &str {
        &self.epoch_label
    }

    pub fn calendar(&self) -> Option<&WorldCalendar> {
        self.calendar.as_ref()
    }

    pub fn current_revision(&self) -> RevisionId {
        self.current_revision
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

pub(crate) fn required(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, DomainError> {
    let value = value.into().trim().to_owned();
    if value.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    Ok(value)
}

pub(crate) fn validate_version(version: u64) -> Result<(), DomainError> {
    if version == 0 {
        return Err(DomainError::InvalidVersion);
    }
    Ok(())
}

fn validate_length(field: &'static str, value: &str, max_chars: usize) -> Result<(), DomainError> {
    if value.chars().count() > max_chars {
        return Err(DomainError::TextTooLong { field, max_chars });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    EmptyWorldName,
    EmptyField {
        field: &'static str,
    },
    TextTooLong {
        field: &'static str,
        max_chars: usize,
    },
    InvalidJsonObject {
        field: &'static str,
    },
    InvalidPeriod,
    InvalidEventTime,
    InvalidRuleValidatorParameters {
        validator: &'static str,
    },
    HardRuleWithoutValidator,
    DuplicateAlias(String),
    DuplicateOrdinal(u32),
    DuplicateReference,
    InvalidClaimContext(&'static str),
    InvalidConfidence,
    InvalidChangeSetContext(&'static str),
    DuplicateChangeOperationId(ChangeOperationId),
    DuplicateDecisionPointId(DecisionPointId),
    InvalidVersion,
    VersionOverflow,
    InvalidContentUri,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWorldName => write!(formatter, "world name cannot be empty"),
            Self::EmptyField { field } => write!(formatter, "{field} cannot be empty"),
            Self::TextTooLong { field, max_chars } => {
                write!(formatter, "{field} cannot exceed {max_chars} characters")
            }
            Self::InvalidJsonObject { field } => {
                write!(formatter, "{field} must be a valid JSON object")
            }
            Self::InvalidPeriod => write!(formatter, "period start cannot be after its end"),
            Self::InvalidEventTime => write!(formatter, "event time fields do not match its kind"),
            Self::InvalidRuleValidatorParameters { validator } => {
                write!(formatter, "parameters are incompatible with {validator}")
            }
            Self::HardRuleWithoutValidator => {
                write!(formatter, "a hard rule requires an implemented validator")
            }
            Self::DuplicateAlias(alias) => write!(formatter, "duplicate entity alias: {alias}"),
            Self::DuplicateOrdinal(ordinal) => write!(formatter, "duplicate ordinal: {ordinal}"),
            Self::DuplicateReference => write!(formatter, "duplicate reference"),
            Self::InvalidClaimContext(reason) => write!(formatter, "invalid claim: {reason}"),
            Self::InvalidConfidence => write!(formatter, "confidence must be between 0 and 1"),
            Self::InvalidChangeSetContext(reason) => {
                write!(formatter, "invalid change set: {reason}")
            }
            Self::DuplicateChangeOperationId(id) => {
                write!(formatter, "duplicate change operation id: {id}")
            }
            Self::DuplicateDecisionPointId(id) => {
                write!(formatter, "duplicate decision point id: {id}")
            }
            Self::InvalidVersion => write!(formatter, "version must be greater than zero"),
            Self::VersionOverflow => write!(formatter, "version cannot be incremented"),
            Self::InvalidContentUri => write!(formatter, "invalid nirmata content URI"),
        }
    }
}

impl Error for DomainError {}

#[cfg(test)]
#[path = "../tests/unit/world/mod.rs"]
mod tests;
