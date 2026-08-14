use nirmata_app::{
    DraftOperationInput, ManualDraftRequest, ManualReviewFreshnessStatus, ManualReviewInput,
    NirmataApp, PendingReviewOrigin,
};
use nirmata_core::{
    World,
    change_set::RetconKind,
    entity::{Entity, EntityKind},
};
use nirmata_store::WorldStore;
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
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

fn create_world(path: &Path) -> World {
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    WorldStore::create(path, &world).expect("create store");
    world
}

fn create_entity_request(name: &str, slug: &str) -> ManualDraftRequest {
    ManualDraftRequest {
        object_type: "entity".to_owned(),
        objective: Some(format!("Crear {name}")),
        source_uris: vec![],
        assumptions: vec![],
        existing_uri: None,
        values: BTreeMap::from([
            ("kind".to_owned(), "person".to_owned()),
            ("name".to_owned(), name.to_owned()),
            ("slug".to_owned(), slug.to_owned()),
            ("aliases".to_owned(), String::new()),
            ("attributes_json".to_owned(), "{}".to_owned()),
        ]),
    }
}

#[test]
fn manual_review_edit_confirm_and_discard_survive_close_and_reopen() {
    let path = project_path("pending-manual-reopen");
    create_world(&path);
    let mut app = open_app(&path);
    let response = app
        .preview_manual_draft(create_entity_request("Mara", "mara"))
        .expect("preview");
    let review_key = response.review.expect("review").review_key;
    app.close_world().expect("close world");
    app.open_world(path.clone()).expect("reopen world");

    let pending = app.list_pending_reviews().expect("list recovered review");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].origin, PendingReviewOrigin::Manual);
    assert_eq!(pending[0].title, "Mara");
    let operation_id = pending[0].review.operations[0].operation_id.clone();
    app.apply_stored_manual_review_edit(
        &review_key,
        operation_id.parse().expect("operation id"),
        create_entity_request("Mara Editada", "mara-editada"),
    )
    .expect("edit recovered review");
    app.close_world().expect("close after edit");
    app.open_world(path.clone()).expect("reopen edited review");
    assert_eq!(
        app.list_pending_reviews().expect("list edited review")[0].title,
        "Mara Editada"
    );
    app.confirm_stored_manual_review(&review_key)
        .expect("confirm recovered review");
    app.close_world().expect("close after confirm");
    app.open_world(path.clone()).expect("reopen after confirm");
    assert!(
        app.list_pending_reviews()
            .expect("pending after confirm")
            .is_empty()
    );

    let second = app
        .preview_manual_draft(create_entity_request("Sera", "sera"))
        .expect("second preview")
        .review
        .expect("second review")
        .review_key;
    app.close_world().expect("close second review");
    app.open_world(path.clone()).expect("reopen second review");
    app.discard_stored_manual_review(&second)
        .expect("discard recovered review");
    app.close_world().expect("close after discard");
    app.open_world(path.clone()).expect("reopen after discard");
    assert!(
        app.list_pending_reviews()
            .expect("pending after discard")
            .is_empty()
    );
    drop(app);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn pending_reviews_are_variant_isolated_and_become_stale_on_their_own_head() {
    let path = project_path("pending-variant-stale");
    let world = create_world(&path);
    let mut app = open_app(&path);
    app.preview_manual_draft(create_entity_request("Mara", "mara"))
        .expect("preview main review");
    let main = app.get_current_world().expect("session").expect("world");
    let branch = app
        .create_variant("alternate", main.current_revision)
        .expect("create branch");
    app.switch_variant(branch.id).expect("switch branch");
    assert!(
        app.list_pending_reviews()
            .expect("branch pending")
            .is_empty()
    );
    app.switch_variant(main.active_variant.id)
        .expect("switch main");
    assert_eq!(app.list_pending_reviews().expect("main pending").len(), 1);

    let advance = Entity::new(
        world.id(),
        EntityKind::Concept,
        "Avance",
        "avance",
        "",
        "",
        "{}",
        vec![],
        3,
    )
    .expect("advance entity");
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Avanzar cabeza".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: advance,
            }],
        })
        .expect("advance review");
    app.confirm_manual_review(&review).expect("advance head");
    let stale = app.list_pending_reviews().expect("stale pending");
    assert_eq!(
        stale[0].review.freshness.status,
        ManualReviewFreshnessStatus::Stale
    );
    assert!(stale[0].review.freshness.can_revalidate);
    drop(app);

    let mut reopened = open_app(&path);
    let stale = reopened
        .list_pending_reviews()
        .expect("reopened stale pending");
    assert_eq!(stale.len(), 1);
    assert_eq!(
        stale[0].review.freshness.status,
        ManualReviewFreshnessStatus::Stale
    );
    drop(reopened);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn corrupted_pending_payload_fails_open_without_activating_the_world() {
    let path = project_path("pending-corrupt");
    create_world(&path);
    let mut app = open_app(&path);
    app.preview_manual_draft(create_entity_request("Mara", "mara"))
        .expect("preview");
    app.close_world().expect("close world");
    let connection = Connection::open(&path).expect("open sqlite");
    connection
        .execute("UPDATE pending_reviews SET payload_json = '{}'", [])
        .expect("corrupt typed payload");
    drop(connection);

    let error = app
        .open_world(path.clone())
        .expect_err("corruption must fail open");
    assert!(error.to_string().contains("invalid typed data"));
    assert!(app.get_current_world().expect("inactive app").is_none());
    drop(app);
    fs::remove_file(path).expect("remove project");
}
