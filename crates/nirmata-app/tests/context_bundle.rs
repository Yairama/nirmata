use nirmata_app::{ContextBudget, ContextBundleRequest, ContextIntent, ContextStage, NirmataApp};
use nirmata_core::{
    Period, World,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{DocumentAggregate, EventAggregate, StructuredSearchTemporal, WorldStore};
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

fn assert_unique_refs(bundle: &nirmata_app::ContextBundle) {
    let refs = bundle
        .all_entries()
        .into_iter()
        .map(|entry| entry.object_ref())
        .collect::<Vec<_>>();
    let unique = refs.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        refs.len(),
        unique.len(),
        "context should not duplicate objects"
    );
}

#[test]
fn entity_context_prioritizes_canon_and_dedupes_sources() {
    let path = project_path("context-entity");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");

    let mara = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer of the harbor",
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
        "Harbor witness",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("sera");
    let gate = Entity::new(
        world.id(),
        EntityKind::Place,
        "North Gate",
        "north-gate",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("gate");
    for entity in [&mara, &sera, &gate] {
        store.insert_entity(entity).expect("insert entity");
    }

    let relation = Relation::new(
        world.id(),
        mara.id(),
        gate.id(),
        "guards",
        RelationDirection::Directed,
        Some(12),
        None,
        Certainty::Certain,
        Some("charter".to_owned()),
        "{}",
    )
    .expect("relation");
    store.insert_relation(&relation).expect("insert relation");

    let canonical_claim = Claim::new(
        world.id(),
        mara.id(),
        "Mara holds the north gate.",
        Some("gate.held".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        Some("record".to_owned()),
        None,
        None,
        None,
        Some(Period::new(Some(12), Some(12)).expect("period")),
        world.current_revision(),
    )
    .expect("canonical claim");
    let attributed_claim = Claim::new(
        world.id(),
        mara.id(),
        "Sera says Mara abandoned the gate.",
        Some("gate.held".to_owned()),
        Some(ClaimObject::Scalar("false".to_owned())),
        ClaimPolarity::Negative,
        ClaimAuthentication::Attributed,
        Some(sera.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        Some("eyewitness".to_owned()),
        Some("rumor".to_owned()),
        None,
        None,
        Some(0.5),
        Some(Period::new(Some(12), Some(12)).expect("period")),
        world.current_revision(),
    )
    .expect("attributed claim");
    store
        .insert_claim(&canonical_claim)
        .expect("insert canonical claim");
    store
        .insert_claim(&attributed_claim)
        .expect("insert attributed claim");

    let rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Watch posts must stay manned.",
        "person",
        RuleSeverity::Advisory,
        Some("code".to_owned()),
        None,
        "{}",
        1,
    )
    .expect("rule");
    store.insert_rule(&rule).expect("insert rule");

    drop(store);

    let app = open_app(&path);
    let bundle = app
        .build_context_bundle(&ContextBundleRequest {
            intent: ContextIntent::EntityQuery,
            anchors: vec![ObjectRef::Entity(mara.id())],
            query_text: None,
            temporal: None,
            temporal_radius: None,
            perspective_entity_ids: vec![],
            include_perspectives: false,
            relation_limit: 4,
            budget: ContextBudget {
                max_objects: 12,
                max_chars: 600,
            },
        })
        .expect("build context");

    assert!(bundle.contains(ObjectRef::Entity(mara.id())));
    assert!(bundle.contains(ObjectRef::Relation(relation.id())));
    assert!(bundle.contains(ObjectRef::Claim(canonical_claim.id())));
    assert!(!bundle.contains(ObjectRef::Claim(attributed_claim.id())));
    assert!(bundle.perspectives.is_empty());
    assert!(
        bundle
            .obligations
            .iter()
            .any(|entry| entry.object_ref() == ObjectRef::Rule(rule.id()))
    );
    assert!(bundle.canon.iter().any(|entry| entry.object_ref()
        == ObjectRef::Claim(canonical_claim.id())
        && entry.provenance == format!("claim:{}", ObjectRef::Entity(mara.id()))));
    assert_unique_refs(&bundle);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn impact_context_adds_temporal_window_goals_and_respects_budgets() {
    let path = project_path("context-impact");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");

    let mara = Entity::new(
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
    .expect("mara");
    let mine = Entity::new(
        world.id(),
        EntityKind::Place,
        "Stormglass Mine",
        "stormglass-mine",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("mine");
    let outsider = Entity::new(
        world.id(),
        EntityKind::Person,
        "Iven",
        "iven",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("outsider");
    for entity in [&mara, &mine, &outsider] {
        store.insert_entity(entity).expect("insert entity");
    }

    let goal = Goal::new(
        world.id(),
        mara.id(),
        "Keep the mine operational.",
        8,
        GoalStatus::Active,
        Some(Period::new(Some(18), Some(24)).expect("period")),
        GoalVisibility::Public,
        Some("orders".to_owned()),
    )
    .expect("goal");
    store.insert_goal(&goal).expect("insert goal");

    let sabotage = Event::new(
        world.id(),
        "sabotage",
        "Saboteurs breach the eastern tunnel.",
        "",
        EventTime::instant(18, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "defender", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("sabotage");
    let collapse = Event::new(
        world.id(),
        "collapse",
        "The Stormglass Mine collapses.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "survivor", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("collapse");
    let response = Event::new(
        world.id(),
        "response",
        "Mara seals the lower shafts.",
        "",
        EventTime::instant(22, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "commander", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("response");
    let unrelated = Event::new(
        world.id(),
        "festival",
        "Iven celebrates in a distant market.",
        "",
        EventTime::instant(21, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(outsider.id(), "guest", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("unrelated");
    store
        .insert_event(&EventAggregate::new(response.clone(), vec![]))
        .expect("insert response");
    store
        .insert_event(&EventAggregate::new(sabotage.clone(), vec![]))
        .expect("insert sabotage");
    store
        .insert_event(&EventAggregate::new(
            collapse.clone(),
            vec![
                EventLink::new(collapse.id(), response.id(), EventLinkKind::Causes).expect("link"),
            ],
        ))
        .expect("insert collapse");
    store
        .insert_event(&EventAggregate::new(unrelated.clone(), vec![]))
        .expect("insert unrelated");

    drop(store);

    let app = open_app(&path);
    let bundle = app
        .build_context_bundle(&ContextBundleRequest {
            intent: ContextIntent::ImpactAnalysis,
            anchors: vec![ObjectRef::Event(collapse.id())],
            query_text: None,
            temporal: None,
            temporal_radius: Some(3),
            perspective_entity_ids: vec![],
            include_perspectives: false,
            relation_limit: 2,
            budget: ContextBudget {
                max_objects: 6,
                max_chars: 260,
            },
        })
        .expect("build impact context");

    assert!(bundle.contains(ObjectRef::Event(collapse.id())));
    assert!(bundle.contains(ObjectRef::Event(sabotage.id())));
    assert!(bundle.contains(ObjectRef::Event(response.id())));
    assert!(!bundle.contains(ObjectRef::Event(unrelated.id())));
    assert!(
        bundle
            .desires
            .iter()
            .any(|entry| entry.object_ref() == ObjectRef::Goal(goal.id()))
    );
    assert!(bundle.canon.iter().any(
        |entry| entry.object_ref() == ObjectRef::Event(sabotage.id())
            && entry.provenance == format!("event:{}", ObjectRef::Entity(mara.id()))
    ));
    assert!(bundle.usage.used_objects <= 6);
    assert!(bundle.usage.used_chars <= 260);
    assert_unique_refs(&bundle);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn contradiction_context_includes_perspectives_when_requested() {
    let path = project_path("context-contradiction");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");

    let gate = Entity::new(
        world.id(),
        EntityKind::Place,
        "North Gate",
        "north-gate",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("gate");
    let sera = Entity::new(
        world.id(),
        EntityKind::Person,
        "Sera",
        "sera",
        "Harbor witness",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("sera");
    store.insert_entity(&gate).expect("insert gate");
    store.insert_entity(&sera).expect("insert sera");

    let journal = Document::new(
        world.id(),
        "Sera's Journal",
        "chronicle",
        Some(sera.id()),
        Some(sera.id()),
        DocumentCanonStatus::NonCanonical,
        "Sera insists the gate closed at dusk.",
        1,
    )
    .expect("journal");
    store
        .insert_document(&DocumentAggregate::new(journal.clone(), vec![]))
        .expect("insert journal");

    let canonical_claim = Claim::new(
        world.id(),
        gate.id(),
        "The North Gate stays open.",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        Some("registry".to_owned()),
        None,
        None,
        None,
        Some(Period::new(Some(12), Some(12)).expect("period")),
        world.current_revision(),
    )
    .expect("canonical claim");
    let perspective_claim = Claim::new(
        world.id(),
        gate.id(),
        "Sera claims the gate closed before moonrise.",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("false".to_owned())),
        ClaimPolarity::Negative,
        ClaimAuthentication::Attributed,
        Some(sera.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        Some("eyewitness".to_owned()),
        Some("journal".to_owned()),
        Some(journal.id()),
        None,
        Some(0.7),
        Some(Period::new(Some(12), Some(12)).expect("period")),
        world.current_revision(),
    )
    .expect("perspective claim");
    store
        .insert_claim(&canonical_claim)
        .expect("insert canonical claim");
    store
        .insert_claim(&perspective_claim)
        .expect("insert perspective claim");

    drop(store);

    let app = open_app(&path);
    let bundle = app
        .build_context_bundle(&ContextBundleRequest {
            intent: ContextIntent::ContradictionCheck,
            anchors: vec![ObjectRef::Entity(gate.id())],
            query_text: None,
            temporal: Some(StructuredSearchTemporal::Tick(12)),
            temporal_radius: None,
            perspective_entity_ids: vec![],
            include_perspectives: true,
            relation_limit: 4,
            budget: ContextBudget {
                max_objects: 12,
                max_chars: 500,
            },
        })
        .expect("build contradiction context");

    assert!(bundle.contains(ObjectRef::Claim(canonical_claim.id())));
    assert!(bundle.contains(ObjectRef::Claim(perspective_claim.id())));
    assert!(bundle.contains(ObjectRef::Document(journal.id())));
    assert!(
        bundle
            .canon
            .iter()
            .any(|entry| entry.object_ref() == ObjectRef::Claim(canonical_claim.id()))
    );
    assert!(bundle.perspectives.iter().any(|entry| entry.object_ref()
        == ObjectRef::Claim(perspective_claim.id())
        && entry.provenance == format!("claim:{}", ObjectRef::Entity(gate.id()))));
    assert!(bundle.perspectives.iter().any(|entry| entry.object_ref()
        == ObjectRef::Document(journal.id())
        && entry.provenance == format!("perspective:{}:document", sera.id())));
    assert_unique_refs(&bundle);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn document_context_preserves_search_evidence_and_character_limits() {
    let path = project_path("context-document");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");

    let mara = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Archivist",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("mara");
    let mine = Entity::new(
        world.id(),
        EntityKind::Place,
        "Stormglass Mine",
        "stormglass-mine",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("mine");
    store.insert_entity(&mara).expect("insert mara");
    store.insert_entity(&mine).expect("insert mine");

    let collapse = Event::new(
        world.id(),
        "collapse",
        "The Stormglass Mine collapses under the bay.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "witness", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("collapse");
    store
        .insert_event(&EventAggregate::new(collapse.clone(), vec![]))
        .expect("insert collapse");

    let ledger = Document::new(
        world.id(),
        "Harbor Ledger",
        "chronicle",
        Some(mara.id()),
        Some(mara.id()),
        DocumentCanonStatus::Canonical,
        "Stormglass tallies mention the collapse in every margin.",
        1,
    )
    .expect("ledger");
    store
        .insert_document(&DocumentAggregate::new(
            ledger.clone(),
            vec![
                ContentReference::new(
                    ObjectRef::Document(ledger.id()),
                    ObjectRef::Entity(mara.id()),
                    0,
                ),
                ContentReference::new(
                    ObjectRef::Document(ledger.id()),
                    ObjectRef::Event(collapse.id()),
                    1,
                ),
            ],
        ))
        .expect("insert ledger");

    let report = Document::new(
        world.id(),
        "Harbor Report",
        "report",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "Stormglass inventory notes survive in a sealed annex.",
        1,
    )
    .expect("report");
    store
        .insert_document(&DocumentAggregate::new(report.clone(), vec![]))
        .expect("insert report");

    drop(store);

    let app = open_app(&path);
    let bundle = app
        .build_context_bundle(&ContextBundleRequest {
            intent: ContextIntent::DocumentDraft,
            anchors: vec![ObjectRef::Document(ledger.id())],
            query_text: Some("Stormglass".to_owned()),
            temporal: None,
            temporal_radius: None,
            perspective_entity_ids: vec![],
            include_perspectives: true,
            relation_limit: 0,
            budget: ContextBudget {
                max_objects: 6,
                max_chars: 140,
            },
        })
        .expect("build document context");

    assert!(bundle.contains(ObjectRef::Document(ledger.id())));
    assert!(bundle.contains(ObjectRef::Entity(mara.id())));
    assert!(bundle.contains(ObjectRef::Event(collapse.id())));
    assert!(bundle.search_evidence.iter().any(|entry| entry.object_ref()
        == ObjectRef::Document(report.id())
        && entry.provenance == "fts5"
        && entry.stage == ContextStage::Search));
    assert!(bundle.usage.used_objects <= 6);
    assert!(bundle.usage.used_chars <= 140);
    assert!(
        bundle
            .search_evidence
            .iter()
            .all(|entry| entry.citation.chars().count() <= 140)
    );
    assert_unique_refs(&bundle);

    drop(app);
    fs::remove_file(path).expect("remove project");
}
