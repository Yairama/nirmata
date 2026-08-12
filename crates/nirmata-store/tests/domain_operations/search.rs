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
fn semantic_search_is_deterministic_world_isolated_and_never_canonical() {
    let first_path = project_path("semantic-first-world");
    let second_path = project_path("semantic-second-world");
    let first_world = World::new("First", "", "Dawn", 1).expect("first world");
    let second_world = World::new("Second", "", "Dawn", 1).expect("second world");
    let mut first_store = WorldStore::create(&first_path, &first_world).expect("first store");
    let mut second_store = WorldStore::create(&second_path, &second_world).expect("second store");
    let first_document = Document::new(
        first_world.id(),
        "First Senate Record",
        "minutes",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "The senate refuses the border pact.",
        1,
    )
    .expect("first document");
    let second_document = Document::new(
        second_world.id(),
        "Second Senate Record",
        "minutes",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "The senate refuses the border pact.",
        1,
    )
    .expect("second document");
    first_store
        .insert_document(&DocumentAggregate::new(first_document.clone(), vec![]))
        .expect("insert first document");
    second_store
        .insert_document(&DocumentAggregate::new(second_document.clone(), vec![]))
        .expect("insert second document");

    let semantic_query = || {
        first_store
            .search_semantic(
                "council rejects treaty",
                &[StructuredSearchKind::Document],
                5,
            )
            .expect("semantic search")
    };
    let first_hits = semantic_query();
    for _ in 0..5 {
        assert_eq!(semantic_query(), first_hits);
    }
    assert_eq!(first_hits.len(), 1);
    assert_eq!(first_hits[0].object, ObjectRef::Document(first_document.id()));
    assert_eq!(first_hits[0].stage, StructuredSearchStage::Semantic);
    assert!(first_hits[0].provenance.starts_with("semantic:wordnet-en-offline:v1:"));
    assert_eq!(
        second_store
            .search_semantic(
                "council rejects treaty",
                &[StructuredSearchKind::Document],
                5,
            )
            .expect("second world semantic search")
            .into_iter()
            .map(|hit| hit.object)
            .collect::<Vec<_>>(),
        vec![ObjectRef::Document(second_document.id())]
    );

    let exact_query = StructuredSearchQuery {
        kinds: vec![StructuredSearchKind::Document],
        text: Some("senate refuses pact".to_owned()),
        limit: 5,
        ..Default::default()
    };
    let exact_before_rebuild = first_store
        .search_structured(&exact_query)
        .expect("exact search before deletion");
    assert_eq!(
        exact_before_rebuild[0].object,
        ObjectRef::Document(first_document.id())
    );

    let connection = rusqlite::Connection::open(&first_path).expect("inspect derived storage");
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE lower(name) LIKE '%semantic%'
                    OR lower(name) LIKE '%embedding%'
                    OR lower(name) LIKE '%vector%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("semantic table count"),
        0,
        "the prototype must not persist a semantic index"
    );
    connection
        .execute("DELETE FROM canon_fts", [])
        .expect("delete rebuildable index contents");
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get::<_, i64>(0))
            .expect("canonical document count"),
        1
    );
    drop(connection);

    assert_eq!(
        first_store
            .get_document(first_document.id())
            .expect("canonical document after index deletion")
            .expect("document remains canonical")
            .object(),
        &first_document
    );
    assert_eq!(semantic_query(), first_hits);
    assert!(
        first_store
            .search_structured_fts(&exact_query)
            .expect("empty exact index")
            .is_empty()
    );
    first_store
        .rebuild_canon_text_index()
        .expect("rebuild exact index from canon");
    assert_eq!(
        first_store
            .search_structured(&exact_query)
            .expect("exact search after rebuild"),
        exact_before_rebuild
    );

    drop(first_store);
    drop(second_store);
    fs::remove_file(first_path).expect("remove first project");
    fs::remove_file(second_path).expect("remove second project");
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
        "Stormglass entries line every page. The senate refuses the border pact.",
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
    let semantic_query = StructuredSearchQuery {
        kinds: vec![StructuredSearchKind::Document],
        text: Some("council rejects treaty".to_owned()),
        limit: 5,
        ..Default::default()
    };
    assert_eq!(
        store
            .search_structured(&semantic_query)
            .expect("semantic search before delete")[0]
            .object,
        ObjectRef::Document(document.id())
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
    assert!(
        store
            .search_structured(&semantic_query)
            .expect("semantic search after delete")
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
