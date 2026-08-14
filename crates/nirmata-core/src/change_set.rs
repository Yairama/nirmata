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
use schemars::JsonSchema;
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

include!("change_set/model.rs");
include!("change_set/change_set_validation.rs");
include!("change_set/operation_validation.rs");
include!("change_set/validation_helpers.rs");
include!("change_set/resulting_state.rs");

#[cfg(test)]
#[path = "../tests/unit/change_set/mod.rs"]
mod tests;
