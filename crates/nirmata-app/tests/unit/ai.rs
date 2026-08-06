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
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>> {
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
