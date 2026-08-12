use nirmata_app::{
    AppError, CreateWorldInput, DraftOperationInput, MAX_CAUSAL_DEPTH, MAX_CAUSAL_RESULTS,
    ManualReviewAction, ManualReviewInput, ManualReviewSession, NirmataApp, ReadScope,
};
use nirmata_core::{
    EventId, RevisionId, VariantId,
    change_set::RetconKind,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimPolarity},
    document::{ContentReference, Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventAggregate, EventLink, EventLinkKind},
    goal::{Goal, GoalStatus, GoalVisibility},
    time::{Certainty, EventTime, TimePrecision},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct NarrativeFixture {
    app: NirmataApp,
    path: PathBuf,
    main_variant: VariantId,
    historical_variant: VariantId,
    base_revision: RevisionId,
    current_revision: RevisionId,
    past_event: EventId,
    present_event: EventId,
    effect_event: EventId,
    ongoing_event: EventId,
    goal_ref: ObjectRef,
    claim_ref: ObjectRef,
    document_ref: ObjectRef,
}

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

fn fixture() -> NarrativeFixture {
    let path = project_path("narrative");
    let mut app = NirmataApp::default();
    let created = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "The Broken Archive".to_owned(),
            premise_md: "Memory records can contradict each other.".to_owned(),
            epoch_label: "Archive Reckoning".to_owned(),
        })
        .expect("create world");
    let world_id = created.world_id;
    let main_variant = created.active_variant.id;

    let archivist = Entity::new(
        world_id,
        EntityKind::Person,
        "Mara",
        "mara",
        "Archivist",
        "",
        "{}",
        vec![],
        2,
    )
    .expect("archivist");
    let active_goal = Goal::new(
        world_id,
        archivist.id(),
        "Recover the missing volume.",
        8,
        GoalStatus::Active,
        None,
        GoalVisibility::Public,
        None,
    )
    .expect("active goal");
    let past = Event::new(
        world_id,
        "loss",
        "The first volume disappears.",
        "",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        3,
    )
    .expect("past event");
    let present = Event::new(
        world_id,
        "discovery",
        "Mara opens the sealed archive.",
        "",
        EventTime::instant(30, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![active_goal.id()],
        4,
    )
    .expect("present event");
    let effect = Event::new(
        world_id,
        "revelation",
        "The archive reveals a forged index.",
        "",
        EventTime::instant(30, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        5,
    )
    .expect("effect event");
    let ongoing = Event::new(
        world_id,
        "search",
        "The search for the missing volume continues.",
        "",
        EventTime::ongoing(40, TimePrecision::Day, Certainty::Certain),
        None,
        vec![],
        vec![active_goal.id()],
        6,
    )
    .expect("ongoing event");
    let chronicle = Document::new(
        world_id,
        "Archive Chronicle",
        "chronicle",
        Some(archivist.id()),
        Some(archivist.id()),
        DocumentCanonStatus::Canonical,
        "Mara opens the archive. Years earlier, the volume vanished.",
        7,
    )
    .expect("chronicle");
    let document_ref = ObjectRef::Document(chronicle.id());
    let chronicle = DocumentAggregate::new(
        chronicle,
        vec![
            ContentReference::new(document_ref, ObjectRef::Event(present.id()), 0),
            ContentReference::new(document_ref, ObjectRef::Event(past.id()), 1),
            ContentReference::new(document_ref, ObjectRef::Event(effect.id()), 2),
            ContentReference::new(document_ref, ObjectRef::Event(ongoing.id()), 3),
        ],
    );
    let disputed = Claim::new(
        world_id,
        archivist.id(),
        "The missing volume was deliberately destroyed.",
        None,
        None,
        ClaimPolarity::Positive,
        ClaimAuthentication::Disputed,
        Some(archivist.id()),
        Some(ClaimModality::Hypothesis),
        Some("testimony".to_owned()),
        Some("conflicting witnesses".to_owned()),
        Some(document_ref.to_string()),
        Some(chronicle.object().id()),
        None,
        Some(0.5),
        None,
        created.current_revision,
    )
    .expect("disputed claim");

    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Create narrative fixture".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![
                DraftOperationInput::CreateEntity {
                    retcon: RetconKind::Additive,
                    after: archivist,
                },
                DraftOperationInput::CreateGoal {
                    retcon: RetconKind::Additive,
                    after: active_goal.clone(),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(past.clone(), vec![]),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(present.clone(), vec![]),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(effect.clone(), vec![]),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(ongoing.clone(), vec![]),
                },
                DraftOperationInput::CreateDocument {
                    retcon: RetconKind::Additive,
                    after: chronicle,
                },
                DraftOperationInput::CreateClaim {
                    retcon: RetconKind::Additive,
                    after: disputed.clone(),
                },
            ],
        })
        .expect("start fixture review");
    record_fixture_judgments(&app, &mut review);
    app.confirm_manual_review(&review)
        .expect("commit narrative fixture");
    let base_revision = app
        .get_current_world()
        .expect("session")
        .expect("open world")
        .current_revision;
    let historical_variant = app
        .create_variant("before-causal-links", base_revision)
        .expect("historical variant")
        .id;

    let present_before = EventAggregate::new(present.clone(), vec![]);
    let effect_before = EventAggregate::new(effect.clone(), vec![]);
    let present_after = EventAggregate::new(
        next_version(&present),
        vec![
            EventLink::new(present.id(), effect.id(), EventLinkKind::Causes).expect("causal link"),
        ],
    );
    let effect_after = EventAggregate::new(
        next_version(&effect),
        vec![
            EventLink::new(effect.id(), present.id(), EventLinkKind::Reveals)
                .expect("cycle-closing link"),
        ],
    );
    let mut link_review = app
        .start_manual_review(ManualReviewInput {
            objective: "Record causal links".to_owned(),
            sources: vec![document_ref],
            assumptions: vec![],
            operations: vec![
                DraftOperationInput::UpdateEvent {
                    retcon: RetconKind::Additive,
                    before: present_before,
                    after: present_after,
                },
                DraftOperationInput::UpdateEvent {
                    retcon: RetconKind::Additive,
                    before: effect_before,
                    after: effect_after,
                },
            ],
        })
        .expect("start causal review");
    record_fixture_judgments(&app, &mut link_review);
    app.confirm_manual_review(&link_review)
        .expect("commit causal links");
    let current_revision = app
        .get_current_world()
        .expect("session")
        .expect("open world")
        .current_revision;

    NarrativeFixture {
        app,
        path,
        main_variant,
        historical_variant,
        base_revision,
        current_revision,
        past_event: past.id(),
        present_event: present.id(),
        effect_event: effect.id(),
        ongoing_event: ongoing.id(),
        goal_ref: ObjectRef::Goal(active_goal.id()),
        claim_ref: ObjectRef::Claim(disputed.id()),
        document_ref,
    }
}

fn record_fixture_judgments(app: &NirmataApp, review: &mut ManualReviewSession) {
    let operation_ids = review
        .operations()
        .iter()
        .map(|operation| operation.operation_id())
        .collect::<Vec<_>>();
    for operation_id in operation_ids {
        app.apply_manual_review_action(
            review,
            ManualReviewAction::RecordJudgment {
                operation_id,
                judgment: "Fixture objects and explicit references were reviewed together."
                    .to_owned(),
            },
        )
        .expect("record fixture judgment");
    }
    assert!(
        review.ready_to_confirm(),
        "validation={:?} effective={:?}",
        review.validation_report(),
        review.effective_report()
    );
}

fn next_version(event: &Event) -> Event {
    Event::restore(
        event.id(),
        event.world_id(),
        event.kind(),
        event.summary(),
        event.body_md(),
        *event.time(),
        event.location_entity_id(),
        event.participants().to_vec(),
        event.affected_goal_ids().to_vec(),
        event.version() + 1,
        event.created_at_ms(),
        event.updated_at_ms() + 1,
    )
    .expect("next event version")
}

#[test]
fn narrative_derivations_are_scoped_deterministic_bounded_and_read_only() {
    let mut fixture = fixture();
    let history_before = fixture
        .app
        .list_revision_history()
        .expect("history before derivation")
        .revisions
        .len();

    let timeline = fixture
        .app
        .derive_narrative_timeline(None)
        .expect("active timeline");
    assert_eq!(
        timeline.scope,
        ReadScope::historical(fixture.main_variant, fixture.current_revision)
    );
    assert_eq!(
        timeline,
        fixture
            .app
            .derive_narrative_timeline(None)
            .expect("deterministic timeline")
    );
    let past_position = timeline
        .story_time
        .iter()
        .position(|entry| entry.event.object_ref == ObjectRef::Event(fixture.past_event))
        .expect("past in story time");
    let present_position = timeline
        .story_time
        .iter()
        .position(|entry| entry.event.object_ref == ObjectRef::Event(fixture.present_event))
        .expect("present in story time");
    assert!(past_position < present_position);
    let discourse = timeline
        .discourse_order
        .iter()
        .find(|sequence| sequence.source.object_ref == fixture.document_ref)
        .expect("document discourse sequence");
    assert_eq!(discourse.events[0].ordinal, 0);
    assert_eq!(
        discourse.events[0].event.object_ref,
        ObjectRef::Event(fixture.present_event)
    );
    assert_eq!(discourse.events[1].ordinal, 1);
    assert_eq!(
        discourse.events[1].event.object_ref,
        ObjectRef::Event(fixture.past_event)
    );
    assert!(
        timeline
            .story_time
            .iter()
            .find(|entry| entry.event.object_ref == ObjectRef::Event(fixture.ongoing_event))
            .expect("ongoing event in story time")
            .evidence_uris
            .contains(&fixture.document_ref.to_string())
    );

    let historical_scope = ReadScope::historical(fixture.historical_variant, fixture.base_revision);
    let historical_timeline = fixture
        .app
        .derive_narrative_timeline(Some(historical_scope))
        .expect("historical variant timeline");
    assert_eq!(historical_timeline.scope, historical_scope);
    assert_eq!(historical_timeline.story_time, timeline.story_time);
    assert_eq!(
        historical_timeline.discourse_order,
        timeline.discourse_order
    );

    let threads = fixture
        .app
        .derive_causal_threads(
            None,
            Some(vec![fixture.present_event]),
            MAX_CAUSAL_DEPTH,
            10,
        )
        .expect("causal threads");
    assert_eq!(
        threads,
        fixture
            .app
            .derive_causal_threads(
                None,
                Some(vec![fixture.present_event]),
                MAX_CAUSAL_DEPTH,
                10,
            )
            .expect("deterministic causal threads")
    );
    assert_eq!(threads.threads.len(), 1);
    assert_eq!(threads.threads[0].links.len(), 1, "cycle edge is omitted");
    let link = &threads.threads[0].links[0];
    assert_eq!(link.depth, 1);
    assert_eq!(link.kind, EventLinkKind::Causes);
    assert_eq!(
        link.source.object_ref,
        ObjectRef::Event(fixture.present_event)
    );
    assert_eq!(
        link.target.object_ref,
        ObjectRef::Event(fixture.effect_event)
    );
    assert!(
        link.evidence_uris
            .contains(&fixture.document_ref.to_string())
    );
    assert!(
        link.evidence_uris
            .contains(&ObjectRef::Event(fixture.present_event).to_string())
    );
    assert_eq!(
        fixture
            .app
            .derive_causal_threads(None, Some(vec![fixture.present_event]), 1, 1)
            .expect("bounded causal thread")
            .threads[0]
            .links
            .len(),
        1
    );
    assert!(matches!(
        fixture.app.derive_causal_threads(
            None,
            Some(vec![fixture.present_event]),
            MAX_CAUSAL_DEPTH + 1,
            1,
        ),
        Err(AppError::InvalidNarrativeQuery(_))
    ));
    assert!(matches!(
        fixture.app.derive_causal_threads(
            None,
            Some(vec![fixture.present_event]),
            1,
            MAX_CAUSAL_RESULTS + 1,
        ),
        Err(AppError::InvalidNarrativeQuery(_))
    ));
    let historical_threads = fixture
        .app
        .derive_causal_threads(
            Some(historical_scope),
            Some(vec![fixture.present_event]),
            MAX_CAUSAL_DEPTH,
            10,
        )
        .expect("historical causal threads");
    assert_eq!(historical_threads.scope, historical_scope);
    assert!(historical_threads.threads[0].links.is_empty());

    let loose_ends = fixture
        .app
        .derive_loose_ends(None)
        .expect("derive loose ends");
    assert_eq!(
        loose_ends,
        fixture
            .app
            .derive_loose_ends(None)
            .expect("deterministic loose ends")
    );
    assert_eq!(loose_ends.findings.len(), 3);
    let codes = loose_ends
        .findings
        .iter()
        .map(|finding| finding.code)
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        vec![
            "active_goal_without_resolution",
            "disputed_claim",
            "ongoing_event",
        ]
    );
    assert!(loose_ends.findings.iter().any(|finding| {
        finding.object_refs.contains(&fixture.goal_ref)
            && finding
                .evidence_uris
                .contains(&fixture.goal_ref.to_string())
    }));
    assert!(loose_ends.findings.iter().any(|finding| {
        finding.object_refs.contains(&fixture.claim_ref)
            && finding
                .evidence_uris
                .contains(&fixture.document_ref.to_string())
    }));
    assert!(loose_ends.findings.iter().all(|finding| {
        !finding.object_refs.is_empty()
            && !finding.evidence_uris.is_empty()
            && finding
                .evidence_uris
                .iter()
                .all(|uri| uri.parse::<ObjectRef>().is_ok())
    }));

    let session_after = fixture
        .app
        .get_current_world()
        .expect("session after derivation")
        .expect("open world");
    assert_eq!(session_after.current_revision, fixture.current_revision);
    assert_eq!(
        fixture
            .app
            .list_revision_history()
            .expect("history after derivation")
            .revisions
            .len(),
        history_before
    );

    fixture.app.close_world().expect("close fixture");
    fs::remove_file(fixture.path).expect("remove fixture");
}
