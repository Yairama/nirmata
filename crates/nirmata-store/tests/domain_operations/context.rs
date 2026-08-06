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
