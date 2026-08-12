use nirmata_app::{
    AppError, ContextBudget, ContextBundleRequest, ContextIntent, EmptySearchClassification,
    NirmataApp, OpenUriResponse, RelatedContextRequest, SearchAuthority, SearchClassification,
    SearchWorldRequest,
};
use nirmata_core::{
    Period, World,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventParticipant},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{
    DocumentAggregate, EventAggregate, StructuredSearchKind, StructuredSearchQuery,
    StructuredSearchTemporal, WorldStore,
};
use std::{
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

#[test]
fn search_world_result_opens_the_exact_source_uri() {
    let path = project_path("search-open-uri");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");

    let document = Document::new(
        world.id(),
        "Harbor Ledger",
        "chronicle",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "Stormglass entries line every page.",
        1,
    )
    .expect("document");
    store
        .insert_document(&DocumentAggregate::new(document.clone(), vec![]))
        .expect("insert document");

    drop(store);

    let app = open_app(&path);
    let response = app
        .search_world(&SearchWorldRequest::new(StructuredSearchQuery {
            text: Some("Stormglass".to_owned()),
            limit: 10,
            ..Default::default()
        }))
        .expect("search world");

    assert_eq!(response.hits.len(), 1);
    assert!(response.absence.is_none());
    let hit = &response.hits[0];
    assert_eq!(hit.object_ref, ObjectRef::Document(document.id()));
    assert_eq!(hit.classification, SearchClassification::Fact);
    assert_eq!(hit.authority, SearchAuthority::Canonical);

    let opened = app.open_uri(&hit.uri).expect("open uri");
    assert_eq!(
        opened,
        OpenUriResponse {
            result: nirmata_app::SearchResult {
                object_ref: ObjectRef::Document(document.id()),
                object_type: "document",
                object_id: document.id().to_string(),
                uri: ObjectRef::Document(document.id()).to_string(),
                snippet: "Harbor Ledger Stormglass entries line every page.".to_owned(),
                authority: SearchAuthority::Canonical,
                classification: SearchClassification::Fact,
                provenance: format!("open_uri:{}", ObjectRef::Document(document.id())),
                stage: "uri".to_owned(),
                score: 100_000,
                rank: 1,
                score_explanation: "explicit URI resolution".to_owned(),
            },
            object: nirmata_store::ResolvedObject::Document(DocumentAggregate::new(
                document.clone(),
                vec![],
            )),
        }
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn active_hybrid_context_is_cited_ranked_rebuildable_and_reads_updated_canon() {
    let path = project_path("active-hybrid-context");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");
    let anchor = Entity::new(
        world.id(),
        EntityKind::Place,
        "North Hall",
        "north-hall",
        "The council chamber.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("anchor");
    let document = Document::new(
        world.id(),
        "Senate Record",
        "minutes",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "The senate refuses the border pact.",
        1,
    )
    .expect("document");
    store.insert_entity(&anchor).expect("insert anchor");
    store
        .insert_document(&DocumentAggregate::new(document.clone(), vec![]))
        .expect("insert document");

    let app = open_app(&path);
    let request = SearchWorldRequest::new(StructuredSearchQuery {
        kinds: vec![StructuredSearchKind::Document],
        text: Some("council rejects treaty".to_owned()),
        limit: 5,
        ..Default::default()
    });
    let before_rebuild = app.search_world(&request).expect("hybrid app search");
    assert_eq!(before_rebuild.hits.len(), 1);
    let semantic = &before_rebuild.hits[0];
    assert_eq!(semantic.object_ref, ObjectRef::Document(document.id()));
    assert_eq!(semantic.uri, ObjectRef::Document(document.id()).to_string());
    assert_eq!(semantic.stage, "semantic");
    assert_eq!(semantic.rank, 1);
    assert!(semantic.score > 10_000);
    assert!(semantic.snippet.contains("senate refuses"));
    assert!(semantic.provenance.contains("wordnet-en-offline:v1"));
    assert!(semantic.score_explanation.contains("basis points"));

    let context = app
        .get_related_context(&RelatedContextRequest {
            bundle: ContextBundleRequest {
                intent: ContextIntent::EntityQuery,
                anchors: vec![ObjectRef::Entity(anchor.id())],
                query_text: Some("council rejects treaty".to_owned()),
                temporal: None,
                temporal_radius: None,
                perspective_entity_ids: vec![],
                include_perspectives: false,
                relation_limit: 0,
                budget: ContextBudget {
                    max_objects: 4,
                    max_chars: 400,
                },
            },
            kinds: vec![],
            empty: EmptySearchClassification::NoEvidence,
        })
        .expect("active hybrid context");
    let anchor_entry = context
        .canon
        .iter()
        .find(|entry| entry.result.object_ref == ObjectRef::Entity(anchor.id()))
        .expect("authoritative anchor");
    let semantic_entry = context
        .search_evidence
        .iter()
        .find(|entry| entry.result.object_ref == ObjectRef::Document(document.id()))
        .expect("semantic context evidence");
    assert_eq!(anchor_entry.result.rank, 1);
    assert_eq!(anchor_entry.result.stage, "selection");
    assert!(anchor_entry.result.score > semantic_entry.result.score);
    assert_eq!(semantic_entry.result.stage, "semantic");
    assert_eq!(semantic_entry.result.uri, semantic.uri);
    assert_eq!(semantic_entry.result.provenance, semantic.provenance);

    store
        .rebuild_canon_text_index()
        .expect("deterministic full derived rebuild");
    assert_eq!(
        app.search_world(&request).expect("search after rebuild"),
        before_rebuild
    );

    let updated = Document::restore(
        document.id(),
        document.world_id(),
        document.title(),
        document.kind(),
        document.author_entity_id(),
        document.perspective_entity_id(),
        document.canon_status(),
        "The bakers count loaves at sunrise.",
        document.version(),
        document.created_at_ms(),
        2,
    )
    .expect("updated document");
    store
        .update_document(&DocumentAggregate::new(updated, vec![]))
        .expect("update document");
    let after_update = app.search_world(&request).expect("search updated canon");
    assert!(after_update.hits.is_empty());
    assert!(after_update.absence.is_some());

    drop(app);
    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn search_world_applies_stable_type_and_temporal_filters() {
    let path = project_path("search-filters");
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
    let sera = Entity::new(
        world.id(),
        EntityKind::Person,
        "Sera",
        "sera",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("sera");
    store.insert_entity(&mara).expect("insert mara");
    store.insert_entity(&sera).expect("insert sera");

    let in_period = Claim::new(
        world.id(),
        mara.id(),
        "Sera swears Mara hid the ember.",
        Some("ember.hidden".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(sera.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.6),
        Some(Period::new(Some(10), Some(20)).expect("period")),
        world.current_revision(),
    )
    .expect("claim");
    let out_of_period = Claim::new(
        world.id(),
        mara.id(),
        "Sera repeats the rumor later.",
        Some("ember.hidden".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(sera.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.6),
        Some(Period::new(Some(30), Some(40)).expect("period")),
        world.current_revision(),
    )
    .expect("later claim");
    let document = Document::new(
        world.id(),
        "Sera's Journal",
        "chronicle",
        Some(sera.id()),
        Some(sera.id()),
        DocumentCanonStatus::Canonical,
        "Sera writes the same rumor down.",
        1,
    )
    .expect("document");
    store.insert_claim(&in_period).expect("insert claim");
    store
        .insert_claim(&out_of_period)
        .expect("insert later claim");
    store
        .insert_document(&DocumentAggregate::new(document, vec![]))
        .expect("insert document");

    drop(store);

    let app = open_app(&path);
    let response = app
        .search_world(&SearchWorldRequest::new(StructuredSearchQuery {
            kinds: vec![StructuredSearchKind::Claim],
            perspective_entity_ids: vec![sera.id()],
            temporal: Some(StructuredSearchTemporal::Period(
                Period::new(Some(10), Some(20)).expect("period"),
            )),
            limit: 10,
            ..Default::default()
        }))
        .expect("search world");

    assert_eq!(response.absence, None);
    assert_eq!(response.hits.len(), 1);
    let hit = &response.hits[0];
    assert_eq!(hit.object_ref, ObjectRef::Claim(in_period.id()));
    assert_eq!(hit.object_type, "claim");
    assert_eq!(hit.authority, SearchAuthority::Perspective);
    assert_eq!(hit.classification, SearchClassification::Perspective);
    assert_eq!(hit.provenance, format!("perspective:{}:claim", sera.id()));

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn get_related_context_filters_output_types_without_losing_temporal_window() {
    let path = project_path("related-context-filters");
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
    store.insert_entity(&mara).expect("insert mara");
    store.insert_entity(&mine).expect("insert mine");

    let earlier = Event::new(
        world.id(),
        "sabotage",
        "Saboteurs breach the eastern tunnel.",
        "",
        EventTime::instant(18, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "defender", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("earlier event");
    let anchor = Event::new(
        world.id(),
        "collapse",
        "The Stormglass Mine collapses.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "survivor", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("anchor event");
    let later = Event::new(
        world.id(),
        "response",
        "Mara seals the lower shafts.",
        "",
        EventTime::instant(22, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![EventParticipant::new(mara.id(), "commander", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("later event");
    let outside = Event::new(
        world.id(),
        "festival",
        "The harbor market celebrates an unrelated festival.",
        "",
        EventTime::instant(30, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        1,
    )
    .expect("outside event");
    for event in [
        earlier.clone(),
        anchor.clone(),
        later.clone(),
        outside.clone(),
    ] {
        store
            .insert_event(&EventAggregate::new(event.clone(), vec![]))
            .expect("insert event");
    }

    drop(store);

    let app = open_app(&path);
    let response = app
        .get_related_context(&RelatedContextRequest {
            bundle: ContextBundleRequest {
                intent: ContextIntent::ImpactAnalysis,
                anchors: vec![ObjectRef::Event(anchor.id())],
                query_text: None,
                temporal: None,
                temporal_radius: Some(3),
                perspective_entity_ids: vec![],
                include_perspectives: false,
                relation_limit: 2,
                budget: ContextBudget {
                    max_objects: 12,
                    max_chars: 500,
                },
            },
            kinds: vec![StructuredSearchKind::Event],
            empty: EmptySearchClassification::NoEvidence,
        })
        .expect("get related context");

    assert!(response.absence.is_none());
    assert!(response.perspectives.is_empty());
    assert!(response.desires.is_empty());
    assert!(response.obligations.is_empty());
    assert!(response.search_evidence.is_empty());
    let event_refs = response
        .canon
        .iter()
        .map(|entry| entry.result.object_ref)
        .collect::<Vec<_>>();
    assert!(event_refs.contains(&ObjectRef::Event(anchor.id())));
    assert!(event_refs.contains(&ObjectRef::Event(earlier.id())));
    assert!(event_refs.contains(&ObjectRef::Event(later.id())));
    assert!(!event_refs.contains(&ObjectRef::Event(outside.id())));
    assert!(
        response
            .canon
            .iter()
            .all(|entry| entry.result.object_type == "event")
    );
    assert!(
        response
            .canon
            .iter()
            .all(|entry| entry.result.classification == SearchClassification::Fact)
    );
    assert_eq!(response.usage.used_objects, response.canon.len());

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn empty_results_use_absence_classification_instead_of_false_negation() {
    let path = project_path("search-empty-absence");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");

    let event = Event::new(
        world.id(),
        "collapse",
        "The Stormglass Mine collapses.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        1,
    )
    .expect("event");
    store
        .insert_event(&EventAggregate::new(event.clone(), vec![]))
        .expect("insert event");

    drop(store);

    let app = open_app(&path);
    let search = app
        .search_world(&SearchWorldRequest {
            query: StructuredSearchQuery {
                text: Some("moon palace".to_owned()),
                limit: 10,
                ..Default::default()
            },
            empty: EmptySearchClassification::NoEvidence,
        })
        .expect("empty search");
    assert!(search.hits.is_empty());
    assert_eq!(
        search.absence,
        Some(nirmata_app::SearchAbsence {
            classification: SearchClassification::NoEvidence,
            provenance: "search_world".to_owned(),
        })
    );

    let context = app
        .get_related_context(&RelatedContextRequest {
            bundle: ContextBundleRequest {
                intent: ContextIntent::EntityQuery,
                anchors: vec![ObjectRef::Event(event.id())],
                query_text: Some("moon palace".to_owned()),
                temporal: None,
                temporal_radius: None,
                perspective_entity_ids: vec![],
                include_perspectives: false,
                relation_limit: 0,
                budget: ContextBudget {
                    max_objects: 6,
                    max_chars: 240,
                },
            },
            kinds: vec![StructuredSearchKind::Document],
            empty: EmptySearchClassification::Unspecified,
        })
        .expect("empty related context");
    assert!(context.all_entries().is_empty());
    assert_eq!(
        context.absence,
        Some(nirmata_app::SearchAbsence {
            classification: SearchClassification::Unspecified,
            provenance: "get_related_context".to_owned(),
        })
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn invalid_uri_is_rejected_without_changing_or_stranding_the_open_project() {
    let path = project_path("invalid-uri-durability");
    let world = base_world(&path);
    let app = open_app(&path);

    let error = app
        .open_uri("javascript:alert(1)")
        .expect_err("hostile URI must be rejected");
    assert!(matches!(error, AppError::InvalidObjectUri(_)));
    assert!(error.to_string().contains("invalid nirmata URI"));
    drop(app);

    let reopened = WorldStore::open(&path).expect("reopen after invalid URI");
    assert_eq!(
        reopened
            .load_world()
            .expect("world after invalid URI")
            .current_revision(),
        world.current_revision()
    );
    assert_eq!(
        reopened
            .list_revisions()
            .expect("history after invalid URI")
            .len(),
        1
    );
    assert!(
        reopened
            .list_entities()
            .expect("canon after invalid URI")
            .is_empty()
    );
    drop(reopened);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn invalid_project_path_does_not_prevent_opening_the_valid_project_afterward() {
    let path = project_path("invalid-path-recovery");
    let world = base_world(&path);
    let mut app = NirmataApp::default();

    let error = app
        .open_world(path.with_extension("txt"))
        .expect_err("invalid extension must fail before file access");
    assert!(matches!(error, AppError::InvalidProjectPath(_)));
    let session = app
        .open_world(path.clone())
        .expect("valid project opens after invalid path");
    assert_eq!(session.current_revision, world.current_revision());
    app.close_world().expect("close recovered project");

    let reopened = WorldStore::open(&path).expect("reopen recovered project");
    assert_eq!(
        reopened
            .load_world()
            .expect("world after invalid path")
            .current_revision(),
        world.current_revision()
    );
    assert_eq!(
        reopened
            .list_revisions()
            .expect("history after invalid path")
            .len(),
        1
    );
    assert!(
        reopened
            .list_entities()
            .expect("canon after invalid path")
            .is_empty()
    );
    drop(reopened);
    fs::remove_file(path).expect("remove project");
}
