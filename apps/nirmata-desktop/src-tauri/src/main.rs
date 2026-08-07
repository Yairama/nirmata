#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nirmata_app::{
    AiError, AiProviderConfig, AiQueryResponse, AiRequestOptions, AiRunId, AiRunSnapshot, AppError,
    CancellationToken, ContextBudget, ContextBundleRequest, ContextIntent, CreateWorldInput,
    EmptySearchClassification, IntentBrief, LogicalVfsDirectory, ManualDraftRequest,
    ManualDraftResponse, ManualReviewActionRequest, ManualReviewSnapshot, NirmataApp, ObjectRef,
    OpenUriResponse, ProviderCredentialStatus, RelatedContextRequest, RelatedContextResponse,
    RevisionHistorySnapshot, RevisionId, SearchWorldRequest, SearchWorldResponse, StoreError,
    StructuredSearchKind, TimelineOverview, WorldSession,
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
            AppError::ManualReviewNotReady => "manual_review_not_ready",
            AppError::ManualReviewStale { .. } => "manual_review_stale",
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
            get_provider_credential_status,
            set_provider_api_key,
            clear_provider_api_key,
            execute_ai_query,
            execute_ai_proposal,
            execute_ai_proposal_from_brief,
            revalidate_ai_run,
            read_ai_run,
            discard_ai_run,
            acknowledge_ai_critique,
            cancel_ai_request,
            preview_manual_draft,
            apply_manual_review_action,
            read_manual_review,
            begin_manual_review_edit,
            apply_manual_review_edit,
            revalidate_manual_review,
            confirm_manual_review,
            list_timeline_events,
            list_revision_history,
            undo_revision,
            close_world
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Nirmata");
}

#[cfg(test)]
#[path = "../tests/unit/desktop.rs"]
mod tests;
