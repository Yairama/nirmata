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
