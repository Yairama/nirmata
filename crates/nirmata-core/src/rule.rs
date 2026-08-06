use crate::{DomainError, JsonObject, RuleId, WorldId, validate_version};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Constitutive,
    Generative,
    Institutional,
    Authorial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSeverity {
    Advisory,
    Hard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleValidatorKind {
    NoResurrection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    id: RuleId,
    world_id: WorldId,
    kind: RuleKind,
    statement_md: String,
    scope: String,
    severity: RuleSeverity,
    source: Option<String>,
    validator_kind: Option<RuleValidatorKind>,
    parameters_json: JsonObject,
    version: u64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl Rule {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        kind: RuleKind,
        statement_md: impl Into<String>,
        scope: impl Into<String>,
        severity: RuleSeverity,
        source: Option<String>,
        validator_kind: Option<RuleValidatorKind>,
        parameters_json: impl Into<String>,
        now_ms: i64,
    ) -> Result<Self, DomainError> {
        Self::restore(
            RuleId::new(),
            world_id,
            kind,
            statement_md,
            scope,
            severity,
            source,
            validator_kind,
            parameters_json,
            1,
            now_ms,
            now_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: RuleId,
        world_id: WorldId,
        kind: RuleKind,
        statement_md: impl Into<String>,
        scope: impl Into<String>,
        severity: RuleSeverity,
        source: Option<String>,
        validator_kind: Option<RuleValidatorKind>,
        parameters_json: impl Into<String>,
        version: u64,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;
        let parameters_json = JsonObject::new("parameters_json", parameters_json)?;

        if severity == RuleSeverity::Hard && validator_kind.is_none() {
            return Err(DomainError::HardRuleWithoutValidator);
        }
        if matches!(validator_kind, Some(RuleValidatorKind::NoResurrection))
            && !parameters_json.is_empty()
        {
            return Err(DomainError::InvalidRuleValidatorParameters {
                validator: "no_resurrection",
            });
        }

        Ok(Self {
            id,
            world_id,
            kind,
            statement_md: statement_md.into(),
            scope: scope.into(),
            severity,
            source,
            validator_kind,
            parameters_json,
            version,
            created_at_ms,
            updated_at_ms,
        })
    }

    pub fn id(&self) -> RuleId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn kind(&self) -> RuleKind {
        self.kind
    }

    pub fn statement_md(&self) -> &str {
        &self.statement_md
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn severity(&self) -> RuleSeverity {
        self.severity
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub fn validator_kind(&self) -> Option<RuleValidatorKind> {
        self.validator_kind
    }

    pub fn parameters_json(&self) -> &JsonObject {
        &self.parameters_json
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

    pub fn can_produce_hard_error(&self) -> bool {
        self.severity == RuleSeverity::Hard && self.validator_kind.is_some()
    }
}

#[cfg(test)]
#[path = "../tests/unit/rule/mod.rs"]
mod tests;
