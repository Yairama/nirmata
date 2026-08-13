use crate::{
    AppError, ContextBundle, ContextBundleRequest, SearchAuthority, SearchClassification,
    SearchResult,
    manual_review::{
        ManualReviewObjectSnapshot, annotate_report_with_change_operations,
        operation_object_snapshot_after, operation_object_snapshot_before,
    },
};
use nirmata_ai::{
    AiError, AzureFoundryClient, CancellationToken, RequestOptions, ResponseRequest, StreamDelta,
    capabilities::{
        AzureFoundryCapabilityClient, CapabilityError, CapabilityInvocation, InvocationMetadata,
    },
    contracts::{
        AdvisoryClassification, AdvisoryResponse, CritiqueReport, DeepSynthesis, ImportExtraction,
        InternalDocumentDraft, InternalDocumentKind, SpecialistReport, StructuredOutputDiagnostic,
        StructuredOutputError, StructuredOutputErrorKind,
    },
};
use nirmata_core::{
    ChangeOperationId, ChangeSetId, EntityId, RevisionId, WorldId,
    change_set::{ChangeOperation, ChangeSetDraft, RetconKind},
    claim::Claim,
    document::{ContentReference, Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
    entity::Entity,
    event::{Event, EventLink},
    goal::Goal,
    relation::Relation,
    rule::Rule,
    validation::{ValidationReport, ValidationSeverity},
};
use nirmata_store::{ReadScope, StructuredSearchTemporal};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    future::Future,
    pin::Pin,
    str::FromStr,
    time::Duration,
};

pub(crate) type ClientFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
type DraftTransform<'a> = dyn Fn(ChangeSetDraft) -> Result<ChangeSetDraft, AppError> + 'a;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiMode {
    Query,
    Propose,
    Critic,
    DocumentDraft,
    DeepImpact,
    Audit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AiRunId(ChangeSetId);

impl AiRunId {
    fn new() -> Self {
        Self(ChangeSetId::new())
    }
}

impl fmt::Display for AiRunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for AiRunId {
    type Err = <ChangeSetId as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ChangeSetId::from_str(value).map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiProviderConfig {
    pub base_url: String,
    pub model: String,
}

impl AiProviderConfig {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AiRequestOptions {
    pub timeout: Duration,
    pub cancellation: Option<CancellationToken>,
}

impl AiRequestOptions {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            cancellation: None,
        }
    }

    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    pub(crate) fn into_request_options(self) -> RequestOptions {
        match self.cancellation {
            Some(cancellation) => RequestOptions::new(self.timeout).with_cancellation(cancellation),
            None => RequestOptions::new(self.timeout),
        }
    }
}

impl Default for AiRequestOptions {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextSnapshot {
    pub world_id: WorldId,
    pub base_revision: RevisionId,
    pub read_scope: ReadScope,
    pub context: ContextBundle,
}

impl AiContextSnapshot {
    pub fn context_object_ids(&self) -> Vec<String> {
        self.context
            .all_entries()
            .into_iter()
            .map(|entry| entry.uri.clone())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQueryInput {
    pub mode: AiMode,
    pub request: String,
    pub snapshot: AiContextSnapshot,
    pub context_object_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProposalInput {
    pub mode: AiMode,
    pub request: String,
    pub snapshot: AiContextSnapshot,
    pub context_object_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InternalDocumentRequest {
    pub instructions: String,
    pub document_kind: InternalDocumentKind,
    pub perspective_entity_id: EntityId,
    pub tick: i64,
    pub anchors: Vec<ObjectRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiInternalDocumentInput {
    pub mode: AiMode,
    pub instructions: String,
    pub requested_document_kind: InternalDocumentKind,
    pub perspective_entity_id: EntityId,
    pub tick: i64,
    pub snapshot: AiContextSnapshot,
    pub context_object_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCritiqueInput {
    pub mode: AiMode,
    pub request: String,
    pub draft: ChangeSetDraft,
    pub deterministic_report: ValidationReport,
    pub semantic_rules: Vec<Rule>,
    pub affected_subgraph: AiAffectedSubgraphSnapshot,
    pub snapshot: AiContextSnapshot,
    pub context_object_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiParsingFailure {
    pub kind: StructuredOutputErrorKind,
    pub message: String,
    pub diagnostic: StructuredOutputDiagnostic,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum AiRepairReport {
    Parsing {
        failure: AiParsingFailure,
    },
    ValidationAndCritique {
        deterministic_report: ValidationReport,
        critique_report: CritiqueReport,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiRepairInput {
    mode: AiMode,
    request: String,
    snapshot: AiContextSnapshot,
    context_object_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failed_draft: Option<ChangeSetDraft>,
    repair_report: AiRepairReport,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAffectedSubgraphSnapshot {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub goals: Vec<Goal>,
    pub events: Vec<Event>,
    pub event_links: Vec<EventLink>,
    pub rules: Vec<Rule>,
    pub claims: Vec<Claim>,
    pub documents: Vec<Document>,
    pub content_references: Vec<ContentReference>,
    pub revisions: Vec<RevisionId>,
}

impl AiAffectedSubgraphSnapshot {
    fn from_validation_snapshot(
        snapshot: &nirmata_core::change_set::ChangeSetValidationSnapshot<'_>,
    ) -> Self {
        Self {
            entities: snapshot.entities.to_vec(),
            relations: snapshot.relations.to_vec(),
            goals: snapshot.goals.to_vec(),
            events: snapshot.events.to_vec(),
            event_links: snapshot.event_links.to_vec(),
            rules: snapshot.rules.to_vec(),
            claims: snapshot.claims.to_vec(),
            documents: snapshot.documents.to_vec(),
            content_references: snapshot.content_references.to_vec(),
            revisions: snapshot.revisions.to_vec(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiQueryProgress {
    PreparingContext,
    CallingModel,
    StreamingDelta { delta: String },
    ParsingResponse,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiProposalProgress {
    PreparingContext,
    IntentBriefReady,
    CallingModel,
    Validating,
    CallingCritic,
    Repairing,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProposalAction {
    pub action: &'static str,
    pub label: String,
    pub request: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQueryCitation {
    pub quote_md: String,
    pub source: SearchResult,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQueryItem {
    pub item_id: String,
    pub classification: SearchClassification,
    pub markdown: String,
    pub content_references: Vec<SearchResult>,
    pub citations: Vec<AiQueryCitation>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiQueryResponse {
    pub request: String,
    pub snapshot: AiContextSnapshot,
    pub items: Vec<AiQueryItem>,
    pub metadata: InvocationMetadata,
    pub proposal_action: Option<AiProposalAction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentBrief {
    pub user_request: String,
    pub objective: String,
    pub scope: String,
    pub entities: Vec<SearchResult>,
    pub restrictions: Vec<String>,
    pub reason: String,
}

impl IntentBrief {
    fn render_request(&self) -> String {
        let entities = if self.entities.is_empty() {
            "- Sin entidades concretas todavía.".to_owned()
        } else {
            self.entities
                .iter()
                .map(|entity| format!("- {}", entity.uri))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let restrictions = if self.restrictions.is_empty() {
            "- Sin restricciones declaradas.".to_owned()
        } else {
            self.restrictions
                .iter()
                .map(|restriction| format!("- {restriction}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "Objetivo: {}\nAlcance: {}\nEntidades:\n{}\nRestricciones:\n{}\nSolicitud original: {}",
            self.objective, self.scope, entities, restrictions, self.user_request
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProposalOperationPreview {
    pub operation_id: String,
    pub kind: &'static str,
    pub target_uri: String,
    pub retcon: &'static str,
    pub affected_objects: Vec<SearchResult>,
    pub before: Option<ManualReviewObjectSnapshot>,
    pub after: Option<ManualReviewObjectSnapshot>,
    pub consequence: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProposalDraftResponse {
    pub request: String,
    pub snapshot: AiContextSnapshot,
    pub draft: ChangeSetDraft,
    pub metadata: InvocationMetadata,
    pub sources: Vec<SearchResult>,
    pub affected_objects: Vec<SearchResult>,
    pub assumptions: Vec<String>,
    pub operations: Vec<AiProposalOperationPreview>,
    pub consequences: Vec<String>,
    pub validation_report: ValidationReport,
    pub critique_report: CritiqueReport,
    pub critique_metadata: InvocationMetadata,
    pub repair_count: u8,
    pub repair_output_failure: Option<AiParsingFailure>,
    pub ready_for_review: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AiProposalResponse {
    IntentBrief {
        snapshot: AiContextSnapshot,
        brief: IntentBrief,
    },
    Draft(AiProposalDraftResponse),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiRunStatus {
    Running,
    IntentBriefReady,
    AwaitingReview,
    AwaitingFinalCritique,
    ReadyToCommit,
    Committed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRunSnapshot {
    pub id: AiRunId,
    pub world_id: WorldId,
    pub base_revision: RevisionId,
    pub mode: AiMode,
    pub request: String,
    pub context: AiContextSnapshot,
    pub status: AiRunStatus,
    pub draft: Option<ChangeSetDraft>,
    pub validation_report: Option<ValidationReport>,
    pub critique_report: Option<CritiqueReport>,
    pub generator_metadata: Option<InvocationMetadata>,
    pub critique_metadata: Option<InvocationMetadata>,
    pub repair_count: u8,
    pub review_key: Option<String>,
    pub intent_brief: Option<IntentBrief>,
    pub committed_revision: Option<RevisionId>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AiRun {
    id: AiRunId,
    world_id: WorldId,
    base_revision: RevisionId,
    mode: AiMode,
    request: String,
    context: AiContextSnapshot,
    critique_acknowledgements: BTreeMap<String, String>,
    state: AiRunState,
}

#[derive(Clone, Debug)]
enum AiRunState {
    Running,
    IntentBriefReady(IntentBrief),
    AwaitingReview(AiRunReview),
    AwaitingFinalCritique(AiRunReview),
    ReadyToCommit {
        review: AiRunReview,
        reviewed_draft: ChangeSetDraft,
        validation_report: ValidationReport,
        critique_report: CritiqueReport,
        critique_metadata: InvocationMetadata,
    },
    FinalCritiqueBlocked {
        review: AiRunReview,
        reviewed_draft: ChangeSetDraft,
        validation_report: ValidationReport,
        critique_report: CritiqueReport,
        critique_metadata: InvocationMetadata,
        base_ready: bool,
    },
    Committed {
        review: AiRunReview,
        revision: RevisionId,
    },
    FinalCritiqueFailed {
        review: AiRunReview,
        error: String,
    },
    FinalCritiqueCancelled {
        review: AiRunReview,
        error: String,
    },
    Failed(String),
    Cancelled(String),
}

#[derive(Clone, Debug)]
struct AiRunReview {
    proposal: AiProposalDraftResponse,
    review_key: String,
}

impl AiRun {
    fn running(request: String, context: AiContextSnapshot) -> Self {
        Self {
            id: AiRunId::new(),
            world_id: context.world_id,
            base_revision: context.base_revision,
            mode: AiMode::Propose,
            request,
            context,
            critique_acknowledgements: BTreeMap::new(),
            state: AiRunState::Running,
        }
    }

    pub(crate) fn status(&self) -> AiRunStatus {
        match &self.state {
            AiRunState::Running => AiRunStatus::Running,
            AiRunState::IntentBriefReady(_) => AiRunStatus::IntentBriefReady,
            AiRunState::AwaitingReview(_) => AiRunStatus::AwaitingReview,
            AiRunState::AwaitingFinalCritique(_) => AiRunStatus::AwaitingFinalCritique,
            AiRunState::ReadyToCommit { .. } => AiRunStatus::ReadyToCommit,
            AiRunState::FinalCritiqueBlocked { .. } => AiRunStatus::AwaitingReview,
            AiRunState::Committed { .. } => AiRunStatus::Committed,
            AiRunState::FinalCritiqueFailed { .. } | AiRunState::Failed(_) => AiRunStatus::Failed,
            AiRunState::FinalCritiqueCancelled { .. } | AiRunState::Cancelled(_) => {
                AiRunStatus::Cancelled
            }
        }
    }

    fn review(&self) -> Option<&AiRunReview> {
        match &self.state {
            AiRunState::AwaitingReview(review)
            | AiRunState::AwaitingFinalCritique(review)
            | AiRunState::ReadyToCommit { review, .. }
            | AiRunState::FinalCritiqueBlocked { review, .. }
            | AiRunState::Committed { review, .. }
            | AiRunState::FinalCritiqueFailed { review, .. }
            | AiRunState::FinalCritiqueCancelled { review, .. } => Some(review),
            _ => None,
        }
    }

    pub(crate) fn mark_review_changed(&mut self) {
        self.critique_acknowledgements.clear();
        if let Some(review) = self.review().cloned() {
            self.state = AiRunState::AwaitingFinalCritique(review);
        }
    }

    fn acknowledge_critique(
        &mut self,
        issue_id: &str,
        judgment: &str,
    ) -> Result<Vec<nirmata_core::ChangeOperationId>, AppError> {
        let AiRunState::FinalCritiqueBlocked {
            review,
            reviewed_draft,
            validation_report,
            critique_report,
            critique_metadata,
            base_ready,
        } = &self.state
        else {
            return Err(AppError::InvalidAiRunTransition {
                run_id: self.id.to_string(),
                status: self.status_label(),
                action: "acknowledge a final critique issue",
            });
        };
        let issue = critique_report
            .issues
            .iter()
            .find(|issue| {
                issue.issue_id.as_str() == issue_id
                    && issue.severity == ValidationSeverity::Conflict
            })
            .ok_or_else(|| AppError::AiCritiqueIssueNotFound {
                run_id: self.id.to_string(),
                issue_id: issue_id.to_owned(),
            })?;
        let operation_ids = issue.affected_operation_ids.clone();
        self.critique_acknowledgements
            .insert(issue_id.to_owned(), judgment.to_owned());
        let all_approved = critique_report
            .issues
            .iter()
            .filter(|issue| issue.severity == ValidationSeverity::Conflict)
            .all(|issue| {
                self.critique_acknowledgements
                    .contains_key(issue.issue_id.as_str())
            });
        if *base_ready && all_approved {
            self.state = AiRunState::ReadyToCommit {
                review: review.clone(),
                reviewed_draft: reviewed_draft.clone(),
                validation_report: validation_report.clone(),
                critique_report: critique_report.clone(),
                critique_metadata: critique_metadata.clone(),
            };
        }
        Ok(operation_ids)
    }

    fn critique_operation_ids(
        &self,
        issue_id: &str,
    ) -> Result<Vec<nirmata_core::ChangeOperationId>, AppError> {
        let AiRunState::FinalCritiqueBlocked {
            critique_report, ..
        } = &self.state
        else {
            return Err(AppError::InvalidAiRunTransition {
                run_id: self.id.to_string(),
                status: self.status_label(),
                action: "acknowledge a final critique issue",
            });
        };
        critique_report
            .issues
            .iter()
            .find(|issue| {
                issue.issue_id.as_str() == issue_id
                    && issue.severity == ValidationSeverity::Conflict
            })
            .map(|issue| issue.affected_operation_ids.clone())
            .ok_or_else(|| AppError::AiCritiqueIssueNotFound {
                run_id: self.id.to_string(),
                issue_id: issue_id.to_owned(),
            })
    }

    pub(crate) fn commit_trace(
        &self,
        review_key: &str,
        draft: &ChangeSetDraft,
    ) -> Result<Value, AppError> {
        let AiRunState::ReadyToCommit {
            review,
            reviewed_draft,
            validation_report,
            critique_report,
            critique_metadata,
        } = &self.state
        else {
            return Err(AppError::InvalidAiRunTransition {
                run_id: self.id.to_string(),
                status: self.status_label(),
                action: "commit",
            });
        };
        if review.review_key != review_key || reviewed_draft != draft {
            return Err(AppError::InvalidAiRunTransition {
                run_id: self.id.to_string(),
                status: self.status_label(),
                action: "commit a review changed after final critique",
            });
        }
        serde_json::to_value(serde_json::json!({
            "kind": "ai_run_summary",
            "runId": self.id,
            "baseRevision": self.base_revision,
            "request": self.request,
            "generator": review.proposal.metadata,
            "initialCritic": review.proposal.critique_metadata,
            "finalCritic": critique_metadata,
            "deterministicReport": validation_report,
            "critiqueReport": critique_report,
            "critiqueAcknowledgements": self.critique_acknowledgements,
            "repairCount": review.proposal.repair_count,
        }))
        .map_err(|error| AppError::Ai(AiError::InvalidResponse(error.to_string())))
    }

    pub(crate) fn mark_committed(&mut self, revision: RevisionId) -> Result<(), AppError> {
        let AiRunState::ReadyToCommit { review, .. } = &self.state else {
            return Err(AppError::InvalidAiRunTransition {
                run_id: self.id.to_string(),
                status: self.status_label(),
                action: "mark committed",
            });
        };
        self.state = AiRunState::Committed {
            review: review.clone(),
            revision,
        };
        Ok(())
    }

    fn status_label(&self) -> &'static str {
        match self.status() {
            AiRunStatus::Running => "running",
            AiRunStatus::IntentBriefReady => "intent_brief_ready",
            AiRunStatus::AwaitingReview => "awaiting_review",
            AiRunStatus::AwaitingFinalCritique => "awaiting_final_critique",
            AiRunStatus::ReadyToCommit => "ready_to_commit",
            AiRunStatus::Committed => "committed",
            AiRunStatus::Failed => "failed",
            AiRunStatus::Cancelled => "cancelled",
        }
    }

    pub(crate) fn snapshot(&self) -> AiRunSnapshot {
        let review = self.review();
        let (validation_report, critique_report, critique_metadata) = match &self.state {
            AiRunState::ReadyToCommit {
                validation_report,
                critique_report,
                critique_metadata,
                ..
            } => (
                Some(validation_report.clone()),
                Some(critique_report.clone()),
                Some(critique_metadata.clone()),
            ),
            AiRunState::FinalCritiqueBlocked {
                validation_report,
                critique_report,
                critique_metadata,
                ..
            } => (
                Some(validation_report.clone()),
                Some(critique_report.clone()),
                Some(critique_metadata.clone()),
            ),
            _ => (
                review.map(|value| value.proposal.validation_report.clone()),
                review.map(|value| value.proposal.critique_report.clone()),
                review.map(|value| value.proposal.critique_metadata.clone()),
            ),
        };
        AiRunSnapshot {
            id: self.id,
            world_id: self.world_id,
            base_revision: self.base_revision,
            mode: self.mode,
            request: self.request.clone(),
            context: self.context.clone(),
            status: self.status(),
            draft: review.map(|value| value.proposal.draft.clone()),
            validation_report,
            critique_report,
            generator_metadata: review.map(|value| value.proposal.metadata.clone()),
            critique_metadata,
            repair_count: review.map_or(0, |value| value.proposal.repair_count),
            review_key: review.map(|value| value.review_key.clone()),
            intent_brief: match &self.state {
                AiRunState::IntentBriefReady(brief) => Some(brief.clone()),
                _ => None,
            },
            committed_revision: match &self.state {
                AiRunState::Committed { revision, .. } => Some(*revision),
                _ => None,
            },
            error: match &self.state {
                AiRunState::Failed(error) | AiRunState::Cancelled(error) => Some(error.clone()),
                AiRunState::FinalCritiqueFailed { error, .. }
                | AiRunState::FinalCritiqueCancelled { error, .. } => Some(error.clone()),
                _ => None,
            },
        }
    }
}

pub(crate) trait AiModeClient {
    fn run_query<'a, F>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
        on_delta: F,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>>
    where
        F: FnMut(StreamDelta) + Send + 'a;

    fn run_proposal<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>>;

    fn run_critic<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<CritiqueReport>, CapabilityError>>;

    fn run_specialist<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<SpecialistReport>, CapabilityError>> {
        let _ = (payload, context_object_ids, options);
        Box::pin(async {
            Err(CapabilityError::Ai(AiError::InvalidResponse(
                "specialist capability is not available".to_owned(),
            )))
        })
    }

    fn run_synthesis<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<DeepSynthesis>, CapabilityError>> {
        let _ = (payload, context_object_ids, options);
        Box::pin(async {
            Err(CapabilityError::Ai(AiError::InvalidResponse(
                "deep synthesis capability is not available".to_owned(),
            )))
        })
    }

    fn run_import_extraction<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ImportExtraction>, CapabilityError>> {
        let _ = (payload, context_object_ids, options);
        Box::pin(async {
            Err(CapabilityError::Ai(AiError::InvalidResponse(
                "import extraction capability is not available".to_owned(),
            )))
        })
    }

    fn run_internal_document<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<InternalDocumentDraft>, CapabilityError>>
    {
        let _ = (payload, context_object_ids, options);
        Box::pin(async {
            Err(CapabilityError::Ai(AiError::InvalidResponse(
                "internal document capability is not available".to_owned(),
            )))
        })
    }
}

impl AiModeClient for AzureFoundryCapabilityClient {
    fn run_query<'a, F>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
        on_delta: F,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>>
    where
        F: FnMut(StreamDelta) + Send + 'a,
    {
        Box::pin(async move {
            self.query_streaming(&payload, context_object_ids, options, on_delta)
                .await
        })
    }

    fn run_proposal<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>> {
        Box::pin(async move { self.propose(&payload, context_object_ids, options).await })
    }

    fn run_critic<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<CritiqueReport>, CapabilityError>> {
        Box::pin(async move { self.critic(&payload, context_object_ids, options).await })
    }

    fn run_specialist<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<SpecialistReport>, CapabilityError>> {
        Box::pin(async move { self.specialist(&payload, context_object_ids, options).await })
    }

    fn run_synthesis<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<DeepSynthesis>, CapabilityError>> {
        Box::pin(async move { self.synthesize(&payload, context_object_ids, options).await })
    }

    fn run_import_extraction<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ImportExtraction>, CapabilityError>> {
        Box::pin(async move {
            self.extract_import(&payload, context_object_ids, options)
                .await
        })
    }

    fn run_internal_document<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<InternalDocumentDraft>, CapabilityError>>
    {
        Box::pin(async move {
            self.generate_internal_document(&payload, context_object_ids, options)
                .await
        })
    }
}

impl crate::NirmataApp {
    pub fn read_ai_run(&self, run_id: AiRunId) -> Result<AiRunSnapshot, AppError> {
        self.ai_runs
            .get(&run_id)
            .map(AiRun::snapshot)
            .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))
    }

    pub fn discard_ai_run(&mut self, run_id: AiRunId) -> Result<AiRunSnapshot, AppError> {
        let review_key = {
            let run = self
                .ai_runs
                .get(&run_id)
                .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
            if run.status() == AiRunStatus::Committed {
                return Err(AppError::InvalidAiRunTransition {
                    run_id: run_id.to_string(),
                    status: "committed",
                    action: "discard",
                });
            }
            run.review().map(|review| review.review_key.clone())
        };
        if let Some(review_key) = review_key {
            if self
                .manual_reviews
                .get(&review_key)
                .is_some_and(|stored| stored.ai_run_id == Some(run_id))
            {
                self.manual_reviews.remove(&review_key);
            }
        }
        let run = self
            .ai_runs
            .get_mut(&run_id)
            .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
        run.state = AiRunState::Cancelled("Discarded by the user before commit.".to_owned());
        Ok(run.snapshot())
    }

    pub fn acknowledge_ai_critique(
        &mut self,
        run_id: AiRunId,
        issue_id: &str,
        judgment: String,
    ) -> Result<AiRunSnapshot, AppError> {
        if judgment.trim().is_empty() {
            return Err(AppError::InvalidAiRunTransition {
                run_id: run_id.to_string(),
                status: "awaiting_review",
                action: "record an empty critique judgment",
            });
        }
        let (review_key, operation_ids) = {
            let run = self
                .ai_runs
                .get(&run_id)
                .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
            let review_key = run
                .review()
                .ok_or(AppError::InvalidAiRunTransition {
                    run_id: run_id.to_string(),
                    status: run.status_label(),
                    action: "acknowledge a final critique issue",
                })?
                .review_key
                .clone();
            (review_key, run.critique_operation_ids(issue_id)?)
        };
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let stored = self
            .manual_reviews
            .get(&review_key)
            .cloned()
            .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.clone()))?;
        let mut review = stored.review;
        for operation_id in operation_ids {
            review = review.apply_action(
                crate::manual_review::ManualReviewAction::RecordJudgment {
                    operation_id,
                    judgment: judgment.trim().to_owned(),
                },
                0,
                &active.store,
            )?;
        }
        self.manual_reviews.insert(
            review_key,
            crate::app::StoredManualReview::from_ai(review, run_id),
        );
        let run = self
            .ai_runs
            .get_mut(&run_id)
            .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
        run.acknowledge_critique(issue_id, judgment.trim())?;
        Ok(run.snapshot())
    }

    pub async fn execute_ai_proposal_run<F>(
        &mut self,
        provider: &AiProviderConfig,
        request: impl Into<String>,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let client = self.provider_client(provider)?;
        self.execute_ai_proposal_run_with(
            &client,
            request.into(),
            context_request,
            options,
            on_progress,
        )
        .await
    }

    pub async fn execute_ai_proposal_run_from_intent_brief<F>(
        &mut self,
        provider: &AiProviderConfig,
        brief: &IntentBrief,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let request = brief.render_request();
        let prepared = self.prepare_ai_proposal(request.clone(), context_request)?;
        let run = AiRun::running(request, prepared.snapshot);
        let run_id = run.id;
        self.ai_runs.insert(run_id, run);
        let result = self
            .execute_ai_proposal_from_intent_brief(
                provider,
                brief,
                context_request,
                options,
                on_progress,
            )
            .await;
        match result {
            Ok(response) => {
                self.complete_ai_proposal_run(run_id, AiProposalResponse::Draft(response))?
            }
            Err(error) => {
                let run = self
                    .ai_runs
                    .get_mut(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                let message = error.to_string();
                run.state = if matches!(error, AppError::Ai(AiError::RequestCancelled)) {
                    AiRunState::Cancelled(message)
                } else {
                    AiRunState::Failed(message)
                };
                return Err(error);
            }
        }
        self.read_ai_run(run_id)
    }

    pub async fn generate_internal_document<F>(
        &mut self,
        provider: &AiProviderConfig,
        request: InternalDocumentRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let client = self.provider_client(provider)?;
        self.generate_internal_document_with(&client, request, options, on_progress)
            .await
    }

    pub(crate) async fn generate_internal_document_with<C, F>(
        &mut self,
        client: &C,
        request: InternalDocumentRequest,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        on_progress(AiProposalProgress::PreparingContext);
        let (prepared, context_request) = self.prepare_internal_document(&request)?;
        let payload = serialize_payload(&prepared, "internal document")?;
        on_progress(AiProposalProgress::CallingModel);
        let invocation = client
            .run_internal_document(
                payload,
                prepared.context_object_ids.clone(),
                options.clone().into_request_options(),
            )
            .await
            .map_err(map_capability_error)?;
        on_progress(AiProposalProgress::Validating);
        validate_internal_document_output(&request, &prepared.snapshot, &invocation.output)?;
        let draft = internal_document_change_set(
            &request,
            &prepared.snapshot,
            &invocation.output,
            crate::app::now_ms()?,
        )?;
        let request_text = format!(
            "Create {} from {} at tick {}: {}",
            request.document_kind.as_str(),
            ObjectRef::Entity(request.perspective_entity_id),
            request.tick,
            request.instructions.trim()
        );
        self.hand_external_draft_to_standard_review(
            client,
            request_text,
            draft,
            invocation.metadata,
            &context_request,
            options,
            on_progress,
        )
        .await
    }

    pub(crate) async fn execute_ai_proposal_run_from_intent_brief_with_transform<C, F, G>(
        &mut self,
        client: &C,
        brief: &IntentBrief,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
        draft_transform: G,
    ) -> Result<AiRunSnapshot, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
        G: Fn(ChangeSetDraft) -> Result<ChangeSetDraft, AppError>,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let request = brief.render_request();
        let prepared = self.prepare_ai_proposal(request.clone(), context_request)?;
        let run = AiRun::running(request, prepared.snapshot);
        let run_id = run.id;
        self.ai_runs.insert(run_id, run);
        let result = self
            .execute_ai_proposal_from_intent_brief_with_transform(
                client,
                brief,
                context_request,
                options,
                Some(&draft_transform),
                on_progress,
            )
            .await;
        match result {
            Ok(response) => {
                self.complete_ai_proposal_run(run_id, AiProposalResponse::Draft(response))?
            }
            Err(error) => {
                let run = self
                    .ai_runs
                    .get_mut(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                let message = error.to_string();
                run.state = if matches!(error, AppError::Ai(AiError::RequestCancelled)) {
                    AiRunState::Cancelled(message)
                } else {
                    AiRunState::Failed(message)
                };
                return Err(error);
            }
        }
        self.read_ai_run(run_id)
    }

    async fn execute_ai_proposal_run_with<C, F>(
        &mut self,
        client: &C,
        request: String,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        let prepared = self.prepare_ai_proposal(request.clone(), context_request)?;
        let run = AiRun::running(request.clone(), prepared.snapshot);
        let run_id = run.id;
        self.ai_runs.insert(run_id, run);

        let result = self
            .execute_ai_proposal_with(client, request, context_request, options, on_progress)
            .await;
        match result {
            Ok(response) => self.complete_ai_proposal_run(run_id, response)?,
            Err(error) => {
                let run = self
                    .ai_runs
                    .get_mut(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                let message = error.to_string();
                run.state = if matches!(error, AppError::Ai(AiError::RequestCancelled)) {
                    AiRunState::Cancelled(message)
                } else {
                    AiRunState::Failed(message)
                };
                return Err(error);
            }
        }
        self.read_ai_run(run_id)
    }

    pub async fn revalidate_ai_run<F>(
        &mut self,
        run_id: AiRunId,
        provider: &AiProviderConfig,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        let client = self.provider_client(provider)?;
        self.revalidate_ai_run_with(run_id, &client, context_request, options, on_progress)
            .await
    }

    pub(crate) async fn revalidate_ai_run_with<C, F>(
        &mut self,
        run_id: AiRunId,
        client: &C,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        let (request, review) = {
            let run = self
                .ai_runs
                .get(&run_id)
                .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
            if !matches!(
                &run.state,
                AiRunState::AwaitingFinalCritique(_)
                    | AiRunState::FinalCritiqueFailed { .. }
                    | AiRunState::FinalCritiqueCancelled { .. }
            ) {
                return Err(AppError::InvalidAiRunTransition {
                    run_id: run_id.to_string(),
                    status: run.status_label(),
                    action: "run final critique before a human review action",
                });
            }
            let review = run.review().cloned().expect("review state checked above");
            (run.request.clone(), review)
        };

        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        let world = active.store.load_world()?;
        active.session.world_id = world.id();
        active.session.current_revision = world.current_revision();
        active.session.world = world;
        let live_head = active.session.current_revision;
        let stored = self
            .manual_reviews
            .get(&review.review_key)
            .cloned()
            .ok_or_else(|| AppError::ReviewSessionNotFound(review.review_key.clone()))?;
        if stored.ai_run_id != Some(run_id) {
            return Err(AppError::InvalidAiRunTransition {
                run_id: run_id.to_string(),
                status: "review_mismatch",
                action: "run final critique",
            });
        }
        let refreshed = stored
            .review
            .revalidate_at_revision(live_head, &active.store)?;
        let reviewed_draft = refreshed.draft().clone();

        if let Some(run) = self.ai_runs.get_mut(&run_id) {
            run.state = AiRunState::AwaitingFinalCritique(review.clone());
        }
        on_progress(AiProposalProgress::Validating);
        let critique_input =
            self.prepare_ai_critique(&request, &reviewed_draft, context_request)?;
        let payload = serialize_payload(&critique_input, "final critique")?;
        on_progress(AiProposalProgress::CallingCritic);
        let critique_result = client
            .run_critic(
                payload,
                critique_input.context_object_ids.clone(),
                options.into_request_options(),
            )
            .await
            .map_err(map_capability_error);
        let critique = match critique_result {
            Ok(critique) => critique,
            Err(error) => {
                let run = self
                    .ai_runs
                    .get_mut(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                let message = error.to_string();
                run.state = if matches!(error, AppError::Ai(AiError::RequestCancelled)) {
                    AiRunState::FinalCritiqueCancelled {
                        review,
                        error: message,
                    }
                } else {
                    AiRunState::FinalCritiqueFailed {
                        review,
                        error: message,
                    }
                };
                return Err(error);
            }
        };
        validate_critique_references(&critique_input, &critique.output)?;

        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        let current_world = active.store.load_world()?;
        if current_world.current_revision() != live_head {
            let stored = self
                .manual_reviews
                .get_mut(&review.review_key)
                .ok_or_else(|| AppError::ReviewSessionNotFound(review.review_key.clone()))?;
            stored.freshness = crate::app::StoredManualReviewFreshness::RefreshRestartRequired {
                current_revision: current_world.current_revision(),
            };
            return Err(AppError::AiBaseRevisionMismatch {
                draft_base_revision: live_head,
                current_revision: current_world.current_revision(),
            });
        }

        let change_set = nirmata_core::change_set::ChangeSet::new(
            reviewed_draft.world_id(),
            reviewed_draft.base_revision(),
            reviewed_draft.objective().to_owned(),
            reviewed_draft.sources().to_vec(),
            reviewed_draft.assumptions().to_vec(),
            reviewed_draft.operations().to_vec(),
            reviewed_draft.decisions().to_vec(),
        )?;
        let mut final_report = active.store.validate_change_set(&change_set)?;
        annotate_report_with_change_operations(&mut final_report, change_set.operations());
        let effective_final_report =
            crate::app::apply_review_waivers(&final_report, refreshed.waivers());
        let unresolved_critique = critique.output.issues.iter().any(|issue| {
            issue.severity == ValidationSeverity::Conflict
                && !self.ai_runs.get(&run_id).is_some_and(|run| {
                    run.critique_acknowledgements
                        .contains_key(issue.issue_id.as_str())
                })
        });
        let base_ready = refreshed.ready_to_confirm() && effective_final_report.is_ok();
        let ready = base_ready && !unresolved_critique;
        self.manual_reviews.insert(
            review.review_key.clone(),
            crate::app::StoredManualReview::from_ai(refreshed, run_id),
        );
        let run = self
            .ai_runs
            .get_mut(&run_id)
            .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
        run.base_revision = live_head;
        run.context = critique_input.snapshot;
        run.state = if ready {
            AiRunState::ReadyToCommit {
                review,
                reviewed_draft,
                validation_report: final_report,
                critique_report: critique.output,
                critique_metadata: critique.metadata,
            }
        } else {
            AiRunState::FinalCritiqueBlocked {
                review,
                reviewed_draft,
                validation_report: final_report,
                critique_report: critique.output,
                critique_metadata: critique.metadata,
                base_ready,
            }
        };
        on_progress(AiProposalProgress::Completed);
        Ok(run.snapshot())
    }

    pub(crate) fn complete_ai_proposal_run(
        &mut self,
        run_id: AiRunId,
        response: AiProposalResponse,
    ) -> Result<(), AppError> {
        match response {
            AiProposalResponse::IntentBrief { snapshot, brief } => {
                let run = self
                    .ai_runs
                    .get_mut(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                run.base_revision = snapshot.base_revision;
                run.context = snapshot;
                run.state = AiRunState::IntentBriefReady(brief);
            }
            AiProposalResponse::Draft(proposal) => {
                let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
                let review = crate::manual_review::ManualReviewSession::from_draft(
                    active.session.active_variant.id,
                    proposal.draft.clone(),
                    &active.store,
                )?;
                let review_key = proposal
                    .draft
                    .operations()
                    .first()
                    .map(|operation| operation.primary_ref().to_string())
                    .unwrap_or_else(|| ObjectRef::World(proposal.draft.world_id()).to_string());
                if self.manual_reviews.contains_key(&review_key) {
                    let run = self
                        .ai_runs
                        .get_mut(&run_id)
                        .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                    run.state = AiRunState::Failed(format!(
                        "another pending review already targets {review_key}"
                    ));
                    return Err(AppError::ReviewSessionConflict(review_key));
                }
                self.manual_reviews.insert(
                    review_key.clone(),
                    crate::app::StoredManualReview::from_ai(review, run_id),
                );
                let run = self
                    .ai_runs
                    .get_mut(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?;
                run.base_revision = proposal.snapshot.base_revision;
                run.context = proposal.snapshot.clone();
                run.state = AiRunState::AwaitingReview(AiRunReview {
                    proposal,
                    review_key,
                });
            }
        }
        Ok(())
    }

    pub fn prepare_ai_query(
        &self,
        request: impl Into<String>,
        context_request: &ContextBundleRequest,
    ) -> Result<AiQueryInput, AppError> {
        let snapshot = self.build_ai_context_snapshot(context_request)?;
        let context_object_ids = snapshot.context_object_ids();
        Ok(AiQueryInput {
            mode: AiMode::Query,
            request: request.into(),
            snapshot,
            context_object_ids,
        })
    }

    pub fn prepare_ai_proposal(
        &self,
        request: impl Into<String>,
        context_request: &ContextBundleRequest,
    ) -> Result<AiProposalInput, AppError> {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let snapshot = self.build_ai_context_snapshot(context_request)?;
        let context_object_ids = snapshot.context_object_ids();
        Ok(AiProposalInput {
            mode: AiMode::Propose,
            request: request.into(),
            snapshot,
            context_object_ids,
        })
    }

    pub fn prepare_internal_document(
        &self,
        request: &InternalDocumentRequest,
    ) -> Result<(AiInternalDocumentInput, ContextBundleRequest), AppError> {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        if request.instructions.trim().is_empty() {
            return Err(AppError::InvalidInternalDocument(
                "instructions cannot be empty".to_owned(),
            ));
        }
        let perspective = ObjectRef::Entity(request.perspective_entity_id);
        let mut context_request = ContextBundleRequest::new(crate::ContextIntent::DocumentDraft);
        context_request.anchors = request.anchors.clone();
        if !context_request.anchors.contains(&perspective) {
            context_request.anchors.push(perspective);
        }
        context_request.temporal = Some(StructuredSearchTemporal::Tick(request.tick));
        context_request.perspective_entity_ids = vec![request.perspective_entity_id];
        context_request.include_perspectives = true;
        let snapshot = self.build_ai_context_snapshot(&context_request)?;
        if !snapshot.context.contains(perspective) {
            return Err(AppError::InvalidInternalDocument(
                "perspective entity is not accessible in the document context".to_owned(),
            ));
        }
        let context_object_ids = snapshot.context_object_ids();
        Ok((
            AiInternalDocumentInput {
                mode: AiMode::DocumentDraft,
                instructions: request.instructions.clone(),
                requested_document_kind: request.document_kind,
                perspective_entity_id: request.perspective_entity_id,
                tick: request.tick,
                snapshot,
                context_object_ids,
            },
            context_request,
        ))
    }

    pub fn prepare_ai_critique(
        &self,
        request: impl Into<String>,
        draft: &ChangeSetDraft,
        context_request: &ContextBundleRequest,
    ) -> Result<AiCritiqueInput, AppError> {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let mut critique_request = context_request.clone();
        merge_source_anchors(&mut critique_request, draft.sources());
        let snapshot = self.build_ai_context_snapshot(&critique_request)?;
        if draft.base_revision() != snapshot.base_revision {
            return Err(AppError::AiBaseRevisionMismatch {
                draft_base_revision: draft.base_revision(),
                current_revision: snapshot.base_revision,
            });
        }
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let graph = active.store.load_affected_graph_for_draft(draft)?;
        let validation_snapshot = graph.validation_snapshot();
        let mut deterministic_report = draft.validation_report(&validation_snapshot);
        annotate_report_with_change_operations(&mut deterministic_report, draft.operations());
        let semantic_rules = validation_snapshot
            .rules
            .iter()
            .filter(|rule| rule.validator_kind().is_none())
            .cloned()
            .collect::<Vec<_>>();
        let affected_subgraph =
            AiAffectedSubgraphSnapshot::from_validation_snapshot(&validation_snapshot);
        let mut context_object_ids = snapshot
            .context_object_ids()
            .into_iter()
            .collect::<BTreeSet<_>>();
        context_object_ids.extend(allowed_critique_uris(draft, &affected_subgraph));
        Ok(AiCritiqueInput {
            mode: AiMode::Critic,
            request: request.into(),
            draft: draft.clone(),
            deterministic_report,
            semantic_rules,
            affected_subgraph,
            snapshot,
            context_object_ids: context_object_ids.into_iter().collect(),
        })
    }

    pub async fn execute_ai_query<F>(
        &self,
        provider: &AiProviderConfig,
        request: impl Into<String>,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiQueryResponse, AppError>
    where
        F: FnMut(AiQueryProgress) + Send,
    {
        let client = self.provider_client(provider)?;
        self.execute_ai_query_with(
            &client,
            request.into(),
            context_request,
            options,
            on_progress,
        )
        .await
    }

    pub async fn execute_ai_proposal<F>(
        &self,
        provider: &AiProviderConfig,
        request: impl Into<String>,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiProposalResponse, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let client = self.provider_client(provider)?;
        self.execute_ai_proposal_with(
            &client,
            request.into(),
            context_request,
            options,
            on_progress,
        )
        .await
    }

    pub async fn execute_ai_proposal_from_intent_brief<F>(
        &self,
        provider: &AiProviderConfig,
        brief: &IntentBrief,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiProposalDraftResponse, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let client = self.provider_client(provider)?;
        self.execute_ai_proposal_from_intent_brief_with(
            &client,
            brief,
            context_request,
            options,
            on_progress,
        )
        .await
    }

    async fn execute_ai_query_with<C, F>(
        &self,
        client: &C,
        request: String,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<AiQueryResponse, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiQueryProgress) + Send,
    {
        on_progress(AiQueryProgress::PreparingContext);
        let prepared = self.prepare_ai_query(request.clone(), context_request)?;
        let payload = serialize_payload(&prepared, "query")?;
        let request_options = options.clone().into_request_options();

        on_progress(AiQueryProgress::CallingModel);
        let invocation = client
            .run_query(
                payload,
                prepared.context_object_ids.clone(),
                request_options,
                |delta| {
                    on_progress(AiQueryProgress::StreamingDelta { delta: delta.delta });
                },
            )
            .await
            .map_err(map_capability_error)?;

        on_progress(AiQueryProgress::ParsingResponse);
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let CapabilityInvocation { output, metadata } = invocation;
        let items = output
            .items
            .iter()
            .map(|item| query_item_from_contract(&active.store, prepared.snapshot.read_scope, item))
            .collect::<Result<Vec<_>, AppError>>()?;

        on_progress(AiQueryProgress::Completed);
        Ok(AiQueryResponse {
            request: prepared.request,
            snapshot: prepared.snapshot,
            items,
            metadata,
            proposal_action: query_request_is_write_like(&request).then(|| AiProposalAction {
                action: "start_proposal",
                label: "Iniciar propuesta revisable".to_owned(),
                request,
            }),
        })
    }

    async fn execute_ai_proposal_with<C, F>(
        &self,
        client: &C,
        request: String,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<AiProposalResponse, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        on_progress(AiProposalProgress::PreparingContext);
        let prepared = self.prepare_ai_proposal(request.clone(), context_request)?;
        if let Some(reason) = proposal_brief_reason(&request, context_request) {
            let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
            let brief = build_intent_brief(&active.store, &prepared.snapshot, &request, reason)?;
            on_progress(AiProposalProgress::IntentBriefReady);
            on_progress(AiProposalProgress::Completed);
            return Ok(AiProposalResponse::IntentBrief {
                snapshot: prepared.snapshot,
                brief,
            });
        }

        let draft = self
            .execute_ai_proposal_input_with(
                client,
                request,
                prepared,
                context_request,
                options,
                None,
                &mut on_progress,
            )
            .await?;
        on_progress(AiProposalProgress::Completed);
        Ok(AiProposalResponse::Draft(draft))
    }

    async fn execute_ai_proposal_from_intent_brief_with<C, F>(
        &self,
        client: &C,
        brief: &IntentBrief,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<AiProposalDraftResponse, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        self.execute_ai_proposal_from_intent_brief_with_transform(
            client,
            brief,
            context_request,
            options,
            None,
            on_progress,
        )
        .await
    }

    async fn execute_ai_proposal_from_intent_brief_with_transform<C, F>(
        &self,
        client: &C,
        brief: &IntentBrief,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        draft_transform: Option<&DraftTransform<'_>>,
        mut on_progress: F,
    ) -> Result<AiProposalDraftResponse, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        on_progress(AiProposalProgress::PreparingContext);
        let request = brief.render_request();
        let prepared = self.prepare_ai_proposal(request.clone(), context_request)?;
        let response = self
            .execute_ai_proposal_input_with(
                client,
                request,
                prepared,
                context_request,
                options,
                draft_transform,
                &mut on_progress,
            )
            .await?;
        on_progress(AiProposalProgress::Completed);
        Ok(response)
    }

    async fn execute_ai_proposal_input_with<C, F>(
        &self,
        client: &C,
        request: String,
        prepared: AiProposalInput,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        draft_transform: Option<&DraftTransform<'_>>,
        on_progress: &mut F,
    ) -> Result<AiProposalDraftResponse, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        let payload = serialize_payload(&prepared, "proposal")?;

        on_progress(AiProposalProgress::CallingModel);
        let initial_result = client
            .run_proposal(
                payload,
                prepared.context_object_ids.clone(),
                options.clone().into_request_options(),
            )
            .await;

        match initial_result {
            Ok(initial_invocation) => {
                let initial_invocation =
                    transform_draft_invocation(initial_invocation, draft_transform)?;
                let (initial_critique_input, initial_critique) = self
                    .evaluate_ai_proposal_with(
                        client,
                        &request,
                        &initial_invocation.output,
                        context_request,
                        &options,
                        on_progress,
                    )
                    .await?;
                if !evaluation_needs_repair(
                    &initial_critique_input.deterministic_report,
                    &initial_critique.output,
                ) {
                    let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
                    return build_proposal_draft_response(
                        &active.store,
                        request,
                        prepared.snapshot,
                        initial_invocation,
                        initial_critique_input.deterministic_report,
                        initial_critique,
                        0,
                        None,
                    );
                }

                let repair_input = AiRepairInput {
                    mode: AiMode::Propose,
                    request: request.clone(),
                    snapshot: prepared.snapshot.clone(),
                    context_object_ids: initial_critique_input.context_object_ids.clone(),
                    failed_draft: Some(initial_invocation.output.clone()),
                    repair_report: AiRepairReport::ValidationAndCritique {
                        deterministic_report: initial_critique_input.deterministic_report.clone(),
                        critique_report: initial_critique.output.clone(),
                    },
                };
                let repair_payload = serialize_payload(&repair_input, "repair")?;
                on_progress(AiProposalProgress::Repairing);
                match client
                    .run_proposal(
                        repair_payload,
                        repair_input.context_object_ids,
                        options.clone().into_request_options(),
                    )
                    .await
                {
                    Ok(repaired_invocation) => {
                        let repaired_invocation =
                            transform_draft_invocation(repaired_invocation, draft_transform)?;
                        let (repaired_input, repaired_critique) = self
                            .evaluate_ai_proposal_with(
                                client,
                                &request,
                                &repaired_invocation.output,
                                context_request,
                                &options,
                                on_progress,
                            )
                            .await?;
                        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
                        build_proposal_draft_response(
                            &active.store,
                            request,
                            prepared.snapshot,
                            repaired_invocation,
                            repaired_input.deterministic_report,
                            repaired_critique,
                            1,
                            None,
                        )
                    }
                    Err(CapabilityError::StructuredOutput(error)) => {
                        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
                        build_proposal_draft_response(
                            &active.store,
                            request,
                            prepared.snapshot,
                            initial_invocation,
                            initial_critique_input.deterministic_report,
                            initial_critique,
                            1,
                            Some(parsing_failure(&error)),
                        )
                    }
                    Err(error) => Err(map_capability_error(error)),
                }
            }
            Err(CapabilityError::StructuredOutput(error)) => {
                let repair_input = AiRepairInput {
                    mode: AiMode::Propose,
                    request: request.clone(),
                    snapshot: prepared.snapshot.clone(),
                    context_object_ids: prepared.context_object_ids.clone(),
                    failed_draft: None,
                    repair_report: AiRepairReport::Parsing {
                        failure: parsing_failure(&error),
                    },
                };
                let repair_payload = serialize_payload(&repair_input, "repair")?;
                on_progress(AiProposalProgress::Repairing);
                let repaired_invocation = client
                    .run_proposal(
                        repair_payload,
                        repair_input.context_object_ids,
                        options.clone().into_request_options(),
                    )
                    .await
                    .map_err(map_capability_error)?;
                let repaired_invocation =
                    transform_draft_invocation(repaired_invocation, draft_transform)?;
                let (repaired_input, repaired_critique) = self
                    .evaluate_ai_proposal_with(
                        client,
                        &request,
                        &repaired_invocation.output,
                        context_request,
                        &options,
                        on_progress,
                    )
                    .await?;
                let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
                build_proposal_draft_response(
                    &active.store,
                    request,
                    prepared.snapshot,
                    repaired_invocation,
                    repaired_input.deterministic_report,
                    repaired_critique,
                    1,
                    None,
                )
            }
            Err(error) => Err(map_capability_error(error)),
        }
    }

    async fn evaluate_ai_proposal_with<C, F>(
        &self,
        client: &C,
        request: &str,
        draft: &ChangeSetDraft,
        context_request: &ContextBundleRequest,
        options: &AiRequestOptions,
        on_progress: &mut F,
    ) -> Result<(AiCritiqueInput, CapabilityInvocation<CritiqueReport>), AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        on_progress(AiProposalProgress::Validating);
        let critique_input = self.prepare_ai_critique(request, draft, context_request)?;
        let critique_payload = serialize_payload(&critique_input, "critique")?;
        on_progress(AiProposalProgress::CallingCritic);
        let critique_invocation = client
            .run_critic(
                critique_payload,
                critique_input.context_object_ids.clone(),
                options.clone().into_request_options(),
            )
            .await
            .map_err(map_capability_error)?;
        validate_critique_references(&critique_input, &critique_invocation.output)?;
        Ok((critique_input, critique_invocation))
    }

    pub(crate) fn build_ai_context_snapshot(
        &self,
        context_request: &ContextBundleRequest,
    ) -> Result<AiContextSnapshot, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let read_scope = active.read_scope;
        let world = active
            .store
            .read_canon_snapshot_scoped(read_scope)?
            .world()
            .clone();
        let context = crate::context_bundle::build_context_bundle_scoped(
            &active.store,
            read_scope,
            context_request,
        )?;
        Ok(AiContextSnapshot {
            world_id: world.id(),
            base_revision: world.current_revision(),
            read_scope,
            context,
        })
    }

    pub(crate) fn provider_client(
        &self,
        provider: &AiProviderConfig,
    ) -> Result<AzureFoundryCapabilityClient, AppError> {
        let api_key = self
            .provider_credentials
            .clone_api_key()
            .ok_or(AiError::MissingProviderApiKey)?;
        AzureFoundryCapabilityClient::new(&provider.base_url, api_key, &provider.model)
            .map_err(Into::into)
    }
}

impl crate::NirmataApp {
    pub async fn diagnose_ai_provider(
        &self,
        provider: &AiProviderConfig,
        options: AiRequestOptions,
    ) -> Result<(), AppError> {
        let api_key = self
            .provider_credentials
            .clone_api_key()
            .ok_or(AiError::MissingProviderApiKey)?;
        let client = AzureFoundryClient::new(&provider.base_url)?;
        client
            .create_response(
                &api_key,
                ResponseRequest::new(
                    &provider.model,
                    "Connectivity diagnostic. Reply with OK only.",
                    "OK",
                )
                .with_max_output_tokens(16),
                options.into_request_options(),
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn hand_external_draft_to_standard_review<C, F>(
        &mut self,
        client: &C,
        request: String,
        draft: ChangeSetDraft,
        generator_metadata: InvocationMetadata,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        crate::app::ensure_active_write_scope(self.active.as_ref().ok_or(AppError::NoWorldOpen)?)?;
        let snapshot = self.build_ai_context_snapshot(context_request)?;
        if draft.world_id() != snapshot.world_id || draft.base_revision() != snapshot.base_revision
        {
            return Err(AppError::AiBaseRevisionMismatch {
                draft_base_revision: draft.base_revision(),
                current_revision: snapshot.base_revision,
            });
        }
        let invocation = CapabilityInvocation {
            output: draft,
            metadata: generator_metadata,
        };
        let (critique_input, critique) = self
            .evaluate_ai_proposal_with(
                client,
                &request,
                &invocation.output,
                context_request,
                &options,
                &mut on_progress,
            )
            .await?;
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let proposal = build_proposal_draft_response(
            &active.store,
            request.clone(),
            snapshot.clone(),
            invocation,
            critique_input.deterministic_report,
            critique,
            0,
            None,
        )?;
        let run = AiRun::running(request, snapshot);
        let run_id = run.id;
        self.ai_runs.insert(run_id, run);
        self.complete_ai_proposal_run(run_id, AiProposalResponse::Draft(proposal))?;
        self.read_ai_run(run_id)
    }

    pub(crate) async fn hand_deep_synthesis_to_standard_review<C, F>(
        &mut self,
        client: &C,
        request: String,
        snapshot: AiContextSnapshot,
        synthesis: CapabilityInvocation<DeepSynthesis>,
        context_request: &ContextBundleRequest,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<AiRunSnapshot, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        let draft_invocation = CapabilityInvocation {
            output: synthesis.output.draft,
            metadata: synthesis.metadata,
        };
        let (critique_input, critique) = self
            .evaluate_ai_proposal_with(
                client,
                &request,
                &draft_invocation.output,
                context_request,
                &options,
                &mut on_progress,
            )
            .await?;
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let proposal = build_proposal_draft_response(
            &active.store,
            request.clone(),
            snapshot.clone(),
            draft_invocation,
            critique_input.deterministic_report,
            critique,
            0,
            None,
        )?;
        let run = AiRun::running(request, snapshot);
        let run_id = run.id;
        self.ai_runs.insert(run_id, run);
        self.complete_ai_proposal_run(run_id, AiProposalResponse::Draft(proposal))?;
        self.read_ai_run(run_id)
    }
}

fn transform_draft_invocation(
    invocation: CapabilityInvocation<ChangeSetDraft>,
    transform: Option<&DraftTransform<'_>>,
) -> Result<CapabilityInvocation<ChangeSetDraft>, AppError> {
    let Some(transform) = transform else {
        return Ok(invocation);
    };
    Ok(CapabilityInvocation {
        output: transform(invocation.output)?,
        metadata: invocation.metadata,
    })
}

fn validate_internal_document_output(
    request: &InternalDocumentRequest,
    snapshot: &AiContextSnapshot,
    output: &InternalDocumentDraft,
) -> Result<(), AppError> {
    if output.document_kind != request.document_kind {
        return Err(AppError::InvalidInternalDocument(format!(
            "model returned {} instead of requested {}",
            output.document_kind.as_str(),
            request.document_kind.as_str()
        )));
    }
    let allowed = snapshot
        .context_object_ids()
        .into_iter()
        .collect::<BTreeSet<_>>();
    for reference in &output.content_reference_uris {
        let uri = String::from(*reference);
        if !allowed.contains(&uri) {
            return Err(AppError::InvalidInternalDocument(format!(
                "document cites {uri} outside the perspective context"
            )));
        }
    }
    if output.content_reference_uris.iter().all(|reference| {
        !matches!(
            reference.object_ref(),
            ObjectRef::Entity(_) | ObjectRef::Event(_) | ObjectRef::Rule(_)
        )
    }) {
        return Err(AppError::InvalidInternalDocument(
            "document must cite at least one accessible entity, event or rule".to_owned(),
        ));
    }
    Ok(())
}

fn internal_document_change_set(
    request: &InternalDocumentRequest,
    snapshot: &AiContextSnapshot,
    output: &InternalDocumentDraft,
    now_ms: i64,
) -> Result<ChangeSetDraft, AppError> {
    let document = Document::new(
        snapshot.world_id,
        output.title.clone(),
        output.document_kind.as_str(),
        Some(request.perspective_entity_id),
        Some(request.perspective_entity_id),
        DocumentCanonStatus::Canonical,
        output.body_markdown.clone(),
        now_ms,
    )?;
    let document_ref = ObjectRef::Document(document.id());
    let sources = output
        .content_reference_uris
        .iter()
        .map(|reference| reference.object_ref())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let references = sources
        .iter()
        .enumerate()
        .map(|(ordinal, target)| {
            Ok(ContentReference::new(
                document_ref,
                *target,
                u32::try_from(ordinal).map_err(|_| {
                    AppError::InvalidInternalDocument("too many content references".to_owned())
                })?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let aggregate = DocumentAggregate::new(document, references);
    let mut affected_ids = BTreeSet::from([
        document_ref,
        ObjectRef::Entity(request.perspective_entity_id),
    ]);
    affected_ids.extend(sources.iter().copied());
    let operation = ChangeOperation::CreateDocument {
        operation_id: ChangeOperationId::new(),
        affected_ids: affected_ids.into_iter().collect(),
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: aggregate,
    };
    ChangeSetDraft::new(
        snapshot.world_id,
        snapshot.base_revision,
        format!(
            "Create internal {}: {}",
            output.document_kind.as_str(),
            output.title
        ),
        sources,
        vec![format!(
            "Written from {} at tick {} using only the scoped context.",
            ObjectRef::Entity(request.perspective_entity_id),
            request.tick
        )],
        vec![operation],
        vec![],
    )
    .map_err(Into::into)
}

fn build_proposal_draft_response(
    store: &nirmata_store::WorldStore,
    request: String,
    snapshot: AiContextSnapshot,
    invocation: CapabilityInvocation<ChangeSetDraft>,
    validation_report: ValidationReport,
    critique_invocation: CapabilityInvocation<CritiqueReport>,
    repair_count: u8,
    repair_output_failure: Option<AiParsingFailure>,
) -> Result<AiProposalDraftResponse, AppError> {
    let CapabilityInvocation {
        output: draft,
        metadata,
    } = invocation;
    let CapabilityInvocation {
        output: critique_report,
        metadata: critique_metadata,
    } = critique_invocation;

    let read_scope = snapshot.read_scope;
    let mut affected_seen = BTreeSet::new();
    let mut affected_objects = Vec::new();
    let draft_objects = draft
        .operations()
        .iter()
        .flat_map(|operation| {
            [
                operation_object_snapshot_before(operation),
                operation_object_snapshot_after(operation),
            ]
            .into_iter()
            .flatten()
        })
        .map(|snapshot| (snapshot.target_uri.clone(), snapshot))
        .collect::<BTreeMap<_, _>>();
    let operations = draft
        .operations()
        .iter()
        .map(|operation| {
            let before = operation_object_snapshot_before(operation);
            let after = operation_object_snapshot_after(operation);
            let affected = operation
                .affected_ids()
                .iter()
                .copied()
                .map(|object| {
                    resolve_proposal_result(
                        store,
                        read_scope,
                        object,
                        before.as_ref(),
                        after.as_ref(),
                        &draft_objects,
                    )
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            for result in &affected {
                if affected_seen.insert(result.uri.clone()) {
                    affected_objects.push(result.clone());
                }
            }
            Ok(AiProposalOperationPreview {
                operation_id: operation.operation_id().to_string(),
                kind: operation_kind(operation),
                target_uri: operation.primary_ref().to_string(),
                retcon: retcon_label(operation),
                consequence: operation_consequence(operation, before.as_ref(), after.as_ref()),
                affected_objects: affected,
                before,
                after,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    let consequences = operations
        .iter()
        .map(|operation| operation.consequence.clone())
        .collect::<Vec<_>>();
    let sources = draft
        .sources()
        .iter()
        .copied()
        .map(|source| resolve_search_result(store, read_scope, source.to_string()))
        .collect::<Result<Vec<_>, AppError>>()?;

    let ready_for_review = !deterministic_blocks_review(&validation_report)
        && !critique_blocks_review(&critique_report)
        && repair_output_failure.is_none();
    Ok(AiProposalDraftResponse {
        request,
        snapshot,
        assumptions: draft.assumptions().to_vec(),
        draft,
        metadata,
        sources,
        affected_objects,
        operations,
        consequences,
        ready_for_review,
        validation_report,
        critique_report,
        critique_metadata,
        repair_count,
        repair_output_failure,
    })
}

fn query_item_from_contract(
    store: &nirmata_store::WorldStore,
    scope: ReadScope,
    item: &nirmata_ai::contracts::AdvisoryItem,
) -> Result<AiQueryItem, AppError> {
    Ok(AiQueryItem {
        item_id: item.item_id.as_str().to_owned(),
        classification: query_classification(item.classification),
        markdown: item.answer.markdown.clone(),
        content_references: item
            .answer
            .content_references
            .iter()
            .copied()
            .map(|reference| resolve_search_result(store, scope, String::from(reference)))
            .collect::<Result<Vec<_>, _>>()?,
        citations: item
            .citations
            .iter()
            .map(|citation| {
                Ok(AiQueryCitation {
                    quote_md: citation.quote_md.clone(),
                    source: resolve_search_result(store, scope, String::from(citation.source_uri))?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?,
    })
}

fn build_intent_brief(
    store: &nirmata_store::WorldStore,
    snapshot: &AiContextSnapshot,
    request: &str,
    reason: String,
) -> Result<IntentBrief, AppError> {
    let entities = snapshot
        .context
        .all_entries()
        .into_iter()
        .filter_map(|entry| {
            matches!(entry.object_ref(), ObjectRef::Entity(_)).then_some(entry.uri.clone())
        })
        .collect::<Vec<_>>();
    let entity_results = entities
        .into_iter()
        .take(6)
        .map(|uri| resolve_search_result(store, snapshot.read_scope, uri))
        .collect::<Result<Vec<_>, AppError>>()?;
    let restrictions = snapshot
        .context
        .obligations
        .iter()
        .map(|entry| entry.citation.clone())
        .filter(|citation| !citation.trim().is_empty())
        .take(4)
        .collect::<Vec<_>>();
    let mut restrictions = if restrictions.is_empty() {
        vec!["No inventar datos fuera del contexto recuperado.".to_owned()]
    } else {
        restrictions
    };
    if !restrictions
        .iter()
        .any(|item| item == "Conservar la revisión base hasta confirmar la propuesta.")
    {
        restrictions.push("Conservar la revisión base hasta confirmar la propuesta.".to_owned());
    }

    let scope = if entity_results.is_empty() {
        "Delimitar primero los objetos concretos que deben cambiar.".to_owned()
    } else {
        format!(
            "Cambios acotados a {}.",
            entity_results
                .iter()
                .take(3)
                .map(|entity| entity.uri.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    Ok(IntentBrief {
        user_request: request.to_owned(),
        objective: request.trim().to_owned(),
        scope,
        entities: entity_results,
        restrictions,
        reason,
    })
}

fn proposal_brief_reason(request: &str, context_request: &ContextBundleRequest) -> Option<String> {
    let words = request.split_whitespace().count();
    let normalized = request.to_lowercase();
    let broad_markers = [
        "amplia",
        "amplio",
        "todo",
        "toda",
        "completo",
        "completa",
        "varios",
        "varias",
        "desarrolla",
        "expande",
        "reimagina",
        "profundiza",
    ];

    if !query_request_is_write_like(request) {
        return Some("La solicitud no define todavía una mutación concreta.".to_owned());
    }
    if words > 16 {
        return Some(
            "La solicitud es amplia y conviene fijar objetivo y alcance antes de llamar al modelo."
                .to_owned(),
        );
    }
    if broad_markers
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return Some(
            "La solicitud usa lenguaje amplio o ambiguo y necesita un brief editable.".to_owned(),
        );
    }
    if context_request.anchors.is_empty() && words > 8 {
        return Some(
            "La solicitud no ancla entidades concretas para construir un draft seguro.".to_owned(),
        );
    }
    None
}

fn query_request_is_write_like(request: &str) -> bool {
    let normalized = request.to_lowercase();
    [
        "haz",
        "hace",
        "cambia",
        "crear",
        "crea",
        "agrega",
        "añade",
        "anade",
        "elimina",
        "borra",
        "actualiza",
        "modifica",
        "reescribe",
        "replace",
        "change",
        "create",
        "delete",
        "update",
        "make",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn query_classification(classification: AdvisoryClassification) -> SearchClassification {
    match classification {
        AdvisoryClassification::Fact => SearchClassification::Fact,
        AdvisoryClassification::Perspective => SearchClassification::Perspective,
        AdvisoryClassification::Inference => SearchClassification::Inference,
        AdvisoryClassification::NoEvidence => SearchClassification::NoEvidence,
        AdvisoryClassification::Unspecified => SearchClassification::Unspecified,
    }
}

fn resolve_search_result(
    store: &nirmata_store::WorldStore,
    scope: ReadScope,
    uri: String,
) -> Result<SearchResult, AppError> {
    crate::search_use_cases::open_uri(store, scope, &uri).map(|response| response.result)
}

fn resolve_proposal_result(
    store: &nirmata_store::WorldStore,
    scope: ReadScope,
    object: ObjectRef,
    before: Option<&ManualReviewObjectSnapshot>,
    after: Option<&ManualReviewObjectSnapshot>,
    draft_objects: &BTreeMap<String, ManualReviewObjectSnapshot>,
) -> Result<SearchResult, AppError> {
    let uri = object.to_string();
    resolve_search_result(store, scope, uri.clone()).or_else(|error| match error {
        AppError::ObjectNotFound { .. } => {
            if let Some(snapshot) = after.filter(|snapshot| snapshot.target_uri == uri) {
                return snapshot_result(snapshot, "draft_after");
            }
            if let Some(snapshot) = before.filter(|snapshot| snapshot.target_uri == uri) {
                return snapshot_result(snapshot, "draft_before");
            }
            if let Some(snapshot) = draft_objects.get(&uri) {
                return snapshot_result(snapshot, "draft_dependency");
            }
            Err(error)
        }
        _ => Err(error),
    })
}

fn snapshot_result(
    snapshot: &ManualReviewObjectSnapshot,
    provenance: &str,
) -> Result<SearchResult, AppError> {
    let object_ref = ObjectRef::from_str(&snapshot.target_uri)
        .map_err(|_| AppError::InvalidObjectUri(snapshot.target_uri.clone()))?;
    let snippet = snapshot
        .lines
        .iter()
        .map(|line| line.value.clone())
        .find(|value| !value.trim().is_empty() && value != "—")
        .unwrap_or_else(|| snapshot.title.clone());
    Ok(SearchResult {
        object_ref,
        object_type: object_ref.kind(),
        object_id: object_id(object_ref),
        uri: snapshot.target_uri.clone(),
        snippet,
        authority: SearchAuthority::Canonical,
        classification: SearchClassification::Fact,
        provenance: provenance.to_owned(),
        stage: "draft".to_owned(),
        score: 100_000,
        rank: 1,
        score_explanation: "explicit draft object".to_owned(),
    })
}

fn operation_kind(operation: &ChangeOperation) -> &'static str {
    match operation {
        ChangeOperation::UpdateWorld { .. } => "update_world",
        ChangeOperation::CreateEntity { .. } => "create_entity",
        ChangeOperation::UpdateEntity { .. } => "update_entity",
        ChangeOperation::DeleteEntity { .. } => "delete_entity",
        ChangeOperation::CreateRelation { .. } => "create_relation",
        ChangeOperation::UpdateRelation { .. } => "update_relation",
        ChangeOperation::DeleteRelation { .. } => "delete_relation",
        ChangeOperation::CreateEvent { .. } => "create_event",
        ChangeOperation::UpdateEvent { .. } => "update_event",
        ChangeOperation::DeleteEvent { .. } => "delete_event",
        ChangeOperation::CreateGoal { .. } => "create_goal",
        ChangeOperation::UpdateGoal { .. } => "update_goal",
        ChangeOperation::DeleteGoal { .. } => "delete_goal",
        ChangeOperation::CreateRule { .. } => "create_rule",
        ChangeOperation::UpdateRule { .. } => "update_rule",
        ChangeOperation::DeleteRule { .. } => "delete_rule",
        ChangeOperation::CreateClaim { .. } => "create_claim",
        ChangeOperation::UpdateClaim { .. } => "update_claim",
        ChangeOperation::DeleteClaim { .. } => "delete_claim",
        ChangeOperation::CreateDocument { .. } => "create_document",
        ChangeOperation::UpdateDocument { .. } => "update_document",
        ChangeOperation::DeleteDocument { .. } => "delete_document",
    }
}

fn object_id(object: ObjectRef) -> String {
    match object {
        ObjectRef::World(id) => id.to_string(),
        ObjectRef::Entity(id) => id.to_string(),
        ObjectRef::Relation(id) => id.to_string(),
        ObjectRef::Event(id) => id.to_string(),
        ObjectRef::Claim(id) => id.to_string(),
        ObjectRef::Rule(id) => id.to_string(),
        ObjectRef::Goal(id) => id.to_string(),
        ObjectRef::Document(id) => id.to_string(),
    }
}

fn retcon_label(operation: &ChangeOperation) -> &'static str {
    match operation.retcon() {
        nirmata_core::change_set::RetconKind::Additive => "additive",
        nirmata_core::change_set::RetconKind::Replacement => "replacement",
        nirmata_core::change_set::RetconKind::Reinterpretive => "reinterpretive",
    }
}

fn operation_consequence(
    operation: &ChangeOperation,
    before: Option<&ManualReviewObjectSnapshot>,
    after: Option<&ManualReviewObjectSnapshot>,
) -> String {
    match (before, after) {
        (None, Some(after)) => format!("Crear {}", after.target_uri),
        (Some(before), Some(after)) => {
            format!("Actualizar {} -> {}", before.target_uri, after.target_uri)
        }
        (Some(before), None) => format!("Eliminar {}", before.target_uri),
        (None, None) => format!("Aplicar {}", operation.primary_ref()),
    }
}

pub(crate) fn serialize_payload<T: Serialize>(payload: &T, label: &str) -> Result<Value, AppError> {
    serde_json::to_value(payload).map_err(|error| {
        AppError::Ai(AiError::InvalidResponse(format!(
            "could not serialize AI {label} payload: {error}"
        )))
    })
}

pub(crate) fn map_capability_error(error: CapabilityError) -> AppError {
    match error {
        CapabilityError::Ai(error) => error.into(),
        CapabilityError::Serialization(message) => AppError::Ai(AiError::InvalidResponse(message)),
        CapabilityError::StructuredOutput(error) => AppError::Ai(AiError::InvalidResponse(
            format!("structured AI output is invalid: {error}"),
        )),
    }
}

fn parsing_failure(error: &StructuredOutputError) -> AiParsingFailure {
    AiParsingFailure {
        kind: error.kind(),
        message: error.message().to_owned(),
        diagnostic: error.diagnostic().clone(),
    }
}

fn evaluation_needs_repair(
    deterministic_report: &ValidationReport,
    critique_report: &CritiqueReport,
) -> bool {
    deterministic_report
        .errors
        .iter()
        .chain(deterministic_report.conflicts.iter())
        .any(|issue| issue.code != "change_set.replacement_decision_unresolved")
        || critique_blocks_review(critique_report)
}

fn deterministic_blocks_review(report: &ValidationReport) -> bool {
    report
        .errors
        .iter()
        .chain(report.conflicts.iter())
        .any(|issue| issue.code != "change_set.replacement_decision_unresolved")
}

fn critique_blocks_review(report: &CritiqueReport) -> bool {
    report
        .issues
        .iter()
        .any(|issue| issue.severity == ValidationSeverity::Conflict)
}

fn merge_source_anchors(context_request: &mut ContextBundleRequest, sources: &[ObjectRef]) {
    for source in sources {
        if !context_request.anchors.contains(source) {
            context_request.anchors.push(*source);
        }
    }
}

fn allowed_critique_uris(
    draft: &ChangeSetDraft,
    snapshot: &AiAffectedSubgraphSnapshot,
) -> BTreeSet<String> {
    let mut uris = BTreeSet::from([ObjectRef::World(draft.world_id()).to_string()]);
    uris.extend(draft.sources().iter().map(ToString::to_string));
    for operation in draft.operations() {
        uris.insert(operation.primary_ref().to_string());
        uris.extend(operation.affected_ids().iter().map(ToString::to_string));
    }
    uris.extend(
        snapshot
            .entities
            .iter()
            .map(|entity| ObjectRef::Entity(entity.id()).to_string()),
    );
    uris.extend(
        snapshot
            .relations
            .iter()
            .map(|relation| ObjectRef::Relation(relation.id()).to_string()),
    );
    uris.extend(
        snapshot
            .goals
            .iter()
            .map(|goal| ObjectRef::Goal(goal.id()).to_string()),
    );
    uris.extend(
        snapshot
            .events
            .iter()
            .map(|event| ObjectRef::Event(event.id()).to_string()),
    );
    uris.extend(
        snapshot
            .rules
            .iter()
            .map(|rule| ObjectRef::Rule(rule.id()).to_string()),
    );
    uris.extend(
        snapshot
            .claims
            .iter()
            .map(|claim| ObjectRef::Claim(claim.id()).to_string()),
    );
    uris.extend(
        snapshot
            .documents
            .iter()
            .map(|document| ObjectRef::Document(document.id()).to_string()),
    );
    for reference in &snapshot.content_references {
        uris.insert(reference.source().to_string());
        uris.insert(reference.target().to_string());
    }
    uris
}

fn validate_critique_references(
    input: &AiCritiqueInput,
    report: &CritiqueReport,
) -> Result<(), AppError> {
    let operation_ids = input
        .draft
        .operations()
        .iter()
        .map(|operation| operation.operation_id().to_string())
        .collect::<BTreeSet<_>>();
    let mut allowed_uris = input
        .context_object_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    allowed_uris.extend(allowed_critique_uris(
        &input.draft,
        &input.affected_subgraph,
    ));

    for issue in &report.issues {
        for operation_id in &issue.affected_operation_ids {
            if !operation_ids.contains(&operation_id.to_string()) {
                return Err(invalid_critique_reference(format!(
                    "issue {} cites operation {operation_id} outside the draft",
                    issue.issue_id.as_str()
                )));
            }
        }

        for uri in issue
            .summary
            .content_references
            .iter()
            .chain(issue.related_object_uris.iter())
            .chain(issue.evidence.iter().map(|evidence| &evidence.source_uri))
            .chain(
                issue
                    .suggested_resolution
                    .iter()
                    .flat_map(|resolution| resolution.content_references.iter()),
            )
        {
            let uri = String::from(*uri);
            if !allowed_uris.contains(&uri) {
                return Err(invalid_critique_reference(format!(
                    "issue {} cites {uri} outside the critic snapshot",
                    issue.issue_id.as_str()
                )));
            }
        }

        if let Some(claim_id) = issue.target_claim_id {
            let claim_uri = ObjectRef::Claim(claim_id).to_string();
            if !allowed_uris.contains(&claim_uri) {
                return Err(invalid_critique_reference(format!(
                    "issue {} targets claim {claim_uri} outside the critic snapshot",
                    issue.issue_id.as_str()
                )));
            }
        }
    }

    Ok(())
}

fn invalid_critique_reference(message: String) -> AppError {
    AppError::Ai(AiError::InvalidResponse(format!(
        "critique report is not grounded: {message}"
    )))
}

#[cfg(test)]
#[path = "../tests/unit/ai.rs"]
mod tests;
