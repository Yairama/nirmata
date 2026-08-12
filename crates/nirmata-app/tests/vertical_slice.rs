use nirmata_app::{
    AppError, ContextBudget, ContextBundleRequest, ContextIntent, CreateWorldInput,
    DraftOperationInput, ManualReviewAction, ManualReviewInput, NirmataApp,
};
use nirmata_core::{
    Period,
    change_set::{ChangeOperation, RetconKind},
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::ObjectRef,
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{EventAggregate, OperationDecision, WorldStore};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
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

#[test]
fn creates_closes_and_reopens_world_from_disk() {
    let path = project_path("vertical-slice");
    let unused_path = project_path("must-not-create");
    let mut app = NirmataApp::default();

    let created = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "The Memory Empire".to_owned(),
            premise_md: "A mineral can store memories.".to_owned(),
            epoch_label: "Before the Collapse".to_owned(),
        })
        .expect("create world");

    assert!(path.is_file());
    assert!(matches!(
        app.create_world(CreateWorldInput {
            path: unused_path.clone(),
            name: "Another world".to_owned(),
            premise_md: String::new(),
            epoch_label: String::new(),
        })
        .expect_err("a second world must not open"),
        AppError::WorldAlreadyOpen
    ));
    assert!(!unused_path.exists());

    app.close_world().expect("close world");
    drop(app);

    let child = Command::new(env::current_exe().expect("current test executable"))
        .args(["--exact", "child_process_reopens_world", "--nocapture"])
        .env("NIRMATA_TEST_PROJECT", &path)
        .env("NIRMATA_TEST_WORLD_ID", created.world_id.to_string())
        .env(
            "NIRMATA_TEST_REVISION_ID",
            created.current_revision.to_string(),
        )
        .output()
        .expect("run reopen process");
    assert!(
        child.status.success(),
        "child process failed:\n{}",
        String::from_utf8_lossy(&child.stderr)
    );

    let mut restarted_app = NirmataApp::default();
    let reopened = restarted_app
        .open_world(path.clone())
        .expect("reopen persisted world");
    assert_eq!(reopened.world_id, created.world_id);
    assert_eq!(reopened.current_revision, created.current_revision);
    assert_eq!(reopened.world.name(), "The Memory Empire");
    assert_eq!(reopened.world.premise_md(), "A mineral can store memories.");
    restarted_app.close_world().expect("close reopened world");
    drop(restarted_app);

    let mut verification_app = NirmataApp::default();
    verification_app
        .open_world(path.clone())
        .expect("project remains valid after close");
    verification_app.close_world().expect("final close");
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn child_process_reopens_world() {
    let Ok(path) = env::var("NIRMATA_TEST_PROJECT") else {
        return;
    };
    let expected_world_id = env::var("NIRMATA_TEST_WORLD_ID").expect("expected world id");
    let expected_revision_id = env::var("NIRMATA_TEST_REVISION_ID").expect("expected revision id");
    let mut app = NirmataApp::default();

    let reopened = app
        .open_world(PathBuf::from(path))
        .expect("child process reopens world");
    assert_eq!(reopened.world_id.to_string(), expected_world_id);
    assert_eq!(reopened.current_revision.to_string(), expected_revision_id);
    app.close_world().expect("child process closes world");
}

#[test]
fn reports_actionable_open_errors() {
    let missing = project_path("missing");
    let mut app = NirmataApp::default();
    let error = app
        .open_world(missing.clone())
        .expect_err("missing file must fail");

    assert!(matches!(error, AppError::FileNotFound(ref path) if path == &missing));
    assert!(error.to_string().contains("was not found"));
}

fn semantic_canon(store: &WorldStore) -> Value {
    // Undo advances editorial history, so compare canon while excluding row-version metadata.
    let world = store.load_world().expect("load world for canon snapshot");
    let mut value = json!({
        "world": {
            "id": world.id(),
            "name": world.name(),
            "premise": world.premise_md(),
            "epoch": world.epoch_label(),
            "createdAtMs": world.created_at_ms(),
        },
        "entities": store.list_entities().expect("snapshot entities"),
        "relations": store.list_relations().expect("snapshot relations"),
        "goals": store.list_goals().expect("snapshot goals"),
        "rules": store.list_rules().expect("snapshot rules"),
        "events": store.list_events().expect("snapshot events"),
        "claims": store.list_claims().expect("snapshot claims"),
        "documents": store.list_documents().expect("snapshot documents"),
    });
    remove_editorial_versions(&mut value);
    value
}

fn remove_editorial_versions(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("version");
            object.remove("updated_at_ms");
            for child in object.values_mut() {
                remove_editorial_versions(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                remove_editorial_versions(child);
            }
        }
        _ => {}
    }
}

#[test]
fn mine_collapse_proposal_is_reviewed_committed_atomically_and_undone_after_reopen() {
    let path = project_path("mine-collapse-vertical-slice");
    let mut creator = NirmataApp::default();
    let created = creator
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "The Mnemonic Empire".to_owned(),
            premise_md: "Memory ore preserves experiences without explaining their meaning."
                .to_owned(),
            epoch_label: "Imperial Reckoning".to_owned(),
        })
        .expect("create mine scenario world");
    creator.close_world().expect("close created world");
    drop(creator);

    let world = created.world;
    let empire = Entity::new(
        world.id(),
        EntityKind::Faction,
        "Mnemonic Empire",
        "mnemonic-empire",
        "The crown depends on taxed memory ore.",
        "",
        "{}",
        vec![],
        2,
    )
    .expect("empire");
    let city = Entity::new(
        world.id(),
        EntityKind::Place,
        "Veyra",
        "veyra",
        "A mining city whose population and rescue capacity are unspecified.",
        "",
        "{}",
        vec![],
        3,
    )
    .expect("mining city");
    let mine = Entity::new(
        world.id(),
        EntityKind::Place,
        "Deep Archive Mine",
        "deep-archive-mine",
        "Veyra's memory-ore mine.",
        "",
        "{}",
        vec![],
        4,
    )
    .expect("mine");
    let religion = Entity::new(
        world.id(),
        EntityKind::Culture,
        "Church of the Last Witness",
        "church-last-witness",
        "A religion that interprets released memories as testimony.",
        "",
        "{}",
        vec![],
        5,
    )
    .expect("religion");
    let mineral = Entity::new(
        world.id(),
        EntityKind::Resource,
        "Mnemonite",
        "mnemonite",
        "A mineral proven to store memories.",
        "",
        "{}",
        vec![],
        6,
    )
    .expect("memory mineral");

    let empire_goal = Goal::new(
        world.id(),
        empire.id(),
        "Keep the imperial supply of mnemonite stable.",
        10,
        GoalStatus::Active,
        None,
        GoalVisibility::Public,
        Some("imperial budget".to_owned()),
    )
    .expect("empire goal");
    let city_goal = Goal::new(
        world.id(),
        city.id(),
        "Preserve Veyra's control over its mines.",
        8,
        GoalStatus::Active,
        None,
        GoalVisibility::Public,
        Some("city charter".to_owned()),
    )
    .expect("city goal");
    let sacred_memory_rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Released memories must be witnessed before anyone claims to interpret them.",
        "mnemonite and the Church of the Last Witness",
        RuleSeverity::Advisory,
        Some("liturgy of witnesses".to_owned()),
        None,
        "{}",
        7,
    )
    .expect("semantic religious rule");

    let governed_by = Relation::new(
        world.id(),
        empire.id(),
        city.id(),
        "governs",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        Some("imperial charter".to_owned()),
        "{}",
    )
    .expect("empire-city relation");
    let contains_mine = Relation::new(
        world.id(),
        city.id(),
        mine.id(),
        "contains",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        Some("survey register".to_owned()),
        "{}",
    )
    .expect("city-mine relation");
    let extracts = Relation::new(
        world.id(),
        mine.id(),
        mineral.id(),
        "extracts",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        Some("mine ledger".to_owned()),
        "{}",
    )
    .expect("mine-mineral relation");
    let venerates = Relation::new(
        world.id(),
        religion.id(),
        mineral.id(),
        "venerates",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        Some("liturgy of witnesses".to_owned()),
        "{}",
    )
    .expect("religion-mineral relation");

    let discovery = Event::new(
        world.id(),
        "discovery",
        "Veyra confirms that mnemonite stores memories.",
        "The assay establishes storage, not what broken ore does to a memory.",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![],
        vec![],
        8,
    )
    .expect("discovery event");
    let collapse = Event::new(
        world.id(),
        "collapse",
        "The Deep Archive Mine collapses.",
        "The collapse is canonical; its cause, casualties and rescue capacity remain unknown.",
        EventTime::instant(20, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![],
        vec![],
        9,
    )
    .expect("collapse event");
    let memory_fact = Claim::new(
        world.id(),
        mineral.id(),
        "Mnemonite stores memories.",
        Some("mnemonite.stores_memories".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        Some(ObjectRef::Event(discovery.id()).to_string()),
        None,
        None,
        None,
        Some(Period::new(Some(10), None).expect("memory fact period")),
        world.current_revision(),
    )
    .expect("canonical memory claim");
    let doctrine = Claim::new(
        world.id(),
        mineral.id(),
        "The Church believes released memories become sacred witnesses.",
        Some("mnemonite.released_memory_meaning".to_owned()),
        Some(ClaimObject::Scalar("sacred witness".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(religion.id()),
        Some(ClaimModality::Belief),
        Some("doctrine".to_owned()),
        Some("liturgy".to_owned()),
        Some(ObjectRef::Rule(sacred_memory_rule.id()).to_string()),
        None,
        None,
        Some(0.8),
        None,
        world.current_revision(),
    )
    .expect("religious perspective");

    let mut store = WorldStore::open(&path).expect("open fixture store");
    for entity in [&empire, &city, &mine, &religion, &mineral] {
        store.insert_entity(entity).expect("insert fixture entity");
    }
    store
        .insert_rule(&sacred_memory_rule)
        .expect("insert fixture rule");
    for relation in [&governed_by, &contains_mine, &extracts, &venerates] {
        store
            .insert_relation(relation)
            .expect("insert fixture relation");
    }
    for goal in [&empire_goal, &city_goal] {
        store.insert_goal(goal).expect("insert fixture goal");
    }
    store
        .insert_event(&EventAggregate::new(discovery.clone(), vec![]))
        .expect("insert discovery");
    let collapse_before = EventAggregate::new(collapse.clone(), vec![]);
    store
        .insert_event(&collapse_before)
        .expect("insert collapse");
    store
        .insert_claim(&memory_fact)
        .expect("insert memory fact");
    store.insert_claim(&doctrine).expect("insert doctrine");
    assert_eq!(store.list_entities().expect("fixture entities").len(), 5);
    assert_eq!(store.list_relations().expect("fixture relations").len(), 4);
    assert_eq!(store.list_goals().expect("fixture goals").len(), 2);
    assert_eq!(store.list_rules().expect("fixture rules").len(), 1);
    assert_eq!(store.list_events().expect("fixture events").len(), 2);
    assert_eq!(store.list_claims().expect("fixture claims").len(), 2);
    assert_eq!(store.list_revisions().expect("fixture revisions").len(), 1);
    let canon_before = semantic_canon(&store);
    drop(store);

    let mut app = NirmataApp::default();
    app.open_world(path.clone()).expect("open fixture in app");
    let context_request = ContextBundleRequest {
        intent: ContextIntent::ImpactAnalysis,
        anchors: vec![
            ObjectRef::Event(collapse.id()),
            ObjectRef::Entity(empire.id()),
            ObjectRef::Entity(city.id()),
            ObjectRef::Entity(mine.id()),
            ObjectRef::Entity(religion.id()),
            ObjectRef::Entity(mineral.id()),
            ObjectRef::Goal(empire_goal.id()),
            ObjectRef::Goal(city_goal.id()),
            ObjectRef::Rule(sacred_memory_rule.id()),
            ObjectRef::Claim(memory_fact.id()),
            ObjectRef::Claim(doctrine.id()),
        ],
        query_text: Some("mine collapse mnemonite".to_owned()),
        temporal: None,
        temporal_radius: Some(15),
        perspective_entity_ids: vec![religion.id()],
        include_perspectives: true,
        relation_limit: 8,
        budget: ContextBudget {
            max_objects: 32,
            max_chars: 8_000,
        },
    };
    let prepared = app
        .prepare_ai_proposal(
            "Create mine-collapse consequences without filling unknown facts.",
            &context_request,
        )
        .expect("prepare proposal from persisted context");
    assert_eq!(prepared.snapshot.base_revision, world.current_revision());
    for source in [
        ObjectRef::Event(collapse.id()),
        ObjectRef::Entity(empire.id()),
        ObjectRef::Entity(city.id()),
        ObjectRef::Entity(mine.id()),
        ObjectRef::Entity(religion.id()),
        ObjectRef::Entity(mineral.id()),
        ObjectRef::Goal(empire_goal.id()),
        ObjectRef::Goal(city_goal.id()),
        ObjectRef::Rule(sacred_memory_rule.id()),
        ObjectRef::Claim(memory_fact.id()),
        ObjectRef::Claim(doctrine.id()),
    ] {
        assert!(
            prepared.snapshot.context.contains(source),
            "persisted proposal context must contain {source}"
        );
    }
    assert!(
        prepared
            .snapshot
            .context
            .canon
            .iter()
            .any(|entry| { entry.object_ref() == ObjectRef::Claim(memory_fact.id()) })
    );
    assert!(
        prepared
            .snapshot
            .context
            .perspectives
            .iter()
            .any(|entry| { entry.object_ref() == ObjectRef::Claim(doctrine.id()) })
    );

    let economic = Event::new(
        world.id(),
        "economic_consequence",
        "Imperial mnemonite deliveries stop.",
        format!(
            "Evidence: {} and {}. The collapse interrupts the recorded supply chain.",
            ObjectRef::Event(collapse.id()),
            ObjectRef::Goal(empire_goal.id())
        ),
        EventTime::instant(21, TimePrecision::Exact, Certainty::Certain),
        Some(city.id()),
        vec![],
        vec![empire_goal.id()],
        10,
    )
    .expect("economic consequence");
    let political = Event::new(
        world.id(),
        "political_consequence",
        "Veyra's council withholds the next imperial levy.",
        format!(
            "Evidence: {} and {}. The council acts to preserve local control.",
            ObjectRef::Event(collapse.id()),
            ObjectRef::Goal(city_goal.id())
        ),
        EventTime::instant(22, TimePrecision::Exact, Certainty::Certain),
        Some(city.id()),
        vec![],
        vec![city_goal.id()],
        11,
    )
    .expect("political consequence");
    let edited_political = Event::restore(
        political.id(),
        political.world_id(),
        political.kind(),
        "Veyra's council delays, rather than refuses, the next imperial levy.",
        format!(
            "Evidence: {} and {}. The delay protects local control without asserting independence.",
            ObjectRef::Event(collapse.id()),
            ObjectRef::Goal(city_goal.id())
        ),
        *political.time(),
        political.location_entity_id(),
        political.participants().to_vec(),
        political.affected_goal_ids().to_vec(),
        political.version(),
        political.created_at_ms(),
        12,
    )
    .expect("edited political consequence");
    let religious = Event::new(
        world.id(),
        "religious_consequence",
        "Witness vigils begin outside the sealed mine.",
        format!(
            "Evidence: {} and {}. Motivation unknown: no canonical goal explains who began the vigils.",
            ObjectRef::Event(collapse.id()),
            ObjectRef::Rule(sacred_memory_rule.id())
        ),
        EventTime::instant(23, TimePrecision::Exact, Certainty::Certain),
        Some(mine.id()),
        vec![],
        vec![],
        13,
    )
    .expect("religious consequence");

    let collapse_after = Event::restore(
        collapse.id(),
        collapse.world_id(),
        collapse.kind(),
        collapse.summary(),
        collapse.body_md(),
        *collapse.time(),
        collapse.location_entity_id(),
        collapse.participants().to_vec(),
        collapse.affected_goal_ids().to_vec(),
        collapse.version() + 1,
        collapse.created_at_ms(),
        14,
    )
    .expect("collapse with causal links");
    let collapse_after = EventAggregate::new(
        collapse_after,
        [&economic, &political, &religious]
            .into_iter()
            .map(|consequence| {
                EventLink::new(collapse.id(), consequence.id(), EventLinkKind::Causes)
                    .expect("collapse consequence link")
            })
            .collect(),
    );

    let collapse_fact = Claim::new(
        world.id(),
        mine.id(),
        "The Deep Archive Mine collapsed at tick 20.",
        Some("mine.collapsed".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        Some(ObjectRef::Event(collapse.id()).to_string()),
        None,
        None,
        None,
        Some(Period::new(Some(20), Some(20)).expect("collapse fact period")),
        world.current_revision(),
    )
    .expect("canonical collapse fact");
    let political_rumor = Claim::new(
        world.id(),
        mine.id(),
        "Veyra's council suspects imperial neglect caused the collapse.",
        Some("collapse.cause".to_owned()),
        Some(ClaimObject::Scalar("imperial neglect".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(city.id()),
        Some(ClaimModality::Hypothesis),
        Some("council rumor".to_owned()),
        Some("political interpretation".to_owned()),
        Some(ObjectRef::Event(collapse.id()).to_string()),
        None,
        None,
        Some(0.4),
        Some(Period::new(Some(20), None).expect("political rumor period")),
        world.current_revision(),
    )
    .expect("political rumor");
    let religious_belief = Claim::new(
        world.id(),
        mine.id(),
        "The Church believes the collapse released un-witnessed memories.",
        Some("collapse.religious_meaning".to_owned()),
        Some(ClaimObject::Scalar("released witnesses".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Attributed,
        Some(religion.id()),
        Some(ClaimModality::Belief),
        Some("doctrine".to_owned()),
        Some("religious interpretation".to_owned()),
        Some(ObjectRef::Rule(sacred_memory_rule.id()).to_string()),
        None,
        None,
        Some(0.7),
        Some(Period::new(Some(20), None).expect("religious belief period")),
        world.current_revision(),
    )
    .expect("religious belief");
    let rejected_invention = Claim::new(
        world.id(),
        mine.id(),
        "Imperial explosives caused the collapse.",
        Some("collapse.cause".to_owned()),
        Some(ClaimObject::Scalar("imperial explosives".to_owned())),
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
        Some(Period::new(Some(20), Some(20)).expect("invented cause period")),
        world.current_revision(),
    )
    .expect("unsupported canonical cause");

    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Record sourced economic, political and religious consequences of the mine collapse."
                .to_owned(),
            sources: vec![
                ObjectRef::Event(collapse.id()),
                ObjectRef::Relation(governed_by.id()),
                ObjectRef::Relation(extracts.id()),
                ObjectRef::Relation(venerates.id()),
                ObjectRef::Goal(empire_goal.id()),
                ObjectRef::Goal(city_goal.id()),
                ObjectRef::Rule(sacred_memory_rule.id()),
                ObjectRef::Claim(memory_fact.id()),
                ObjectRef::Claim(doctrine.id()),
            ],
            assumptions: vec![
                "The collapse cause and casualty count remain unspecified.".to_owned(),
                "No motive is inferred where the canon has no actor goal.".to_owned(),
            ],
            operations: vec![
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(economic.clone(), vec![]),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(political.clone(), vec![]),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(religious.clone(), vec![]),
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: collapse_fact.clone(),
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: political_rumor.clone(),
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: religious_belief.clone(),
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: rejected_invention.clone(),
                },
                DraftOperationInput::UpdateEvent {
                    retcon: RetconKind::Additive,
                    before: collapse_before.clone(),
                    after: collapse_after.clone(),
                },
            ],
        })
        .expect("start collapse review");
    assert_eq!(review.original_draft().operations().len(), 8);
    assert!(
        review
            .original_draft()
            .sources()
            .contains(&ObjectRef::Event(collapse.id()))
    );
    assert!(
        review
            .original_draft()
            .sources()
            .contains(&ObjectRef::Goal(empire_goal.id()))
    );
    assert!(
        review
            .original_draft()
            .sources()
            .contains(&ObjectRef::Goal(city_goal.id()))
    );
    assert!(
        review
            .original_draft()
            .sources()
            .contains(&ObjectRef::Rule(sacred_memory_rule.id()))
    );

    let economic_operation_id = review.operations()[0].operation_id();
    let political_operation_id = review.operations()[1].operation_id();
    let collapse_fact_operation_id = review.operations()[3].operation_id();
    let rejected_operation_id = review.operations()[6].operation_id();
    let collapse_operation_id = review.operations()[7].operation_id();
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Accept {
            operation_id: economic_operation_id,
        },
    )
    .expect("accept economic consequence");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Accept {
            operation_id: collapse_fact_operation_id,
        },
    )
    .expect("accept canonical collapse fact");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Edit {
            operation_id: political_operation_id,
            replacement: DraftOperationInput::CreateEvent {
                retcon: RetconKind::Additive,
                after: EventAggregate::new(edited_political.clone(), vec![]),
            },
        },
    )
    .expect("edit political consequence");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::Reject {
            operation_id: rejected_operation_id,
        },
    )
    .expect("reject unsupported cause");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::RecordJudgment {
            operation_id: collapse_operation_id,
            judgment: "The three causal links are justified by the reviewed consequence events."
                .to_owned(),
        },
    )
    .expect("record broad-impact judgment");

    assert_eq!(review.operations()[0].decision(), OperationDecision::Accept);
    assert_eq!(review.operations()[1].decision(), OperationDecision::Edit);
    assert_eq!(review.operations()[6].decision(), OperationDecision::Reject);
    assert_eq!(review.draft().operations().len(), 7);
    assert!(
        review.ready_to_confirm(),
        "{:#?}",
        review.effective_report()
    );
    assert!(review.draft().operations().iter().all(|operation| {
        !matches!(
            operation,
            ChangeOperation::CreateClaim { after, .. }
                if after.id() == rejected_invention.id()
        )
    }));
    assert!(review.draft().operations().iter().all(|operation| {
        !matches!(
            operation,
            ChangeOperation::CreateClaim { after, .. }
                if after.predicate_key() == Some("collapse.casualty_count")
        )
    }));

    let reviewed_collapse = review
        .draft()
        .operations()
        .iter()
        .find_map(|operation| match operation {
            ChangeOperation::UpdateEvent { after, .. } if after.event().id() == collapse.id() => {
                Some(after)
            }
            _ => None,
        })
        .expect("reviewed collapse update");
    for consequence in [&economic, &edited_political, &religious] {
        assert!(reviewed_collapse.links().iter().any(|link| {
            link.kind() == EventLinkKind::Causes
                && link.source_event_id() == collapse.id()
                && link.target_event_id() == consequence.id()
        }));
        if consequence.affected_goal_ids().is_empty() {
            assert!(consequence.body_md().contains("Motivation unknown"));
        } else {
            assert!(consequence.affected_goal_ids().iter().all(|goal_id| {
                review
                    .draft()
                    .sources()
                    .contains(&ObjectRef::Goal(*goal_id))
            }));
        }
        assert!(
            consequence
                .body_md()
                .contains(&ObjectRef::Event(collapse.id()).to_string())
        );
    }
    assert!(review.draft().operations().iter().any(|operation| matches!(
        operation,
        ChangeOperation::CreateEvent { after, .. }
            if after.event().kind() == "economic_consequence"
    )));
    assert!(review.draft().operations().iter().any(|operation| matches!(
        operation,
        ChangeOperation::CreateEvent { after, .. }
            if after.event().kind() == "political_consequence"
                && after.event().summary().contains("delays")
    )));
    assert!(review.draft().operations().iter().any(|operation| matches!(
        operation,
        ChangeOperation::CreateEvent { after, .. }
            if after.event().kind() == "religious_consequence"
    )));

    let committed = app
        .confirm_manual_review(&review)
        .expect("commit reviewed collapse proposal");
    assert_ne!(committed.current_revision, world.current_revision());
    let store = WorldStore::open(&path).expect("reopen committed store");
    let revisions = store.list_revisions().expect("committed revisions");
    assert_eq!(
        revisions.len(),
        2,
        "the proposal creates exactly one revision"
    );
    let revision = store
        .get_revision(committed.current_revision)
        .expect("load committed revision")
        .expect("committed revision exists");
    assert_eq!(
        revision.parent_revision_id(),
        Some(world.current_revision())
    );
    let record = store
        .get_committed_change_set(revision.change_set_id().expect("change set id"))
        .expect("load committed change set")
        .expect("committed change set exists");
    let committed_operation_ids = record
        .change_set()
        .operations()
        .iter()
        .map(ChangeOperation::operation_id)
        .collect::<Vec<_>>();
    let selected_operation_ids = review
        .draft()
        .operations()
        .iter()
        .map(ChangeOperation::operation_id)
        .collect::<Vec<_>>();
    assert_eq!(committed_operation_ids, selected_operation_ids);
    assert_eq!(record.audits().len(), 7);
    assert!(record.audits().iter().all(|audit| {
        audit.operation_id() != rejected_operation_id
            && audit.decision() != OperationDecision::Reject
    }));
    assert_eq!(
        record
            .audits()
            .iter()
            .find(|audit| audit.operation_id() == political_operation_id)
            .expect("edited operation audit")
            .decision(),
        OperationDecision::Edit
    );
    assert_eq!(
        store
            .get_event(edited_political.id())
            .expect("load political consequence")
            .expect("political consequence persisted")
            .event()
            .summary(),
        edited_political.summary()
    );
    for consequence in [&economic, &edited_political, &religious] {
        assert_eq!(
            store
                .get_event(consequence.id())
                .expect("load selected consequence")
                .expect("selected consequence persisted")
                .event()
                .body_md(),
            consequence.body_md()
        );
    }
    assert_eq!(
        store
            .get_event(collapse.id())
            .expect("load collapse after commit")
            .expect("collapse persisted")
            .links(),
        reviewed_collapse.links()
    );
    assert_eq!(
        store
            .get_claim(rejected_invention.id())
            .expect("look up rejected cause"),
        None
    );
    assert_eq!(
        store
            .get_entity(city.id())
            .expect("load city")
            .expect("city persisted")
            .attributes_json()
            .as_str(),
        "{}"
    );
    let committed_claims = store.list_claims().expect("committed claims");
    let canonical_collapse = committed_claims
        .iter()
        .find(|claim| claim.id() == collapse_fact.id())
        .expect("canonical collapse fact persisted");
    assert_eq!(
        canonical_collapse.authentication(),
        ClaimAuthentication::Canonical
    );
    assert_eq!(canonical_collapse.holder_entity_id(), None);
    assert_eq!(canonical_collapse.modality(), None);
    for interpretation in [&political_rumor, &religious_belief, &doctrine] {
        let stored = committed_claims
            .iter()
            .find(|claim| claim.id() == interpretation.id())
            .expect("attributed interpretation persisted");
        assert_eq!(stored.authentication(), ClaimAuthentication::Attributed);
        assert!(stored.holder_entity_id().is_some());
        assert!(stored.modality().is_some());
    }
    assert!(committed_claims.iter().all(|claim| {
        claim.predicate_key() != Some("collapse.casualty_count")
            && !(claim.authentication() == ClaimAuthentication::Canonical
                && claim.predicate_key() == Some("collapse.cause"))
    }));
    drop(store);

    app.close_world().expect("close committed world");
    drop(app);
    let mut reopened_app = NirmataApp::default();
    let reopened = reopened_app
        .open_world(path.clone())
        .expect("reopen committed world before undo");
    assert_eq!(reopened.current_revision, committed.current_revision);
    let undone = reopened_app
        .undo_last_commit()
        .expect("undo collapse proposal after reopen");
    assert_ne!(undone.current_revision, committed.current_revision);
    reopened_app.close_world().expect("close undone world");
    drop(reopened_app);

    let restored = WorldStore::open(&path).expect("reopen world after undo");
    assert_eq!(semantic_canon(&restored), canon_before);
    assert_eq!(
        restored
            .get_claim(rejected_invention.id())
            .expect("rejected claim remains absent after undo"),
        None
    );
    assert_eq!(
        restored
            .list_revisions()
            .expect("revisions after undo")
            .len(),
        3
    );
    drop(restored);

    fs::remove_file(path).expect("remove mine scenario project");
}
