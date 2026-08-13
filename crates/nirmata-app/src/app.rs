use crate::ai::{AiRun, AiRunId, AiRunStatus};
use crate::context_bundle::{build_context_bundle_scoped, calendar_tick_label};
use crate::deep_review::{DeepReviewRun, DeepReviewRunId};
use crate::manual_review::{
    ManualReviewFreshnessSnapshot, annotate_report_with_change_operations, create_undo_review,
    object_snapshot_from_change_value,
};
use crate::variants::apply_variant_merge_review_action;
use crate::{
    AppError, ContextBundle, ContextBundleRequest, LogicalVfsDirectory, ManualDraftRequest,
    ManualDraftResponse, ManualReviewAction, ManualReviewActionRequest,
    ManualReviewFreshnessStatus, ManualReviewInput, ManualReviewObjectSnapshot,
    ManualReviewSession, ManualReviewSnapshot, ManualReviewWaiverSnapshot, OpenUriResponse,
    ProviderCredentialStatus, RelatedContextRequest, RelatedContextResponse, RevisionId,
    SearchWorldRequest, SearchWorldResponse, StoreError,
};
use nirmata_ai::ProviderCredentialStore;
use nirmata_core::{
    World, WorldId,
    change_set::ChangeSet,
    validation::{ValidationIssue, ValidationReport},
};
use nirmata_store::{
    CommittedChangeSetRecord, OperationAudit, ReadScope, StoredRevision, Variant,
    VariantComparison, WorldStore,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct CreateWorldInput {
    pub path: PathBuf,
    pub name: String,
    pub premise_md: String,
    pub epoch_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorldSession {
    pub path: PathBuf,
    pub world_id: WorldId,
    pub current_revision: RevisionId,
    pub world: World,
    pub active_variant: Variant,
    pub read_scope: ReadScope,
    pub read_only: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEventEntry {
    pub uri: String,
    pub summary: String,
    pub kind: String,
    pub time: nirmata_core::time::EventTime,
    pub start_calendar: Option<CalendarTickPresentation>,
    pub end_calendar: Option<CalendarTickPresentation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarTickPresentation {
    pub tick: i64,
    pub label: String,
    pub date_input: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineOverview {
    pub known: Vec<TimelineEventEntry>,
    pub unknown: Vec<TimelineEventEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionHistorySnapshot {
    pub current_head_revision_id: String,
    pub undo_target_revision_id: Option<String>,
    pub revisions: Vec<RevisionHistoryEntrySnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionHistoryEntrySnapshot {
    pub revision_id: String,
    pub parent_revision_id: Option<String>,
    pub change_set_id: String,
    pub author: String,
    pub summary: String,
    pub created_at_ms: i64,
    pub undone_revision_id: Option<String>,
    pub is_current_head: bool,
    pub is_current_undo_target: bool,
    pub operations: Vec<RevisionAuditOperationSnapshot>,
    pub waivers: Vec<ManualReviewWaiverSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionAuditOperationSnapshot {
    pub operation_id: String,
    pub target_uri: String,
    pub decision: String,
    pub source: String,
    pub decided_at_ms: i64,
    pub before: Option<ManualReviewObjectSnapshot>,
    pub after: Option<ManualReviewObjectSnapshot>,
    pub waivers: Vec<ManualReviewWaiverSnapshot>,
}

pub(crate) struct ActiveWorld {
    pub(crate) store: WorldStore,
    pub(crate) session: WorldSession,
    pub(crate) read_scope: ReadScope,
}

#[derive(Clone)]
pub(crate) struct StoredManualReview {
    pub(crate) review: ManualReviewSession,
    pub(crate) freshness: StoredManualReviewFreshness,
    pub(crate) ai_run_id: Option<AiRunId>,
    pub(crate) revalidation_allowed: bool,
    pub(crate) merge_source_revision: Option<RevisionId>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum StoredManualReviewFreshness {
    Current,
    Stale { current_revision: RevisionId },
    RefreshRestartRequired { current_revision: RevisionId },
}

pub struct NirmataApp {
    pub(crate) active: Option<ActiveWorld>,
    pub(crate) manual_reviews: HashMap<String, StoredManualReview>,
    pub(crate) ai_runs: HashMap<AiRunId, AiRun>,
    pub(crate) deep_review_runs: HashMap<DeepReviewRunId, DeepReviewRun>,
    pub(crate) import_review_traces: HashMap<String, Value>,
    pub(crate) simulation_scenarios:
        BTreeMap<crate::SimulationScenarioId, crate::SimulationScenario>,
    pub(crate) provider_credentials: ProviderCredentialStore,
}

impl Default for NirmataApp {
    fn default() -> Self {
        Self {
            active: None,
            manual_reviews: HashMap::new(),
            ai_runs: HashMap::new(),
            deep_review_runs: HashMap::new(),
            import_review_traces: HashMap::new(),
            simulation_scenarios: BTreeMap::new(),
            provider_credentials: ProviderCredentialStore::new(),
        }
    }
}

impl StoredManualReview {
    pub(crate) fn new(review: ManualReviewSession) -> Self {
        Self {
            review,
            freshness: StoredManualReviewFreshness::Current,
            ai_run_id: None,
            revalidation_allowed: true,
            merge_source_revision: None,
        }
    }

    pub(crate) fn from_snapshot_import(review: ManualReviewSession) -> Self {
        Self {
            review,
            freshness: StoredManualReviewFreshness::Current,
            ai_run_id: None,
            revalidation_allowed: false,
            merge_source_revision: None,
        }
    }

    pub(crate) fn from_ai(review: ManualReviewSession, ai_run_id: AiRunId) -> Self {
        Self {
            review,
            freshness: StoredManualReviewFreshness::Current,
            ai_run_id: Some(ai_run_id),
            revalidation_allowed: true,
            merge_source_revision: None,
        }
    }

    pub(crate) fn sync_with_revision(&mut self, current_revision: RevisionId) {
        self.freshness = if self.review.draft().base_revision() == current_revision {
            StoredManualReviewFreshness::Current
        } else {
            match self.freshness {
                StoredManualReviewFreshness::RefreshRestartRequired { .. } => {
                    StoredManualReviewFreshness::RefreshRestartRequired { current_revision }
                }
                _ => StoredManualReviewFreshness::Stale { current_revision },
            }
        };
    }

    pub(crate) fn snapshot(&self, review_key: &str) -> ManualReviewSnapshot {
        let mut snapshot = self.review.snapshot(
            review_key,
            self.freshness.snapshot(self.review.draft().base_revision()),
        );
        if !self.revalidation_allowed
            && snapshot.freshness.status != ManualReviewFreshnessStatus::Current
        {
            snapshot.freshness.can_revalidate = false;
            snapshot.freshness.message =
                "El snapshot se basa en otra revisión; exporta e importa una copia nueva."
                    .to_owned();
        }
        snapshot
    }
}

impl StoredManualReviewFreshness {
    fn snapshot(self, base_revision: RevisionId) -> ManualReviewFreshnessSnapshot {
        match self {
            Self::Current => ManualReviewFreshnessSnapshot {
                status: ManualReviewFreshnessStatus::Current,
                current_revision: base_revision.to_string(),
                can_revalidate: false,
                message: "La revisión está alineada con la cabeza actual.".to_owned(),
            },
            Self::Stale { current_revision } => ManualReviewFreshnessSnapshot {
                status: ManualReviewFreshnessStatus::Stale,
                current_revision: current_revision.to_string(),
                can_revalidate: true,
                message: format!(
                    "La cabeza cambió a {current_revision}. Revalida antes de confirmar."
                ),
            },
            Self::RefreshRestartRequired { current_revision } => ManualReviewFreshnessSnapshot {
                status: ManualReviewFreshnessStatus::RefreshRestartRequired,
                current_revision: current_revision.to_string(),
                can_revalidate: true,
                message: format!(
                    "La cabeza volvió a cambiar mientras se refrescaba ({current_revision}). Reinicia la revalidación."
                ),
            },
        }
    }
}

impl NirmataApp {
    pub fn create_world(&mut self, input: CreateWorldInput) -> Result<WorldSession, AppError> {
        self.ensure_no_active_world()?;
        validate_project_path(&input.path)?;
        let world = World::new(input.name, input.premise_md, input.epoch_label, now_ms()?)?;
        let store = WorldStore::create(&input.path, &world)?;
        self.activate(input.path, store, world)
    }

    pub fn open_world(&mut self, path: PathBuf) -> Result<WorldSession, AppError> {
        self.ensure_no_active_world()?;
        validate_project_path(&path)?;
        let store = WorldStore::open(&path)?;
        let world = store.load_world()?;
        self.activate(path, store, world)
    }

    pub fn get_current_world(&mut self) -> Result<Option<WorldSession>, AppError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        refresh_active_world(active)?;
        Ok(Some(active.session.clone()))
    }

    pub fn build_context_bundle(
        &self,
        request: &ContextBundleRequest,
    ) -> Result<ContextBundle, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        build_context_bundle_scoped(&active.store, active.read_scope, request)
    }

    pub fn search_world(
        &self,
        request: &SearchWorldRequest,
    ) -> Result<SearchWorldResponse, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        crate::search_use_cases::search_world(&active.store, active.read_scope, request)
    }

    pub fn open_uri(&self, uri: &str) -> Result<OpenUriResponse, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let uri = validate_object_uri(uri)?;
        crate::search_use_cases::open_uri(&active.store, active.read_scope, uri)
    }

    pub fn get_related_context(
        &self,
        request: &RelatedContextRequest,
    ) -> Result<RelatedContextResponse, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        crate::search_use_cases::get_related_context(&active.store, active.read_scope, request)
    }

    pub fn read_logical_vfs(&self) -> Result<LogicalVfsDirectory, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        active
            .store
            .read_logical_vfs_scoped(active.read_scope)
            .map_err(Into::into)
    }

    pub fn preview_manual_draft(
        &mut self,
        request: ManualDraftRequest,
    ) -> Result<ManualDraftResponse, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        ensure_active_write_scope(active)?;
        let world = active.store.load_world()?;
        active.session.world_id = world.id();
        active.session.current_revision = world.current_revision();
        active.session.world = world.clone();
        let outcome = crate::manual_forms::preview_manual_draft(
            &active.store,
            &active.session,
            &world,
            request,
            now_ms()?,
        )?;
        if let (Some(review), Some(snapshot)) = (&outcome.review, &outcome.response.review) {
            if self.manual_reviews.contains_key(&snapshot.review_key) {
                return Err(AppError::ReviewSessionConflict(snapshot.review_key.clone()));
            }
            self.manual_reviews.insert(
                snapshot.review_key.clone(),
                StoredManualReview::new(review.clone()),
            );
        }
        Ok(outcome.response)
    }

    pub fn close_world(&mut self) -> Result<(), AppError> {
        self.active.take().ok_or(AppError::NoWorldOpen)?;
        self.manual_reviews.clear();
        self.ai_runs.clear();
        self.deep_review_runs.clear();
        self.import_review_traces.clear();
        self.simulation_scenarios.clear();
        Ok(())
    }

    pub fn list_variants(&self) -> Result<Vec<Variant>, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        active.store.list_variants().map_err(Into::into)
    }

    pub fn create_variant(
        &mut self,
        name: &str,
        from_revision: RevisionId,
    ) -> Result<Variant, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        active
            .store
            .create_variant(name, from_revision, now_ms()?)
            .map_err(Into::into)
    }

    pub fn rename_variant(
        &mut self,
        id: nirmata_core::VariantId,
        name: &str,
    ) -> Result<Variant, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        let variant = active.store.rename_variant(id, name)?;
        if active.session.active_variant.id == id {
            active.session.active_variant = variant.clone();
        }
        Ok(variant)
    }

    pub fn archive_variant(
        &mut self,
        id: nirmata_core::VariantId,
        allow_referenced: bool,
    ) -> Result<(), AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        active
            .store
            .archive_variant(id, allow_referenced)
            .map_err(Into::into)
    }

    pub fn switch_variant(
        &mut self,
        id: nirmata_core::VariantId,
    ) -> Result<WorldSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        let variant = active.store.switch_variant(id)?;
        active.read_scope = ReadScope::head(variant.id);
        active.session.active_variant = variant;
        active.session.read_scope = active.read_scope;
        active.session.read_only = false;
        refresh_active_world(active)?;
        self.manual_reviews.clear();
        self.ai_runs.clear();
        self.deep_review_runs.clear();
        self.import_review_traces.clear();
        Ok(active.session.clone())
    }

    pub fn set_read_scope(&mut self, scope: ReadScope) -> Result<WorldSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        let revision = active.store.resolve_scope(scope)?;
        let observed = active.store.read_canon_snapshot_scoped(scope)?;
        active.read_scope = scope;
        active.session.read_scope = if scope.revision_id.is_some() {
            ReadScope::historical(scope.variant_id, revision)
        } else {
            scope
        };
        active.session.read_only =
            scope.revision_id.is_some() || scope.variant_id != active.session.active_variant.id;
        active.session.world = observed.world().clone();
        Ok(active.session.clone())
    }

    pub fn view_active_head(&mut self) -> Result<WorldSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        active.read_scope = ReadScope::head(active.session.active_variant.id);
        active.session.read_scope = active.read_scope;
        active.session.read_only = false;
        active.session.world = active.store.load_world()?;
        Ok(active.session.clone())
    }

    pub fn compare_scopes(
        &self,
        left: ReadScope,
        right: ReadScope,
    ) -> Result<VariantComparison, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        active.store.compare_scopes(left, right).map_err(Into::into)
    }

    pub fn get_provider_credential_status(&self) -> ProviderCredentialStatus {
        self.provider_credentials.status()
    }

    pub fn set_provider_api_key(
        &mut self,
        api_key: String,
    ) -> Result<ProviderCredentialStatus, AppError> {
        self.provider_credentials
            .set_provider_api_key(api_key)
            .map_err(Into::into)
    }

    pub fn set_session_provider_api_key(
        &mut self,
        api_key: String,
    ) -> Result<ProviderCredentialStatus, AppError> {
        self.provider_credentials
            .set_session_provider_api_key(api_key)
            .map_err(Into::into)
    }

    pub fn clear_provider_api_key(&mut self) -> Result<ProviderCredentialStatus, AppError> {
        self.provider_credentials
            .clear_provider_api_key()
            .map_err(Into::into)
    }

    pub fn apply_stored_manual_review_action(
        &mut self,
        review_key: &str,
        action: ManualReviewActionRequest,
    ) -> Result<ManualReviewSnapshot, AppError> {
        validate_review_key(review_key)?;
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let review = self
            .manual_reviews
            .get(review_key)
            .cloned()
            .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.to_owned()))?;
        let action = action.into_action()?;
        let decided_at_ms = now_ms()?;
        let updated_review = if review.merge_source_revision.is_some() {
            apply_variant_merge_review_action(&review.review, action, decided_at_ms, &active.store)?
        } else {
            review
                .review
                .apply_action(action, decided_at_ms, &active.store)?
        };
        let mut updated = match (review.ai_run_id, review.revalidation_allowed) {
            (Some(run_id), _) => StoredManualReview::from_ai(updated_review, run_id),
            (None, false) => StoredManualReview::from_snapshot_import(updated_review),
            (None, true) => StoredManualReview::new(updated_review),
        };
        updated.merge_source_revision = review.merge_source_revision;
        updated.sync_with_revision(active.session.current_revision);
        self.manual_reviews.insert(review_key.to_owned(), updated);
        if let Some(run_id) = review.ai_run_id
            && let Some(run) = self.ai_runs.get_mut(&run_id)
        {
            run.mark_review_changed();
        }
        let snapshot = self
            .manual_reviews
            .get(review_key)
            .expect("review was just inserted")
            .snapshot(review_key);
        Ok(gate_ai_review_snapshot(
            snapshot,
            review
                .ai_run_id
                .and_then(|run_id| self.ai_runs.get(&run_id).map(AiRun::status)),
        ))
    }

    pub fn read_stored_manual_review(
        &mut self,
        review_key: &str,
    ) -> Result<ManualReviewSnapshot, AppError> {
        validate_review_key(review_key)?;
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let review = self
            .manual_reviews
            .get_mut(review_key)
            .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.to_owned()))?;
        review.sync_with_revision(active.session.current_revision);
        let ai_run_id = review.ai_run_id;
        let snapshot = review.snapshot(review_key);
        Ok(gate_ai_review_snapshot(
            snapshot,
            ai_run_id.and_then(|run_id| self.ai_runs.get(&run_id).map(AiRun::status)),
        ))
    }

    pub fn discard_stored_manual_review(&mut self, review_key: &str) -> Result<(), AppError> {
        validate_review_key(review_key)?;
        self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        self.manual_reviews
            .remove(review_key)
            .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.to_owned()))?;
        self.import_review_traces.remove(review_key);
        Ok(())
    }

    pub fn begin_stored_manual_review_edit(
        &mut self,
        review_key: &str,
        operation_id: nirmata_core::ChangeOperationId,
    ) -> Result<ManualDraftRequest, AppError> {
        validate_review_key(review_key)?;
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let review = self
            .manual_reviews
            .get_mut(review_key)
            .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.to_owned()))?;
        review.sync_with_revision(active.session.current_revision);
        crate::manual_forms::manual_request_for_review_operation(&review.review, operation_id)
    }

    pub fn apply_stored_manual_review_edit(
        &mut self,
        review_key: &str,
        operation_id: nirmata_core::ChangeOperationId,
        request: ManualDraftRequest,
    ) -> Result<ManualDraftResponse, AppError> {
        validate_review_key(review_key)?;
        let ai_run_id = self
            .manual_reviews
            .get(review_key)
            .and_then(|stored| stored.ai_run_id);
        let revalidation_allowed = self
            .manual_reviews
            .get(review_key)
            .map(|stored| stored.revalidation_allowed)
            .unwrap_or(true);
        let merge_source_revision = self
            .manual_reviews
            .get(review_key)
            .and_then(|stored| stored.merge_source_revision);
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let Some(stored) = self.manual_reviews.get_mut(review_key) else {
            return Err(AppError::ReviewSessionNotFound(review_key.to_owned()));
        };
        stored.sync_with_revision(active.session.current_revision);
        let prepared = crate::manual_forms::prepare_manual_operation(
            &active.store,
            &active.session,
            &active.session.world,
            request,
            now_ms()?,
        )?;
        let Some(prepared) = prepared.prepared else {
            return Ok(ManualDraftResponse {
                draft: None,
                review: None,
                field_issues: prepared.field_issues,
            });
        };
        let existing = stored
            .review
            .operations()
            .iter()
            .find(|operation| operation.operation_id() == operation_id)
            .ok_or(AppError::UnknownReviewOperation(operation_id))?;
        let updated = stored.review.apply_action(
            ManualReviewAction::Edit {
                operation_id,
                replacement: prepared
                    .built
                    .operation
                    .with_retcon(existing.current().retcon()),
            },
            now_ms()?,
            &active.store,
        )?;
        *stored = match (ai_run_id, revalidation_allowed) {
            (Some(run_id), _) => StoredManualReview::from_ai(updated, run_id),
            (None, false) => StoredManualReview::from_snapshot_import(updated),
            (None, true) => StoredManualReview::new(updated),
        };
        stored.merge_source_revision = merge_source_revision;
        stored.sync_with_revision(active.session.current_revision);
        let snapshot = stored.snapshot(review_key);
        let response = ManualDraftResponse {
            draft: None,
            review: Some(snapshot),
            field_issues: vec![],
        };
        if let Some(run_id) = ai_run_id
            && let Some(run) = self.ai_runs.get_mut(&run_id)
        {
            run.mark_review_changed();
        }
        Ok(ManualDraftResponse {
            review: response.review.map(|snapshot| {
                gate_ai_review_snapshot(
                    snapshot,
                    ai_run_id.and_then(|run_id| self.ai_runs.get(&run_id).map(AiRun::status)),
                )
            }),
            ..response
        })
    }

    pub fn revalidate_stored_manual_review(
        &mut self,
        review_key: &str,
    ) -> Result<ManualReviewSnapshot, AppError> {
        validate_review_key(review_key)?;
        if let Some(run_id) = self
            .manual_reviews
            .get(review_key)
            .and_then(|stored| stored.ai_run_id)
            && let Some(run) = self.ai_runs.get_mut(&run_id)
        {
            run.mark_review_changed();
        }
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let review = self
            .manual_reviews
            .get_mut(review_key)
            .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.to_owned()))?;
        if !review.revalidation_allowed {
            return Err(AppError::ManualReviewRevalidationFailed);
        }
        let live_head = active.session.current_revision;
        match review.freshness {
            StoredManualReviewFreshness::Stale { current_revision }
            | StoredManualReviewFreshness::RefreshRestartRequired { current_revision }
                if current_revision != live_head =>
            {
                review.freshness = StoredManualReviewFreshness::RefreshRestartRequired {
                    current_revision: live_head,
                };
                return Ok(review.snapshot(review_key));
            }
            _ => {}
        }

        let refreshed = review
            .review
            .revalidate_at_revision(live_head, &active.store)?;
        refresh_active_world(active)?;
        if active.session.current_revision != live_head {
            review.freshness = StoredManualReviewFreshness::RefreshRestartRequired {
                current_revision: active.session.current_revision,
            };
            return Ok(review.snapshot(review_key));
        }

        review.review = refreshed;
        review.sync_with_revision(live_head);
        Ok(review.snapshot(review_key))
    }

    pub fn confirm_stored_manual_review(
        &mut self,
        review_key: &str,
    ) -> Result<WorldSession, AppError> {
        validate_review_key(review_key)?;
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let (review, ai_run_id, merge_source_revision) = {
            let review = self
                .manual_reviews
                .get_mut(review_key)
                .ok_or_else(|| AppError::ReviewSessionNotFound(review_key.to_owned()))?;
            review.sync_with_revision(active.session.current_revision);
            if review.freshness != StoredManualReviewFreshness::Current {
                return Err(AppError::ManualReviewStale {
                    base_revision: review.review.draft().base_revision(),
                    current_revision: active.session.current_revision,
                });
            }
            (
                review.review.clone(),
                review.ai_run_id,
                review.merge_source_revision,
            )
        };
        let ai_trace = ai_run_id
            .map(|run_id| {
                self.ai_runs
                    .get(&run_id)
                    .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?
                    .commit_trace(review_key, review.draft())
            })
            .transpose()?;
        let import_trace = self.import_review_traces.get(review_key).cloned();
        let is_import = import_trace.is_some();
        let session = commit_review(
            active,
            &review,
            if is_import {
                "lore_import"
            } else if ai_run_id.is_some() {
                "ai_review"
            } else {
                "manual_review"
            },
            if is_import {
                "lore_import"
            } else if ai_run_id.is_some() {
                "ai_review"
            } else {
                "manual_review"
            },
            None,
            import_trace.or(ai_trace),
            merge_source_revision,
        )?;
        if let Some(run_id) = ai_run_id {
            self.ai_runs
                .get_mut(&run_id)
                .ok_or_else(|| AppError::AiRunNotFound(run_id.to_string()))?
                .mark_committed(session.current_revision)?;
        }
        self.manual_reviews.remove(review_key);
        self.import_review_traces.remove(review_key);
        Ok(session)
    }

    pub fn list_timeline_events(&self) -> Result<TimelineOverview, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let snapshot = active.store.read_canon_snapshot_scoped(active.read_scope)?;
        let calendar = snapshot.world().calendar();
        let mut known = Vec::new();
        let mut unknown = Vec::new();
        for aggregate in snapshot.events() {
            let start_calendar = aggregate
                .event()
                .time()
                .start_tick()
                .and_then(|tick| calendar_tick_presentation(calendar, tick));
            let end_calendar = aggregate
                .event()
                .time()
                .end_tick()
                .and_then(|tick| calendar_tick_presentation(calendar, tick));
            let entry = TimelineEventEntry {
                uri: format!("nirmata://event/{}", aggregate.event().id()),
                summary: aggregate.event().summary().to_owned(),
                kind: aggregate.event().kind().to_owned(),
                time: *aggregate.event().time(),
                start_calendar,
                end_calendar,
            };
            if aggregate.event().time().start_tick().is_some() {
                known.push(entry);
            } else {
                unknown.push(entry);
            }
        }
        known.sort_by(|left, right| {
            timeline_sort_key(&left.time, &left.summary)
                .cmp(&timeline_sort_key(&right.time, &right.summary))
        });
        unknown.sort_by(|left, right| left.summary.cmp(&right.summary));
        Ok(TimelineOverview { known, unknown })
    }

    pub fn list_revision_history(&mut self) -> Result<RevisionHistorySnapshot, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let observed_variant = active
            .store
            .get_variant(active.read_scope.variant_id)?
            .ok_or_else(|| StoreError::InvalidVariant("viewed variant was not found".to_owned()))?;
        let observed_head = observed_variant.head_revision_id;
        let undo_target_revision_id =
            if active.read_scope == ReadScope::head(active.session.active_variant.id) {
                match resolve_undo_target(&active.store, active.session.current_revision, None) {
                    Ok(target) => Some(target.revision.id()),
                    Err(AppError::NoUndoableRevision) => None,
                    Err(error) => return Err(error),
                }
            } else {
                None
            };
        let mut revisions = revision_chain(&active.store, observed_head)?
            .into_iter()
            .rev()
            .filter_map(|revision| {
                revision
                    .change_set_id()
                    .map(|change_set_id| (revision, change_set_id))
            })
            .map(|(revision, change_set_id)| {
                let record = active
                    .store
                    .get_committed_change_set(change_set_id)?
                    .ok_or_else(|| {
                        StoreError::InvalidChangeSet(format!(
                            "revision {} is missing its committed change set",
                            revision.id()
                        ))
                    })?;
                Ok(revision_history_entry_snapshot(
                    &record,
                    observed_head,
                    undo_target_revision_id,
                ))
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        revisions.shrink_to_fit();
        Ok(RevisionHistorySnapshot {
            current_head_revision_id: observed_head.to_string(),
            undo_target_revision_id: undo_target_revision_id.map(|value| value.to_string()),
            revisions,
        })
    }

    pub fn start_manual_review(
        &mut self,
        input: ManualReviewInput,
    ) -> Result<ManualReviewSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        ensure_active_write_scope(active)?;
        let world = active.store.load_world()?;
        active.session.world_id = world.id();
        active.session.current_revision = world.current_revision();
        active.session.world = world;

        ManualReviewSession::create(
            active.session.active_variant.id,
            active.session.world_id,
            active.session.current_revision,
            input,
            &active.store,
        )
    }

    pub fn apply_manual_review_action(
        &self,
        review: &mut ManualReviewSession,
        action: ManualReviewAction,
    ) -> Result<(), AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        *review = review.apply_action(action, now_ms()?, &active.store)?;
        Ok(())
    }

    pub fn confirm_manual_review(
        &mut self,
        review: &ManualReviewSession,
    ) -> Result<WorldSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        commit_review(
            active,
            review,
            "manual_review",
            "manual_review",
            None,
            None,
            None,
        )
    }

    pub fn undo_last_commit(&mut self) -> Result<WorldSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let target = resolve_undo_target(&active.store, active.session.current_revision, None)?;
        let review = create_undo_review(
            active.session.active_variant.id,
            active.session.world_id,
            active.session.current_revision,
            &target.revision,
            &target.record,
            &active.store,
            now_ms()?,
        )?;
        commit_review(
            active,
            &review,
            "undo",
            "undo",
            Some(target.revision.id()),
            None,
            None,
        )
    }

    pub fn undo_revision(&mut self, revision_id: RevisionId) -> Result<WorldSession, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        refresh_active_world(active)?;
        let target = resolve_undo_target(
            &active.store,
            active.session.current_revision,
            Some(revision_id),
        )?;
        let review = create_undo_review(
            active.session.active_variant.id,
            active.session.world_id,
            active.session.current_revision,
            &target.revision,
            &target.record,
            &active.store,
            now_ms()?,
        )?;
        commit_review(
            active,
            &review,
            "undo",
            "undo",
            Some(target.revision.id()),
            None,
            None,
        )
    }

    fn ensure_no_active_world(&self) -> Result<(), AppError> {
        if self.active.is_some() {
            return Err(AppError::WorldAlreadyOpen);
        }
        Ok(())
    }

    fn activate(
        &mut self,
        path: PathBuf,
        store: WorldStore,
        world: World,
    ) -> Result<WorldSession, AppError> {
        self.manual_reviews.clear();
        self.ai_runs.clear();
        self.deep_review_runs.clear();
        self.import_review_traces.clear();
        self.simulation_scenarios.clear();
        let active_variant = store.active_variant()?;
        let read_scope = ReadScope::head(active_variant.id);
        let session = WorldSession {
            path,
            world_id: world.id(),
            current_revision: world.current_revision(),
            world,
            active_variant,
            read_scope,
            read_only: false,
        };
        self.active = Some(ActiveWorld {
            store,
            session: session.clone(),
            read_scope,
        });
        Ok(session)
    }
}

fn calendar_tick_presentation(
    calendar: Option<&nirmata_core::calendar::WorldCalendar>,
    tick: i64,
) -> Option<CalendarTickPresentation> {
    let calendar = calendar?;
    let date = calendar.tick_to_date(tick).ok()?;
    Some(CalendarTickPresentation {
        tick,
        label: calendar_tick_label(Some(calendar), tick)?,
        date_input: format!(
            "{}|{}|{}|{}",
            date.year, date.month, date.day, date.tick_in_day
        ),
    })
}

fn timeline_sort_key(time: &nirmata_core::time::EventTime, summary: &str) -> (i64, i64, String) {
    (
        time.start_tick().unwrap_or(i64::MAX),
        time.end_tick()
            .unwrap_or(time.start_tick().unwrap_or(i64::MAX)),
        summary.to_owned(),
    )
}

pub(crate) fn now_ms() -> Result<i64, AppError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::ClockBeforeUnixEpoch)?
        .as_millis();
    i64::try_from(millis).map_err(|_| AppError::ClockOutOfRange)
}

#[derive(Debug, Clone)]
struct LogicalCommittedRevision {
    revision: StoredRevision,
    record: CommittedChangeSetRecord,
}

fn refresh_active_world(active: &mut ActiveWorld) -> Result<(), AppError> {
    let world = active.store.load_world()?;
    active.session.world_id = world.id();
    active.session.current_revision = world.current_revision();
    active.session.active_variant = active.store.active_variant()?;
    active.session.world = if active.read_scope == ReadScope::head(active.session.active_variant.id)
    {
        world
    } else {
        active
            .store
            .read_canon_snapshot_scoped(active.read_scope)?
            .world()
            .clone()
    };
    Ok(())
}

pub(crate) fn ensure_active_write_scope(active: &ActiveWorld) -> Result<(), AppError> {
    if active.read_scope != ReadScope::head(active.session.active_variant.id) {
        return Err(AppError::ReadOnlyScope);
    }
    Ok(())
}

fn commit_review(
    active: &mut ActiveWorld,
    review: &ManualReviewSession,
    revision_author: &str,
    audit_source: &str,
    undone_revision_id: Option<RevisionId>,
    deterministic_report: Option<Value>,
    source_revision: Option<RevisionId>,
) -> Result<WorldSession, AppError> {
    ensure_active_write_scope(active)?;
    if review.variant_id() != active.session.active_variant.id {
        return Err(AppError::ManualReviewVariantMismatch {
            expected: active.session.active_variant.id,
            found: review.variant_id(),
        });
    }
    let world = active.store.load_world()?;
    if world.current_revision() != review.draft().base_revision() {
        return Err(AppError::ManualReviewStale {
            base_revision: review.draft().base_revision(),
            current_revision: world.current_revision(),
        });
    }
    if !review.ready_to_confirm() {
        return Err(match undone_revision_id {
            Some(target_revision) => AppError::UndoConflict {
                target_revision,
                reason: first_issue_message(review.effective_report()),
            },
            None => AppError::ManualReviewNotReady,
        });
    }

    let change_set = ChangeSet::new(
        world.id(),
        review.draft().base_revision(),
        review.draft().objective().to_owned(),
        review.draft().sources().to_vec(),
        review.draft().assumptions().to_vec(),
        review.draft().operations().to_vec(),
        review.draft().decisions().to_vec(),
    )?;
    let mut validation_report = active.store.validate_change_set(&change_set)?;
    annotate_report_with_change_operations(&mut validation_report, change_set.operations());
    if !apply_review_waivers(&validation_report, review.waivers()).is_ok() {
        return Err(match undone_revision_id {
            Some(target_revision) => AppError::UndoConflict {
                target_revision,
                reason: first_issue_message(&validation_report),
            },
            None => AppError::ManualReviewRevalidationFailed,
        });
    }

    let committed_at_ms = now_ms()?;
    let revision = nirmata_store::StoredRevision::new(
        world.id(),
        Some(change_set.base_revision()),
        Some(change_set.id()),
        revision_author,
        review.draft().objective(),
        committed_at_ms,
    )?;
    let audits = review
        .operations()
        .iter()
        .filter(|operation| operation.is_selected())
        .map(|operation| {
            OperationAudit::from_operation(
                operation.current(),
                operation.decision(),
                audit_source,
                committed_at_ms,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let record = CommittedChangeSetRecord::new(
        change_set,
        deterministic_report,
        review.waivers().to_vec(),
        audits,
        revision,
        undone_revision_id,
    )?;
    active
        .store
        .commit_change_set_from_source(&record, source_revision)?;
    refresh_active_world(active)?;
    Ok(active.session.clone())
}

fn resolve_undo_target(
    store: &WorldStore,
    head_revision: RevisionId,
    requested_revision: Option<RevisionId>,
) -> Result<LogicalCommittedRevision, AppError> {
    let logical_revisions =
        logical_committed_revisions(store, head_revision, store.active_variant()?.id)?;
    let Some(current_target) = logical_revisions.last().cloned() else {
        return Err(AppError::NoUndoableRevision);
    };

    if let Some(found) = requested_revision {
        if current_target.revision.id() != found {
            return Err(AppError::UndoTargetNotCurrentLogicalAncestor {
                expected: current_target.revision.id(),
                found,
            });
        }
    }

    Ok(current_target)
}

fn logical_committed_revisions(
    store: &WorldStore,
    head_revision: RevisionId,
    variant_id: nirmata_core::VariantId,
) -> Result<Vec<LogicalCommittedRevision>, AppError> {
    let mut stack: Vec<LogicalCommittedRevision> = Vec::new();
    for revision in revision_chain(store, head_revision)? {
        if store.revision_variant_id(revision.id())? != variant_id {
            continue;
        }
        let Some(change_set_id) = revision.change_set_id() else {
            continue;
        };
        let record = store
            .get_committed_change_set(change_set_id)?
            .ok_or_else(|| {
                StoreError::InvalidChangeSet(format!(
                    "revision {} is missing its committed change set",
                    revision.id()
                ))
            })?;
        if let Some(undone_revision_id) = record.undone_revision_id() {
            let Some(top) = stack.pop() else {
                return Err(StoreError::InvalidChangeSet(format!(
                    "undo revision {} has no logical ancestor to revert",
                    revision.id()
                ))
                .into());
            };
            if top.revision.id() != undone_revision_id {
                return Err(StoreError::InvalidChangeSet(format!(
                    "undo revision {} must target the current logical ancestor {} but references {}",
                    revision.id(),
                    top.revision.id(),
                    undone_revision_id
                ))
                .into());
            }
        } else {
            stack.push(LogicalCommittedRevision { revision, record });
        }
    }
    Ok(stack)
}

fn revision_chain(
    store: &WorldStore,
    head_revision: RevisionId,
) -> Result<Vec<StoredRevision>, AppError> {
    let mut chain = Vec::new();
    let mut current = Some(head_revision);
    while let Some(revision_id) = current {
        let revision = store.get_revision(revision_id)?.ok_or_else(|| {
            StoreError::InvalidChangeSet(format!(
                "revision {} is missing from the linear history",
                revision_id
            ))
        })?;
        current = revision.parent_revision_id();
        chain.push(revision);
    }
    chain.reverse();
    Ok(chain)
}

fn revision_history_entry_snapshot(
    record: &CommittedChangeSetRecord,
    current_head_revision: RevisionId,
    undo_target_revision_id: Option<RevisionId>,
) -> RevisionHistoryEntrySnapshot {
    let revision = record.revision();
    RevisionHistoryEntrySnapshot {
        revision_id: revision.id().to_string(),
        parent_revision_id: revision.parent_revision_id().map(|value| value.to_string()),
        change_set_id: record.change_set().id().to_string(),
        author: revision.author().to_owned(),
        summary: revision.summary().to_owned(),
        created_at_ms: revision.created_at_ms(),
        undone_revision_id: record.undone_revision_id().map(|value| value.to_string()),
        is_current_head: revision.id() == current_head_revision,
        is_current_undo_target: undo_target_revision_id == Some(revision.id()),
        operations: record
            .change_set()
            .operations()
            .iter()
            .filter_map(|operation| {
                record
                    .audits()
                    .iter()
                    .find(|audit| audit.operation_id() == operation.operation_id())
                    .map(|audit| RevisionAuditOperationSnapshot {
                        operation_id: audit.operation_id().to_string(),
                        target_uri: operation.primary_ref().to_string(),
                        decision: decision_label(audit.decision()).to_owned(),
                        source: audit.source().to_owned(),
                        decided_at_ms: audit.decided_at_ms(),
                        before: audit.before().map(object_snapshot_from_change_value),
                        after: audit.after().map(object_snapshot_from_change_value),
                        waivers: operation_waiver_snapshots(record.waivers(), audit.operation_id()),
                    })
            })
            .collect(),
        waivers: record
            .waivers()
            .iter()
            .map(|waiver| ManualReviewWaiverSnapshot {
                issue_code: waiver.issue_code().to_owned(),
                rationale: waiver.rationale().to_owned(),
                created_at_ms: waiver.created_at_ms(),
            })
            .collect(),
    }
}

fn operation_waiver_snapshots(
    waivers: &[nirmata_store::ChangeSetWaiver],
    operation_id: nirmata_core::ChangeOperationId,
) -> Vec<ManualReviewWaiverSnapshot> {
    waivers
        .iter()
        .filter(|waiver| waiver.operation_id() == operation_id)
        .map(|waiver| ManualReviewWaiverSnapshot {
            issue_code: waiver.issue_code().to_owned(),
            rationale: waiver.rationale().to_owned(),
            created_at_ms: waiver.created_at_ms(),
        })
        .collect()
}

fn decision_label(decision: nirmata_store::OperationDecision) -> &'static str {
    match decision {
        nirmata_store::OperationDecision::Accept => "accept",
        nirmata_store::OperationDecision::Edit => "edit",
        nirmata_store::OperationDecision::Reject => "reject",
    }
}

fn validate_project_path(path: &std::path::Path) -> Result<(), AppError> {
    if path.as_os_str().is_empty() {
        return Err(AppError::InvalidProjectPath(PathBuf::new()));
    }
    let has_valid_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("nirmata"));
    if !has_valid_extension {
        return Err(AppError::InvalidProjectPath(path.to_path_buf()));
    }
    Ok(())
}

fn validate_object_uri<'a>(uri: &'a str) -> Result<&'a str, AppError> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidObjectUri(uri.to_owned()));
    }
    nirmata_core::document::ObjectRef::from_str(trimmed)
        .map_err(|_| AppError::InvalidObjectUri(trimmed.to_owned()))?;
    Ok(trimmed)
}

fn validate_review_key(review_key: &str) -> Result<(), AppError> {
    validate_object_uri(review_key).map(|_| ())
}

fn first_issue_message(report: &ValidationReport) -> String {
    report
        .errors
        .iter()
        .chain(report.conflicts.iter())
        .chain(report.warnings.iter())
        .chain(report.info.iter())
        .next()
        .map(|issue| issue.message.clone())
        .unwrap_or_else(|| "validation rejected the undo".to_owned())
}

fn gate_ai_review_snapshot(
    mut snapshot: ManualReviewSnapshot,
    run_status: Option<AiRunStatus>,
) -> ManualReviewSnapshot {
    if run_status.is_some() && run_status != Some(AiRunStatus::ReadyToCommit) {
        snapshot.ready_to_confirm = false;
    }
    snapshot
}

pub(crate) fn apply_review_waivers(
    report: &ValidationReport,
    waivers: &[nirmata_store::ChangeSetWaiver],
) -> ValidationReport {
    ValidationReport {
        errors: report.errors.clone(),
        conflicts: report
            .conflicts
            .iter()
            .filter(|issue| !issue_is_waived(issue, waivers))
            .cloned()
            .collect(),
        warnings: report
            .warnings
            .iter()
            .filter(|issue| !issue_is_waived(issue, waivers))
            .cloned()
            .collect(),
        info: report
            .info
            .iter()
            .filter(|issue| !issue_is_waived(issue, waivers))
            .cloned()
            .collect(),
    }
}

fn issue_is_waived(issue: &ValidationIssue, waivers: &[nirmata_store::ChangeSetWaiver]) -> bool {
    waivers.iter().any(|waiver| {
        waiver.issue_code() == issue.code && issue_has_operation(issue, waiver.operation_id())
    })
}

fn issue_has_operation(
    issue: &ValidationIssue,
    operation_id: nirmata_core::ChangeOperationId,
) -> bool {
    let operation_id = operation_id.to_string();
    issue
        .objects
        .iter()
        .any(|object| object.kind == "change_operation" && object.id == operation_id)
}
