use nirmata_app::{
    AppError, DraftOperationInput, ManualReviewAction, ManualReviewInput, NirmataApp,
};
use nirmata_core::{
    Period, World,
    change_set::{ChangeOperation, ChangeSet, DecisionPoint, RetconKind},
    claim::{Claim, ClaimAuthentication, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{
    ChangeOperationValue, CommittedChangeSetRecord, DocumentAggregate, EventAggregate,
    OperationAudit, OperationDecision, StoredRevision, WorldStore,
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

fn person(world: &World, name: &str, slug: &str, now_ms: i64) -> Entity {
    Entity::new(
        world.id(),
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

fn renamed_entity(entity: &Entity, name: &str, slug: &str, now_ms: i64) -> Entity {
    Entity::restore(
        entity.id(),
        entity.world_id(),
        entity.kind(),
        name,
        slug,
        entity.summary().to_owned(),
        entity.body_md().to_owned(),
        entity.attributes_json().as_str().to_owned(),
        entity.aliases().to_vec(),
        entity.version() + 1,
        entity.created_at_ms(),
        now_ms,
    )
    .expect("renamed entity")
}

fn assert_person_matches(entity: &Entity, expected: &Entity) {
    assert_eq!(entity.id(), expected.id());
    assert_eq!(entity.world_id(), expected.world_id());
    assert_eq!(entity.kind(), expected.kind());
    assert_eq!(entity.name(), expected.name());
    assert_eq!(entity.slug(), expected.slug());
    assert_eq!(entity.summary(), expected.summary());
    assert_eq!(entity.body_md(), expected.body_md());
    assert_eq!(entity.attributes_json(), expected.attributes_json());
    assert_eq!(entity.aliases(), expected.aliases());
}

fn assert_event_matches(event: &Event, expected: &Event) {
    assert_eq!(event.id(), expected.id());
    assert_eq!(event.world_id(), expected.world_id());
    assert_eq!(event.kind(), expected.kind());
    assert_eq!(event.summary(), expected.summary());
    assert_eq!(event.body_md(), expected.body_md());
    assert_eq!(event.time(), expected.time());
    assert_eq!(event.location_entity_id(), expected.location_entity_id());
    assert_eq!(event.participants(), expected.participants());
    assert_eq!(event.affected_goal_ids(), expected.affected_goal_ids());
}

fn event_aggregate(event: Event) -> EventAggregate {
    EventAggregate::new(event, vec![])
}

fn canonical_claim(
    world: &World,
    subject_entity_id: nirmata_core::EntityId,
    polarity: ClaimPolarity,
) -> Claim {
    Claim::new(
        world.id(),
        subject_entity_id,
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
        world.current_revision(),
    )
    .expect("claim")
}

#[test]
fn committing_and_undoing_world_and_event_causality_round_trips() {
    let path = project_path("manual-review-world-event-causality");
    let world = base_world(&path);
    let cause = Event::new(
        world.id(),
        "collapse",
        "Derrumbe",
        "",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        2,
    )
    .expect("cause event");
    let effect = Event::new(
        world.id(),
        "aftermath",
        "Consecuencia",
        "",
        EventTime::instant(12, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        3,
    )
    .expect("effect event");
    let mut store = WorldStore::open(&path).expect("open store");
    store
        .insert_event(&EventAggregate::new(cause.clone(), vec![]))
        .expect("insert cause");
    store
        .insert_event(&EventAggregate::new(effect.clone(), vec![]))
        .expect("insert effect");
    drop(store);

    let updated_world = World::restore(
        world.id(),
        "Arcadia Prime",
        "Una ciudad que recuerda cada juramento.",
        "Second Dawn",
        world.current_revision(),
        world.created_at_ms(),
        20,
    )
    .expect("updated world");
    let updated_cause = Event::restore(
        cause.id(),
        cause.world_id(),
        cause.kind(),
        "Derrumbe confirmado",
        cause.body_md(),
        cause.time().clone(),
        cause.location_entity_id(),
        cause.participants().to_vec(),
        cause.affected_goal_ids().to_vec(),
        cause.version() + 1,
        cause.created_at_ms(),
        21,
    )
    .expect("updated cause");
    let cause_link = EventLink::new(updated_cause.id(), effect.id(), EventLinkKind::Causes)
        .expect("causal link");

    let mut app = open_app(&path);
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Actualizar mundo y causalidad".to_owned(),
            sources: vec![ObjectRef::World(world.id()), ObjectRef::Event(cause.id())],
            assumptions: vec![],
            operations: vec![
                DraftOperationInput::UpdateWorld {
                    retcon: RetconKind::Reinterpretive,
                    before: world.clone(),
                    after: updated_world.clone(),
                },
                DraftOperationInput::UpdateEvent {
                    retcon: RetconKind::Reinterpretive,
                    before: event_aggregate(cause.clone()),
                    after: EventAggregate::new(updated_cause.clone(), vec![cause_link.clone()]),
                },
            ],
        })
        .expect("start review");
    assert!(review.ready_to_confirm());

    let committed = app.confirm_manual_review(&review).expect("commit review");
    assert_ne!(committed.current_revision, world.current_revision());
    let mut reopened = WorldStore::open(&path).expect("reopen committed store");
    let stored_world = reopened.load_world().expect("load committed world");
    assert_eq!(stored_world.name(), "Arcadia Prime");
    assert_eq!(stored_world.epoch_label(), "Second Dawn");
    let stored_event = reopened
        .get_event(cause.id())
        .expect("load committed event")
        .expect("stored event");
    assert_event_matches(stored_event.event(), &updated_cause);
    assert_eq!(stored_event.links(), &[cause_link.clone()]);
    drop(reopened);

    let undone = app.undo_last_commit().expect("undo committed review");
    assert_ne!(undone.current_revision, committed.current_revision);
    drop(app);
    reopened = WorldStore::open(&path).expect("reopen store after undo");
    let restored_world = reopened.load_world().expect("load restored world");
    assert_eq!(restored_world.name(), world.name());
    assert_eq!(restored_world.epoch_label(), world.epoch_label());
    let restored_event = reopened
        .get_event(cause.id())
        .expect("load restored event")
        .expect("stored restored event");
    assert_event_matches(restored_event.event(), &cause);
    assert!(restored_event.links().is_empty());
    drop(reopened);

    fs::remove_file(path).expect("remove project");
}

fn chronicle_document(
    world: &World,
    chronicler_id: nirmata_core::EntityId,
    now_ms: i64,
) -> Document {
    Document::new(
        world.id(),
        "Mine Chronicle",
        "chronicle",
        Some(chronicler_id),
        Some(chronicler_id),
        DocumentCanonStatus::Canonical,
        "The mine collapsed at dusk.",
        now_ms,
    )
    .expect("document")
}

fn revised_document(document: &Document, title: &str, body_md: &str, now_ms: i64) -> Document {
    Document::restore(
        document.id(),
        document.world_id(),
        title,
        document.kind(),
        document.author_entity_id(),
        document.perspective_entity_id(),
        document.canon_status(),
        body_md,
        document.version() + 1,
        document.created_at_ms(),
        now_ms,
    )
    .expect("revised document")
}

fn collapse_event(world: &World, mara: &Entity, now_ms: i64) -> Event {
    Event::new(
        world.id(),
        "collapse",
        "The mine collapsed at dusk.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(mara.id(), "witness", 0).expect("participant")],
        vec![],
        now_ms,
    )
    .expect("event")
}

fn revised_event(event: &Event, summary: &str, body_md: &str, now_ms: i64) -> Event {
    Event::restore(
        event.id(),
        event.world_id(),
        event.kind(),
        summary,
        body_md,
        event.time().clone(),
        event.location_entity_id(),
        event.participants().to_vec(),
        event.affected_goal_ids().to_vec(),
        event.version() + 1,
        event.created_at_ms(),
        now_ms,
    )
    .expect("revised event")
}

fn sourced_claim(
    world: &World,
    subject_entity_id: nirmata_core::EntityId,
    source_document_id: Option<nirmata_core::DocumentId>,
    source_claim_id: Option<nirmata_core::ClaimId>,
    registered_revision_id: nirmata_core::RevisionId,
) -> Claim {
    Claim::new(
        world.id(),
        subject_entity_id,
        "Mara escaped after the collapse.",
        Some("mara.escaped".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        Some("Chronicle".to_owned()),
        source_document_id,
        source_claim_id,
        None,
        Some(Period::new(Some(21), Some(21)).expect("period")),
        registered_revision_id,
    )
    .expect("claim")
}

#[test]
fn rejecting_a_required_operation_marks_the_dependency_as_broken() {
    let path = project_path("manual-review-selection");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let goal = Goal::new(
        world.id(),
        mara.id(),
        "Protect the mine",
        1,
        GoalStatus::Active,
        None,
        GoalVisibility::Public,
        None,
    )
    .expect("goal");

    let mut app = open_app(&path);
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Add Mara and her goal".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![
                DraftOperationInput::CreateEntity {
                    retcon: RetconKind::Additive,
                    after: mara.clone(),
                },
                DraftOperationInput::CreateGoal {
                    retcon: RetconKind::Additive,
                    after: goal,
                },
            ],
        })
        .expect("start review");

    assert!(review.ready_to_confirm());
    let create_entity_id = review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Reject {
            operation_id: create_entity_id,
        },
    )
    .expect("reject operation");

    assert!(!review.ready_to_confirm());
    assert!(
        review
            .validation_report()
            .errors
            .iter()
            .any(|issue| issue.code == "manual_review.dependency_broken")
    );
    assert!(
        review
            .validation_report()
            .errors
            .iter()
            .any(|issue| issue.code == "goal.holder_missing")
    );

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn editing_an_operation_revalidates_and_updates_the_report() {
    let path = project_path("manual-review-edit");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let vale = person(&world, "Vale", "vale", 3);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    store.insert_entity(&vale).expect("insert Vale");
    drop(store);

    let invalid_after = renamed_entity(&mara, "Mara Vale", "vale", 4);
    let valid_after = renamed_entity(&mara, "Mara Vale", "mara-vale", 5);
    let mut app = open_app(&path);
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Rename Mara".to_owned(),
            sources: vec![ObjectRef::Entity(mara.id()), ObjectRef::Entity(vale.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: mara.clone(),
                after: invalid_after,
            }],
        })
        .expect("start review");

    assert!(!review.ready_to_confirm());
    assert!(
        review
            .validation_report()
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.entity.duplicate_slug")
    );

    let operation_id = review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Edit {
            operation_id,
            replacement: DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: mara,
                after: valid_after,
            },
        },
    )
    .expect("edit operation");

    assert!(review.ready_to_confirm());
    assert_eq!(
        review.operations()[0].decision(),
        nirmata_store::OperationDecision::Edit
    );
    assert!(
        review
            .validation_report()
            .errors
            .iter()
            .all(|issue| issue.code != "change_set.entity.duplicate_slug")
    );

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn waiving_a_conflict_can_leave_a_valid_set_ready_to_confirm() {
    let path = project_path("manual-review-waiver-conflict");
    let world = base_world(&path);
    let gate = person(&world, "North Gate", "north-gate", 2);
    let existing = canonical_claim(&world, gate.id(), ClaimPolarity::Positive);
    let proposed = canonical_claim(&world, gate.id(), ClaimPolarity::Negative);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&gate).expect("insert gate");
    store.insert_claim(&existing).expect("insert claim");
    drop(store);

    let mut app = open_app(&path);
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Record an opposing report".to_owned(),
            sources: vec![ObjectRef::Entity(gate.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateClaim {
                retcon: RetconKind::Additive,
                after: proposed,
            }],
        })
        .expect("start review");

    assert!(!review.ready_to_confirm());
    let operation_id = review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::AddWaiver {
            operation_id,
            issue_code: "claim.canonical_opposition".to_owned(),
            rationale: "Intentional contradiction pending editorial review".to_owned(),
        },
    )
    .expect("waive conflict");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::RecordJudgment {
            operation_id,
            judgment: "The contradiction is intentional and I want to inspect the evidence before deciding.".to_owned(),
        },
    )
    .expect("record judgment");

    assert!(review.ready_to_confirm());
    assert_eq!(review.waivers().len(), 1);
    assert!(review.effective_report().conflicts.is_empty());
    assert!(
        review
            .validation_report()
            .conflicts
            .iter()
            .any(|issue| issue.code == "claim.canonical_opposition")
    );

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn revision_history_exposes_before_after_waivers_and_visible_undo() {
    let path = project_path("manual-review-history");
    let world = base_world(&path);
    let gate = person(&world, "North Gate", "north-gate", 2);
    let renamed_gate = renamed_entity(&gate, "North Gate Archive", "north-gate-archive", 3);
    let existing = canonical_claim(&world, gate.id(), ClaimPolarity::Positive);
    let proposed = canonical_claim(&world, gate.id(), ClaimPolarity::Negative);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&gate).expect("insert gate");
    store.insert_claim(&existing).expect("insert claim");
    drop(store);

    let mut app = open_app(&path);
    let rename_review = app
        .start_manual_review(ManualReviewInput {
            objective: "Rename gate for the local audit".to_owned(),
            sources: vec![ObjectRef::Entity(gate.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: gate.clone(),
                after: renamed_gate.clone(),
            }],
        })
        .expect("start rename review");
    let renamed = app
        .confirm_manual_review(&rename_review)
        .expect("confirm rename review");

    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Preserve the contradictory report".to_owned(),
            sources: vec![
                ObjectRef::Entity(gate.id()),
                ObjectRef::Claim(existing.id()),
            ],
            assumptions: vec!["Keep the local audit visible before undoing anything.".to_owned()],
            operations: vec![DraftOperationInput::CreateClaim {
                retcon: RetconKind::Additive,
                after: proposed,
            }],
        })
        .expect("start review");

    let claim_operation_id = review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::AddWaiver {
            operation_id: claim_operation_id,
            issue_code: "claim.canonical_opposition".to_owned(),
            rationale: "Intentional contradiction for local editorial review".to_owned(),
        },
    )
    .expect("waive contradiction");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::RecordJudgment {
            operation_id: claim_operation_id,
            judgment: "Keep both reports visible so the audit can show why the waiver exists."
                .to_owned(),
        },
    )
    .expect("record claim judgment");
    assert!(review.ready_to_confirm());

    let committed = app.confirm_manual_review(&review).expect("confirm review");
    let committed_id = committed.current_revision.to_string();
    let renamed_id = renamed.current_revision.to_string();
    let history = app.list_revision_history().expect("list revision history");
    assert_eq!(
        history.undo_target_revision_id.as_deref(),
        Some(committed_id.as_str())
    );
    let committed_entry = history
        .revisions
        .iter()
        .find(|entry| entry.revision_id == committed_id)
        .expect("committed entry");
    assert!(committed_entry.is_current_undo_target);
    assert_eq!(committed_entry.operations.len(), 1);
    assert_eq!(committed_entry.waivers.len(), 1);
    let updated_gate = history
        .revisions
        .iter()
        .find(|entry| entry.revision_id == renamed_id)
        .expect("rename entry")
        .operations
        .iter()
        .find(|operation| operation.target_uri == ObjectRef::Entity(gate.id()).to_string())
        .expect("entity audit");
    assert!(updated_gate.before.is_some());
    assert!(updated_gate.after.is_some());
    let waived_claim = committed_entry
        .operations
        .iter()
        .find(|operation| {
            operation
                .waivers
                .iter()
                .any(|waiver| waiver.issue_code == "claim.canonical_opposition")
        })
        .expect("claim waiver audit");
    assert_eq!(waived_claim.waivers.len(), 1);

    let undone = app
        .undo_revision(committed.current_revision)
        .expect("undo visible revision");
    let undo_id = undone.current_revision.to_string();
    let after_undo = app
        .list_revision_history()
        .expect("list revision history after undo");
    let undo_entry = after_undo
        .revisions
        .iter()
        .find(|entry| entry.revision_id == undo_id)
        .expect("undo entry");
    assert_eq!(
        undo_entry.undone_revision_id.as_deref(),
        Some(committed_id.as_str())
    );
    assert!(undo_entry.is_current_head);

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn waivers_do_not_hide_hard_errors() {
    let path = project_path("manual-review-waiver-hard-error");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let rule = Rule::new(
        world.id(),
        RuleKind::Constitutive,
        "The dead do not return.",
        "world",
        RuleSeverity::Hard,
        None,
        Some(RuleValidatorKind::NoResurrection),
        "{}",
        3,
    )
    .expect("rule");
    let death = Event::new(
        world.id(),
        "death",
        "Mara dies.",
        "",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(mara.id(), "subject", 0).expect("participant")],
        vec![],
        4,
    )
    .expect("death");
    let return_event = Event::new(
        world.id(),
        "return",
        "Mara returns.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(mara.id(), "actor", 0).expect("participant")],
        vec![],
        5,
    )
    .expect("return");
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    store.insert_rule(&rule).expect("insert rule");
    store
        .insert_event(&EventAggregate::new(death, vec![]))
        .expect("insert death");
    drop(store);

    let mut app = open_app(&path);
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Bring Mara back".to_owned(),
            sources: vec![ObjectRef::Entity(mara.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEvent {
                retcon: RetconKind::Additive,
                after: event_aggregate(return_event),
            }],
        })
        .expect("start review");

    let operation_id = review.operations()[0].operation_id();
    let error = app
        .apply_manual_review_action(
            &mut review,
            ManualReviewAction::AddWaiver {
                operation_id,
                issue_code: "rule.no_resurrection".to_owned(),
                rationale: "Ignore the rule".to_owned(),
            },
        )
        .expect_err("hard errors cannot be waived");

    assert!(matches!(
        error,
        AppError::CannotWaiveHardIssue { operation_id: found, issue_code }
            if found == operation_id && issue_code == "rule.no_resurrection"
    ));
    assert!(!review.ready_to_confirm());
    assert!(
        review
            .validation_report()
            .errors
            .iter()
            .any(|issue| issue.code == "rule.no_resurrection")
    );
    assert!(
        review
            .effective_report()
            .errors
            .iter()
            .any(|issue| issue.code == "rule.no_resurrection")
    );

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn confirming_a_valid_manual_review_applies_operations_and_creates_one_revision() {
    let path = project_path("manual-review-confirm");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let goal = Goal::new(
        world.id(),
        mara.id(),
        "Protect the mine",
        1,
        GoalStatus::Active,
        None,
        GoalVisibility::Public,
        None,
    )
    .expect("goal");

    let mut app = open_app(&path);
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Add Mara and her goal".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![
                DraftOperationInput::CreateEntity {
                    retcon: RetconKind::Additive,
                    after: mara.clone(),
                },
                DraftOperationInput::CreateGoal {
                    retcon: RetconKind::Additive,
                    after: goal.clone(),
                },
            ],
        })
        .expect("start review");

    let committed = app
        .confirm_manual_review(&review)
        .expect("confirm valid review");
    assert_ne!(committed.current_revision, world.current_revision());

    let store = WorldStore::open(&path).expect("open store");
    assert_eq!(
        store.get_entity(mara.id()).expect("load entity"),
        Some(mara)
    );
    assert_eq!(store.get_goal(goal.id()).expect("load goal"), Some(goal));
    let revisions = store.list_revisions().expect("list revisions");
    assert_eq!(revisions.len(), 2);
    let committed_revision = revisions.last().expect("committed revision");
    assert_eq!(committed_revision.id(), committed.current_revision);
    let committed_change_set = store
        .get_committed_change_set(
            committed_revision
                .change_set_id()
                .expect("committed revision change set"),
        )
        .expect("load committed change set")
        .expect("committed record");
    assert_eq!(committed_change_set.change_set().operations().len(), 2);
    drop(store);

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn stale_version_during_confirmation_leaves_canon_and_revision_unchanged() {
    let path = project_path("manual-review-stale-version");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    drop(store);

    let reviewed_after = renamed_entity(&mara, "Mara of the Gate", "mara-gate", 3);
    let mut app = open_app(&path);
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Rename Mara".to_owned(),
            sources: vec![ObjectRef::Entity(mara.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: mara.clone(),
                after: reviewed_after.clone(),
            }],
        })
        .expect("start review");

    let external_after = Entity::restore(
        mara.id(),
        mara.world_id(),
        mara.kind(),
        "Mara Updated Elsewhere",
        "mara-updated",
        mara.summary().to_owned(),
        mara.body_md().to_owned(),
        mara.attributes_json().as_str().to_owned(),
        mara.aliases().to_vec(),
        mara.version(),
        mara.created_at_ms(),
        4,
    )
    .expect("external entity update");
    let mut store = WorldStore::open(&path).expect("reopen store");
    let applied_external = store
        .update_entity(&external_after)
        .expect("apply external update");
    let revision_before = store
        .load_world()
        .expect("world before failure")
        .current_revision();
    drop(store);

    let error = app
        .confirm_manual_review(&review)
        .expect_err("stale entity version must fail");
    assert!(matches!(error, AppError::ManualReviewRevalidationFailed));

    let store = WorldStore::open(&path).expect("reopen store after failure");
    assert_eq!(
        store
            .load_world()
            .expect("world after failure")
            .current_revision(),
        revision_before
    );
    assert_eq!(
        store
            .get_entity(mara.id())
            .expect("load entity after failure"),
        Some(applied_external)
    );
    assert_eq!(store.list_revisions().expect("list revisions").len(), 1);
    assert_ne!(
        store.get_entity(mara.id()).expect("load reviewed entity"),
        Some(reviewed_after)
    );
    drop(store);
    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn complete_manual_workflow_covers_replacement_stale_commit_rollback_audit_and_undo() {
    let path = project_path("manual-review-end-to-end");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let chronicler = person(&world, "Archivist", "archivist", 3);
    let collapse = collapse_event(&world, &mara, 4);
    let chronicle = chronicle_document(&world, chronicler.id(), 5);
    let chronicle_aggregate = DocumentAggregate::new(
        chronicle.clone(),
        vec![
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Entity(mara.id()),
                0,
            ),
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Event(collapse.id()),
                1,
            ),
        ],
    );
    let existing_claim = sourced_claim(
        &world,
        mara.id(),
        Some(chronicle.id()),
        None,
        world.current_revision(),
    );
    let rejected_document = revised_document(
        &chronicle,
        "Mine Chronicle (draft)",
        "Draft text that should be rejected.",
        6,
    );
    let stale_replacement = revised_event(
        &collapse,
        "The mine collapsed at dawn.",
        "The chronicle misstated the time of the collapse.",
        7,
    );
    let invalid_claim = sourced_claim(
        &world,
        mara.id(),
        Some(nirmata_core::DocumentId::new()),
        None,
        world.current_revision(),
    );
    let valid_claim = sourced_claim(
        &world,
        mara.id(),
        Some(chronicle.id()),
        None,
        world.current_revision(),
    );

    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    store.insert_entity(&chronicler).expect("insert chronicler");
    store
        .insert_event(&EventAggregate::new(collapse.clone(), vec![]))
        .expect("insert event");
    store
        .insert_document(&chronicle_aggregate)
        .expect("insert document");
    store
        .insert_claim(&existing_claim)
        .expect("insert existing claim");
    drop(store);

    let mut app = open_app(&path);
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Correct the collapse record".to_owned(),
            sources: vec![
                ObjectRef::Entity(mara.id()),
                ObjectRef::Event(collapse.id()),
                ObjectRef::Claim(existing_claim.id()),
                ObjectRef::Document(chronicle.id()),
            ],
            assumptions: vec![
                "Preserve the existing chronicle until the replacement is accepted.".to_owned(),
            ],
            operations: vec![
                DraftOperationInput::UpdateEvent {
                    retcon: RetconKind::Replacement,
                    before: event_aggregate(collapse.clone()),
                    after: event_aggregate(stale_replacement.clone()),
                },
                DraftOperationInput::UpdateDocument {
                    retcon: RetconKind::Additive,
                    before: chronicle_aggregate.clone(),
                    after: DocumentAggregate::new(
                        rejected_document.clone(),
                        chronicle_aggregate.references().to_vec(),
                    ),
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: invalid_claim.clone(),
                },
            ],
        })
        .expect("start review");

    assert!(!review.ready_to_confirm());
    assert!(
        review
            .validation_report()
            .errors
            .iter()
            .any(|issue| issue.code == "claim.source_document_missing")
    );
    let replacement_decision = review
        .draft()
        .decisions()
        .first()
        .expect("replacement decision");
    assert_eq!(
        replacement_decision.replacement_target(),
        Some(ObjectRef::Event(collapse.id()))
    );

    let reject_document_id = review.operations()[1].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Reject {
            operation_id: reject_document_id,
        },
    )
    .expect("reject document change");
    assert_eq!(review.draft().operations().len(), 2);

    let claim_operation_id = review.operations()[2].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Edit {
            operation_id: claim_operation_id,
            replacement: DraftOperationInput::CreateClaim {
                retcon: RetconKind::Additive,
                after: valid_claim.clone(),
            },
        },
    )
    .expect("fix claim provenance");
    let replacement_operation_id = review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::RecordJudgment {
            operation_id: replacement_operation_id,
            judgment:
                "The replacement keeps the collapse as canon after reviewing the cited conflict."
                    .to_owned(),
        },
    )
    .expect("record replacement judgment");
    assert!(review.ready_to_confirm());

    let external_event = Event::restore(
        collapse.id(),
        collapse.world_id(),
        collapse.kind(),
        "The mine collapsed at dusk, according to a later edit.",
        collapse.body_md(),
        collapse.time().clone(),
        collapse.location_entity_id(),
        collapse.participants().to_vec(),
        collapse.affected_goal_ids().to_vec(),
        collapse.version(),
        collapse.created_at_ms(),
        8,
    )
    .expect("external event");
    let mut store = WorldStore::open(&path).expect("reopen store");
    let externally_applied = store
        .update_event(&EventAggregate::new(external_event, vec![]))
        .expect("apply external event update");
    let revision_before_retry = store
        .load_world()
        .expect("world before retry")
        .current_revision();
    drop(store);

    let error = app
        .confirm_manual_review(&review)
        .expect_err("stale event version must fail");
    assert!(matches!(error, AppError::ManualReviewRevalidationFailed));

    let store = WorldStore::open(&path).expect("reopen store after stale failure");
    assert_eq!(
        store
            .load_world()
            .expect("world after stale failure")
            .current_revision(),
        revision_before_retry
    );
    assert_eq!(
        store
            .get_claim(valid_claim.id())
            .expect("claim after stale failure"),
        None
    );
    assert_eq!(
        store
            .get_event(collapse.id())
            .expect("event after stale failure")
            .expect("stored event")
            .event(),
        externally_applied.event()
    );
    assert_eq!(
        store
            .get_document(chronicle.id())
            .expect("document after stale failure")
            .expect("stored document")
            .object(),
        &chronicle
    );
    drop(store);

    let current_event = externally_applied.event().clone();
    let committed_replacement = revised_event(
        &current_event,
        "The collapse began before sunrise.",
        "The chronicle's timeline was replaced after manual review.",
        9,
    );
    let invalid_retry_claim = sourced_claim(
        &world,
        mara.id(),
        Some(nirmata_core::DocumentId::new()),
        None,
        revision_before_retry,
    );
    let valid_retry_claim = sourced_claim(
        &world,
        mara.id(),
        Some(chronicle.id()),
        None,
        revision_before_retry,
    );
    let rejected_retry_document = revised_document(
        &chronicle,
        "Mine Chronicle (rejected again)",
        "This draft should never reach the canon.",
        10,
    );

    let mut retry_review = app
        .start_manual_review(ManualReviewInput {
            objective: "Correct the collapse record".to_owned(),
            sources: vec![
                ObjectRef::Entity(mara.id()),
                ObjectRef::Event(collapse.id()),
                ObjectRef::Claim(existing_claim.id()),
                ObjectRef::Document(chronicle.id()),
            ],
            assumptions: vec!["Preserve the chronicle body until a later pass.".to_owned()],
            operations: vec![
                DraftOperationInput::UpdateEvent {
                    retcon: RetconKind::Replacement,
                    before: event_aggregate(current_event.clone()),
                    after: event_aggregate(committed_replacement.clone()),
                },
                DraftOperationInput::UpdateDocument {
                    retcon: RetconKind::Additive,
                    before: chronicle_aggregate.clone(),
                    after: DocumentAggregate::new(
                        rejected_retry_document,
                        chronicle_aggregate.references().to_vec(),
                    ),
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: invalid_retry_claim,
                },
            ],
        })
        .expect("restart review");
    let rejected_retry_document_id = retry_review.operations()[1].operation_id();
    app.apply_manual_review_action(
        &mut retry_review,
        ManualReviewAction::Reject {
            operation_id: rejected_retry_document_id,
        },
    )
    .expect("reject document change again");
    let retry_claim_operation_id = retry_review.operations()[2].operation_id();
    app.apply_manual_review_action(
        &mut retry_review,
        ManualReviewAction::Edit {
            operation_id: retry_claim_operation_id,
            replacement: DraftOperationInput::CreateClaim {
                retcon: RetconKind::Additive,
                after: valid_retry_claim.clone(),
            },
        },
    )
    .expect("repair claim again");
    let retry_replacement_operation_id = retry_review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut retry_review,
        ManualReviewAction::RecordJudgment {
            operation_id: retry_replacement_operation_id,
            judgment:
                "The refreshed replacement is still justified after re-reading the updated event."
                    .to_owned(),
        },
    )
    .expect("record retry replacement judgment");
    assert!(retry_review.ready_to_confirm());

    let committed_session = app
        .confirm_manual_review(&retry_review)
        .expect("confirm full manual workflow");
    let committed_revision_id = committed_session.current_revision;

    let mut store = WorldStore::open(&path).expect("reopen store after commit");
    let committed_revision = store
        .get_revision(committed_revision_id)
        .expect("load committed revision")
        .expect("stored committed revision");
    let committed_record = store
        .get_committed_change_set(
            committed_revision
                .change_set_id()
                .expect("committed change set id"),
        )
        .expect("load committed change set")
        .expect("stored committed change set");
    assert_eq!(committed_record.change_set().operations().len(), 2);
    assert_eq!(committed_record.change_set().decisions().len(), 1);
    assert_eq!(
        committed_record.change_set().decisions()[0].replacement_target(),
        Some(ObjectRef::Event(collapse.id()))
    );
    assert_eq!(committed_record.audits().len(), 2);
    assert!(
        committed_record
            .audits()
            .iter()
            .all(|audit| audit.operation_id() != rejected_retry_document_id)
    );
    let event_audit = committed_record
        .audits()
        .iter()
        .find(|audit| audit.operation_id() == retry_review.operations()[0].operation_id())
        .expect("event audit");
    assert_eq!(event_audit.decision(), OperationDecision::Accept);
    assert_eq!(event_audit.source(), "manual_review");
    assert!(matches!(
        event_audit.before(),
        Some(ChangeOperationValue::Event(before)) if before == &event_aggregate(current_event.clone())
    ));
    assert!(matches!(
        event_audit.after(),
        Some(ChangeOperationValue::Event(after)) if after == &event_aggregate(committed_replacement.clone())
    ));
    let claim_audit = committed_record
        .audits()
        .iter()
        .find(|audit| audit.operation_id() == retry_claim_operation_id)
        .expect("claim audit");
    assert_eq!(claim_audit.decision(), OperationDecision::Edit);
    assert!(matches!(
        claim_audit.after(),
        Some(ChangeOperationValue::Claim(after)) if after == &valid_retry_claim
    ));
    assert_eq!(
        store
            .get_event(collapse.id())
            .expect("load committed event")
            .expect("stored committed event")
            .event(),
        &committed_replacement
    );
    assert_eq!(
        store
            .get_claim(valid_retry_claim.id())
            .expect("load committed claim"),
        Some(valid_retry_claim.clone())
    );
    assert_eq!(
        store
            .get_document(chronicle.id())
            .expect("load committed document")
            .expect("stored committed document")
            .object(),
        &chronicle
    );

    let rollback_entity = person(&world, "Mara Duplicate", "mara", 11);
    let rollback_operation = ChangeOperation::CreateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(rollback_entity.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: rollback_entity.clone(),
    };
    let rollback_change_set = ChangeSet::new(
        world.id(),
        committed_revision_id,
        "Inject failing manual review",
        vec![ObjectRef::Entity(mara.id())],
        vec![],
        vec![rollback_operation.clone()],
        vec![],
    )
    .expect("rollback change set");
    let rollback_revision = StoredRevision::restore(
        world.current_revision(),
        world.id(),
        Some(committed_revision_id),
        Some(rollback_change_set.id()),
        "manual_review",
        "Failing revision",
        12,
    )
    .expect("rollback revision");
    let rollback_record = CommittedChangeSetRecord::new(
        rollback_change_set.clone(),
        None,
        vec![],
        vec![
            OperationAudit::from_operation(
                &rollback_operation,
                OperationDecision::Accept,
                "manual_review",
                12,
            )
            .expect("rollback audit"),
        ],
        rollback_revision,
        None,
    )
    .expect("rollback record");
    let rollback_error = store
        .commit_change_set(&rollback_record)
        .expect_err("failing record must roll back");
    assert!(matches!(
        rollback_error,
        nirmata_store::StoreError::Database(_, _)
    ));
    assert_eq!(
        store
            .load_world()
            .expect("world after rollback attempt")
            .current_revision(),
        committed_revision_id
    );
    assert_eq!(
        store
            .get_entity(rollback_entity.id())
            .expect("rollback entity lookup"),
        None
    );
    assert_eq!(
        store
            .get_committed_change_set(rollback_change_set.id())
            .expect("rollback record lookup"),
        None
    );
    assert_eq!(
        store
            .list_revisions()
            .expect("revisions after rollback")
            .len(),
        2
    );
    drop(store);

    let undone = app
        .undo_last_commit()
        .expect("undo integrated workflow commit");
    assert_ne!(undone.current_revision, committed_revision_id);
    app.close_world().expect("close world");

    let reopened = WorldStore::open(&path).expect("reopen store after undo");
    let restored_event = reopened
        .get_event(collapse.id())
        .expect("event after undo")
        .expect("stored event after undo");
    assert_event_matches(restored_event.event(), &current_event);
    assert!(restored_event.event().version() > current_event.version());
    assert_eq!(
        reopened
            .get_claim(valid_retry_claim.id())
            .expect("claim after undo"),
        None
    );
    assert_eq!(
        reopened
            .get_document(chronicle.id())
            .expect("document after undo")
            .expect("stored document after undo")
            .object(),
        &chronicle
    );
    let undo_revision = reopened
        .get_revision(undone.current_revision)
        .expect("load undo revision")
        .expect("stored undo revision");
    let undo_record = reopened
        .get_committed_change_set(undo_revision.change_set_id().expect("undo change set id"))
        .expect("load undo record")
        .expect("stored undo record");
    assert_eq!(
        undo_record.undone_revision_id(),
        Some(committed_revision_id)
    );
    drop(reopened);

    fs::remove_file(path).expect("remove project");
}

#[test]
fn undoing_a_created_entity_persists_across_reopen() {
    let path = project_path("manual-review-undo-create");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);

    let mut app = open_app(&path);
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Add Mara".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: mara.clone(),
            }],
        })
        .expect("start review");
    let committed = app
        .confirm_manual_review(&review)
        .expect("confirm create review");
    let created_revision = committed.current_revision;

    let undone = app.undo_last_commit().expect("undo created entity");
    assert_ne!(undone.current_revision, created_revision);
    app.close_world().expect("close world");

    let reopened = WorldStore::open(&path).expect("reopen store");
    assert_eq!(
        reopened.get_entity(mara.id()).expect("entity after undo"),
        None
    );
    let undo_revision = reopened
        .get_revision(undone.current_revision)
        .expect("load undo revision")
        .expect("stored undo revision");
    let undo_change_set = reopened
        .get_committed_change_set(undo_revision.change_set_id().expect("undo change set id"))
        .expect("load undo change set")
        .expect("stored undo change set");
    assert_eq!(undo_change_set.undone_revision_id(), Some(created_revision));
    assert_eq!(
        reopened
            .load_world()
            .expect("world after reopen")
            .current_revision(),
        undone.current_revision
    );
    drop(reopened);

    fs::remove_file(path).expect("remove project");
}

#[test]
fn undoing_an_updated_entity_restores_its_previous_state() {
    let path = project_path("manual-review-undo-update");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    drop(store);

    let updated = renamed_entity(&mara, "Mara of the Gate", "mara-gate", 3);
    let mut app = open_app(&path);
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Rename Mara".to_owned(),
            sources: vec![ObjectRef::Entity(mara.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::UpdateEntity {
                retcon: RetconKind::Additive,
                before: mara.clone(),
                after: updated,
            }],
        })
        .expect("start review");
    app.confirm_manual_review(&review)
        .expect("confirm update review");
    app.undo_last_commit().expect("undo updated entity");
    app.close_world().expect("close world");

    let reopened = WorldStore::open(&path).expect("reopen store");
    let restored = reopened
        .get_entity(mara.id())
        .expect("load restored entity")
        .expect("restored entity");
    assert_person_matches(&restored, &mara);
    assert!(restored.version() > mara.version());
    drop(reopened);

    fs::remove_file(path).expect("remove project");
}

#[test]
fn undoing_a_deleted_entity_recreates_it() {
    let path = project_path("manual-review-undo-delete");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    let delete_operation = ChangeOperation::DeleteEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(mara.id())],
        expected_version: mara.version(),
        retcon: RetconKind::Replacement,
        before: mara.clone(),
    };
    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Remove Mara",
        vec![ObjectRef::Entity(mara.id())],
        vec![],
        vec![delete_operation.clone()],
        vec![
            DecisionPoint::new_replacement(
                vec![delete_operation.operation_id()],
                "Should Mara leave the canon?",
                vec!["Keep Mara".to_owned(), "Delete Mara".to_owned()],
                ObjectRef::Entity(mara.id()),
                "Undo testing needs a committed delete revision.",
                "Delete Mara",
            )
            .expect("replacement decision"),
        ],
    )
    .expect("delete change set");
    let revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(change_set.id()),
        "manual_review",
        "Remove Mara",
        3,
    )
    .expect("delete revision");
    store
        .commit_change_set(
            &CommittedChangeSetRecord::new(
                change_set,
                None,
                vec![],
                vec![
                    OperationAudit::from_operation(
                        &delete_operation,
                        OperationDecision::Accept,
                        "manual_review",
                        3,
                    )
                    .expect("delete audit"),
                ],
                revision,
                None,
            )
            .expect("delete record"),
        )
        .expect("commit delete revision");
    drop(store);

    let mut app = open_app(&path);
    app.undo_last_commit().expect("undo deleted entity");
    app.close_world().expect("close world");

    let reopened = WorldStore::open(&path).expect("reopen store");
    let restored = reopened
        .get_entity(mara.id())
        .expect("load recreated entity")
        .expect("recreated entity");
    assert_person_matches(&restored, &mara);
    drop(reopened);

    fs::remove_file(path).expect("remove project");
}

#[test]
fn rejecting_an_undo_that_is_not_the_current_logical_ancestor_is_visible() {
    let path = project_path("manual-review-undo-invalid-target");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let vale = person(&world, "Vale", "vale", 3);

    let mut app = open_app(&path);
    let first = app
        .start_manual_review(ManualReviewInput {
            objective: "Add Mara".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: mara,
            }],
        })
        .expect("start first review");
    let first_revision = app
        .confirm_manual_review(&first)
        .expect("confirm first review")
        .current_revision;

    let second = app
        .start_manual_review(ManualReviewInput {
            objective: "Add Vale".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: vale,
            }],
        })
        .expect("start second review");
    let second_revision = app
        .confirm_manual_review(&second)
        .expect("confirm second review")
        .current_revision;

    let error = app
        .undo_revision(first_revision)
        .expect_err("older logical revision must be rejected");
    assert!(matches!(
        error,
        AppError::UndoTargetNotCurrentLogicalAncestor { expected, found }
            if expected == second_revision && found == first_revision
    ));

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn confirming_document_review_persists_reordered_content_references_without_touching_event_time() {
    let path = project_path("manual-review-document-references");
    let world = base_world(&path);
    let mara = person(&world, "Mara", "mara", 2);
    let chronicler = person(&world, "Archivist", "archivist", 3);
    let collapse = collapse_event(&world, &mara, 4);
    let aftermath = Event::new(
        world.id(),
        "aftermath",
        "Ash settled over the mine",
        "",
        EventTime::interval(12, 14, TimePrecision::Day, Certainty::Approximate)
            .expect("valid interval"),
        None,
        vec![EventParticipant::new(mara.id(), "survivor", 0).expect("participant")],
        vec![],
        5,
    )
    .expect("aftermath");
    let chronicle = chronicle_document(&world, chronicler.id(), 6);
    let before = DocumentAggregate::new(
        chronicle.clone(),
        vec![
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Event(collapse.id()),
                0,
            ),
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Entity(mara.id()),
                1,
            ),
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Event(aftermath.id()),
                2,
            ),
        ],
    );
    let after = DocumentAggregate::new(
        revised_document(
            &chronicle,
            "Mine Chronicle",
            "The scribe now opens with the aftermath before revisiting the collapse.",
            7,
        ),
        vec![
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Event(aftermath.id()),
                0,
            ),
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Event(collapse.id()),
                1,
            ),
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Entity(mara.id()),
                2,
            ),
        ],
    );

    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&mara).expect("insert Mara");
    store.insert_entity(&chronicler).expect("insert chronicler");
    store
        .insert_event(&EventAggregate::new(collapse.clone(), vec![]))
        .expect("insert collapse");
    store
        .insert_event(&EventAggregate::new(aftermath.clone(), vec![]))
        .expect("insert aftermath");
    store.insert_document(&before).expect("insert document");
    drop(store);

    let mut app = open_app(&path);
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Reorder chronicle discourse".to_owned(),
            sources: vec![
                ObjectRef::Document(chronicle.id()),
                ObjectRef::Event(collapse.id()),
                ObjectRef::Event(aftermath.id()),
            ],
            assumptions: vec!["Changing ordinal does not rewrite story time.".to_owned()],
            operations: vec![DraftOperationInput::UpdateDocument {
                retcon: RetconKind::Additive,
                before: before.clone(),
                after: after.clone(),
            }],
        })
        .expect("start document review");
    let operation_id = review.operations()[0].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::RecordJudgment {
            operation_id,
            judgment: "The discourse reorder changes several linked references but not the underlying event chronology.".to_owned(),
        },
    )
    .expect("record broad-impact judgment");
    assert!(review.ready_to_confirm());

    let session = app
        .confirm_manual_review(&review)
        .expect("confirm document review");
    let store = WorldStore::open(&path).expect("reopen store");
    let stored_document = store
        .get_document(chronicle.id())
        .expect("load document")
        .expect("stored document");
    assert_eq!(stored_document.references(), after.references());
    assert_eq!(
        store
            .get_event(collapse.id())
            .expect("load collapse")
            .expect("stored collapse")
            .event()
            .time(),
        collapse.time()
    );
    assert_eq!(
        store
            .load_world()
            .expect("load world after commit")
            .current_revision(),
        session.current_revision
    );

    app.close_world().expect("close world");
    drop(store);
    fs::remove_file(path).expect("remove project");
}
