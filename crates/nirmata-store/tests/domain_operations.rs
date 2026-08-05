use nirmata_core::{
    Period, RevisionId, World,
    change_set::{ChangeOperation, ChangeSet, ChangeSetDraft, DecisionPoint, RetconKind},
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{
    AnchorContextQuery, ChangeSetDraftRecord, CommittedChangeSetRecord, DocumentAggregate,
    EventAggregate, OperationAudit, OperationDecision, ResolvedObject, StoreError, StoredRevision,
    StructuredSearchHit, StructuredSearchKind, StructuredSearchQuery, StructuredSearchStage,
    StructuredSearchTemporal, WorldStore,
};
use serde_json::json;
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

fn create_entity_change_operation(world: &World, now_ms: i64) -> (Entity, ChangeOperation) {
    let entity = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        now_ms,
    )
    .expect("entity");
    let operation = ChangeOperation::CreateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: entity.clone(),
    };
    (entity, operation)
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

fn update_entity_operation(before: &Entity, after: &Entity) -> ChangeOperation {
    ChangeOperation::UpdateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(before.id())],
        expected_version: before.version(),
        retcon: RetconKind::Additive,
        before: before.clone(),
        after: after.clone(),
    }
}

fn create_claim_operation(claim: &Claim, retcon: RetconKind) -> ChangeOperation {
    let mut affected_ids = vec![
        ObjectRef::Claim(claim.id()),
        ObjectRef::Entity(claim.subject_entity_id()),
    ];
    if let Some(holder_id) = claim.holder_entity_id() {
        affected_ids.push(ObjectRef::Entity(holder_id));
    }
    if let Some(ClaimObject::Entity(entity_id)) = claim.object() {
        affected_ids.push(ObjectRef::Entity(*entity_id));
    }
    if let Some(document_id) = claim.source_document_id() {
        affected_ids.push(ObjectRef::Document(document_id));
    }
    if let Some(source_claim_id) = claim.source_claim_id() {
        affected_ids.push(ObjectRef::Claim(source_claim_id));
    }
    ChangeOperation::CreateClaim {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids,
        expected_version: 0,
        retcon,
        after: claim.clone(),
    }
}

fn delete_entity_operation(entity: &Entity, retcon: RetconKind) -> ChangeOperation {
    ChangeOperation::DeleteEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: entity.version(),
        retcon,
        before: entity.clone(),
    }
}

fn canonical_claim(
    world: &World,
    subject_entity_id: nirmata_core::EntityId,
    predicate_key: &str,
    object: ClaimObject,
    polarity: ClaimPolarity,
    period: Period,
) -> Claim {
    Claim::new(
        world.id(),
        subject_entity_id,
        "canonical claim",
        Some(predicate_key.to_owned()),
        Some(object),
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
        Some(period),
        world.current_revision(),
    )
    .expect("canonical claim")
}

fn attributed_claim(
    world: &World,
    subject_entity_id: nirmata_core::EntityId,
    holder_entity_id: nirmata_core::EntityId,
    register: &str,
    polarity: ClaimPolarity,
) -> Claim {
    Claim::new(
        world.id(),
        subject_entity_id,
        "attributed claim",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        polarity,
        ClaimAuthentication::Attributed,
        Some(holder_entity_id),
        Some(ClaimModality::Belief),
        Some(register.to_owned()),
        None,
        None,
        None,
        None,
        Some(0.6),
        Some(Period::new(Some(10), Some(10)).expect("period")),
        world.current_revision(),
    )
    .expect("attributed claim")
}

#[test]
fn round_trips_lists_and_updates_every_canon_aggregate() {
    let path = project_path("domain-operations");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Oaths bind.",
        "realm",
        RuleSeverity::Advisory,
        Some("charter".to_owned()),
        None,
        "{}",
        1,
    )
    .expect("rule");
    store.insert_rule(&rule).expect("insert rule");

    let actor = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        r#"{"rank":"captain"}"#,
        vec!["The Witness".to_owned()],
        1,
    )
    .expect("actor");
    let place = Entity::new(
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
    .expect("place");
    store.insert_entity(&actor).expect("insert actor");
    store.insert_entity(&place).expect("insert place");

    let relation = Relation::new(
        world.id(),
        actor.id(),
        place.id(),
        "guards",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        Some("charter".to_owned()),
        "{}",
    )
    .expect("relation");
    store.insert_relation(&relation).expect("insert relation");

    let goal = Goal::new(
        world.id(),
        actor.id(),
        "Keep the gate safe.",
        10,
        GoalStatus::Active,
        Some(Period::new(Some(1), None).expect("period")),
        GoalVisibility::Public,
        Some("oath".to_owned()),
    )
    .expect("goal");
    store.insert_goal(&goal).expect("insert goal");

    let consequence = Event::new(
        world.id(),
        "aftermath",
        "The gate holds.",
        "",
        EventTime::instant(3, TimePrecision::Exact, Certainty::Certain),
        Some(place.id()),
        vec![],
        vec![],
        1,
    )
    .expect("consequence event");
    store
        .insert_event(&EventAggregate::new(consequence.clone(), vec![]))
        .expect("insert consequence");

    let event = Event::new(
        world.id(),
        "defense",
        "Mara defends the gate.",
        "# Defense",
        EventTime::instant(2, TimePrecision::Exact, Certainty::Certain),
        Some(place.id()),
        vec![EventParticipant::new(actor.id(), "defender", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("event");
    let event_link =
        EventLink::new(event.id(), consequence.id(), EventLinkKind::Causes).expect("event link");
    let event_aggregate = EventAggregate::new(event.clone(), vec![event_link]);
    store.insert_event(&event_aggregate).expect("insert event");

    let document = Document::new(
        world.id(),
        "Gate Chronicle",
        "chronicle",
        Some(actor.id()),
        Some(actor.id()),
        DocumentCanonStatus::Canonical,
        "Mara defended the gate.",
        1,
    )
    .expect("document");
    let document_reference = ContentReference::new(
        ObjectRef::Document(document.id()),
        ObjectRef::Event(event.id()),
        0,
    );
    let document_aggregate =
        DocumentAggregate::new(document.clone(), vec![document_reference.clone()]);
    store
        .insert_document(&document_aggregate)
        .expect("insert document");

    let claim = Claim::new(
        world.id(),
        actor.id(),
        "Mara defended the gate.",
        Some("gate.defended_by".to_owned()),
        Some(ClaimObject::Entity(actor.id())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        Some("direct observation".to_owned()),
        Some("chronicle".to_owned()),
        Some(document.id()),
        None,
        None,
        Some(Period::new(Some(2), Some(2)).expect("claim period")),
        world.current_revision(),
    )
    .expect("claim");
    store.insert_claim(&claim).expect("insert claim");

    assert_eq!(
        store.get_rule(rule.id()).expect("get rule"),
        Some(rule.clone())
    );
    assert_eq!(
        store.get_entity(actor.id()).expect("get entity"),
        Some(actor.clone())
    );
    assert_eq!(
        store.get_relation(relation.id()).expect("get relation"),
        Some(relation.clone())
    );
    assert_eq!(
        store.get_goal(goal.id()).expect("get goal"),
        Some(goal.clone())
    );
    assert_eq!(
        store.get_event(event.id()).expect("get event"),
        Some(event_aggregate.clone())
    );
    assert_eq!(
        store.get_document(document.id()).expect("get document"),
        Some(document_aggregate.clone())
    );
    assert_eq!(
        store.get_claim(claim.id()).expect("get claim"),
        Some(claim.clone())
    );
    assert_eq!(store.list_rules().expect("list rules"), vec![rule.clone()]);
    assert_eq!(
        store.list_relations().expect("list relations"),
        vec![relation.clone()]
    );
    assert_eq!(store.list_goals().expect("list goals"), vec![goal.clone()]);
    assert_eq!(
        store.list_claims().expect("list claims"),
        vec![claim.clone()]
    );
    assert!(
        store
            .list_entities()
            .expect("list entities")
            .contains(&actor)
    );
    assert!(
        store
            .list_events()
            .expect("list events")
            .contains(&event_aggregate)
    );
    assert_eq!(
        store.list_documents().expect("list documents"),
        vec![document_aggregate.clone()]
    );

    let changed_rule = Rule::restore(
        rule.id(),
        world.id(),
        RuleKind::Institutional,
        "Oaths bind every guard.",
        "realm",
        RuleSeverity::Advisory,
        rule.source().map(str::to_owned),
        None,
        "{}",
        rule.version(),
        rule.created_at_ms(),
        2,
    )
    .expect("changed rule");
    let changed_actor = Entity::restore(
        actor.id(),
        world.id(),
        EntityKind::Person,
        "Mara Vale",
        "mara-vale",
        actor.summary(),
        actor.body_md(),
        actor.attributes_json().as_str(),
        vec!["The Captain".to_owned()],
        actor.version(),
        actor.created_at_ms(),
        2,
    )
    .expect("changed actor");
    let changed_relation = Relation::restore(
        relation.id(),
        world.id(),
        actor.id(),
        place.id(),
        "protects",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        relation.source_reference().map(str::to_owned),
        "{}",
        relation.version(),
    )
    .expect("changed relation");
    let changed_goal = Goal::restore(
        goal.id(),
        world.id(),
        actor.id(),
        "Keep the gate safe.",
        10,
        GoalStatus::Achieved,
        goal.period(),
        GoalVisibility::Public,
        goal.source().map(str::to_owned),
        goal.version(),
    )
    .expect("changed goal");
    let changed_event = Event::restore(
        event.id(),
        world.id(),
        event.kind(),
        "Mara saved the gate.",
        event.body_md(),
        *event.time(),
        event.location_entity_id(),
        vec![EventParticipant::new(actor.id(), "leader", 0).expect("participant")],
        vec![goal.id()],
        event.version(),
        event.created_at_ms(),
        2,
    )
    .expect("changed event");
    let changed_event_aggregate = EventAggregate::new(
        changed_event,
        vec![
            EventLink::new(event.id(), consequence.id(), EventLinkKind::Enables)
                .expect("changed link"),
        ],
    );
    let changed_document = Document::restore(
        document.id(),
        world.id(),
        "Revised Gate Chronicle",
        document.kind(),
        document.author_entity_id(),
        document.perspective_entity_id(),
        document.canon_status(),
        document.body_md(),
        document.version(),
        document.created_at_ms(),
        2,
    )
    .expect("changed document");
    let changed_document_aggregate = DocumentAggregate::new(
        changed_document,
        vec![ContentReference::new(
            ObjectRef::Document(document.id()),
            ObjectRef::Entity(actor.id()),
            0,
        )],
    );
    let changed_claim = Claim::restore(
        claim.id(),
        world.id(),
        actor.id(),
        "Mara saved the gate.",
        claim.predicate_key().map(str::to_owned),
        claim.object().cloned(),
        claim.polarity(),
        claim.authentication(),
        claim.holder_entity_id(),
        claim.modality(),
        claim.register().map(str::to_owned),
        claim.epistemic_basis().map(str::to_owned),
        claim.source().map(str::to_owned),
        claim.source_document_id(),
        claim.source_claim_id(),
        claim.holder_confidence(),
        claim.period(),
        claim.registered_revision_id(),
        claim.superseded_revision_id(),
        claim.version(),
    )
    .expect("changed claim");

    assert_eq!(
        store
            .update_rule(&changed_rule)
            .expect("update rule")
            .version(),
        2
    );
    assert_eq!(
        store
            .update_entity(&changed_actor)
            .expect("update entity")
            .version(),
        2
    );
    assert_eq!(
        store
            .update_relation(&changed_relation)
            .expect("update relation")
            .version(),
        2
    );
    assert_eq!(
        store
            .update_goal(&changed_goal)
            .expect("update goal")
            .version(),
        2
    );
    let stored_event = store
        .update_event(&changed_event_aggregate)
        .expect("update event");
    assert_eq!(stored_event.event().version(), 2);
    assert_eq!(stored_event.links(), changed_event_aggregate.links());
    let stored_document = store
        .update_document(&changed_document_aggregate)
        .expect("update document");
    assert_eq!(stored_document.object().version(), 2);
    assert_eq!(
        stored_document.references(),
        changed_document_aggregate.references()
    );
    assert_eq!(
        store
            .update_claim(&changed_claim)
            .expect("update claim")
            .version(),
        2
    );

    assert!(matches!(
        store.update_rule(&rule),
        Err(StoreError::StaleVersion { object: "rule", .. })
    ));
    assert!(matches!(
        store.update_entity(&actor),
        Err(StoreError::StaleVersion {
            object: "entity",
            ..
        })
    ));
    assert!(matches!(
        store.update_relation(&relation),
        Err(StoreError::StaleVersion {
            object: "relation",
            ..
        })
    ));
    assert!(matches!(
        store.update_goal(&goal),
        Err(StoreError::StaleVersion { object: "goal", .. })
    ));
    assert!(matches!(
        store.update_event(&event_aggregate),
        Err(StoreError::StaleVersion {
            object: "event",
            ..
        })
    ));
    assert!(matches!(
        store.update_document(&document_aggregate),
        Err(StoreError::StaleVersion {
            object: "document",
            ..
        })
    ));
    assert!(matches!(
        store.update_claim(&claim),
        Err(StoreError::StaleVersion {
            object: "claim",
            ..
        })
    ));

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn incomplete_event_aggregate_rolls_back_insert_and_update() {
    let path = project_path("aggregate-rollback");
    let world = World::new("Arcadia", "", "", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let missing_participant = Event::new(
        world.id(),
        "arrival",
        "",
        "",
        EventTime::unknown(Certainty::Uncertain),
        None,
        vec![
            EventParticipant::new(nirmata_core::EntityId::new(), "traveler", 0)
                .expect("participant"),
        ],
        vec![],
        1,
    )
    .expect("event");
    assert!(
        store
            .insert_event(&EventAggregate::new(missing_participant.clone(), vec![]))
            .is_err()
    );
    assert_eq!(
        store
            .get_event(missing_participant.id())
            .expect("event absent"),
        None
    );

    let incomplete_document = Document::new(
        world.id(),
        "Broken Chronicle",
        "chronicle",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "",
        1,
    )
    .expect("document");
    let missing_reference = ContentReference::new(
        ObjectRef::Document(incomplete_document.id()),
        ObjectRef::Entity(nirmata_core::EntityId::new()),
        0,
    );
    assert!(
        store
            .insert_document(&DocumentAggregate::new(
                incomplete_document.clone(),
                vec![missing_reference],
            ))
            .is_err()
    );
    assert_eq!(
        store
            .get_document(incomplete_document.id())
            .expect("document absent"),
        None
    );

    let actor = Entity::new(
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
    .expect("actor");
    store.insert_entity(&actor).expect("insert actor");
    let event = Event::new(
        world.id(),
        "arrival",
        "Original",
        "",
        EventTime::unknown(Certainty::Uncertain),
        None,
        vec![EventParticipant::new(actor.id(), "traveler", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("event");
    let original = EventAggregate::new(event.clone(), vec![]);
    store.insert_event(&original).expect("insert event");
    let invalid_update = Event::restore(
        event.id(),
        world.id(),
        event.kind(),
        "Must roll back",
        event.body_md(),
        *event.time(),
        None,
        event.participants().to_vec(),
        vec![nirmata_core::GoalId::new()],
        event.version(),
        event.created_at_ms(),
        2,
    )
    .expect("replacement");
    assert!(
        store
            .update_event(&EventAggregate::new(invalid_update, vec![]))
            .is_err()
    );
    assert_eq!(
        store.get_event(event.id()).expect("original survives"),
        Some(original)
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn reopens_and_recovers_change_set_storage() {
    let path = project_path("change-set-recovery");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let (_entity, operation) = create_entity_change_operation(&world, 2);
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Add Mara",
        vec![ObjectRef::Document(nirmata_core::DocumentId::new())],
        vec!["Mara appears after the siege.".to_owned()],
        vec![operation.clone()],
        vec![],
    )
    .expect("draft");
    let draft_record = ChangeSetDraftRecord::new(
        draft.clone(),
        Some(json!({
            "issues": [],
            "summary": "draft is structurally valid"
        })),
        2,
        3,
    );
    store
        .save_change_set_draft(&draft_record)
        .expect("save draft");

    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Add Mara",
        draft.sources().to_vec(),
        draft.assumptions().to_vec(),
        vec![operation.clone()],
        vec![],
    )
    .expect("change set");
    let revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(change_set.id()),
        "manual_review",
        "Add Mara to the canon",
        4,
    )
    .expect("revision");
    let committed = CommittedChangeSetRecord::new(
        change_set.clone(),
        Some(json!({
            "issues": [],
            "summary": "ready to commit"
        })),
        vec![],
        vec![
            OperationAudit::from_operation(
                &operation,
                OperationDecision::Accept,
                "manual_review",
                4,
            )
            .expect("audit"),
        ],
        revision.clone(),
        None,
    )
    .expect("committed");
    store
        .commit_change_set(&committed)
        .expect("commit change set");
    drop(store);

    let reopened = WorldStore::open(&path).expect("reopen store");
    assert_eq!(
        reopened
            .get_change_set_draft(draft.id())
            .expect("load draft after reopen"),
        Some(draft_record)
    );
    assert_eq!(
        reopened
            .get_committed_change_set(change_set.id())
            .expect("load committed change set"),
        Some(committed)
    );
    assert_eq!(
        reopened.get_revision(revision.id()).expect("load revision"),
        Some(revision.clone())
    );
    assert_eq!(
        reopened
            .load_world()
            .expect("world after reopen")
            .current_revision(),
        revision.id()
    );
    assert_eq!(
        reopened
            .get_entity(_entity.id())
            .expect("load committed entity"),
        Some(_entity)
    );
    assert_eq!(reopened.list_revisions().expect("list revisions").len(), 2);

    drop(reopened);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn commit_applies_update_operations_with_next_version_snapshots() {
    let path = project_path("commit-update-operation");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let mara = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "",
        "",
        "{}",
        vec![],
        2,
    )
    .expect("entity");
    store.insert_entity(&mara).expect("insert entity");
    let after = renamed_entity(&mara, "Mara of the Gate", "mara-gate", 3);
    let operation = update_entity_operation(&mara, &after);
    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Rename Mara",
        vec![ObjectRef::Entity(mara.id())],
        vec![],
        vec![operation.clone()],
        vec![],
    )
    .expect("change set");
    let revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(change_set.id()),
        "manual_review",
        "Rename Mara",
        4,
    )
    .expect("revision");
    let committed = CommittedChangeSetRecord::new(
        change_set,
        Some(json!({ "issues": [] })),
        vec![],
        vec![
            OperationAudit::from_operation(
                &operation,
                OperationDecision::Accept,
                "manual_review",
                4,
            )
            .expect("audit"),
        ],
        revision.clone(),
        None,
    )
    .expect("committed record");

    store
        .commit_change_set(&committed)
        .expect("commit update change set");
    let stored = store
        .get_entity(mara.id())
        .expect("load renamed entity")
        .expect("stored entity");
    assert_eq!(stored.name(), "Mara of the Gate");
    assert_eq!(stored.slug(), "mara-gate");
    assert_eq!(stored.version(), after.version());
    assert_eq!(
        store
            .load_world()
            .expect("world after update")
            .current_revision(),
        revision.id()
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn reopens_and_recovers_linear_undo_metadata() {
    let path = project_path("change-set-undo-recovery");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let (entity, create_operation) = create_entity_change_operation(&world, 2);
    let create_change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Add Mara",
        vec![],
        vec![],
        vec![create_operation.clone()],
        vec![],
    )
    .expect("create change set");
    let create_revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(create_change_set.id()),
        "manual_review",
        "Add Mara",
        3,
    )
    .expect("create revision");
    store
        .commit_change_set(
            &CommittedChangeSetRecord::new(
                create_change_set.clone(),
                Some(json!({ "issues": [] })),
                vec![],
                vec![
                    OperationAudit::from_operation(
                        &create_operation,
                        OperationDecision::Accept,
                        "manual_review",
                        3,
                    )
                    .expect("create audit"),
                ],
                create_revision.clone(),
                None,
            )
            .expect("create record"),
        )
        .expect("commit create change set");

    let delete_operation = delete_entity_operation(&entity, RetconKind::Additive);
    let undo_change_set = ChangeSet::new(
        world.id(),
        create_revision.id(),
        "Undo revision",
        vec![ObjectRef::Entity(entity.id())],
        vec![],
        vec![delete_operation.clone()],
        vec![],
    )
    .expect("undo change set");
    let undo_revision = StoredRevision::new(
        world.id(),
        Some(create_revision.id()),
        Some(undo_change_set.id()),
        "undo",
        "Undo revision",
        4,
    )
    .expect("undo revision");
    let undo_record = CommittedChangeSetRecord::new(
        undo_change_set.clone(),
        Some(json!({ "issues": [] })),
        vec![],
        vec![
            OperationAudit::from_operation(&delete_operation, OperationDecision::Accept, "undo", 4)
                .expect("undo audit"),
        ],
        undo_revision.clone(),
        Some(create_revision.id()),
    )
    .expect("undo record");
    store
        .commit_change_set(&undo_record)
        .expect("commit undo change set");
    drop(store);

    let reopened = WorldStore::open(&path).expect("reopen store");
    let recovered = reopened
        .get_committed_change_set(undo_change_set.id())
        .expect("load undo change set")
        .expect("stored undo change set");
    assert_eq!(recovered.undone_revision_id(), Some(create_revision.id()));
    assert_eq!(
        reopened.get_entity(entity.id()).expect("entity after undo"),
        None
    );
    assert_eq!(
        reopened
            .load_world()
            .expect("world after reopen")
            .current_revision(),
        undo_revision.id()
    );

    drop(reopened);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn commit_rolls_back_canon_and_revision_on_constraint_failure() {
    let path = project_path("commit-constraint-rollback");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let existing = Entity::new(
        world.id(),
        EntityKind::Person,
        "Existing Mara",
        "mara",
        "",
        "",
        "{}",
        vec![],
        2,
    )
    .expect("existing entity");
    store
        .insert_entity(&existing)
        .expect("insert existing entity");

    let conflicting = Entity::new(
        world.id(),
        EntityKind::Person,
        "Conflicting Mara",
        "mara",
        "",
        "",
        "{}",
        vec![],
        3,
    )
    .expect("conflicting entity");
    let operation = ChangeOperation::CreateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(conflicting.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: conflicting.clone(),
    };
    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Add conflicting entity",
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
        "manual_review",
        "Conflict revision",
        4,
    )
    .expect("revision");
    let committed = CommittedChangeSetRecord::new(
        change_set,
        Some(json!({ "issues": [] })),
        vec![],
        vec![
            OperationAudit::from_operation(
                &operation,
                OperationDecision::Accept,
                "manual_review",
                4,
            )
            .expect("audit"),
        ],
        revision.clone(),
        None,
    )
    .expect("committed record");

    let error = store
        .commit_change_set(&committed)
        .expect_err("constraint failure must roll back");
    assert!(matches!(error, StoreError::Database(_, _)));
    assert_eq!(
        store
            .load_world()
            .expect("world after rollback")
            .current_revision(),
        world.current_revision()
    );
    assert_eq!(
        store
            .list_revisions()
            .expect("revisions after rollback")
            .len(),
        1
    );
    assert_eq!(
        store
            .get_entity(existing.id())
            .expect("existing entity survives"),
        Some(existing)
    );
    assert_eq!(
        store
            .get_entity(conflicting.id())
            .expect("conflicting entity absent"),
        None
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn rejects_a_second_revision_head() {
    let path = project_path("second-head");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let (_entity, first_operation) = create_entity_change_operation(&world, 2);
    let first_change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Add Mara",
        vec![],
        vec![],
        vec![first_operation.clone()],
        vec![],
    )
    .expect("first change set");
    let first_revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(first_change_set.id()),
        "manual_review",
        "First revision",
        3,
    )
    .expect("first revision");
    store
        .commit_change_set(
            &CommittedChangeSetRecord::new(
                first_change_set,
                Some(json!({ "issues": [] })),
                vec![],
                vec![
                    OperationAudit::from_operation(
                        &first_operation,
                        OperationDecision::Accept,
                        "manual_review",
                        3,
                    )
                    .expect("first audit"),
                ],
                first_revision.clone(),
                None,
            )
            .expect("first record"),
        )
        .expect("commit first change set");

    let (_entity, stale_operation) = create_entity_change_operation(&world, 4);
    let stale_change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Add Talia from stale base",
        vec![],
        vec![],
        vec![stale_operation.clone()],
        vec![],
    )
    .expect("stale change set");
    let stale_revision = StoredRevision::restore(
        RevisionId::new(),
        world.id(),
        Some(world.current_revision()),
        Some(stale_change_set.id()),
        "manual_review",
        "Stale revision",
        4,
    )
    .expect("stale revision");
    let error = store
        .commit_change_set(
            &CommittedChangeSetRecord::new(
                stale_change_set,
                Some(json!({ "issues": [] })),
                vec![],
                vec![
                    OperationAudit::from_operation(
                        &stale_operation,
                        OperationDecision::Accept,
                        "manual_review",
                        4,
                    )
                    .expect("stale audit"),
                ],
                stale_revision,
                None,
            )
            .expect("stale record"),
        )
        .expect_err("reject stale head");
    assert!(matches!(
        error,
        StoreError::StaleRevision {
            expected_current,
            found_base,
        } if expected_current == first_revision.id() && found_base == world.current_revision()
    ));
    assert_eq!(
        store
            .load_world()
            .expect("world after rejection")
            .current_revision(),
        first_revision.id()
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn replacement_requires_target_reason_and_resolved_decision() {
    let path = project_path("replacement-decision");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
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
    .expect("entity");
    store.insert_entity(&mara).expect("insert entity");

    let operation = delete_entity_operation(&mara, RetconKind::Replacement);
    let decision = DecisionPoint::new(
        vec![operation.operation_id()],
        "Should Mara be replaced?",
        vec!["Keep Mara".to_owned(), "Replace Mara".to_owned()],
    )
    .expect("decision");
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Replace Mara",
        vec![],
        vec![],
        vec![operation],
        vec![decision],
    )
    .expect("draft");

    let report = store
        .validate_change_set_draft(&draft)
        .expect("validate draft");

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.replacement_target_missing")
    );
    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.replacement_reason_missing")
    );
    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.replacement_decision_unresolved")
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn additive_retcon_cannot_delete_canon() {
    let path = project_path("additive-delete");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
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
    .expect("entity");
    store.insert_entity(&mara).expect("insert entity");

    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Delete Mara additively",
        vec![],
        vec![],
        vec![delete_entity_operation(&mara, RetconKind::Additive)],
        vec![],
    )
    .expect("draft");

    let report = store
        .validate_change_set_draft(&draft)
        .expect("validate draft");

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.retcon.additive_delete")
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn attributed_claims_from_distinct_holders_coexist() {
    let path = project_path("claims-distinct-holders");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
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
    let witness_one = Entity::new(
        world.id(),
        EntityKind::Person,
        "Witness One",
        "witness-one",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("holder one");
    let witness_two = Entity::new(
        world.id(),
        EntityKind::Person,
        "Witness Two",
        "witness-two",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("holder two");
    for entity in [&gate, &witness_one, &witness_two] {
        store.insert_entity(entity).expect("insert entity");
    }

    let existing = attributed_claim(
        &world,
        gate.id(),
        witness_one.id(),
        "rumor",
        ClaimPolarity::Positive,
    );
    store.insert_claim(&existing).expect("insert claim");

    let proposed = attributed_claim(
        &world,
        gate.id(),
        witness_two.id(),
        "testimony",
        ClaimPolarity::Negative,
    );
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Record a second perspective",
        vec![],
        vec![],
        vec![create_claim_operation(&proposed, RetconKind::Additive)],
        vec![],
    )
    .expect("draft");

    let report = store
        .validate_change_set_draft(&draft)
        .expect("validate draft");

    assert!(report.is_ok());
    assert!(
        report
            .conflicts
            .iter()
            .all(|issue| issue.code != "claim.canonical_opposition")
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn opposite_canonical_claims_in_same_context_block_validation() {
    let path = project_path("claims-canonical-opposition");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
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
    store.insert_entity(&gate).expect("insert gate");

    let existing = canonical_claim(
        &world,
        gate.id(),
        "gate.open",
        ClaimObject::Scalar("true".to_owned()),
        ClaimPolarity::Positive,
        Period::new(Some(10), Some(10)).expect("period"),
    );
    store.insert_claim(&existing).expect("insert claim");

    let proposed = canonical_claim(
        &world,
        gate.id(),
        "gate.open",
        ClaimObject::Scalar("true".to_owned()),
        ClaimPolarity::Negative,
        Period::new(Some(10), Some(10)).expect("period"),
    );
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Contradict the gate state",
        vec![],
        vec![],
        vec![create_claim_operation(&proposed, RetconKind::Additive)],
        vec![],
    )
    .expect("draft");

    let report = store
        .validate_change_set_draft(&draft)
        .expect("validate draft");

    assert!(
        report
            .conflicts
            .iter()
            .any(|issue| issue.code == "claim.canonical_opposition")
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn content_references_make_deleting_a_referenced_object_fail() {
    let path = project_path("content-reference-impact");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
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
    .expect("entity");
    store.insert_entity(&mara).expect("insert entity");
    let document = Document::new(
        world.id(),
        "Chronicle",
        "chronicle",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "Mara appears in the chronicle.",
        1,
    )
    .expect("document");
    store
        .insert_document(&DocumentAggregate::new(
            document.clone(),
            vec![ContentReference::new(
                ObjectRef::Document(document.id()),
                ObjectRef::Entity(mara.id()),
                0,
            )],
        ))
        .expect("insert document");

    let operation = delete_entity_operation(&mara, RetconKind::Replacement);
    let decision = DecisionPoint::new_replacement(
        vec![operation.operation_id()],
        "Replace Mara with another witness?",
        vec!["Keep Mara".to_owned(), "Replace Mara".to_owned()],
        ObjectRef::Entity(mara.id()),
        "The chronicle misidentified the actor.",
        "Replace Mara",
    )
    .expect("replacement decision");
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Replace Mara in the chronicle",
        vec![],
        vec![],
        vec![operation],
        vec![decision],
    )
    .expect("draft");

    let report = store
        .validate_change_set_draft(&draft)
        .expect("validate draft");

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.delete_orphan")
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn affected_graph_includes_rules_and_dependent_events() {
    let path = project_path("affected-graph-rules-events");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
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
    .expect("entity");
    store.insert_entity(&mara).expect("insert entity");
    let rule = Rule::new(
        world.id(),
        RuleKind::Constitutive,
        "The dead do not return.",
        "world",
        RuleSeverity::Hard,
        None,
        Some(RuleValidatorKind::NoResurrection),
        "{}",
        1,
    )
    .expect("rule");
    store.insert_rule(&rule).expect("insert rule");
    let death = Event::new(
        world.id(),
        "death",
        "Mara dies.",
        "",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(mara.id(), "subject", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("death event");
    store
        .insert_event(&EventAggregate::new(death, vec![]))
        .expect("insert death");

    let return_event = Event::new(
        world.id(),
        "return",
        "Mara returns.",
        "",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(mara.id(), "actor", 0).expect("participant")],
        vec![],
        1,
    )
    .expect("return event");
    let draft = ChangeSetDraft::new(
        world.id(),
        world.current_revision(),
        "Bring Mara back",
        vec![],
        vec![],
        vec![ChangeOperation::CreateEvent {
            operation_id: nirmata_core::ChangeOperationId::new(),
            affected_ids: vec![
                ObjectRef::Event(return_event.id()),
                ObjectRef::Entity(mara.id()),
            ],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: EventAggregate::new(return_event, vec![]),
        }],
        vec![],
    )
    .expect("draft");

    let report = store
        .validate_change_set_draft(&draft)
        .expect("validate draft");

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "rule.no_resurrection")
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn public_api_is_domain_oriented() {
    let _: fn(&mut WorldStore, &Rule) -> Result<(), StoreError> = WorldStore::insert_rule;
    let _: fn(&WorldStore, nirmata_core::RuleId) -> Result<Option<Rule>, StoreError> =
        WorldStore::get_rule;
    let _: fn(&WorldStore) -> Result<Vec<Entity>, StoreError> = WorldStore::list_entities;
    let _: fn(&WorldStore, &str) -> Result<Vec<ObjectRef>, StoreError> =
        WorldStore::search_canon_text;
    let _: fn(&mut WorldStore) -> Result<(), StoreError> = WorldStore::rebuild_canon_text_index;
    let _: fn(&mut WorldStore, &EventAggregate) -> Result<EventAggregate, StoreError> =
        WorldStore::update_event;
    let _: fn(&mut WorldStore, &DocumentAggregate) -> Result<(), StoreError> =
        WorldStore::insert_document;
    let _: fn(&mut WorldStore, &ChangeSetDraftRecord) -> Result<(), StoreError> =
        WorldStore::save_change_set_draft;
    let _: fn(
        &WorldStore,
        nirmata_core::ChangeSetId,
    ) -> Result<Option<ChangeSetDraftRecord>, StoreError> = WorldStore::get_change_set_draft;
    let _: fn(
        &WorldStore,
        &ChangeSetDraft,
    ) -> Result<nirmata_core::validation::ValidationReport, StoreError> =
        WorldStore::validate_change_set_draft;
    let _: fn(&mut WorldStore, &CommittedChangeSetRecord) -> Result<StoredRevision, StoreError> =
        WorldStore::commit_change_set;
}

#[test]
fn text_search_indexes_supported_canon_fields_and_rebuilds_equivalently() {
    let path = project_path("fts-rebuild");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let subject = Entity::new(
        world.id(),
        EntityKind::Person,
        "Starwatcher",
        "starwatcher",
        "Chartkeeper",
        "Moonfire charts the outer reefs.",
        "{}",
        vec![],
        1,
    )
    .expect("subject");
    store.insert_entity(&subject).expect("insert subject");

    let rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Oathglass must be witnessed before dawn.",
        "realm",
        RuleSeverity::Advisory,
        None,
        None,
        "{}",
        1,
    )
    .expect("rule");
    store.insert_rule(&rule).expect("insert rule");

    let event = Event::new(
        world.id(),
        "ritual",
        "Dawnbreak begins.",
        "Sunspike banners were raised over the harbor.",
        EventTime::instant(2, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        1,
    )
    .expect("event");
    store
        .insert_event(&EventAggregate::new(event.clone(), vec![]))
        .expect("insert event");

    let claim = Claim::new(
        world.id(),
        subject.id(),
        "Silver rumor says the reef moved.",
        None,
        None,
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        world.current_revision(),
    )
    .expect("claim");
    store.insert_claim(&claim).expect("insert claim");

    let document = Document::new(
        world.id(),
        "Night Ledger",
        "chronicle",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "Inkstone copies were locked in the archive.",
        1,
    )
    .expect("document");
    store
        .insert_document(&DocumentAggregate::new(document.clone(), vec![]))
        .expect("insert document");

    let expected = vec![
        ("Oathglass", ObjectRef::Rule(rule.id())),
        ("Starwatcher", ObjectRef::Entity(subject.id())),
        ("Chartkeeper", ObjectRef::Entity(subject.id())),
        ("Moonfire", ObjectRef::Entity(subject.id())),
        ("Dawnbreak", ObjectRef::Event(event.id())),
        ("Sunspike", ObjectRef::Event(event.id())),
        ("Silver", ObjectRef::Claim(claim.id())),
        ("Night", ObjectRef::Document(document.id())),
        ("Inkstone", ObjectRef::Document(document.id())),
    ];
    for (term, target) in &expected {
        assert_eq!(
            store
                .search_canon_text(term)
                .expect("search before rebuild"),
            vec![*target]
        );
    }

    store
        .rebuild_canon_text_index()
        .expect("rebuild text search index");

    for (term, target) in &expected {
        assert_eq!(
            store.search_canon_text(term).expect("search after rebuild"),
            vec![*target]
        );
    }

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn text_search_tracks_updated_text() {
    let path = project_path("fts-update");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let entity = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Old compass bearer",
        "Lantern maps.",
        "{}",
        vec![],
        1,
    )
    .expect("entity");
    store.insert_entity(&entity).expect("insert entity");
    assert_eq!(
        store
            .search_canon_text("compass")
            .expect("search old summary"),
        vec![ObjectRef::Entity(entity.id())]
    );
    assert_eq!(
        store
            .search_canon_text("Lantern")
            .expect("search old markdown"),
        vec![ObjectRef::Entity(entity.id())]
    );

    let updated = Entity::restore(
        entity.id(),
        entity.world_id(),
        entity.kind(),
        "Mara Tide",
        entity.slug().to_owned(),
        "New astrolabe keeper".to_owned(),
        "Harbor songs.".to_owned(),
        entity.attributes_json().as_str().to_owned(),
        entity.aliases().to_vec(),
        entity.version(),
        entity.created_at_ms(),
        2,
    )
    .expect("updated entity");
    store.update_entity(&updated).expect("update entity");

    assert!(
        store
            .search_canon_text("compass")
            .expect("search removed summary")
            .is_empty()
    );
    assert!(
        store
            .search_canon_text("Lantern")
            .expect("search removed markdown")
            .is_empty()
    );
    assert_eq!(
        store
            .search_canon_text("astrolabe")
            .expect("search new summary"),
        vec![ObjectRef::Entity(entity.id())]
    );
    assert_eq!(
        store
            .search_canon_text("Harbor")
            .expect("search new markdown"),
        vec![ObjectRef::Entity(entity.id())]
    );
    assert_eq!(
        store.search_canon_text("Tide").expect("search new name"),
        vec![ObjectRef::Entity(entity.id())]
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn text_search_drops_deleted_objects_after_commit() {
    let path = project_path("fts-delete");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

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
    assert_eq!(
        store
            .search_canon_text("Stormglass")
            .expect("search before delete"),
        vec![ObjectRef::Document(document.id())]
    );

    let operation = ChangeOperation::DeleteDocument {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Document(document.id())],
        expected_version: document.version(),
        retcon: RetconKind::Replacement,
        before: DocumentAggregate::new(document.clone(), vec![]),
    };
    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        "Delete harbor ledger",
        vec![],
        vec![],
        vec![operation.clone()],
        vec![
            DecisionPoint::new_replacement(
                vec![operation.operation_id()],
                "Should the harbor ledger be removed?",
                vec!["Keep it".to_owned(), "Remove it".to_owned()],
                ObjectRef::Document(document.id()),
                "The ledger was copied into another canon source.",
                "Remove it",
            )
            .expect("replacement decision"),
        ],
    )
    .expect("change set");
    let revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(change_set.id()),
        "manual_review",
        "Delete harbor ledger",
        2,
    )
    .expect("revision");
    let committed = CommittedChangeSetRecord::new(
        change_set,
        Some(json!({ "issues": [] })),
        vec![],
        vec![
            OperationAudit::from_operation(
                &operation,
                OperationDecision::Accept,
                "manual_review",
                2,
            )
            .expect("audit"),
        ],
        revision,
        None,
    )
    .expect("committed record");

    store
        .commit_change_set(&committed)
        .expect("commit delete change set");

    assert!(
        store
            .search_canon_text("Stormglass")
            .expect("search after delete")
            .is_empty()
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn structured_search_matches_entity_aliases() {
    let path = project_path("structured-search-alias");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let entity = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec!["The Witness".to_owned()],
        1,
    )
    .expect("entity");
    store.insert_entity(&entity).expect("insert entity");

    let hits = store
        .search_structured(&StructuredSearchQuery {
            alias: Some("the witness".to_owned()),
            limit: 10,
            ..Default::default()
        })
        .expect("search alias");

    assert_eq!(
        hits,
        vec![StructuredSearchHit {
            object: ObjectRef::Entity(entity.id()),
            fragment: "Mara Cartographer".to_owned(),
            provenance: "alias:The Witness".to_owned(),
            stage: StructuredSearchStage::Alias,
        }]
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn structured_search_filters_neighbor_relations_by_type() {
    let path = project_path("structured-search-neighbor");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let actor = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("actor");
    let place = Entity::new(
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
    .expect("place");
    store.insert_entity(&actor).expect("insert actor");
    store.insert_entity(&place).expect("insert place");

    let relation = Relation::new(
        world.id(),
        actor.id(),
        place.id(),
        "guards",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        Some("charter".to_owned()),
        "{}",
    )
    .expect("relation");
    store.insert_relation(&relation).expect("insert relation");

    let hits = store
        .search_structured(&StructuredSearchQuery {
            kinds: vec![StructuredSearchKind::Relation],
            neighbors_of: vec![ObjectRef::Entity(actor.id())],
            limit: 10,
            ..Default::default()
        })
        .expect("search neighbors");

    assert_eq!(
        hits,
        vec![StructuredSearchHit {
            object: ObjectRef::Relation(relation.id()),
            fragment: "guards".to_owned(),
            provenance: format!("neighbor:{}", ObjectRef::Entity(actor.id())),
            stage: StructuredSearchStage::Neighbor,
        }]
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn structured_search_combines_goal_and_tick_filters() {
    let path = project_path("structured-search-goal");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let actor = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("actor");
    store.insert_entity(&actor).expect("insert actor");

    let goal = Goal::new(
        world.id(),
        actor.id(),
        "Keep the north gate standing.",
        10,
        GoalStatus::Active,
        Some(Period::new(Some(10), Some(20)).expect("period")),
        GoalVisibility::Public,
        Some("oath".to_owned()),
    )
    .expect("goal");
    store.insert_goal(&goal).expect("insert goal");

    let event = Event::new(
        world.id(),
        "defense",
        "The gate holds at dawn.",
        "",
        EventTime::instant(12, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(actor.id(), "defender", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("event");
    store
        .insert_event(&EventAggregate::new(event.clone(), vec![]))
        .expect("insert event");

    let later_event = Event::new(
        world.id(),
        "siege",
        "A later siege begins.",
        "",
        EventTime::instant(30, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![EventParticipant::new(actor.id(), "defender", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("later event");
    store
        .insert_event(&EventAggregate::new(later_event, vec![]))
        .expect("insert later event");

    let hits = store
        .search_structured(&StructuredSearchQuery {
            kinds: vec![StructuredSearchKind::Event],
            goal_ids: vec![goal.id()],
            temporal: Some(StructuredSearchTemporal::Tick(12)),
            limit: 10,
            ..Default::default()
        })
        .expect("search goal");

    assert_eq!(
        hits,
        vec![StructuredSearchHit {
            object: ObjectRef::Event(event.id()),
            fragment: "The gate holds at dawn.".to_owned(),
            provenance: format!("goal:{}:event", goal.id()),
            stage: StructuredSearchStage::Goal,
        }]
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn structured_search_combines_period_and_perspective_filters() {
    let path = project_path("structured-search-perspective");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let subject = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("subject");
    let holder = Entity::new(
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
    .expect("holder");
    store.insert_entity(&subject).expect("insert subject");
    store.insert_entity(&holder).expect("insert holder");

    let in_period = Claim::new(
        world.id(),
        subject.id(),
        "Sera swears Mara hid the ember.",
        Some("ember.hidden".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(holder.id()),
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
        subject.id(),
        "Sera repeats the rumor later.",
        Some("ember.hidden".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(holder.id()),
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
    .expect("claim");
    store.insert_claim(&in_period).expect("insert claim");
    store
        .insert_claim(&out_of_period)
        .expect("insert later claim");

    let document = Document::new(
        world.id(),
        "Sera's Journal",
        "chronicle",
        Some(holder.id()),
        Some(holder.id()),
        DocumentCanonStatus::Canonical,
        "Sera writes the same rumor down.",
        1,
    )
    .expect("document");
    store
        .insert_document(&DocumentAggregate::new(document, vec![]))
        .expect("insert document");

    let hits = store
        .search_structured(&StructuredSearchQuery {
            perspective_entity_ids: vec![holder.id()],
            temporal: Some(StructuredSearchTemporal::Period(
                Period::new(Some(10), Some(20)).expect("period"),
            )),
            limit: 10,
            ..Default::default()
        })
        .expect("search period and perspective");

    assert_eq!(
        hits,
        vec![StructuredSearchHit {
            object: ObjectRef::Claim(in_period.id()),
            fragment: "Sera swears Mara hid the ember.".to_owned(),
            provenance: format!("perspective:{}:claim", holder.id()),
            stage: StructuredSearchStage::Perspective,
        }]
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn structured_search_uses_fts5_fragments() {
    let path = project_path("structured-search-fts");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

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

    let hits = store
        .search_structured(&StructuredSearchQuery {
            text: Some("Stormglass".to_owned()),
            limit: 10,
            ..Default::default()
        })
        .expect("search text");

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].object, ObjectRef::Document(document.id()));
    assert_eq!(hits[0].stage, StructuredSearchStage::Text);
    assert_eq!(hits[0].provenance, "fts5");
    assert!(hits[0].fragment.contains("Stormglass"));

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn structured_search_respects_limits() {
    let path = project_path("structured-search-limit");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let mut documents = Vec::new();
    for title in ["Harbor Ledger", "Harbor Chronicle", "Harbor Orders"] {
        let document = Document::new(
            world.id(),
            title,
            "chronicle",
            None,
            None,
            DocumentCanonStatus::Canonical,
            "Harbor entries stay searchable.",
            1,
        )
        .expect("document");
        store
            .insert_document(&DocumentAggregate::new(document.clone(), vec![]))
            .expect("insert document");
        documents.push(ObjectRef::Document(document.id()));
    }

    let hits = store
        .search_structured(&StructuredSearchQuery {
            text: Some("Harbor".to_owned()),
            limit: 2,
            ..Default::default()
        })
        .expect("search limit");

    assert_eq!(hits.len(), 2);
    assert!(
        hits.iter()
            .all(|hit| hit.stage == StructuredSearchStage::Text)
    );
    assert!(hits.iter().all(|hit| documents.contains(&hit.object)));

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn anchor_context_prioritizes_anchors_and_loads_related_records() {
    let path = project_path("anchor-context");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let actor = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("actor");
    let witness = Entity::new(
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
    .expect("witness");
    let place = Entity::new(
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
    .expect("place");
    let outpost = Entity::new(
        world.id(),
        EntityKind::Place,
        "South Outpost",
        "south-outpost",
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("outpost");
    let outsider = Entity::new(
        world.id(),
        EntityKind::Person,
        "Iven",
        "iven",
        "Unrelated traveler",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("outsider");
    for entity in [&actor, &witness, &place, &outpost, &outsider] {
        store.insert_entity(entity).expect("insert entity");
    }

    let guarded_relation = Relation::new(
        world.id(),
        actor.id(),
        place.id(),
        "guards",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        Some("charter".to_owned()),
        "{}",
    )
    .expect("relation");
    let patrol_relation = Relation::new(
        world.id(),
        actor.id(),
        outpost.id(),
        "patrols",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        Some("orders".to_owned()),
        "{}",
    )
    .expect("relation");
    store
        .insert_relation(&guarded_relation)
        .expect("insert relation");
    store
        .insert_relation(&patrol_relation)
        .expect("insert relation");

    let goal = Goal::new(
        world.id(),
        actor.id(),
        "Keep the north gate standing.",
        10,
        GoalStatus::Active,
        Some(Period::new(Some(10), Some(20)).expect("period")),
        GoalVisibility::Public,
        Some("oath".to_owned()),
    )
    .expect("goal");
    store.insert_goal(&goal).expect("insert goal");

    let event = Event::new(
        world.id(),
        "defense",
        "Mara and Sera hold the gate.",
        "",
        EventTime::instant(12, TimePrecision::Exact, Certainty::Certain),
        Some(place.id()),
        vec![
            EventParticipant::new(actor.id(), "defender", 0).expect("participant"),
            EventParticipant::new(witness.id(), "witness", 1).expect("participant"),
        ],
        vec![goal.id()],
        1,
    )
    .expect("event");
    store
        .insert_event(&EventAggregate::new(event.clone(), vec![]))
        .expect("insert event");

    let claim = Claim::new(
        world.id(),
        actor.id(),
        "Sera swears Mara held the gate.",
        Some("gate.held".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(witness.id()),
        Some(ClaimModality::Belief),
        Some("report".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.8),
        Some(Period::new(Some(12), Some(12)).expect("period")),
        world.current_revision(),
    )
    .expect("claim");
    let unrelated_claim = Claim::new(
        world.id(),
        outsider.id(),
        "Iven mentions an unrelated harbor rumor.",
        Some("harbor.rumor".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(outsider.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.4),
        Some(Period::new(Some(30), Some(30)).expect("period")),
        world.current_revision(),
    )
    .expect("claim");
    store.insert_claim(&claim).expect("insert claim");
    store
        .insert_claim(&unrelated_claim)
        .expect("insert unrelated claim");

    let world_rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "The world keeps oath records.",
        "world",
        RuleSeverity::Advisory,
        None,
        None,
        "{}",
        1,
    )
    .expect("rule");
    let person_rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "People must honor their sworn posts.",
        "person",
        RuleSeverity::Advisory,
        None,
        None,
        "{}",
        1,
    )
    .expect("rule");
    let place_rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Places have toll schedules.",
        "place",
        RuleSeverity::Advisory,
        None,
        None,
        "{}",
        1,
    )
    .expect("rule");
    for rule in [&world_rule, &person_rule, &place_rule] {
        store.insert_rule(rule).expect("insert rule");
    }

    let bundle = store
        .load_anchor_context(&AnchorContextQuery {
            anchors: vec![ObjectRef::Entity(actor.id())],
            relation_limit: 1,
        })
        .expect("load anchor context");

    let ordered = bundle.ordered_entries();
    assert_eq!(
        ordered[0].object.object_ref(),
        ObjectRef::Entity(actor.id())
    );
    assert_eq!(
        bundle.anchors[0].provenance,
        format!("anchor:{}", ObjectRef::Entity(actor.id()))
    );
    assert_eq!(bundle.relations.len(), 1);
    assert!(
        bundle.relations[0].object.object_ref() == ObjectRef::Relation(guarded_relation.id())
            || bundle.relations[0].object.object_ref() == ObjectRef::Relation(patrol_relation.id())
    );
    assert_eq!(bundle.events.len(), 1);
    assert_eq!(
        bundle.events[0].object.object_ref(),
        ObjectRef::Event(event.id())
    );
    assert_eq!(
        bundle.events[0].provenance,
        format!("event:{}", ObjectRef::Entity(actor.id()))
    );
    assert_eq!(bundle.participants.len(), 1);
    assert_eq!(
        bundle.participants[0].object.object_ref(),
        ObjectRef::Entity(witness.id())
    );
    assert_eq!(
        bundle.participants[0].provenance,
        format!("participant:{}", ObjectRef::Event(event.id()))
    );
    assert_eq!(bundle.claims.len(), 1);
    assert_eq!(
        bundle.claims[0].object.object_ref(),
        ObjectRef::Claim(claim.id())
    );
    assert_eq!(
        bundle.claims[0].provenance,
        format!("claim:{}", ObjectRef::Entity(actor.id()))
    );
    assert_eq!(bundle.goals.len(), 1);
    assert_eq!(
        bundle.goals[0].object.object_ref(),
        ObjectRef::Goal(goal.id())
    );
    assert_eq!(
        bundle.goals[0].provenance,
        format!("goal:{}", ObjectRef::Entity(actor.id()))
    );
    let rule_refs: Vec<_> = bundle
        .rules
        .iter()
        .map(|entry| entry.object.object_ref())
        .collect();
    assert!(rule_refs.contains(&ObjectRef::Rule(world_rule.id())));
    assert!(rule_refs.contains(&ObjectRef::Rule(person_rule.id())));
    assert!(!rule_refs.contains(&ObjectRef::Rule(place_rule.id())));
    assert!(
        ordered
            .iter()
            .all(|entry| entry.object.object_ref() != ObjectRef::Claim(unrelated_claim.id()))
    );

    let other_path = project_path("anchor-context-other-world");
    let other_world = World::new("Elsewhere", "", "Second Dawn", 1).expect("world");
    let other_store = WorldStore::create(&other_path, &other_world).expect("other store");
    match other_store.load_anchor_context(&AnchorContextQuery {
        anchors: vec![ObjectRef::Entity(actor.id())],
        relation_limit: 1,
    }) {
        Err(StoreError::ObjectNotFound { object, id }) => {
            assert_eq!(object, "entity");
            assert_eq!(id, actor.id().to_string());
        }
        other => panic!("expected isolated world error, got {other:?}"),
    }

    drop(other_store);
    fs::remove_file(other_path).expect("remove other project");
    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn logical_uri_resolves_after_rename_and_reports_missing_objects() {
    let path = project_path("logical-uri");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let entity = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("entity");
    store.insert_entity(&entity).expect("insert entity");

    let renamed = Entity::restore(
        entity.id(),
        entity.world_id(),
        entity.kind(),
        "Mara Tide",
        "mara-tide",
        entity.summary(),
        entity.body_md(),
        entity.attributes_json().as_str(),
        entity.aliases().to_vec(),
        entity.version(),
        entity.created_at_ms(),
        2,
    )
    .expect("renamed entity");
    let updated = store.update_entity(&renamed).expect("rename entity");

    let uri = ObjectRef::Entity(entity.id()).to_string();
    let resolved = store.resolve_uri(&uri).expect("resolve uri");
    assert_eq!(resolved, ResolvedObject::Entity(updated));

    let missing_document = nirmata_core::DocumentId::new();
    let missing_uri = ObjectRef::Document(missing_document).to_string();
    match store.resolve_uri(&missing_uri) {
        Err(StoreError::ObjectNotFound { object, id }) => {
            assert_eq!(object, "document");
            assert_eq!(id, missing_document.to_string());
        }
        other => panic!("expected missing document error, got {other:?}"),
    }

    match store.resolve_uri("file://entity/not-a-nirmata-uri") {
        Err(StoreError::InvalidObjectUri(uri)) => {
            assert_eq!(uri, "file://entity/not-a-nirmata-uri");
        }
        other => panic!("expected invalid uri error, got {other:?}"),
    }

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn logical_vfs_groups_entries_and_isolates_worlds() {
    let path = project_path("logical-vfs");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");

    let entity = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("entity");
    store.insert_entity(&entity).expect("insert entity");
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

    let tree = store.read_logical_vfs().expect("read tree");
    let people = tree
        .child_directory("entities")
        .and_then(|dir| dir.child_directory("people"))
        .expect("people directory");
    let mara = people.child_object("Mara").expect("mara object");
    assert_eq!(mara.uri, ObjectRef::Entity(entity.id()).to_string());
    let chronicles = tree
        .child_directory("documents")
        .and_then(|dir| dir.child_directory("chronicle"))
        .expect("chronicle directory");
    assert!(chronicles.child_object("Harbor Ledger").is_some());

    let renamed = Entity::restore(
        entity.id(),
        entity.world_id(),
        entity.kind(),
        "Mara Tide",
        "mara-tide",
        entity.summary(),
        entity.body_md(),
        entity.attributes_json().as_str(),
        entity.aliases().to_vec(),
        entity.version(),
        entity.created_at_ms(),
        2,
    )
    .expect("renamed entity");
    store.update_entity(&renamed).expect("rename entity");

    let rebuilt_tree = store.read_logical_vfs().expect("rebuild tree");
    let rebuilt_people = rebuilt_tree
        .child_directory("entities")
        .and_then(|dir| dir.child_directory("people"))
        .expect("people directory");
    assert!(rebuilt_people.child_object("Mara").is_none());
    let mara_tide = rebuilt_people
        .child_object("Mara Tide")
        .expect("renamed object");
    assert_eq!(mara_tide.uri, ObjectRef::Entity(entity.id()).to_string());

    let other_path = project_path("logical-vfs-other-world");
    let other_world = World::new("Elsewhere", "", "Second Dawn", 1).expect("world");
    let mut other_store = WorldStore::create(&other_path, &other_world).expect("other store");
    let other_entity = Entity::new(
        other_world.id(),
        EntityKind::Person,
        "Iven",
        "iven",
        "Traveler",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("entity");
    other_store
        .insert_entity(&other_entity)
        .expect("insert other entity");
    let other_tree = other_store.read_logical_vfs().expect("other tree");
    let other_people = other_tree
        .child_directory("entities")
        .and_then(|dir| dir.child_directory("people"))
        .expect("other people");
    assert!(other_people.child_object("Iven").is_some());
    assert!(other_people.child_object("Mara Tide").is_none());

    match other_store.resolve_uri(&ObjectRef::Entity(entity.id()).to_string()) {
        Err(StoreError::ObjectNotFound { object, id }) => {
            assert_eq!(object, "entity");
            assert_eq!(id, entity.id().to_string());
        }
        other => panic!("expected other-world object not found, got {other:?}"),
    }

    drop(other_store);
    fs::remove_file(other_path).expect("remove other project");
    drop(store);
    fs::remove_file(path).expect("remove project");
}
