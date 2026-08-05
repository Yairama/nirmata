use crate::claim::Claim;
use crate::document::{ContentReference, Document, DocumentAggregate, ObjectRef};
use crate::entity::{Entity, EntityKind};
use crate::event::{Event, EventAggregate, EventLink};
use crate::goal::Goal;
use crate::relation::Relation;
use crate::rule::{Rule, RuleValidatorKind};
use crate::validation::{
    IssueObject, ValidationIssue, ValidationReport, ValidationSeverity, validate_claims,
    validate_documents, validate_event_links, validate_events, validate_expected_version,
    validate_goals, validate_lifecycle, validate_no_resurrection, validate_rules,
};
use crate::{
    ChangeOperationId, ChangeSetId, DecisionPointId, DomainError, RevisionId, World, WorldId,
    required,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const MAX_CHANGE_SET_OBJECTIVE_CHARS: usize = 1_000;
pub const MAX_CHANGE_SET_SOURCES: usize = 64;
pub const MAX_CHANGE_SET_ASSUMPTIONS: usize = 32;
pub const MAX_CHANGE_SET_ASSUMPTION_CHARS: usize = 500;
pub const MAX_CHANGE_SET_OPERATIONS: usize = 128;
pub const MAX_CHANGE_SET_DECISIONS: usize = 32;
pub const MAX_DECISION_PROMPT_CHARS: usize = 500;
pub const MAX_DECISION_ALTERNATIVE_CHARS: usize = 200;
pub const MAX_DECISION_ALTERNATIVES: usize = 8;
pub const MAX_REPLACEMENT_REASON_CHARS: usize = 500;
pub const MAX_OPERATION_AFFECTED_IDS: usize = 32;

pub struct ChangeSetValidationSnapshot<'a> {
    pub entities: &'a [Entity],
    pub relations: &'a [Relation],
    pub goals: &'a [Goal],
    pub events: &'a [Event],
    pub event_links: &'a [EventLink],
    pub rules: &'a [Rule],
    pub claims: &'a [Claim],
    pub documents: &'a [Document],
    pub content_references: &'a [ContentReference],
    pub revisions: &'a [RevisionId],
}

impl<'a> ChangeSetValidationSnapshot<'a> {
    pub const fn empty() -> Self {
        Self {
            entities: &[],
            relations: &[],
            goals: &[],
            events: &[],
            event_links: &[],
            rules: &[],
            claims: &[],
            documents: &[],
            content_references: &[],
            revisions: &[],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetconKind {
    Additive,
    Reinterpretive,
    Replacement,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionPoint {
    decision_point_id: DecisionPointId,
    operation_ids: Vec<ChangeOperationId>,
    prompt: String,
    alternatives: Vec<String>,
    replacement_target: Option<ObjectRef>,
    reason: Option<String>,
    resolved_alternative: Option<String>,
}

impl DecisionPoint {
    pub fn new(
        operation_ids: Vec<ChangeOperationId>,
        prompt: impl Into<String>,
        alternatives: Vec<String>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            DecisionPointId::new(),
            operation_ids,
            prompt,
            alternatives,
            None,
            None,
            None,
        )
    }

    pub fn new_replacement(
        operation_ids: Vec<ChangeOperationId>,
        prompt: impl Into<String>,
        alternatives: Vec<String>,
        replacement_target: ObjectRef,
        reason: impl Into<String>,
        resolved_alternative: impl Into<String>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            DecisionPointId::new(),
            operation_ids,
            prompt,
            alternatives,
            Some(replacement_target),
            Some(reason.into()),
            Some(resolved_alternative.into()),
        )
    }

    pub fn restore(
        decision_point_id: DecisionPointId,
        operation_ids: Vec<ChangeOperationId>,
        prompt: impl Into<String>,
        alternatives: Vec<String>,
        replacement_target: Option<ObjectRef>,
        reason: Option<String>,
        resolved_alternative: Option<String>,
    ) -> Result<Self, DomainError> {
        let prompt = required("prompt", prompt)?;
        if operation_ids.is_empty() {
            return Err(DomainError::InvalidChangeSetContext(
                "a decision point must reference at least one operation",
            ));
        }
        if alternatives.len() < 2 {
            return Err(DomainError::InvalidChangeSetContext(
                "a decision point must expose at least two alternatives",
            ));
        }

        let mut seen_operations = HashSet::with_capacity(operation_ids.len());
        for operation_id in &operation_ids {
            if !seen_operations.insert(*operation_id) {
                return Err(DomainError::InvalidChangeSetContext(
                    "a decision point cannot repeat an operation id",
                ));
            }
        }

        let mut seen_alternatives = HashSet::with_capacity(alternatives.len());
        let mut normalized_alternatives = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let alternative = required("alternative", alternative)?;
            if !seen_alternatives.insert(alternative.clone()) {
                return Err(DomainError::InvalidChangeSetContext(
                    "a decision point cannot repeat an alternative",
                ));
            }
            normalized_alternatives.push(alternative);
        }

        let reason = reason.map(|value| required("reason", value)).transpose()?;
        let resolved_alternative = resolved_alternative
            .map(|value| required("resolved_alternative", value))
            .transpose()?;
        if resolved_alternative.as_deref().is_some_and(|alternative| {
            !normalized_alternatives
                .iter()
                .any(|value| value == alternative)
        }) {
            return Err(DomainError::InvalidChangeSetContext(
                "a resolved alternative must match one of the decision alternatives",
            ));
        }

        Ok(Self {
            decision_point_id,
            operation_ids,
            prompt,
            alternatives: normalized_alternatives,
            replacement_target,
            reason,
            resolved_alternative,
        })
    }

    pub fn decision_point_id(&self) -> DecisionPointId {
        self.decision_point_id
    }

    pub fn operation_ids(&self) -> &[ChangeOperationId] {
        &self.operation_ids
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn alternatives(&self) -> &[String] {
        &self.alternatives
    }

    pub fn replacement_target(&self) -> Option<ObjectRef> {
        self.replacement_target
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    pub fn resolved_alternative(&self) -> Option<&str> {
        self.resolved_alternative.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ChangeOperation {
    UpdateWorld {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: World,
        after: World,
    },
    CreateEntity {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Entity,
    },
    UpdateEntity {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Entity,
        after: Entity,
    },
    DeleteEntity {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Entity,
    },
    CreateRelation {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Relation,
    },
    UpdateRelation {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Relation,
        after: Relation,
    },
    DeleteRelation {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Relation,
    },
    CreateEvent {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: EventAggregate,
    },
    UpdateEvent {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: EventAggregate,
        after: EventAggregate,
    },
    DeleteEvent {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: EventAggregate,
    },
    CreateGoal {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Goal,
    },
    UpdateGoal {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Goal,
        after: Goal,
    },
    DeleteGoal {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Goal,
    },
    CreateRule {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Rule,
    },
    UpdateRule {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Rule,
        after: Rule,
    },
    DeleteRule {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Rule,
    },
    CreateClaim {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Claim,
    },
    UpdateClaim {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Claim,
        after: Claim,
    },
    DeleteClaim {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Claim,
    },
    CreateDocument {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: DocumentAggregate,
    },
    UpdateDocument {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: DocumentAggregate,
        after: DocumentAggregate,
    },
    DeleteDocument {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: DocumentAggregate,
    },
}

impl ChangeOperation {
    pub fn operation_id(&self) -> ChangeOperationId {
        match self {
            Self::UpdateWorld { operation_id, .. }
            | Self::CreateEntity { operation_id, .. }
            | Self::UpdateEntity { operation_id, .. }
            | Self::DeleteEntity { operation_id, .. }
            | Self::CreateRelation { operation_id, .. }
            | Self::UpdateRelation { operation_id, .. }
            | Self::DeleteRelation { operation_id, .. }
            | Self::CreateEvent { operation_id, .. }
            | Self::UpdateEvent { operation_id, .. }
            | Self::DeleteEvent { operation_id, .. }
            | Self::CreateGoal { operation_id, .. }
            | Self::UpdateGoal { operation_id, .. }
            | Self::DeleteGoal { operation_id, .. }
            | Self::CreateRule { operation_id, .. }
            | Self::UpdateRule { operation_id, .. }
            | Self::DeleteRule { operation_id, .. }
            | Self::CreateClaim { operation_id, .. }
            | Self::UpdateClaim { operation_id, .. }
            | Self::DeleteClaim { operation_id, .. }
            | Self::CreateDocument { operation_id, .. }
            | Self::UpdateDocument { operation_id, .. }
            | Self::DeleteDocument { operation_id, .. } => *operation_id,
        }
    }

    pub fn retcon(&self) -> RetconKind {
        match self {
            Self::UpdateWorld { retcon, .. }
            | Self::CreateEntity { retcon, .. }
            | Self::UpdateEntity { retcon, .. }
            | Self::DeleteEntity { retcon, .. }
            | Self::CreateRelation { retcon, .. }
            | Self::UpdateRelation { retcon, .. }
            | Self::DeleteRelation { retcon, .. }
            | Self::CreateEvent { retcon, .. }
            | Self::UpdateEvent { retcon, .. }
            | Self::DeleteEvent { retcon, .. }
            | Self::CreateGoal { retcon, .. }
            | Self::UpdateGoal { retcon, .. }
            | Self::DeleteGoal { retcon, .. }
            | Self::CreateRule { retcon, .. }
            | Self::UpdateRule { retcon, .. }
            | Self::DeleteRule { retcon, .. }
            | Self::CreateClaim { retcon, .. }
            | Self::UpdateClaim { retcon, .. }
            | Self::DeleteClaim { retcon, .. }
            | Self::CreateDocument { retcon, .. }
            | Self::UpdateDocument { retcon, .. }
            | Self::DeleteDocument { retcon, .. } => *retcon,
        }
    }

    pub fn expected_version(&self) -> u64 {
        match self {
            Self::UpdateWorld {
                expected_version, ..
            }
            | Self::CreateEntity {
                expected_version, ..
            }
            | Self::UpdateEntity {
                expected_version, ..
            }
            | Self::DeleteEntity {
                expected_version, ..
            }
            | Self::CreateRelation {
                expected_version, ..
            }
            | Self::UpdateRelation {
                expected_version, ..
            }
            | Self::DeleteRelation {
                expected_version, ..
            }
            | Self::CreateEvent {
                expected_version, ..
            }
            | Self::UpdateEvent {
                expected_version, ..
            }
            | Self::DeleteEvent {
                expected_version, ..
            }
            | Self::CreateGoal {
                expected_version, ..
            }
            | Self::UpdateGoal {
                expected_version, ..
            }
            | Self::DeleteGoal {
                expected_version, ..
            }
            | Self::CreateRule {
                expected_version, ..
            }
            | Self::UpdateRule {
                expected_version, ..
            }
            | Self::DeleteRule {
                expected_version, ..
            }
            | Self::CreateClaim {
                expected_version, ..
            }
            | Self::UpdateClaim {
                expected_version, ..
            }
            | Self::DeleteClaim {
                expected_version, ..
            }
            | Self::CreateDocument {
                expected_version, ..
            }
            | Self::UpdateDocument {
                expected_version, ..
            }
            | Self::DeleteDocument {
                expected_version, ..
            } => *expected_version,
        }
    }

    pub fn affected_ids(&self) -> &[ObjectRef] {
        match self {
            Self::UpdateWorld { affected_ids, .. }
            | Self::CreateEntity { affected_ids, .. }
            | Self::UpdateEntity { affected_ids, .. }
            | Self::DeleteEntity { affected_ids, .. }
            | Self::CreateRelation { affected_ids, .. }
            | Self::UpdateRelation { affected_ids, .. }
            | Self::DeleteRelation { affected_ids, .. }
            | Self::CreateEvent { affected_ids, .. }
            | Self::UpdateEvent { affected_ids, .. }
            | Self::DeleteEvent { affected_ids, .. }
            | Self::CreateGoal { affected_ids, .. }
            | Self::UpdateGoal { affected_ids, .. }
            | Self::DeleteGoal { affected_ids, .. }
            | Self::CreateRule { affected_ids, .. }
            | Self::UpdateRule { affected_ids, .. }
            | Self::DeleteRule { affected_ids, .. }
            | Self::CreateClaim { affected_ids, .. }
            | Self::UpdateClaim { affected_ids, .. }
            | Self::DeleteClaim { affected_ids, .. }
            | Self::CreateDocument { affected_ids, .. }
            | Self::UpdateDocument { affected_ids, .. }
            | Self::DeleteDocument { affected_ids, .. } => affected_ids,
        }
    }

    pub fn primary_ref(&self) -> ObjectRef {
        match self {
            Self::UpdateWorld { before, .. } => ObjectRef::World(before.id()),
            Self::CreateEntity { after, .. } => ObjectRef::Entity(after.id()),
            Self::UpdateEntity { before, .. } | Self::DeleteEntity { before, .. } => {
                ObjectRef::Entity(before.id())
            }
            Self::CreateRelation { after, .. } => ObjectRef::Relation(after.id()),
            Self::UpdateRelation { before, .. } | Self::DeleteRelation { before, .. } => {
                ObjectRef::Relation(before.id())
            }
            Self::CreateEvent { after, .. } => ObjectRef::Event(after.event().id()),
            Self::UpdateEvent { before, .. } | Self::DeleteEvent { before, .. } => {
                ObjectRef::Event(before.event().id())
            }
            Self::CreateGoal { after, .. } => ObjectRef::Goal(after.id()),
            Self::UpdateGoal { before, .. } | Self::DeleteGoal { before, .. } => {
                ObjectRef::Goal(before.id())
            }
            Self::CreateRule { after, .. } => ObjectRef::Rule(after.id()),
            Self::UpdateRule { before, .. } | Self::DeleteRule { before, .. } => {
                ObjectRef::Rule(before.id())
            }
            Self::CreateClaim { after, .. } => ObjectRef::Claim(after.id()),
            Self::UpdateClaim { before, .. } | Self::DeleteClaim { before, .. } => {
                ObjectRef::Claim(before.id())
            }
            Self::CreateDocument { after, .. } => ObjectRef::Document(after.object().id()),
            Self::UpdateDocument { before, .. } | Self::DeleteDocument { before, .. } => {
                ObjectRef::Document(before.object().id())
            }
        }
    }

    fn created_ref(&self) -> Option<ObjectRef> {
        match self {
            Self::CreateEntity { after, .. } => Some(ObjectRef::Entity(after.id())),
            Self::CreateRelation { after, .. } => Some(ObjectRef::Relation(after.id())),
            Self::CreateEvent { after, .. } => Some(ObjectRef::Event(after.event().id())),
            Self::CreateGoal { after, .. } => Some(ObjectRef::Goal(after.id())),
            Self::CreateRule { after, .. } => Some(ObjectRef::Rule(after.id())),
            Self::CreateClaim { after, .. } => Some(ObjectRef::Claim(after.id())),
            Self::CreateDocument { after, .. } => Some(ObjectRef::Document(after.object().id())),
            _ => None,
        }
    }

    fn validate(&self, world_id: WorldId) -> Result<(), DomainError> {
        match self {
            Self::UpdateWorld {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_world_update(world_id, affected_ids, *expected_version, before, after),
            Self::CreateEntity {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Entity(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateEntity {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Entity(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Entity(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteEntity {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Entity(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateRelation {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Relation(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateRelation {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Relation(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Relation(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteRelation {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Relation(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateEvent {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Event(after.event().id()),
                after.event().world_id(),
                after.event().version(),
            ),
            Self::UpdateEvent {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Event(before.event().id()),
                before.event().world_id(),
                before.event().version(),
                ObjectRef::Event(after.event().id()),
                after.event().world_id(),
                after.event().version(),
            ),
            Self::DeleteEvent {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Event(before.event().id()),
                before.event().world_id(),
                before.event().version(),
            ),
            Self::CreateGoal {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Goal(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateGoal {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Goal(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Goal(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteGoal {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Goal(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateRule {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Rule(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateRule {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Rule(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Rule(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteRule {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Rule(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateClaim {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Claim(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateClaim {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Claim(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Claim(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteClaim {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Claim(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateDocument {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Document(after.object().id()),
                after.object().world_id(),
                after.object().version(),
            ),
            Self::UpdateDocument {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Document(before.object().id()),
                before.object().world_id(),
                before.object().version(),
                ObjectRef::Document(after.object().id()),
                after.object().world_id(),
                after.object().version(),
            ),
            Self::DeleteDocument {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Document(before.object().id()),
                before.object().world_id(),
                before.object().version(),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSetDraft {
    id: ChangeSetId,
    world_id: WorldId,
    base_revision: RevisionId,
    objective: String,
    sources: Vec<ObjectRef>,
    assumptions: Vec<String>,
    operations: Vec<ChangeOperation>,
    decisions: Vec<DecisionPoint>,
}

impl ChangeSetDraft {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            ChangeSetId::new(),
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: ChangeSetId,
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        let objective = required("objective", objective)?;
        let assumptions = normalize_strings("assumption", assumptions)?;
        validate_operations_and_decisions(world_id, &operations, &decisions)?;

        Ok(Self {
            id,
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        })
    }

    pub fn id(&self) -> ChangeSetId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn base_revision(&self) -> RevisionId {
        self.base_revision
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn sources(&self) -> &[ObjectRef] {
        &self.sources
    }

    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn operations(&self) -> &[ChangeOperation] {
        &self.operations
    }

    pub fn decisions(&self) -> &[DecisionPoint] {
        &self.decisions
    }

    pub fn validation_report(
        &self,
        snapshot: &ChangeSetValidationSnapshot<'_>,
    ) -> ValidationReport {
        validate_change_set_parts(
            self.world_id,
            self.base_revision,
            &self.objective,
            &self.sources,
            &self.assumptions,
            &self.operations,
            &self.decisions,
            snapshot,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    id: ChangeSetId,
    world_id: WorldId,
    base_revision: RevisionId,
    objective: String,
    sources: Vec<ObjectRef>,
    assumptions: Vec<String>,
    operations: Vec<ChangeOperation>,
    decisions: Vec<DecisionPoint>,
}

impl ChangeSet {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            ChangeSetId::new(),
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: ChangeSetId,
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        let objective = required("objective", objective)?;
        let assumptions = normalize_strings("assumption", assumptions)?;
        validate_operations_and_decisions(world_id, &operations, &decisions)?;

        Ok(Self {
            id,
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        })
    }

    pub fn id(&self) -> ChangeSetId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn base_revision(&self) -> RevisionId {
        self.base_revision
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn sources(&self) -> &[ObjectRef] {
        &self.sources
    }

    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn operations(&self) -> &[ChangeOperation] {
        &self.operations
    }

    pub fn decisions(&self) -> &[DecisionPoint] {
        &self.decisions
    }

    pub fn validation_report(
        &self,
        snapshot: &ChangeSetValidationSnapshot<'_>,
    ) -> ValidationReport {
        validate_change_set_parts(
            self.world_id,
            self.base_revision,
            &self.objective,
            &self.sources,
            &self.assumptions,
            &self.operations,
            &self.decisions,
            snapshot,
        )
    }
}

fn validate_change_set_parts(
    world_id: WorldId,
    base_revision: RevisionId,
    objective: &str,
    sources: &[ObjectRef],
    assumptions: &[String],
    operations: &[ChangeOperation],
    decisions: &[DecisionPoint],
    snapshot: &ChangeSetValidationSnapshot<'_>,
) -> ValidationReport {
    let mut report = ValidationReport::new();

    validate_non_empty_field(
        objective,
        "change_set.objective_empty",
        "change set objective cannot be empty",
        &mut report,
    );
    validate_max_chars(
        objective,
        MAX_CHANGE_SET_OBJECTIVE_CHARS,
        "change_set.objective_too_long",
        "change set objective exceeds the allowed size",
        &mut report,
    );
    validate_max_items(
        sources.len(),
        MAX_CHANGE_SET_SOURCES,
        "change_set.sources_too_many",
        "change set includes too many source references",
        &mut report,
    );
    validate_max_items(
        assumptions.len(),
        MAX_CHANGE_SET_ASSUMPTIONS,
        "change_set.assumptions_too_many",
        "change set includes too many assumptions",
        &mut report,
    );
    if operations.is_empty() {
        report.push(ValidationIssue::new(
            "change_set.operations_empty",
            ValidationSeverity::Error,
            vec![],
            "change set must include at least one operation",
        ));
    }
    validate_max_items(
        operations.len(),
        MAX_CHANGE_SET_OPERATIONS,
        "change_set.operations_too_many",
        "change set includes too many operations",
        &mut report,
    );
    validate_max_items(
        decisions.len(),
        MAX_CHANGE_SET_DECISIONS,
        "change_set.decisions_too_many",
        "change set includes too many decision points",
        &mut report,
    );

    let mut seen_sources = HashSet::with_capacity(sources.len());
    for source in sources {
        if !seen_sources.insert(*source) {
            report.push(ValidationIssue::new(
                "change_set.source_duplicate",
                ValidationSeverity::Error,
                vec![issue_object(*source)],
                "change set sources cannot repeat the same object",
            ));
        }
    }

    let mut seen_assumptions = HashSet::with_capacity(assumptions.len());
    for assumption in assumptions {
        validate_non_empty_field(
            assumption,
            "change_set.assumption_empty",
            "assumptions cannot be empty",
            &mut report,
        );
        validate_max_chars(
            assumption,
            MAX_CHANGE_SET_ASSUMPTION_CHARS,
            "change_set.assumption_too_long",
            "assumption exceeds the allowed size",
            &mut report,
        );
        if !seen_assumptions.insert(assumption.trim()) {
            report.push(ValidationIssue::new(
                "change_set.assumption_duplicate",
                ValidationSeverity::Error,
                vec![],
                "change set assumptions cannot repeat the same value",
            ));
        }
    }

    let mut operation_ids = HashSet::with_capacity(operations.len());
    let mut replacement_operations = HashSet::new();
    for operation in operations {
        if !operation_ids.insert(operation.operation_id()) {
            report.push(ValidationIssue::new(
                "change_set.operation_id_duplicate",
                ValidationSeverity::Error,
                vec![IssueObject::new(
                    "change_operation",
                    operation.operation_id(),
                )],
                "change set repeats an operation id",
            ));
        }
        if operation.retcon() == RetconKind::Replacement {
            replacement_operations.insert(operation.operation_id());
        }
    }

    let mut state = ValidationState::from_snapshot(snapshot, world_id, base_revision);
    let mut decision_ids = HashSet::with_capacity(decisions.len());
    let mut replacement_decisions = HashSet::new();
    let mut resolved_replacement_decisions = HashSet::new();
    for decision in decisions {
        let decision_object = IssueObject::new("decision_point", decision.decision_point_id());
        let touches_replacement = decision
            .operation_ids()
            .iter()
            .any(|operation_id| replacement_operations.contains(operation_id));
        if !decision_ids.insert(decision.decision_point_id()) {
            report.push(ValidationIssue::new(
                "change_set.decision_id_duplicate",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "change set repeats a decision point id",
            ));
        }

        validate_non_empty_field(
            decision.prompt(),
            "change_set.decision_prompt_empty",
            "decision prompt cannot be empty",
            &mut report,
        );
        validate_max_chars(
            decision.prompt(),
            MAX_DECISION_PROMPT_CHARS,
            "change_set.decision_prompt_too_long",
            "decision prompt exceeds the allowed size",
            &mut report,
        );
        if decision.operation_ids().is_empty() {
            report.push(ValidationIssue::new(
                "change_set.decision_operations_empty",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "decision point must reference at least one operation",
            ));
        }
        if decision.alternatives().len() < 2 {
            report.push(ValidationIssue::new(
                "change_set.decision_alternatives_too_few",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "decision point must expose at least two alternatives",
            ));
        }
        validate_max_items(
            decision.alternatives().len(),
            MAX_DECISION_ALTERNATIVES,
            "change_set.decision_alternatives_too_many",
            "decision point exposes too many alternatives",
            &mut report,
        );

        let mut seen_operation_ids = HashSet::with_capacity(decision.operation_ids().len());
        for operation_id in decision.operation_ids() {
            if !operation_ids.contains(operation_id) {
                report.push(ValidationIssue::new(
                    "change_set.decision_operation_missing",
                    ValidationSeverity::Error,
                    vec![
                        decision_object.clone(),
                        IssueObject::new("change_operation", operation_id),
                    ],
                    "decision point references an unknown operation",
                ));
            }
            if !seen_operation_ids.insert(*operation_id) {
                report.push(ValidationIssue::new(
                    "change_set.decision_operation_duplicate",
                    ValidationSeverity::Error,
                    vec![
                        decision_object.clone(),
                        IssueObject::new("change_operation", operation_id),
                    ],
                    "decision point repeats the same operation id",
                ));
            }
            if replacement_operations.contains(operation_id) {
                replacement_decisions.insert(*operation_id);
                if decision.resolved_alternative().is_some() {
                    resolved_replacement_decisions.insert(*operation_id);
                }
            }
        }

        let mut seen_alternatives = HashSet::with_capacity(decision.alternatives().len());
        for alternative in decision.alternatives() {
            validate_non_empty_field(
                alternative,
                "change_set.decision_alternative_empty",
                "decision alternative cannot be empty",
                &mut report,
            );
            validate_max_chars(
                alternative,
                MAX_DECISION_ALTERNATIVE_CHARS,
                "change_set.decision_alternative_too_long",
                "decision alternative exceeds the allowed size",
                &mut report,
            );
            if !seen_alternatives.insert(alternative.trim()) {
                report.push(ValidationIssue::new(
                    "change_set.decision_alternative_duplicate",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "decision point cannot repeat the same alternative",
                ));
            }
        }

        if decision.resolved_alternative().is_some_and(|alternative| {
            !decision
                .alternatives()
                .iter()
                .any(|value| value == alternative)
        }) {
            report.push(ValidationIssue::new(
                "change_set.decision_resolution_unknown",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "resolved alternative must match one of the decision alternatives",
            ));
        }

        if touches_replacement {
            match decision.replacement_target() {
                None => report.push(ValidationIssue::new(
                    "change_set.replacement_target_missing",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "replacement decisions must identify the canon they replace",
                )),
                Some(target) => match state.object_world(target) {
                    None => report.push(ValidationIssue::new(
                        "change_set.replacement_target_unknown",
                        ValidationSeverity::Error,
                        vec![decision_object.clone(), issue_object(target)],
                        "replacement target must exist in the validated snapshot",
                    )),
                    Some(target_world_id) if target_world_id != world_id => {
                        report.push(ValidationIssue::new(
                            "change_set.replacement_target_cross_world",
                            ValidationSeverity::Error,
                            vec![decision_object.clone(), issue_object(target)],
                            "replacement target belongs to another world",
                        ))
                    }
                    Some(_) => {}
                },
            }

            match decision.reason() {
                None => report.push(ValidationIssue::new(
                    "change_set.replacement_reason_missing",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "replacement decisions must include a reason",
                )),
                Some(reason) if reason.chars().count() > MAX_REPLACEMENT_REASON_CHARS => report
                    .push(ValidationIssue::new(
                        "change_set.replacement_reason_too_long",
                        ValidationSeverity::Error,
                        vec![decision_object.clone()],
                        "replacement reason exceeds the allowed size",
                    )),
                Some(_) => {}
            }

            if decision.resolved_alternative().is_none() {
                report.push(ValidationIssue::new(
                    "change_set.replacement_decision_unresolved",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "replacement decisions must be resolved before validation succeeds",
                ));
            }
        }
    }

    for operation_id in replacement_operations {
        if !replacement_decisions.contains(&operation_id) {
            report.push(ValidationIssue::new(
                "change_set.replacement_decision_missing",
                ValidationSeverity::Error,
                vec![IssueObject::new("change_operation", operation_id)],
                "replacement operations require a decision point",
            ));
        } else if !resolved_replacement_decisions.contains(&operation_id) {
            report.push(ValidationIssue::new(
                "change_set.replacement_decision_unresolved",
                ValidationSeverity::Error,
                vec![IssueObject::new("change_operation", operation_id)],
                "replacement operations require a resolved decision point",
            ));
        }
    }

    let future_creations = collect_future_creations(operations);
    let mut written_objects = HashMap::with_capacity(operations.len());
    let mut resulting_state_scope = ResultingStateValidationScope::default();

    for (index, operation) in operations.iter().enumerate() {
        let mut issues = validate_operation(
            index,
            world_id,
            operation,
            &state,
            &future_creations,
            &mut written_objects,
        );
        let can_apply = issues.iter().all(|issue| {
            !matches!(
                issue.severity,
                ValidationSeverity::Error | ValidationSeverity::Conflict
            )
        });
        report.extend(issues.drain(..));
        if can_apply {
            resulting_state_scope.observe(operation);
            state.apply(operation);
        }
    }

    report.extend(validate_resulting_state(&state, &resulting_state_scope));

    for source in sources {
        if !state.contains(*source) {
            report.push(ValidationIssue::new(
                "change_set.source_missing",
                ValidationSeverity::Error,
                vec![issue_object(*source)],
                "source reference does not exist in the validated snapshot",
            ));
        }
    }

    report
}

fn validate_operation(
    index: usize,
    world_id: WorldId,
    operation: &ChangeOperation,
    state: &ValidationState,
    future_creations: &HashMap<ObjectRef, usize>,
    written_objects: &mut HashMap<ObjectRef, ChangeOperationId>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let operation_object = IssueObject::new("change_operation", operation.operation_id());
    let primary_ref = operation.primary_ref();
    let primary_object = issue_object(primary_ref);

    if let Some(previous_operation_id) = written_objects.get(&primary_ref) {
        issues.push(ValidationIssue::new(
            "change_set.operation.double_write",
            ValidationSeverity::Conflict,
            vec![
                IssueObject::new("change_operation", previous_operation_id),
                operation_object.clone(),
                primary_object.clone(),
            ],
            "multiple operations write the same object",
        ));
    } else {
        written_objects.insert(primary_ref, operation.operation_id());
    }

    issues.extend(validate_operation_metadata(
        operation,
        &operation_object,
        &primary_object,
    ));

    match operation {
        ChangeOperation::UpdateWorld { before, after, .. } => {
            if before.id() != world_id || after.id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if before.id() != after.id() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_identity_changed",
                    ValidationSeverity::Error,
                    "update operations must preserve the aggregate id",
                ));
            }
            if operation.expected_version() != 0 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_expected_version",
                    ValidationSeverity::Error,
                    "world updates do not use numeric versions",
                ));
            }
            if before.current_revision() != after.current_revision() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_revision_changed",
                    ValidationSeverity::Error,
                    "world updates must preserve the base revision inside the operation payload",
                ));
            }
        }
        ChangeOperation::CreateEntity { after, .. } => {
            if after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if operation.expected_version() != 0 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_expected_version",
                    ValidationSeverity::Error,
                    "create operations must expect version 0",
                ));
            }
            if after.version() != 1 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_initial_version",
                    ValidationSeverity::Error,
                    "created aggregates must start at version 1",
                ));
            }
            if state.contains(primary_ref) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_exists",
                    ValidationSeverity::Error,
                    "create operation targets an object that already exists",
                ));
            }
            if state.has_entity_slug(after.world_id(), after.slug(), Some(after.id())) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.entity.duplicate_slug",
                    ValidationSeverity::Error,
                    "entity slug must remain unique within its world",
                ));
            }
        }
        ChangeOperation::UpdateEntity { before, after, .. } => {
            if before.world_id() != world_id || after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if before.id() != after.id() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_identity_changed",
                    ValidationSeverity::Error,
                    "update operations must preserve the aggregate id",
                ));
            }
            if operation.expected_version() != before.version() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_expected_version",
                    ValidationSeverity::Error,
                    "update operations must expect the current version",
                ));
            }
            match state.version(primary_ref) {
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_missing",
                    ValidationSeverity::Error,
                    "update operation targets an object that does not exist",
                )),
                Some(actual_version) => {
                    if let Some(issue) = validate_expected_version(
                        primary_object.clone(),
                        actual_version,
                        operation.expected_version(),
                    ) {
                        issues.push(with_operation(issue, operation.operation_id()));
                    }
                }
            }
            match before.version().checked_add(1) {
                Some(next_version) if after.version() != next_version => {
                    issues.push(operation_issue(
                        operation,
                        primary_ref,
                        "change_set.operation.update_version_increment",
                        ValidationSeverity::Error,
                        "updated aggregates must increment version by one",
                    ));
                }
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_version_increment",
                    ValidationSeverity::Error,
                    "updated aggregate version overflowed",
                )),
                Some(_) => {}
            }
            if state.has_entity_slug(after.world_id(), after.slug(), Some(after.id())) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.entity.duplicate_slug",
                    ValidationSeverity::Error,
                    "entity slug must remain unique within its world",
                ));
            }
        }
        ChangeOperation::DeleteEntity { before, .. } => {
            if before.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if operation.expected_version() != before.version() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.delete_expected_version",
                    ValidationSeverity::Error,
                    "delete operations must expect the current version",
                ));
            }
            match state.version(primary_ref) {
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_missing",
                    ValidationSeverity::Error,
                    "delete operation targets an object that does not exist",
                )),
                Some(actual_version) => {
                    if let Some(issue) = validate_expected_version(
                        primary_object.clone(),
                        actual_version,
                        operation.expected_version(),
                    ) {
                        issues.push(with_operation(issue, operation.operation_id()));
                    }
                    let dependents = state.dependents(primary_ref);
                    if !dependents.is_empty() {
                        let mut objects = vec![operation_object.clone(), primary_object.clone()];
                        objects.extend(dependents);
                        issues.push(ValidationIssue::new(
                            "change_set.delete_orphan",
                            ValidationSeverity::Error,
                            objects,
                            "delete operation would leave orphaned references",
                        ));
                    }
                }
            }
        }
        ChangeOperation::CreateRelation { after, .. } => {
            if after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if operation.expected_version() != 0 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_expected_version",
                    ValidationSeverity::Error,
                    "create operations must expect version 0",
                ));
            }
            if after.version() != 1 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_initial_version",
                    ValidationSeverity::Error,
                    "created aggregates must start at version 1",
                ));
            }
            if state.contains(primary_ref) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_exists",
                    ValidationSeverity::Error,
                    "create operation targets an object that already exists",
                ));
            }
            issues.extend(validate_relation_references(
                index,
                operation,
                state,
                future_creations,
                after,
            ));
        }
        ChangeOperation::UpdateRelation { before, after, .. } => {
            if before.world_id() != world_id || after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if before.id() != after.id() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_identity_changed",
                    ValidationSeverity::Error,
                    "update operations must preserve the aggregate id",
                ));
            }
            if operation.expected_version() != before.version() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_expected_version",
                    ValidationSeverity::Error,
                    "update operations must expect the current version",
                ));
            }
            match state.version(primary_ref) {
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_missing",
                    ValidationSeverity::Error,
                    "update operation targets an object that does not exist",
                )),
                Some(actual_version) => {
                    if let Some(issue) = validate_expected_version(
                        primary_object.clone(),
                        actual_version,
                        operation.expected_version(),
                    ) {
                        issues.push(with_operation(issue, operation.operation_id()));
                    }
                }
            }
            match before.version().checked_add(1) {
                Some(next_version) if after.version() != next_version => {
                    issues.push(operation_issue(
                        operation,
                        primary_ref,
                        "change_set.operation.update_version_increment",
                        ValidationSeverity::Error,
                        "updated aggregates must increment version by one",
                    ));
                }
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_version_increment",
                    ValidationSeverity::Error,
                    "updated aggregate version overflowed",
                )),
                Some(_) => {}
            }
            issues.extend(validate_relation_references(
                index,
                operation,
                state,
                future_creations,
                after,
            ));
        }
        ChangeOperation::DeleteRelation { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateEvent { after, .. } => {
            let event = after.event();
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                event.world_id(),
                event.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_events(
                    std::slice::from_ref(event),
                    &state.entity_values(),
                    &state.goal_values(),
                ),
            ));
            let mut scoped_events = state.event_values();
            scoped_events.push(event.clone());
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_event_links(after.links(), &scoped_events),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_event_objects(after),
            );
        }
        ChangeOperation::UpdateEvent { before, after, .. } => {
            let before_event = before.event();
            let after_event = after.event();
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before_event.id() == after_event.id(),
                before_event.world_id(),
                after_event.world_id(),
                before_event.version(),
                after_event.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_events(
                    std::slice::from_ref(after_event),
                    &state.entity_values(),
                    &state.goal_values(),
                ),
            ));
            let mut scoped_events = state.event_values();
            scoped_events.retain(|event| event.id() != before_event.id());
            scoped_events.push(after_event.clone());
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_event_links(after.links(), &scoped_events),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_event_objects(after),
            );
        }
        ChangeOperation::DeleteEvent { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.event().world_id(),
                before.event().version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateGoal { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.world_id(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_goals(std::slice::from_ref(after), &state.entity_values()),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                vec![ObjectRef::Entity(after.holder_entity_id())],
            );
        }
        ChangeOperation::UpdateGoal { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.id() == after.id(),
                before.world_id(),
                after.world_id(),
                before.version(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_goals(std::slice::from_ref(after), &state.entity_values()),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                vec![ObjectRef::Entity(after.holder_entity_id())],
            );
        }
        ChangeOperation::DeleteGoal { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateRule { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.world_id(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_rules(std::slice::from_ref(after)),
            ));
        }
        ChangeOperation::UpdateRule { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.id() == after.id(),
                before.world_id(),
                after.world_id(),
                before.version(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_rules(std::slice::from_ref(after)),
            ));
        }
        ChangeOperation::DeleteRule { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateClaim { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.world_id(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_claims(
                    std::slice::from_ref(after),
                    &state.entity_values(),
                    &state.document_values(),
                    &state.revisions,
                ),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_claim_objects(after),
            );
        }
        ChangeOperation::UpdateClaim { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.id() == after.id(),
                before.world_id(),
                after.world_id(),
                before.version(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_claims(
                    std::slice::from_ref(after),
                    &state.entity_values(),
                    &state.document_values(),
                    &state.revisions,
                ),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_claim_objects(after),
            );
        }
        ChangeOperation::DeleteClaim { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateDocument { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.object().world_id(),
                after.object().version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_documents(std::slice::from_ref(after.object()), &state.entity_values()),
            ));
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_document_references(after, state),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_document_objects(after),
            );
        }
        ChangeOperation::UpdateDocument { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.object().id() == after.object().id(),
                before.object().world_id(),
                after.object().world_id(),
                before.object().version(),
                after.object().version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_documents(std::slice::from_ref(after.object()), &state.entity_values()),
            ));
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_document_references(after, state),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_document_objects(after),
            );
        }
        ChangeOperation::DeleteDocument { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.object().world_id(),
                before.object().version(),
                world_id,
                state,
            );
        }
    }

    issues
}

fn validate_operation_metadata(
    operation: &ChangeOperation,
    operation_object: &IssueObject,
    primary_object: &IssueObject,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if operation.affected_ids().is_empty() {
        issues.push(ValidationIssue::new(
            "change_set.operation.affected_empty",
            ValidationSeverity::Error,
            vec![operation_object.clone(), primary_object.clone()],
            "operation must affect at least one object",
        ));
    }
    if operation.affected_ids().len() > MAX_OPERATION_AFFECTED_IDS {
        issues.push(ValidationIssue::new(
            "change_set.operation.affected_too_many",
            ValidationSeverity::Error,
            vec![operation_object.clone(), primary_object.clone()],
            "operation affects too many objects",
        ));
    }
    if !operation.affected_ids().contains(&operation.primary_ref()) {
        issues.push(ValidationIssue::new(
            "change_set.operation.primary_missing",
            ValidationSeverity::Error,
            vec![operation_object.clone(), primary_object.clone()],
            "operation must include its primary object id in affected_ids",
        ));
    }

    let mut seen = HashSet::with_capacity(operation.affected_ids().len());
    for affected_id in operation.affected_ids() {
        if !seen.insert(*affected_id) {
            issues.push(ValidationIssue::new(
                "change_set.operation.affected_duplicate",
                ValidationSeverity::Error,
                vec![operation_object.clone(), issue_object(*affected_id)],
                "operation repeats an affected object id",
            ));
        }
    }

    match operation.retcon() {
        RetconKind::Additive if is_delete_operation(operation) => {
            issues.push(ValidationIssue::new(
                "change_set.retcon.additive_delete",
                ValidationSeverity::Error,
                vec![operation_object.clone(), primary_object.clone()],
                "additive retcons cannot delete canon",
            ));
        }
        RetconKind::Reinterpretive if is_delete_operation(operation) => {
            issues.push(ValidationIssue::new(
                "change_set.retcon.reinterpretive_delete",
                ValidationSeverity::Error,
                vec![operation_object.clone(), primary_object.clone()],
                "reinterpretive retcons must preserve prior canon",
            ));
        }
        RetconKind::Replacement => {}
        _ => {}
    }

    issues
}

fn is_delete_operation(operation: &ChangeOperation) -> bool {
    matches!(
        operation,
        ChangeOperation::DeleteEntity { .. }
            | ChangeOperation::DeleteRelation { .. }
            | ChangeOperation::DeleteEvent { .. }
            | ChangeOperation::DeleteGoal { .. }
            | ChangeOperation::DeleteRule { .. }
            | ChangeOperation::DeleteClaim { .. }
            | ChangeOperation::DeleteDocument { .. }
    )
}

fn validate_create_target(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
    world_id: WorldId,
    state: &ValidationState,
) {
    if object_world_id != world_id {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.world_mismatch",
            ValidationSeverity::Error,
            "operation world does not match change set world",
        ));
    }
    if operation.expected_version() != 0 {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.create_expected_version",
            ValidationSeverity::Error,
            "create operations must expect version 0",
        ));
    }
    if object_version != 1 {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.create_initial_version",
            ValidationSeverity::Error,
            "created aggregates must start at version 1",
        ));
    }
    if state.contains(primary_ref) {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.target_exists",
            ValidationSeverity::Error,
            "create operation targets an object that already exists",
        ));
    }
}

fn validate_update_target(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    same_identity: bool,
    before_world_id: WorldId,
    after_world_id: WorldId,
    before_version: u64,
    after_version: u64,
    world_id: WorldId,
    state: &ValidationState,
) {
    if before_world_id != world_id || after_world_id != world_id {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.world_mismatch",
            ValidationSeverity::Error,
            "operation world does not match change set world",
        ));
    }
    if !same_identity {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.update_identity_changed",
            ValidationSeverity::Error,
            "update operations must preserve the aggregate id",
        ));
    }
    if operation.expected_version() != before_version {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.update_expected_version",
            ValidationSeverity::Error,
            "update operations must expect the current version",
        ));
    }
    match state.version(primary_ref) {
        None => issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.target_missing",
            ValidationSeverity::Error,
            "update operation targets an object that does not exist",
        )),
        Some(actual_version) => {
            if let Some(issue) = validate_expected_version(
                issue_object(primary_ref),
                actual_version,
                operation.expected_version(),
            ) {
                issues.push(with_operation(issue, operation.operation_id()));
            }
        }
    }
    match before_version.checked_add(1) {
        Some(next_version) if after_version != next_version => {
            issues.push(operation_issue(
                operation,
                primary_ref,
                "change_set.operation.update_version_increment",
                ValidationSeverity::Error,
                "updated aggregates must increment version by one",
            ));
        }
        None => issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.update_version_increment",
            ValidationSeverity::Error,
            "updated aggregate version overflowed",
        )),
        Some(_) => {}
    }
}

fn validate_delete_target(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
    world_id: WorldId,
    state: &ValidationState,
) {
    if object_world_id != world_id {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.world_mismatch",
            ValidationSeverity::Error,
            "operation world does not match change set world",
        ));
    }
    if operation.expected_version() != object_version {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.delete_expected_version",
            ValidationSeverity::Error,
            "delete operations must expect the current version",
        ));
    }
    match state.version(primary_ref) {
        None => issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.target_missing",
            ValidationSeverity::Error,
            "delete operation targets an object that does not exist",
        )),
        Some(actual_version) => {
            if let Some(issue) = validate_expected_version(
                issue_object(primary_ref),
                actual_version,
                operation.expected_version(),
            ) {
                issues.push(with_operation(issue, operation.operation_id()));
            }
            let dependents = state.dependents(primary_ref);
            if !dependents.is_empty() {
                let mut objects = vec![
                    IssueObject::new("change_operation", operation.operation_id()),
                    issue_object(primary_ref),
                ];
                objects.extend(dependents);
                issues.push(ValidationIssue::new(
                    "change_set.delete_orphan",
                    ValidationSeverity::Error,
                    objects,
                    "delete operation would leave orphaned references",
                ));
            }
        }
    }
}

fn validate_relation_references(
    index: usize,
    operation: &ChangeOperation,
    state: &ValidationState,
    future_creations: &HashMap<ObjectRef, usize>,
    relation: &Relation,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let primary_ref = operation.primary_ref();
    let source_ref = ObjectRef::Entity(relation.source_entity_id());
    let target_ref = ObjectRef::Entity(relation.target_entity_id());

    for (entity_ref, missing_code) in [
        (source_ref, "relation.source_missing"),
        (target_ref, "relation.target_missing"),
    ] {
        match state.object_world(entity_ref) {
            None => issues.push(operation_issue(
                operation,
                primary_ref,
                missing_code,
                ValidationSeverity::Error,
                "relation references an entity that does not exist",
            )),
            Some(reference_world_id) if reference_world_id != relation.world_id() => {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "reference.cross_world",
                    ValidationSeverity::Error,
                    "relation references an entity from another world",
                ));
            }
            Some(_) => {}
        }
    }

    if matches!(
        (relation.valid_from_tick(), relation.valid_to_tick()),
        (Some(start), Some(end)) if start > end
    ) {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "period.inverted",
            ValidationSeverity::Error,
            "relation period starts after it ends",
        ));
    }

    report_future_dependencies(
        &mut issues,
        operation,
        index,
        state,
        future_creations,
        vec![source_ref, target_ref],
    );

    issues
}

fn report_future_dependencies(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    index: usize,
    state: &ValidationState,
    future_creations: &HashMap<ObjectRef, usize>,
    references: Vec<ObjectRef>,
) {
    for reference in references {
        if !state.contains(reference)
            && future_creations
                .get(&reference)
                .is_some_and(|future_index| *future_index > index)
        {
            issues.push(ValidationIssue::new(
                "change_set.dependency_order",
                ValidationSeverity::Error,
                vec![
                    IssueObject::new("change_operation", operation.operation_id()),
                    issue_object(operation.primary_ref()),
                    issue_object(reference),
                ],
                "referenced object must exist or be created earlier in the same change set",
            ));
        }
    }
}

fn referenced_event_objects(event: &EventAggregate) -> Vec<ObjectRef> {
    let mut refs = Vec::new();
    if let Some(location_id) = event.event().location_entity_id() {
        refs.push(ObjectRef::Entity(location_id));
    }
    refs.extend(
        event
            .event()
            .participants()
            .iter()
            .map(|participant| ObjectRef::Entity(participant.entity_id())),
    );
    refs.extend(
        event
            .event()
            .affected_goal_ids()
            .iter()
            .copied()
            .map(ObjectRef::Goal),
    );
    refs.extend(
        event
            .links()
            .iter()
            .map(|link| ObjectRef::Event(link.target_event_id())),
    );
    refs
}

fn referenced_claim_objects(claim: &Claim) -> Vec<ObjectRef> {
    let mut refs = vec![ObjectRef::Entity(claim.subject_entity_id())];
    if let Some(holder_id) = claim.holder_entity_id() {
        refs.push(ObjectRef::Entity(holder_id));
    }
    if let Some(crate::claim::ClaimObject::Entity(entity_id)) = claim.object() {
        refs.push(ObjectRef::Entity(*entity_id));
    }
    if let Some(document_id) = claim.source_document_id() {
        refs.push(ObjectRef::Document(document_id));
    }
    if let Some(claim_id) = claim.source_claim_id() {
        refs.push(ObjectRef::Claim(claim_id));
    }
    refs
}

fn validate_document_references(
    aggregate: &DocumentAggregate,
    state: &ValidationState,
) -> Vec<ValidationIssue> {
    let document = aggregate.object();
    let source = ObjectRef::Document(document.id());
    let mut ordinals = HashSet::new();
    let mut issues = Vec::new();

    for reference in aggregate.references() {
        let objects = vec![issue_object(source), issue_object(reference.target())];
        if reference.source() != source {
            issues.push(ValidationIssue::new(
                "content_reference.source_invalid",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference source must match its document",
            ));
        }
        match state.object_world(reference.target()) {
            None => issues.push(ValidationIssue::new(
                "content_reference.target_missing",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference target does not exist",
            )),
            Some(target_world) if target_world != document.world_id() => {
                issues.push(ValidationIssue::new(
                    "reference.cross_world",
                    ValidationSeverity::Error,
                    objects.clone(),
                    "content reference crosses world boundaries",
                ));
            }
            Some(_) => {}
        }
        if !ordinals.insert(reference.ordinal()) {
            issues.push(ValidationIssue::new(
                "content_reference.ordinal_duplicate",
                ValidationSeverity::Error,
                objects,
                format!(
                    "content ordinal {} is duplicated for its source",
                    reference.ordinal()
                ),
            ));
        }
    }

    issues
}

fn referenced_document_objects(document: &DocumentAggregate) -> Vec<ObjectRef> {
    let document_object = document.object();
    let mut refs = Vec::new();
    if let Some(author_id) = document_object.author_entity_id() {
        refs.push(ObjectRef::Entity(author_id));
    }
    if let Some(perspective_id) = document_object.perspective_entity_id() {
        refs.push(ObjectRef::Entity(perspective_id));
    }
    refs.extend(document_refs_targets(document));
    refs
}

fn document_refs_targets(document: &DocumentAggregate) -> impl Iterator<Item = ObjectRef> + '_ {
    document
        .references()
        .iter()
        .map(|reference| reference.target())
}

fn collect_future_creations(operations: &[ChangeOperation]) -> HashMap<ObjectRef, usize> {
    let mut created = HashMap::new();
    for (index, operation) in operations.iter().enumerate() {
        if let Some(created_ref) = operation.created_ref() {
            created.entry(created_ref).or_insert(index);
        }
    }
    created
}

fn with_operation(mut issue: ValidationIssue, operation_id: ChangeOperationId) -> ValidationIssue {
    let operation_object = IssueObject::new("change_operation", operation_id);
    if !issue.objects.contains(&operation_object) {
        issue.objects.insert(0, operation_object);
    }
    issue
}

fn with_operation_issues(
    operation_id: ChangeOperationId,
    issues: impl IntoIterator<Item = ValidationIssue>,
) -> Vec<ValidationIssue> {
    issues
        .into_iter()
        .map(|issue| with_operation(issue, operation_id))
        .collect()
}

fn operation_issue(
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    code: &'static str,
    severity: ValidationSeverity,
    message: impl Into<String>,
) -> ValidationIssue {
    ValidationIssue::new(
        code,
        severity,
        vec![
            IssueObject::new("change_operation", operation.operation_id()),
            issue_object(primary_ref),
        ],
        message,
    )
}

fn issue_object(object: ObjectRef) -> IssueObject {
    IssueObject::new(object.kind(), object.to_string())
}

fn validate_non_empty_field(
    value: &str,
    code: &'static str,
    message: &'static str,
    report: &mut ValidationReport,
) {
    if value.trim().is_empty() {
        report.push(ValidationIssue::new(
            code,
            ValidationSeverity::Error,
            vec![],
            message,
        ));
    }
}

fn validate_max_chars(
    value: &str,
    max_chars: usize,
    code: &'static str,
    message: &'static str,
    report: &mut ValidationReport,
) {
    if value.chars().count() > max_chars {
        report.push(ValidationIssue::new(
            code,
            ValidationSeverity::Error,
            vec![],
            message,
        ));
    }
}

fn validate_max_items(
    actual: usize,
    max_items: usize,
    code: &'static str,
    message: &'static str,
    report: &mut ValidationReport,
) {
    if actual > max_items {
        report.push(ValidationIssue::new(
            code,
            ValidationSeverity::Error,
            vec![],
            message,
        ));
    }
}

struct ValidationState {
    world_id: WorldId,
    entities: HashMap<crate::EntityId, Entity>,
    relations: HashMap<crate::RelationId, Relation>,
    goals: HashMap<crate::GoalId, Goal>,
    events: HashMap<crate::EventId, Event>,
    event_links: Vec<EventLink>,
    rules: HashMap<crate::RuleId, Rule>,
    claims: HashMap<crate::ClaimId, Claim>,
    documents: HashMap<crate::DocumentId, Document>,
    content_references: Vec<ContentReference>,
    revisions: HashSet<RevisionId>,
}

impl ValidationState {
    fn from_snapshot(
        snapshot: &ChangeSetValidationSnapshot<'_>,
        world_id: WorldId,
        base_revision: RevisionId,
    ) -> Self {
        let mut revisions = snapshot.revisions.iter().copied().collect::<HashSet<_>>();
        revisions.insert(base_revision);

        Self {
            world_id,
            entities: snapshot
                .entities
                .iter()
                .cloned()
                .map(|entity| (entity.id(), entity))
                .collect(),
            relations: snapshot
                .relations
                .iter()
                .cloned()
                .map(|relation| (relation.id(), relation))
                .collect(),
            goals: snapshot
                .goals
                .iter()
                .cloned()
                .map(|goal| (goal.id(), goal))
                .collect(),
            events: snapshot
                .events
                .iter()
                .cloned()
                .map(|event| (event.id(), event))
                .collect(),
            event_links: snapshot.event_links.to_vec(),
            rules: snapshot
                .rules
                .iter()
                .cloned()
                .map(|rule| (rule.id(), rule))
                .collect(),
            claims: snapshot
                .claims
                .iter()
                .cloned()
                .map(|claim| (claim.id(), claim))
                .collect(),
            documents: snapshot
                .documents
                .iter()
                .cloned()
                .map(|document| (document.id(), document))
                .collect(),
            content_references: snapshot.content_references.to_vec(),
            revisions,
        }
    }

    fn contains(&self, object: ObjectRef) -> bool {
        match object {
            ObjectRef::World(id) => id == self.world_id,
            ObjectRef::Entity(id) => self.entities.contains_key(&id),
            ObjectRef::Relation(id) => self.relations.contains_key(&id),
            ObjectRef::Event(id) => self.events.contains_key(&id),
            ObjectRef::Claim(id) => self.claims.contains_key(&id),
            ObjectRef::Rule(id) => self.rules.contains_key(&id),
            ObjectRef::Goal(id) => self.goals.contains_key(&id),
            ObjectRef::Document(id) => self.documents.contains_key(&id),
        }
    }

    fn version(&self, object: ObjectRef) -> Option<u64> {
        match object {
            ObjectRef::World(_) => None,
            ObjectRef::Entity(id) => self.entities.get(&id).map(Entity::version),
            ObjectRef::Relation(id) => self.relations.get(&id).map(Relation::version),
            ObjectRef::Event(id) => self.events.get(&id).map(Event::version),
            ObjectRef::Claim(id) => self.claims.get(&id).map(Claim::version),
            ObjectRef::Rule(id) => self.rules.get(&id).map(Rule::version),
            ObjectRef::Goal(id) => self.goals.get(&id).map(Goal::version),
            ObjectRef::Document(id) => self.documents.get(&id).map(Document::version),
        }
    }

    fn object_world(&self, object: ObjectRef) -> Option<WorldId> {
        match object {
            ObjectRef::World(id) => (id == self.world_id).then_some(self.world_id),
            ObjectRef::Entity(id) => self.entities.get(&id).map(Entity::world_id),
            ObjectRef::Relation(id) => self.relations.get(&id).map(Relation::world_id),
            ObjectRef::Event(id) => self.events.get(&id).map(Event::world_id),
            ObjectRef::Claim(id) => self.claims.get(&id).map(Claim::world_id),
            ObjectRef::Rule(id) => self.rules.get(&id).map(Rule::world_id),
            ObjectRef::Goal(id) => self.goals.get(&id).map(Goal::world_id),
            ObjectRef::Document(id) => self.documents.get(&id).map(Document::world_id),
        }
    }

    fn has_entity_slug(
        &self,
        world_id: WorldId,
        slug: &str,
        ignore_id: Option<crate::EntityId>,
    ) -> bool {
        self.entities.values().any(|entity| {
            entity.world_id() == world_id && entity.slug() == slug && Some(entity.id()) != ignore_id
        })
    }

    fn entity_values(&self) -> Vec<Entity> {
        self.entities.values().cloned().collect()
    }

    fn goal_values(&self) -> Vec<Goal> {
        self.goals.values().cloned().collect()
    }

    fn event_values(&self) -> Vec<Event> {
        self.events.values().cloned().collect()
    }

    fn document_values(&self) -> Vec<Document> {
        self.documents.values().cloned().collect()
    }

    fn claim_values(&self) -> Vec<Claim> {
        self.claims.values().cloned().collect()
    }

    fn dependents(&self, target: ObjectRef) -> Vec<IssueObject> {
        let mut dependents = HashSet::new();

        match target {
            ObjectRef::World(_) => {}
            ObjectRef::Entity(entity_id) => {
                for relation in self.relations.values() {
                    if relation.source_entity_id() == entity_id
                        || relation.target_entity_id() == entity_id
                    {
                        dependents.insert(IssueObject::new("relation", relation.id()));
                    }
                }
                for goal in self.goals.values() {
                    if goal.holder_entity_id() == entity_id {
                        dependents.insert(IssueObject::new("goal", goal.id()));
                    }
                }
                for event in self.events.values() {
                    if event.location_entity_id() == Some(entity_id)
                        || event
                            .participants()
                            .iter()
                            .any(|participant| participant.entity_id() == entity_id)
                    {
                        dependents.insert(IssueObject::new("event", event.id()));
                    }
                }
                for claim in self.claims.values() {
                    let object_entity = match claim.object() {
                        Some(crate::claim::ClaimObject::Entity(object_id)) => Some(*object_id),
                        _ => None,
                    };
                    if claim.subject_entity_id() == entity_id
                        || claim.holder_entity_id() == Some(entity_id)
                        || object_entity == Some(entity_id)
                    {
                        dependents.insert(IssueObject::new("claim", claim.id()));
                    }
                }
                for document in self.documents.values() {
                    if document.author_entity_id() == Some(entity_id)
                        || document.perspective_entity_id() == Some(entity_id)
                    {
                        dependents.insert(IssueObject::new("document", document.id()));
                    }
                }
            }
            ObjectRef::Goal(goal_id) => {
                for event in self.events.values() {
                    if event.affected_goal_ids().contains(&goal_id) {
                        dependents.insert(IssueObject::new("event", event.id()));
                    }
                }
            }
            ObjectRef::Document(document_id) => {
                for claim in self.claims.values() {
                    if claim.source_document_id() == Some(document_id) {
                        dependents.insert(IssueObject::new("claim", claim.id()));
                    }
                }
            }
            ObjectRef::Claim(claim_id) => {
                for claim in self.claims.values() {
                    if claim.source_claim_id() == Some(claim_id) {
                        dependents.insert(IssueObject::new("claim", claim.id()));
                    }
                }
            }
            ObjectRef::Relation(_) | ObjectRef::Rule(_) => {}
            ObjectRef::Event(event_id) => {
                for link in &self.event_links {
                    if link.target_event_id() == event_id {
                        dependents.insert(IssueObject::new("event", link.source_event_id()));
                    }
                }
            }
        }

        for reference in &self.content_references {
            if reference.target() == target {
                dependents.insert(issue_object(reference.source()));
            }
        }

        let mut dependents: Vec<_> = dependents.into_iter().collect();
        dependents.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.id.cmp(&right.id))
        });
        dependents
    }

    fn apply(&mut self, operation: &ChangeOperation) {
        if is_delete_operation(operation) {
            let deleted_ref = operation.primary_ref();
            self.content_references.retain(|reference| {
                reference.source() != deleted_ref && reference.target() != deleted_ref
            });
        }

        match operation {
            ChangeOperation::UpdateWorld { .. } => {}
            ChangeOperation::CreateEntity { after, .. }
            | ChangeOperation::UpdateEntity { after, .. } => {
                self.entities.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteEntity { before, .. } => {
                self.entities.remove(&before.id());
            }
            ChangeOperation::CreateRelation { after, .. }
            | ChangeOperation::UpdateRelation { after, .. } => {
                self.relations.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteRelation { before, .. } => {
                self.relations.remove(&before.id());
            }
            ChangeOperation::CreateEvent { after, .. }
            | ChangeOperation::UpdateEvent { after, .. } => {
                self.events
                    .insert(after.event().id(), after.event().clone());
                self.event_links
                    .retain(|link| link.source_event_id() != after.event().id());
                self.event_links.extend(after.links().iter().cloned());
            }
            ChangeOperation::DeleteEvent { before, .. } => {
                self.events.remove(&before.event().id());
                self.event_links.retain(|link| {
                    link.source_event_id() != before.event().id()
                        && link.target_event_id() != before.event().id()
                });
            }
            ChangeOperation::CreateGoal { after, .. }
            | ChangeOperation::UpdateGoal { after, .. } => {
                self.goals.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteGoal { before, .. } => {
                self.goals.remove(&before.id());
            }
            ChangeOperation::CreateRule { after, .. }
            | ChangeOperation::UpdateRule { after, .. } => {
                self.rules.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteRule { before, .. } => {
                self.rules.remove(&before.id());
            }
            ChangeOperation::CreateClaim { after, .. }
            | ChangeOperation::UpdateClaim { after, .. } => {
                self.claims.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteClaim { before, .. } => {
                self.claims.remove(&before.id());
            }
            ChangeOperation::CreateDocument { after, .. }
            | ChangeOperation::UpdateDocument { after, .. } => {
                let source = ObjectRef::Document(after.object().id());
                self.documents
                    .insert(after.object().id(), after.object().clone());
                self.content_references
                    .retain(|reference| reference.source() != source);
                self.content_references
                    .extend(after.references().iter().cloned());
            }
            ChangeOperation::DeleteDocument { before, .. } => {
                self.documents.remove(&before.object().id());
            }
        }
    }
}

#[derive(Default)]
struct ResultingStateValidationScope {
    entity_ids: HashSet<crate::EntityId>,
    event_ids: HashSet<crate::EventId>,
    scan_all_claims: bool,
    scan_all_lifecycles: bool,
}

impl ResultingStateValidationScope {
    fn observe(&mut self, operation: &ChangeOperation) {
        match operation {
            ChangeOperation::UpdateWorld { .. } => {}
            ChangeOperation::CreateEntity { after, .. }
            | ChangeOperation::UpdateEntity { after, .. } => {
                self.entity_ids.insert(after.id());
            }
            ChangeOperation::DeleteEntity { before, .. } => {
                self.entity_ids.insert(before.id());
            }
            ChangeOperation::CreateEvent { after, .. } => self.track_event(after.event()),
            ChangeOperation::UpdateEvent { before, after, .. } => {
                self.track_event(before.event());
                self.track_event(after.event());
            }
            ChangeOperation::DeleteEvent { before, .. } => self.track_event(before.event()),
            ChangeOperation::CreateClaim { .. }
            | ChangeOperation::UpdateClaim { .. }
            | ChangeOperation::DeleteClaim { .. } => {
                self.scan_all_claims = true;
            }
            ChangeOperation::CreateRule { after, .. }
            | ChangeOperation::UpdateRule { after, .. } => {
                if after.can_produce_hard_error()
                    && matches!(
                        after.validator_kind(),
                        Some(RuleValidatorKind::NoResurrection)
                    )
                {
                    self.scan_all_lifecycles = true;
                }
            }
            _ => {}
        }
    }

    fn track_event(&mut self, event: &Event) {
        self.event_ids.insert(event.id());
        self.entity_ids.extend(
            event
                .participants()
                .iter()
                .map(|participant| participant.entity_id()),
        );
    }
}

fn validate_resulting_state(
    state: &ValidationState,
    scope: &ResultingStateValidationScope,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if scope.scan_all_claims {
        issues.extend(validate_claims(
            &state.claim_values(),
            &state.entity_values(),
            &state.document_values(),
            &state.revisions,
        ));
    }

    if !scope.event_ids.is_empty() {
        let events = state.event_values();
        let relevant_links: Vec<_> = state
            .event_links
            .iter()
            .filter(|link| {
                scope.event_ids.contains(&link.source_event_id())
                    || scope.event_ids.contains(&link.target_event_id())
            })
            .cloned()
            .collect();
        issues.extend(validate_event_links(&relevant_links, &events));
    }

    let no_resurrection_rules: Vec<_> = state
        .rules
        .values()
        .filter(|rule| {
            rule.can_produce_hard_error()
                && matches!(
                    rule.validator_kind(),
                    Some(RuleValidatorKind::NoResurrection)
                )
        })
        .collect();
    if no_resurrection_rules.is_empty() && scope.entity_ids.is_empty() && !scope.scan_all_lifecycles
    {
        return issues;
    }

    let entity_ids: Vec<_> = if scope.scan_all_lifecycles {
        state.entities.keys().copied().collect()
    } else {
        scope.entity_ids.iter().copied().collect()
    };
    for entity_id in entity_ids {
        let Some(entity) = state.entities.get(&entity_id) else {
            continue;
        };
        if entity.kind() != EntityKind::Person {
            continue;
        }

        let lifecycle = collect_entity_lifecycle(entity, state.events.values());
        issues.extend(validate_lifecycle(
            entity,
            lifecycle.birth,
            lifecycle.death,
            &lifecycle.participations,
        ));

        if let Some(death) = lifecycle.death {
            for rule in &no_resurrection_rules {
                issues.extend(validate_no_resurrection(
                    rule,
                    entity,
                    death,
                    &lifecycle.participations,
                ));
            }
        }
    }

    issues
}

struct EntityLifecycle<'a> {
    birth: Option<&'a Event>,
    death: Option<&'a Event>,
    participations: Vec<&'a Event>,
}

fn collect_entity_lifecycle<'a>(
    entity: &Entity,
    events: impl Iterator<Item = &'a Event>,
) -> EntityLifecycle<'a> {
    let mut births = Vec::new();
    let mut deaths = Vec::new();
    let mut participations = Vec::new();

    for event in events {
        if !event
            .participants()
            .iter()
            .any(|participant| participant.entity_id() == entity.id())
        {
            continue;
        }

        participations.push(event);
        if event.kind().eq_ignore_ascii_case("birth")
            && lifecycle_subject_matches(event, entity.id(), &["subject", "born", "child"])
        {
            births.push(event);
        }
        if event.kind().eq_ignore_ascii_case("death")
            && lifecycle_subject_matches(event, entity.id(), &["subject", "deceased", "victim"])
        {
            deaths.push(event);
        }
    }

    EntityLifecycle {
        birth: unique_event(births),
        death: unique_event(deaths),
        participations,
    }
}

fn lifecycle_subject_matches(event: &Event, entity_id: crate::EntityId, roles: &[&str]) -> bool {
    if let Some(participant) = event.participants().iter().find(|participant| {
        roles
            .iter()
            .any(|role| participant.role().eq_ignore_ascii_case(role))
    }) {
        return participant.entity_id() == entity_id;
    }

    event.participants().len() == 1 && event.participants()[0].entity_id() == entity_id
}

fn unique_event(events: Vec<&Event>) -> Option<&Event> {
    match events.as_slice() {
        [event] => Some(*event),
        _ => None,
    }
}

fn validate_operations_and_decisions(
    world_id: WorldId,
    operations: &[ChangeOperation],
    decisions: &[DecisionPoint],
) -> Result<(), DomainError> {
    let mut operation_ids = HashSet::with_capacity(operations.len());
    let mut replacement_operations = HashSet::new();

    for operation in operations {
        if !operation_ids.insert(operation.operation_id()) {
            return Err(DomainError::DuplicateChangeOperationId(
                operation.operation_id(),
            ));
        }
        operation.validate(world_id)?;
        if operation.retcon() == RetconKind::Replacement {
            replacement_operations.insert(operation.operation_id());
        }
    }

    let mut decision_point_ids = HashSet::with_capacity(decisions.len());
    let mut replacement_decisions = HashSet::new();

    for decision in decisions {
        if !decision_point_ids.insert(decision.decision_point_id()) {
            return Err(DomainError::DuplicateDecisionPointId(
                decision.decision_point_id(),
            ));
        }
        decision.validate(&operation_ids)?;
        for operation_id in decision.operation_ids() {
            if replacement_operations.contains(operation_id) {
                replacement_decisions.insert(*operation_id);
            }
        }
    }

    for operation_id in replacement_operations {
        if !replacement_decisions.contains(&operation_id) {
            return Err(DomainError::InvalidChangeSetContext(
                "replacement operations require a decision point",
            ));
        }
    }

    Ok(())
}

fn validate_world_update(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    before: &World,
    after: &World,
) -> Result<(), DomainError> {
    if before.id() != world_id || after.id() != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if before.id() != after.id() {
        return Err(DomainError::InvalidChangeSetContext(
            "update operations must preserve the aggregate id",
        ));
    }
    if expected_version != 0 {
        return Err(DomainError::InvalidChangeSetContext(
            "world updates do not use numeric versions",
        ));
    }
    if before.current_revision() != after.current_revision() {
        return Err(DomainError::InvalidChangeSetContext(
            "world updates must preserve the base revision during draft construction",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if !affected_ids.contains(&ObjectRef::World(after.id())) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_create(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
) -> Result<(), DomainError> {
    if object_world_id != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if expected_version != 0 {
        return Err(DomainError::InvalidChangeSetContext(
            "create operations must expect version 0",
        ));
    }
    if object_version != 1 {
        return Err(DomainError::InvalidChangeSetContext(
            "created aggregates must start at version 1",
        ));
    }
    if !affected_ids.contains(&primary_ref) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_update(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    before_ref: ObjectRef,
    before_world_id: WorldId,
    before_version: u64,
    after_ref: ObjectRef,
    after_world_id: WorldId,
    after_version: u64,
) -> Result<(), DomainError> {
    if before_world_id != world_id || after_world_id != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if before_ref != after_ref {
        return Err(DomainError::InvalidChangeSetContext(
            "update operations must preserve the aggregate id",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if expected_version != before_version {
        return Err(DomainError::InvalidChangeSetContext(
            "expected version must match the current version",
        ));
    }
    let next_version = before_version
        .checked_add(1)
        .ok_or(DomainError::VersionOverflow)?;
    if after_version != next_version {
        return Err(DomainError::InvalidChangeSetContext(
            "updated aggregates must increment version by one",
        ));
    }
    if !affected_ids.contains(&after_ref) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_delete(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
) -> Result<(), DomainError> {
    if object_world_id != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if expected_version != object_version {
        return Err(DomainError::InvalidChangeSetContext(
            "expected version must match the current version",
        ));
    }
    if !affected_ids.contains(&primary_ref) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_affected_ids(affected_ids: &[ObjectRef]) -> Result<(), DomainError> {
    let mut seen = HashSet::with_capacity(affected_ids.len());
    for affected_id in affected_ids {
        if !seen.insert(*affected_id) {
            return Err(DomainError::InvalidChangeSetContext(
                "an operation cannot repeat an affected id",
            ));
        }
    }
    Ok(())
}

fn normalize_strings(field: &'static str, values: Vec<String>) -> Result<Vec<String>, DomainError> {
    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::with_capacity(values.len());

    for value in values {
        let value = required(field, value)?;
        if !seen.insert(value.clone()) {
            return Err(DomainError::InvalidChangeSetContext(
                "duplicate string values are not allowed",
            ));
        }
        normalized.push(value);
    }

    Ok(normalized)
}

impl DecisionPoint {
    fn validate(&self, operation_ids: &HashSet<ChangeOperationId>) -> Result<(), DomainError> {
        for operation_id in &self.operation_ids {
            if !operation_ids.contains(operation_id) {
                return Err(DomainError::InvalidChangeSetContext(
                    "a decision point references an unknown operation",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityKind;
    use crate::event::{EventAggregate, EventLinkKind, EventParticipant};
    use crate::goal::{GoalStatus, GoalVisibility};
    use crate::relation::RelationDirection;
    use crate::rule::{RuleKind, RuleSeverity};
    use crate::time::{Certainty, EventTime, TimePrecision};
    use serde_json::Value;

    fn create_entity_operation(world_id: WorldId) -> ChangeOperation {
        let entity = Entity::new(
            world_id,
            EntityKind::Person,
            "Mara",
            "mara",
            "",
            "",
            "{}",
            vec![],
            1,
        )
        .expect("entity");

        ChangeOperation::CreateEntity {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(entity.id())],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: entity,
        }
    }

    fn entity_with(
        world_id: WorldId,
        id: crate::EntityId,
        name: &str,
        slug: &str,
        version: u64,
    ) -> Entity {
        Entity::restore(
            id,
            world_id,
            EntityKind::Person,
            name,
            slug,
            "",
            "",
            "{}",
            vec![],
            version,
            1,
            1,
        )
        .expect("entity")
    }

    fn create_goal_operation(
        world_id: WorldId,
        holder_entity_id: crate::EntityId,
    ) -> ChangeOperation {
        let goal = Goal::new(
            world_id,
            holder_entity_id,
            "Protect the gate",
            1,
            GoalStatus::Active,
            None,
            GoalVisibility::Secret,
            None,
        )
        .expect("goal");

        ChangeOperation::CreateGoal {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![
                ObjectRef::Goal(goal.id()),
                ObjectRef::Entity(holder_entity_id),
            ],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: goal,
        }
    }

    fn update_world_operation(
        world: &World,
        name: &str,
        premise_md: &str,
        epoch_label: &str,
    ) -> ChangeOperation {
        let after = World::restore(
            world.id(),
            name,
            premise_md,
            epoch_label,
            world.current_revision(),
            world.created_at_ms(),
            world.updated_at_ms() + 1,
        )
        .expect("updated world");

        ChangeOperation::UpdateWorld {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::World(world.id())],
            expected_version: 0,
            retcon: RetconKind::Reinterpretive,
            before: world.clone(),
            after,
        }
    }

    fn update_entity_operation(
        before: &Entity,
        after_name: &str,
        after_slug: &str,
    ) -> ChangeOperation {
        let after = Entity::restore(
            before.id(),
            before.world_id(),
            before.kind(),
            after_name,
            after_slug,
            before.summary(),
            before.body_md(),
            before.attributes_json().as_str(),
            before.aliases().to_vec(),
            before.version() + 1,
            before.created_at_ms(),
            before.updated_at_ms() + 1,
        )
        .expect("updated entity");

        ChangeOperation::UpdateEntity {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(before.id())],
            expected_version: before.version(),
            retcon: RetconKind::Additive,
            before: before.clone(),
            after,
        }
    }

    fn delete_entity_operation(entity: &Entity) -> ChangeOperation {
        ChangeOperation::DeleteEntity {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(entity.id())],
            expected_version: entity.version(),
            retcon: RetconKind::Additive,
            before: entity.clone(),
        }
    }

    fn snapshot<'a>(
        entities: &'a [Entity],
        relations: &'a [Relation],
    ) -> ChangeSetValidationSnapshot<'a> {
        ChangeSetValidationSnapshot {
            entities,
            relations,
            goals: &[],
            events: &[],
            event_links: &[],
            rules: &[],
            claims: &[],
            documents: &[],
            content_references: &[],
            revisions: &[],
        }
    }

    fn snapshot_with_events<'a>(
        entities: &'a [Entity],
        events: &'a [Event],
        event_links: &'a [EventLink],
        rules: &'a [Rule],
    ) -> ChangeSetValidationSnapshot<'a> {
        ChangeSetValidationSnapshot {
            entities,
            relations: &[],
            goals: &[],
            events,
            event_links,
            rules,
            claims: &[],
            documents: &[],
            content_references: &[],
            revisions: &[],
        }
    }

    fn event_with(
        world_id: WorldId,
        id: crate::EventId,
        kind: &str,
        tick: Option<i64>,
        participants: Vec<EventParticipant>,
        version: u64,
    ) -> Event {
        let time = tick.map_or_else(
            || EventTime::unknown(Certainty::Uncertain),
            |tick| EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
        );
        Event::restore(
            id,
            world_id,
            kind,
            "",
            "",
            time,
            None,
            participants,
            vec![],
            version,
            1,
            1,
        )
        .expect("event")
    }

    fn create_event_operation(after: Event) -> ChangeOperation {
        ChangeOperation::CreateEvent {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Event(after.id())],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: EventAggregate::new(after, vec![]),
        }
    }

    fn create_event_operation_with_links(after: Event, links: Vec<EventLink>) -> ChangeOperation {
        let mut affected_ids = vec![ObjectRef::Event(after.id())];
        affected_ids.extend(links.iter().flat_map(|link| {
            [
                ObjectRef::Event(link.source_event_id()),
                ObjectRef::Event(link.target_event_id()),
            ]
        }));
        affected_ids.sort_unstable();
        affected_ids.dedup();

        ChangeOperation::CreateEvent {
            operation_id: ChangeOperationId::new(),
            affected_ids,
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: EventAggregate::new(after, links),
        }
    }

    fn update_event_operation_with_links(
        before: &Event,
        tick: i64,
        links: Vec<EventLink>,
    ) -> ChangeOperation {
        let after = Event::restore(
            before.id(),
            before.world_id(),
            before.kind(),
            before.summary(),
            before.body_md(),
            EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
            before.location_entity_id(),
            before.participants().to_vec(),
            before.affected_goal_ids().to_vec(),
            before.version() + 1,
            before.created_at_ms(),
            before.updated_at_ms() + 1,
        )
        .expect("updated event");

        let mut affected_ids = vec![ObjectRef::Event(before.id())];
        affected_ids.extend(links.iter().flat_map(|link| {
            [
                ObjectRef::Event(link.source_event_id()),
                ObjectRef::Event(link.target_event_id()),
            ]
        }));
        affected_ids.sort_unstable();
        affected_ids.dedup();

        ChangeOperation::UpdateEvent {
            operation_id: ChangeOperationId::new(),
            affected_ids,
            expected_version: before.version(),
            retcon: RetconKind::Additive,
            before: EventAggregate::new(before.clone(), links.clone()),
            after: EventAggregate::new(after, links),
        }
    }

    fn no_resurrection_rule(world_id: WorldId) -> Rule {
        Rule::new(
            world_id,
            RuleKind::Constitutive,
            "The dead do not return.",
            "world",
            RuleSeverity::Hard,
            None,
            Some(RuleValidatorKind::NoResurrection),
            "{}",
            1,
        )
        .expect("rule")
    }

    #[test]
    fn round_trips_changeset_variants() {
        let world_id = WorldId::new();
        let base_revision = RevisionId::new();
        let world = World::restore(
            world_id,
            "Arcadia",
            "Original premise",
            "First Dawn",
            base_revision,
            1,
            1,
        )
        .expect("world");
        let link_target = crate::EventId::new();
        let linked_event = event_with(world_id, crate::EventId::new(), "storm", Some(5), vec![], 1);
        let link = EventLink::new(linked_event.id(), link_target, EventLinkKind::Causes)
            .expect("causal link");
        let event_operation = create_event_operation_with_links(linked_event, vec![link]);
        let draft = ChangeSetDraft::new(
            world_id,
            base_revision,
            "Add Mara",
            vec![],
            vec!["Mara is new".to_owned()],
            vec![
                update_world_operation(&world, "Arcadia Revised", "Updated premise", "Second Dawn"),
                event_operation.clone(),
            ],
            vec![],
        )
        .expect("draft");
        let draft_json = serde_json::to_string(&draft).expect("serialize draft");
        let restored_draft: ChangeSetDraft =
            serde_json::from_str(&draft_json).expect("deserialize draft");
        assert_eq!(restored_draft, draft);

        let change_set = ChangeSet::new(
            world_id,
            base_revision,
            "Add Mara",
            vec![],
            vec!["Mara is new".to_owned()],
            vec![
                update_world_operation(&world, "Arcadia Revised", "Updated premise", "Second Dawn"),
                event_operation,
            ],
            vec![],
        )
        .expect("change set");
        let change_set_json = serde_json::to_string(&change_set).expect("serialize change set");
        let restored_change_set: ChangeSet =
            serde_json::from_str(&change_set_json).expect("deserialize change set");
        assert_eq!(restored_change_set, change_set);
    }

    #[test]
    fn rejects_unknown_fields_and_missing_required_fields() {
        let world_id = WorldId::new();
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Add Mara",
            vec![],
            vec![],
            vec![create_entity_operation(world_id)],
            vec![],
        )
        .expect("draft");

        let mut value = serde_json::to_value(&draft).expect("value");
        value
            .as_object_mut()
            .expect("object")
            .insert("unexpected".to_owned(), Value::Bool(true));
        assert!(serde_json::from_value::<ChangeSetDraft>(value).is_err());

        let mut value = serde_json::to_value(&draft).expect("value");
        value.as_object_mut().expect("object").remove("objective");
        assert!(serde_json::from_value::<ChangeSetDraft>(value).is_err());
    }

    #[test]
    fn rejects_cross_world_operations() {
        let world_id = WorldId::new();
        let other_world_id = WorldId::new();

        let result = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Add Mara",
            vec![],
            vec![],
            vec![create_entity_operation(other_world_id)],
            vec![],
        );

        assert_eq!(
            result,
            Err(DomainError::InvalidChangeSetContext(
                "operation world does not match change set world"
            ))
        );
    }

    #[test]
    fn rejects_world_updates_that_mutate_revision_or_expected_version() {
        let world_id = WorldId::new();
        let before = World::restore(
            world_id,
            "Arcadia",
            "",
            "First Dawn",
            RevisionId::new(),
            1,
            1,
        )
        .expect("world");
        let after = World::restore(
            world_id,
            "Arcadia",
            "",
            "First Dawn",
            RevisionId::new(),
            1,
            2,
        )
        .expect("world");

        let result = ChangeSetDraft::new(
            world_id,
            before.current_revision(),
            "Invalid world update",
            vec![],
            vec![],
            vec![ChangeOperation::UpdateWorld {
                operation_id: ChangeOperationId::new(),
                affected_ids: vec![ObjectRef::World(world_id)],
                expected_version: 1,
                retcon: RetconKind::Reinterpretive,
                before,
                after,
            }],
            vec![],
        );

        assert_eq!(
            result,
            Err(DomainError::InvalidChangeSetContext(
                "world updates do not use numeric versions"
            ))
        );
    }

    #[test]
    fn reports_cross_world_event_links_in_resulting_state() {
        let world_id = WorldId::new();
        let other_world_id = WorldId::new();
        let local_event = event_with(world_id, crate::EventId::new(), "storm", Some(5), vec![], 1);
        let foreign_event = event_with(
            other_world_id,
            crate::EventId::new(),
            "aftermath",
            Some(6),
            vec![],
            1,
        );
        let link = EventLink::new(local_event.id(), foreign_event.id(), EventLinkKind::Causes)
            .expect("cross world link");
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Link across worlds",
            vec![],
            vec![],
            vec![create_event_operation_with_links(local_event, vec![link])],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&snapshot_with_events(
            &[],
            std::slice::from_ref(&foreign_event),
            &[],
            &[],
        ));

        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "reference.cross_world")
        );
    }

    #[test]
    fn reports_missing_entity_reference_in_changeset_validation() {
        let world_id = WorldId::new();
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Add goal",
            vec![],
            vec![],
            vec![create_goal_operation(world_id, crate::EntityId::new())],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());

        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "goal.holder_missing")
        );
    }

    #[test]
    fn reports_double_write_conflict() {
        let world_id = WorldId::new();
        let entity = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Rename Mara twice",
            vec![],
            vec![],
            vec![
                update_entity_operation(&entity, "Mara One", "mara-one"),
                update_entity_operation(&entity, "Mara Two", "mara-two"),
            ],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&snapshot(std::slice::from_ref(&entity), &[]));

        assert!(
            report
                .conflicts
                .iter()
                .any(|issue| issue.code == "change_set.operation.double_write")
        );
    }

    #[test]
    fn reports_orphan_delete() {
        let world_id = WorldId::new();
        let mara = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
        let vale = entity_with(world_id, crate::EntityId::new(), "Vale", "vale", 1);
        let relation = Relation::new(
            world_id,
            mara.id(),
            vale.id(),
            "allied_with",
            RelationDirection::Directed,
            None,
            None,
            Certainty::Certain,
            None,
            "{}",
        )
        .expect("relation");
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Delete Mara",
            vec![],
            vec![],
            vec![delete_entity_operation(&mara)],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&snapshot(&[mara, vale], &[relation]));

        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "change_set.delete_orphan")
        );
    }

    #[test]
    fn reports_missing_dependency_when_reference_is_created_later() {
        let world_id = WorldId::new();
        let entity = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
        let create_entity = ChangeOperation::CreateEntity {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(entity.id())],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: entity.clone(),
        };
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Add goal before entity",
            vec![],
            vec![],
            vec![create_goal_operation(world_id, entity.id()), create_entity],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());

        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "change_set.dependency_order")
        );
    }

    #[test]
    fn allows_creating_an_entity_before_referencing_it() {
        let world_id = WorldId::new();
        let entity = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
        let create_entity = ChangeOperation::CreateEntity {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(entity.id())],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: entity.clone(),
        };
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Add entity then goal",
            vec![],
            vec![],
            vec![create_entity, create_goal_operation(world_id, entity.id())],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());

        assert!(report.is_ok());
    }

    #[test]
    fn blocks_no_resurrection_in_the_resulting_state() {
        let world_id = WorldId::new();
        let mara = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
        let death = event_with(
            world_id,
            crate::EventId::new(),
            "death",
            Some(10),
            vec![EventParticipant::new(mara.id(), "subject", 0).expect("participant")],
            1,
        );
        let return_event = event_with(
            world_id,
            crate::EventId::new(),
            "return",
            Some(20),
            vec![EventParticipant::new(mara.id(), "actor", 0).expect("participant")],
            1,
        );
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Bring Mara back",
            vec![],
            vec![],
            vec![create_event_operation(return_event)],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&snapshot_with_events(
            std::slice::from_ref(&mara),
            std::slice::from_ref(&death),
            &[],
            std::slice::from_ref(&no_resurrection_rule(world_id)),
        ));

        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "rule.no_resurrection")
        );
    }

    #[test]
    fn blocks_cause_after_effect_in_the_resulting_state() {
        let world_id = WorldId::new();
        let cause = event_with(world_id, crate::EventId::new(), "cause", Some(5), vec![], 1);
        let effect = event_with(
            world_id,
            crate::EventId::new(),
            "effect",
            Some(10),
            vec![],
            1,
        );
        let link =
            EventLink::new(cause.id(), effect.id(), EventLinkKind::Causes).expect("causal link");
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Move cause after effect",
            vec![],
            vec![],
            vec![update_event_operation_with_links(
                &cause,
                20,
                vec![link.clone()],
            )],
            vec![],
        )
        .expect("draft");

        let report =
            draft.validation_report(&snapshot_with_events(&[], &[cause, effect], &[link], &[]));

        assert!(
            report
                .conflicts
                .iter()
                .any(|issue| issue.code == "causality.cause_after_effect")
        );
    }

    #[test]
    fn leaves_semantic_rules_and_unspecified_time_as_non_blocking() {
        let world_id = WorldId::new();
        let mara = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
        let unknown_death = event_with(
            world_id,
            crate::EventId::new(),
            "death",
            None,
            vec![EventParticipant::new(mara.id(), "subject", 0).expect("participant")],
            1,
        );
        let travel = event_with(
            world_id,
            crate::EventId::new(),
            "travel",
            Some(20),
            vec![EventParticipant::new(mara.id(), "traveler", 0).expect("participant")],
            1,
        );
        let institutional_rule = Rule::new(
            world_id,
            RuleKind::Institutional,
            "No one may travel alone.",
            "capital",
            RuleSeverity::Advisory,
            None,
            None,
            "{}",
            1,
        )
        .expect("rule");
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Send Mara traveling",
            vec![],
            vec![],
            vec![create_event_operation(travel)],
            vec![],
        )
        .expect("draft");

        let report = draft.validation_report(&snapshot_with_events(
            std::slice::from_ref(&mara),
            std::slice::from_ref(&unknown_death),
            &[],
            &[institutional_rule, no_resurrection_rule(world_id)],
        ));

        assert!(report.is_ok());
        assert!(
            report
                .errors
                .iter()
                .all(|issue| issue.code != "rule.no_resurrection")
        );
    }
}
