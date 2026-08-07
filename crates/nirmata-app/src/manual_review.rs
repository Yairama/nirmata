use crate::AppError;
use nirmata_core::{
    ChangeOperationId, DecisionPointId, RevisionId, World, WorldId,
    change_set::{ChangeOperation, ChangeSetDraft, DecisionPoint, RetconKind},
    claim::{Claim, ClaimObject},
    document::{Document, DocumentAggregate, ObjectRef},
    entity::Entity,
    event::{Event, EventAggregate},
    goal::Goal,
    relation::Relation,
    rule::Rule,
    validation::{IssueObject, ValidationIssue, ValidationReport, ValidationSeverity},
};
use nirmata_store::{
    ChangeOperationValue, ChangeSetWaiver, CommittedChangeSetRecord, OperationAudit,
    OperationDecision, StoreError, StoredRevision, WorldStore,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

const DEFAULT_REPLACEMENT_KEEP: &str = "Keep current canon";
const DEFAULT_REPLACEMENT_APPLY: &str = "Apply replacement";
const HIGH_IMPACT_AFFECTED_OBJECTS_THRESHOLD: usize = 4;

include!("manual_review/model.rs");
include!("manual_review/workflow.rs");
include!("manual_review/snapshots.rs");
include!("manual_review/annotations.rs");
include!("manual_review/undo.rs");
