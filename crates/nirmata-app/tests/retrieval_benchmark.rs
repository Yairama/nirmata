use nirmata_app::{
    ContextBudget, ContextBundleRequest, ContextIntent, ContextStage, EmptySearchClassification,
    NirmataApp, RelatedContextRequest, SearchClassification, SearchWorldRequest,
};
use nirmata_core::{
    Period, World,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{
    DocumentAggregate, EventAggregate, StructuredSearchKind, StructuredSearchQuery,
    StructuredSearchStage, StructuredSearchTemporal, WorldStore,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

fn open_app(path: &Path) -> NirmataApp {
    let mut app = NirmataApp::default();
    app.open_world(path.to_path_buf()).expect("open world");
    app
}

fn base_world(path: &Path) -> World {
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    WorldStore::create(path, &world).expect("create store");
    world
}

struct BenchmarkFixture {
    vaultglass: ObjectRef,
    vaultglass_registry: ObjectRef,
    memory_ore_memorandum: ObjectRef,
    caldris_mine: ObjectRef,
    collapse: ObjectRef,
    sabotage: ObjectRef,
    rationing: ObjectRef,
    festival: ObjectRef,
    sera_claim: ObjectRef,
    orun_claim: ObjectRef,
    sera_journal: ObjectRef,
    orun_dispatch: ObjectRef,
    outsider_goal: ObjectRef,
    supply_goal: ObjectRef,
    mine_relation: ObjectRef,
    world_rule: ObjectRef,
    empire: ObjectRef,
    sera_id: nirmata_core::EntityId,
    labels: BTreeMap<ObjectRef, String>,
}

struct BenchmarkExpectation {
    question: &'static str,
    required_sources: Vec<ObjectRef>,
    irrelevant_sources: Vec<ObjectRef>,
    omitted_contradictions: Vec<ObjectRef>,
}

fn build_fixture(path: &Path) -> BenchmarkFixture {
    let world = base_world(path);
    let mut store = WorldStore::open(path).expect("open store");

    let caldris = Entity::new(
        world.id(),
        EntityKind::Place,
        "Caldris",
        "caldris",
        "Ciudad minera del imperio.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("caldris");
    let caldris_mine = Entity::new(
        world.id(),
        EntityKind::Place,
        "Caldris Mine",
        "caldris-mine",
        "Main shaft under the bay.",
        "",
        "{}",
        vec!["Deepworks".to_owned()],
        1,
    )
    .expect("mine");
    let empire = Entity::new(
        world.id(),
        EntityKind::Faction,
        "Amber Empire",
        "amber-empire",
        "Imperial authority over Caldris.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("empire");
    let glass_rite = Entity::new(
        world.id(),
        EntityKind::Culture,
        "Glass Rite",
        "glass-rite",
        "Religion that keeps ancestral memory vaults.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("religion");
    let vaultglass = Entity::new(
        world.id(),
        EntityKind::Resource,
        "Vaultglass",
        "vaultglass",
        "Mineral that stores ancestral memories.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("vaultglass");
    let sera = Entity::new(
        world.id(),
        EntityKind::Person,
        "Sera",
        "sera",
        "Mine surveyor.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("sera");
    let orun = Entity::new(
        world.id(),
        EntityKind::Person,
        "Orun",
        "orun",
        "Imperial foreman.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("orun");
    let iven = Entity::new(
        world.id(),
        EntityKind::Person,
        "Iven",
        "iven",
        "Harbor clerk.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("iven");
    for entity in [
        &caldris,
        &caldris_mine,
        &empire,
        &glass_rite,
        &vaultglass,
        &sera,
        &orun,
        &iven,
    ] {
        store.insert_entity(entity).expect("insert entity");
    }

    let mine_relation = Relation::new(
        world.id(),
        empire.id(),
        caldris_mine.id(),
        "controls",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        Some("imperial charter".to_owned()),
        "{}",
    )
    .expect("mine relation");
    store
        .insert_relation(&mine_relation)
        .expect("insert mine relation");

    let world_rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Imperial mines must preserve a public accident ledger.",
        "place",
        RuleSeverity::Advisory,
        Some("mining code".to_owned()),
        None,
        "{}",
        1,
    )
    .expect("world rule");
    store.insert_rule(&world_rule).expect("insert world rule");

    let vaultglass_registry = Document::new(
        world.id(),
        "Vaultglass Registry",
        "ledger",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "Vaultglass stores ancestral memories in sealed shards.",
        1,
    )
    .expect("registry");
    let memory_ore_memorandum = Document::new(
        world.id(),
        "Memory Ore Memorandum",
        "report",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "The memory ore stores ancestral memories with lower yield.",
        1,
    )
    .expect("memorandum");
    let sera_journal = Document::new(
        world.id(),
        "Sera's Journal",
        "chronicle",
        Some(sera.id()),
        Some(sera.id()),
        DocumentCanonStatus::NonCanonical,
        "Sera says imperial powder charges cracked the lower galleries.",
        1,
    )
    .expect("sera journal");
    let orun_dispatch = Document::new(
        world.id(),
        "Orun's Dispatch",
        "dispatch",
        Some(orun.id()),
        Some(orun.id()),
        DocumentCanonStatus::NonCanonical,
        "Orun insists the collapse came from old supports, not sabotage.",
        1,
    )
    .expect("orun dispatch");
    for document in [
        &vaultglass_registry,
        &memory_ore_memorandum,
        &sera_journal,
        &orun_dispatch,
    ] {
        store
            .insert_document(&DocumentAggregate::new(document.clone(), vec![]))
            .expect("insert document");
    }

    let sera_claim = Claim::new(
        world.id(),
        caldris_mine.id(),
        "Sera says imperial powder charges caused the collapse.",
        Some("collapse.cause.sabotage".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(sera.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        Some("survey notes".to_owned()),
        Some("journal".to_owned()),
        Some(sera_journal.id()),
        None,
        Some(0.7),
        Some(Period::new(Some(20), Some(20)).expect("sera period")),
        world.current_revision(),
    )
    .expect("sera claim");
    let orun_claim = Claim::new(
        world.id(),
        caldris_mine.id(),
        "Orun insists the collapse was an accident, not sabotage.",
        Some("collapse.cause.sabotage".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Negative,
        ClaimAuthentication::Attributed,
        Some(orun.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        Some("foreman report".to_owned()),
        Some("dispatch".to_owned()),
        Some(orun_dispatch.id()),
        None,
        Some(0.6),
        Some(Period::new(Some(20), Some(20)).expect("orun period")),
        world.current_revision(),
    )
    .expect("orun claim");
    for claim in [&sera_claim, &orun_claim] {
        store.insert_claim(claim).expect("insert claim");
    }

    let supply_goal = Goal::new(
        world.id(),
        empire.id(),
        "Restore the vaultglass supply.",
        9,
        GoalStatus::Active,
        Some(Period::new(Some(18), Some(30)).expect("supply period")),
        GoalVisibility::Public,
        Some("imperial directive".to_owned()),
    )
    .expect("supply goal");
    store.insert_goal(&supply_goal).expect("insert supply goal");

    let sabotage = Event::new(
        world.id(),
        "sabotage",
        "Powder charges rupture the east shaft.",
        "",
        EventTime::instant(19, TimePrecision::Exact, Certainty::Certain),
        Some(caldris_mine.id()),
        vec![EventParticipant::new(empire.id(), "occupier", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("sabotage");
    let collapse = Event::new(
        world.id(),
        "collapse",
        "The Caldris Mine collapses.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        Some(caldris_mine.id()),
        vec![
            EventParticipant::new(empire.id(), "authority", 0).expect("participant"),
            EventParticipant::new(glass_rite.id(), "custodian", 1).expect("participant"),
        ],
        vec![supply_goal.id()],
        1,
    )
    .expect("collapse");
    let rationing = Event::new(
        world.id(),
        "rationing",
        "Caldris rations memory shards after the collapse.",
        "",
        EventTime::instant(21, TimePrecision::Exact, Certainty::Certain),
        Some(caldris_mine.id()),
        vec![EventParticipant::new(glass_rite.id(), "custodian", 0).expect("participant")],
        vec![supply_goal.id()],
        1,
    )
    .expect("rationing");
    let festival = Event::new(
        world.id(),
        "festival",
        "The harbor opens an unrelated lantern festival.",
        "",
        EventTime::instant(21, TimePrecision::Exact, Certainty::Certain),
        Some(caldris.id()),
        vec![EventParticipant::new(iven.id(), "host", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("festival");
    store
        .insert_event(&EventAggregate::new(rationing.clone(), vec![]))
        .expect("insert rationing");
    store
        .insert_event(&EventAggregate::new(
            collapse.clone(),
            vec![
                EventLink::new(collapse.id(), rationing.id(), EventLinkKind::Causes)
                    .expect("collapse link"),
            ],
        ))
        .expect("insert collapse");
    store
        .insert_event(&EventAggregate::new(
            sabotage.clone(),
            vec![
                EventLink::new(sabotage.id(), collapse.id(), EventLinkKind::Causes)
                    .expect("sabotage link"),
            ],
        ))
        .expect("insert sabotage");
    store
        .insert_event(&EventAggregate::new(festival.clone(), vec![]))
        .expect("insert festival");

    let outsider_goal = Goal::new(
        world.id(),
        iven.id(),
        "Keep the harbor tariffs unchanged.",
        5,
        GoalStatus::Active,
        Some(Period::new(Some(10), Some(30)).expect("goal period")),
        GoalVisibility::Public,
        Some("civic agenda".to_owned()),
    )
    .expect("goal");
    store.insert_goal(&outsider_goal).expect("insert goal");

    for index in 0..20 {
        let document = Document::new(
            world.id(),
            format!("Background Dossier {index:02}"),
            "dossier",
            None,
            None,
            DocumentCanonStatus::Canonical,
            format!("Cobalt harbor permit record number {index:02}."),
            1,
        )
        .expect("background document");
        store
            .insert_document(&DocumentAggregate::new(document, vec![]))
            .expect("insert background document");
    }

    drop(store);

    let labels = BTreeMap::from([
        (ObjectRef::Entity(caldris.id()), "caldris".to_owned()),
        (
            ObjectRef::Entity(caldris_mine.id()),
            "caldris-mine".to_owned(),
        ),
        (ObjectRef::Entity(empire.id()), "amber-empire".to_owned()),
        (ObjectRef::Entity(glass_rite.id()), "glass-rite".to_owned()),
        (ObjectRef::Entity(vaultglass.id()), "vaultglass".to_owned()),
        (ObjectRef::Entity(sera.id()), "sera".to_owned()),
        (ObjectRef::Entity(orun.id()), "orun".to_owned()),
        (ObjectRef::Entity(iven.id()), "iven".to_owned()),
        (
            ObjectRef::Relation(mine_relation.id()),
            "empire-controls-mine".to_owned(),
        ),
        (
            ObjectRef::Rule(world_rule.id()),
            "mine-ledger-rule".to_owned(),
        ),
        (
            ObjectRef::Document(vaultglass_registry.id()),
            "vaultglass-registry".to_owned(),
        ),
        (
            ObjectRef::Document(memory_ore_memorandum.id()),
            "memory-ore-memorandum".to_owned(),
        ),
        (
            ObjectRef::Document(sera_journal.id()),
            "sera-journal".to_owned(),
        ),
        (
            ObjectRef::Document(orun_dispatch.id()),
            "orun-dispatch".to_owned(),
        ),
        (ObjectRef::Claim(sera_claim.id()), "sera-claim".to_owned()),
        (ObjectRef::Claim(orun_claim.id()), "orun-claim".to_owned()),
        (ObjectRef::Event(sabotage.id()), "sabotage".to_owned()),
        (ObjectRef::Event(collapse.id()), "collapse".to_owned()),
        (ObjectRef::Event(rationing.id()), "rationing".to_owned()),
        (ObjectRef::Event(festival.id()), "festival".to_owned()),
        (
            ObjectRef::Goal(supply_goal.id()),
            "restore-supply-goal".to_owned(),
        ),
        (
            ObjectRef::Goal(outsider_goal.id()),
            "harbor-tariff-goal".to_owned(),
        ),
    ]);

    BenchmarkFixture {
        vaultglass: ObjectRef::Entity(vaultglass.id()),
        vaultglass_registry: ObjectRef::Document(vaultglass_registry.id()),
        memory_ore_memorandum: ObjectRef::Document(memory_ore_memorandum.id()),
        caldris_mine: ObjectRef::Entity(caldris_mine.id()),
        collapse: ObjectRef::Event(collapse.id()),
        sabotage: ObjectRef::Event(sabotage.id()),
        rationing: ObjectRef::Event(rationing.id()),
        festival: ObjectRef::Event(festival.id()),
        sera_claim: ObjectRef::Claim(sera_claim.id()),
        orun_claim: ObjectRef::Claim(orun_claim.id()),
        sera_journal: ObjectRef::Document(sera_journal.id()),
        orun_dispatch: ObjectRef::Document(orun_dispatch.id()),
        outsider_goal: ObjectRef::Goal(outsider_goal.id()),
        supply_goal: ObjectRef::Goal(supply_goal.id()),
        mine_relation: ObjectRef::Relation(mine_relation.id()),
        world_rule: ObjectRef::Rule(world_rule.id()),
        empire: ObjectRef::Entity(empire.id()),
        sera_id: sera.id(),
        labels,
    }
}

fn assert_sources(expectation: &BenchmarkExpectation, returned: &[ObjectRef]) {
    let returned = returned.iter().copied().collect::<BTreeSet<_>>();
    for source in &expectation.required_sources {
        assert!(
            returned.contains(source),
            "{} should recover required source {source}",
            expectation.question
        );
    }
    for source in &expectation.irrelevant_sources {
        assert!(
            !returned.contains(source),
            "{} should exclude irrelevant source {source}",
            expectation.question
        );
    }
    for source in &expectation.omitted_contradictions {
        assert!(
            !returned.contains(source),
            "{} should omit incompatible contradiction {source}",
            expectation.question
        );
    }
}

#[test]
fn retrieval_benchmark_exact_vocab_fts_hits_required_sources_within_limit() {
    let path = project_path("retrieval-benchmark-fts");
    let fixture = build_fixture(&path);
    let app = open_app(&path);

    let expectation = BenchmarkExpectation {
        question: "¿Qué fuentes canonicas usan exactamente el vocabulario 'Vaultglass'?",
        required_sources: vec![fixture.vaultglass, fixture.vaultglass_registry],
        irrelevant_sources: vec![fixture.memory_ore_memorandum],
        omitted_contradictions: vec![],
    };
    let response = app
        .search_world(&SearchWorldRequest::new(StructuredSearchQuery {
            text: Some("Vaultglass".to_owned()),
            limit: 2,
            ..Default::default()
        }))
        .expect("search exact vocabulary");
    let returned = response
        .hits
        .iter()
        .map(|hit| hit.object_ref)
        .collect::<Vec<_>>();

    assert!(response.absence.is_none());
    assert_eq!(response.hits.len(), 2);
    assert!(response.hits.iter().all(|hit| hit.provenance == "fts5"));
    assert!(
        response
            .hits
            .iter()
            .all(|hit| hit.classification == SearchClassification::Fact)
    );
    assert_sources(&expectation, &returned);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn retrieval_benchmark_perspective_query_keeps_incompatible_holders_out() {
    let path = project_path("retrieval-benchmark-perspective");
    let fixture = build_fixture(&path);
    let app = open_app(&path);

    let expectation = BenchmarkExpectation {
        question: "¿Que sostiene Sera sobre el derrumbe de la mina?",
        required_sources: vec![fixture.sera_claim, fixture.sera_journal],
        irrelevant_sources: vec![fixture.memory_ore_memorandum],
        omitted_contradictions: vec![fixture.orun_claim, fixture.orun_dispatch],
    };
    let response = app
        .get_related_context(&RelatedContextRequest {
            bundle: ContextBundleRequest {
                intent: ContextIntent::ContradictionCheck,
                anchors: vec![fixture.caldris_mine],
                query_text: None,
                temporal: None,
                temporal_radius: None,
                perspective_entity_ids: vec![fixture.sera_id],
                include_perspectives: true,
                relation_limit: 4,
                budget: ContextBudget {
                    max_objects: 30,
                    max_chars: 4_000,
                },
            },
            kinds: vec![StructuredSearchKind::Claim, StructuredSearchKind::Document],
            empty: EmptySearchClassification::NoEvidence,
        })
        .expect("load holder context");
    let returned = response
        .all_entries()
        .into_iter()
        .map(|entry| entry.result.object_ref)
        .collect::<Vec<_>>();

    assert!(response.absence.is_none());
    assert!(response.canon.is_empty());
    assert!(
        response
            .perspectives
            .iter()
            .all(|entry| entry.result.classification == SearchClassification::Perspective)
    );
    assert!(response.usage.used_objects <= response.usage.max_objects);
    assert!(response.usage.used_chars <= response.usage.max_chars);
    assert_sources(&expectation, &returned);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn retrieval_benchmark_causal_context_keeps_required_events_within_budget() {
    let path = project_path("retrieval-benchmark-causality");
    let fixture = build_fixture(&path);
    let app = open_app(&path);

    let expectation = BenchmarkExpectation {
        question: "¿Que evento causal explica el derrumbe y su consecuencia inmediata?",
        required_sources: vec![fixture.collapse, fixture.sabotage, fixture.rationing],
        irrelevant_sources: vec![fixture.festival],
        omitted_contradictions: vec![fixture.sera_claim, fixture.orun_claim],
    };
    let response = app
        .get_related_context(&RelatedContextRequest {
            bundle: ContextBundleRequest {
                intent: ContextIntent::ImpactAnalysis,
                anchors: vec![fixture.collapse],
                query_text: None,
                temporal: None,
                temporal_radius: Some(2),
                perspective_entity_ids: vec![],
                include_perspectives: false,
                relation_limit: 2,
                budget: ContextBudget {
                    max_objects: 8,
                    max_chars: 360,
                },
            },
            kinds: vec![StructuredSearchKind::Event],
            empty: EmptySearchClassification::NoEvidence,
        })
        .expect("load causal context");
    let returned = response
        .all_entries()
        .into_iter()
        .map(|entry| entry.result.object_ref)
        .collect::<Vec<_>>();

    assert!(response.absence.is_none());
    assert!(response.perspectives.is_empty());
    assert!(response.desires.is_empty());
    assert!(response.obligations.is_empty());
    assert!(response.search_evidence.is_empty());
    assert!(response.usage.used_objects <= response.usage.max_objects);
    assert!(response.usage.used_chars <= response.usage.max_chars);
    assert!(
        response
            .canon
            .iter()
            .all(|entry| entry.result.classification == SearchClassification::Fact)
    );
    assert_sources(&expectation, &returned);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn retrieval_benchmark_marks_unspecified_data_without_backfilling_irrelevant_goals() {
    let path = project_path("retrieval-benchmark-unspecified");
    let fixture = build_fixture(&path);
    let app = open_app(&path);

    let expectation = BenchmarkExpectation {
        question: "¿Que objetivo conocido motivo el derrumbe?",
        required_sources: vec![],
        irrelevant_sources: vec![fixture.outsider_goal],
        omitted_contradictions: vec![],
    };
    let response = app
        .get_related_context(&RelatedContextRequest {
            bundle: ContextBundleRequest {
                intent: ContextIntent::ImpactAnalysis,
                anchors: vec![fixture.collapse],
                query_text: None,
                temporal: None,
                temporal_radius: Some(1),
                perspective_entity_ids: vec![],
                include_perspectives: false,
                relation_limit: 2,
                budget: ContextBudget {
                    max_objects: 3,
                    max_chars: 180,
                },
            },
            kinds: vec![StructuredSearchKind::Goal],
            empty: EmptySearchClassification::Unspecified,
        })
        .expect("load unspecified goal context");
    let returned = response
        .all_entries()
        .into_iter()
        .map(|entry| entry.result.object_ref)
        .collect::<Vec<_>>();

    assert!(response.all_entries().is_empty());
    assert_eq!(
        response.absence,
        Some(nirmata_app::SearchAbsence {
            classification: SearchClassification::Unspecified,
            provenance: "get_related_context".to_owned(),
        })
    );
    assert!(response.usage.used_objects <= response.usage.max_objects);
    assert!(response.usage.used_chars <= response.usage.max_chars);
    assert_sources(&expectation, &returned);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[derive(Debug, Deserialize)]
struct RetrievalCorpus {
    version: String,
    recall_target_bps: u32,
    cited_precision_floor_bps: u32,
    latency_budget_ms: u64,
    minimum_affected_paraphrase_queries: usize,
    distractor_documents: usize,
    lexical_cases: Vec<LexicalCase>,
}

#[derive(Debug, Deserialize)]
struct LexicalCase {
    id: String,
    title: String,
    body: String,
    exact_query: String,
    paraphrase_query: String,
}

struct LexicalFixture {
    sources: BTreeMap<String, ObjectRef>,
    labels: BTreeMap<ObjectRef, String>,
}

#[derive(Clone, Copy)]
enum BenchmarkWorld {
    Structural,
    Lexical,
}

#[derive(Clone)]
enum BenchmarkRequest {
    Context(RelatedContextRequest),
    Structured(StructuredSearchQuery),
}

#[derive(Clone)]
struct QuerySpec {
    id: String,
    family: &'static str,
    question: String,
    world: BenchmarkWorld,
    request: BenchmarkRequest,
    required_sources: Vec<ObjectRef>,
    contradiction_sources: Vec<ObjectRef>,
    expected_category: &'static str,
    expected_recall_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetrievedSource {
    object: ObjectRef,
    category: &'static str,
    provenance: String,
    citation_chars: usize,
}

struct QueryMeasurement {
    spec: QuerySpec,
    retrieved: Vec<RetrievedSource>,
    irrelevant: Vec<ObjectRef>,
    recall_bps: u32,
    cited_precision_bps: Option<u32>,
    contradiction_preserved: Option<bool>,
    citation_chars: usize,
    latency_p50: Duration,
    latency_p95: Duration,
    latency_samples: Vec<Duration>,
}

fn load_retrieval_corpus() -> RetrievalCorpus {
    serde_json::from_str(include_str!("fixtures/retrieval_benchmark.json"))
        .expect("valid retrieval benchmark corpus")
}

fn build_lexical_fixture(path: &Path, corpus: &RetrievalCorpus) -> LexicalFixture {
    let world = World::new(
        "Lexical Marches",
        "Controlled vocabulary benchmark",
        "Archive Year",
        1,
    )
    .expect("lexical world");
    WorldStore::create(path, &world).expect("create lexical store");
    let mut store = WorldStore::open(path).expect("open lexical store");
    let mut sources = BTreeMap::new();
    let mut labels = BTreeMap::new();

    for case in &corpus.lexical_cases {
        let document = Document::new(
            world.id(),
            case.title.clone(),
            "benchmark_source",
            None,
            None,
            DocumentCanonStatus::Canonical,
            case.body.clone(),
            1,
        )
        .expect("lexical source document");
        let object = ObjectRef::Document(document.id());
        store
            .insert_document(&DocumentAggregate::new(document, vec![]))
            .expect("insert lexical source document");
        sources.insert(case.id.clone(), object);
        labels.insert(object, format!("{}.source", case.id));
    }

    for index in 0..corpus.distractor_documents {
        let document = Document::new(
            world.id(),
            format!("Administrative Codex {index:02}"),
            "distractor",
            None,
            None,
            DocumentCanonStatus::Canonical,
            format!("Basalt measurements and copper permits, volume {index:02}."),
            1,
        )
        .expect("lexical distractor");
        store
            .insert_document(&DocumentAggregate::new(document, vec![]))
            .expect("insert lexical distractor");
    }

    LexicalFixture { sources, labels }
}

fn expanded_query_specs(
    fixture: &BenchmarkFixture,
    lexical: &LexicalFixture,
    corpus: &RetrievalCorpus,
) -> Vec<QuerySpec> {
    let mut specs = vec![
        QuerySpec {
            id: "anchor-01".to_owned(),
            family: "explicit_anchor",
            question: "Open the explicitly selected Caldris mine.".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Context(RelatedContextRequest {
                bundle: ContextBundleRequest {
                    intent: ContextIntent::EntityQuery,
                    anchors: vec![fixture.caldris_mine],
                    query_text: None,
                    temporal: None,
                    temporal_radius: None,
                    perspective_entity_ids: vec![],
                    include_perspectives: false,
                    relation_limit: 0,
                    budget: ContextBudget {
                        max_objects: 1,
                        max_chars: 200,
                    },
                },
                kinds: vec![StructuredSearchKind::Entity],
                empty: EmptySearchClassification::NoEvidence,
            }),
            required_sources: vec![fixture.caldris_mine],
            contradiction_sources: vec![],
            expected_category: "explicit_anchor",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "sql-type-01".to_owned(),
            family: "structured_sql",
            question: "List the applicable rule in this controlled world.".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Rule],
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.world_rule],
            contradiction_sources: vec![],
            expected_category: "structured_sql",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "sql-alias-01".to_owned(),
            family: "structured_sql",
            question: "Which entity has the explicit alias Deepworks?".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                alias: Some("Deepworks".to_owned()),
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.caldris_mine],
            contradiction_sources: vec![],
            expected_category: "structured_sql",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "relations-01".to_owned(),
            family: "relations",
            question: "What direct relation connects the empire to the mine?".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Context(RelatedContextRequest {
                bundle: ContextBundleRequest {
                    intent: ContextIntent::EntityQuery,
                    anchors: vec![fixture.empire],
                    query_text: None,
                    temporal: None,
                    temporal_radius: None,
                    perspective_entity_ids: vec![],
                    include_perspectives: false,
                    relation_limit: 4,
                    budget: ContextBudget {
                        max_objects: 12,
                        max_chars: 1_200,
                    },
                },
                kinds: vec![StructuredSearchKind::Relation],
                empty: EmptySearchClassification::NoEvidence,
            }),
            required_sources: vec![fixture.mine_relation],
            contradiction_sources: vec![],
            expected_category: "relations",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "relations-02".to_owned(),
            family: "relations",
            question: "Which events causally precede and follow the collapse?".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Event],
                neighbors_of: vec![fixture.collapse],
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.sabotage, fixture.rationing],
            contradiction_sources: vec![],
            expected_category: "relations",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "time-01".to_owned(),
            family: "time",
            question: "What event occurs at tick 19?".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Event],
                temporal: Some(StructuredSearchTemporal::Tick(19)),
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.sabotage],
            contradiction_sources: vec![],
            expected_category: "time",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "goals-01".to_owned(),
            family: "goals",
            question: "Which goal and events track restoration of supply?".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Goal, StructuredSearchKind::Event],
                goal_ids: vec![match fixture.supply_goal {
                    ObjectRef::Goal(id) => id,
                    _ => unreachable!("fixture goal"),
                }],
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.supply_goal, fixture.collapse, fixture.rationing],
            contradiction_sources: vec![],
            expected_category: "goals",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "perspectives-01".to_owned(),
            family: "perspectives",
            question: "What does Sera's perspective assert and cite?".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Claim, StructuredSearchKind::Document],
                perspective_entity_ids: vec![fixture.sera_id],
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.sera_claim, fixture.sera_journal],
            contradiction_sources: vec![],
            expected_category: "perspectives",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "contradictions-01".to_owned(),
            family: "perspectives",
            question: "Preserve both incompatible accounts of the collapse.".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Context(RelatedContextRequest {
                bundle: ContextBundleRequest {
                    intent: ContextIntent::ContradictionCheck,
                    anchors: vec![fixture.caldris_mine],
                    query_text: None,
                    temporal: None,
                    temporal_radius: None,
                    perspective_entity_ids: vec![],
                    include_perspectives: true,
                    relation_limit: 4,
                    budget: ContextBudget {
                        max_objects: 30,
                        max_chars: 4_000,
                    },
                },
                kinds: vec![StructuredSearchKind::Claim],
                empty: EmptySearchClassification::NoEvidence,
            }),
            required_sources: vec![fixture.sera_claim, fixture.orun_claim],
            contradiction_sources: vec![fixture.sera_claim, fixture.orun_claim],
            expected_category: "relations",
            expected_recall_bps: 10_000,
        },
        QuerySpec {
            id: "fts5-01".to_owned(),
            family: "fts5_exact",
            question: "Find sources containing the exact term Vaultglass.".to_owned(),
            world: BenchmarkWorld::Structural,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Entity, StructuredSearchKind::Document],
                text: Some("Vaultglass".to_owned()),
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![fixture.vaultglass, fixture.vaultglass_registry],
            contradiction_sources: vec![],
            expected_category: "fts5",
            expected_recall_bps: 10_000,
        },
    ];

    for case in &corpus.lexical_cases {
        let source = *lexical.sources.get(&case.id).expect("lexical source ID");
        specs.push(QuerySpec {
            id: format!("{}-exact", case.id),
            family: "fts5_exact",
            question: case.exact_query.clone(),
            world: BenchmarkWorld::Lexical,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Document],
                text: Some(case.exact_query.clone()),
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![source],
            contradiction_sources: vec![],
            expected_category: "fts5",
            expected_recall_bps: 10_000,
        });
        specs.push(QuerySpec {
            id: format!("{}-paraphrase", case.id),
            family: "paraphrase_gap",
            question: case.paraphrase_query.clone(),
            world: BenchmarkWorld::Lexical,
            request: BenchmarkRequest::Structured(StructuredSearchQuery {
                kinds: vec![StructuredSearchKind::Document],
                text: Some(case.paraphrase_query.clone()),
                limit: 10,
                ..Default::default()
            }),
            required_sources: vec![source],
            contradiction_sources: vec![],
            expected_category: "fts5",
            expected_recall_bps: 0,
        });
    }
    specs
}

fn execute_query(
    spec: &QuerySpec,
    structural_app: &NirmataApp,
    structural_store: &WorldStore,
    lexical_app: &NirmataApp,
    lexical_store: &WorldStore,
    semantic_fallback: bool,
) -> Vec<RetrievedSource> {
    let (app, store) = match spec.world {
        BenchmarkWorld::Structural => (structural_app, structural_store),
        BenchmarkWorld::Lexical => (lexical_app, lexical_store),
    };
    match &spec.request {
        BenchmarkRequest::Context(request) => app
            .get_related_context(request)
            .expect("benchmark context query")
            .all_entries()
            .into_iter()
            .map(|entry| {
                let (category, stage) = context_attribution(entry.stage);
                RetrievedSource {
                    object: entry.result.object_ref,
                    category,
                    provenance: format!("{stage}/{}", provenance_prefix(&entry.result.provenance)),
                    citation_chars: entry.result.snippet.chars().count(),
                }
            })
            .collect(),
        BenchmarkRequest::Structured(query) => {
            if semantic_fallback {
                app.search_world(&SearchWorldRequest::new(query.clone()))
                    .expect("benchmark active app search")
                    .hits
                    .into_iter()
                    .map(|hit| {
                        let (category, stage) = app_attribution(&hit.stage);
                        assert_eq!(hit.uri, hit.object_ref.to_string());
                        assert!(hit.rank > 0);
                        assert!(hit.score > 0);
                        assert!(!hit.score_explanation.is_empty());
                        RetrievedSource {
                            object: hit.object_ref,
                            category,
                            provenance: format!("{stage}/{}", provenance_prefix(&hit.provenance)),
                            citation_chars: hit.snippet.chars().count(),
                        }
                    })
                    .collect()
            } else {
                store
                    .search_structured_fts(query)
                    .expect("benchmark baseline structured query")
                    .into_iter()
                    .map(|hit| {
                        let (category, stage) = structured_attribution(hit.stage);
                        RetrievedSource {
                            object: hit.object,
                            category,
                            provenance: format!("{stage}/{}", provenance_prefix(&hit.provenance)),
                            citation_chars: hit.fragment.chars().count(),
                        }
                    })
                    .collect()
            }
        }
    }
}

fn app_attribution(stage: &str) -> (&'static str, &'static str) {
    match stage {
        "structured_sql" => ("structured_sql", "type"),
        "alias" => ("structured_sql", "alias"),
        "relation" => ("relations", "neighbor"),
        "goal" => ("goals", "goal"),
        "perspective" => ("perspectives", "perspective"),
        "time" => ("time", "temporal"),
        "fts5" => ("fts5", "text"),
        "semantic" => ("semantic", "semantic"),
        stage => panic!("unexpected app retrieval stage {stage}"),
    }
}

fn context_attribution(stage: ContextStage) -> (&'static str, &'static str) {
    match stage {
        ContextStage::Selection => ("explicit_anchor", "selection"),
        ContextStage::Relation => ("relations", "relation"),
        ContextStage::Temporal => ("time", "temporal"),
        ContextStage::Goal => ("goals", "goal"),
        ContextStage::Perspective => ("perspectives", "perspective"),
        ContextStage::Search => ("fts5", "search"),
        ContextStage::Semantic => ("semantic", "semantic"),
    }
}

fn structured_attribution(stage: StructuredSearchStage) -> (&'static str, &'static str) {
    match stage {
        StructuredSearchStage::Type => ("structured_sql", "type"),
        StructuredSearchStage::Alias => ("structured_sql", "alias"),
        StructuredSearchStage::Neighbor => ("relations", "neighbor"),
        StructuredSearchStage::Goal => ("goals", "goal"),
        StructuredSearchStage::Perspective => ("perspectives", "perspective"),
        StructuredSearchStage::Temporal => ("time", "temporal"),
        StructuredSearchStage::Text => ("fts5", "text"),
        StructuredSearchStage::Semantic => ("semantic", "semantic"),
    }
}

fn provenance_prefix(provenance: &str) -> &str {
    provenance.split(':').next().unwrap_or(provenance)
}

fn measure_query(
    spec: QuerySpec,
    structural_app: &NirmataApp,
    structural_store: &WorldStore,
    lexical_app: &NirmataApp,
    lexical_store: &WorldStore,
    latency_budget: Duration,
    semantic_fallback: bool,
) -> QueryMeasurement {
    let expected = execute_query(
        &spec,
        structural_app,
        structural_store,
        lexical_app,
        lexical_store,
        semantic_fallback,
    );
    let mut samples = Vec::with_capacity(9);
    for _ in 0..9 {
        let started = Instant::now();
        let retrieved = execute_query(
            &spec,
            structural_app,
            structural_store,
            lexical_app,
            lexical_store,
            semantic_fallback,
        );
        samples.push(started.elapsed());
        assert_eq!(retrieved, expected, "{} must be deterministic", spec.id);
    }
    samples.sort_unstable();
    let latency_p50 = samples[4];
    let latency_p95 = samples[8];
    assert_local_latency(&spec.id, latency_p95, latency_budget);

    let returned = expected
        .iter()
        .map(|source| source.object)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        returned.len(),
        expected.len(),
        "{} returned duplicate citations",
        spec.id
    );
    for source in &expected {
        assert_eq!(
            source.category,
            if semantic_fallback && spec.family == "paraphrase_gap" {
                "semantic"
            } else {
                spec.expected_category
            },
            "{} attributed {} to the wrong retrieval stage",
            spec.id,
            source.object
        );
    }
    let required = spec
        .required_sources
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let relevant_retrieved = returned.intersection(&required).count();
    let recall_bps = ratio_bps(relevant_retrieved, required.len());
    if !semantic_fallback {
        assert_eq!(
            recall_bps, spec.expected_recall_bps,
            "{} retrieval recall regressed",
            spec.id
        );
    }
    let cited_precision_bps =
        (!returned.is_empty()).then(|| ratio_bps(relevant_retrieved, returned.len()));
    let irrelevant = returned.difference(&required).copied().collect::<Vec<_>>();
    if !semantic_fallback {
        assert!(
            irrelevant.is_empty(),
            "{} returned irrelevant citations: {:?}",
            spec.id,
            irrelevant
        );
    }
    let contradiction_preserved = (!spec.contradiction_sources.is_empty()).then(|| {
        spec.contradiction_sources
            .iter()
            .all(|source| returned.contains(source))
    });
    if let Some(preserved) = contradiction_preserved {
        assert!(preserved, "{} omitted a contradictory source", spec.id);
    }
    let citation_chars = expected.iter().map(|source| source.citation_chars).sum();

    QueryMeasurement {
        spec,
        retrieved: expected,
        irrelevant,
        recall_bps,
        cited_precision_bps,
        contradiction_preserved,
        citation_chars,
        latency_p50,
        latency_p95,
        latency_samples: samples,
    }
}

fn ratio_bps(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        return 10_000;
    }
    ((numerator * 10_000) / denominator) as u32
}

fn assert_local_latency(label: &str, latency_p95: Duration, latency_budget: Duration) {
    // Wall-clock latency is meaningful in the dedicated benchmark command, not
    // while nextest deliberately runs this binary beside the whole workspace.
    if std::env::var_os("NEXTEST").is_none() {
        assert!(
            latency_p95 <= latency_budget,
            "{label} p95 {latency_p95:?} exceeded local budget {latency_budget:?}"
        );
    }
}

fn labels_for<'a>(
    world: BenchmarkWorld,
    structural: &'a BTreeMap<ObjectRef, String>,
    lexical: &'a BTreeMap<ObjectRef, String>,
) -> &'a BTreeMap<ObjectRef, String> {
    match world {
        BenchmarkWorld::Structural => structural,
        BenchmarkWorld::Lexical => lexical,
    }
}

fn source_names(sources: &[ObjectRef], labels: &BTreeMap<ObjectRef, String>) -> String {
    if sources.is_empty() {
        return "none".to_owned();
    }
    sources
        .iter()
        .map(|source| {
            labels
                .get(source)
                .expect("every benchmark source has a stable label")
                .as_str()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_measurements(
    corpus: &RetrievalCorpus,
    measurements: &[QueryMeasurement],
    structural_labels: &BTreeMap<ObjectRef, String>,
    lexical_labels: &BTreeMap<ObjectRef, String>,
    semantic_fallback: bool,
) {
    println!("NIR-053 benchmark corpus {}", corpus.version);
    println!(
        "| Query | Request | Family | Required | Retrieved | Irrelevant | Contradiction | Stage/provenance | Recall | Cited precision | Citation chars (~tokens) | Local p50/p95/budget |"
    );
    println!("|---|---|---|---|---|---|---|---|---:|---:|---:|---:|");
    for measurement in measurements {
        let labels = labels_for(measurement.spec.world, structural_labels, lexical_labels);
        let retrieved_refs = measurement
            .retrieved
            .iter()
            .map(|source| source.object)
            .collect::<Vec<_>>();
        let attribution = if measurement.retrieved.is_empty() {
            format!(
                "{}/none",
                if semantic_fallback && measurement.spec.family == "paraphrase_gap" {
                    "semantic"
                } else {
                    measurement.spec.expected_category
                }
            )
        } else {
            measurement
                .retrieved
                .iter()
                .map(|source| {
                    format!(
                        "{}@{}",
                        labels.get(&source.object).expect("retrieved source label"),
                        source.provenance
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let contradiction = match measurement.contradiction_preserved {
            Some(true) => "preserved",
            Some(false) => "omitted",
            None => "n/a",
        };
        let precision = measurement
            .cited_precision_bps
            .map(|value| format!("{:.1}%", value as f64 / 100.0))
            .unwrap_or_else(|| "n/a".to_owned());
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.1}% | {} | {} (~{}) | {:.3}/{:.3}/{:?} ms |",
            measurement.spec.id,
            measurement.spec.question.replace('|', "\\|"),
            measurement.spec.family,
            source_names(&measurement.spec.required_sources, labels),
            source_names(&retrieved_refs, labels),
            source_names(&measurement.irrelevant, labels),
            contradiction,
            attribution,
            measurement.recall_bps as f64 / 100.0,
            precision,
            measurement.citation_chars,
            measurement.citation_chars.div_ceil(4),
            measurement.latency_p50.as_secs_f64() * 1_000.0,
            measurement.latency_p95.as_secs_f64() * 1_000.0,
            corpus.latency_budget_ms,
        );
    }
}

#[test]
fn expanded_retrieval_corpus_measures_stage_attribution_and_opens_the_prototype_gate() {
    let corpus = load_retrieval_corpus();
    assert_eq!(corpus.version, "nir-053-v1");
    assert_eq!(corpus.recall_target_bps, 9_000);
    assert_eq!(corpus.cited_precision_floor_bps, 9_500);
    assert_eq!(corpus.latency_budget_ms, 250);
    assert_eq!(corpus.minimum_affected_paraphrase_queries, 10);

    let structural_path = project_path("retrieval-benchmark-expanded-structural");
    let lexical_path = project_path("retrieval-benchmark-expanded-lexical");
    let structural_fixture = build_fixture(&structural_path);
    let lexical_fixture = build_lexical_fixture(&lexical_path, &corpus);
    let structural_app = open_app(&structural_path);
    let lexical_app = open_app(&lexical_path);
    let structural_store = WorldStore::open(&structural_path).expect("structural benchmark store");
    let lexical_store = WorldStore::open(&lexical_path).expect("lexical benchmark store");
    let specs = expanded_query_specs(&structural_fixture, &lexical_fixture, &corpus);
    assert_eq!(
        specs.len(),
        34,
        "corpus query count is part of the gate record"
    );

    let latency_budget = Duration::from_millis(corpus.latency_budget_ms);
    let measurements = specs
        .into_iter()
        .map(|spec| {
            measure_query(
                spec,
                &structural_app,
                &structural_store,
                &lexical_app,
                &lexical_store,
                latency_budget,
                false,
            )
        })
        .collect::<Vec<_>>();

    let categories = measurements
        .iter()
        .map(|measurement| measurement.spec.expected_category)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        categories,
        BTreeSet::from([
            "explicit_anchor",
            "structured_sql",
            "relations",
            "time",
            "goals",
            "perspectives",
            "fts5",
        ]),
        "every deterministic retrieval stage must remain attributed"
    );

    let non_paraphrase = measurements
        .iter()
        .filter(|measurement| measurement.spec.family != "paraphrase_gap")
        .collect::<Vec<_>>();
    let non_paraphrase_recall = ratio_bps(
        non_paraphrase
            .iter()
            .map(|measurement| {
                let returned = measurement
                    .retrieved
                    .iter()
                    .map(|source| source.object)
                    .collect::<BTreeSet<_>>();
                measurement
                    .spec
                    .required_sources
                    .iter()
                    .filter(|source| returned.contains(source))
                    .count()
            })
            .sum(),
        non_paraphrase
            .iter()
            .map(|measurement| measurement.spec.required_sources.len())
            .sum(),
    );
    assert!(
        non_paraphrase_recall >= corpus.recall_target_bps,
        "deterministic retrieval fell below the agreed recall target"
    );

    let affected_paraphrases = measurements
        .iter()
        .filter(|measurement| {
            measurement.spec.family == "paraphrase_gap"
                && measurement.recall_bps < corpus.recall_target_bps
        })
        .count();
    assert_eq!(affected_paraphrases, 12);
    assert!(
        affected_paraphrases >= corpus.minimum_affected_paraphrase_queries,
        "NIR-054 prototype requires at least ten repeatable paraphrase gaps"
    );

    let cited_hits = measurements
        .iter()
        .flat_map(|measurement| &measurement.retrieved)
        .count();
    let irrelevant_hits = measurements
        .iter()
        .map(|measurement| measurement.irrelevant.len())
        .sum::<usize>();
    let cited_precision = ratio_bps(cited_hits.saturating_sub(irrelevant_hits), cited_hits);
    assert!(
        cited_precision >= corpus.cited_precision_floor_bps,
        "baseline cited precision fell below the gate floor"
    );
    assert!(measurements.iter().any(|measurement| {
        measurement.spec.id == "contradictions-01"
            && measurement.contradiction_preserved == Some(true)
    }));

    let required_total = measurements
        .iter()
        .map(|measurement| measurement.spec.required_sources.len())
        .sum::<usize>();
    let required_retrieved = measurements
        .iter()
        .map(|measurement| {
            let returned = measurement
                .retrieved
                .iter()
                .map(|source| source.object)
                .collect::<BTreeSet<_>>();
            measurement
                .spec
                .required_sources
                .iter()
                .filter(|source| returned.contains(source))
                .count()
        })
        .sum::<usize>();
    let mut latency_samples = measurements
        .iter()
        .flat_map(|measurement| measurement.latency_samples.iter().copied())
        .collect::<Vec<_>>();
    latency_samples.sort_unstable();
    let latency_p50 = latency_samples[latency_samples.len() / 2];
    let latency_p95 = latency_samples[(latency_samples.len() * 95).div_ceil(100) - 1];
    assert_local_latency("baseline aggregate", latency_p95, latency_budget);

    print_measurements(
        &corpus,
        &measurements,
        &structural_fixture.labels,
        &lexical_fixture.labels,
        false,
    );
    println!(
        "SUMMARY queries={} required={}/{} overall_recall={:.1}% non_paraphrase_recall={:.1}% paraphrase_recall=0.0% cited_precision={:.1}% affected_paraphrases={} contradictions=preserved local_p50_ms={:.3} local_p95_ms={:.3} budget_ms={} prototype_justified=true",
        measurements.len(),
        required_retrieved,
        required_total,
        ratio_bps(required_retrieved, required_total) as f64 / 100.0,
        non_paraphrase_recall as f64 / 100.0,
        cited_precision as f64 / 100.0,
        affected_paraphrases,
        latency_p50.as_secs_f64() * 1_000.0,
        latency_p95.as_secs_f64() * 1_000.0,
        corpus.latency_budget_ms,
    );

    drop(structural_store);
    drop(lexical_store);
    drop(structural_app);
    drop(lexical_app);
    fs::remove_file(structural_path).expect("remove structural benchmark project");
    fs::remove_file(lexical_path).expect("remove lexical benchmark project");
}

#[test]
fn hybrid_active_path_meets_the_nir_053_gate() {
    let corpus = load_retrieval_corpus();
    let structural_path = project_path("retrieval-semantic-structural");
    let lexical_path = project_path("retrieval-semantic-lexical");
    let structural_fixture = build_fixture(&structural_path);
    let lexical_fixture = build_lexical_fixture(&lexical_path, &corpus);
    let structural_app = open_app(&structural_path);
    let lexical_app = open_app(&lexical_path);
    let structural_store = WorldStore::open(&structural_path).expect("structural semantic store");
    let lexical_store = WorldStore::open(&lexical_path).expect("lexical semantic store");
    let latency_budget = Duration::from_millis(corpus.latency_budget_ms);
    let specs = expanded_query_specs(&structural_fixture, &lexical_fixture, &corpus);
    let baseline = specs
        .iter()
        .cloned()
        .map(|spec| {
            measure_query(
                spec,
                &structural_app,
                &structural_store,
                &lexical_app,
                &lexical_store,
                latency_budget,
                false,
            )
        })
        .collect::<Vec<_>>();
    let measurements = specs
        .into_iter()
        .map(|spec| {
            measure_query(
                spec,
                &structural_app,
                &structural_store,
                &lexical_app,
                &lexical_store,
                latency_budget,
                true,
            )
        })
        .collect::<Vec<_>>();

    let baseline_paraphrases = baseline
        .iter()
        .filter(|measurement| measurement.spec.family == "paraphrase_gap")
        .collect::<Vec<_>>();
    let baseline_paraphrase_recall = ratio_bps(
        baseline_paraphrases
            .iter()
            .filter(|measurement| measurement.recall_bps == 10_000)
            .count(),
        baseline_paraphrases.len(),
    );
    let paraphrases = measurements
        .iter()
        .filter(|measurement| measurement.spec.family == "paraphrase_gap")
        .collect::<Vec<_>>();
    let paraphrase_recall = ratio_bps(
        paraphrases
            .iter()
            .filter(|measurement| measurement.recall_bps == 10_000)
            .count(),
        paraphrases.len(),
    );
    let non_paraphrases = measurements
        .iter()
        .filter(|measurement| measurement.spec.family != "paraphrase_gap")
        .collect::<Vec<_>>();
    let non_paraphrase_required = non_paraphrases
        .iter()
        .map(|measurement| measurement.spec.required_sources.len())
        .sum::<usize>();
    let non_paraphrase_retrieved = non_paraphrases
        .iter()
        .map(|measurement| {
            let returned = measurement
                .retrieved
                .iter()
                .map(|source| source.object)
                .collect::<BTreeSet<_>>();
            measurement
                .spec
                .required_sources
                .iter()
                .filter(|source| returned.contains(source))
                .count()
        })
        .sum::<usize>();
    let non_paraphrase_recall = ratio_bps(non_paraphrase_retrieved, non_paraphrase_required);
    let cited_hits = measurements
        .iter()
        .map(|measurement| measurement.retrieved.len())
        .sum::<usize>();
    let irrelevant_hits = measurements
        .iter()
        .map(|measurement| measurement.irrelevant.len())
        .sum::<usize>();
    let cited_precision = ratio_bps(cited_hits.saturating_sub(irrelevant_hits), cited_hits);
    let baseline_cited_hits = baseline
        .iter()
        .map(|measurement| measurement.retrieved.len())
        .sum::<usize>();
    let baseline_irrelevant_hits = baseline
        .iter()
        .map(|measurement| measurement.irrelevant.len())
        .sum::<usize>();
    let baseline_cited_precision = ratio_bps(
        baseline_cited_hits.saturating_sub(baseline_irrelevant_hits),
        baseline_cited_hits,
    );
    let mut latency_samples = measurements
        .iter()
        .flat_map(|measurement| measurement.latency_samples.iter().copied())
        .collect::<Vec<_>>();
    latency_samples.sort_unstable();
    let latency_p95 = latency_samples[(latency_samples.len() * 95).div_ceil(100) - 1];

    print_measurements(
        &corpus,
        &measurements,
        &structural_fixture.labels,
        &lexical_fixture.labels,
        true,
    );
    println!(
        "NIR-058 SUMMARY baseline_paraphrase_recall={:.1}% paraphrase_recall={:.1}% non_paraphrase_recall={:.1}% baseline_cited_precision={:.1}% cited_precision={:.1}% irrelevant={} contradictions=2/2 local_p95_ms={:.3} budget_ms={}",
        baseline_paraphrase_recall as f64 / 100.0,
        paraphrase_recall as f64 / 100.0,
        non_paraphrase_recall as f64 / 100.0,
        baseline_cited_precision as f64 / 100.0,
        cited_precision as f64 / 100.0,
        irrelevant_hits,
        latency_p95.as_secs_f64() * 1_000.0,
        corpus.latency_budget_ms,
    );
    assert!(
        paraphrase_recall >= baseline_paraphrase_recall.saturating_add(1_000),
        "recall must improve by ten points"
    );
    assert_eq!(
        (baseline_paraphrase_recall, paraphrase_recall),
        (0, 2_500),
        "the active semantic technology must retain its measured 3/12 benefit"
    );
    assert_eq!(
        (
            non_paraphrase_retrieved,
            non_paraphrase_required,
            non_paraphrase_recall
        ),
        (28, 28, 10_000),
        "all authoritative and FTS retrieval stages must retain full recall"
    );
    assert_eq!(
        (cited_hits, irrelevant_hits, cited_precision),
        (31, 0, 10_000),
        "every active retrieval hit must remain a relevant citation"
    );
    assert!(measurements.iter().any(|measurement| {
        measurement.spec.id == "contradictions-01"
            && measurement.contradiction_preserved == Some(true)
            && measurement.retrieved.len() == 2
    }));
    assert!(
        cited_precision.saturating_add(500) >= baseline_cited_precision,
        "cited precision may lose at most five points"
    );
    assert_local_latency("hybrid aggregate", latency_p95, latency_budget);

    drop(structural_store);
    drop(lexical_store);
    drop(structural_app);
    drop(lexical_app);
    fs::remove_file(structural_path).expect("remove structural semantic project");
    fs::remove_file(lexical_path).expect("remove lexical semantic project");
}
