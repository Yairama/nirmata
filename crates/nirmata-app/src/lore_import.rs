use crate::{
    AiProposalProgress, AiProviderConfig, AiRequestOptions, AiRunSnapshot, AppError,
    ContextBundleRequest, ContextIntent, DraftOperationInput, ManualReviewInput, NirmataApp,
    ai::{AiModeClient, map_capability_error},
};
use nirmata_ai::{
    capabilities::{CapabilityInvocation, InvocationMetadata, InvocationStatus},
    contracts::ImportCandidate,
};
use nirmata_core::{
    ChangeSetId, EntityId, RevisionId, World,
    change_set::RetconKind,
    claim::{Claim, ClaimAuthentication, ClaimObject},
    document::ObjectRef,
    entity::Entity,
    event::{Event, EventAggregate, EventParticipant},
    relation::Relation,
    rule::{Rule, RuleKind, RuleSeverity},
    time::{Certainty, EventTime},
};
use nirmata_store::{
    StoredImportBatch, StoredImportCandidate, StoredImportChunk, StoredImportSource,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    str::FromStr,
};

pub const MAX_IMPORT_SOURCE_BYTES: u64 = 1_048_576;
const MAX_IMPORT_PREVIEW_CHARS: usize = 4_000;
const MAX_IMPORT_CHUNK_BYTES: usize = 2_048;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSourceFormat {
    Markdown,
    Text,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateImportBatchInput {
    pub source_root: PathBuf,
    pub files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceSnapshot {
    pub id: String,
    pub path: PathBuf,
    pub file_name: String,
    pub format: ImportSourceFormat,
    pub content_hash: String,
    pub size_bytes: u64,
    pub status: String,
    pub preview: String,
    pub chunks: Vec<ImportChunkSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportChunkSnapshot {
    pub id: String,
    pub source_id: String,
    pub source_hash: String,
    pub ordinal: u32,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line_start: u32,
    pub line_end: u32,
    pub heading: Option<String>,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportChunkLocation {
    pub source_path: PathBuf,
    pub source_hash: String,
    pub chunk: ImportChunkSnapshot,
    pub original_matches_hash: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchSnapshot {
    pub id: String,
    pub world_id: String,
    pub target_revision: String,
    pub variant_id: String,
    pub status: String,
    pub sources: Vec<ImportSourceSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportExtractionProgress {
    Preparing,
    Extracting,
    Validating,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateSnapshot {
    pub id: String,
    pub candidate: ImportCandidate,
    pub resolved_source_candidate_id: Option<String>,
    pub resolved_target_candidate_id: Option<String>,
    pub status: String,
    pub identity_suggestion: String,
    pub identity_matches: Vec<ImportIdentityMatch>,
    pub identity_decision: Option<String>,
    pub canonical_uri: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportIdentityMatch {
    pub uri: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ImportCandidateDecision {
    New,
    Exact { canonical_uri: String },
    Ambiguous,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateDecisionRequest {
    pub candidate_id: String,
    pub selected: bool,
    pub identity: Option<ImportCandidateDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDecisionPoint {
    pub candidate_id: String,
    pub prompt: String,
    pub alternatives: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTrace {
    pub candidate_id: String,
    pub operation_uri: String,
    pub chunk_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReviewPreparation {
    pub batch_id: String,
    pub run: Option<AiRunSnapshot>,
    pub review_key: Option<String>,
    pub decision_points: Vec<ImportDecisionPoint>,
    pub traces: Vec<ImportTrace>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExtractionResult {
    pub batch_id: String,
    pub target_revision: String,
    pub candidates: Vec<ImportCandidateSnapshot>,
    pub invocations: Vec<InvocationMetadata>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportExtractionInput {
    batch_id: String,
    target_revision: String,
    focus_chunk_id: String,
    chunks: Vec<ImportChunkSnapshot>,
}

impl NirmataApp {
    pub fn create_import_batch(
        &mut self,
        input: CreateImportBatchInput,
    ) -> Result<ImportBatchSnapshot, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        crate::app::ensure_active_write_scope(active)?;
        let world = active.store.load_world()?;
        let variant = active.store.active_variant()?;
        let root = validate_source_root(&input.source_root)?;
        if input.files.is_empty() {
            return Err(invalid_import(
                &input.source_root,
                "select at least one source file",
            ));
        }

        let batch_id = ChangeSetId::new().to_string();
        let mut prepared = Vec::with_capacity(input.files.len());
        for path in input.files {
            prepared.push(read_source(&batch_id, &root, &path)?);
        }
        prepared.sort_by(|left, right| left.stored.id.cmp(&right.stored.id));
        let batch = StoredImportBatch {
            id: batch_id,
            world_id: world.id(),
            target_revision: world.current_revision(),
            variant_id: variant.id,
            status: "ready".to_owned(),
            created_at_ms: crate::app::now_ms()?,
        };
        let sources = prepared
            .iter()
            .map(|source| source.stored.clone())
            .collect::<Vec<_>>();
        let chunks = prepared
            .iter()
            .flat_map(|source| source.chunks.iter().cloned())
            .collect::<Vec<_>>();
        active
            .store
            .create_import_batch(&batch, &sources, &chunks)?;
        Ok(snapshot(batch, prepared))
    }

    pub fn read_import_batch(&self, batch_id: &str) -> Result<ImportBatchSnapshot, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let (batch, sources, chunks) = active
            .store
            .get_import_batch(batch_id)?
            .ok_or_else(|| AppError::LoreImportBatchNotFound(batch_id.to_owned()))?;
        Ok(snapshot(
            batch,
            sources
                .into_iter()
                .map(|stored| PreparedSource {
                    format: parse_format(&stored.format),
                    chunks: chunks
                        .iter()
                        .filter(|chunk| chunk.source_id == stored.id)
                        .cloned()
                        .collect(),
                    stored,
                })
                .collect(),
        ))
    }

    pub fn replace_import_source(
        &mut self,
        batch_id: &str,
        source_id: &str,
        path: PathBuf,
    ) -> Result<ImportBatchSnapshot, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        let (_, sources, _) = active
            .store
            .get_import_batch(batch_id)?
            .ok_or_else(|| AppError::LoreImportBatchNotFound(batch_id.to_owned()))?;
        let current = sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or_else(|| invalid_import(&path, "source does not belong to the batch"))?;
        let root = current
            .source_path
            .parent()
            .ok_or_else(|| invalid_import(&path, "source has no confined parent"))?;
        let mut replacement = read_source(batch_id, root, &path)?;
        if replacement.stored.id != current.id {
            return Err(invalid_import(
                &path,
                "replacement must use the same original source path",
            ));
        }
        replacement.stored.status = "replaced".to_owned();
        active
            .store
            .replace_import_source(&replacement.stored, &replacement.chunks)?;
        self.read_import_batch(batch_id)
    }

    pub fn open_import_chunk(
        &self,
        batch_id: &str,
        chunk_id: &str,
    ) -> Result<ImportChunkLocation, AppError> {
        let batch = self.read_import_batch(batch_id)?;
        let (source, chunk) = batch
            .sources
            .iter()
            .find_map(|source| {
                source
                    .chunks
                    .iter()
                    .find(|chunk| chunk.id == chunk_id)
                    .map(|chunk| (source, chunk))
            })
            .ok_or_else(|| invalid_import(Path::new(chunk_id), "chunk was not found"))?;
        let original_matches_hash = fs::symlink_metadata(&source.path)
            .ok()
            .filter(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
            .and_then(|_| fs::read(&source.path).ok())
            .is_some_and(|bytes| {
                format!("sha256:{:x}", Sha256::digest(bytes)) == source.content_hash
            });
        Ok(ImportChunkLocation {
            source_path: source.path.clone(),
            source_hash: source.content_hash.clone(),
            chunk: chunk.clone(),
            original_matches_hash,
        })
    }

    pub fn delete_import_batch(&mut self, batch_id: &str) -> Result<(), AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        if !active.store.delete_import_batch(batch_id)? {
            return Err(AppError::LoreImportBatchNotFound(batch_id.to_owned()));
        }
        let review_keys = self
            .import_review_traces
            .iter()
            .filter(|(_, trace)| {
                trace.get("batchId").and_then(serde_json::Value::as_str) == Some(batch_id)
            })
            .map(|(review_key, _)| review_key.clone())
            .collect::<Vec<_>>();
        for review_key in review_keys {
            self.manual_reviews.remove(&review_key);
            self.import_review_traces.remove(&review_key);
        }
        Ok(())
    }

    pub async fn execute_import_extraction<F>(
        &mut self,
        batch_id: &str,
        provider: &AiProviderConfig,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<ImportExtractionResult, AppError>
    where
        F: FnMut(ImportExtractionProgress) + Send,
    {
        let client = self.provider_client(provider)?;
        self.execute_import_extraction_with(batch_id, &client, options, on_progress)
            .await
    }

    pub(crate) async fn execute_import_extraction_with<C, F>(
        &mut self,
        batch_id: &str,
        client: &C,
        options: AiRequestOptions,
        mut on_progress: F,
    ) -> Result<ImportExtractionResult, AppError>
    where
        C: AiModeClient,
        F: FnMut(ImportExtractionProgress) + Send,
    {
        on_progress(ImportExtractionProgress::Preparing);
        let batch = self.read_import_batch(batch_id)?;
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let world = active.store.load_world()?;
        let active_variant = active.store.active_variant()?;
        if active_variant.id.to_string() != batch.variant_id
            || world.current_revision().to_string() != batch.target_revision
        {
            return Err(AppError::AiBaseRevisionMismatch {
                draft_base_revision: batch.target_revision.parse().map_err(|_| {
                    AppError::InvalidLoreImport {
                        path: PathBuf::from(batch_id),
                        reason: "invalid staged target revision".to_owned(),
                    }
                })?,
                current_revision: world.current_revision(),
            });
        }

        let focus_chunks = batch
            .sources
            .iter()
            .flat_map(|source| source.chunks.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut inputs = Vec::with_capacity(focus_chunks.len());
        for focus in &focus_chunks {
            let neighborhood = active
                .store
                .import_chunk_neighborhood(batch_id, &focus.id)?
                .into_iter()
                .map(chunk_snapshot)
                .collect::<Vec<_>>();
            inputs.push(ImportExtractionInput {
                batch_id: batch.id.clone(),
                target_revision: batch.target_revision.clone(),
                focus_chunk_id: focus.id.clone(),
                chunks: neighborhood,
            });
        }

        let mut extracted = BTreeMap::<String, ImportCandidate>::new();
        let mut invocations = Vec::with_capacity(inputs.len());
        for input in inputs {
            on_progress(ImportExtractionProgress::Extracting);
            let payload = serde_json::to_value(&input).map_err(|error| {
                AppError::Ai(nirmata_ai::AiError::InvalidResponse(error.to_string()))
            })?;
            let context_ids = input
                .chunks
                .iter()
                .map(|chunk| chunk.id.clone())
                .collect::<Vec<_>>();
            let CapabilityInvocation { output, metadata } = client
                .run_import_extraction(payload, context_ids, options.clone().into_request_options())
                .await
                .map_err(map_capability_error)?;
            for candidate in output.candidates {
                let id = candidate.candidate_id().as_str().to_owned();
                if let Some(existing) = extracted.get(&id) {
                    if existing != &candidate {
                        return Err(invalid_import(
                            Path::new(batch_id),
                            format!("candidate {id} changed between chunk extractions"),
                        ));
                    }
                } else {
                    extracted.insert(id, candidate);
                }
            }
            invocations.push(metadata);
        }

        on_progress(ImportExtractionProgress::Validating);
        let available_chunks = focus_chunks
            .iter()
            .map(|chunk| (chunk.id.as_str(), chunk))
            .collect::<BTreeMap<_, _>>();
        for candidate in extracted.values() {
            validate_candidate_citations(candidate, &available_chunks, batch_id)?;
        }
        let resolved = resolve_import_graph(batch_id, extracted.into_values().collect())?;
        let stored = resolved
            .iter()
            .map(|snapshot| stored_candidate(batch_id, snapshot))
            .collect::<Result<Vec<_>, AppError>>()?;
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        active.store.replace_import_candidates(batch_id, &stored)?;
        on_progress(ImportExtractionProgress::Completed);
        Ok(ImportExtractionResult {
            batch_id: batch.id,
            target_revision: batch.target_revision,
            candidates: resolved,
            invocations,
        })
    }

    pub fn read_import_candidates(
        &self,
        batch_id: &str,
    ) -> Result<Vec<ImportCandidateSnapshot>, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let entities = active.store.list_entities()?;
        active
            .store
            .list_import_candidates(batch_id)?
            .into_iter()
            .map(|stored| {
                let candidate = serde_json::from_str::<ImportCandidate>(&stored.payload_json)
                    .map_err(|error| invalid_import(Path::new(batch_id), error.to_string()))?;
                let (resolved_source_candidate_id, resolved_target_candidate_id) =
                    resolved_ids_from_json(&stored.citations_json)?;
                let (identity_suggestion, identity_matches) =
                    identity_suggestion(&candidate, &entities);
                Ok(ImportCandidateSnapshot {
                    id: stored.id,
                    candidate,
                    resolved_source_candidate_id,
                    resolved_target_candidate_id,
                    status: stored.status,
                    identity_suggestion,
                    identity_matches,
                    identity_decision: stored.identity_decision,
                    canonical_uri: stored.canonical_uri,
                })
            })
            .collect()
    }

    pub fn decide_import_candidate(
        &mut self,
        batch_id: &str,
        request: ImportCandidateDecisionRequest,
    ) -> Result<Vec<ImportCandidateSnapshot>, AppError> {
        let candidates = self.read_import_candidates(batch_id)?;
        let candidate = candidates
            .iter()
            .find(|candidate| candidate.id == request.candidate_id)
            .ok_or_else(|| invalid_import(Path::new(batch_id), "candidate was not found"))?;
        let (status, decision, canonical_uri) = if !request.selected {
            ("rejected", None, None)
        } else {
            let identity = request.identity.ok_or_else(|| {
                invalid_import(
                    Path::new(batch_id),
                    "selected candidate requires an identity decision",
                )
            })?;
            match identity {
                ImportCandidateDecision::New => ("selected", Some("new"), None),
                ImportCandidateDecision::Ambiguous => ("selected", Some("ambiguous"), None),
                ImportCandidateDecision::Exact { canonical_uri } => {
                    if !candidate
                        .identity_matches
                        .iter()
                        .any(|identity| identity.uri == canonical_uri)
                    {
                        return Err(invalid_import(
                            Path::new(batch_id),
                            "exact identity must select one of the canonical matches",
                        ));
                    }
                    ("selected", Some("exact"), Some(canonical_uri))
                }
            }
        };
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        if !active.store.decide_import_candidate(
            batch_id,
            &request.candidate_id,
            status,
            decision,
            canonical_uri.as_deref(),
        )? {
            return Err(invalid_import(
                Path::new(batch_id),
                "candidate was not found",
            ));
        }
        self.read_import_candidates(batch_id)
    }

    pub fn edit_import_candidate(
        &mut self,
        batch_id: &str,
        candidate_id: &str,
        replacement: ImportCandidate,
    ) -> Result<Vec<ImportCandidateSnapshot>, AppError> {
        let existing = self
            .read_import_candidates(batch_id)?
            .into_iter()
            .find(|candidate| candidate.id == candidate_id)
            .ok_or_else(|| invalid_import(Path::new(batch_id), "candidate was not found"))?;
        if existing.candidate.candidate_id() != replacement.candidate_id()
            || existing.candidate.kind_label() != replacement.kind_label()
            || existing.candidate.citations() != replacement.citations()
        {
            return Err(invalid_import(
                Path::new(batch_id),
                "candidate edits cannot change identity, kind or provenance",
            ));
        }
        if !replacement.technical_confidence().is_finite()
            || !(0.0..=1.0).contains(&replacement.technical_confidence())
        {
            return Err(invalid_import(
                Path::new(batch_id),
                "invalid technical confidence",
            ));
        }
        let payload = serde_json::to_string(&replacement)
            .map_err(|error| invalid_import(Path::new(batch_id), error.to_string()))?;
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        if !active.store.edit_import_candidate(
            batch_id,
            candidate_id,
            replacement.kind_label(),
            &payload,
            replacement.technical_confidence(),
            replacement.contradiction_key().map(|key| key.as_str()),
        )? {
            return Err(invalid_import(
                Path::new(batch_id),
                "candidate was not found",
            ));
        }
        self.read_import_candidates(batch_id)
    }

    pub async fn prepare_import_review<F>(
        &mut self,
        batch_id: &str,
        provider: &AiProviderConfig,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<ImportReviewPreparation, AppError>
    where
        F: FnMut(AiProposalProgress) + Send,
    {
        let client = self.provider_client(provider)?;
        self.prepare_import_review_with(batch_id, &client, options, on_progress)
            .await
    }

    pub(crate) async fn prepare_import_review_with<C, F>(
        &mut self,
        batch_id: &str,
        client: &C,
        options: AiRequestOptions,
        on_progress: F,
    ) -> Result<ImportReviewPreparation, AppError>
    where
        C: AiModeClient,
        F: FnMut(AiProposalProgress) + Send,
    {
        let batch = self.read_import_batch(batch_id)?;
        let candidates = self.read_import_candidates(batch_id)?;
        let (draft, traces, decision_points, anchors) = {
            let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
            let world = active.store.load_world()?;
            let active_variant = active.store.active_variant()?;
            if active_variant.id.to_string() != batch.variant_id
                || world.current_revision().to_string() != batch.target_revision
            {
                return Err(AppError::AiBaseRevisionMismatch {
                    draft_base_revision: RevisionId::from_str(&batch.target_revision).map_err(
                        |_| invalid_import(Path::new(batch_id), "invalid target revision"),
                    )?,
                    current_revision: world.current_revision(),
                });
            }
            build_import_draft(&active.store, &world, batch_id, &candidates)?
        };
        if !decision_points.is_empty() {
            return Ok(ImportReviewPreparation {
                batch_id: batch_id.to_owned(),
                run: None,
                review_key: None,
                decision_points,
                traces,
            });
        }
        let draft = draft.ok_or_else(|| {
            invalid_import(
                Path::new(batch_id),
                "select at least one candidate before review",
            )
        })?;
        let mut context_request = ContextBundleRequest::new(ContextIntent::ContradictionCheck);
        context_request.anchors = anchors;
        context_request.include_perspectives = true;
        let metadata = InvocationMetadata {
            model: "deterministic-import-mapper".to_owned(),
            prompt_version: "import_mapping_v1".to_owned(),
            context_object_ids: draft
                .operations()
                .iter()
                .flat_map(|operation| operation.affected_ids().iter().map(ToString::to_string))
                .collect(),
            status: InvocationStatus::Completed,
            usage: None,
        };
        let run = self
            .hand_external_draft_to_standard_review(
                client,
                format!("Review selected lore import candidates from batch {batch_id}"),
                draft,
                metadata,
                &context_request,
                options,
                on_progress,
            )
            .await?;
        let review_key = run.review_key.clone();
        if let Some(review_key) = &review_key {
            let trace = serde_json::json!({
                "kind": "lore_import_review",
                "batchId": batch_id,
                "targetRevision": batch.target_revision,
                "sources": batch.sources.iter().map(|source| serde_json::json!({
                    "sourceId": source.id,
                    "path": source.path,
                    "contentHash": source.content_hash,
                })).collect::<Vec<_>>(),
                "traces": traces,
            });
            self.import_review_traces.insert(review_key.clone(), trace);
        }
        Ok(ImportReviewPreparation {
            batch_id: batch_id.to_owned(),
            run: Some(run),
            review_key,
            decision_points,
            traces,
        })
    }
}

fn validate_candidate_citations(
    candidate: &ImportCandidate,
    chunks: &BTreeMap<&str, &ImportChunkSnapshot>,
    batch_id: &str,
) -> Result<(), AppError> {
    for citation in candidate.citations() {
        let chunk = chunks.get(citation.chunk_id.as_str()).ok_or_else(|| {
            invalid_import(Path::new(batch_id), "candidate cites an unknown chunk")
        })?;
        if chunk.source_id != citation.source_id.as_str()
            || chunk.source_hash != citation.source_hash
            || !chunk.content.contains(&citation.excerpt)
        {
            return Err(invalid_import(
                Path::new(batch_id),
                "candidate citation does not match the current source hash and literal chunk",
            ));
        }
    }
    Ok(())
}

fn resolve_import_graph(
    batch_id: &str,
    candidates: Vec<ImportCandidate>,
) -> Result<Vec<ImportCandidateSnapshot>, AppError> {
    let mut names = BTreeMap::<String, BTreeSet<String>>::new();
    for candidate in &candidates {
        if let ImportCandidate::Entity {
            candidate_id,
            name,
            aliases,
            ..
        } = candidate
        {
            for value in std::iter::once(name).chain(aliases) {
                names
                    .entry(normalize_name(value))
                    .or_default()
                    .insert(candidate_id.as_str().to_owned());
            }
        }
    }
    let mut resolved = candidates
        .into_iter()
        .map(|candidate| {
            let (source, target) = match &candidate {
                ImportCandidate::Relation {
                    source_name,
                    target_name,
                    ..
                } => (
                    unique_candidate(&names, source_name),
                    unique_candidate(&names, target_name),
                ),
                _ => (None, None),
            };
            let identity = format!(
                "{batch_id}:{}:{}",
                candidate.kind_label(),
                candidate.candidate_id().as_str()
            );
            Ok(ImportCandidateSnapshot {
                id: stable_id("candidate", identity.as_bytes()),
                candidate,
                resolved_source_candidate_id: source,
                resolved_target_candidate_id: target,
                status: "pending".to_owned(),
                identity_suggestion: "new".to_owned(),
                identity_matches: vec![],
                identity_decision: None,
                canonical_uri: None,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    resolved.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(resolved)
}

fn unique_candidate(names: &BTreeMap<String, BTreeSet<String>>, name: &str) -> Option<String> {
    let values = names.get(&normalize_name(name))?;
    (values.len() == 1)
        .then(|| values.first().cloned())
        .flatten()
}

fn normalize_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn identity_suggestion(
    candidate: &ImportCandidate,
    entities: &[Entity],
) -> (String, Vec<ImportIdentityMatch>) {
    let ImportCandidate::Entity { name, aliases, .. } = candidate else {
        return ("new".to_owned(), vec![]);
    };
    let candidate_names = std::iter::once(name)
        .chain(aliases)
        .map(|value| normalize_name(value))
        .collect::<BTreeSet<_>>();
    let matches = entities
        .iter()
        .filter(|entity| {
            std::iter::once(entity.name())
                .chain(entity.aliases().iter().map(String::as_str))
                .map(normalize_name)
                .any(|name| candidate_names.contains(&name))
        })
        .map(|entity| ImportIdentityMatch {
            uri: ObjectRef::Entity(entity.id()).to_string(),
            name: entity.name().to_owned(),
        })
        .collect::<Vec<_>>();
    let suggestion = match matches.len() {
        0 => "new",
        1 => "exact",
        _ => "ambiguous",
    };
    (suggestion.to_owned(), matches)
}

type ImportDraftMaterial = (
    Option<nirmata_core::change_set::ChangeSetDraft>,
    Vec<ImportTrace>,
    Vec<ImportDecisionPoint>,
    Vec<ObjectRef>,
);

fn build_import_draft(
    store: &nirmata_store::WorldStore,
    world: &World,
    batch_id: &str,
    candidates: &[ImportCandidateSnapshot],
) -> Result<ImportDraftMaterial, AppError> {
    let selected = candidates
        .iter()
        .filter(|candidate| candidate.status == "selected")
        .collect::<Vec<_>>();
    let mut decision_points = selected
        .iter()
        .filter(|candidate| candidate.identity_decision.as_deref() == Some("ambiguous"))
        .map(|candidate| ImportDecisionPoint {
            candidate_id: candidate.id.clone(),
            prompt: "Elige una identidad canónica existente o crea un objeto nuevo.".to_owned(),
            alternatives: candidate
                .identity_matches
                .iter()
                .map(|identity| identity.uri.clone())
                .chain(std::iter::once("new".to_owned()))
                .collect(),
        })
        .collect::<Vec<_>>();
    for candidate in &selected {
        if matches!(
            candidate.candidate,
            ImportCandidate::Claim {
                authentication: ClaimAuthentication::Attributed,
                ..
            }
        ) {
            decision_points.push(ImportDecisionPoint {
                candidate_id: candidate.id.clone(),
                prompt: "Una afirmación atribuida necesita titular y modalidad. Márcala como canónica solo si expresa un hecho del mundo, o recházala."
                    .to_owned(),
                alternatives: vec!["mark_canonical".to_owned(), "reject".to_owned()],
            });
        }
    }
    if !decision_points.is_empty() {
        return Ok((None, vec![], decision_points, vec![]));
    }

    let canonical_entities = store.list_entities()?;
    let mut occupied_slugs = canonical_entities
        .iter()
        .map(|entity| entity.slug().to_owned())
        .collect::<BTreeSet<_>>();
    let mut entity_by_candidate = BTreeMap::<String, Entity>::new();
    let mut entity_by_name = BTreeMap::<String, BTreeSet<EntityId>>::new();
    for entity in &canonical_entities {
        index_entity_names(&mut entity_by_name, entity);
    }
    let mut operations = Vec::new();
    let mut traces = Vec::new();
    let mut anchors = BTreeSet::new();

    for candidate in &selected {
        let ImportCandidate::Entity {
            candidate_id,
            name,
            entity_kind,
            aliases,
            summary,
            ..
        } = &candidate.candidate
        else {
            continue;
        };
        let entity = match candidate.identity_decision.as_deref() {
            Some("exact") => {
                let uri = candidate.canonical_uri.as_deref().ok_or_else(|| {
                    invalid_import(
                        Path::new(batch_id),
                        "exact identity is missing its canonical URI",
                    )
                })?;
                let ObjectRef::Entity(entity_id) = ObjectRef::from_str(uri).map_err(|_| {
                    invalid_import(Path::new(batch_id), "invalid canonical entity URI")
                })?
                else {
                    return Err(invalid_import(
                        Path::new(batch_id),
                        "identity URI is not an entity",
                    ));
                };
                let before = store.get_entity(entity_id)?.ok_or_else(|| {
                    invalid_import(Path::new(batch_id), "canonical identity no longer exists")
                })?;
                let merged_aliases = merge_aliases(&before, name, aliases);
                let after = Entity::restore(
                    before.id(),
                    before.world_id(),
                    before.kind(),
                    before.name(),
                    before.slug(),
                    if summary.trim().is_empty() {
                        before.summary()
                    } else {
                        summary
                    },
                    before.body_md(),
                    before.attributes_json().as_str(),
                    merged_aliases,
                    before.version(),
                    before.created_at_ms(),
                    crate::app::now_ms()?,
                )?;
                operations.push(DraftOperationInput::UpdateEntity {
                    retcon: RetconKind::Additive,
                    before,
                    after: after.clone(),
                });
                anchors.insert(ObjectRef::Entity(entity_id));
                after
            }
            Some("new") => {
                let slug = unique_slug(name, candidate_id.as_str(), &mut occupied_slugs);
                let entity = Entity::new(
                    world.id(),
                    *entity_kind,
                    name,
                    slug,
                    summary,
                    "",
                    "{}",
                    aliases.clone(),
                    crate::app::now_ms()?,
                )?;
                operations.push(DraftOperationInput::CreateEntity {
                    retcon: RetconKind::Additive,
                    after: entity.clone(),
                });
                entity
            }
            _ => {
                return Err(invalid_import(
                    Path::new(batch_id),
                    "every selected entity requires exact, ambiguous or new identity",
                ));
            }
        };
        index_entity_names(&mut entity_by_name, &entity);
        entity_by_candidate.insert(candidate_id.as_str().to_owned(), entity.clone());
        traces.push(trace_for(candidate, ObjectRef::Entity(entity.id())));
    }

    for candidate in &selected {
        match &candidate.candidate {
            ImportCandidate::Entity { .. } => {}
            ImportCandidate::Relation {
                source_name,
                target_name,
                relation_kind,
                direction,
                ..
            } => {
                require_new_identity(candidate, batch_id)?;
                let source = resolve_endpoint(
                    candidate.resolved_source_candidate_id.as_deref(),
                    source_name,
                    &entity_by_candidate,
                    &entity_by_name,
                    batch_id,
                )?;
                let target = resolve_endpoint(
                    candidate.resolved_target_candidate_id.as_deref(),
                    target_name,
                    &entity_by_candidate,
                    &entity_by_name,
                    batch_id,
                )?;
                let relation = Relation::new(
                    world.id(),
                    source,
                    target,
                    relation_kind,
                    *direction,
                    None,
                    None,
                    Certainty::Uncertain,
                    Some(import_reference(batch_id, candidate)),
                    "{}",
                )?;
                traces.push(trace_for(candidate, ObjectRef::Relation(relation.id())));
                operations.push(DraftOperationInput::CreateRelation {
                    retcon: RetconKind::Additive,
                    after: relation,
                });
            }
            ImportCandidate::Event {
                summary,
                body_md,
                participant_names,
                ..
            } => {
                require_new_identity(candidate, batch_id)?;
                let participants = participant_names
                    .iter()
                    .enumerate()
                    .map(|(ordinal, name)| {
                        resolve_endpoint(
                            None,
                            name,
                            &entity_by_candidate,
                            &entity_by_name,
                            batch_id,
                        )
                        .and_then(|id| {
                            EventParticipant::new(id, "participant", ordinal as u32)
                                .map_err(Into::into)
                        })
                    })
                    .collect::<Result<Vec<_>, AppError>>()?;
                let event = Event::new(
                    world.id(),
                    "imported",
                    summary,
                    body_md,
                    EventTime::unknown(Certainty::Uncertain),
                    None,
                    participants,
                    vec![],
                    crate::app::now_ms()?,
                )?;
                traces.push(trace_for(candidate, ObjectRef::Event(event.id())));
                operations.push(DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(event, vec![]),
                });
            }
            ImportCandidate::Claim {
                subject_name,
                content_md,
                predicate_key,
                object_scalar,
                polarity,
                authentication,
                ..
            } => {
                require_new_identity(candidate, batch_id)?;
                let subject = resolve_endpoint(
                    None,
                    subject_name,
                    &entity_by_candidate,
                    &entity_by_name,
                    batch_id,
                )?;
                let claim = Claim::new(
                    world.id(),
                    subject,
                    content_md,
                    predicate_key.clone(),
                    object_scalar.clone().map(ClaimObject::Scalar),
                    *polarity,
                    *authentication,
                    None,
                    None,
                    Some("imported_source".to_owned()),
                    None,
                    Some(import_reference(batch_id, candidate)),
                    None,
                    None,
                    None,
                    None,
                    world.current_revision(),
                )?;
                traces.push(trace_for(candidate, ObjectRef::Claim(claim.id())));
                operations.push(DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: claim,
                });
            }
            ImportCandidate::Rule {
                statement_md,
                scope,
                ..
            } => {
                require_new_identity(candidate, batch_id)?;
                let rule = Rule::new(
                    world.id(),
                    RuleKind::Authorial,
                    statement_md,
                    scope,
                    RuleSeverity::Advisory,
                    Some(import_reference(batch_id, candidate)),
                    None,
                    "{}",
                    crate::app::now_ms()?,
                )?;
                traces.push(trace_for(candidate, ObjectRef::Rule(rule.id())));
                operations.push(DraftOperationInput::CreateRule {
                    retcon: RetconKind::Additive,
                    after: rule,
                });
            }
        }
    }

    if operations.is_empty() {
        return Ok((None, traces, vec![], anchors.into_iter().collect()));
    }
    let review = crate::manual_review::ManualReviewSession::create(
        store.active_variant()?.id,
        world.id(),
        world.current_revision(),
        ManualReviewInput {
            objective: "Import selected lore candidates".to_owned(),
            sources: vec![],
            assumptions: vec![format!(
                "External batch {batch_id} remains untrusted; only reviewed operations may commit."
            )],
            operations,
        },
        store,
    )?;
    Ok((
        Some(review.draft().clone()),
        traces,
        vec![],
        anchors.into_iter().collect(),
    ))
}

fn require_new_identity(
    candidate: &ImportCandidateSnapshot,
    batch_id: &str,
) -> Result<(), AppError> {
    if candidate.identity_decision.as_deref() != Some("new") {
        return Err(invalid_import(
            Path::new(batch_id),
            "selected non-entity candidates must be explicitly marked new",
        ));
    }
    Ok(())
}

fn index_entity_names(index: &mut BTreeMap<String, BTreeSet<EntityId>>, entity: &Entity) {
    for name in std::iter::once(entity.name()).chain(entity.aliases().iter().map(String::as_str)) {
        index
            .entry(normalize_name(name))
            .or_default()
            .insert(entity.id());
    }
}

fn resolve_endpoint(
    candidate_id: Option<&str>,
    name: &str,
    candidates: &BTreeMap<String, Entity>,
    names: &BTreeMap<String, BTreeSet<EntityId>>,
    batch_id: &str,
) -> Result<EntityId, AppError> {
    if let Some(entity) = candidate_id.and_then(|id| candidates.get(id)) {
        return Ok(entity.id());
    }
    let matches = names.get(&normalize_name(name)).ok_or_else(|| {
        invalid_import(
            Path::new(batch_id),
            format!("entity reference {name:?} has no explicit identity"),
        )
    })?;
    if matches.len() != 1 {
        return Err(invalid_import(
            Path::new(batch_id),
            format!("entity reference {name:?} is ambiguous"),
        ));
    }
    Ok(*matches.first().expect("one entity match"))
}

fn merge_aliases(existing: &Entity, candidate_name: &str, aliases: &[String]) -> Vec<String> {
    let mut values = BTreeMap::<String, String>::new();
    for alias in existing
        .aliases()
        .iter()
        .chain(aliases)
        .chain(std::iter::once(&candidate_name.to_owned()))
    {
        if normalize_name(alias) != normalize_name(existing.name()) {
            values
                .entry(normalize_name(alias))
                .or_insert_with(|| alias.clone());
        }
    }
    values.into_values().collect()
}

fn unique_slug(name: &str, candidate_id: &str, occupied: &mut BTreeSet<String>) -> String {
    let mut base = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if base.is_empty() {
        base = "imported".to_owned();
    }
    let mut slug = base.clone();
    if occupied.contains(&slug) {
        slug = format!("{base}-{}", &candidate_id[..candidate_id.len().min(8)]);
    }
    let mut suffix = 2;
    while !occupied.insert(slug.clone()) {
        slug = format!("{base}-{suffix}");
        suffix += 1;
    }
    slug
}

fn import_reference(batch_id: &str, candidate: &ImportCandidateSnapshot) -> String {
    let citation = candidate
        .candidate
        .citations()
        .first()
        .expect("contract requires citation");
    format!(
        "import://{batch_id}/{}/{}?hash={}",
        citation.source_id.as_str(),
        citation.chunk_id.as_str(),
        citation.source_hash
    )
}

fn trace_for(candidate: &ImportCandidateSnapshot, operation: ObjectRef) -> ImportTrace {
    ImportTrace {
        candidate_id: candidate.id.clone(),
        operation_uri: operation.to_string(),
        chunk_ids: candidate
            .candidate
            .citations()
            .iter()
            .map(|citation| citation.chunk_id.as_str().to_owned())
            .collect(),
    }
}

fn stored_candidate(
    batch_id: &str,
    snapshot: &ImportCandidateSnapshot,
) -> Result<StoredImportCandidate, AppError> {
    let citation = snapshot
        .candidate
        .citations()
        .first()
        .expect("AI contract requires citations");
    let trace = serde_json::json!({
        "citations": snapshot.candidate.citations(),
        "resolvedSourceCandidateId": snapshot.resolved_source_candidate_id,
        "resolvedTargetCandidateId": snapshot.resolved_target_candidate_id,
    });
    Ok(StoredImportCandidate {
        id: snapshot.id.clone(),
        batch_id: batch_id.to_owned(),
        source_id: citation.source_id.as_str().to_owned(),
        source_hash: citation.source_hash.clone(),
        kind: snapshot.candidate.kind_label().to_owned(),
        payload_json: serde_json::to_string(&snapshot.candidate)
            .map_err(|error| invalid_import(Path::new(batch_id), error.to_string()))?,
        citations_json: serde_json::to_string(&trace)
            .map_err(|error| invalid_import(Path::new(batch_id), error.to_string()))?,
        technical_confidence: snapshot.candidate.technical_confidence(),
        status: snapshot.status.clone(),
        identity_decision: None,
        canonical_uri: None,
        contradiction_key: snapshot
            .candidate
            .contradiction_key()
            .map(|key| key.as_str().to_owned()),
    })
}

fn resolved_ids_from_json(value: &str) -> Result<(Option<String>, Option<String>), AppError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Trace {
        resolved_source_candidate_id: Option<String>,
        resolved_target_candidate_id: Option<String>,
    }
    let trace = serde_json::from_str::<Trace>(value)
        .map_err(|error| invalid_import(Path::new("candidate-trace"), error.to_string()))?;
    Ok((
        trace.resolved_source_candidate_id,
        trace.resolved_target_candidate_id,
    ))
}

struct PreparedSource {
    stored: StoredImportSource,
    format: ImportSourceFormat,
    chunks: Vec<StoredImportChunk>,
}

fn validate_source_root(path: &Path) -> Result<PathBuf, AppError> {
    if !path.is_absolute() || has_unsafe_component(path) {
        return Err(invalid_import(
            path,
            "source root must be an absolute confined path",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| invalid_import(path, "source root is unreadable"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(invalid_import(
            path,
            "source root must be a non-symlink directory",
        ));
    }
    fs::canonicalize(path).map_err(|_| invalid_import(path, "source root is unreadable"))
}

fn read_source(batch_id: &str, root: &Path, path: &Path) -> Result<PreparedSource, AppError> {
    if !path.is_absolute() || has_unsafe_component(path) {
        return Err(invalid_import(
            path,
            "source path must be absolute and cannot traverse",
        ));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| invalid_import(path, "source is unreadable"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_import(
            path,
            "source must be a regular non-symlink file",
        ));
    }
    if metadata.len() > MAX_IMPORT_SOURCE_BYTES {
        return Err(invalid_import(path, "source exceeds the 1 MiB limit"));
    }
    let canonical =
        fs::canonicalize(path).map_err(|_| invalid_import(path, "source is unreadable"))?;
    if !canonical.starts_with(root) {
        return Err(invalid_import(path, "source escapes the selected root"));
    }
    let (format, format_label) = source_format(&canonical)?;
    let bytes = fs::read(&canonical).map_err(|_| invalid_import(path, "source is unreadable"))?;
    if bytes.len() as u64 != metadata.len() || bytes.len() as u64 > MAX_IMPORT_SOURCE_BYTES {
        return Err(invalid_import(
            path,
            "source changed while it was being read",
        ));
    }
    let content = std::str::from_utf8(&bytes)
        .map_err(|_| invalid_import(path, "source is not valid UTF-8"))?;
    if looks_binary(content) {
        return Err(invalid_import(
            path,
            "binary control content is not supported",
        ));
    }
    let content_hash = format!("sha256:{:x}", Sha256::digest(&bytes));
    let source_id = stable_id("source", canonical.to_string_lossy().as_bytes());
    let file_name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_import(path, "source filename is not UTF-8"))?
        .to_owned();
    let chunks = chunk_source(&source_id, &content_hash, content, format);
    Ok(PreparedSource {
        format,
        chunks,
        stored: StoredImportSource {
            id: source_id,
            batch_id: batch_id.to_owned(),
            source_path: canonical,
            file_name,
            format: format_label.to_owned(),
            content_hash,
            size_bytes: bytes.len() as u64,
            content_utf8: content.to_owned(),
            status: "ready".to_owned(),
        },
    })
}

fn chunk_source(
    source_id: &str,
    source_hash: &str,
    content: &str,
    format: ImportSourceFormat,
) -> Vec<StoredImportChunk> {
    if content.is_empty() {
        return vec![];
    }
    let mut boundaries = vec![0usize];
    let mut current_start = 0usize;
    for (line_start, line) in lines_with_offsets(content) {
        let structural_break = format == ImportSourceFormat::Markdown
            && line_start > current_start
            && markdown_heading(line).is_some();
        let size_break = line_start > current_start
            && line_start.saturating_sub(current_start) >= MAX_IMPORT_CHUNK_BYTES;
        if structural_break || size_break {
            boundaries.push(line_start);
            current_start = line_start;
        }
        while line_start.saturating_sub(current_start) > MAX_IMPORT_CHUNK_BYTES {
            let split = floor_char_boundary(content, current_start + MAX_IMPORT_CHUNK_BYTES);
            if split <= current_start {
                break;
            }
            boundaries.push(split);
            current_start = split;
        }
    }
    while content.len().saturating_sub(current_start) > MAX_IMPORT_CHUNK_BYTES {
        let split = floor_char_boundary(content, current_start + MAX_IMPORT_CHUNK_BYTES);
        boundaries.push(split);
        current_start = split;
    }
    boundaries.push(content.len());
    boundaries.sort_unstable();
    boundaries.dedup();

    boundaries
        .windows(2)
        .enumerate()
        .filter(|(_, range)| range[0] < range[1])
        .map(|(ordinal, range)| {
            let start = range[0];
            let end = range[1];
            let chunk_content = &content[start..end];
            let line_start = line_number_at(content, start);
            let line_end = line_number_at(content, end.saturating_sub(1));
            let heading = if format == ImportSourceFormat::Markdown {
                chunk_content.lines().find_map(markdown_heading)
            } else {
                None
            };
            let identity = format!("{source_id}:{source_hash}:{ordinal}:{start}:{end}");
            StoredImportChunk {
                id: stable_id("chunk", identity.as_bytes()),
                source_id: source_id.to_owned(),
                source_hash: source_hash.to_owned(),
                ordinal: ordinal as u32,
                byte_start: start as u64,
                byte_end: end as u64,
                line_start,
                line_end,
                heading,
                content_utf8: chunk_content.to_owned(),
            }
        })
        .collect()
}

fn lines_with_offsets(content: &str) -> Vec<(usize, &str)> {
    let mut offset = 0usize;
    content
        .split_inclusive('\n')
        .map(|line| {
            let start = offset;
            offset += line.len();
            (start, line)
        })
        .collect()
}

fn markdown_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let title = trimmed.strip_prefix('#')?.trim_start_matches('#').trim();
    (!title.is_empty()).then(|| title.to_owned())
}

fn floor_char_boundary(content: &str, mut offset: usize) -> usize {
    offset = offset.min(content.len());
    while !content.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn line_number_at(content: &str, offset: usize) -> u32 {
    1 + content.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count() as u32
}

fn source_format(path: &Path) -> Result<(ImportSourceFormat, &'static str), AppError> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("md" | "markdown") => Ok((ImportSourceFormat::Markdown, "markdown")),
        Some("txt") => Ok((ImportSourceFormat::Text, "text")),
        _ => Err(invalid_import(
            path,
            "only .md, .markdown and .txt are supported",
        )),
    }
}

fn looks_binary(content: &str) -> bool {
    content.contains('\0')
        || content
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn has_unsafe_component(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}

fn stable_id(prefix: &str, value: &[u8]) -> String {
    let hash = format!("{:x}", Sha256::digest(value));
    format!("{prefix}-{}", &hash[..24])
}

fn parse_format(value: &str) -> ImportSourceFormat {
    match value {
        "markdown" => ImportSourceFormat::Markdown,
        _ => ImportSourceFormat::Text,
    }
}

fn snapshot(batch: StoredImportBatch, sources: Vec<PreparedSource>) -> ImportBatchSnapshot {
    ImportBatchSnapshot {
        id: batch.id,
        world_id: batch.world_id.to_string(),
        target_revision: batch.target_revision.to_string(),
        variant_id: batch.variant_id.to_string(),
        status: batch.status,
        sources: sources
            .into_iter()
            .map(|source| ImportSourceSnapshot {
                id: source.stored.id,
                path: source.stored.source_path,
                file_name: source.stored.file_name,
                format: source.format,
                content_hash: source.stored.content_hash,
                size_bytes: source.stored.size_bytes,
                status: source.stored.status,
                preview: source
                    .stored
                    .content_utf8
                    .chars()
                    .take(MAX_IMPORT_PREVIEW_CHARS)
                    .collect(),
                chunks: source.chunks.into_iter().map(chunk_snapshot).collect(),
            })
            .collect(),
    }
}

fn chunk_snapshot(chunk: StoredImportChunk) -> ImportChunkSnapshot {
    ImportChunkSnapshot {
        id: chunk.id,
        source_id: chunk.source_id,
        source_hash: chunk.source_hash,
        ordinal: chunk.ordinal,
        byte_start: chunk.byte_start,
        byte_end: chunk.byte_end,
        line_start: chunk.line_start,
        line_end: chunk.line_end,
        heading: chunk.heading,
        content: chunk.content_utf8,
    }
}

fn invalid_import(path: &Path, reason: impl Into<String>) -> AppError {
    AppError::InvalidLoreImport {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(test)]
#[path = "../tests/unit/lore_import.rs"]
mod tests;
