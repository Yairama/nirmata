#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nirmata_app::{
    AiError, AppError, ContextBudget, ContextBundleRequest, ContextIntent, CreateWorldInput,
    EmptySearchClassification, LogicalVfsDirectory, ManualDraftRequest, ManualDraftResponse,
    ManualReviewActionRequest, ManualReviewSnapshot, NirmataApp, ObjectRef, OpenUriResponse,
    ProviderCredentialStatus, RelatedContextRequest, RelatedContextResponse,
    RevisionHistorySnapshot, RevisionId, SearchWorldRequest, SearchWorldResponse, StoreError,
    StructuredSearchKind, TimelineOverview, WorldSession,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Mutex, MutexGuard},
};
use tauri::State;

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
            AppError::UnknownReviewOperation(_) => "unknown_review_operation",
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
    state: &'a State<'_, Mutex<NirmataApp>>,
) -> Result<MutexGuard<'a, NirmataApp>, CommandError> {
    state.inner().lock().map_err(|_| CommandError {
        code: "internal_error",
        message: "Nirmata could not access the current session; restart the app".to_owned(),
    })
}

#[tauri::command]
fn create_world(
    input: CreateWorldRequest,
    state: State<'_, Mutex<NirmataApp>>,
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
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .open_world(parse_project_path(&path)?)
        .map_err(Into::into)
}

#[tauri::command]
fn get_current_world(
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<Option<WorldSession>, CommandError> {
    lock_app(&state)?.get_current_world().map_err(Into::into)
}

#[tauri::command]
fn search_world(
    input: SearchWorldCommand,
    state: State<'_, Mutex<NirmataApp>>,
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
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<OpenUriResponse, CommandError> {
    lock_app(&state)?
        .open_uri(parse_object_uri(&uri)?)
        .map_err(Into::into)
}

#[tauri::command]
fn get_related_context(
    input: RelatedContextCommand,
    state: State<'_, Mutex<NirmataApp>>,
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
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<LogicalVfsDirectory, CommandError> {
    lock_app(&state)?.read_logical_vfs().map_err(Into::into)
}

#[tauri::command]
fn get_provider_credential_status(
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ProviderCredentialStatus, CommandError> {
    Ok(lock_app(&state)?.get_provider_credential_status())
}

#[tauri::command]
fn set_provider_api_key(
    api_key: String,
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ProviderCredentialStatus, CommandError> {
    lock_app(&state)?
        .set_provider_api_key(api_key)
        .map_err(Into::into)
}

#[tauri::command]
fn clear_provider_api_key(
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ProviderCredentialStatus, CommandError> {
    lock_app(&state)?
        .clear_provider_api_key()
        .map_err(Into::into)
}

#[tauri::command]
fn preview_manual_draft(
    input: ManualDraftRequest,
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ManualDraftResponse, CommandError> {
    lock_app(&state)?
        .preview_manual_draft(input)
        .map_err(Into::into)
}

#[tauri::command]
fn apply_manual_review_action(
    input: ManualReviewActionCommand,
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .apply_stored_manual_review_action(parse_review_key(&input.review_key)?, input.action)
        .map_err(Into::into)
}

#[tauri::command]
fn confirm_manual_review(
    input: ReviewKeyCommand,
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .confirm_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn read_manual_review(
    input: ReviewKeyCommand,
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .read_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn begin_manual_review_edit(
    input: ReviewOperationCommand,
    state: State<'_, Mutex<NirmataApp>>,
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
    state: State<'_, Mutex<NirmataApp>>,
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
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<ManualReviewSnapshot, CommandError> {
    lock_app(&state)?
        .revalidate_stored_manual_review(parse_review_key(&input.review_key)?)
        .map_err(Into::into)
}

#[tauri::command]
fn list_timeline_events(
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<TimelineOverview, CommandError> {
    lock_app(&state)?.list_timeline_events().map_err(Into::into)
}

#[tauri::command]
fn list_revision_history(
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<RevisionHistorySnapshot, CommandError> {
    lock_app(&state)?
        .list_revision_history()
        .map_err(Into::into)
}

#[tauri::command]
fn undo_revision(
    input: RevisionCommand,
    state: State<'_, Mutex<NirmataApp>>,
) -> Result<WorldSession, CommandError> {
    lock_app(&state)?
        .undo_revision(parse_revision_id(&input.revision_id)?)
        .map_err(Into::into)
}

#[tauri::command]
fn close_world(state: State<'_, Mutex<NirmataApp>>) -> Result<(), CommandError> {
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

fn parse_operation_id(value: &str) -> Result<nirmata_app::ChangeOperationId, CommandError> {
    nirmata_app::ChangeOperationId::from_str(value).map_err(|_| CommandError {
        code: "invalid_review_operation",
        message: format!("invalid manual review operation id: {value}"),
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(NirmataApp::default()))
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
mod tests {
    use super::{CommandError, parse_object_uri, parse_project_path, parse_revision_id};
    use nirmata_app::{AiError, AppError};
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn rejecting_invalid_project_paths_happens_before_opening_files() {
        let error = parse_project_path(Path::new("C:\\data\\world.txt"))
            .expect_err("only .nirmata files are accepted");
        assert_eq!(error.code, "invalid_project_path");
    }

    #[test]
    fn rejecting_invalid_object_uris_happens_before_dispatch() {
        let error =
            parse_object_uri("javascript:alert(1)").expect_err("only nirmata:// URIs are accepted");
        assert_eq!(error.code, "invalid_object_uri");
    }

    #[test]
    fn rejecting_invalid_revision_ids_happens_before_undo() {
        let error = parse_revision_id("not-a-revision-id").expect_err("revision ids must be UUIDs");
        assert_eq!(error.code, "invalid_revision_id");
    }

    #[test]
    fn provider_errors_map_to_stable_command_codes() {
        let timeout = CommandError::from(AppError::Ai(AiError::RequestTimedOut(
            Duration::from_secs(5),
        )));
        assert_eq!(timeout.code, "provider_timeout");

        let cancelled = CommandError::from(AppError::Ai(AiError::RequestCancelled));
        assert_eq!(cancelled.code, "provider_cancelled");
    }
}
