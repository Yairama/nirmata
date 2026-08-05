use nirmata_app::{
    ContextBudget, ContextBundleRequest, ContextIntent, EmptySearchClassification, NirmataApp,
    RelatedContextRequest, SearchClassification, SearchWorldRequest,
};
use nirmata_core::{
    Period, World,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{
    DocumentAggregate, EventAggregate, StructuredSearchKind, StructuredSearchQuery, WorldStore,
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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
    sera_id: nirmata_core::EntityId,
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
        vec![],
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
        vec![],
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
        vec![],
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

    drop(store);

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
        sera_id: sera.id(),
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
                    max_objects: 10,
                    max_chars: 520,
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
