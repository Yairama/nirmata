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
