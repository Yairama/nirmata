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
    contracts::{AdvisoryClassification, AdvisoryResponse},
};
use nirmata_core::{
    RevisionId, WorldId,
    change_set::{ChangeOperation, ChangeSetDraft},
    document::ObjectRef,
    validation::ValidationReport,
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
    pub snapshot: AiContextSnapshot,
    pub context_object_ids: Vec<String>,
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
        let context_object_ids = snapshot.context_object_ids();
        Ok(AiCritiqueInput {
            mode: AiMode::Critic,
            request: request.into(),
            draft: draft.clone(),
            snapshot,
            context_object_ids,
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
        let request_options = options.into_request_options();

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
            .execute_ai_proposal_input_with(client, request, prepared, options, &mut on_progress)
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
            .execute_ai_proposal_input_with(client, request, prepared, options, &mut on_progress)
            .await?;
        on_progress(AiProposalProgress::Completed);
        Ok(response)
    }

    async fn execute_ai_proposal_input_with<C, F>(
        &self,
        client: &C,
        request: String,
        prepared: AiProposalInput,
        options: AiRequestOptions,
        on_progress: &mut F,
    ) -> Result<AiProposalDraftResponse, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        let payload = serialize_payload(&prepared, "proposal")?;
        let request_options = options.into_request_options();

        on_progress(AiProposalProgress::CallingModel);
        let invocation = client
            .run_proposal(
                payload,
                prepared.context_object_ids.clone(),
                request_options,
            )
            .await
            .map_err(map_capability_error)?;

        on_progress(AiProposalProgress::Validating);
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        build_proposal_draft_response(&active.store, request, prepared.snapshot, invocation)
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
) -> Result<AiProposalDraftResponse, AppError> {
    let CapabilityInvocation {
        output: draft,
        metadata,
    } = invocation;
    let mut validation_report = store.validate_change_set_draft(&draft)?;
    annotate_report_with_change_operations(&mut validation_report, draft.operations());

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
        ready_for_review: validation_report.is_ok(),
        validation_report,
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

fn merge_source_anchors(context_request: &mut ContextBundleRequest, sources: &[ObjectRef]) {
    for source in sources {
        if !context_request.anchors.contains(source) {
            context_request.anchors.push(*source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextIntent, NirmataApp};
    use nirmata_core::{
        ChangeOperationId, Period, World,
        change_set::{ChangeOperation, RetconKind},
        claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
        document::ObjectRef,
        entity::{Entity, EntityKind},
        rule::{Rule, RuleKind, RuleSeverity},
    };
    use nirmata_store::WorldStore;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[derive(Clone)]
    struct FakeClient {
        query_response: AdvisoryResponse,
        query_deltas: Vec<String>,
        query_delay: Duration,
        proposal_draft: ChangeSetDraft,
        proposal_delay: Duration,
        query_calls: Arc<Mutex<usize>>,
        proposal_calls: Arc<Mutex<usize>>,
    }

    impl FakeClient {
        fn new(query_response: AdvisoryResponse, proposal_draft: ChangeSetDraft) -> Self {
            Self {
                query_response,
                query_deltas: vec![],
                query_delay: Duration::ZERO,
                proposal_draft,
                proposal_delay: Duration::ZERO,
                query_calls: Arc::new(Mutex::new(0)),
                proposal_calls: Arc::new(Mutex::new(0)),
            }
        }

        fn with_query_deltas(mut self, deltas: Vec<&str>) -> Self {
            self.query_deltas = deltas.into_iter().map(str::to_owned).collect();
            self
        }

        fn with_query_delay(mut self, delay: Duration) -> Self {
            self.query_delay = delay;
            self
        }

        fn proposal_calls(&self) -> usize {
            *self.proposal_calls.lock().expect("proposal calls lock")
        }
    }

    impl AiModeClient for FakeClient {
        fn run_query<'a, F>(
            &'a self,
            _payload: Value,
            context_object_ids: Vec<String>,
            options: RequestOptions,
            mut on_delta: F,
        ) -> ClientFuture<'a, Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>>
        where
            F: FnMut(StreamDelta) + Send + 'a,
        {
            Box::pin(async move {
                *self.query_calls.lock().expect("query calls lock") += 1;
                sleep_or_cancel(self.query_delay, options.cancellation.clone()).await?;
                for delta in &self.query_deltas {
                    on_delta(StreamDelta {
                        delta: delta.clone(),
                    });
                }
                Ok(CapabilityInvocation {
                    output: self.query_response.clone(),
                    metadata: test_metadata("query_test", context_object_ids),
                })
            })
        }

        fn run_proposal<'a>(
            &'a self,
            _payload: Value,
            context_object_ids: Vec<String>,
            options: RequestOptions,
        ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>>
        {
            Box::pin(async move {
                *self.proposal_calls.lock().expect("proposal calls lock") += 1;
                sleep_or_cancel(self.proposal_delay, options.cancellation.clone()).await?;
                Ok(CapabilityInvocation {
                    output: self.proposal_draft.clone(),
                    metadata: test_metadata("proposal_test", context_object_ids),
                })
            })
        }
    }

    struct SeededWorld {
        mara: Entity,
        _sera: Entity,
        rumor: Claim,
        _rule: Rule,
    }

    fn test_metadata(prompt_version: &str, context_object_ids: Vec<String>) -> InvocationMetadata {
        InvocationMetadata {
            model: "fake-model".to_owned(),
            prompt_version: prompt_version.to_owned(),
            context_object_ids,
            status: nirmata_ai::capabilities::InvocationStatus::Completed,
            usage: None,
        }
    }

    async fn sleep_or_cancel(
        delay: Duration,
        cancellation: Option<CancellationToken>,
    ) -> Result<(), CapabilityError> {
        if delay.is_zero() {
            if cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
            {
                return Err(CapabilityError::Ai(AiError::RequestCancelled));
            }
            return Ok(());
        }

        match cancellation {
            Some(token) => {
                tokio::select! {
                    _ = token.cancelled() => Err(CapabilityError::Ai(AiError::RequestCancelled)),
                    _ = tokio::time::sleep(delay) => Ok(()),
                }
            }
            None => {
                tokio::time::sleep(delay).await;
                Ok(())
            }
        }
    }

    fn project_path(label: &str) -> PathBuf {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
        fs::create_dir_all(&directory).expect("create test directory");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
    }

    fn base_world(path: &Path) -> World {
        let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
        WorldStore::create(path, &world).expect("create store");
        world
    }

    fn open_app(path: &Path) -> NirmataApp {
        let mut app = NirmataApp::default();
        app.open_world(path.to_path_buf()).expect("open world");
        app
    }

    fn seed_world(path: &Path) -> SeededWorld {
        let world = base_world(path);
        let mut store = WorldStore::open(path).expect("open store");
        let mara = Entity::new(
            world.id(),
            EntityKind::Person,
            "Mara",
            "mara",
            "Cartógrafa del puerto",
            "",
            "{}",
            vec![],
            1,
        )
        .expect("mara");
        let sera = Entity::new(
            world.id(),
            EntityKind::Person,
            "Sera",
            "sera",
            "Cronista de taberna",
            "",
            "{}",
            vec![],
            1,
        )
        .expect("sera");
        store.insert_entity(&mara).expect("insert mara");
        store.insert_entity(&sera).expect("insert sera");

        let rumor = Claim::new(
            world.id(),
            mara.id(),
            "Sera cree que Mara negocia con contrabandistas.",
            Some("rumor.mara".to_owned()),
            Some(ClaimObject::Scalar("true".to_owned())),
            ClaimPolarity::Positive,
            ClaimAuthentication::Attributed,
            Some(sera.id()),
            Some(ClaimModality::Belief),
            Some("rumor".to_owned()),
            Some("testigo".to_owned()),
            Some("taberna".to_owned()),
            None,
            None,
            Some(0.6),
            Some(Period::new(Some(12), Some(12)).expect("period")),
            world.current_revision(),
        )
        .expect("rumor");
        store.insert_claim(&rumor).expect("insert rumor");

        let rule = Rule::new(
            world.id(),
            RuleKind::Institutional,
            "Los guardianes del puerto no abandonan su puesto.",
            "person",
            RuleSeverity::Advisory,
            Some("código del puerto".to_owned()),
            None,
            "{}",
            1,
        )
        .expect("rule");
        store.insert_rule(&rule).expect("insert rule");

        SeededWorld {
            mara,
            _sera: sera,
            rumor,
            _rule: rule,
        }
    }

    fn context_request(anchor: ObjectRef) -> ContextBundleRequest {
        let mut request = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);
        request.anchors = vec![anchor];
        request.include_perspectives = true;
        request
    }

    fn advisory_response(items: Vec<nirmata_ai::contracts::AdvisoryItem>) -> AdvisoryResponse {
        AdvisoryResponse { items }
    }

    fn draft_for_new_faction(
        world: &World,
        source: ObjectRef,
        name: &str,
        slug: &str,
    ) -> ChangeSetDraft {
        let after = Entity::new(
            world.id(),
            EntityKind::Faction,
            name,
            slug,
            "Nueva facción del puerto",
            "",
            "{}",
            vec![],
            1,
        )
        .expect("proposal entity");
        ChangeSetDraft::new(
            world.id(),
            world.current_revision(),
            format!("Crear {name}"),
            vec![source],
            vec!["La facción aún no existe en el canon.".to_owned()],
            vec![ChangeOperation::CreateEntity {
                operation_id: ChangeOperationId::new(),
                affected_ids: vec![ObjectRef::Entity(after.id())],
                expected_version: 0,
                retcon: RetconKind::Additive,
                after,
            }],
            vec![],
        )
        .expect("proposal draft")
    }

    fn invalid_additive_delete_draft(world: &World, entity: &Entity) -> ChangeSetDraft {
        ChangeSetDraft::new(
            world.id(),
            world.current_revision(),
            format!("Eliminar {} aditivamente", entity.name()),
            vec![ObjectRef::Entity(entity.id())],
            vec![],
            vec![ChangeOperation::DeleteEntity {
                operation_id: ChangeOperationId::new(),
                affected_ids: vec![ObjectRef::Entity(entity.id())],
                expected_version: entity.version(),
                retcon: RetconKind::Additive,
                before: entity.clone(),
            }],
            vec![],
        )
        .expect("invalid additive delete draft")
    }

    #[tokio::test]
    async fn query_streams_citations_and_offers_proposal_action_for_write_requests() {
        let path = project_path("ai-query-write");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let fake = FakeClient::new(
            advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
                item_id: "impact-1".to_owned().try_into().expect("item id"),
                classification: AdvisoryClassification::Fact,
                answer: nirmata_ai::contracts::ReferencedMarkdown {
                    markdown: "Mara ya controla el puerto norte.".to_owned(),
                    content_references: vec![
                        format!("nirmata://entity/{}", seeded.mara.id())
                            .try_into()
                            .expect("uri"),
                    ],
                },
                citations: vec![nirmata_ai::contracts::AdvisoryCitation {
                    source_uri: format!("nirmata://entity/{}", seeded.mara.id())
                        .try_into()
                        .expect("uri"),
                    quote_md: "Cartógrafa del puerto".to_owned(),
                }],
            }]),
            draft_for_new_faction(
                &world,
                ObjectRef::Entity(seeded.mara.id()),
                "Guardia Norte",
                "guardia-norte",
            ),
        )
        .with_query_deltas(vec!["{\"items\":[", "{\"itemId\":\"impact-1\"}", "]}"]);
        let mut progress = Vec::new();

        let response = app
            .execute_ai_query_with(
                &fake,
                "Haz independiente la ciudad del puerto".to_owned(),
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::default(),
                |event| progress.push(event),
            )
            .await
            .expect("query response");

        assert_eq!(response.items.len(), 1);
        assert_eq!(response.items[0].classification, SearchClassification::Fact);
        assert_eq!(response.items[0].content_references.len(), 1);
        assert_eq!(
            response.items[0].content_references[0].uri,
            format!("nirmata://entity/{}", seeded.mara.id())
        );
        assert_eq!(response.items[0].citations.len(), 1);
        assert_eq!(
            response.proposal_action,
            Some(AiProposalAction {
                action: "start_proposal",
                label: "Iniciar propuesta revisable".to_owned(),
                request: "Haz independiente la ciudad del puerto".to_owned(),
            })
        );
        assert!(progress.contains(&AiQueryProgress::PreparingContext));
        assert!(progress.contains(&AiQueryProgress::CallingModel));
        assert!(progress.contains(&AiQueryProgress::Completed));
        assert!(progress.iter().any(|event| matches!(
            event,
            AiQueryProgress::StreamingDelta { delta } if delta.contains("impact-1")
        )));

        drop(app);
        fs::remove_file(path).expect("remove project");
    }

    #[tokio::test]
    async fn query_keeps_perspectives_and_no_evidence_without_inventing_sources() {
        let path = project_path("ai-query-rumor");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let fake = FakeClient::new(
            advisory_response(vec![
                nirmata_ai::contracts::AdvisoryItem {
                    item_id: "rumor-1".to_owned().try_into().expect("item id"),
                    classification: AdvisoryClassification::Perspective,
                    answer: nirmata_ai::contracts::ReferencedMarkdown {
                        markdown: "Sera sospecha que Mara favorece a contrabandistas.".to_owned(),
                        content_references: vec![
                            format!("nirmata://claim/{}", seeded.rumor.id())
                                .try_into()
                                .expect("uri"),
                        ],
                    },
                    citations: vec![nirmata_ai::contracts::AdvisoryCitation {
                        source_uri: format!("nirmata://claim/{}", seeded.rumor.id())
                            .try_into()
                            .expect("uri"),
                        quote_md: "Sera cree que Mara negocia con contrabandistas.".to_owned(),
                    }],
                },
                nirmata_ai::contracts::AdvisoryItem {
                    item_id: "empty-1".to_owned().try_into().expect("item id"),
                    classification: AdvisoryClassification::NoEvidence,
                    answer: nirmata_ai::contracts::ReferencedMarkdown {
                        markdown: "No hay evidencia recuperada sobre pactos formales.".to_owned(),
                        content_references: vec![],
                    },
                    citations: vec![],
                },
            ]),
            draft_for_new_faction(
                &world,
                ObjectRef::Entity(seeded.mara.id()),
                "Liga del Faro",
                "liga-del-faro",
            ),
        );

        let response = app
            .execute_ai_query_with(
                &fake,
                "¿Qué rumores rodean a Mara?".to_owned(),
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::default(),
                |_| {},
            )
            .await
            .expect("query response");

        assert_eq!(response.items.len(), 2);
        assert_eq!(
            response.items[0].classification,
            SearchClassification::Perspective
        );
        assert_eq!(
            response.items[0].content_references[0].classification,
            SearchClassification::Perspective
        );
        assert_eq!(
            response.items[1].classification,
            SearchClassification::NoEvidence
        );
        assert!(response.items[1].content_references.is_empty());
        assert!(response.items[1].citations.is_empty());
        assert!(response.proposal_action.is_none());

        drop(app);
        fs::remove_file(path).expect("remove project");
    }

    #[tokio::test]
    async fn query_cancellation_stops_the_request_and_keeps_the_app_usable() {
        let path = project_path("ai-query-cancel");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let fake = FakeClient::new(
            advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
                item_id: "unused-1".to_owned().try_into().expect("item id"),
                classification: AdvisoryClassification::Fact,
                answer: nirmata_ai::contracts::ReferencedMarkdown {
                    markdown: "Respuesta tardía".to_owned(),
                    content_references: vec![
                        format!("nirmata://entity/{}", seeded.mara.id())
                            .try_into()
                            .expect("uri"),
                    ],
                },
                citations: vec![nirmata_ai::contracts::AdvisoryCitation {
                    source_uri: format!("nirmata://entity/{}", seeded.mara.id())
                        .try_into()
                        .expect("uri"),
                    quote_md: "Cartógrafa del puerto".to_owned(),
                }],
            }]),
            draft_for_new_faction(
                &world,
                ObjectRef::Entity(seeded.mara.id()),
                "Custodia del Puerto",
                "custodia-del-puerto",
            ),
        )
        .with_query_delay(Duration::from_millis(50));
        let cancellation = CancellationToken::new();
        let cancel_after = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            cancel_after.cancel();
        });

        let error = app
            .execute_ai_query_with(
                &fake,
                "Explica el puerto".to_owned(),
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::new(Duration::from_secs(1)).with_cancellation(cancellation),
                |_| {},
            )
            .await
            .expect_err("query must be cancelled");

        assert!(matches!(error, AppError::Ai(AiError::RequestCancelled)));
        let uri = format!("nirmata://entity/{}", seeded.mara.id());
        let opened = app.open_uri(&uri).expect("app remains usable");
        assert_eq!(opened.result.uri, uri);

        drop(app);
        fs::remove_file(path).expect("remove project");
    }

    #[tokio::test]
    async fn proposal_returns_intent_brief_for_broad_requests_without_calling_the_model() {
        let path = project_path("ai-proposal-brief");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let fake = FakeClient::new(
            advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
                item_id: "unused-1".to_owned().try_into().expect("item id"),
                classification: AdvisoryClassification::NoEvidence,
                answer: nirmata_ai::contracts::ReferencedMarkdown {
                    markdown: "Sin uso".to_owned(),
                    content_references: vec![],
                },
                citations: vec![],
            }]),
            draft_for_new_faction(
                &world,
                ObjectRef::Entity(seeded.mara.id()),
                "Liga del Muelle",
                "liga-del-muelle",
            ),
        );
        let mut progress = Vec::new();

        let response = app
            .execute_ai_proposal_with(
                &fake,
                "Desarrolla una reorganización completa de la política del puerto y de todos sus actores principales.".to_owned(),
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::default(),
                |event| progress.push(event),
            )
            .await
            .expect("proposal outcome");

        let AiProposalResponse::IntentBrief { brief, .. } = response else {
            panic!("expected an intent brief");
        };
        assert!(!brief.reason.is_empty());
        assert!(!brief.entities.is_empty());
        assert!(
            brief
                .restrictions
                .iter()
                .any(|restriction| restriction.contains("Conservar la revisión base"))
        );
        assert_eq!(fake.proposal_calls(), 0);
        assert!(progress.contains(&AiProposalProgress::IntentBriefReady));

        drop(app);
        fs::remove_file(path).expect("remove project");
    }

    #[tokio::test]
    async fn proposal_generates_a_ready_draft_for_small_requests() {
        let path = project_path("ai-proposal-direct");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let draft = draft_for_new_faction(
            &world,
            ObjectRef::Entity(seeded.mara.id()),
            "Guardia Norte",
            "guardia-norte",
        );
        let fake = FakeClient::new(
            advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
                item_id: "unused-1".to_owned().try_into().expect("item id"),
                classification: AdvisoryClassification::NoEvidence,
                answer: nirmata_ai::contracts::ReferencedMarkdown {
                    markdown: "Sin uso".to_owned(),
                    content_references: vec![],
                },
                citations: vec![],
            }]),
            draft.clone(),
        );

        let response = app
            .execute_ai_proposal_with(
                &fake,
                "Crea una nueva facción que proteja el puerto.".to_owned(),
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::default(),
                |_| {},
            )
            .await
            .expect("proposal response");

        let AiProposalResponse::Draft(draft_response) = response else {
            panic!("expected a draft");
        };
        assert_eq!(draft_response.draft, draft);
        assert!(draft_response.ready_for_review);
        assert_eq!(draft_response.sources.len(), 1);
        assert_eq!(draft_response.operations.len(), 1);
        assert_eq!(draft_response.operations[0].kind, "create_entity");
        assert_eq!(draft_response.operations[0].retcon, "additive");
        assert!(draft_response.operations[0].after.is_some());
        assert!(!draft_response.consequences.is_empty());
        assert!(draft_response.validation_report.is_ok());

        drop(app);
        fs::remove_file(path).expect("remove project");
    }

    #[tokio::test]
    async fn proposal_marks_invalid_drafts_as_not_ready_for_review() {
        let path = project_path("ai-proposal-invalid");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let invalid = invalid_additive_delete_draft(&world, &seeded.mara);
        let fake = FakeClient::new(
            advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
                item_id: "unused-1".to_owned().try_into().expect("item id"),
                classification: AdvisoryClassification::NoEvidence,
                answer: nirmata_ai::contracts::ReferencedMarkdown {
                    markdown: "Sin uso".to_owned(),
                    content_references: vec![],
                },
                citations: vec![],
            }]),
            invalid,
        );

        let response = app
            .execute_ai_proposal_with(
                &fake,
                "Crea una nueva facción llamada Mara.".to_owned(),
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::default(),
                |_| {},
            )
            .await
            .expect("proposal response");

        let AiProposalResponse::Draft(draft_response) = response else {
            panic!("expected a draft");
        };
        assert!(!draft_response.ready_for_review);
        assert!(draft_response.validation_report.has_errors());
        assert!(
            draft_response
                .validation_report
                .errors
                .iter()
                .any(|issue| issue.code == "change_set.retcon.additive_delete")
        );

        drop(app);
        fs::remove_file(path).expect("remove project");
    }

    #[tokio::test]
    async fn proposal_can_resume_from_an_intent_brief() {
        let path = project_path("ai-proposal-resume");
        let seeded = seed_world(&path);
        let world = WorldStore::open(&path)
            .expect("open store")
            .load_world()
            .expect("load world");
        let app = open_app(&path);
        let fake = FakeClient::new(
            advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
                item_id: "unused-1".to_owned().try_into().expect("item id"),
                classification: AdvisoryClassification::NoEvidence,
                answer: nirmata_ai::contracts::ReferencedMarkdown {
                    markdown: "Sin uso".to_owned(),
                    content_references: vec![],
                },
                citations: vec![],
            }]),
            draft_for_new_faction(
                &world,
                ObjectRef::Entity(seeded.mara.id()),
                "Vigías del Dique",
                "vigias-del-dique",
            ),
        );
        let brief = IntentBrief {
            user_request: "Reorganiza la política del puerto".to_owned(),
            objective: "Crear una facción menor para estabilizar el puerto".to_owned(),
            scope: "Cambios acotados al entorno de Mara.".to_owned(),
            entities: vec![
                app.open_uri(&format!("nirmata://entity/{}", seeded.mara.id()))
                    .expect("open mara")
                    .result,
            ],
            restrictions: vec!["No inventar datos fuera del contexto recuperado.".to_owned()],
            reason: "La solicitud original era amplia.".to_owned(),
        };

        let response = app
            .execute_ai_proposal_from_intent_brief_with(
                &fake,
                &brief,
                &context_request(ObjectRef::Entity(seeded.mara.id())),
                AiRequestOptions::default(),
                |_| {},
            )
            .await
            .expect("proposal from brief");

        assert!(response.ready_for_review);
        assert!(response.request.contains("Objetivo:"));
        assert_eq!(
            response.sources[0].uri,
            format!("nirmata://entity/{}", seeded.mara.id())
        );

        drop(app);
        fs::remove_file(path).expect("remove project");
    }
}
