#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nirmata_app::{
    AiError, AiProviderConfig, AiQueryResponse, AiRequestOptions, AiRunId, AiRunSnapshot, AppError,
    CancellationToken, ContextBudget, ContextBundleRequest, ContextIntent, CreateWorldInput,
    DeepReviewMode, DeepReviewPlan, DeepReviewRun, DeepReviewRunId, EmptySearchClassification,
    EntityId, EventId, ExportSnapshotInput, ExportSnapshotResult, ImportBatchSnapshot,
    ImportCandidate, ImportCandidateDecisionRequest, ImportCandidateSnapshot, ImportChunkLocation,
    ImportExtractionResult, ImportReviewPreparation, ImportSnapshotInput, ImportSnapshotResult,
    IntentBrief, InternalDocumentKind, InternalDocumentRequest, LogicalVfsDirectory,
    ManualDraftRequest, ManualDraftResponse, ManualReviewActionRequest, ManualReviewSnapshot,
    MergeReviewResult, NarrativeCausalThreads, NarrativeContinuityExploration,
    NarrativeContinuityProposal, NarrativeContinuitySelection, NarrativeLooseEnds,
    NarrativeTimeline, NirmataApp, ObjectRef, OpenUriResponse, ProviderCredentialStatus, ReadScope,
    RelatedContextRequest, RelatedContextResponse, RevisionHistorySnapshot, RevisionId,
    SearchWorldRequest, SearchWorldResponse, SimulationPromotionInput, SimulationRun,
    SimulationScenario, SimulationScenarioId, SimulationScenarioInput, SpecialistRole, StoreError,
    StructuredSearchKind, TimelineOverview, Variant, VariantComparison, VariantId, WorldSession,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Mutex, MutexGuard, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, State};

struct AiCancellations(Mutex<HashMap<String, CancellationToken>>);
static AI_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Deserialize)]
struct CreateWorldRequest {
    path: PathBuf,
    name: String,
    premise_md: String,
    epoch_label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchWorldCommand {
    query_text: Option<String>,
    kind: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct RelatedContextCommand {
    uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportSnapshotCommand {
    parent_directory: PathBuf,
    snapshot_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportSnapshotCommand {
    snapshot_directory: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateLoreImportCommand {
    source_file: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoreImportCommand {
    batch_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoreImportChunkCommand {
    batch_id: String,
    chunk_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplaceLoreSourceCommand {
    batch_id: String,
    source_id: String,
    source_file: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecideLoreCandidateCommand {
    batch_id: String,
    decision: ImportCandidateDecisionRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditLoreCandidateCommand {
    batch_id: String,
    candidate_id: String,
    replacement: ImportCandidate,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiLoreImportCommand {
    request_id: String,
    batch_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualReviewActionCommand {
    review_key: String,
    action: ManualReviewActionRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewKeyCommand {
    review_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewOperationCommand {
    review_key: String,
    operation_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RevisionCommand {
    revision_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateVariantCommand {
    name: String,
    from_revision_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameVariantCommand {
    variant_id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VariantCommand {
    variant_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveVariantCommand {
    variant_id: String,
    allow_referenced: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadScopeCommand {
    scope: ReadScope,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompareScopesCommand {
    left: ReadScope,
    right: ReadScope,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManualReviewEditCommand {
    review_key: String,
    operation_id: String,
    request: ManualDraftRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiRequestCommand {
    request_id: String,
    request: String,
    anchor_uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiRunCommand {
    request_id: String,
    run_id: String,
    anchor_uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiIntentBriefCommand {
    request_id: String,
    user_request: String,
    objective: String,
    scope: String,
    entity_uris: Vec<String>,
    restrictions: Vec<String>,
    reason: String,
    anchor_uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiCritiqueDecisionCommand {
    run_id: String,
    issue_id: String,
    judgment: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeepReviewPlanCommand {
    mode: String,
    request: String,
    anchor_uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeepReviewExecuteCommand {
    request_id: String,
    mode: String,
    request: String,
    roles: Vec<SpecialistRole>,
    anchor_uri: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateSimulationScenarioCommand {
    scenario: SimulationScenarioInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateSimulationScenarioCommand {
    scenario_id: String,
    scenario: SimulationScenarioInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SimulationScenarioCommand {
    scenario_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrepareSimulationReviewCommand {
    scenario_id: String,
    promotion: SimulationPromotionInput,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NarrativeReadScopeCommand {
    variant_id: String,
    revision_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeriveNarrativeTimelineCommand {
    scope: Option<NarrativeReadScopeCommand>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeriveCausalThreadsCommand {
    scope: Option<NarrativeReadScopeCommand>,
    start_event_ids: Option<Vec<String>>,
    max_depth: u8,
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeriveLooseEndsCommand {
    scope: Option<NarrativeReadScopeCommand>,
}

#[derive(Clone, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum NarrativeContinuitySelectionCommand {
    LooseEnd { code: String, object_uri: String },
    CausalThread { start_event_id: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExploreNarrativeContinuityCommand {
    scope: Option<NarrativeReadScopeCommand>,
    selection: NarrativeContinuitySelectionCommand,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateInternalDocumentCommand {
    request_id: String,
    document_kind: String,
    title: String,
    request: String,
    perspective_entity_id: String,
    tick: i64,
    anchor_uris: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposeNarrativeContinuityCommand {
    request_id: String,
    scope: Option<NarrativeReadScopeCommand>,
    selection: NarrativeContinuitySelectionCommand,
    alternative_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiProgressEvent<T> {
    request_id: String,
    progress: T,
}

#[derive(Serialize)]
struct CommandError {
    code: &'static str,
    message: String,
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        let code = match &error {
            AppError::WorldAlreadyOpen => "world_already_open",
            AppError::NoWorldOpen => "no_world_open",
            AppError::ReadOnlyScope => "read_only_scope",
            AppError::ManualReviewNotReady => "manual_review_not_ready",
            AppError::ManualReviewStale { .. } => "manual_review_stale",
            AppError::ManualReviewVariantMismatch { .. } => "manual_review_stale",
            AppError::ManualReviewRevalidationFailed => "manual_review_revalidation_failed",
            AppError::NoUndoableRevision => "no_undoable_revision",
            AppError::UndoTargetNotCurrentLogicalAncestor { .. } => "undo_target_invalid",
            AppError::UndoConflict { .. } => "undo_conflict",
            AppError::FileAlreadyExists(_) => "file_already_exists",
            AppError::FileNotFound(_) => "file_not_found",
            AppError::InvalidProjectPath(_) => "invalid_project_path",
            AppError::InvalidProjectFormat(_) => "invalid_project_format",
            AppError::IncompatibleSchema { .. } => "incompatible_schema",
            AppError::ProjectLocked(_) => "project_locked",
            AppError::CorruptProject(_, _) => "corrupt_project",
            AppError::InvalidSnapshotParent(_) => "invalid_snapshot_parent",
            AppError::InvalidSnapshotName(_) => "invalid_snapshot_name",
            AppError::SnapshotDestinationOccupied(_) => "snapshot_destination_occupied",
            AppError::SnapshotIo { .. } => "snapshot_io_error",
            AppError::SnapshotSerialization(_) => "snapshot_serialization_error",
            AppError::InvalidSnapshotImport { .. } => "invalid_snapshot_import",
            AppError::SnapshotHasNoChanges => "snapshot_has_no_changes",
            AppError::InvalidLoreImport { .. } => "invalid_lore_import",
            AppError::LoreImportBatchNotFound(_) => "lore_import_not_found",
            AppError::InvalidObjectUri(_) => "invalid_object_uri",
            AppError::ObjectNotFound { .. } => "object_not_found",
            AppError::ReviewSessionNotFound(_) => "review_not_found",
            AppError::ReviewSessionConflict(_) => "review_conflict",
            AppError::UnknownReviewOperation(_) => "unknown_review_operation",
            AppError::UnknownReviewDecision(_) => "unknown_review_decision",
            AppError::InvalidReviewDecisionAlternative { .. } => {
                "invalid_review_decision_alternative"
            }
            AppError::ReviewIssueNotFound { .. } => "review_issue_not_found",
            AppError::CannotWaiveHardIssue { .. } => "cannot_waive_hard_issue",
            AppError::Ai(AiError::InvalidBaseUrl(_)) => "invalid_provider_base_url",
            AppError::Ai(AiError::EmptyProviderApiKey | AiError::MissingProviderApiKey) => {
                "provider_key_missing"
            }
            AppError::Ai(AiError::CredentialStoreClearFailed) => "provider_store_error",
            AppError::Ai(AiError::RequestTimedOut(_)) => "provider_timeout",
            AppError::Ai(AiError::RequestCancelled) => "provider_cancelled",
            AppError::Ai(AiError::Transport(_)) => "provider_transport_error",
            AppError::Ai(AiError::InvalidHttpStatus { .. }) => "provider_http_error",
            AppError::Ai(AiError::InvalidResponse(_) | AiError::StreamInterrupted) => {
                "provider_response_error"
            }
            AppError::AiBaseRevisionMismatch { .. } => "ai_context_stale",
            AppError::AiRunNotFound(_) => "ai_run_not_found",
            AppError::DeepReviewRunNotFound(_) => "deep_review_run_not_found",
            AppError::InvalidDeepReview(_) => "invalid_deep_review",
            AppError::InvalidSimulationScenario(_) => "invalid_simulation_scenario",
            AppError::SimulationScenarioNotFound(_) => "simulation_scenario_not_found",
            AppError::InvalidNarrativeQuery(_) => "invalid_narrative_query",
            AppError::InvalidInternalDocument(_) => "invalid_internal_document",
            AppError::InvalidSimulationPromotion(_) => "invalid_simulation_promotion",
            AppError::SimulationScenarioStale { .. } => "simulation_scenario_stale",
            AppError::AiCritiqueIssueNotFound { .. } => "ai_critique_issue_not_found",
            AppError::InvalidAiRunTransition { .. } => "invalid_ai_run_transition",
            AppError::Domain(_) => "invalid_world",
            AppError::Storage(StoreError::StaleVersion { .. })
            | AppError::Storage(StoreError::StaleRevision { .. })
            | AppError::Storage(StoreError::InvalidChangeSet(_))
            | AppError::Storage(StoreError::InvalidAggregate(_))
            | AppError::Storage(StoreError::WrongWorld { .. }) => "validation_error",
            AppError::Storage(StoreError::Path(_, _)) => "file_error",
            AppError::Storage(StoreError::Database(_, message))
                if message.to_ascii_lowercase().contains("constraint") =>
            {
                "constraint_error"
            }
            AppError::Storage(_) => "storage_error",
            AppError::ClockBeforeUnixEpoch | AppError::ClockOutOfRange => "clock_error",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

fn lock_app<'a>(
    state: &'a State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<MutexGuard<'a, NirmataApp>, CommandError> {
    if !AI_ACTIVE.load(Ordering::Acquire) {
        return state.inner().lock().map_err(|_| internal_error());
    }
    match state.inner().try_lock() {
        Ok(app) => Ok(app),
        Err(TryLockError::WouldBlock) => Err(CommandError {
            code: "app_busy",
            message:
                "Nirmata is processing an AI request; cancel it or wait before changing the world."
                    .to_owned(),
        }),
        Err(TryLockError::Poisoned(_)) => Err(internal_error()),
    }
}

#[tauri::command]
fn create_world(
    input: CreateWorldRequest,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    let path = parse_project_path(&input.path)?;
    lock_app(&state)?
        .create_world(CreateWorldInput {
            path,
            name: input.name,
            premise_md: input.premise_md,
            epoch_label: input.epoch_label,
        })
        .map_err(Into::into)
}

#[tauri::command]
fn open_world(
    path: PathBuf,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .open_world(parse_project_path(&path)?)
        .map_err(Into::into)
}

#[tauri::command]
fn get_current_world(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Option<WorldSession>, CommandError> {
    lock_app(&state)?.get_current_world().map_err(Into::into)
}

#[tauri::command]
fn search_world(
    input: SearchWorldCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<SearchWorldResponse, CommandError> {
    let mut request = SearchWorldRequest::new(Default::default());
    request.empty = EmptySearchClassification::NoEvidence;
    request.query.kinds = search_kinds(input.kind.as_deref())?;
    request.query.text = normalize_optional_text(input.query_text);
    request.query.limit = input.limit.unwrap_or(200).clamp(1, 500);
    lock_app(&state)?.search_world(&request).map_err(Into::into)
}

#[tauri::command]
fn open_uri(
    uri: String,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<OpenUriResponse, CommandError> {
    lock_app(&state)?
        .open_uri(parse_object_uri(&uri)?)
        .map_err(Into::into)
}

#[tauri::command]
fn get_related_context(
    input: RelatedContextCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<RelatedContextResponse, CommandError> {
    let uri = parse_object_uri(&input.uri)?;
    let resolved = lock_app(&state)?
        .open_uri(uri)
        .map_err(CommandError::from)?;
    let mut bundle = ContextBundleRequest::new(context_intent_for_uri(&resolved));
    bundle.anchors = vec![resolved.result.object_ref];
    bundle.include_perspectives = true;
    bundle.relation_limit = 8;
    bundle.budget = ContextBudget {
        max_objects: 24,
        max_chars: 4_000,
    };
    let request = RelatedContextRequest::new(bundle);
    lock_app(&state)?
        .get_related_context(&request)
        .map_err(Into::into)
}

#[tauri::command]
fn read_logical_vfs(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<LogicalVfsDirectory, CommandError> {
    lock_app(&state)?.read_logical_vfs().map_err(Into::into)
}

#[tauri::command]
fn export_vfs_snapshot(
    input: ExportSnapshotCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ExportSnapshotResult, CommandError> {
    let parent_directory = parse_snapshot_parent(&input.parent_directory)?;
    let snapshot_name = parse_snapshot_name(&input.snapshot_name)?;
    lock_app(&state)?
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory,
            snapshot_name,
        })
        .map_err(Into::into)
}

#[tauri::command]
fn import_vfs_snapshot(
    input: ImportSnapshotCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ImportSnapshotResult, CommandError> {
    let snapshot_directory = parse_snapshot_directory(&input.snapshot_directory)?;
    lock_app(&state)?
        .import_vfs_snapshot(ImportSnapshotInput { snapshot_directory })
        .map_err(Into::into)
}

#[tauri::command]
fn create_lore_import(
    input: CreateLoreImportCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ImportBatchSnapshot, CommandError> {
    let source_file = input.source_file;
    let source_root = source_file.parent().ok_or(CommandError {
        code: "invalid_lore_import",
        message: "The selected source has no parent directory.".to_owned(),
    })?;
    lock_app(&state)?
        .create_import_batch(nirmata_app::CreateImportBatchInput {
            source_root: source_root.to_path_buf(),
            files: vec![source_file.clone()],
        })
        .map_err(Into::into)
}

#[tauri::command]
fn read_lore_import(
    input: LoreImportCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ImportBatchSnapshot, CommandError> {
    lock_app(&state)?
        .read_import_batch(input.batch_id.trim())
        .map_err(Into::into)
}

#[tauri::command]
fn read_lore_candidates(
    input: LoreImportCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Vec<ImportCandidateSnapshot>, CommandError> {
    lock_app(&state)?
        .read_import_candidates(input.batch_id.trim())
        .map_err(Into::into)
}

#[tauri::command]
fn open_lore_chunk(
    input: LoreImportChunkCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ImportChunkLocation, CommandError> {
    lock_app(&state)?
        .open_import_chunk(input.batch_id.trim(), input.chunk_id.trim())
        .map_err(Into::into)
}

#[tauri::command]
fn replace_lore_source(
    input: ReplaceLoreSourceCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ImportBatchSnapshot, CommandError> {
    lock_app(&state)?
        .replace_import_source(
            input.batch_id.trim(),
            input.source_id.trim(),
            input.source_file,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn decide_lore_candidate(
    input: DecideLoreCandidateCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Vec<ImportCandidateSnapshot>, CommandError> {
    lock_app(&state)?
        .decide_import_candidate(input.batch_id.trim(), input.decision)
        .map_err(Into::into)
}

#[tauri::command]
fn edit_lore_candidate(
    input: EditLoreCandidateCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Vec<ImportCandidateSnapshot>, CommandError> {
    lock_app(&state)?
        .edit_import_candidate(
            input.batch_id.trim(),
            input.candidate_id.trim(),
            input.replacement,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn delete_lore_import(
    input: LoreImportCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<(), CommandError> {
    lock_app(&state)?
        .delete_import_batch(input.batch_id.trim())
        .map_err(Into::into)
}

#[tauri::command]
async fn extract_lore_import(
    input: AiLoreImportCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<ImportExtractionResult, CommandError> {
    let provider = provider_config()?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.execute_import_extraction(
            input.batch_id.trim(),
            &provider,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "lore-import-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
async fn prepare_lore_import_review(
    input: AiLoreImportCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<ImportReviewPreparation, CommandError> {
    let provider = provider_config()?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.prepare_import_review(
            input.batch_id.trim(),
            &provider,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-proposal-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
fn get_provider_credential_status(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ProviderCredentialStatus, CommandError> {
    Ok(lock_app(&state)?.get_provider_credential_status())
}

#[tauri::command]
fn set_provider_api_key(
    api_key: String,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ProviderCredentialStatus, CommandError> {
    lock_app(&state)?
        .set_provider_api_key(api_key)
        .map_err(Into::into)
}

#[tauri::command]
fn clear_provider_api_key(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ProviderCredentialStatus, CommandError> {
    lock_app(&state)?
        .clear_provider_api_key()
        .map_err(Into::into)
}

fn internal_error() -> CommandError {
    CommandError {
        code: "internal_error",
        message: "Nirmata could not access the current session; restart the app".to_owned(),
    }
}

#[tauri::command]
async fn execute_ai_query(
    input: AiRequestCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<AiQueryResponse, CommandError> {
    let provider = provider_config()?;
    let context = ai_context_request(input.anchor_uri.as_deref(), ContextIntent::EntityQuery)?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.execute_ai_query(
            &provider,
            input.request,
            &context,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-query-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
async fn execute_ai_proposal(
    input: AiRequestCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<AiRunSnapshot, CommandError> {
    let provider = provider_config()?;
    let context = ai_context_request(input.anchor_uri.as_deref(), ContextIntent::ImpactAnalysis)?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.execute_ai_proposal_run(
            &provider,
            input.request,
            &context,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-proposal-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
fn prepare_deep_review(
    input: DeepReviewPlanCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<DeepReviewPlan, CommandError> {
    let mode = parse_deep_review_mode(&input.mode)?;
    let context = ai_context_request(input.anchor_uri.as_deref(), ContextIntent::ImpactAnalysis)?;
    lock_app(&state)?
        .prepare_deep_review(mode, input.request, None, &context)
        .map_err(Into::into)
}

#[tauri::command]
async fn execute_deep_review(
    input: DeepReviewExecuteCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<DeepReviewRun, CommandError> {
    let provider = provider_config()?;
    let mode = parse_deep_review_mode(&input.mode)?;
    let context = ai_context_request(input.anchor_uri.as_deref(), ContextIntent::ImpactAnalysis)?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        let plan = app
            .prepare_deep_review(mode, input.request, Some(input.roles.clone()), &context)
            .map_err(CommandError::from)?
            .confirm(input.roles)
            .map_err(CommandError::from)?;
        tauri::async_runtime::block_on(app.execute_deep_review(
            &provider,
            plan,
            &context,
            token,
            move |progress| {
                let _ = app_handle.emit(
                    "deep-review-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
fn read_deep_review_run(
    run_id: String,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<DeepReviewRun, CommandError> {
    lock_app(&state)?
        .read_deep_review_run(parse_deep_review_run_id(&run_id)?)
        .map_err(Into::into)
}

#[tauri::command]
async fn execute_ai_proposal_from_brief(
    input: AiIntentBriefCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<AiRunSnapshot, CommandError> {
    let provider = provider_config()?;
    let context = ai_context_request(input.anchor_uri.as_deref(), ContextIntent::ImpactAnalysis)?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        let entities = input
            .entity_uris
            .iter()
            .map(|uri| {
                app.open_uri(parse_object_uri(uri)?)
                    .map(|response| response.result)
                    .map_err(CommandError::from)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let brief = IntentBrief {
            user_request: input.user_request,
            objective: input.objective,
            scope: input.scope,
            entities,
            restrictions: input.restrictions,
            reason: input.reason,
        };
        tauri::async_runtime::block_on(app.execute_ai_proposal_run_from_intent_brief(
            &provider,
            &brief,
            &context,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-proposal-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
async fn revalidate_ai_run(
    input: AiRunCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<AiRunSnapshot, CommandError> {
    let provider = provider_config()?;
    let context = ai_context_request(input.anchor_uri.as_deref(), ContextIntent::ImpactAnalysis)?;
    let run_id = parse_ai_run_id(&input.run_id)?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.revalidate_ai_run(
            run_id,
            &provider,
            &context,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-proposal-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
fn read_ai_run(
    run_id: String,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<AiRunSnapshot, CommandError> {
    lock_app(&state)?
        .read_ai_run(parse_ai_run_id(&run_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn discard_ai_run(
    run_id: String,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<AiRunSnapshot, CommandError> {
    lock_app(&state)?
        .discard_ai_run(parse_ai_run_id(&run_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn acknowledge_ai_critique(
    input: AiCritiqueDecisionCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<AiRunSnapshot, CommandError> {
    lock_app(&state)?
        .acknowledge_ai_critique(
            parse_ai_run_id(&input.run_id)?,
            input.issue_id.trim(),
            input.judgment,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn cancel_ai_request(
    request_id: String,
    cancellations: State<'_, AiCancellations>,
) -> Result<(), CommandError> {
    let cancellations = cancellations.0.lock().map_err(|_| internal_error())?;
    let token = cancellations.get(request_id.trim()).ok_or(CommandError {
        code: "ai_request_not_found",
        message: "The AI request already finished or does not exist.".to_owned(),
    })?;
    token.cancel();
    Ok(())
}

#[tauri::command]
fn preview_manual_draft(
    input: ManualDraftRequest,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualDraftResponse, CommandError> {
    lock_app(&state)?
        .preview_manual_draft(input)
        .map_err(Into::into)
}

#[tauri::command]
fn apply_manual_review_action(
    input: ManualReviewActionCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .apply_stored_manual_review_action(parse_review_key(&input.review_key)?, input.action)
        .map_err(Into::into)
}

#[tauri::command]
fn confirm_manual_review(
    input: ReviewKeyCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .confirm_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn read_manual_review(
    input: ReviewKeyCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .read_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn discard_manual_review(
    input: ReviewKeyCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<(), CommandError> {
    lock_app(&state)?
        .discard_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn begin_manual_review_edit(
    input: ReviewOperationCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualDraftRequest, CommandError> {
    lock_app(&state)?
        .begin_stored_manual_review_edit(
            parse_review_key(&input.review_key)?,
            parse_operation_id(&input.operation_id)?,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn apply_manual_review_edit(
    input: ManualReviewEditCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualDraftResponse, CommandError> {
    lock_app(&state)?
        .apply_stored_manual_review_edit(
            parse_review_key(&input.review_key)?,
            parse_operation_id(&input.operation_id)?,
            input.request,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn revalidate_manual_review(
    input: ReviewKeyCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .revalidate_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn create_simulation_scenario(
    input: CreateSimulationScenarioCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<SimulationScenario, CommandError> {
    lock_app(&state)?
        .create_simulation_scenario(input.scenario)
        .map_err(Into::into)
}

#[tauri::command]
fn update_simulation_scenario(
    input: UpdateSimulationScenarioCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<SimulationScenario, CommandError> {
    lock_app(&state)?
        .update_simulation_scenario(
            parse_simulation_scenario_id(&input.scenario_id)?,
            input.scenario,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn delete_simulation_scenario(
    input: SimulationScenarioCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<SimulationScenario, CommandError> {
    lock_app(&state)?
        .delete_simulation_scenario(parse_simulation_scenario_id(&input.scenario_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn list_simulation_scenarios(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Vec<SimulationScenario>, CommandError> {
    lock_app(&state)?
        .list_simulation_scenarios()
        .map_err(Into::into)
}

#[tauri::command]
fn run_simulation_scenario(
    input: SimulationScenarioCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<SimulationRun, CommandError> {
    lock_app(&state)?
        .run_simulation_scenario(parse_simulation_scenario_id(&input.scenario_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn prepare_simulation_review(
    input: PrepareSimulationReviewCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .prepare_simulation_review(
            parse_simulation_scenario_id(&input.scenario_id)?,
            input.promotion,
        )
        .map_err(Into::into)
}

#[tauri::command]
fn derive_narrative_timeline(
    input: DeriveNarrativeTimelineCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<NarrativeTimeline, CommandError> {
    let scope = parse_narrative_scope(input.scope)?;
    lock_app(&state)?
        .derive_narrative_timeline(scope)
        .map_err(Into::into)
}

#[tauri::command]
fn derive_causal_threads(
    input: DeriveCausalThreadsCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<NarrativeCausalThreads, CommandError> {
    let scope = parse_narrative_scope(input.scope)?;
    let start_event_ids = input
        .start_event_ids
        .map(|ids| ids.iter().map(|id| parse_event_id(id)).collect())
        .transpose()?;
    lock_app(&state)?
        .derive_causal_threads(scope, start_event_ids, input.max_depth, input.limit)
        .map_err(Into::into)
}

#[tauri::command]
fn derive_loose_ends(
    input: DeriveLooseEndsCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<NarrativeLooseEnds, CommandError> {
    let scope = parse_narrative_scope(input.scope)?;
    lock_app(&state)?
        .derive_loose_ends(scope)
        .map_err(Into::into)
}

#[tauri::command]
fn explore_narrative_continuity(
    input: ExploreNarrativeContinuityCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<NarrativeContinuityExploration, CommandError> {
    let scope = parse_narrative_scope(input.scope)?;
    let selection = parse_narrative_selection(input.selection)?;
    lock_app(&state)?
        .explore_narrative_continuity(scope, selection)
        .map_err(Into::into)
}

#[tauri::command]
async fn generate_internal_document(
    input: GenerateInternalDocumentCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<AiRunSnapshot, CommandError> {
    let title = parse_required_text("title", input.title, 200, "invalid_internal_document")?;
    let request = parse_required_text(
        "request",
        input.request,
        20_000,
        "invalid_internal_document",
    )?;
    let document_kind = parse_internal_document_kind(&input.document_kind)?;
    let perspective_entity_id = parse_entity_id(&input.perspective_entity_id)?;
    let anchors = input
        .anchor_uris
        .iter()
        .map(|uri| {
            ObjectRef::from_str(parse_object_uri(uri)?).map_err(|_| CommandError {
                code: "invalid_object_uri",
                message: format!("invalid nirmata URI {uri}"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let provider = provider_config()?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.generate_internal_document(
            &provider,
            InternalDocumentRequest {
                instructions: format!(
                    "Use exactly this document title: {title}\n\nDocument request: {request}"
                ),
                document_kind,
                perspective_entity_id,
                tick: input.tick,
                anchors,
            },
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-proposal-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
async fn propose_narrative_continuity(
    input: ProposeNarrativeContinuityCommand,
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
    cancellations: State<'_, AiCancellations>,
) -> Result<NarrativeContinuityProposal, CommandError> {
    let scope = parse_narrative_scope(input.scope)?;
    let selection = parse_narrative_selection(input.selection)?;
    let alternative_id = parse_required_text(
        "alternativeId",
        input.alternative_id,
        64,
        "invalid_narrative_query",
    )?;
    let provider = provider_config()?;
    let token = register_cancellation(&cancellations, &input.request_id)?;
    let request_id = input.request_id.clone();
    let cleanup_id = input.request_id.clone();
    let app_state = Arc::clone(state.inner());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut app = app_state.lock().map_err(|_| internal_error())?;
        tauri::async_runtime::block_on(app.propose_narrative_continuity(
            &provider,
            scope,
            selection,
            &alternative_id,
            AiRequestOptions::default().with_cancellation(token),
            move |progress| {
                let _ = app_handle.emit(
                    "ai-proposal-progress",
                    AiProgressEvent {
                        request_id: request_id.clone(),
                        progress,
                    },
                );
            },
        ))
        .map_err(CommandError::from)
    })
    .await
    .map_err(|_| internal_error())?;
    remove_cancellation(&cancellations, &cleanup_id);
    result
}

#[tauri::command]
fn list_timeline_events(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<TimelineOverview, CommandError> {
    lock_app(&state)?.list_timeline_events().map_err(Into::into)
}

#[tauri::command]
fn list_revision_history(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<RevisionHistorySnapshot, CommandError> {
    lock_app(&state)?
        .list_revision_history()
        .map_err(Into::into)
}

#[tauri::command]
fn list_variants(state: State<'_, Arc<Mutex<NirmataApp>>>) -> Result<Vec<Variant>, CommandError> {
    lock_app(&state)?.list_variants().map_err(Into::into)
}

#[tauri::command]
fn create_variant(
    input: CreateVariantCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Variant, CommandError> {
    lock_app(&state)?
        .create_variant(&input.name, parse_revision_id(&input.from_revision_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn rename_variant(
    input: RenameVariantCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<Variant, CommandError> {
    lock_app(&state)?
        .rename_variant(parse_variant_id(&input.variant_id)?, &input.name)
        .map_err(Into::into)
}

#[tauri::command]
fn switch_variant(
    input: VariantCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .switch_variant(parse_variant_id(&input.variant_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn archive_variant(
    input: ArchiveVariantCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<(), CommandError> {
    lock_app(&state)?
        .archive_variant(parse_variant_id(&input.variant_id)?, input.allow_referenced)
        .map_err(Into::into)
}

#[tauri::command]
fn set_read_scope(
    input: ReadScopeCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .set_read_scope(input.scope)
        .map_err(Into::into)
}

#[tauri::command]
fn view_active_head(
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?.view_active_head().map_err(Into::into)
}

#[tauri::command]
fn compare_variant_scopes(
    input: CompareScopesCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<VariantComparison, CommandError> {
    lock_app(&state)?
        .compare_scopes(input.left, input.right)
        .map_err(Into::into)
}

#[tauri::command]
fn prepare_variant_merge(
    input: ReadScopeCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<MergeReviewResult, CommandError> {
    lock_app(&state)?
        .prepare_variant_merge(input.scope)
        .map_err(Into::into)
}

#[tauri::command]
fn undo_revision(
    input: RevisionCommand,
    state: State<'_, Arc<Mutex<NirmataApp>>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .undo_revision(parse_revision_id(&input.revision_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn close_world(state: State<'_, Arc<Mutex<NirmataApp>>>) -> Result<(), CommandError> {
    lock_app(&state)?.close_world().map_err(Into::into)
}

fn search_kinds(value: Option<&str>) -> Result<Vec<StructuredSearchKind>, CommandError> {
    let Some(kind) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(all_search_kinds());
    };

    let kind = match kind {
        "all" => return Ok(all_search_kinds()),
        "entity" => StructuredSearchKind::Entity,
        "relation" => StructuredSearchKind::Relation,
        "event" => StructuredSearchKind::Event,
        "claim" => StructuredSearchKind::Claim,
        "rule" => StructuredSearchKind::Rule,
        "goal" => StructuredSearchKind::Goal,
        "document" => StructuredSearchKind::Document,
        _ => {
            return Err(CommandError {
                code: "invalid_search_kind",
                message: format!("unsupported search kind: {kind}"),
            });
        }
    };
    Ok(vec![kind])
}

fn all_search_kinds() -> Vec<StructuredSearchKind> {
    vec![
        StructuredSearchKind::Entity,
        StructuredSearchKind::Relation,
        StructuredSearchKind::Event,
        StructuredSearchKind::Claim,
        StructuredSearchKind::Rule,
        StructuredSearchKind::Goal,
        StructuredSearchKind::Document,
    ]
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    let trimmed = value?.trim().to_owned();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn ai_context_request(
    anchor_uri: Option<&str>,
    intent: ContextIntent,
) -> Result<ContextBundleRequest, CommandError> {
    let mut request = ContextBundleRequest::new(intent);
    if let Some(anchor_uri) = anchor_uri.map(str::trim).filter(|value| !value.is_empty()) {
        request.anchors =
            vec![
                ObjectRef::from_str(parse_object_uri(anchor_uri)?).map_err(|_| CommandError {
                    code: "invalid_object_uri",
                    message: format!("invalid nirmata URI {anchor_uri}"),
                })?,
            ];
    }
    request.include_perspectives = true;
    request.relation_limit = 8;
    request.budget = ContextBudget {
        max_objects: 32,
        max_chars: 8_000,
    };
    Ok(request)
}

fn register_cancellation(
    cancellations: &State<'_, AiCancellations>,
    request_id: &str,
) -> Result<CancellationToken, CommandError> {
    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err(CommandError {
            code: "invalid_ai_request_id",
            message: "AI request id cannot be empty.".to_owned(),
        });
    }
    let token = CancellationToken::new();
    cancellations
        .0
        .lock()
        .map_err(|_| internal_error())?
        .insert(request_id.to_owned(), token.clone());
    AI_ACTIVE.store(true, Ordering::Release);
    Ok(token)
}

fn remove_cancellation(cancellations: &State<'_, AiCancellations>, request_id: &str) {
    if let Ok(mut values) = cancellations.0.lock() {
        values.remove(request_id.trim());
        if values.is_empty() {
            AI_ACTIVE.store(false, Ordering::Release);
        }
    }
}

fn provider_config() -> Result<AiProviderConfig, CommandError> {
    let base_url = development_config_value("BASE_URL").ok_or(CommandError {
        code: "provider_config_missing",
        message: "BASE_URL is not configured for Microsoft Foundry.".to_owned(),
    })?;
    if !base_url.to_ascii_lowercase().starts_with("https://") {
        return Err(CommandError {
            code: "invalid_provider_base_url",
            message: "Microsoft Foundry BASE_URL must use HTTPS.".to_owned(),
        });
    }
    let model = development_config_value("AZURE_FOUNDRY_MODEL")
        .or_else(|| development_config_value("GPT-5.6-SOL"))
        .ok_or(CommandError {
            code: "provider_config_missing",
            message: "AZURE_FOUNDRY_MODEL is not configured for Microsoft Foundry.".to_owned(),
        })?;
    Ok(AiProviderConfig::new(base_url, model))
}

fn development_config_value(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .and_then(non_empty_config_value)
        .or_else(|| {
            dotenv_paths().into_iter().find_map(|path| {
                fs::read_to_string(path)
                    .ok()
                    .and_then(|contents| dotenv_value(&contents, name))
            })
        })
}

fn dotenv_paths() -> [PathBuf; 1] {
    [Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.env")]
}

fn dotenv_value(contents: &str, name: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line.split_once('=')?;
        if key.trim() != name {
            return None;
        }
        non_empty_config_value(value.trim().trim_matches(['\'', '"']).to_owned())
    })
}

fn non_empty_config_value(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn parse_operation_id(value: &str) -> Result<nirmata_app::ChangeOperationId, CommandError> {
    nirmata_app::ChangeOperationId::from_str(value).map_err(|_| CommandError {
        code: "invalid_review_operation",
        message: format!("invalid manual review operation id: {value}"),
    })
}

fn parse_ai_run_id(value: &str) -> Result<AiRunId, CommandError> {
    AiRunId::from_str(value.trim()).map_err(|_| CommandError {
        code: "invalid_ai_run_id",
        message: format!("invalid AI run id: {value}"),
    })
}

fn parse_deep_review_run_id(value: &str) -> Result<DeepReviewRunId, CommandError> {
    DeepReviewRunId::from_str(value.trim()).map_err(|_| CommandError {
        code: "invalid_deep_review_run_id",
        message: format!("invalid deep review run id: {value}"),
    })
}

fn parse_simulation_scenario_id(value: &str) -> Result<SimulationScenarioId, CommandError> {
    SimulationScenarioId::from_str(value.trim()).map_err(|_| CommandError {
        code: "invalid_simulation_scenario_id",
        message: format!("invalid simulation scenario id: {value}"),
    })
}

fn parse_event_id(value: &str) -> Result<EventId, CommandError> {
    EventId::from_str(value.trim()).map_err(|_| CommandError {
        code: "invalid_event_id",
        message: format!("invalid event id: {value}"),
    })
}

fn parse_entity_id(value: &str) -> Result<EntityId, CommandError> {
    EntityId::from_str(value.trim()).map_err(|_| CommandError {
        code: "invalid_entity_id",
        message: format!("invalid entity id: {value}"),
    })
}

fn parse_narrative_scope(
    scope: Option<NarrativeReadScopeCommand>,
) -> Result<Option<ReadScope>, CommandError> {
    scope
        .map(|scope| {
            let variant_id =
                VariantId::from_str(scope.variant_id.trim()).map_err(|_| CommandError {
                    code: "invalid_narrative_scope",
                    message: "narrative scope variantId must be a UUID".to_owned(),
                })?;
            match scope.revision_id {
                Some(revision_id) => parse_revision_id(&revision_id)
                    .map(|revision_id| ReadScope::historical(variant_id, revision_id))
                    .map_err(|_| CommandError {
                        code: "invalid_narrative_scope",
                        message: "narrative scope revisionId must be a UUID or null".to_owned(),
                    }),
                None => Ok(ReadScope::head(variant_id)),
            }
        })
        .transpose()
}

fn parse_narrative_selection(
    selection: NarrativeContinuitySelectionCommand,
) -> Result<NarrativeContinuitySelection, CommandError> {
    match selection {
        NarrativeContinuitySelectionCommand::LooseEnd { code, object_uri } => {
            let code = parse_required_text("code", code, 100, "invalid_narrative_selection")?;
            let object_ref =
                ObjectRef::from_str(parse_object_uri(&object_uri)?).map_err(|_| CommandError {
                    code: "invalid_narrative_selection",
                    message: format!("invalid narrative selection URI: {object_uri}"),
                })?;
            Ok(NarrativeContinuitySelection::LooseEnd { code, object_ref })
        }
        NarrativeContinuitySelectionCommand::CausalThread { start_event_id } => {
            Ok(NarrativeContinuitySelection::CausalThread {
                start_event_id: parse_event_id(&start_event_id)?,
            })
        }
    }
}

fn parse_internal_document_kind(value: &str) -> Result<InternalDocumentKind, CommandError> {
    match value.trim() {
        "chronicle" => Ok(InternalDocumentKind::Chronicle),
        "letter" => Ok(InternalDocumentKind::Letter),
        "report" => Ok(InternalDocumentKind::Report),
        "myth" => Ok(InternalDocumentKind::Myth),
        "short_story" => Ok(InternalDocumentKind::ShortStory),
        _ => Err(CommandError {
            code: "invalid_internal_document_kind",
            message: format!("unsupported internal document kind: {value}"),
        }),
    }
}

fn parse_required_text(
    field: &'static str,
    value: String,
    max_chars: usize,
    code: &'static str,
) -> Result<String, CommandError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(CommandError {
            code,
            message: format!("{field} must contain between 1 and {max_chars} characters"),
        });
    }
    Ok(value)
}

fn parse_deep_review_mode(value: &str) -> Result<DeepReviewMode, CommandError> {
    match value.trim() {
        "deep_impact" => Ok(DeepReviewMode::DeepImpact),
        "audit" => Ok(DeepReviewMode::Audit),
        _ => Err(CommandError {
            code: "invalid_deep_review_mode",
            message: format!("unsupported deep review mode: {value}"),
        }),
    }
}

fn parse_project_path(path: &Path) -> Result<PathBuf, CommandError> {
    if path.as_os_str().is_empty() {
        return Err(CommandError {
            code: "invalid_project_path",
            message: "project paths must point to a .nirmata file".to_owned(),
        });
    }
    let has_valid_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("nirmata"));
    if !has_valid_extension {
        return Err(CommandError {
            code: "invalid_project_path",
            message: format!("{} must point to a .nirmata file", path.display()),
        });
    }
    Ok(path.to_path_buf())
}

fn parse_snapshot_parent(path: &Path) -> Result<PathBuf, CommandError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(CommandError {
            code: "invalid_snapshot_parent",
            message: "snapshot parent must be an absolute directory selected by the user"
                .to_owned(),
        });
    }
    Ok(path.to_path_buf())
}

fn parse_snapshot_directory(path: &Path) -> Result<PathBuf, CommandError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(CommandError {
            code: "invalid_snapshot_directory",
            message: "snapshot import path must be an absolute directory selected by the user"
                .to_owned(),
        });
    }
    Ok(path.to_path_buf())
}

fn parse_snapshot_name(value: &str) -> Result<String, CommandError> {
    if value.is_empty()
        || value.len() > 80
        || value.starts_with('.')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(CommandError {
            code: "invalid_snapshot_name",
            message: "snapshot name must contain only letters, numbers, '-' or '_'".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn parse_object_uri<'a>(value: &'a str) -> Result<&'a str, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || ObjectRef::from_str(trimmed).is_err() {
        return Err(CommandError {
            code: "invalid_object_uri",
            message: format!("invalid nirmata URI {value}"),
        });
    }
    Ok(trimmed)
}

fn parse_review_key(value: &str) -> Result<&str, CommandError> {
    parse_object_uri(value)
}

fn parse_revision_id(value: &str) -> Result<RevisionId, CommandError> {
    RevisionId::from_str(value.trim()).map_err(|_| CommandError {
        code: "invalid_revision_id",
        message: format!("invalid revision id: {value}"),
    })
}

fn parse_variant_id(value: &str) -> Result<VariantId, CommandError> {
    VariantId::from_str(value).map_err(|_| CommandError {
        code: "invalid_variant_id",
        message: "variant ID must be a UUID".to_owned(),
    })
}

fn context_intent_for_uri(response: &OpenUriResponse) -> ContextIntent {
    match response.result.object_type {
        "event" => ContextIntent::ImpactAnalysis,
        "claim" => ContextIntent::ContradictionCheck,
        _ => ContextIntent::EntityQuery,
    }
}

fn main() {
    let mut app = NirmataApp::default();
    if !app.get_provider_credential_status().configured
        && let Some(api_key) = development_config_value("PROVIDER_API_KEY")
    {
        let _ = app.set_session_provider_api_key(api_key);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(Mutex::new(app)))
        .manage(AiCancellations(Mutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![
            create_world,
            open_world,
            get_current_world,
            search_world,
            open_uri,
            get_related_context,
            read_logical_vfs,
            export_vfs_snapshot,
            import_vfs_snapshot,
            create_lore_import,
            read_lore_import,
            read_lore_candidates,
            open_lore_chunk,
            replace_lore_source,
            decide_lore_candidate,
            edit_lore_candidate,
            delete_lore_import,
            extract_lore_import,
            prepare_lore_import_review,
            get_provider_credential_status,
            set_provider_api_key,
            clear_provider_api_key,
            execute_ai_query,
            execute_ai_proposal,
            prepare_deep_review,
            execute_deep_review,
            read_deep_review_run,
            execute_ai_proposal_from_brief,
            revalidate_ai_run,
            read_ai_run,
            discard_ai_run,
            acknowledge_ai_critique,
            cancel_ai_request,
            preview_manual_draft,
            apply_manual_review_action,
            read_manual_review,
            discard_manual_review,
            begin_manual_review_edit,
            apply_manual_review_edit,
            revalidate_manual_review,
            confirm_manual_review,
            create_simulation_scenario,
            update_simulation_scenario,
            delete_simulation_scenario,
            list_simulation_scenarios,
            run_simulation_scenario,
            prepare_simulation_review,
            derive_narrative_timeline,
            derive_causal_threads,
            derive_loose_ends,
            generate_internal_document,
            explore_narrative_continuity,
            propose_narrative_continuity,
            list_timeline_events,
            list_revision_history,
            list_variants,
            create_variant,
            rename_variant,
            switch_variant,
            archive_variant,
            set_read_scope,
            view_active_head,
            compare_variant_scopes,
            prepare_variant_merge,
            undo_revision,
            close_world
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Nirmata");
}

#[cfg(test)]
#[path = "../tests/unit/desktop.rs"]
mod tests;
