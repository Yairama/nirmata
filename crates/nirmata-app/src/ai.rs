use crate::{
    AppError, ContextBundle, ContextBundleRequest, SearchAuthority, SearchClassification,
    SearchResult,
    manual_review::{
        ManualReviewObjectSnapshot, annotate_report_with_change_operations,
        operation_object_snapshot_after, operation_object_snapshot_before,
    },
};
use nirmata_ai::{
    AiError, CancellationToken, RequestOptions, StreamDelta,
    capabilities::{
        AzureFoundryCapabilityClient, CapabilityError, CapabilityInvocation, InvocationMetadata,
    },
    contracts::{
        AdvisoryClassification, AdvisoryResponse, CritiqueReport, StructuredOutputDiagnostic,
        StructuredOutputError, StructuredOutputErrorKind,
    },
};
use nirmata_core::{
    RevisionId, WorldId,
    change_set::{ChangeOperation, ChangeSetDraft},
    claim::Claim,
    document::{ContentReference, Document, ObjectRef},
    entity::Entity,
    event::{Event, EventLink},
    goal::Goal,
    relation::Relation,
    rule::Rule,
    validation::{ValidationReport, ValidationSeverity},
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, future::Future, pin::Pin, str::FromStr, time::Duration};

type ClientFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiMode {
    Query,
    Propose,
    Critic,
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

    fn into_request_options(self) -> RequestOptions {
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

trait AiModeClient {
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
}

impl crate::NirmataApp {
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
        let snapshot = self.build_ai_context_snapshot(context_request)?;
        let context_object_ids = snapshot.context_object_ids();
        Ok(AiProposalInput {
            mode: AiMode::Propose,
            request: request.into(),
            snapshot,
            context_object_ids,
        })
    }

    pub fn prepare_ai_critique(
        &self,
        request: impl Into<String>,
        draft: &ChangeSetDraft,
        context_request: &ContextBundleRequest,
    ) -> Result<AiCritiqueInput, AppError> {
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
            .map(|item| query_item_from_contract(&active.store, item))
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

    fn build_ai_context_snapshot(
        &self,
        context_request: &ContextBundleRequest,
    ) -> Result<AiContextSnapshot, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let world = active.store.load_world()?;
        let context = crate::context_bundle::build_context_bundle(&active.store, context_request)?;
        Ok(AiContextSnapshot {
            world_id: world.id(),
            base_revision: world.current_revision(),
            context,
        })
    }

    fn provider_client(
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

    let mut affected_seen = BTreeSet::new();
    let mut affected_objects = Vec::new();
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
                    resolve_proposal_result(store, object, before.as_ref(), after.as_ref())
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
        .map(|source| resolve_search_result(store, source.to_string()))
        .collect::<Result<Vec<_>, AppError>>()?;

    let ready_for_review = validation_report.is_ok()
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
            .map(|reference| resolve_search_result(store, String::from(reference)))
            .collect::<Result<Vec<_>, _>>()?,
        citations: item
            .citations
            .iter()
            .map(|citation| {
                Ok(AiQueryCitation {
                    quote_md: citation.quote_md.clone(),
                    source: resolve_search_result(store, String::from(citation.source_uri))?,
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
        .map(|uri| resolve_search_result(store, uri))
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
    uri: String,
) -> Result<SearchResult, AppError> {
    crate::search_use_cases::open_uri(store, &uri).map(|response| response.result)
}

fn resolve_proposal_result(
    store: &nirmata_store::WorldStore,
    object: ObjectRef,
    before: Option<&ManualReviewObjectSnapshot>,
    after: Option<&ManualReviewObjectSnapshot>,
) -> Result<SearchResult, AppError> {
    let uri = object.to_string();
    resolve_search_result(store, uri.clone()).or_else(|error| match error {
        AppError::ObjectNotFound { .. } => {
            if let Some(snapshot) = after.filter(|snapshot| snapshot.target_uri == uri) {
                return snapshot_result(snapshot, "draft_after");
            }
            if let Some(snapshot) = before.filter(|snapshot| snapshot.target_uri == uri) {
                return snapshot_result(snapshot, "draft_before");
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

fn serialize_payload<T: Serialize>(payload: &T, label: &str) -> Result<Value, AppError> {
    serde_json::to_value(payload).map_err(|error| {
        AppError::Ai(AiError::InvalidResponse(format!(
            "could not serialize AI {label} payload: {error}"
        )))
    })
}

fn map_capability_error(error: CapabilityError) -> AppError {
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
