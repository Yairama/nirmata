use nirmata_app::{
    AppError, DraftOperationInput, ManualDraftRequest, ManualReviewFreshnessStatus,
    ManualReviewInput, NirmataApp,
};
use nirmata_core::{
    ChangeOperationId, World, WorldId,
    change_set::RetconKind,
    document::{ContentReference, Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::Event,
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{EventAggregate, WorldStore};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
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

fn person(world_id: WorldId, name: &str, slug: &str, now_ms: i64) -> Entity {
    Entity::new(
        world_id,
        EntityKind::Person,
        name,
        slug,
        "",
        "",
        "{}",
        vec![],
        now_ms,
    )
    .expect("entity")
}

fn commit_entity_create(
    app: &mut NirmataApp,
    world_id: WorldId,
    name: &str,
    slug: &str,
    now_ms: i64,
) {
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: format!("Add {name}"),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: person(world_id, name, slug, now_ms),
            }],
        })
        .expect("start review");
    app.confirm_manual_review(&review).expect("confirm review");
}

#[test]
fn previewing_manual_entity_create_returns_a_changeset_draft() {
    let path = project_path("manual-form-entity-create");
    base_world(&path);
    let mut app = open_app(&path);

    let response = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "entity".to_owned(),
            objective: Some("Create entity Mara".to_owned()),
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "person".to_owned()),
                ("name".to_owned(), "Mara".to_owned()),
                ("slug".to_owned(), "mara".to_owned()),
                ("aliases".to_owned(), "The Cartographer".to_owned()),
                ("attributes_json".to_owned(), "{}".to_owned()),
            ]),
        })
        .expect("preview manual draft");

    assert!(response.field_issues.is_empty());
    let draft = response.draft.expect("draft preview");
    assert_eq!(draft.object_type, "entity");
    assert_eq!(draft.mode, "create");
    assert!(draft.target_uri.starts_with("nirmata://entity/"));
    assert!(draft.ready_to_confirm);
    assert!(draft.validation_report.errors.is_empty());
    assert_eq!(draft.logical_path, "/entities/people/Mara");

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn logical_vfs_and_field_validation_are_available_via_app_use_cases() {
    let path = project_path("manual-form-vfs");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");
    let entity = Entity::new(
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
    .expect("entity");
    store.insert_entity(&entity).expect("insert entity");
    drop(store);

    let mut app = open_app(&path);
    let tree = app.read_logical_vfs().expect("logical tree");
    let people = tree
        .children
        .iter()
        .find_map(|node| match node {
            nirmata_store::LogicalVfsNode::Directory(directory) if directory.name == "entities" => {
                Some(directory)
            }
            _ => None,
        })
        .and_then(|directory| directory.child_directory("people"))
        .expect("people directory");
    let mara = people.child_object("Mara").expect("mara object");
    assert_eq!(mara.uri, ObjectRef::Entity(entity.id()).to_string());

    let response = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "relation".to_owned(),
            objective: Some("Create invalid relation".to_owned()),
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([("kind".to_owned(), "ally".to_owned())]),
        })
        .expect("invalid preview response");

    assert!(response.draft.is_none());
    assert!(
        response
            .field_issues
            .iter()
            .any(|issue| issue.field == "source_entity")
    );
    assert!(
        response
            .field_issues
            .iter()
            .any(|issue| issue.field == "target_entity")
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn previewing_manual_world_update_returns_a_changeset_draft() {
    let path = project_path("manual-form-world-update");
    let world = base_world(&path);
    let mut app = open_app(&path);
    let world_uri = ObjectRef::World(world.id()).to_string();

    let response = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "world".to_owned(),
            objective: Some("Actualizar mundo Arcadia".to_owned()),
            source_uris: vec![world_uri.clone()],
            assumptions: vec![],
            existing_uri: Some(world_uri.clone()),
            values: BTreeMap::from([
                ("name".to_owned(), "Arcadia Prime".to_owned()),
                (
                    "premise_md".to_owned(),
                    "Una ciudad que recuerda cada juramento.".to_owned(),
                ),
                ("epoch_label".to_owned(), "Second Dawn".to_owned()),
            ]),
        })
        .expect("preview world draft");

    assert!(response.field_issues.is_empty());
    let draft = response.draft.expect("world draft preview");
    assert_eq!(draft.object_type, "world");
    assert_eq!(draft.mode, "update");
    assert_eq!(draft.target_uri, world_uri);
    assert_eq!(draft.logical_path, "/world");
    assert!(draft.ready_to_confirm);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn previewing_manual_event_create_supports_causal_links() {
    let path = project_path("manual-form-event-causality");
    let world = base_world(&path);
    let consequence = Event::new(
        world.id(),
        "aftermath",
        "Puerta cerrada",
        "",
        EventTime::instant(12, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        2,
    )
    .expect("consequence event");
    let mut store = WorldStore::open(&path).expect("open store");
    store
        .insert_event(&EventAggregate::new(consequence.clone(), vec![]))
        .expect("insert consequence");
    drop(store);

    let mut app = open_app(&path);
    let response = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "event".to_owned(),
            objective: Some("Crear derrumbe con causalidad".to_owned()),
            source_uris: vec![ObjectRef::Event(consequence.id()).to_string()],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "collapse".to_owned()),
                ("summary".to_owned(), "Derrumbe de la mina".to_owned()),
                ("time_kind".to_owned(), "instant".to_owned()),
                ("time_precision".to_owned(), "exact".to_owned()),
                ("time_certainty".to_owned(), "certain".to_owned()),
                ("start_tick".to_owned(), "10".to_owned()),
                (
                    "causal_links".to_owned(),
                    format!("{}|causes", ObjectRef::Event(consequence.id())),
                ),
            ]),
        })
        .expect("preview event draft");

    assert!(response.field_issues.is_empty());
    let draft = response.draft.expect("event draft preview");
    assert_eq!(draft.object_type, "event");
    assert_eq!(draft.mode, "create");
    assert!(draft.ready_to_confirm);
    assert!(draft.validation_report.errors.is_empty());

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn calendar_configuration_and_exact_date_input_flow_through_review() {
    let path = project_path("manual-form-calendar");
    let world = base_world(&path);
    let mut app = open_app(&path);
    let world_uri = ObjectRef::World(world.id()).to_string();
    let calendar = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "world".to_owned(),
            objective: Some("Configurar calendario fijo".to_owned()),
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: Some(world_uri.clone()),
            values: BTreeMap::from([
                ("name".to_owned(), world.name().to_owned()),
                ("premise_md".to_owned(), world.premise_md().to_owned()),
                ("epoch_label".to_owned(), world.epoch_label().to_owned()),
                ("calendar_mode".to_owned(), "fixed".to_owned()),
                ("calendar_name".to_owned(), "Imperial".to_owned()),
                ("calendar_epoch_tick".to_owned(), "100".to_owned()),
                ("calendar_ticks_per_day".to_owned(), "10".to_owned()),
                (
                    "calendar_weekdays".to_owned(),
                    "First\nSecond\nThird".to_owned(),
                ),
                ("calendar_months".to_owned(), "Ash|2\nRain|3".to_owned()),
            ]),
        })
        .expect("calendar preview");
    assert!(calendar.field_issues.is_empty());
    let review = calendar.review.expect("calendar review");
    assert!(review.ready_to_confirm);
    app.confirm_stored_manual_review(&review.review_key)
        .expect("commit calendar");
    let configured = app.get_current_world().expect("session").expect("world");
    assert_eq!(
        configured.world.calendar().expect("calendar").name(),
        "Imperial"
    );

    let event = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "event".to_owned(),
            objective: Some("Crear evento por fecha".to_owned()),
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "festival".to_owned()),
                ("summary".to_owned(), "Festival de la lluvia".to_owned()),
                ("time_kind".to_owned(), "instant".to_owned()),
                ("time_precision".to_owned(), "exact".to_owned()),
                ("time_certainty".to_owned(), "certain".to_owned()),
                ("start_calendar_date".to_owned(), "0|2|1|0".to_owned()),
            ]),
        })
        .expect("event date preview");
    assert!(event.field_issues.is_empty());
    let event_review = event.review.expect("event review");
    app.confirm_stored_manual_review(&event_review.review_key)
        .expect("commit dated event");
    let timeline = app.list_timeline_events().expect("timeline");
    let festival = timeline
        .known
        .iter()
        .find(|entry| entry.summary == "Festival de la lluvia")
        .expect("festival");
    assert_eq!(festival.time.start_tick(), Some(120));
    assert!(
        festival
            .start_calendar
            .as_ref()
            .expect("calendar label")
            .label
            .contains("Rain 1")
    );
    let citation = app
        .open_uri(&event_review.review_key)
        .expect("open cited event");
    assert!(citation.result.snippet.contains("Imperial"));
    assert!(citation.result.snippet.contains("tick 120"));

    let invalid = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "event".to_owned(),
            objective: None,
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "festival".to_owned()),
                ("summary".to_owned(), "Fecha imposible".to_owned()),
                ("time_kind".to_owned(), "instant".to_owned()),
                ("time_precision".to_owned(), "exact".to_owned()),
                ("time_certainty".to_owned(), "certain".to_owned()),
                ("start_calendar_date".to_owned(), "0|2|4|0".to_owned()),
            ]),
        })
        .expect("invalid date response");
    assert!(invalid.draft.is_none());
    assert!(
        invalid
            .field_issues
            .iter()
            .any(|issue| issue.field == "start_calendar_date")
    );

    app.close_world().expect("close");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn previewing_manual_document_update_supports_content_reference_reordering() {
    let path = project_path("manual-form-document-references");
    let world = base_world(&path);
    let chronicler = Entity::new(
        world.id(),
        EntityKind::Person,
        "Archivist",
        "archivist",
        "",
        "",
        "{}",
        vec![],
        2,
    )
    .expect("chronicler");
    let collapse = Event::new(
        world.id(),
        "collapse",
        "Mine collapse",
        "",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        3,
    )
    .expect("collapse");
    let aftermath = Event::new(
        world.id(),
        "aftermath",
        "Ash settled over the tunnels",
        "",
        EventTime::ongoing(12, TimePrecision::Day, Certainty::Approximate),
        None,
        vec![],
        vec![],
        4,
    )
    .expect("aftermath");
    let chronicle = Document::new(
        world.id(),
        "Mine Chronicle",
        "chronicle",
        Some(chronicler.id()),
        Some(chronicler.id()),
        DocumentCanonStatus::Canonical,
        "Original order.",
        5,
    )
    .expect("document");

    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&chronicler).expect("insert chronicler");
    store
        .insert_event(&EventAggregate::new(collapse.clone(), vec![]))
        .expect("insert collapse");
    store
        .insert_event(&EventAggregate::new(aftermath.clone(), vec![]))
        .expect("insert aftermath");
    store
        .insert_document(&nirmata_store::DocumentAggregate::new(
            chronicle.clone(),
            vec![
                ContentReference::new(
                    ObjectRef::Document(chronicle.id()),
                    ObjectRef::Event(collapse.id()),
                    0,
                ),
                ContentReference::new(
                    ObjectRef::Document(chronicle.id()),
                    ObjectRef::Event(aftermath.id()),
                    1,
                ),
            ],
        ))
        .expect("insert chronicle");
    drop(store);

    let mut app = open_app(&path);
    let response = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "document".to_owned(),
            objective: Some("Reordenar el discurso".to_owned()),
            source_uris: vec![ObjectRef::Document(chronicle.id()).to_string()],
            assumptions: vec!["El ordinal no altera EventTime.".to_owned()],
            existing_uri: Some(ObjectRef::Document(chronicle.id()).to_string()),
            values: BTreeMap::from([
                ("title".to_owned(), chronicle.title().to_owned()),
                ("kind".to_owned(), chronicle.kind().to_owned()),
                (
                    "author_entity".to_owned(),
                    ObjectRef::Entity(chronicler.id()).to_string(),
                ),
                (
                    "perspective_entity".to_owned(),
                    ObjectRef::Entity(chronicler.id()).to_string(),
                ),
                ("canon_status".to_owned(), "canonical".to_owned()),
                (
                    "body_md".to_owned(),
                    "The chronicle now starts from the aftermath.".to_owned(),
                ),
                (
                    "content_references".to_owned(),
                    format!(
                        "{}|0\n{}|1",
                        ObjectRef::Event(aftermath.id()),
                        ObjectRef::Event(collapse.id())
                    ),
                ),
            ]),
        })
        .expect("preview document draft");

    assert!(response.field_issues.is_empty());
    assert!(
        response
            .draft
            .as_ref()
            .is_some_and(|draft| !draft.ready_to_confirm)
    );
    let review = response.review.expect("review snapshot");
    assert_eq!(review.operations.len(), 1);
    assert!(review.operations[0].risk.requires_judgment);
    assert_eq!(
        review.operations[0]
            .after
            .as_ref()
            .expect("after snapshot")
            .lines[2]
            .value,
        "2"
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn editing_a_stored_review_operation_reuses_the_manual_form_and_revalidates() {
    let path = project_path("manual-form-review-edit");
    let world = base_world(&path);
    let mara = person(world.id(), "Mara", "mara", 2);
    let vale = person(world.id(), "Vale", "vale", 3);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    store.insert_entity(&vale).expect("insert Vale");
    drop(store);

    let mut app = open_app(&path);
    let revision_before = app
        .get_current_world()
        .expect("session")
        .expect("world")
        .current_revision;
    let review = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "entity".to_owned(),
            objective: Some("Rename Mara".to_owned()),
            source_uris: vec![
                ObjectRef::Entity(mara.id()).to_string(),
                ObjectRef::Entity(vale.id()).to_string(),
            ],
            assumptions: vec![],
            existing_uri: Some(ObjectRef::Entity(mara.id()).to_string()),
            values: BTreeMap::from([
                ("kind".to_owned(), "person".to_owned()),
                ("name".to_owned(), "Mara Vale".to_owned()),
                ("slug".to_owned(), "vale".to_owned()),
                ("aliases".to_owned(), String::new()),
                ("summary".to_owned(), String::new()),
                ("body_md".to_owned(), String::new()),
                ("attributes_json".to_owned(), "{}".to_owned()),
            ]),
        })
        .expect("preview draft")
        .review
        .expect("review snapshot");
    assert!(!review.ready_to_confirm);

    let operation_id =
        ChangeOperationId::from_str(&review.operations[0].operation_id).expect("operation id");
    let mut request = app
        .begin_stored_manual_review_edit(&review.review_key, operation_id)
        .expect("begin edit");
    assert_eq!(request.values.get("slug").expect("slug"), "vale");
    request
        .values
        .insert("slug".to_owned(), "mara-vale".to_owned());

    let response = app
        .apply_stored_manual_review_edit(&review.review_key, operation_id, request)
        .expect("apply review edit");
    assert!(response.field_issues.is_empty());
    let updated = response.review.expect("updated review");
    assert_eq!(updated.review_key, review.review_key);
    assert_eq!(updated.operations.len(), review.operations.len());
    assert_eq!(
        updated.operations[0].operation_id,
        review.operations[0].operation_id
    );
    assert!(updated.ready_to_confirm);
    assert_eq!(updated.operations[0].decision, "edit");
    assert!(
        updated
            .validation_report
            .errors
            .iter()
            .all(|issue| issue.code != "change_set.entity.duplicate_slug")
    );
    let stored = app
        .read_stored_manual_review(&review.review_key)
        .expect("stored edited review");
    assert_eq!(
        stored.operations[0].operation_id,
        review.operations[0].operation_id
    );
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .current_revision,
        revision_before,
        "editing a review cannot change canon"
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn discarding_a_stored_manual_review_releases_its_key() {
    let path = project_path("manual-form-discard-key");
    let world = base_world(&path);
    let mara = person(world.id(), "Mara", "mara", 2);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    drop(store);
    let mut app = open_app(&path);
    let request = ManualDraftRequest {
        object_type: "entity".to_owned(),
        objective: Some("Rename Mara".to_owned()),
        source_uris: vec![],
        assumptions: vec![],
        existing_uri: Some(ObjectRef::Entity(mara.id()).to_string()),
        values: BTreeMap::from([
            ("kind".to_owned(), "person".to_owned()),
            ("name".to_owned(), "Mara Vale".to_owned()),
            ("slug".to_owned(), "mara-vale".to_owned()),
            ("aliases".to_owned(), String::new()),
            ("summary".to_owned(), String::new()),
            ("body_md".to_owned(), String::new()),
            ("attributes_json".to_owned(), "{}".to_owned()),
        ]),
    };
    let revision_before = app
        .get_current_world()
        .expect("session")
        .expect("world")
        .current_revision;
    let first = app
        .preview_manual_draft(request.clone())
        .expect("first preview")
        .review
        .expect("first review");
    assert!(matches!(
        app.preview_manual_draft(request.clone()),
        Err(AppError::ReviewSessionConflict(_))
    ));
    app.discard_stored_manual_review(&first.review_key)
        .expect("discard review");
    assert!(matches!(
        app.read_stored_manual_review(&first.review_key),
        Err(AppError::ReviewSessionNotFound(_))
    ));
    let second = app
        .preview_manual_draft(request)
        .expect("second preview")
        .review
        .expect("second review");
    assert_eq!(second.review_key, first.review_key);
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .current_revision,
        revision_before
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn stale_stored_reviews_cannot_be_confirmed_until_revalidated() {
    let path = project_path("manual-form-review-stale");
    let world = base_world(&path);
    let mut app = open_app(&path);

    let review = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "entity".to_owned(),
            objective: Some("Create Mara".to_owned()),
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "person".to_owned()),
                ("name".to_owned(), "Mara".to_owned()),
                ("slug".to_owned(), "mara".to_owned()),
                ("aliases".to_owned(), String::new()),
                ("summary".to_owned(), String::new()),
                ("body_md".to_owned(), String::new()),
                ("attributes_json".to_owned(), "{}".to_owned()),
            ]),
        })
        .expect("preview draft")
        .review
        .expect("review snapshot");
    assert!(review.ready_to_confirm);

    commit_entity_create(&mut app, world.id(), "Vale", "vale", 4);

    let stale = app
        .read_stored_manual_review(&review.review_key)
        .expect("read stale review");
    assert_eq!(stale.freshness.status, ManualReviewFreshnessStatus::Stale);
    assert!(!stale.ready_to_confirm);
    assert!(matches!(
        app.confirm_stored_manual_review(&review.review_key),
        Err(AppError::ManualReviewStale { .. })
    ));
    let preserved = app
        .read_stored_manual_review(&review.review_key)
        .expect("review remains available after failed confirm");
    assert_eq!(preserved.review_key, review.review_key);
    assert_eq!(preserved.base_revision, review.base_revision);

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn preview_manual_draft_rejects_invalid_source_uris() {
    let path = project_path("manual-form-invalid-source-uri");
    base_world(&path);
    let mut app = open_app(&path);

    let response = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "entity".to_owned(),
            objective: Some("Create Mara".to_owned()),
            source_uris: vec!["nirmata://entity/not-a-uuid".to_owned()],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "person".to_owned()),
                ("name".to_owned(), "Mara".to_owned()),
                ("slug".to_owned(), "mara".to_owned()),
                ("aliases".to_owned(), String::new()),
                ("summary".to_owned(), String::new()),
                ("body_md".to_owned(), String::new()),
                ("attributes_json".to_owned(), "{}".to_owned()),
            ]),
        })
        .expect("preview draft");

    assert!(response.draft.is_none());
    assert!(
        response
            .field_issues
            .iter()
            .any(|issue| issue.field == "sourceUris")
    );

    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn a_second_head_change_requires_restarting_manual_review_revalidation() {
    let path = project_path("manual-form-review-refresh-restart");
    let world = base_world(&path);
    let mut app = open_app(&path);

    let review = app
        .preview_manual_draft(ManualDraftRequest {
            object_type: "entity".to_owned(),
            objective: Some("Create Mara".to_owned()),
            source_uris: vec![],
            assumptions: vec![],
            existing_uri: None,
            values: BTreeMap::from([
                ("kind".to_owned(), "person".to_owned()),
                ("name".to_owned(), "Mara".to_owned()),
                ("slug".to_owned(), "mara".to_owned()),
                ("aliases".to_owned(), String::new()),
                ("summary".to_owned(), String::new()),
                ("body_md".to_owned(), String::new()),
                ("attributes_json".to_owned(), "{}".to_owned()),
            ]),
        })
        .expect("preview draft")
        .review
        .expect("review snapshot");

    commit_entity_create(&mut app, world.id(), "Vale", "vale", 4);
    let stale = app
        .read_stored_manual_review(&review.review_key)
        .expect("read stale review");
    assert_eq!(stale.freshness.status, ManualReviewFreshnessStatus::Stale);

    commit_entity_create(&mut app, world.id(), "Talia", "talia", 5);
    let interrupted = app
        .revalidate_stored_manual_review(&review.review_key)
        .expect("revalidation interrupted");
    assert_eq!(
        interrupted.freshness.status,
        ManualReviewFreshnessStatus::RefreshRestartRequired
    );
    assert!(!interrupted.ready_to_confirm);

    let refreshed = app
        .revalidate_stored_manual_review(&review.review_key)
        .expect("revalidate after restart");
    assert_eq!(
        refreshed.freshness.status,
        ManualReviewFreshnessStatus::Current
    );
    assert!(refreshed.ready_to_confirm);

    drop(app);
    fs::remove_file(path).expect("remove project");
}
