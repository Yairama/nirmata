use nirmata_app::{
    AppError, ContextBundleRequest, ContextIntent, CreateWorldInput, DraftOperationInput,
    ExportSnapshotInput, ImportSnapshotInput, ManualReviewActionRequest, ManualReviewInput,
    NirmataApp, PendingReviewOrigin, ReadScope, RelatedContextRequest, SearchWorldRequest,
    VariantDiffKind,
};
use nirmata_core::{
    Period, World, WorldId,
    calendar::{CalendarMonth, WorldCalendar},
    change_set::RetconKind,
    claim::{Claim, ClaimAuthentication, ClaimObject, ClaimPolarity},
    document::ObjectRef,
    entity::{Entity, EntityKind},
    event::{Event, EventAggregate, EventParticipant},
    relation::{Relation, RelationDirection},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{ResolvedObject, StructuredSearchQuery, WorldStore};
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

fn commit_event(app: &mut NirmataApp, value: EventAggregate) {
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Create event".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEvent {
                retcon: RetconKind::Additive,
                after: value,
            }],
        })
        .expect("start event review");
    app.confirm_manual_review(&review).expect("commit event");
}

fn commit_relation(app: &mut NirmataApp, value: Relation) {
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Create relation".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateRelation {
                retcon: RetconKind::Additive,
                after: value,
            }],
        })
        .expect("start relation review");
    app.confirm_manual_review(&review).expect("commit relation");
}

fn participant_event(
    world_id: WorldId,
    entity_id: nirmata_core::EntityId,
    kind: &str,
    summary: &str,
    tick: i64,
    role: &str,
    now: i64,
) -> EventAggregate {
    EventAggregate::new(
        Event::new(
            world_id,
            kind,
            summary,
            "",
            EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
            None,
            vec![EventParticipant::new(entity_id, role, 0).expect("participant")],
            vec![],
            now,
        )
        .expect("event"),
        vec![],
    )
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

fn update_world_premise(app: &mut NirmataApp, premise: &str) {
    let session = app
        .get_current_world()
        .expect("session")
        .expect("open world");
    let before = session.world;
    let after = World::restore(
        before.id(),
        before.name(),
        premise,
        before.epoch_label(),
        before.calendar().cloned(),
        before.current_revision(),
        before.created_at_ms(),
        before.updated_at_ms() + 1,
    )
    .expect("updated world");
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Update world premise".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateWorld {
                retcon: RetconKind::Additive,
                before,
                after,
            }],
        })
        .expect("world review");
    app.confirm_manual_review(&review)
        .expect("commit world update");
}

fn update_world_calendar(app: &mut NirmataApp, calendar: Option<WorldCalendar>) {
    let session = app
        .get_current_world()
        .expect("session")
        .expect("open world");
    let before = session.world;
    let after = World::restore(
        before.id(),
        before.name(),
        before.premise_md(),
        before.epoch_label(),
        calendar,
        before.current_revision(),
        before.created_at_ms(),
        before.updated_at_ms() + 1,
    )
    .expect("updated calendar");
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Update world calendar".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateWorld {
                retcon: RetconKind::Reinterpretive,
                before,
                after,
            }],
        })
        .expect("calendar review");
    app.confirm_manual_review(&review).expect("commit calendar");
}

fn fixed_calendar(name: &str, epoch_tick: i64) -> WorldCalendar {
    WorldCalendar::new(
        name,
        epoch_tick,
        10,
        vec!["First".to_owned(), "Second".to_owned(), "Third".to_owned()],
        vec![
            CalendarMonth::new("Ash", 2).expect("month"),
            CalendarMonth::new("Rain", 3).expect("month"),
        ],
    )
    .expect("calendar")
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
    let summaries = app.list_variant_summaries().expect("variant summaries");
    let branch_summary = summaries
        .iter()
        .find(|summary| summary.variant.id == branch.id)
        .expect("branch summary");
    assert_eq!(
        branch_summary.origin_variant_name.as_deref(),
        Some(main.active_variant.name.as_str())
    );
    assert_eq!(branch_summary.origin_summary, "Create Shared");
    assert_eq!(branch_summary.latest_summary, "Create Branch only");
    assert!(branch_summary.latest_created_at_ms >= branch_summary.origin_created_at_ms);

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
    let source_difference = comparison
        .differences
        .iter()
        .find(|difference| difference.object_ref == ObjectRef::Entity(source_only.id()))
        .expect("source-only difference");
    assert_eq!(source_difference.kind, VariantDiffKind::Deleted);
    let provenance = source_difference
        .left_source
        .as_ref()
        .expect("audited source provenance");
    assert_eq!(provenance.revision_id, source_head);
    assert_eq!(provenance.retcon, RetconKind::Additive);
    assert_eq!(
        provenance.scope,
        ReadScope::historical(main.active_variant.id, source_head)
    );

    let merge = app
        .prepare_variant_merge(ReadScope::head(main.active_variant.id))
        .expect("prepare merge");
    assert_eq!(merge.automatic_operation_ids.len(), 1);
    assert!(merge.decision_operation_ids.is_empty());
    assert!(merge.review.ready_to_confirm);
    app.close_world().expect("close pending merge");
    app.open_world(path.clone()).expect("reopen pending merge");
    let pending = app.list_pending_reviews().expect("recovered merge review");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].origin, PendingReviewOrigin::VersionsMerge);
    assert!(pending[0].merge.is_some());
    app.confirm_stored_manual_review(&merge.review.review_key)
        .expect("commit merge");
    let merged_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
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
    app.close_world().expect("close for provenance check");
    let store = WorldStore::open(&path).expect("reopen merge store");
    assert_eq!(
        store
            .revision_source_revision_id(merged_head)
            .expect("merge provenance"),
        Some(source_head)
    );
    drop(store);
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
    app.apply_stored_manual_review_action(
        &merge.review.review_key,
        ManualReviewActionRequest::RecordJudgment {
            operation_id: operation.operation_id.clone(),
            judgment: "Replace the destination value with the reviewed source value.".to_owned(),
        },
    )
    .expect("record replacement judgment");
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

#[test]
fn cross_id_temporal_conflict_requires_a_manual_merge_decision() {
    let path = project_path("phase10-temporal-merge");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let mara = entity(main.world_id, "Mara", "mara", 2);
    commit_entity(&mut app, mara.clone());
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");
    let return_event = participant_event(
        main.world_id,
        mara.id(),
        "return",
        "Mara returns",
        20,
        "actor",
        3,
    );
    commit_event(&mut app, return_event.clone());
    app.switch_variant(branch.id).expect("switch destination");
    let death_event = participant_event(
        main.world_id,
        mara.id(),
        "death",
        "Mara dies",
        10,
        "subject",
        4,
    );
    commit_event(&mut app, death_event.clone());

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare temporal merge");
    assert!(merge.automatic_operation_ids.is_empty());
    assert_eq!(merge.decision_operation_ids.len(), 1);
    assert!(
        merge
            .review
            .validation_report
            .conflicts
            .iter()
            .any(|issue| { issue.code == "lifecycle.participation_after_death" })
    );
    let operation = merge
        .review
        .operations
        .iter()
        .find(|operation| {
            operation.target_uri == ObjectRef::Event(return_event.event().id()).to_string()
        })
        .expect("return operation");
    let decision = operation
        .decision_points
        .first()
        .expect("temporal decision");
    assert!(
        decision
            .prompt
            .contains(&death_event.event().id().to_string())
    );
    let updated = app
        .apply_stored_manual_review_action(
            &merge.review.review_key,
            ManualReviewActionRequest::ResolveDecision {
                decision_point_id: decision.decision_point_id.clone(),
                alternative: "keep_destination".to_owned(),
            },
        )
        .expect("keep destination timeline");
    assert!(updated.effective_report.conflicts.is_empty());
    assert!(!updated.operations[0].selected);
}

#[test]
fn rename_and_archive_persist_and_detect_transitive_descendants() {
    let path = project_path("phase10-variant-lifecycle");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let renamed = app
        .rename_variant(main.active_variant.id, "primary")
        .expect("rename active variant");
    assert_eq!(renamed.name, "primary");
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("open")
            .active_variant
            .name,
        "primary"
    );
    assert!(
        app.create_variant("PRIMARY", main.current_revision)
            .is_err(),
        "variant names are unique case-insensitively"
    );

    let parent = app
        .create_variant("parent", main.current_revision)
        .expect("parent variant");
    app.switch_variant(parent.id).expect("switch parent");
    commit_entity(
        &mut app,
        entity(main.world_id, "Parent", "parent-entity", 2),
    );
    let parent_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
    let child = app
        .create_variant("child", parent_head)
        .expect("child variant");
    app.switch_variant(child.id).expect("switch child");
    commit_entity(&mut app, entity(main.world_id, "Child", "child-entity", 3));
    let child_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
    let grandchild = app
        .create_variant("grandchild", child_head)
        .expect("grandchild variant");
    app.switch_variant(main.active_variant.id)
        .expect("return to main");

    assert!(app.archive_variant(child.id, false).is_err());
    app.archive_variant(child.id, true)
        .expect("archive referenced child explicitly");
    assert!(
        app.archive_variant(parent.id, false).is_err(),
        "active grandchild remains a transitive descendant"
    );
    app.archive_variant(parent.id, true)
        .expect("archive transitive parent explicitly");
    assert!(app.switch_variant(child.id).is_err());

    app.close_world().expect("close");
    let reopened = app.open_world(path).expect("reopen");
    assert_eq!(reopened.active_variant.name, "primary");
    let variants = app.list_variants().expect("variants");
    assert!(
        variants
            .iter()
            .any(|variant| variant.id == child.id && variant.archived)
    );
    assert!(
        variants
            .iter()
            .any(|variant| variant.id == parent.id && variant.archived)
    );
    assert!(
        variants
            .iter()
            .any(|variant| variant.id == grandchild.id && !variant.archived)
    );
}

#[test]
fn comparison_keeps_same_names_separate_and_reports_structured_references() {
    let path = project_path("phase10-comparison-identity");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let left = entity(main.world_id, "Left", "left", 2);
    let right = entity(main.world_id, "Right", "right", 3);
    commit_entity(&mut app, left.clone());
    commit_entity(&mut app, right.clone());
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");

    let source_twin = entity(main.world_id, "Twin", "twin", 4);
    commit_entity(&mut app, source_twin.clone());
    let relation = Relation::new(
        main.world_id,
        left.id(),
        right.id(),
        "connects",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation");
    commit_relation(&mut app, relation.clone());
    app.switch_variant(branch.id).expect("switch destination");
    let destination_twin = entity(main.world_id, "Twin", "twin", 5);
    commit_entity(&mut app, destination_twin.clone());

    let comparison = app
        .compare_scopes(
            ReadScope::head(fork.active_variant.id),
            ReadScope::head(branch.id),
        )
        .expect("compare identities");
    assert!(comparison.differences.iter().any(|difference| {
        difference.object_ref == ObjectRef::Entity(source_twin.id())
            && difference.kind == VariantDiffKind::Deleted
    }));
    assert!(comparison.differences.iter().any(|difference| {
        difference.object_ref == ObjectRef::Entity(destination_twin.id())
            && difference.kind == VariantDiffKind::Created
    }));
    let relation_diff = comparison
        .differences
        .iter()
        .find(|difference| difference.object_ref == ObjectRef::Relation(relation.id()))
        .expect("relation difference");
    assert_eq!(relation_diff.kind, VariantDiffKind::Deleted);
    let mut expected_references = vec![ObjectRef::Entity(left.id()), ObjectRef::Entity(right.id())];
    expected_references.sort();
    assert_eq!(relation_diff.affected_references, expected_references);
    let provenance = relation_diff
        .left_source
        .as_ref()
        .expect("relation provenance");
    assert_eq!(provenance.retcon, RetconKind::Additive);
    assert_eq!(provenance.audit_source, "manual_review");
}

#[test]
fn historical_scope_unifies_uri_search_context_timeline_and_vfs() {
    let path = project_path("phase10-historical-reads");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let mara = entity(main.world_id, "Mara Old", "mara-old", 2);
    commit_entity(&mut app, mara.clone());
    let old_event = participant_event(
        main.world_id,
        mara.id(),
        "arrival",
        "Old arrival",
        5,
        "actor",
        3,
    );
    commit_event(&mut app, old_event.clone());
    let historical = app.get_current_world().expect("session").expect("open");
    update_entity(&mut app, &mara, "Mara Future", "mara-future", 4);
    commit_event(
        &mut app,
        participant_event(
            main.world_id,
            mara.id(),
            "departure",
            "Future departure",
            20,
            "actor",
            5,
        ),
    );
    let current_head = app
        .get_current_world()
        .expect("session")
        .expect("open")
        .current_revision;
    app.set_read_scope(ReadScope::historical(
        historical.active_variant.id,
        historical.current_revision,
    ))
    .expect("observe history");

    let ResolvedObject::Entity(opened) = app
        .open_uri(&ObjectRef::Entity(mara.id()).to_string())
        .expect("open historical entity")
        .object
    else {
        panic!("expected entity");
    };
    assert_eq!(opened.name(), "Mara Old");
    let search = app
        .search_world(&SearchWorldRequest::new(StructuredSearchQuery {
            text: Some("Mara Old".to_owned()),
            limit: 10,
            ..Default::default()
        }))
        .expect("historical search");
    assert!(
        search
            .hits
            .iter()
            .any(|hit| hit.object_ref == ObjectRef::Entity(mara.id()))
    );
    let mut context_request = ContextBundleRequest::new(ContextIntent::ImpactAnalysis);
    context_request.anchors = vec![ObjectRef::Entity(mara.id())];
    let context = app
        .get_related_context(&RelatedContextRequest::new(context_request))
        .expect("historical context");
    assert!(
        context
            .canon
            .iter()
            .any(|entry| entry.result.snippet.contains("Mara Old"))
    );
    let timeline = app.list_timeline_events().expect("historical timeline");
    assert!(
        timeline
            .known
            .iter()
            .any(|event| event.summary == "Old arrival")
    );
    assert!(
        timeline
            .known
            .iter()
            .all(|event| event.summary != "Future departure")
    );
    let vfs = format!("{:?}", app.read_logical_vfs().expect("historical VFS"));
    assert!(vfs.contains("Mara Old"));
    assert!(!vfs.contains("Mara Future"));
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("open")
            .current_revision,
        current_head,
        "observing history cannot move the materialized head"
    );
}

#[test]
fn world_metadata_is_mergeable_without_treating_revision_ids_as_canon_changes() {
    let path = project_path("phase10-world-merge");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "Original".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let branch = app
        .create_variant("alternate", main.current_revision)
        .expect("branch");
    update_world_premise(&mut app, "Source premise");
    app.switch_variant(branch.id).expect("switch destination");

    let merge = app
        .prepare_variant_merge(ReadScope::head(main.active_variant.id))
        .expect("prepare world merge");
    assert_eq!(merge.automatic_operation_ids.len(), 1);
    assert!(merge.decision_operation_ids.is_empty());
    app.confirm_stored_manual_review(&merge.review.review_key)
        .expect("commit world merge");
    assert_eq!(
        app.get_current_world()
            .expect("session")
            .expect("open")
            .world
            .premise_md(),
        "Source premise"
    );
}

#[test]
fn merge_decision_groups_operations_that_depend_on_the_conflicted_object() {
    let path = project_path("phase10-dependent-merge");
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path,
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let anchor = entity(main.world_id, "Anchor", "anchor", 2);
    commit_entity(&mut app, anchor.clone());
    let fork = app.get_current_world().expect("session").expect("open");
    let branch = app
        .create_variant("alternate", fork.current_revision)
        .expect("branch");
    let source_dependency = entity(main.world_id, "Source dependency", "dependency", 3);
    commit_entity(&mut app, source_dependency.clone());
    let relation = Relation::new(
        main.world_id,
        anchor.id(),
        source_dependency.id(),
        "depends_on",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation");
    commit_relation(&mut app, relation.clone());
    app.switch_variant(branch.id).expect("switch destination");
    let destination_dependency = Entity::restore(
        source_dependency.id(),
        source_dependency.world_id(),
        source_dependency.kind(),
        "Destination dependency",
        "dependency",
        "",
        "",
        "{}",
        vec![],
        1,
        source_dependency.created_at_ms(),
        4,
    )
    .expect("destination dependency");
    commit_entity(&mut app, destination_dependency);

    let merge = app
        .prepare_variant_merge(ReadScope::head(fork.active_variant.id))
        .expect("prepare dependent merge");
    assert!(merge.automatic_operation_ids.is_empty());
    assert_eq!(merge.decision_operation_ids.len(), 2);
    let decision = merge
        .review
        .operations
        .iter()
        .find(|operation| {
            operation.target_uri == ObjectRef::Entity(source_dependency.id()).to_string()
        })
        .and_then(|operation| operation.decision_points.first())
        .expect("grouped decision");
    let updated = app
        .apply_stored_manual_review_action(
            &merge.review.review_key,
            ManualReviewActionRequest::ResolveDecision {
                decision_point_id: decision.decision_point_id.clone(),
                alternative: "keep_destination".to_owned(),
            },
        )
        .expect("keep destination dependency");
    assert!(
        updated
            .operations
            .iter()
            .all(|operation| !operation.selected)
    );
    assert!(
        updated
            .effective_report
            .errors
            .iter()
            .all(|issue| { issue.code != "change_set.dependency_missing" })
    );
}

#[test]
fn calendar_is_scoped_by_revision_variant_snapshot_and_undo_without_changing_ticks() {
    let path = project_path("phase11-calendar-history");
    let export_parent = path.parent().expect("parent").to_path_buf();
    let mut app = NirmataApp::default();
    let main = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "Arcadia".to_owned(),
            premise_md: "".to_owned(),
            epoch_label: "Dawn".to_owned(),
        })
        .expect("create world");
    let mara = entity(main.world_id, "Mara", "mara", 1);
    commit_entity(&mut app, mara.clone());
    let shared_event = participant_event(
        main.world_id,
        mara.id(),
        "festival",
        "Shared festival",
        120,
        "actor",
        2,
    );
    let event_id = shared_event.event().id();
    commit_event(&mut app, shared_event);
    let without_calendar = app.get_current_world().expect("session").expect("world");
    let branch = app
        .create_variant("alternate", without_calendar.current_revision)
        .expect("branch");

    update_world_calendar(&mut app, Some(fixed_calendar("Imperial", 100)));
    let main_timeline = app.list_timeline_events().expect("main timeline");
    let main_event = main_timeline
        .known
        .iter()
        .find(|entry| entry.uri == ObjectRef::Event(event_id).to_string())
        .expect("main event");
    assert_eq!(main_event.time.start_tick(), Some(120));
    assert!(
        main_event
            .start_calendar
            .as_ref()
            .expect("main label")
            .label
            .contains("Imperial")
    );

    app.set_read_scope(ReadScope::historical(
        without_calendar.active_variant.id,
        without_calendar.current_revision,
    ))
    .expect("view pre-calendar revision");
    assert!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .world
            .calendar()
            .is_none()
    );
    assert!(
        app.list_timeline_events()
            .expect("historical timeline")
            .known[0]
            .start_calendar
            .is_none()
    );

    app.view_active_head().expect("main head");
    app.switch_variant(branch.id).expect("branch");
    update_world_calendar(&mut app, Some(fixed_calendar("Republic", 90)));
    let branch_event = app
        .list_timeline_events()
        .expect("branch timeline")
        .known
        .into_iter()
        .find(|entry| entry.uri == ObjectRef::Event(event_id).to_string())
        .expect("branch event");
    assert_eq!(branch_event.time.start_tick(), Some(120));
    assert!(
        branch_event
            .start_calendar
            .as_ref()
            .expect("branch label")
            .label
            .contains("Republic")
    );

    let exported = app
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory: export_parent,
            snapshot_name: format!("calendar-{}", branch.id),
        })
        .expect("export calendar snapshot");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(exported.path.join("manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    let world_metadata = manifest["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .find(|object| object["object_type"] == "world")
        .expect("world metadata");
    assert_eq!(world_metadata["metadata"]["calendar"]["name"], "Republic");

    app.undo_last_commit().expect("undo branch calendar");
    assert!(
        app.get_current_world()
            .expect("session")
            .expect("world")
            .world
            .calendar()
            .is_none()
    );
    fs::remove_dir_all(exported.path).expect("remove snapshot");
}
