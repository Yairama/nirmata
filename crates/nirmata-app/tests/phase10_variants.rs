use nirmata_app::{
    AppError, CreateWorldInput, DraftOperationInput, ExportSnapshotInput, ImportSnapshotInput,
    ManualReviewActionRequest, ManualReviewInput, NirmataApp, ReadScope, VariantDiffKind,
};
use nirmata_core::{
    Period, WorldId,
    change_set::RetconKind,
    claim::{Claim, ClaimAuthentication, ClaimObject, ClaimPolarity},
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

fn commit_claim(app: &mut NirmataApp, value: Claim) {
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Create canonical claim".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateClaim {
                retcon: RetconKind::Additive,
                after: value,
            }],
        })
        .expect("start claim review");
    app.confirm_manual_review(&review).expect("commit claim");
}

fn canonical_claim(
    world_id: WorldId,
    revision_id: nirmata_core::RevisionId,
    subject_id: nirmata_core::EntityId,
    polarity: ClaimPolarity,
) -> Claim {
    Claim::new(
        world_id,
        subject_id,
        "gate state",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        polarity,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(Period::new(Some(10), Some(10)).expect("period")),
        revision_id,
    )
    .expect("canonical claim")
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

fn delete_entity(app: &mut NirmataApp, before: &Entity) {
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: format!("Delete {}", before.name()),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::DeleteEntity {
                retcon: RetconKind::Replacement,
                before: before.clone(),
            }],
        })
        .expect("start delete");
    let operation_id = review.operations()[0].operation_id();
    let decision_point_id = review.original_draft().decisions()[0].decision_point_id();
    app.apply_manual_review_action(
        &mut review,
        nirmata_app::ManualReviewAction::RecordJudgment {
            operation_id,
            judgment: "Remove this object from the source variant.".to_owned(),
        },
    )
    .expect("record delete judgment");
    app.apply_manual_review_action(
        &mut review,
        nirmata_app::ManualReviewAction::ResolveDecision {
            decision_point_id,
            alternative: "Apply replacement".to_owned(),
        },
    )
    .expect("resolve delete");
    assert!(
        review.ready_to_confirm(),
        "validation={:?} effective={:?} decisions={:?}",
        review.validation_report(),
        review.effective_report(),
        review.original_draft().decisions()
    );
    app.confirm_manual_review(&review).expect("commit delete");
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

#[test]
fn keep_destination_rejects_only_the_conflicting_source_operation() {
    let path = project_path("phase10-keep-destination");
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

    update_entity(&mut app, &shared, "Mara North", "mara-north", 3);
    let source_only = entity(main.world_id, "Source only", "source-only", 4);
    commit_entity(&mut app, source_only.clone());
    app.switch_variant(branch.id).expect("switch destination");
    update_entity(&mut app, &shared, "Mara South", "mara-south", 5);

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare merge");
    assert_eq!(merge.automatic_operation_ids.len(), 1);
    assert_eq!(merge.decision_operation_ids.len(), 1);
    let conflicting = merge
        .review
        .operations
        .iter()
        .find(|operation| operation.target_uri == ObjectRef::Entity(shared.id()).to_string())
        .expect("conflicting operation");
    let decision = conflicting.decision_points.first().expect("merge decision");
    let updated = app
        .apply_stored_manual_review_action(
            &merge.review.review_key,
            ManualReviewActionRequest::ResolveDecision {
                decision_point_id: decision.decision_point_id.clone(),
                alternative: "keep_destination".to_owned(),
            },
        )
        .expect("keep destination");
    let conflicting = updated
        .operations
        .iter()
        .find(|operation| operation.target_uri == ObjectRef::Entity(shared.id()).to_string())
        .expect("conflicting operation");
    assert!(!conflicting.selected);
    assert_eq!(conflicting.decision, "reject");
    assert!(updated.ready_to_confirm);

    app.confirm_stored_manual_review(&merge.review.review_key)
        .expect("commit independent source change");
    let ResolvedObject::Entity(destination_shared) = app
        .open_uri(&ObjectRef::Entity(shared.id()).to_string())
        .expect("destination entity")
        .object
    else {
        panic!("expected entity");
    };
    assert_eq!(destination_shared.name(), "Mara South");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(source_only.id()).to_string())
            .expect("independent source entity")
            .object,
        ResolvedObject::Entity(_)
    ));
}

#[test]
fn take_source_keeps_and_applies_the_conflicting_operation() {
    let path = project_path("phase10-take-source");
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
    update_entity(&mut app, &shared, "Mara North", "mara-north", 3);
    app.switch_variant(branch.id).expect("switch destination");
    update_entity(&mut app, &shared, "Mara South", "mara-south", 4);

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare merge");
    let operation = merge.review.operations.first().expect("merge operation");
    let decision = operation.decision_points.first().expect("merge decision");
    let updated = app
        .apply_stored_manual_review_action(
            &merge.review.review_key,
            ManualReviewActionRequest::ResolveDecision {
                decision_point_id: decision.decision_point_id.clone(),
                alternative: "take_source".to_owned(),
            },
        )
        .expect("take source");
    assert!(updated.operations.first().expect("operation").selected);
    assert!(updated.ready_to_confirm);
    app.confirm_stored_manual_review(&merge.review.review_key)
        .expect("commit source value");
    let ResolvedObject::Entity(destination_shared) = app
        .open_uri(&ObjectRef::Entity(shared.id()).to_string())
        .expect("destination entity")
        .object
    else {
        panic!("expected entity");
    };
    assert_eq!(destination_shared.name(), "Mara North");
}

#[test]
fn revision_history_follows_only_the_observed_variant_lineage() {
    let path = project_path("phase10-history-lineage");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    commit_entity(&mut app, entity(main.world_id, "Shared", "shared", 2));
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");
    commit_entity(&mut app, entity(main.world_id, "Main only", "main-only", 3));
    let main_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
    app.switch_variant(branch.id).expect("switch branch");
    commit_entity(
        &mut app,
        entity(main.world_id, "Branch only", "branch-only", 4),
    );
    let branch_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;

    app.set_read_scope(ReadScope::head(fork.active_variant.id))
        .expect("observe main");
    let main_history = app.list_revision_history().expect("main history");
    assert_eq!(main_history.current_head_revision_id, main_head.to_string());
    assert!(main_history.undo_target_revision_id.is_none());
    assert!(
        main_history
            .revisions
            .iter()
            .any(|revision| revision.revision_id == main_head.to_string())
    );
    assert!(
        main_history
            .revisions
            .iter()
            .all(|revision| revision.revision_id != branch_head.to_string())
    );

    app.view_active_head().expect("return to branch head");
    let branch_history = app.list_revision_history().expect("branch history");
    assert_eq!(
        branch_history.current_head_revision_id,
        branch_head.to_string()
    );
    assert!(branch_history.undo_target_revision_id.is_some());
    assert!(
        branch_history
            .revisions
            .iter()
            .any(|revision| revision.revision_id == fork.current_revision.to_string())
    );
    assert!(
        branch_history
            .revisions
            .iter()
            .all(|revision| revision.revision_id != main_head.to_string())
    );
}

#[test]
fn opposing_canonical_claim_requires_a_manual_merge_decision() {
    let path = project_path("phase10-opposing-claim");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let subject = entity(main.world_id, "Gate", "gate", 2);
    commit_entity(&mut app, subject.clone());
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");

    commit_claim(
        &mut app,
        canonical_claim(
            main.world_id,
            fork.current_revision,
            subject.id(),
            ClaimPolarity::Positive,
        ),
    );
    let source_only = entity(main.world_id, "Source only", "source-only", 3);
    commit_entity(&mut app, source_only.clone());
    app.switch_variant(branch.id).expect("switch destination");
    commit_claim(
        &mut app,
        canonical_claim(
            main.world_id,
            fork.current_revision,
            subject.id(),
            ClaimPolarity::Negative,
        ),
    );
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(subject.id()).to_string())
            .expect("destination subject")
            .object,
        ResolvedObject::Entity(_)
    ));

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare merge");
    assert_eq!(merge.automatic_operation_ids.len(), 1);
    assert_eq!(merge.decision_operation_ids.len(), 1);
    assert!(
        merge
            .review
            .validation_report
            .conflicts
            .iter()
            .any(|issue| issue.code == "claim.canonical_opposition"),
        "{:?}",
        merge.review.validation_report
    );
    let claim_operation = merge
        .review
        .operations
        .iter()
        .find(|operation| operation.target_uri.starts_with("nirmata://claim/"))
        .expect("claim operation");
    let decision = claim_operation
        .decision_points
        .first()
        .expect("claim decision");
    let updated = app
        .apply_stored_manual_review_action(
            &merge.review.review_key,
            ManualReviewActionRequest::ResolveDecision {
                decision_point_id: decision.decision_point_id.clone(),
                alternative: "keep_destination".to_owned(),
            },
        )
        .expect("keep destination claim");
    assert!(updated.ready_to_confirm);
    assert!(updated.effective_report.conflicts.is_empty());
    app.confirm_stored_manual_review(&merge.review.review_key)
        .expect("commit independent operation");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(source_only.id()).to_string())
            .expect("source entity merged")
            .object,
        ResolvedObject::Entity(_)
    ));
}

#[test]
fn source_delete_is_a_reviewed_replacement_and_can_remove_destination_canon() {
    let path = project_path("phase10-delete-merge");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let shared = entity(main.world_id, "Obsolete", "obsolete", 2);
    commit_entity(&mut app, shared.clone());
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");
    delete_entity(&mut app, &shared);
    app.switch_variant(branch.id).expect("switch destination");

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare delete merge");
    assert!(merge.automatic_operation_ids.is_empty());
    assert_eq!(merge.decision_operation_ids.len(), 1);
    assert!(
        merge
            .review
            .validation_report
            .errors
            .iter()
            .all(|issue| { issue.code != "change_set.retcon.additive_delete" })
    );
    let operation = merge.review.operations.first().expect("delete operation");
    assert!(operation.risk.requires_judgment);
    let decision = operation.decision_points.first().expect("delete decision");
    let review_key = merge.review.review_key.clone();
    app.apply_stored_manual_review_action(
        &review_key,
        ManualReviewActionRequest::RecordJudgment {
            operation_id: operation.operation_id.clone(),
            judgment: "Apply the reviewed source deletion.".to_owned(),
        },
    )
    .expect("record merge judgment");
    let updated = app
        .apply_stored_manual_review_action(
            &review_key,
            ManualReviewActionRequest::ResolveDecision {
                decision_point_id: decision.decision_point_id.clone(),
                alternative: "take_source".to_owned(),
            },
        )
        .expect("take source deletion");
    assert!(updated.ready_to_confirm);
    app.confirm_stored_manual_review(&review_key)
        .expect("commit merge deletion");
    assert!(matches!(
        app.open_uri(&ObjectRef::Entity(shared.id()).to_string()),
        Err(AppError::ObjectNotFound { .. })
    ));
}
