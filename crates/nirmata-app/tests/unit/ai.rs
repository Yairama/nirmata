use super::*;
use crate::{ContextIntent, DraftOperationInput, ManualReviewInput, NirmataApp};
use nirmata_core::{
    ChangeOperationId, Period, World,
    change_set::{ChangeOperation, RetconKind},
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::ObjectRef,
    entity::{Entity, EntityKind},
    goal::{Goal, GoalStatus, GoalVisibility},
    rule::{Rule, RuleKind, RuleSeverity},
};
use nirmata_store::WorldStore;
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
enum FakeProposalReply {
    Draft(ChangeSetDraft),
    Structured(StructuredOutputError),
}

#[derive(Clone)]
enum FakeDocumentReply {
    Draft(InternalDocumentDraft),
    Failure,
}

#[derive(Clone)]
struct FakeClient {
    query_response: AdvisoryResponse,
    query_deltas: Vec<String>,
    query_delay: Duration,
    proposal_replies: Arc<Mutex<VecDeque<FakeProposalReply>>>,
    document_replies: Arc<Mutex<VecDeque<FakeDocumentReply>>>,
    critique_reports: Arc<Mutex<VecDeque<CritiqueReport>>>,
    proposal_delay: Duration,
    document_delay: Duration,
    query_calls: Arc<Mutex<usize>>,
    proposal_calls: Arc<Mutex<usize>>,
    critique_calls: Arc<Mutex<usize>>,
    critique_payloads: Arc<Mutex<Vec<Value>>>,
    proposal_payloads: Arc<Mutex<Vec<Value>>>,
}

impl FakeClient {
    fn new(query_response: AdvisoryResponse, proposal_draft: ChangeSetDraft) -> Self {
        let proposal_replies = VecDeque::from([
            FakeProposalReply::Draft(proposal_draft.clone()),
            FakeProposalReply::Draft(proposal_draft),
        ]);
        Self {
            query_response,
            query_deltas: vec![],
            query_delay: Duration::ZERO,
            proposal_replies: Arc::new(Mutex::new(proposal_replies)),
            document_replies: Arc::new(Mutex::new(VecDeque::new())),
            critique_reports: Arc::new(Mutex::new(VecDeque::from([
                CritiqueReport { issues: vec![] },
                CritiqueReport { issues: vec![] },
            ]))),
            proposal_delay: Duration::ZERO,
            document_delay: Duration::ZERO,
            query_calls: Arc::new(Mutex::new(0)),
            proposal_calls: Arc::new(Mutex::new(0)),
            critique_calls: Arc::new(Mutex::new(0)),
            critique_payloads: Arc::new(Mutex::new(vec![])),
            proposal_payloads: Arc::new(Mutex::new(vec![])),
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

    fn with_proposal_delay(mut self, delay: Duration) -> Self {
        self.proposal_delay = delay;
        self
    }

    fn with_document_replies(mut self, replies: Vec<FakeDocumentReply>) -> Self {
        self.document_replies = Arc::new(Mutex::new(replies.into()));
        self
    }

    fn with_document_delay(mut self, delay: Duration) -> Self {
        self.document_delay = delay;
        self
    }

    fn with_critique_report(mut self, report: CritiqueReport) -> Self {
        self.critique_reports = Arc::new(Mutex::new(VecDeque::from([report])));
        self
    }

    fn with_proposal_replies(mut self, replies: Vec<FakeProposalReply>) -> Self {
        self.proposal_replies = Arc::new(Mutex::new(replies.into()));
        self
    }

    fn with_critique_reports(mut self, reports: Vec<CritiqueReport>) -> Self {
        self.critique_reports = Arc::new(Mutex::new(reports.into()));
        self
    }

    fn proposal_calls(&self) -> usize {
        *self.proposal_calls.lock().expect("proposal calls lock")
    }

    fn critique_calls(&self) -> usize {
        *self.critique_calls.lock().expect("critique calls lock")
    }

    fn last_critique_payload(&self) -> Value {
        self.critique_payloads
            .lock()
            .expect("critique payload lock")
            .last()
            .cloned()
            .expect("captured critique payload")
    }

    fn last_proposal_payload(&self) -> Value {
        self.proposal_payloads
            .lock()
            .expect("proposal payload lock")
            .last()
            .cloned()
            .expect("captured proposal payload")
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
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>> {
        Box::pin(async move {
            *self.proposal_calls.lock().expect("proposal calls lock") += 1;
            self.proposal_payloads
                .lock()
                .expect("proposal payload lock")
                .push(payload);
            sleep_or_cancel(self.proposal_delay, options.cancellation.clone()).await?;
            let reply = self
                .proposal_replies
                .lock()
                .expect("proposal replies lock")
                .pop_front()
                .expect("queued proposal reply");
            match reply {
                FakeProposalReply::Draft(output) => Ok(CapabilityInvocation {
                    output,
                    metadata: test_metadata("proposal_test", context_object_ids),
                }),
                FakeProposalReply::Structured(error) => {
                    Err(CapabilityError::StructuredOutput(error))
                }
            }
        })
    }

    fn run_critic<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<CritiqueReport>, CapabilityError>> {
        Box::pin(async move {
            *self.critique_calls.lock().expect("critique calls lock") += 1;
            self.critique_payloads
                .lock()
                .expect("critique payload lock")
                .push(payload);
            sleep_or_cancel(Duration::ZERO, options.cancellation.clone()).await?;
            let output = self
                .critique_reports
                .lock()
                .expect("critique reports lock")
                .pop_front()
                .expect("queued critique report");
            Ok(CapabilityInvocation {
                output,
                metadata: test_metadata("critic_test", context_object_ids),
            })
        })
    }

    fn run_internal_document<'a>(
        &'a self,
        _payload: Value,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<InternalDocumentDraft>, CapabilityError>>
    {
        Box::pin(async move {
            sleep_or_cancel(self.document_delay, options.cancellation.clone()).await?;
            match self
                .document_replies
                .lock()
                .expect("document replies lock")
                .pop_front()
                .expect("queued document reply")
            {
                FakeDocumentReply::Draft(output) => Ok(CapabilityInvocation {
                    output,
                    metadata: test_metadata("internal_document_test", context_object_ids),
                }),
                FakeDocumentReply::Failure => Err(CapabilityError::Ai(AiError::InvalidResponse(
                    "document generation failed".to_owned(),
                ))),
            }
        })
    }
}

struct SeededWorld {
    mara: Entity,
    sera: Entity,
    rumor: Claim,
    rule: Rule,
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
        sera,
        rumor,
        rule,
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

fn internal_document_draft(
    kind: InternalDocumentKind,
    title: &str,
    body: &str,
    references: Vec<ObjectRef>,
) -> InternalDocumentDraft {
    InternalDocumentDraft {
        document_kind: kind,
        title: title.to_owned(),
        body_markdown: body.to_owned(),
        content_reference_uris: references
            .into_iter()
            .map(|reference| reference.to_string().try_into().expect("valid content URI"))
            .collect(),
    }
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

fn grounded_rule_critique(
    draft: &ChangeSetDraft,
    rule: &Rule,
    severity: ValidationSeverity,
) -> CritiqueReport {
    let rule_uri = format!("nirmata://rule/{}", rule.id())
        .try_into()
        .expect("rule uri");
    CritiqueReport {
        issues: vec![nirmata_ai::contracts::CritiqueIssue {
            issue_id: "rule-conflict".to_owned().try_into().expect("issue id"),
            summary: nirmata_ai::contracts::ReferencedMarkdown {
                markdown: "La propuesta contradice la regla del puerto.".to_owned(),
                content_references: vec![rule_uri],
            },
            affected_operation_ids: vec![draft.operations()[0].operation_id()],
            related_object_uris: vec![rule_uri],
            evidence: vec![nirmata_ai::contracts::CritiqueEvidence {
                source_uri: rule_uri,
                excerpt_md: rule.statement_md().to_owned(),
            }],
            severity,
            category: nirmata_ai::contracts::CritiqueCategory::UniverseRule,
            attack_type: Some(nirmata_ai::contracts::CritiqueAttackType::Rebuts),
            target_claim_id: None,
            confidence: 0.9,
            suggested_resolution: None,
        }],
    }
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
async fn historical_query_keeps_its_scope_and_proposals_are_blocked_before_model_calls() {
    let path = project_path("ai-historical-scope");
    let seeded = seed_world(&path);
    let mut app = open_app(&path);
    let baseline = Entity::new(
        seeded.mara.world_id(),
        EntityKind::Concept,
        "Baseline",
        "baseline",
        "",
        "",
        "{}",
        vec![],
        seeded.mara.updated_at_ms() + 1,
    )
    .expect("baseline entity");
    let baseline_review = app
        .start_manual_review(ManualReviewInput {
            objective: "Capture historical baseline".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: baseline,
            }],
        })
        .expect("baseline review");
    app.confirm_manual_review(&baseline_review)
        .expect("commit baseline");
    let session = app
        .get_current_world()
        .expect("session")
        .expect("open world");
    let historical_revision = session.current_revision;
    let historical_variant = session.active_variant.id;
    let future_mara = Entity::restore(
        seeded.mara.id(),
        seeded.mara.world_id(),
        seeded.mara.kind(),
        "Mara Future",
        "mara-future",
        seeded.mara.summary().to_owned(),
        seeded.mara.body_md().to_owned(),
        seeded.mara.attributes_json().as_str().to_owned(),
        seeded.mara.aliases().to_vec(),
        seeded.mara.version() + 1,
        seeded.mara.created_at_ms(),
        seeded.mara.updated_at_ms() + 1,
    )
    .expect("future entity");
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Advance Mara".to_owned(),
            sources: vec![ObjectRef::Entity(seeded.mara.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: seeded.mara.clone(),
                after: future_mara,
            }],
        })
        .expect("review future rename");
    app.confirm_manual_review(&review)
        .expect("commit future rename");
    app.set_read_scope(ReadScope::historical(
        historical_variant,
        historical_revision,
    ))
    .expect("observe historical revision");

    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let fake = FakeClient::new(
        advisory_response(vec![nirmata_ai::contracts::AdvisoryItem {
            item_id: "historical-1".to_owned().try_into().expect("item id"),
            classification: AdvisoryClassification::Fact,
            answer: nirmata_ai::contracts::ReferencedMarkdown {
                markdown: "Mara en la revisión histórica.".to_owned(),
                content_references: vec![
                    ObjectRef::Entity(seeded.mara.id())
                        .to_string()
                        .try_into()
                        .expect("uri"),
                ],
            },
            citations: vec![nirmata_ai::contracts::AdvisoryCitation {
                source_uri: ObjectRef::Entity(seeded.mara.id())
                    .to_string()
                    .try_into()
                    .expect("uri"),
                quote_md: "Mara histórica".to_owned(),
            }],
        }]),
        draft_for_new_faction(
            &world,
            ObjectRef::Entity(seeded.mara.id()),
            "Guardia Futura",
            "guardia-futura",
        ),
    );
    let context = context_request(ObjectRef::Entity(seeded.mara.id()));
    let response = app
        .execute_ai_query_with(
            &fake,
            "¿Quién es Mara?".to_owned(),
            &context,
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("historical query");
    assert_eq!(
        response.snapshot.read_scope,
        ReadScope::historical(historical_variant, historical_revision)
    );
    assert!(
        response.items[0].citations[0]
            .source
            .snippet
            .contains("Mara")
    );
    assert!(
        !response.items[0].citations[0]
            .source
            .snippet
            .contains("Mara Future")
    );

    let error = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una guardia".to_owned(),
            &context,
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect_err("historical proposal must fail");
    assert!(matches!(error, AppError::ReadOnlyScope));
    assert_eq!(fake.proposal_calls(), 0);

    app.close_world().expect("close world");
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
    let reopened = WorldStore::open(&path).expect("reopen after cancelled query stream");
    assert_eq!(
        reopened
            .load_world()
            .expect("world after cancelled query stream")
            .current_revision(),
        world.current_revision()
    );
    assert_eq!(
        reopened
            .list_revisions()
            .expect("history after cancelled query stream")
            .len(),
        1
    );
    assert_eq!(
        reopened
            .list_entities()
            .expect("canon after cancelled query stream")
            .len(),
        2
    );
    drop(reopened);
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

    let mut progress = Vec::new();
    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una nueva facción que proteja el puerto.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |event| progress.push(event),
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
    assert!(draft_response.critique_report.issues.is_empty());
    assert_eq!(
        draft_response.critique_metadata.prompt_version,
        "critic_test"
    );
    assert_eq!(fake.critique_calls(), 1);
    assert_eq!(fake.proposal_calls(), 1);
    assert_eq!(draft_response.repair_count, 0);
    assert!(draft_response.repair_output_failure.is_none());
    assert!(progress.contains(&AiProposalProgress::CallingCritic));
    assert!(!progress.contains(&AiProposalProgress::Repairing));
    let critique_payload = fake.last_critique_payload();
    assert_eq!(
        critique_payload["draft"],
        serde_json::to_value(&draft).expect("draft json")
    );
    assert!(critique_payload.get("deterministicReport").is_some());
    assert!(
        critique_payload["semanticRules"]
            .as_array()
            .is_some_and(|rules| !rules.is_empty())
    );
    assert!(critique_payload.get("affectedSubgraph").is_some());

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
    assert_eq!(draft_response.repair_count, 1);
    assert_eq!(fake.proposal_calls(), 2);
    assert_eq!(fake.critique_calls(), 2);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn proposal_replaces_an_invalid_draft_with_one_complete_repair() {
    let path = project_path("ai-proposal-repair-validation");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let initial = invalid_additive_delete_draft(&world, &seeded.mara);
    let repaired = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Custodios del Faro",
        "custodios-del-faro",
    );
    let fake =
        FakeClient::new(advisory_response(vec![]), initial.clone()).with_proposal_replies(vec![
            FakeProposalReply::Draft(initial),
            FakeProposalReply::Draft(repaired.clone()),
        ]);
    let app = open_app(&path);

    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea los custodios del faro.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("repair response");
    let AiProposalResponse::Draft(response) = response else {
        panic!("expected repaired draft");
    };

    assert_eq!(response.draft, repaired);
    assert_eq!(response.repair_count, 1);
    assert!(response.ready_for_review);
    assert_eq!(fake.proposal_calls(), 2);
    assert_eq!(fake.critique_calls(), 2);
    let payload = fake.last_proposal_payload();
    assert_eq!(payload["repairReport"]["kind"], "validation_and_critique");
    assert!(payload.get("failedDraft").is_some());
    assert!(
        payload["repairReport"]["deterministicReport"]["errors"]
            .as_array()
            .is_some_and(|issues| issues
                .iter()
                .any(|issue| issue["code"] == "change_set.retcon.additive_delete"))
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn proposal_repairs_structured_output_once_without_raw_payload() {
    let path = project_path("ai-proposal-repair-parsing");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let repaired = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Vigías del Canal",
        "vigias-del-canal",
    );
    let parse_error = nirmata_ai::contracts::parse_change_set_draft("{\"worldId\":")
        .expect_err("truncated output");
    let fake =
        FakeClient::new(advisory_response(vec![]), repaired.clone()).with_proposal_replies(vec![
            FakeProposalReply::Structured(parse_error),
            FakeProposalReply::Draft(repaired.clone()),
        ]);
    let app = open_app(&path);

    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea vigías para el canal.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("parsing repair response");
    let AiProposalResponse::Draft(response) = response else {
        panic!("expected repaired draft");
    };

    assert_eq!(response.draft, repaired);
    assert_eq!(response.repair_count, 1);
    assert_eq!(fake.proposal_calls(), 2);
    assert_eq!(fake.critique_calls(), 1);
    let payload = fake.last_proposal_payload();
    assert_eq!(payload["repairReport"]["kind"], "parsing");
    assert_eq!(payload["repairReport"]["failure"]["kind"], "truncated_json");
    assert!(payload.get("failedDraft").is_none());
    assert!(!payload.to_string().contains("{\"worldId\":"));

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn failed_repair_keeps_the_initial_draft_reviewable_without_a_third_call() {
    let path = project_path("ai-proposal-repair-output-failure");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let initial = invalid_additive_delete_draft(&world, &seeded.mara);
    let parse_error =
        nirmata_ai::contracts::parse_change_set_draft("{").expect_err("truncated repair");
    let fake =
        FakeClient::new(advisory_response(vec![]), initial.clone()).with_proposal_replies(vec![
            FakeProposalReply::Draft(initial.clone()),
            FakeProposalReply::Structured(parse_error),
        ]);
    let app = open_app(&path);

    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una facción para proteger el puerto.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("initial draft remains available");
    let AiProposalResponse::Draft(response) = response else {
        panic!("expected initial draft fallback");
    };

    assert_eq!(response.draft, initial);
    assert_eq!(response.repair_count, 1);
    assert_eq!(
        response.repair_output_failure.expect("repair failure").kind,
        StructuredOutputErrorKind::TruncatedJson
    );
    assert!(!response.ready_for_review);
    assert_eq!(fake.proposal_calls(), 2);
    assert_eq!(fake.critique_calls(), 1);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn two_parsing_failures_stop_after_the_single_repair() {
    let path = project_path("ai-proposal-two-parsing-failures");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let unused = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Unused",
        "unused",
    );
    let first = nirmata_ai::contracts::parse_change_set_draft("{\"objective\":\"PRIVATE_LORE_BODY")
        .expect_err("first truncated output");
    let second = nirmata_ai::contracts::parse_change_set_draft("{\"id\":\"PRIVATE_API_KEY_VALUE")
        .expect_err("second truncated output");
    let fake = FakeClient::new(advisory_response(vec![]), unused).with_proposal_replies(vec![
        FakeProposalReply::Structured(first),
        FakeProposalReply::Structured(second),
    ]);
    let app = open_app(&path);

    let error = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una facción menor.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect_err("second parser failure ends the workflow");

    assert!(matches!(error, AppError::Ai(AiError::InvalidResponse(_))));
    let error_output = error.to_string();
    assert!(!error_output.contains("PRIVATE_LORE_BODY"));
    assert!(!error_output.contains("PRIVATE_API_KEY_VALUE"));
    assert_eq!(fake.proposal_calls(), 2);
    assert_eq!(fake.critique_calls(), 0);

    drop(app);
    let reopened = WorldStore::open(&path).expect("reopen after truncated AI output");
    assert_eq!(
        reopened
            .load_world()
            .expect("world after truncated output")
            .current_revision(),
        world.current_revision()
    );
    assert_eq!(
        reopened
            .list_revisions()
            .expect("history after truncated output")
            .len(),
        1
    );
    assert_eq!(
        reopened
            .list_entities()
            .expect("canon after truncated output")
            .len(),
        2
    );
    drop(reopened);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn proposal_repairs_one_critic_conflict_and_never_loops() {
    let path = project_path("ai-proposal-repair-critic");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let initial = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Guardia del Puerto",
        "guardia-del-puerto",
    );
    let repaired = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Guardia del Dique",
        "guardia-del-dique",
    );
    let first_conflict =
        grounded_rule_critique(&initial, &seeded.rule, ValidationSeverity::Conflict);
    let second_conflict =
        grounded_rule_critique(&repaired, &seeded.rule, ValidationSeverity::Conflict);
    let fake = FakeClient::new(advisory_response(vec![]), initial.clone())
        .with_proposal_replies(vec![
            FakeProposalReply::Draft(initial),
            FakeProposalReply::Draft(repaired.clone()),
        ])
        .with_critique_reports(vec![first_conflict, second_conflict]);
    let app = open_app(&path);

    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una guardia para el puerto.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("bounded critic repair");
    let AiProposalResponse::Draft(response) = response else {
        panic!("expected repaired draft");
    };

    assert_eq!(response.draft, repaired);
    assert_eq!(response.repair_count, 1);
    assert!(!response.ready_for_review);
    assert_eq!(fake.proposal_calls(), 2);
    assert_eq!(fake.critique_calls(), 2);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn proposal_rejects_critique_references_outside_the_draft() {
    let path = project_path("ai-critic-unknown-operation");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let draft = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Guardia del Faro",
        "guardia-del-faro",
    );
    let rule_uri = format!("nirmata://rule/{}", seeded.rule.id())
        .try_into()
        .expect("rule uri");
    let report = CritiqueReport {
        issues: vec![nirmata_ai::contracts::CritiqueIssue {
            issue_id: "unknown-operation".to_owned().try_into().expect("issue id"),
            summary: nirmata_ai::contracts::ReferencedMarkdown {
                markdown: "La propuesta contradice la regla del puerto.".to_owned(),
                content_references: vec![rule_uri],
            },
            affected_operation_ids: vec![ChangeOperationId::new()],
            related_object_uris: vec![rule_uri],
            evidence: vec![nirmata_ai::contracts::CritiqueEvidence {
                source_uri: rule_uri,
                excerpt_md: seeded.rule.statement_md().to_owned(),
            }],
            severity: nirmata_core::validation::ValidationSeverity::Conflict,
            category: nirmata_ai::contracts::CritiqueCategory::UniverseRule,
            attack_type: Some(nirmata_ai::contracts::CritiqueAttackType::Rebuts),
            target_claim_id: None,
            confidence: 0.9,
            suggested_resolution: None,
        }],
    };
    let fake = FakeClient::new(advisory_response(vec![]), draft).with_critique_report(report);
    let app = open_app(&path);

    let error = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una nueva guardia para el faro.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect_err("unknown operation reference must fail");

    assert!(error.to_string().contains("outside the draft"));
    assert_eq!(fake.critique_calls(), 1);
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

#[tokio::test]
async fn ai_run_requires_fresh_final_critique_before_commit_and_persists_summary() {
    let path = project_path("ai-run-review-commit");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let draft = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Vigías del Estuario",
        "vigias-del-estuario",
    );
    let operation_id = draft.operations()[0].operation_id();
    let fake = FakeClient::new(advisory_response(vec![]), draft.clone());
    let context = context_request(ObjectRef::Entity(seeded.mara.id()));
    let mut app = open_app(&path);

    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea una guardia menor para el estuario.".to_owned(),
            &context,
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("proposal response");
    let prepared = app
        .prepare_ai_proposal("Crea una guardia menor para el estuario.", &context)
        .expect("prepare run");
    let run = AiRun::running(prepared.request, prepared.snapshot);
    let run_id = run.id;
    app.ai_runs.insert(run_id, run);
    app.complete_ai_proposal_run(run_id, response)
        .expect("complete run");

    let initial = app.read_ai_run(run_id).expect("read initial run");
    assert_eq!(initial.status, AiRunStatus::AwaitingReview);
    assert_eq!(
        initial.draft.as_ref().expect("run draft").operations()[0].operation_id(),
        operation_id
    );
    let review_key = initial.review_key.expect("review key");
    let error = app
        .confirm_stored_manual_review(&review_key)
        .expect_err("initial critique must not authorize commit");
    assert!(matches!(error, AppError::InvalidAiRunTransition { .. }));
    let error = app
        .revalidate_ai_run_with(run_id, &fake, &context, AiRequestOptions::default(), |_| {})
        .await
        .expect_err("final critique requires a human review action");
    assert!(matches!(error, AppError::InvalidAiRunTransition { .. }));

    app.apply_stored_manual_review_action(
        &review_key,
        crate::ManualReviewActionRequest::Accept {
            operation_id: operation_id.to_string(),
        },
    )
    .expect("record human acceptance");
    assert_eq!(
        app.read_ai_run(run_id).expect("read changed run").status,
        AiRunStatus::AwaitingFinalCritique
    );

    let ready = app
        .revalidate_ai_run_with(run_id, &fake, &context, AiRequestOptions::default(), |_| {})
        .await
        .expect("final critique");
    assert_eq!(ready.status, AiRunStatus::ReadyToCommit);
    assert_eq!(fake.critique_calls(), 2);

    let session = app
        .confirm_stored_manual_review(&review_key)
        .expect("commit reviewed AI run");
    assert_eq!(
        app.read_ai_run(run_id).expect("read committed run").status,
        AiRunStatus::Committed
    );
    drop(app);

    let store = WorldStore::open(&path).expect("reopen store");
    let revision = store
        .get_revision(session.current_revision)
        .expect("load revision")
        .expect("committed revision");
    let record = store
        .get_committed_change_set(revision.change_set_id().expect("change set id"))
        .expect("load change set")
        .expect("committed change set");
    assert_eq!(
        record
            .deterministic_report()
            .and_then(|report| report.get("kind"))
            .and_then(Value::as_str),
        Some("ai_run_summary")
    );
    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn cancelled_ai_run_records_terminal_state_without_changing_canon() {
    let path = project_path("ai-run-cancelled");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let fake = FakeClient::new(
        advisory_response(vec![]),
        draft_for_new_faction(
            &world,
            ObjectRef::Entity(seeded.mara.id()),
            "Guardia Cancelada",
            "guardia-cancelada",
        ),
    )
    .with_proposal_delay(Duration::from_secs(5));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let mut app = open_app(&path);

    let error = app
        .execute_ai_proposal_run_with(
            &fake,
            "Crea una guardia cancelada.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default().with_cancellation(cancellation),
            |_| {},
        )
        .await
        .expect_err("cancelled run");
    assert!(matches!(error, AppError::Ai(AiError::RequestCancelled)));
    let run = app
        .ai_runs
        .values()
        .next()
        .expect("recorded run")
        .snapshot();
    assert_eq!(run.status, AiRunStatus::Cancelled);
    assert!(
        run.error
            .as_deref()
            .is_some_and(|value| value.contains("cancel"))
    );
    let retry_fake = FakeClient::new(
        advisory_response(vec![]),
        draft_for_new_faction(
            &world,
            ObjectRef::Entity(seeded.mara.id()),
            "Guardia Reintentada",
            "guardia-reintentada",
        ),
    );
    let retried = app
        .execute_ai_proposal_run_with(
            &retry_fake,
            "Reintenta crear una guardia.".to_owned(),
            &context_request(ObjectRef::Entity(seeded.mara.id())),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("cancelled proposal can be retried");
    assert_eq!(retried.status, AiRunStatus::AwaitingReview);
    drop(app);

    let store = WorldStore::open(&path).expect("reopen store");
    assert_eq!(
        store.load_world().expect("world").current_revision(),
        world.current_revision()
    );
    assert_eq!(store.list_entities().expect("entities").len(), 2);
    assert_eq!(store.list_revisions().expect("revisions").len(), 1);
    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn final_critic_conflict_requires_judgment_for_that_recorded_issue() {
    let path = project_path("ai-run-final-critic-judgment");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("load world");
    let draft = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Custodios del Faro",
        "custodios-del-faro",
    );
    let operation_id = draft.operations()[0].operation_id();
    let conflict = grounded_rule_critique(&draft, &seeded.rule, ValidationSeverity::Conflict);
    let fake = FakeClient::new(advisory_response(vec![]), draft).with_critique_reports(vec![
        CritiqueReport { issues: vec![] },
        conflict.clone(),
        conflict,
    ]);
    let context = context_request(ObjectRef::Entity(seeded.mara.id()));
    let mut app = open_app(&path);
    let response = app
        .execute_ai_proposal_with(
            &fake,
            "Crea custodios para el faro.".to_owned(),
            &context,
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("initial proposal");
    let prepared = app
        .prepare_ai_proposal("Crea custodios para el faro.", &context)
        .expect("prepare run");
    let run = AiRun::running(prepared.request, prepared.snapshot);
    let run_id = run.id;
    app.ai_runs.insert(run_id, run);
    app.complete_ai_proposal_run(run_id, response)
        .expect("complete run");
    let review_key = app
        .read_ai_run(run_id)
        .expect("run")
        .review_key
        .expect("review key");
    app.apply_stored_manual_review_action(
        &review_key,
        crate::ManualReviewActionRequest::Accept {
            operation_id: operation_id.to_string(),
        },
    )
    .expect("human review action");

    let blocked = app
        .revalidate_ai_run_with(run_id, &fake, &context, AiRequestOptions::default(), |_| {})
        .await
        .expect("blocking final critique");
    assert_eq!(blocked.status, AiRunStatus::AwaitingReview);
    assert_eq!(
        blocked
            .critique_report
            .as_ref()
            .expect("final critique visible")
            .issues[0]
            .issue_id
            .as_str(),
        "rule-conflict"
    );

    let ready = app
        .acknowledge_ai_critique(
            run_id,
            "rule-conflict",
            "Acepto esta excepción semántica para esta propuesta concreta.".to_owned(),
        )
        .expect("acknowledge recorded final critique");
    assert_eq!(ready.status, AiRunStatus::ReadyToCommit);
    let trace = app
        .ai_runs
        .get(&run_id)
        .expect("run")
        .commit_trace(
            &review_key,
            app.manual_reviews
                .get(&review_key)
                .expect("stored review")
                .review
                .draft(),
        )
        .expect("commit trace");
    assert_eq!(
        trace["critiqueAcknowledgements"]["rule-conflict"].as_str(),
        Some("Acepto esta excepción semántica para esta propuesta concreta.")
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn internal_document_is_perspective_scoped_referenced_and_stored_only_for_review() {
    let path = project_path("internal-document-review");
    let seeded = seed_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");
    let world = store.load_world().expect("world");
    let secret = Goal::new(
        world.id(),
        seeded.sera.id(),
        "Sera plans to poison the harbor well.",
        10,
        GoalStatus::Active,
        None,
        GoalVisibility::Secret,
        None,
    )
    .expect("secret goal");
    store.insert_goal(&secret).expect("insert secret goal");
    drop(store);

    let mut app = open_app(&path);
    let request = InternalDocumentRequest {
        instructions: "Record the ships that Mara can see.".to_owned(),
        document_kind: InternalDocumentKind::Chronicle,
        perspective_entity_id: seeded.mara.id(),
        tick: 12,
        anchors: vec![
            ObjectRef::Entity(seeded.mara.id()),
            ObjectRef::Goal(secret.id()),
        ],
    };
    let (prepared, _) = app
        .prepare_internal_document(&request)
        .expect("prepare document context");
    assert!(
        prepared
            .snapshot
            .context
            .contains(ObjectRef::Entity(seeded.mara.id()))
    );
    assert!(
        !prepared
            .snapshot
            .context
            .contains(ObjectRef::Goal(secret.id()))
    );
    assert!(
        !serde_json::to_string(&prepared)
            .expect("serialize input")
            .contains(secret.desired_state_md())
    );

    let fake = FakeClient::new(
        advisory_response(vec![]),
        draft_for_new_faction(
            &world,
            ObjectRef::Entity(seeded.mara.id()),
            "Unused",
            "unused-document",
        ),
    )
    .with_document_replies(vec![FakeDocumentReply::Draft(internal_document_draft(
        InternalDocumentKind::Chronicle,
        "Harbor Chronicle",
        "Mara records the ships visible from the quay.",
        vec![ObjectRef::Entity(seeded.mara.id())],
    ))]);
    let run = app
        .generate_internal_document_with(&fake, request, AiRequestOptions::default(), |_| {})
        .await
        .expect("document reaches standard review");

    assert_eq!(run.status, AiRunStatus::AwaitingReview);
    let draft = run.draft.as_ref().expect("document draft");
    assert_eq!(draft.sources(), &[ObjectRef::Entity(seeded.mara.id())]);
    let ChangeOperation::CreateDocument { after, .. } = &draft.operations()[0] else {
        panic!("expected CreateDocument");
    };
    assert_eq!(after.object().kind(), "chronicle");
    assert_eq!(
        after.object().perspective_entity_id(),
        Some(seeded.mara.id())
    );
    assert_eq!(after.references().len(), 1);
    assert_eq!(
        after.references()[0].target(),
        ObjectRef::Entity(seeded.mara.id())
    );
    let review_key = run.review_key.expect("stored review key");
    assert!(app.manual_reviews.contains_key(&review_key));
    assert!(
        app.active
            .as_ref()
            .expect("active world")
            .store
            .list_documents()
            .expect("documents")
            .is_empty(),
        "review must not write the document"
    );
    assert_eq!(
        app.active
            .as_ref()
            .expect("active world")
            .store
            .load_world()
            .expect("world")
            .current_revision(),
        world.current_revision()
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn internal_document_failure_or_cancellation_creates_no_review_or_document() {
    let path = project_path("internal-document-failure");
    let seeded = seed_world(&path);
    let world = WorldStore::open(&path)
        .expect("open store")
        .load_world()
        .expect("world");
    let mut app = open_app(&path);
    let request = InternalDocumentRequest {
        instructions: "Write a letter.".to_owned(),
        document_kind: InternalDocumentKind::Letter,
        perspective_entity_id: seeded.mara.id(),
        tick: 12,
        anchors: vec![ObjectRef::Entity(seeded.mara.id())],
    };
    let unused = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Unused",
        "unused-failure",
    );
    let outside_context = ObjectRef::Entity(nirmata_core::EntityId::new());
    let ungrounded = FakeClient::new(advisory_response(vec![]), unused.clone())
        .with_document_replies(vec![FakeDocumentReply::Draft(internal_document_draft(
            InternalDocumentKind::Letter,
            "Ungrounded Letter",
            "This reference was not available to Mara.",
            vec![outside_context],
        ))]);
    let error = app
        .generate_internal_document_with(
            &ungrounded,
            request.clone(),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect_err("reference outside context must fail");
    assert!(matches!(error, AppError::InvalidInternalDocument(_)));
    assert!(app.manual_reviews.is_empty());

    let failed = FakeClient::new(advisory_response(vec![]), unused.clone())
        .with_document_replies(vec![FakeDocumentReply::Failure]);
    assert!(
        app.generate_internal_document_with(
            &failed,
            request.clone(),
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .is_err()
    );
    assert!(app.manual_reviews.is_empty());

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let cancelled = FakeClient::new(advisory_response(vec![]), unused)
        .with_document_replies(vec![FakeDocumentReply::Draft(internal_document_draft(
            InternalDocumentKind::Letter,
            "Cancelled Letter",
            "This must never become a draft.",
            vec![ObjectRef::Entity(seeded.mara.id())],
        ))])
        .with_document_delay(Duration::from_secs(1));
    let error = app
        .generate_internal_document_with(
            &cancelled,
            request,
            AiRequestOptions::default().with_cancellation(cancellation),
            |_| {},
        )
        .await
        .expect_err("cancel document generation");
    assert!(matches!(error, AppError::Ai(AiError::RequestCancelled)));
    assert!(app.manual_reviews.is_empty());
    assert!(app.ai_runs.is_empty());
    assert!(
        app.active
            .as_ref()
            .expect("active world")
            .store
            .list_documents()
            .expect("documents")
            .is_empty()
    );
    assert_eq!(
        app.active
            .as_ref()
            .expect("active world")
            .store
            .load_world()
            .expect("world")
            .current_revision(),
        world.current_revision()
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn narrative_continuity_is_read_only_then_preserves_alternatives_and_sources_in_standard_review()
 {
    let path = project_path("narrative-continuity");
    let seeded = seed_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");
    let world = store.load_world().expect("world");
    let goal = Goal::new(
        world.id(),
        seeded.mara.id(),
        "Recover the missing harbor ledger.",
        8,
        GoalStatus::Active,
        None,
        GoalVisibility::Public,
        None,
    )
    .expect("active goal");
    store.insert_goal(&goal).expect("insert goal");
    drop(store);
    let mut app = open_app(&path);
    let selection = crate::NarrativeContinuitySelection::LooseEnd {
        code: "active_goal_without_resolution".to_owned(),
        object_ref: ObjectRef::Goal(goal.id()),
    };

    let exploration = app
        .explore_narrative_continuity(None, selection.clone())
        .expect("continuity exploration");
    assert!(!exploration.question.is_empty());
    assert_eq!(exploration.alternatives.len(), 3);
    assert!(
        exploration
            .source_uris
            .contains(&ObjectRef::Goal(goal.id()).to_string())
    );
    assert_eq!(
        app.active
            .as_ref()
            .expect("active world")
            .store
            .load_world()
            .expect("world")
            .current_revision(),
        world.current_revision(),
        "exploration must be read-only"
    );

    let generated = draft_for_new_faction(
        &world,
        ObjectRef::Entity(seeded.mara.id()),
        "Ledger Seekers",
        "ledger-seekers",
    );
    let fake = FakeClient::new(advisory_response(vec![]), generated);
    let proposal = app
        .propose_narrative_continuity_with(
            &fake,
            None,
            selection,
            "complicate_goal",
            AiRequestOptions::default(),
            |_| {},
        )
        .await
        .expect("continuity proposal");

    assert_eq!(proposal.run.status, AiRunStatus::AwaitingReview);
    assert_eq!(proposal.exploration.alternatives.len(), 3);
    assert!(
        proposal
            .intent_brief
            .restrictions
            .iter()
            .any(|restriction| restriction.contains("DecisionPoint"))
    );
    let draft = proposal.run.draft.as_ref().expect("review draft");
    assert!(draft.sources().contains(&ObjectRef::Goal(goal.id())));
    let decision = draft.decisions().last().expect("continuity decision");
    assert_eq!(decision.alternatives().len(), 3);
    assert_eq!(
        decision.resolved_alternative(),
        Some("Complicar el objetivo")
    );
    let critique_payload = fake.last_critique_payload();
    let serialized_goal =
        serde_json::to_value(ObjectRef::Goal(goal.id())).expect("serialize goal source");
    assert!(
        critique_payload["draft"]["sources"]
            .as_array()
            .expect("draft sources")
            .iter()
            .any(|source| source == &serialized_goal)
    );
    let review = app
        .read_stored_manual_review(
            proposal
                .run
                .review_key
                .as_deref()
                .expect("standard review key"),
        )
        .expect("stored standard review");
    assert_eq!(review.operations[0].decision_points.len(), 1);
    assert_eq!(
        review.operations[0].decision_points[0].alternatives.len(),
        3
    );
    assert_eq!(
        app.active
            .as_ref()
            .expect("active world")
            .store
            .load_world()
            .expect("world")
            .current_revision(),
        world.current_revision(),
        "proposal review must not write canon"
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}
