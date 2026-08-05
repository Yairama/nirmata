use nirmata_app::{AppError, ContextBundleRequest, ContextIntent, NirmataApp};
use nirmata_core::{
    World,
    change_set::{ChangeOperation, ChangeSet, ChangeSetDraft, RetconKind},
    document::ObjectRef,
    entity::{Entity, EntityKind},
};
use nirmata_store::{
    CommittedChangeSetRecord, OperationAudit, OperationDecision, StoredRevision, WorldStore,
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

fn commit_external_entity(
    path: &Path,
    name: &str,
    slug: &str,
    committed_at_ms: i64,
) -> nirmata_core::RevisionId {
    let mut store = WorldStore::open(path).expect("open store");
    let world = store.load_world().expect("load world");
    let after = Entity::new(
        world.id(),
        EntityKind::Person,
        name,
        slug,
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("entity");
    let operation = ChangeOperation::CreateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(after.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after,
    };
    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        format!("Create {name}"),
        vec![],
        vec![],
        vec![operation.clone()],
        vec![],
    )
    .expect("change set");
    let revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(change_set.id()),
        "external_test",
        format!("Create {name}"),
        committed_at_ms,
    )
    .expect("revision");
    let revision_id = revision.id();
    store
        .commit_change_set(
            &CommittedChangeSetRecord::new(
                change_set,
                None,
                vec![],
                vec![
                    OperationAudit::from_operation(
                        &operation,
                        OperationDecision::Accept,
                        "external_test",
                        committed_at_ms,
                    )
                    .expect("audit"),
                ],
                revision,
                None,
            )
            .expect("record"),
        )
        .expect("commit change set");
    revision_id
}

#[test]
fn critique_input_keeps_sources_on_base_revision_and_rejects_stale_drafts() {
    let path = project_path("ai-capabilities");
    let world = base_world(&path);
    let mut store = WorldStore::open(&path).expect("open store");
    let mara = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Harbor cartographer.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("mara");
    store.insert_entity(&mara).expect("insert Mara");
    drop(store);

    let proposed = Entity::new(
        world.id(),
        EntityKind::Faction,
        "North Watch",
        "north-watch",
        "Keeps the upper gate.",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("proposed entity");
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Create the North Watch",
        vec![ObjectRef::Entity(mara.id())],
        vec![],
        vec![ChangeOperation::CreateEntity {
            operation_id: nirmata_core::ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(proposed.id())],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: proposed,
        }],
        vec![],
    )
    .expect("draft");

    let mut app = open_app(&path);
    let critique = app
        .prepare_ai_critique(
            "Check continuity",
            &draft,
            &ContextBundleRequest::new(ContextIntent::ContradictionCheck),
        )
        .expect("prepare critique");

    assert_eq!(critique.mode, nirmata_app::AiMode::Critic);
    assert_eq!(critique.snapshot.base_revision, draft.base_revision());
    assert!(
        critique
            .snapshot
            .context
            .contains(ObjectRef::Entity(mara.id()))
    );
    assert!(
        critique
            .context_object_ids
            .contains(&format!("nirmata://entity/{}", mara.id()))
    );

    let serialized = serde_json::to_string(&critique).expect("serialize critique input");
    assert!(serialized.contains(&draft.base_revision().to_string()));
    assert!(!serialized.contains(path.to_string_lossy().as_ref()));

    let current_revision = commit_external_entity(&path, "Iven", "iven", 3);
    let error = app
        .prepare_ai_critique(
            "Check continuity",
            &draft,
            &ContextBundleRequest::new(ContextIntent::ContradictionCheck),
        )
        .expect_err("stale draft must be rejected");
    assert!(matches!(
        error,
        AppError::AiBaseRevisionMismatch {
            draft_base_revision,
            current_revision: observed,
        } if draft_base_revision == draft.base_revision() && observed == current_revision
    ));

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}
