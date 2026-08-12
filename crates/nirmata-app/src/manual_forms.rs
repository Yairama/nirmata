use crate::{
    AppError, DraftOperationInput, ManualReviewInput, WorldSession,
    manual_review::{ManualReviewSession, ManualReviewSnapshot},
};
use nirmata_core::{
    ChangeOperationId, ClaimId, DocumentId, DomainError, EntityId, GoalId, Period, World,
    calendar::{CalendarDate, CalendarMonth, WorldCalendar},
    change_set::ChangeOperation,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventAggregate, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
    time::{Certainty, EventTime, EventTimeKind, TimePrecision},
};
use nirmata_store::{ResolvedObject, WorldStore};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDraftResponse {
    pub draft: Option<ManualDraftPreview>,
    pub review: Option<ManualReviewSnapshot>,
    pub field_issues: Vec<ManualFieldIssue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualFieldIssue {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDraftPreview {
    pub draft_key: String,
    pub target_uri: String,
    pub object_type: &'static str,
    pub mode: &'static str,
    pub title: String,
    pub objective: String,
    pub source_uris: Vec<String>,
    pub assumptions: Vec<String>,
    pub logical_path: String,
    pub validation_report: nirmata_core::validation::ValidationReport,
    pub ready_to_confirm: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDraftRequest {
    pub object_type: String,
    pub existing_uri: Option<String>,
    pub objective: Option<String>,
    #[serde(default)]
    pub source_uris: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

struct Builder<'a> {
    store: &'a WorldStore,
    session: &'a WorldSession,
    world: &'a World,
    request: ManualDraftRequest,
    now_ms: i64,
    issues: Vec<ManualFieldIssue>,
}

pub(crate) struct BuiltOperation {
    target_uri: String,
    object_type: &'static str,
    mode: &'static str,
    title: String,
    logical_path: String,
    pub operation: DraftOperationInput,
}

pub(crate) struct PreviewManualDraftOutcome {
    pub response: ManualDraftResponse,
    pub review: Option<ManualReviewSession>,
}

pub(crate) struct PreparedManualOperation {
    pub objective: String,
    pub sources: Vec<ObjectRef>,
    pub assumptions: Vec<String>,
    pub built: BuiltOperation,
}

pub(crate) struct PreparedManualOperationOutcome {
    pub prepared: Option<PreparedManualOperation>,
    pub field_issues: Vec<ManualFieldIssue>,
}

#[derive(Clone, Copy)]
struct EventLinkSpec {
    target_event_id: nirmata_core::EventId,
    kind: EventLinkKind,
}

#[derive(Clone)]
struct ContentReferenceSpec {
    target: ObjectRef,
    ordinal: u32,
}

include!("manual_forms/builder_objects.rs");
include!("manual_forms/builder_values.rs");
include!("manual_forms/operations.rs");
include!("manual_forms/values.rs");
