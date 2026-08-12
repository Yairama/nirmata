use super::*;
use crate::CreateWorldInput;
use crate::ai::{AiModeClient, ClientFuture};
use nirmata_ai::{
    AiError, RequestOptions, StreamDelta,
    capabilities::{CapabilityError, CapabilityInvocation, InvocationMetadata, InvocationStatus},
    contracts::{
        AdvisoryResponse, ContractId, CritiqueReport, ImportCandidate, ImportCitation,
        ImportExtraction,
    },
};
use nirmata_core::{
    change_set::ChangeSetDraft,
    claim::{ClaimAuthentication, ClaimPolarity},
    entity::EntityKind,
    relation::RelationDirection,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    fs,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn fixture(label: &str) -> (NirmataApp, PathBuf) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/nirmata-tests")
        .join(format!("lore-{label}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root).expect("create test root");
    let root = fs::canonicalize(root).expect("canonical test root");
    let mut app = NirmataApp::default();
    app.create_world(CreateWorldInput {
        path: root.join("world.nirmata"),
        name: "Lore".to_owned(),
        premise_md: "Original canon".to_owned(),
        epoch_label: "Epoch".to_owned(),
    })
    .expect("create world");
    (app, root)
}

fn remove_fixture(root: PathBuf) {
    let mut last_error = None;
    for _ in 0..40 {
        match fs::remove_dir_all(&root) {
            Ok(()) => return,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
    panic!("remove fixture: {}", last_error.expect("cleanup error"));
}

#[test]
fn nir_065_ingests_confined_utf8_sources_with_inert_preview_and_hash() {
    let (mut app, root) = fixture("valid");
    let markdown = root.join("chronicle.md");
    let text = root.join("aliases.txt");
    fs::write(
        &markdown,
        "# Chronicle\n<script>alert(1)</script>\n[open](file:///secret)",
    )
    .expect("write markdown");
    fs::write(&text, "Mara is also called the Keeper.").expect("write text");
    let before = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();

    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![markdown.clone(), text],
        })
        .expect("create import batch");
    assert_eq!(batch.sources.len(), 2);
    assert!(
        batch
            .sources
            .iter()
            .all(|source| source.content_hash.starts_with("sha256:"))
    );
    assert!(
        batch
            .sources
            .iter()
            .any(|source| source.preview.contains("<script>"))
    );
    assert_eq!(app.read_import_batch(&batch.id).unwrap(), batch);
    let after = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    assert_eq!(before, after, "staging must not modify canon");

    app.delete_import_batch(&batch.id).expect("delete batch");
    assert!(matches!(
        app.read_import_batch(&batch.id),
        Err(AppError::LoreImportBatchNotFound(_))
    ));
    app.close_world().unwrap();
    remove_fixture(root);
}

#[test]
fn nir_065_rejects_unsupported_binary_oversized_and_unconfined_sources_atomically() {
    let (mut app, root) = fixture("hostile");
    let good = root.join("good.md");
    let binary = root.join("binary.txt");
    let unsupported = root.join("image.pdf");
    let oversized = root.join("large.md");
    fs::write(&good, "valid").unwrap();
    fs::write(&binary, b"lore\0binary").unwrap();
    fs::write(&unsupported, "not a PDF").unwrap();
    fs::write(&oversized, vec![b'x'; MAX_IMPORT_SOURCE_BYTES as usize + 1]).unwrap();
    let outside = root.parent().unwrap().join("outside-lore.txt");
    fs::write(&outside, "outside").unwrap();
    let before = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();

    for hostile in [&binary, &unsupported, &oversized, &outside] {
        let result = app.create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![good.clone(), hostile.clone()],
        });
        assert!(matches!(result, Err(AppError::InvalidLoreImport { .. })));
    }
    let after = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    assert_eq!(before, after);
    app.close_world().unwrap();
    fs::remove_file(outside).unwrap();
    remove_fixture(root);
}

#[test]
fn nir_066_chunks_are_stable_open_exact_ranges_and_replacement_invalidates_old_hash() {
    let (mut app, root) = fixture("chunks");
    let source = root.join("chronicle.md");
    let original = "# First\nMara entered the archive.\n\n## Second\nThe Keeper left.\n";
    fs::write(&source, original).unwrap();
    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![source.clone()],
        })
        .unwrap();
    let imported = &batch.sources[0];
    assert_eq!(
        imported
            .chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<String>(),
        original
    );
    assert_eq!(
        imported
            .chunks
            .iter()
            .map(|chunk| chunk.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let first = app
        .open_import_chunk(&batch.id, &imported.chunks[0].id)
        .unwrap();
    assert!(first.original_matches_hash);
    assert_eq!(
        &original.as_bytes()[first.chunk.byte_start as usize..first.chunk.byte_end as usize],
        first.chunk.content.as_bytes()
    );

    let old_hash = imported.content_hash.clone();
    let old_chunk_ids = imported
        .chunks
        .iter()
        .map(|chunk| chunk.id.clone())
        .collect::<Vec<_>>();
    fs::write(&source, "# Replaced\nOnly the new account remains.\n").unwrap();
    let replaced = app
        .replace_import_source(&batch.id, &imported.id, source.clone())
        .unwrap();
    let new_source = &replaced.sources[0];
    assert_ne!(new_source.content_hash, old_hash);
    assert!(
        new_source
            .chunks
            .iter()
            .all(|chunk| chunk.source_hash == new_source.content_hash)
    );
    assert!(
        old_chunk_ids
            .iter()
            .all(|id| app.open_import_chunk(&batch.id, id).is_err())
    );
    assert_eq!(
        new_source
            .chunks
            .iter()
            .map(|chunk| chunk.content.as_str())
            .collect::<String>(),
        "# Replaced\nOnly the new account remains.\n"
    );
    app.close_world().unwrap();
    remove_fixture(root);
}

#[derive(Clone)]
struct FakeImportClient {
    outputs: Arc<Mutex<VecDeque<ImportExtraction>>>,
}

impl AiModeClient for FakeImportClient {
    fn run_query<'a, F>(
        &'a self,
        _payload: Value,
        _context_object_ids: Vec<String>,
        _options: RequestOptions,
        _on_delta: F,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>>
    where
        F: FnMut(StreamDelta) + Send + 'a,
    {
        unavailable()
    }

    fn run_proposal<'a>(
        &'a self,
        _payload: Value,
        _context_object_ids: Vec<String>,
        _options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>> {
        unavailable()
    }

    fn run_critic<'a>(
        &'a self,
        _payload: Value,
        context_object_ids: Vec<String>,
        _options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<CritiqueReport>, CapabilityError>> {
        Box::pin(async move {
            Ok(CapabilityInvocation {
                output: CritiqueReport { issues: vec![] },
                metadata: InvocationMetadata {
                    model: "offline-critic-fake".to_owned(),
                    prompt_version: "critic_v3".to_owned(),
                    context_object_ids,
                    status: InvocationStatus::Completed,
                    usage: None,
                },
            })
        })
    }

    fn run_import_extraction<'a>(
        &'a self,
        _payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ImportExtraction>, CapabilityError>> {
        Box::pin(async move {
            if options
                .cancellation
                .is_some_and(|token| token.is_cancelled())
            {
                return Err(CapabilityError::Ai(AiError::RequestCancelled));
            }
            let output = self
                .outputs
                .lock()
                .expect("fake outputs")
                .pop_front()
                .expect("one output per chunk");
            Ok(CapabilityInvocation {
                output,
                metadata: InvocationMetadata {
                    model: "offline-import-fake".to_owned(),
                    prompt_version: "import_extraction_v1".to_owned(),
                    context_object_ids,
                    status: InvocationStatus::Completed,
                    usage: None,
                },
            })
        })
    }
}

fn unavailable<'a, T>() -> ClientFuture<'a, Result<T, CapabilityError>> {
    Box::pin(async {
        Err(CapabilityError::Ai(AiError::InvalidResponse(
            "unused fake capability".to_owned(),
        )))
    })
}

fn contract_id(value: &str) -> ContractId {
    ContractId::try_from(value.to_owned()).expect("valid contract id")
}

fn citation(chunk: &ImportChunkSnapshot, excerpt: &str) -> ImportCitation {
    ImportCitation {
        chunk_id: contract_id(&chunk.id),
        source_id: contract_id(&chunk.source_id),
        source_hash: chunk.source_hash.clone(),
        excerpt: excerpt.to_owned(),
    }
}

#[tokio::test]
async fn nir_067_offline_graph_aware_extraction_resolves_aliases_and_preserves_contradictions() {
    let (mut app, root) = fixture("extract");
    let source = root.join("multipage.md");
    fs::write(
        &source,
        "# People\nMara, called the Keeper, guards the Archive.\n\n# Reports\nThe Keeper entered the Archive. One report says the gate is open; another says it is not open.\n",
    )
    .unwrap();
    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![source],
        })
        .unwrap();
    let chunks = &batch.sources[0].chunks;
    assert_eq!(chunks.len(), 2);
    let first = citation(&chunks[0], "Mara, called the Keeper");
    let second = citation(&chunks[1], "The Keeper entered the Archive");
    let contradiction = Some(contract_id("gate-state"));
    let client = FakeImportClient {
        outputs: Arc::new(Mutex::new(VecDeque::from([
            ImportExtraction {
                candidates: vec![
                    ImportCandidate::Entity {
                        candidate_id: contract_id("mara"),
                        name: "Mara".to_owned(),
                        entity_kind: EntityKind::Person,
                        aliases: vec!["Keeper".to_owned()],
                        summary: "Guards the Archive.".to_owned(),
                        contradiction_key: None,
                        citations: vec![first.clone()],
                        technical_confidence: 0.95,
                    },
                    ImportCandidate::Entity {
                        candidate_id: contract_id("archive"),
                        name: "Archive".to_owned(),
                        entity_kind: EntityKind::Place,
                        aliases: vec![],
                        summary: String::new(),
                        contradiction_key: None,
                        citations: vec![first],
                        technical_confidence: 0.9,
                    },
                ],
            },
            ImportExtraction {
                candidates: vec![
                    ImportCandidate::Relation {
                        candidate_id: contract_id("keeper-entered-archive"),
                        source_name: "Keeper".to_owned(),
                        target_name: "Archive".to_owned(),
                        relation_kind: "entered".to_owned(),
                        direction: RelationDirection::Directed,
                        contradiction_key: None,
                        citations: vec![second.clone()],
                        technical_confidence: 0.88,
                    },
                    ImportCandidate::Claim {
                        candidate_id: contract_id("gate-open"),
                        subject_name: "Mara".to_owned(),
                        content_md: "The gate is open.".to_owned(),
                        predicate_key: Some("gate.open".to_owned()),
                        object_scalar: Some("true".to_owned()),
                        polarity: ClaimPolarity::Positive,
                        authentication: ClaimAuthentication::Canonical,
                        contradiction_key: contradiction.clone(),
                        citations: vec![second.clone()],
                        technical_confidence: 0.75,
                    },
                    ImportCandidate::Claim {
                        candidate_id: contract_id("gate-closed"),
                        subject_name: "Mara".to_owned(),
                        content_md: "The gate is not open.".to_owned(),
                        predicate_key: Some("gate.open".to_owned()),
                        object_scalar: Some("true".to_owned()),
                        polarity: ClaimPolarity::Negative,
                        authentication: ClaimAuthentication::Canonical,
                        contradiction_key: contradiction,
                        citations: vec![second],
                        technical_confidence: 0.72,
                    },
                ],
            },
        ]))),
    };
    let before = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    let result = app
        .execute_import_extraction_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    assert_eq!(result.invocations.len(), 2);
    assert_eq!(result.candidates.len(), 5);
    let relation = result
        .candidates
        .iter()
        .find(|candidate| matches!(candidate.candidate, ImportCandidate::Relation { .. }))
        .unwrap();
    assert_eq!(
        relation.resolved_source_candidate_id.as_deref(),
        Some("mara")
    );
    assert_eq!(
        relation.resolved_target_candidate_id.as_deref(),
        Some("archive")
    );
    let claims = result
        .candidates
        .iter()
        .filter(|candidate| matches!(candidate.candidate, ImportCandidate::Claim { .. }))
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 2);
    assert_eq!(
        claims[0].candidate.contradiction_key(),
        claims[1].candidate.contradiction_key()
    );
    assert!(
        result
            .candidates
            .iter()
            .all(|candidate| !candidate.candidate.citations().is_empty())
    );
    assert_eq!(
        app.read_import_candidates(&batch.id).unwrap(),
        result.candidates
    );
    let after = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    assert_eq!(before, after, "extraction is staging-only");
    assert!(
        app.deep_review_runs.is_empty(),
        "import does not depend on deep review"
    );
    app.close_world().unwrap();
    remove_fixture(root);
}

fn candidate_storage_id(candidates: &[ImportCandidateSnapshot], contract_id: &str) -> String {
    candidates
        .iter()
        .find(|candidate| candidate.candidate.candidate_id().as_str() == contract_id)
        .expect("candidate by contract id")
        .id
        .clone()
}

#[tokio::test]
async fn nir_068_only_explicitly_selected_identity_decisions_become_typed_review_operations() {
    let (mut app, root) = fixture("identity");
    let source = root.join("candidates.txt");
    fs::write(
        &source,
        "Mara guards the archive. The archive must remain sealed.",
    )
    .unwrap();
    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![source],
        })
        .unwrap();
    let chunk = &batch.sources[0].chunks[0];
    let cite = citation(chunk, "Mara guards the archive");
    let client = FakeImportClient {
        outputs: Arc::new(Mutex::new(VecDeque::from([ImportExtraction {
            candidates: vec![
                ImportCandidate::Entity {
                    candidate_id: contract_id("mara"),
                    name: "Mara".to_owned(),
                    entity_kind: EntityKind::Person,
                    aliases: vec![],
                    summary: "Archive guard".to_owned(),
                    contradiction_key: None,
                    citations: vec![cite.clone()],
                    technical_confidence: 0.95,
                },
                ImportCandidate::Rule {
                    candidate_id: contract_id("sealed-rule"),
                    statement_md: "The archive must remain sealed.".to_owned(),
                    scope: "archive".to_owned(),
                    contradiction_key: None,
                    citations: vec![cite.clone()],
                    technical_confidence: 0.8,
                },
                ImportCandidate::Entity {
                    candidate_id: contract_id("discarded"),
                    name: "Noise".to_owned(),
                    entity_kind: EntityKind::Concept,
                    aliases: vec![],
                    summary: String::new(),
                    contradiction_key: None,
                    citations: vec![cite],
                    technical_confidence: 0.2,
                },
            ],
        }]))),
    };
    let extraction = app
        .execute_import_extraction_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    let mara = candidate_storage_id(&extraction.candidates, "mara");
    let rule = candidate_storage_id(&extraction.candidates, "sealed-rule");
    let discarded = candidate_storage_id(&extraction.candidates, "discarded");
    for candidate_id in [mara, rule] {
        app.decide_import_candidate(
            &batch.id,
            ImportCandidateDecisionRequest {
                candidate_id,
                selected: true,
                identity: Some(ImportCandidateDecision::New),
            },
        )
        .unwrap();
    }
    app.decide_import_candidate(
        &batch.id,
        ImportCandidateDecisionRequest {
            candidate_id: discarded,
            selected: false,
            identity: None,
        },
    )
    .unwrap();
    let before = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    let prepared = app
        .prepare_import_review_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    assert!(prepared.decision_points.is_empty());
    let review = app
        .read_stored_manual_review(prepared.review_key.as_deref().unwrap())
        .unwrap();
    assert_eq!(review.operations.len(), 2);
    assert!(review.operations.iter().any(|operation| {
        operation
            .after
            .as_ref()
            .is_some_and(|after| after.object_type == "entity")
    }));
    assert!(review.operations.iter().any(|operation| {
        operation
            .after
            .as_ref()
            .is_some_and(|after| after.object_type == "rule")
    }));
    assert_eq!(prepared.traces.len(), 2);
    let after = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    assert_eq!(before, after, "review preparation cannot write canon");
    app.close_world().unwrap();
    remove_fixture(root);
}

#[tokio::test]
async fn nir_068_ambiguous_identity_stays_a_decision_point() {
    let (mut app, root) = fixture("ambiguous");
    let world = app.active.as_ref().unwrap().store.load_world().unwrap();
    for (name, slug) in [
        ("Keeper North", "keeper-north"),
        ("Keeper South", "keeper-south"),
    ] {
        let entity = nirmata_core::entity::Entity::new(
            world.id(),
            EntityKind::Person,
            name,
            slug,
            "",
            "",
            "{}",
            vec!["Keeper".to_owned()],
            1,
        )
        .unwrap();
        app.active
            .as_mut()
            .unwrap()
            .store
            .insert_entity(&entity)
            .unwrap();
    }
    let source = root.join("ambiguous.txt");
    fs::write(&source, "The Keeper arrived.").unwrap();
    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![source],
        })
        .unwrap();
    let cite = citation(&batch.sources[0].chunks[0], "The Keeper arrived");
    let client = FakeImportClient {
        outputs: Arc::new(Mutex::new(VecDeque::from([ImportExtraction {
            candidates: vec![ImportCandidate::Entity {
                candidate_id: contract_id("keeper"),
                name: "Keeper".to_owned(),
                entity_kind: EntityKind::Person,
                aliases: vec![],
                summary: String::new(),
                contradiction_key: None,
                citations: vec![cite],
                technical_confidence: 0.6,
            }],
        }]))),
    };
    let extraction = app
        .execute_import_extraction_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    let keeper = candidate_storage_id(&extraction.candidates, "keeper");
    let staged = app.read_import_candidates(&batch.id).unwrap();
    let candidate = staged
        .iter()
        .find(|candidate| candidate.id == keeper)
        .unwrap();
    assert_eq!(candidate.identity_suggestion, "ambiguous");
    assert_eq!(candidate.identity_matches.len(), 2);
    app.decide_import_candidate(
        &batch.id,
        ImportCandidateDecisionRequest {
            candidate_id: keeper,
            selected: true,
            identity: Some(ImportCandidateDecision::Ambiguous),
        },
    )
    .unwrap();
    let prepared = app
        .prepare_import_review_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    assert!(prepared.run.is_none());
    assert_eq!(prepared.decision_points.len(), 1);
    assert!(app.manual_reviews.is_empty());
    app.close_world().unwrap();
    remove_fixture(root);
}

#[tokio::test]
async fn nir_068_opposing_canonical_claim_remains_a_review_conflict() {
    let (mut app, root) = fixture("opposing");
    let world = app.active.as_ref().unwrap().store.load_world().unwrap();
    let mara = nirmata_core::entity::Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .unwrap();
    app.active
        .as_mut()
        .unwrap()
        .store
        .insert_entity(&mara)
        .unwrap();
    let positive = nirmata_core::claim::Claim::new(
        world.id(),
        mara.id(),
        "The gate is open.",
        Some("gate.open".to_owned()),
        Some(nirmata_core::claim::ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        world.current_revision(),
    )
    .unwrap();
    app.active
        .as_mut()
        .unwrap()
        .store
        .insert_claim(&positive)
        .unwrap();
    let source = root.join("opposing.txt");
    fs::write(&source, "The gate is not open.").unwrap();
    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![source],
        })
        .unwrap();
    let cite = citation(&batch.sources[0].chunks[0], "gate is not open");
    let client = FakeImportClient {
        outputs: Arc::new(Mutex::new(VecDeque::from([ImportExtraction {
            candidates: vec![ImportCandidate::Claim {
                candidate_id: contract_id("gate-negative"),
                subject_name: "Mara".to_owned(),
                content_md: "The gate is not open.".to_owned(),
                predicate_key: Some("gate.open".to_owned()),
                object_scalar: Some("true".to_owned()),
                polarity: ClaimPolarity::Negative,
                authentication: ClaimAuthentication::Canonical,
                contradiction_key: Some(contract_id("gate-state")),
                citations: vec![cite],
                technical_confidence: 0.8,
            }],
        }]))),
    };
    let extraction = app
        .execute_import_extraction_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    app.decide_import_candidate(
        &batch.id,
        ImportCandidateDecisionRequest {
            candidate_id: extraction.candidates[0].id.clone(),
            selected: true,
            identity: Some(ImportCandidateDecision::New),
        },
    )
    .unwrap();
    let prepared = app
        .prepare_import_review_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    let review = app
        .read_stored_manual_review(prepared.review_key.as_deref().unwrap())
        .unwrap();
    assert!(!review.effective_report.conflicts.is_empty());
    assert!(!review.ready_to_confirm);
    assert_eq!(
        app.active
            .as_ref()
            .unwrap()
            .store
            .list_claims()
            .unwrap()
            .len(),
        1
    );
    app.close_world().unwrap();
    remove_fixture(root);
}

#[tokio::test]
async fn nir_070_offline_multipage_import_commits_only_reviewed_provenance_and_undoes_after_reopen()
{
    let (mut app, root) = fixture("e2e");
    let fixture_root =
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lore_import"))
            .unwrap();
    let chronicle_fixture = fixture_root.join("chronicle.md");
    let orders_fixture = fixture_root.join("orders.txt");
    let chronicle_original = fs::read(&chronicle_fixture).unwrap();
    let orders_original = fs::read(&orders_fixture).unwrap();
    let chronicle = root.join("chronicle.md");
    let orders = root.join("orders.txt");
    fs::copy(&chronicle_fixture, &chronicle).unwrap();
    fs::copy(&orders_fixture, &orders).unwrap();

    let binary = root.join("hostile.txt");
    fs::write(&binary, b"binary\0payload").unwrap();
    let canon_before_hostile = app
        .active
        .as_ref()
        .unwrap()
        .store
        .read_canon_snapshot()
        .unwrap();
    assert!(matches!(
        app.create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![chronicle.clone(), binary],
        }),
        Err(AppError::InvalidLoreImport { .. })
    ));
    assert_eq!(
        canon_before_hostile,
        app.active
            .as_ref()
            .unwrap()
            .store
            .read_canon_snapshot()
            .unwrap()
    );

    let replace_file = root.join("replace.txt");
    fs::write(&replace_file, "old account").unwrap();
    let replaced_batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![replace_file.clone()],
        })
        .unwrap();
    let old_hash = replaced_batch.sources[0].content_hash.clone();
    fs::write(&replace_file, "new account").unwrap();
    let replaced_batch = app
        .replace_import_source(
            &replaced_batch.id,
            &replaced_batch.sources[0].id,
            replace_file,
        )
        .unwrap();
    assert_ne!(replaced_batch.sources[0].content_hash, old_hash);
    app.delete_import_batch(&replaced_batch.id).unwrap();

    let batch = app
        .create_import_batch(CreateImportBatchInput {
            source_root: root.clone(),
            files: vec![chronicle.clone(), orders.clone()],
        })
        .unwrap();
    let mut outputs = VecDeque::new();
    for chunk in batch.sources.iter().flat_map(|source| source.chunks.iter()) {
        let mut extracted = Vec::new();
        if chunk.content.contains("Mara, called the Keeper") {
            extracted.push(ImportCandidate::Entity {
                candidate_id: contract_id("mara-e2e"),
                name: "Mara".to_owned(),
                entity_kind: EntityKind::Person,
                aliases: vec!["Keeper".to_owned()],
                summary: "Guards the Archive of Bells.".to_owned(),
                contradiction_key: None,
                citations: vec![citation(chunk, "Mara, called the Keeper")],
                technical_confidence: 0.96,
            });
        }
        if chunk.content.contains("One witness says") {
            let cite = citation(chunk, "memory gate is open");
            extracted.push(ImportCandidate::Claim {
                candidate_id: contract_id("gate-open-e2e"),
                subject_name: "Mara".to_owned(),
                content_md: "One witness says the memory gate is open.".to_owned(),
                predicate_key: Some("memory_gate.open".to_owned()),
                object_scalar: Some("true".to_owned()),
                polarity: ClaimPolarity::Positive,
                authentication: ClaimAuthentication::Disputed,
                contradiction_key: Some(contract_id("memory-gate-state")),
                citations: vec![cite.clone()],
                technical_confidence: 0.82,
            });
            extracted.push(ImportCandidate::Claim {
                candidate_id: contract_id("gate-closed-e2e"),
                subject_name: "Mara".to_owned(),
                content_md: "Another witness says the memory gate is not open.".to_owned(),
                predicate_key: Some("memory_gate.open".to_owned()),
                object_scalar: Some("true".to_owned()),
                polarity: ClaimPolarity::Negative,
                authentication: ClaimAuthentication::Disputed,
                contradiction_key: Some(contract_id("memory-gate-state")),
                citations: vec![citation(chunk, "memory gate is not open")],
                technical_confidence: 0.79,
            });
            extracted.push(ImportCandidate::Rule {
                candidate_id: contract_id("hostile-script-e2e"),
                statement_md: "<script>fetch attacker</script>".to_owned(),
                scope: "untrusted source".to_owned(),
                contradiction_key: None,
                citations: vec![cite],
                technical_confidence: 0.1,
            });
        }
        if chunk.content.contains("Keeper entered") {
            extracted.push(ImportCandidate::Relation {
                candidate_id: contract_id("prompt-injection-relation"),
                source_name: "Keeper".to_owned(),
                target_name: "Archive of Bells".to_owned(),
                relation_kind: "entered".to_owned(),
                direction: RelationDirection::Directed,
                contradiction_key: None,
                citations: vec![citation(chunk, "Keeper entered the Archive of Bells")],
                technical_confidence: 0.7,
            });
        }
        outputs.push_back(ImportExtraction {
            candidates: extracted,
        });
    }
    let client = FakeImportClient {
        outputs: Arc::new(Mutex::new(outputs)),
    };

    let cancelled_token = crate::CancellationToken::new();
    cancelled_token.cancel();
    let cancelled = app
        .execute_import_extraction_with(
            &batch.id,
            &client,
            AiRequestOptions::default().with_cancellation(cancelled_token),
            |_| {},
        )
        .await;
    assert!(matches!(
        cancelled,
        Err(AppError::Ai(AiError::RequestCancelled))
    ));
    assert!(app.read_import_candidates(&batch.id).unwrap().is_empty());

    let extraction = app
        .execute_import_extraction_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    assert_eq!(extraction.candidates.len(), 5);
    for candidate in &extraction.candidates {
        let selected = matches!(
            candidate.candidate.candidate_id().as_str(),
            "mara-e2e" | "gate-open-e2e"
        );
        app.decide_import_candidate(
            &batch.id,
            ImportCandidateDecisionRequest {
                candidate_id: candidate.id.clone(),
                selected,
                identity: selected.then_some(ImportCandidateDecision::New),
            },
        )
        .unwrap();
    }
    let prepared = app
        .prepare_import_review_with(&batch.id, &client, AiRequestOptions::default(), |_| {})
        .await
        .unwrap();
    let run = prepared.run.clone().unwrap();
    let review_key = prepared.review_key.clone().unwrap();
    let review = app.read_stored_manual_review(&review_key).unwrap();
    assert_eq!(
        review.operations.len(),
        2,
        "only selected candidates become operations"
    );
    assert_eq!(prepared.traces.len(), 2);

    let world = app.active.as_ref().unwrap().store.load_world().unwrap();
    let filler = nirmata_core::entity::Entity::new(
        world.id(),
        EntityKind::Concept,
        "Intervening",
        "intervening",
        "",
        "",
        "{}",
        vec![],
        2,
    )
    .unwrap();
    let filler_review = app
        .start_manual_review(ManualReviewInput {
            objective: "Intervening commit".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: nirmata_core::change_set::RetconKind::Additive,
                after: filler.clone(),
            }],
        })
        .unwrap();
    app.confirm_manual_review(&filler_review).unwrap();
    let stale = app.read_stored_manual_review(&review_key).unwrap();
    assert_eq!(
        stale.freshness.status,
        crate::ManualReviewFreshnessStatus::Stale
    );
    app.apply_stored_manual_review_action(
        &review_key,
        crate::ManualReviewActionRequest::Accept {
            operation_id: stale.operations[0].operation_id.clone(),
        },
    )
    .unwrap();
    let context = crate::ContextBundleRequest::new(crate::ContextIntent::ContradictionCheck);
    let final_run = app
        .revalidate_ai_run_with(
            run.id,
            &client,
            &context,
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(final_run.status, crate::AiRunStatus::ReadyToCommit);
    let committed = app.confirm_stored_manual_review(&review_key).unwrap();
    let import_revision = committed.current_revision;

    let store = &app.active.as_ref().unwrap().store;
    assert_eq!(store.list_claims().unwrap().len(), 1);
    let claim = &store.list_claims().unwrap()[0];
    assert!(
        claim
            .source()
            .is_some_and(|source| source.starts_with("import://"))
    );
    assert!(
        store
            .list_entities()
            .unwrap()
            .iter()
            .any(|entity| entity.name() == "Mara")
    );
    assert!(
        !store
            .list_rules()
            .unwrap()
            .iter()
            .any(|rule| rule.statement_md().contains("script"))
    );
    assert!(store.list_relations().unwrap().is_empty());
    let revision = store.get_revision(import_revision).unwrap().unwrap();
    let record = store
        .get_committed_change_set(revision.change_set_id().unwrap())
        .unwrap()
        .unwrap();
    assert!(
        record
            .audits()
            .iter()
            .all(|audit| audit.source() == "lore_import")
    );
    let trace = record.deterministic_report().unwrap();
    assert_eq!(trace["kind"], "lore_import_review");
    assert_eq!(trace["traces"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read(&chronicle).unwrap(), chronicle_original);
    assert_eq!(fs::read(&orders).unwrap(), orders_original);
    assert_eq!(fs::read(&chronicle_fixture).unwrap(), chronicle_original);
    assert_eq!(fs::read(&orders_fixture).unwrap(), orders_original);

    app.delete_import_batch(&batch.id).unwrap();
    let project = app.active.as_ref().unwrap().session.path.clone();
    app.close_world().unwrap();
    let mut reopened = NirmataApp::default();
    reopened.open_world(project.clone()).unwrap();
    assert_eq!(
        reopened
            .active
            .as_ref()
            .unwrap()
            .store
            .list_claims()
            .unwrap()
            .len(),
        1
    );
    reopened.undo_last_commit().unwrap();
    reopened.close_world().unwrap();
    reopened.open_world(project).unwrap();
    let store = &reopened.active.as_ref().unwrap().store;
    assert!(store.list_claims().unwrap().is_empty());
    assert!(
        !store
            .list_entities()
            .unwrap()
            .iter()
            .any(|entity| entity.name() == "Mara")
    );
    assert!(
        store
            .list_entities()
            .unwrap()
            .iter()
            .any(|entity| entity.id() == filler.id())
    );
    reopened.close_world().unwrap();
    remove_fixture(root);
}
