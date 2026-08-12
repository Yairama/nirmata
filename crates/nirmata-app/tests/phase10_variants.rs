use nirmata_app::{
    AppError, CreateWorldInput, DraftOperationInput, ExportSnapshotInput, ImportSnapshotInput,
    ManualReviewInput, NirmataApp, ReadScope, VariantDiffKind,
};
use nirmata_core::{
    WorldId,
    change_set::RetconKind,
    document::ObjectRef,
    entity::{Entity, EntityKind},
};
use nirmata_store::ResolvedObject;
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
        .expect("clock")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

fn entity(world_id: WorldId, name: &str, slug: &str, now: i64) -> Entity {
    Entity::new(
        world_id,
        EntityKind::Person,
        name,
        slug,
        "",
        "",
        "{}",
        vec![],
        now,
    )
    .expect("entity")
}

fn commit_entity(app: &mut NirmataApp, value: Entity) {
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: format!("Create {}", value.name()),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: value,
            }],
        })
        .expect("start review");
    app.confirm_manual_review(&review).expect("commit entity");
}

fn update_entity(
    app: &mut NirmataApp,
    before: &Entity,
    name: &str,
    slug: &str,
    now: i64,
) -> Entity {
    let after = Entity::restore(
        before.id(),
        before.world_id(),
        before.kind(),
        name,
        slug,
        before.summary().to_owned(),
        before.body_md().to_owned(),
        before.attributes_json().as_str().to_owned(),
        before.aliases().to_vec(),
        before.version() + 1,
        before.created_at_ms(),
        now,
    )
    .expect("updated entity");
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: format!("Rename {}", before.name()),
            sources: vec![ObjectRef::Entity(before.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: before.clone(),
                after: after.clone(),
            }],
        })
        .expect("start update");
    app.confirm_manual_review(&review).expect("commit update");
    after
}

#[test]
fn variants_isolate_heads_history_reopen_stale_and_undo() {
    let path = project_path("phase10-isolation");
    let mut app = NirmataApp::default();
    let created = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let shared = entity(created.world_id, "Shared", "shared", 2);
    commit_entity(&mut app, shared.clone());
    let main = app.get_current_world().expect("session").expect("open");
    let fork_revision = main.current_revision;
    let branch = app
        .create_variant("alternate", fork_revision)
        .expect("create variant");
    app.switch_variant(branch.id).expect("switch branch");
    let branch_only = entity(created.world_id, "Branch only", "branch-only", 3);
    commit_entity(&mut app, branch_only.clone());
    let branch_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;

    app.set_read_scope(ReadScope::historical(branch.id, fork_revision))
        .expect("view fork");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(branch_only.id()).to_string()),
        Err(AppError::ObjectNotFound { .. })
    ));
    assert!(matches!(
        app.start_manual_review(ManualReviewInput {
            objective: "blocked history edit".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![],
        }),
        Err(AppError::ReadOnlyScope)
    ));
    app.view_active_head().expect("return to branch head");

    app.switch_variant(main.active_variant.id)
        .expect("switch main");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(branch_only.id()).to_string()),
        Err(AppError::ObjectNotFound { .. })
    ));
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("open")
            .current_revision,
        fork_revision
    );

    let stale = app
        .start_manual_review(ManualReviewInput {
            objective: "main draft".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: entity(created.world_id, "Draft", "draft", 4),
            }],
        })
        .expect("main draft");
    app.switch_variant(branch.id).expect("switch branch again");
    assert!(matches!(
        app.confirm_manual_review(&stale),
        Err(AppError::ManualReviewVariantMismatch { .. })
    ));
    app.undo_last_commit().expect("undo branch commit");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(branch_only.id()).to_string()),
        Err(AppError::ObjectNotFound { .. })
    ));
    app.close_world().expect("close");

    let reopened = app.open_world(path).expect("reopen");
    assert_eq!(reopened.active_variant.id, branch.id);
    assert_ne!(reopened.current_revision, branch_head);
    app.switch_variant(main.active_variant.id)
        .expect("reopen main");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(shared.id()).to_string())
            .expect("shared entity")
            .object,
        ResolvedObject::Entity(_)
    ));
}

#[test]
fn compare_and_limited_merge_use_ids_and_leave_source_untouched() {
    let path = project_path("phase10-merge");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let branch = app
        .create_variant("alternate", main.current_revision)
        .expect("branch");
    let source_only = entity(main.world_id, "Source", "source", 2);
    commit_entity(&mut app, source_only.clone());
    let source_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
    let exported = app
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory: path.parent().expect("parent").to_path_buf(),
            snapshot_name: format!("cross-{}", source_head),
        })
        .expect("export source snapshot");
    app.switch_variant(branch.id).expect("destination branch");
    assert!(matches!(
        app.import_vfs_snapshot(ImportSnapshotInput {
            snapshot_directory: exported.path.clone(),
        }),
        Err(AppError::InvalidSnapshotImport { .. })
    ));
    let destination_only = entity(main.world_id, "Destination", "destination", 3);
    commit_entity(&mut app, destination_only);
    let destination_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;

    let comparison = app
        .compare_scopes(
            ReadScope::head(main.active_variant.id),
            ReadScope::head(branch.id),
        )
        .expect("compare variants");
    assert!(comparison.differences.iter().any(|difference| {
        difference.object_ref == ObjectRef::Entity(source_only.id())
            && difference.kind == VariantDiffKind::Deleted
            && difference.left_source.is_some()
    }));

    let merge = app
        .prepare_variant_merge(ReadScope::head(main.active_variant.id))
        .expect("prepare merge");
    assert_eq!(merge.automatic_operation_ids.len(), 1);
    assert!(merge.decision_operation_ids.is_empty());
    assert!(merge.review.ready_to_confirm);
    app.confirm_stored_manual_review(&merge.review.review_key)
        .expect("commit merge");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(source_only.id()).to_string())
            .expect("merged entity")
            .object,
        ResolvedObject::Entity(_)
    ));

    app.switch_variant(main.active_variant.id)
        .expect("inspect source");
    let source_after = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
    assert_eq!(source_after, source_head);
    assert_ne!(source_after, destination_head);
    fs::remove_dir_all(exported.path).expect("remove snapshot");
}

#[test]
fn overlapping_rename_is_a_sourced_manual_decision_not_a_slug_match() {
    let path = project_path("phase10-conflict");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let shared = entity(main.world_id, "Mara", "mara", 2);
    commit_entity(&mut app, shared.clone());
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");
    let same_head_draft = app
        .start_manual_review(ManualReviewInput {
            objective: "same revision, different variant".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: entity(main.world_id, "Blocked", "blocked", 3),
            }],
        })
        .expect("same-head draft");
    app.switch_variant(branch.id).expect("switch at same head");
    assert!(matches!(
        app.confirm_manual_review(&same_head_draft),
        Err(AppError::ManualReviewVariantMismatch { .. })
    ));
    app.switch_variant(fork.active_variant.id)
        .expect("return main");
    let source_value = update_entity(&mut app, &shared, "Mara North", "mara-north", 3);
    app.switch_variant(branch.id).expect("switch branch");
    update_entity(&mut app, &shared, "Mara South", "mara-south", 4);

    let comparison = app
        .compare_scopes(
            ReadScope::head(fork.active_variant.id),
            ReadScope::head(branch.id),
        )
        .expect("compare rename");
    let rename = comparison
        .differences
        .iter()
        .find(|difference| difference.object_ref == ObjectRef::Entity(shared.id()))
        .expect("same stable entity differs");
    assert_eq!(rename.kind, VariantDiffKind::Renamed);
    assert!(rename.left_source.is_some());
    assert!(rename.right_source.is_some());

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare conflicting merge");
    assert!(merge.automatic_operation_ids.is_empty());
    assert_eq!(merge.decision_operation_ids.len(), 1);
    assert!(!merge.review.ready_to_confirm);
    assert_eq!(source_value.id(), shared.id());
}
