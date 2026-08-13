use super::*;
use crate::{ContextIntent, NirmataApp, ai::ClientFuture};
use nirmata_ai::{
    ResponseUsage, StreamDelta,
    capabilities::{CapabilityError, CapabilityInvocation, InvocationMetadata, InvocationStatus},
    contracts::{
        AdvisoryResponse, ContentUri, CritiqueReport, DeepSynthesis, ReferencedMarkdown,
        SpecialistDecisionPosition, SpecialistEvidence, SpecialistFinding, SpecialistReport,
        SynthesisDecisionOrigin, SynthesisOperationOrigin,
    },
};
use nirmata_core::{
    ChangeOperationId, World,
    change_set::{ChangeOperation, ChangeSetDraft, DecisionPoint, RetconKind},
    document::ObjectRef,
    entity::{Entity, EntityKind},
};
use nirmata_store::{ReadScope, WorldStore};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn app_with_world(label: &str) -> (NirmataApp, std::path::PathBuf) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("test directory");
    let path = directory.join(format!(
        "deep-policy-{label}-{}.nirmata",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    let world = World::new("Deep policy", "", "Epoch", 1).expect("world");
    WorldStore::create(&path, &world).expect("store");
    let mut app = NirmataApp::default();
    app.open_world(path.clone()).expect("open world");
    (app, path)
}

fn app_with_entity(label: &str) -> (NirmataApp, std::path::PathBuf, World, Entity) {
    let (mut app, path) = app_with_world(label);
    let world = app
        .get_current_world()
        .expect("session")
        .expect("world")
        .world;
    app.close_world().expect("close to seed");
    let mut store = WorldStore::open(&path).expect("store");
    let entity = Entity::new(
        world.id(),
        EntityKind::Place,
        "Iron Mine",
        "iron-mine",
        "The only iron source.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("entity");
    store.insert_entity(&entity).expect("insert entity");
    drop(store);
    app.open_world(path.clone()).expect("reopen");
    (app, path, world, entity)
}

fn context_for(entity: &Entity) -> ContextBundleRequest {
    let mut context = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);
    context.anchors = vec![ObjectRef::Entity(entity.id())];
    context
}

fn report(
    role: SpecialistRole,
    entity: &Entity,
    finding_id: &str,
    position: Option<(&str, &str)>,
) -> SpecialistReport {
    let source: ContentUri = ObjectRef::Entity(entity.id())
        .to_string()
        .try_into()
        .expect("source uri");
    SpecialistReport {
        specialist: role,
        sources: vec![source],
        findings: vec![SpecialistFinding {
            finding_id: finding_id.to_owned().try_into().expect("finding id"),
            summary: ReferencedMarkdown {
                markdown: format!("Finding from {role:?}"),
                content_references: vec![source],
            },
            affected_object_uris: vec![source],
            candidate_consequences: vec![ReferencedMarkdown {
                markdown: "A traced consequence.".to_owned(),
                content_references: vec![source],
            }],
            assumptions: vec![],
            evidence: vec![SpecialistEvidence {
                source_uri: source,
                excerpt_md: entity.summary().to_owned(),
            }],
            confidence: 0.8,
            unresolved_questions: vec![],
            decision_position: position.map(|(key, alternative)| SpecialistDecisionPosition {
                decision_key: key.to_owned().try_into().expect("decision key"),
                alternative: alternative.to_owned(),
            }),
        }],
    }
}

fn draft(world: &World, entity: &Entity, decision_findings: Option<[&str; 2]>) -> DeepSynthesis {
    let created = Entity::new(
        world.id(),
        EntityKind::Faction,
        "Mine Council",
        "mine-council",
        "A proposed council.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("created entity");
    let operation_id = ChangeOperationId::new();
    let operation = ChangeOperation::CreateEntity {
        operation_id,
        affected_ids: vec![ObjectRef::Entity(created.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: created,
    };
    let decisions = decision_findings
        .map(|_| {
            vec![
                DecisionPoint::new(
                    vec![operation_id],
                    "Who controls the mine?",
                    vec!["Council".to_owned(), "Crown".to_owned()],
                )
                .expect("decision"),
            ]
        })
        .unwrap_or_default();
    let decision_origins = decisions
        .iter()
        .zip(decision_findings)
        .map(|(decision, findings)| SynthesisDecisionOrigin {
            decision_point_id: decision.decision_point_id(),
            finding_ids: findings
                .into_iter()
                .map(|id| id.to_owned().try_into().expect("finding id"))
                .collect(),
        })
        .collect();
    DeepSynthesis {
        draft: ChangeSetDraft::new(
            world.id(),
            world.current_revision(),
            "Create a mine council",
            vec![ObjectRef::Entity(entity.id())],
            vec![],
            vec![operation],
            decisions,
        )
        .expect("draft"),
        operation_origins: vec![SynthesisOperationOrigin {
            operation_id,
            finding_ids: vec!["economic-impact".to_owned().try_into().expect("finding id")],
        }],
        decision_origins,
    }
}

#[derive(Clone)]
enum SpecialistOutcome {
    Report(SpecialistReport, Duration),
    Timeout,
    Failed,
}

#[derive(Clone)]
struct FakeDeepClient {
    outcomes: Arc<Mutex<HashMap<SpecialistRole, SpecialistOutcome>>>,
    synthesis: DeepSynthesis,
    active: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
    specialist_calls: Arc<AtomicUsize>,
    synthesis_calls: Arc<AtomicUsize>,
    critic_calls: Arc<AtomicUsize>,
}

impl FakeDeepClient {
    fn new(outcomes: HashMap<SpecialistRole, SpecialistOutcome>, synthesis: DeepSynthesis) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes)),
            synthesis,
            active: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(0)),
            specialist_calls: Arc::new(AtomicUsize::new(0)),
            synthesis_calls: Arc::new(AtomicUsize::new(0)),
            critic_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

fn metadata(prompt: &str, context_object_ids: Vec<String>) -> InvocationMetadata {
    InvocationMetadata {
        model: "offline-fake".to_owned(),
        prompt_version: prompt.to_owned(),
        context_object_ids,
        status: InvocationStatus::Completed,
        usage: Some(ResponseUsage {
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_tokens: Some(30),
        }),
    }
}

impl AiModeClient for FakeDeepClient {
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
        Box::pin(async { panic!("standard query must not run") })
    }

    fn run_proposal<'a>(
        &'a self,
        _payload: Value,
        _context_object_ids: Vec<String>,
        _options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>> {
        Box::pin(async { panic!("standard generator must not run after synthesis") })
    }

    fn run_critic<'a>(
        &'a self,
        _payload: Value,
        context_object_ids: Vec<String>,
        _options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<CritiqueReport>, CapabilityError>> {
        Box::pin(async move {
            self.critic_calls.fetch_add(1, Ordering::SeqCst);
            Ok(CapabilityInvocation {
                output: CritiqueReport { issues: vec![] },
                metadata: metadata("critic-test", context_object_ids),
            })
        })
    }

    fn run_specialist<'a>(
        &'a self,
        payload: Value,
        context_object_ids: Vec<String>,
        _options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<SpecialistReport>, CapabilityError>> {
        Box::pin(async move {
            assert_eq!(payload["budget"]["maxSpecialistCalls"], MAX_SPECIALISTS);
            assert_eq!(payload["budget"]["maxContextExpansions"], 2);
            assert_eq!(payload["budget"]["maxReadToolCalls"], 6);
            assert_eq!(payload["budget"]["maxNestedDelegations"], 0);
            assert_eq!(payload["budget"]["specialistMaxOutputTokens"], 2_048);
            assert_eq!(payload["budget"]["specialistTimeoutMs"], 30_000);
            let tools = payload["readTools"]
                .as_array()
                .expect("read tool allowlist");
            assert_eq!(tools.len(), 6);
            assert!(tools.iter().all(|tool| {
                tool.as_str()
                    .is_some_and(|name| !name.contains("commit") && !name.contains("delegate"))
            }));
            self.specialist_calls.fetch_add(1, Ordering::SeqCst);
            let role: SpecialistRole =
                serde_json::from_value(payload["specialist"].clone()).expect("payload role");
            let outcome = self
                .outcomes
                .lock()
                .expect("outcomes")
                .get(&role)
                .cloned()
                .expect("role outcome");
            match outcome {
                SpecialistOutcome::Report(report, delay) => {
                    let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                    self.max_active.fetch_max(active, Ordering::SeqCst);
                    tokio::time::sleep(delay).await;
                    self.active.fetch_sub(1, Ordering::SeqCst);
                    Ok(CapabilityInvocation {
                        output: report,
                        metadata: metadata("specialist-test", context_object_ids),
                    })
                }
                SpecialistOutcome::Timeout => Err(CapabilityError::Ai(AiError::RequestTimedOut(
                    SPECIALIST_TIMEOUT,
                ))),
                SpecialistOutcome::Failed => Err(CapabilityError::Ai(AiError::Transport(
                    "offline failure".to_owned(),
                ))),
            }
        })
    }

    fn run_synthesis<'a>(
        &'a self,
        _payload: Value,
        context_object_ids: Vec<String>,
        _options: RequestOptions,
    ) -> ClientFuture<'a, Result<CapabilityInvocation<DeepSynthesis>, CapabilityError>> {
        Box::pin(async move {
            self.synthesis_calls.fetch_add(1, Ordering::SeqCst);
            Ok(CapabilityInvocation {
                output: self.synthesis.clone(),
                metadata: metadata("synthesis-test", context_object_ids),
            })
        })
    }
}

#[test]
fn rule_selection_is_relevant_bounded_and_requires_confirmation() {
    let (app, path) = app_with_world("rules");
    let context = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);
    let plan = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "Analiza la sucesión política tras la guerra y la escasez de recursos",
            None,
            &context,
        )
        .expect("plan");
    assert_eq!(
        plan.roles,
        vec![
            SpecialistRole::Economist,
            SpecialistRole::Historian,
            SpecialistRole::PoliticalScientist
        ]
    );
    assert!(!plan.confirmed);
    assert!(plan.validate_for_execution().is_err());
    assert_eq!(plan.budget, DeepReviewBudget::default());
    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn explicit_selection_rejects_a_fifth_role_and_wrong_mode_roles() {
    let (app, path) = app_with_world("explicit");
    let context = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);
    let error = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "Analiza el impacto",
            Some(vec![
                SpecialistRole::Economist,
                SpecialistRole::Historian,
                SpecialistRole::PoliticalScientist,
                SpecialistRole::Anthropologist,
                SpecialistRole::Geographer,
            ]),
            &context,
        )
        .expect_err("fifth role must fail");
    assert!(error.to_string().contains("cannot exceed 4"));
    assert!(
        app.prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "Analiza el impacto",
            Some(vec![SpecialistRole::TemporalAuditor]),
            &context,
        )
        .is_err()
    );
    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn deep_impact_requires_the_active_head_but_audit_remains_read_only() {
    let (mut app, path) = app_with_world("historical-scope");
    let session = app
        .get_current_world()
        .expect("session")
        .expect("open world");
    let observed = app
        .create_variant("observed", session.current_revision)
        .expect("create observed variant");
    app.set_read_scope(ReadScope::head(observed.id))
        .expect("observe alternate head");
    let context = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);

    assert!(matches!(
        app.prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "Analiza el impacto",
            None,
            &context,
        ),
        Err(AppError::ReadOnlyScope)
    ));
    assert!(
        app.prepare_deep_review(DeepReviewMode::Audit, "Audita el mundo", None, &context)
            .is_ok()
    );

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn only_fixed_read_tools_are_allowed_and_delegation_budget_is_zero() {
    assert!(validate_specialist_tool("get_entity").is_ok());
    assert!(validate_specialist_tool("commit_change_set").is_err());
    assert!(validate_specialist_tool("delegate_agent").is_err());
    assert_eq!(DeepReviewBudget::default().max_nested_delegations, 0);
    assert_eq!(DeepReviewBudget::default().max_read_tool_calls, 6);
    assert_eq!(DeepReviewBudget::default().max_context_expansions, 2);
}

#[test]
fn offline_role_regression_covers_resource_succession_geography_and_audits() {
    let (app, path) = app_with_world("role-matrix");
    let context = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);
    let cases = [
        (
            "crisis de recursos y comercio",
            vec![SpecialistRole::Economist],
        ),
        (
            "guerra de sucesión por el poder",
            vec![
                SpecialistRole::Historian,
                SpecialistRole::PoliticalScientist,
            ],
        ),
        (
            "cambio geográfico de una ruta comercial",
            vec![SpecialistRole::Economist, SpecialistRole::Geographer],
        ),
    ];
    for (request, expected) in cases {
        let plan = app
            .prepare_deep_review(DeepReviewMode::DeepImpact, request, None, &context)
            .expect("rule plan");
        assert_eq!(plan.roles, expected, "{request}");
        assert!(!plan.confirmed);
    }
    let audit = app
        .prepare_deep_review(DeepReviewMode::Audit, "audita el mundo", None, &context)
        .expect("audit plan");
    assert_eq!(
        audit.roles,
        vec![
            SpecialistRole::TemporalAuditor,
            SpecialistRole::RulesAuditor,
            SpecialistRole::CausalAuditor,
            SpecialistRole::PerspectivesAuditor,
        ]
    );
    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn specialists_run_concurrently_and_partial_failure_preserves_successes() {
    let (mut app, path, world, entity) = app_with_entity("partial");
    let synthesis = draft(&world, &entity, None);
    let fake = FakeDeepClient::new(
        HashMap::from([
            (
                SpecialistRole::Economist,
                SpecialistOutcome::Report(
                    report(SpecialistRole::Economist, &entity, "economic-impact", None),
                    Duration::from_millis(30),
                ),
            ),
            (
                SpecialistRole::Historian,
                SpecialistOutcome::Report(
                    report(
                        SpecialistRole::Historian,
                        &entity,
                        "historical-impact",
                        None,
                    ),
                    Duration::from_millis(30),
                ),
            ),
            (
                SpecialistRole::PoliticalScientist,
                SpecialistOutcome::Timeout,
            ),
        ]),
        synthesis,
    );
    let plan = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "resource war succession",
            Some(vec![
                SpecialistRole::Economist,
                SpecialistRole::Historian,
                SpecialistRole::PoliticalScientist,
            ]),
            &context_for(&entity),
        )
        .expect("plan")
        .confirm(vec![
            SpecialistRole::Economist,
            SpecialistRole::Historian,
            SpecialistRole::PoliticalScientist,
        ])
        .expect("confirm");

    let run = app
        .execute_deep_review_with(
            &fake,
            plan,
            &context_for(&entity),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .expect("deep review");
    let deep_run_id = run.id;

    assert_eq!(run.status, DeepReviewStatus::AwaitingReview);
    assert_eq!(
        run.specialists
            .iter()
            .filter(|result| result.status == SpecialistRunStatus::Completed)
            .count(),
        2
    );
    assert!(run.specialists.iter().any(|result| {
        result.role == SpecialistRole::PoliticalScientist
            && result.status == SpecialistRunStatus::TimedOut
    }));
    assert!(fake.max_active.load(Ordering::SeqCst) >= 2);
    assert_eq!(fake.synthesis_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fake.critic_calls.load(Ordering::SeqCst), 1);
    let standard = app
        .read_ai_run(run.standard_run_id.expect("standard run"))
        .expect("standard run exists");
    assert_eq!(standard.status, crate::AiRunStatus::AwaitingReview);
    assert_eq!(standard.base_revision, world.current_revision());

    app.close_world().expect("close world");
    assert!(matches!(
        app.read_deep_review_run(deep_run_id),
        Err(crate::AppError::DeepReviewRunNotFound(_))
    ));

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn total_failure_and_precancelled_runs_never_synthesize() {
    let (mut app, path, world, entity) = app_with_entity("failure");
    let fake = FakeDeepClient::new(
        HashMap::from([
            (SpecialistRole::Economist, SpecialistOutcome::Failed),
            (SpecialistRole::Historian, SpecialistOutcome::Timeout),
        ]),
        draft(&world, &entity, None),
    );
    let roles = vec![SpecialistRole::Economist, SpecialistRole::Historian];
    let plan = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "resource history",
            Some(roles.clone()),
            &context_for(&entity),
        )
        .expect("plan")
        .confirm(roles.clone())
        .expect("confirm");
    let failed = app
        .execute_deep_review_with(
            &fake,
            plan,
            &context_for(&entity),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .expect("failed run remains visible");
    assert_eq!(failed.status, DeepReviewStatus::Failed);
    assert!(failed.synthesis.is_none());
    assert!(failed.standard_run_id.is_none());
    assert_eq!(fake.synthesis_calls.load(Ordering::SeqCst), 0);

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let plan = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "resource history",
            Some(roles.clone()),
            &context_for(&entity),
        )
        .expect("plan")
        .confirm(roles)
        .expect("confirm");
    let calls_before = fake.specialist_calls.load(Ordering::SeqCst);
    let cancelled = app
        .execute_deep_review_with(&fake, plan, &context_for(&entity), cancellation, |_| {})
        .await
        .expect("cancelled run remains visible");
    assert_eq!(cancelled.status, DeepReviewStatus::Cancelled);
    assert_eq!(fake.specialist_calls.load(Ordering::SeqCst), calls_before);
    assert_eq!(fake.synthesis_calls.load(Ordering::SeqCst), 0);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn disagreement_requires_a_sourced_decision_point_before_standard_review() {
    let (mut app, path, world, entity) = app_with_entity("disagreement");
    let outcomes = HashMap::from([
        (
            SpecialistRole::Economist,
            SpecialistOutcome::Report(
                report(
                    SpecialistRole::Economist,
                    &entity,
                    "economic-impact",
                    Some(("mine-control", "Council")),
                ),
                Duration::ZERO,
            ),
        ),
        (
            SpecialistRole::PoliticalScientist,
            SpecialistOutcome::Report(
                report(
                    SpecialistRole::PoliticalScientist,
                    &entity,
                    "political-impact",
                    Some(("mine-control", "Crown")),
                ),
                Duration::ZERO,
            ),
        ),
    ]);
    let roles = vec![
        SpecialistRole::Economist,
        SpecialistRole::PoliticalScientist,
    ];
    let plan = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "resource political control",
            Some(roles.clone()),
            &context_for(&entity),
        )
        .expect("plan")
        .confirm(roles.clone())
        .expect("confirm");
    let silently_resolved = FakeDeepClient::new(outcomes.clone(), draft(&world, &entity, None));
    let rejected = app
        .execute_deep_review_with(
            &silently_resolved,
            plan,
            &context_for(&entity),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .expect("run result");
    assert_eq!(rejected.status, DeepReviewStatus::Failed);
    assert!(
        rejected
            .error
            .as_deref()
            .is_some_and(|error| error.contains("silently resolved"))
    );
    assert!(rejected.standard_run_id.is_none());

    let plan = app
        .prepare_deep_review(
            DeepReviewMode::DeepImpact,
            "resource political control",
            Some(roles.clone()),
            &context_for(&entity),
        )
        .expect("plan")
        .confirm(roles)
        .expect("confirm");
    let preserved = FakeDeepClient::new(
        outcomes,
        draft(
            &world,
            &entity,
            Some(["economic-impact", "political-impact"]),
        ),
    );
    let run = app
        .execute_deep_review_with(
            &preserved,
            plan,
            &context_for(&entity),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .expect("deep review");
    assert_eq!(run.status, DeepReviewStatus::AwaitingReview);
    let standard = app
        .read_ai_run(run.standard_run_id.expect("standard run"))
        .expect("standard run");
    assert_eq!(standard.draft.expect("draft").decisions().len(), 1);
    assert_eq!(standard.status, crate::AiRunStatus::AwaitingReview);
    let review_key = standard.review_key.expect("review key");
    assert!(matches!(
        app.confirm_stored_manual_review(&review_key),
        Err(AppError::InvalidAiRunTransition { .. })
    ));

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[tokio::test]
async fn audit_regression_consolidates_all_read_only_roles_without_a_proposal() {
    let (mut app, path, world, entity) = app_with_entity("audit");
    let roles = vec![
        SpecialistRole::TemporalAuditor,
        SpecialistRole::RulesAuditor,
        SpecialistRole::CausalAuditor,
        SpecialistRole::PerspectivesAuditor,
    ];
    let outcomes = roles
        .iter()
        .copied()
        .enumerate()
        .map(|(index, role)| {
            (
                role,
                SpecialistOutcome::Report(
                    report(role, &entity, &format!("audit-{index}"), None),
                    Duration::ZERO,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let fake = FakeDeepClient::new(outcomes, draft(&world, &entity, None));
    let plan = app
        .prepare_deep_review(
            DeepReviewMode::Audit,
            "audit temporal causal rules perspectives",
            Some(roles.clone()),
            &context_for(&entity),
        )
        .expect("audit plan")
        .confirm(roles)
        .expect("confirm audit roles");
    let run = app
        .execute_deep_review_with(
            &fake,
            plan,
            &context_for(&entity),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .expect("audit run");
    assert_eq!(run.status, DeepReviewStatus::CompletedAudit);
    assert_eq!(run.specialists.len(), 4);
    assert!(run.specialists.iter().all(|result| {
        result.status == SpecialistRunStatus::Completed && result.report.is_some()
    }));
    assert_eq!(
        run.audit_result
            .expect("audit result")
            .validation_report
            .warnings
            .len(),
        4
    );
    assert!(run.synthesis.is_none());
    assert!(run.standard_run_id.is_none());
    assert_eq!(fake.synthesis_calls.load(Ordering::SeqCst), 0);
    assert_eq!(fake.critic_calls.load(Ordering::SeqCst), 0);

    drop(app);
    fs::remove_file(path).expect("remove project");
}
