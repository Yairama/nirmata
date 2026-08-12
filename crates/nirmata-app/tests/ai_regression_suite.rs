use nirmata_ai::contracts::{
    CritiqueAttackType, CritiqueCategory, StructuredOutputErrorKind, parse_change_set_draft,
    parse_critique_report,
};
use nirmata_app::{
    AppError, ContextBundleRequest, ContextIntent, DraftOperationInput, ManualReviewAction,
    ManualReviewInput, NirmataApp,
};
use nirmata_core::{
    ChangeOperationId, DomainError, EntityId, Period, RevisionId, World, WorldId,
    change_set::{ChangeOperation, ChangeSetDraft, ChangeSetValidationSnapshot, RetconKind},
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, ObjectRef, ordered_content_references},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
    time::{Certainty, EventTime, EventTimeKind, PartialTruth, TimePrecision},
    validation::{
        ValidationIssue, ValidationSeverity, relation_active_at, validate_claims,
        validate_event_links, validate_lifecycle, validate_no_resurrection,
    },
};
use nirmata_store::WorldStore;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

const DETERMINISTIC_CASES: [u8; 17] = [
    1, 2, 3, 4, 10, 11, 12, 13, 20, 21, 23, 25, 26, 27, 29, 30, 31,
];
const SEMANTIC_CASES: [u8; 17] = [
    5, 6, 7, 8, 9, 14, 15, 16, 17, 18, 19, 22, 24, 28, 32, 33, 34,
];

fn person(world_id: WorldId, name: &str) -> Entity {
    Entity::new(
        world_id,
        EntityKind::Person,
        name,
        name.to_ascii_lowercase().replace(' ', "-"),
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("person fixture")
}

fn event(world_id: WorldId, kind: &str, tick: i64, participants: Vec<EventParticipant>) -> Event {
    Event::new(
        world_id,
        kind,
        kind,
        "",
        EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
        None,
        participants,
        vec![],
        1,
    )
    .expect("event fixture")
}

fn create_entity_operation(entity: Entity, retcon: RetconKind) -> ChangeOperation {
    ChangeOperation::CreateEntity {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: 0,
        retcon,
        after: entity,
    }
}

fn canonical_claim(
    world_id: WorldId,
    subject_id: EntityId,
    revision: RevisionId,
    polarity: ClaimPolarity,
) -> Claim {
    Claim::new(
        world_id,
        subject_id,
        "The gate state is explicitly recorded.",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        polarity,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        Some("registry".to_owned()),
        None,
        None,
        None,
        Some(Period::new(Some(12), Some(12)).expect("claim period")),
        revision,
    )
    .expect("canonical claim fixture")
}

fn attributed_claim(
    world_id: WorldId,
    subject_id: EntityId,
    holder_id: EntityId,
    revision: RevisionId,
    polarity: ClaimPolarity,
) -> Claim {
    Claim::new(
        world_id,
        subject_id,
        "A holder reports the gate state.",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        polarity,
        ClaimAuthentication::Attributed,
        Some(holder_id),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        Some("hearsay".to_owned()),
        Some("testimony".to_owned()),
        None,
        None,
        Some(0.6),
        Some(Period::new(Some(12), Some(12)).expect("claim period")),
        revision,
    )
    .expect("attributed claim fixture")
}

fn assert_issue<'a>(
    issues: &'a [ValidationIssue],
    code: &str,
    severity: ValidationSeverity,
) -> &'a ValidationIssue {
    let issue = issues
        .iter()
        .find(|issue| issue.code == code)
        .unwrap_or_else(|| panic!("missing expected issue {code}: {issues:#?}"));
    assert_eq!(issue.severity, severity, "severity for {code}");
    assert!(
        !issue.objects.is_empty(),
        "{code} must cite conflicting objects"
    );
    issue
}

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
fn deterministic_regressions_block_invalid_state_and_preserve_allowed_cases() {
    let mut covered = Vec::new();

    // 1. A goal with a missing holder is a hard structural error.
    {
        let world_id = WorldId::new();
        let missing_holder = EntityId::new();
        let goal = Goal::new(
            world_id,
            missing_holder,
            "Protect the gate",
            1,
            GoalStatus::Active,
            None,
            GoalVisibility::Public,
            None,
        )
        .expect("goal shape");
        let operation = ChangeOperation::CreateGoal {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![
                ObjectRef::Goal(goal.id()),
                ObjectRef::Entity(missing_holder),
            ],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: goal,
        };
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Create a goal for the missing actor",
            vec![],
            vec![],
            vec![operation],
            vec![],
        )
        .expect("draft");
        let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());
        assert_issue(
            &report.errors,
            "goal.holder_missing",
            ValidationSeverity::Error,
        );
        assert!(
            !report.is_ok(),
            "the missing-reference operation is blocked"
        );
        covered.push(1);
    }

    // 2. Participation after a known death remains a cited conflict.
    {
        let world_id = WorldId::new();
        let mara = person(world_id, "Mara");
        let death = event(world_id, "death", 10, vec![]);
        let return_event = event(
            world_id,
            "return",
            20,
            vec![EventParticipant::new(mara.id(), "actor", 0).expect("participant")],
        );
        let issues = validate_lifecycle(&mara, None, Some(&death), &[&return_event]);
        let issue = assert_issue(
            &issues,
            "lifecycle.participation_after_death",
            ValidationSeverity::Conflict,
        );
        assert!(
            issue
                .objects
                .iter()
                .any(|object| object.id == death.id().to_string())
        );
        assert!(
            issue
                .objects
                .iter()
                .any(|object| object.id == return_event.id().to_string())
        );
        covered.push(2);
    }

    // 3. A cause after its effect blocks the causal operation.
    {
        let world_id = WorldId::new();
        let effect = event(world_id, "effect", 10, vec![]);
        let cause = event(world_id, "cause", 20, vec![]);
        let link =
            EventLink::new(cause.id(), effect.id(), EventLinkKind::Causes).expect("causal link");
        let issues = validate_event_links(&[link], &[cause, effect]);
        assert_issue(
            &issues,
            "causality.cause_after_effect",
            ValidationSeverity::Conflict,
        );
        covered.push(3);
    }

    // 4. The implemented no_resurrection rule is a hard error and cites the rule.
    {
        let world_id = WorldId::new();
        let mara = person(world_id, "Mara");
        let death = event(world_id, "death", 10, vec![]);
        let return_event = event(world_id, "return", 20, vec![]);
        let rule = Rule::new(
            world_id,
            RuleKind::Constitutive,
            "The dead do not return.",
            "world",
            RuleSeverity::Hard,
            None,
            Some(RuleValidatorKind::NoResurrection),
            "{}",
            1,
        )
        .expect("coded rule");
        let issues = validate_no_resurrection(&rule, &mara, &death, &[&return_event]);
        let issue = assert_issue(&issues, "rule.no_resurrection", ValidationSeverity::Error);
        assert!(
            issue
                .objects
                .iter()
                .any(|object| object.id == rule.id().to_string())
        );
        covered.push(4);
    }

    // 10. Free text can never be interpreted as a mutation.
    {
        let error = parse_change_set_draft("Add the queen back to the throne.")
            .expect_err("free text mutation must fail");
        assert_eq!(error.kind(), StructuredOutputErrorKind::FreeTextMutation);
        covered.push(10);
    }

    // 11. A draft from another revision cannot even reach semantic critique.
    {
        let path = project_path("ai-regression-stale");
        let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
        WorldStore::create(&path, &world).expect("create store");
        let mut app = NirmataApp::default();
        app.open_world(path.clone()).expect("open world");
        let proposed = person(world.id(), "Stale Mara");
        let draft = ChangeSetDraft::new(
            world.id(),
            RevisionId::new(),
            "Create from an obsolete snapshot",
            vec![],
            vec![],
            vec![create_entity_operation(proposed, RetconKind::Additive)],
            vec![],
        )
        .expect("stale draft");
        let error = app
            .prepare_ai_critique(
                "Apply the stale proposal",
                &draft,
                &ContextBundleRequest::new(ContextIntent::ImpactAnalysis),
            )
            .expect_err("stale draft must be rejected before critique");
        assert!(matches!(error, AppError::AiBaseRevisionMismatch { .. }));
        app.close_world().expect("close world");
        fs::remove_file(path).expect("remove project");
        covered.push(11);
    }

    // 12. An attributed rumor may oppose canon without blocking it.
    {
        let world_id = WorldId::new();
        let revision = RevisionId::new();
        let gate = person(world_id, "Gate");
        let witness = person(world_id, "Witness");
        let canon = canonical_claim(world_id, gate.id(), revision, ClaimPolarity::Positive);
        let rumor = attributed_claim(
            world_id,
            gate.id(),
            witness.id(),
            revision,
            ClaimPolarity::Negative,
        );
        let issues = validate_claims(
            &[canon, rumor],
            &[gate, witness],
            &[],
            &HashSet::from([revision]),
        );
        assert!(
            issues
                .iter()
                .all(|issue| issue.code != "claim.canonical_opposition")
        );
        covered.push(12);
    }

    // 13. An unspecified relation is unknown, never false.
    {
        let world_id = WorldId::new();
        let left = person(world_id, "Left");
        let right = person(world_id, "Right");
        let relation = Relation::new(
            world_id,
            left.id(),
            right.id(),
            "allied_with",
            RelationDirection::Directed,
            None,
            None,
            Certainty::Uncertain,
            None,
            "{}",
        )
        .expect("open relation");
        assert_eq!(relation_active_at(&relation, 50), PartialTruth::Unspecified);
        covered.push(13);
    }

    // 20. Opposite beliefs from different holders remain separate perspectives.
    {
        let world_id = WorldId::new();
        let revision = RevisionId::new();
        let gate = person(world_id, "Gate");
        let first = person(world_id, "First Witness");
        let second = person(world_id, "Second Witness");
        let claims = [
            attributed_claim(
                world_id,
                gate.id(),
                first.id(),
                revision,
                ClaimPolarity::Positive,
            ),
            attributed_claim(
                world_id,
                gate.id(),
                second.id(),
                revision,
                ClaimPolarity::Negative,
            ),
        ];
        let issues = validate_claims(
            &claims,
            &[gate, first, second],
            &[],
            &HashSet::from([revision]),
        );
        assert!(
            issues
                .iter()
                .all(|issue| issue.code != "claim.canonical_opposition")
        );
        covered.push(20);
    }

    // 21. Opposite active canonical claims in the same period conflict and cite both claims.
    {
        let world_id = WorldId::new();
        let revision = RevisionId::new();
        let gate = person(world_id, "Gate");
        let positive = canonical_claim(world_id, gate.id(), revision, ClaimPolarity::Positive);
        let negative = canonical_claim(world_id, gate.id(), revision, ClaimPolarity::Negative);
        let ids = [positive.id().to_string(), negative.id().to_string()];
        let issues = validate_claims(
            &[positive, negative],
            &[gate],
            &[],
            &HashSet::from([revision]),
        );
        let issue = assert_issue(
            &issues,
            "claim.canonical_opposition",
            ValidationSeverity::Conflict,
        );
        assert!(
            ids.iter()
                .all(|id| issue.objects.iter().any(|object| &object.id == id))
        );
        covered.push(21);
    }

    // 23. Additive creation is valid without replacement metadata.
    {
        let world_id = WorldId::new();
        let draft = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Add a new witness",
            vec![],
            vec![],
            vec![create_entity_operation(
                person(world_id, "New Witness"),
                RetconKind::Additive,
            )],
            vec![],
        )
        .expect("additive draft");
        let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());
        assert!(report.is_ok());
        assert!(
            report
                .errors
                .iter()
                .all(|issue| !issue.code.contains("replacement"))
        );
        covered.push(23);
    }

    // 25. Replacement without a DecisionPoint is a hard blocking error.
    {
        let world_id = WorldId::new();
        let existing = person(world_id, "Old Witness");
        let operation =
            create_entity_operation(person(world_id, "New Witness"), RetconKind::Replacement);
        let result = ChangeSetDraft::new(
            world_id,
            RevisionId::new(),
            "Replace the witness",
            vec![ObjectRef::Entity(existing.id())],
            vec![],
            vec![operation],
            vec![],
        );
        assert_eq!(
            result,
            Err(DomainError::InvalidChangeSetContext(
                "replacement operations require a decision point"
            ))
        );
        covered.push(25);
    }

    // 26. An interval ending before it starts is rejected at construction.
    {
        assert_eq!(
            EventTime::interval(20, 10, TimePrecision::Exact, Certainty::Certain),
            Err(DomainError::InvalidEventTime)
        );
        covered.push(26);
    }

    // 27. Ongoing time has no invented end and is not treated as completed.
    {
        let ongoing = EventTime::ongoing(10, TimePrecision::Day, Certainty::Certain);
        assert_eq!(ongoing.kind(), EventTimeKind::Ongoing);
        assert_eq!(ongoing.end_tick(), None);
        assert_eq!(
            ongoing.before(&EventTime::instant(
                20,
                TimePrecision::Exact,
                Certainty::Certain
            )),
            PartialTruth::Unspecified
        );
        covered.push(27);
    }

    // 29. Flashback discourse order changes only the content ordinal.
    {
        let world_id = WorldId::new();
        let flashback_event = event(world_id, "flashback", 5, vec![]);
        let source = ObjectRef::Event(event(world_id, "narration", 20, vec![]).id());
        let first = ContentReference::new(source, ObjectRef::Event(flashback_event.id()), 2);
        let flashback = ContentReference::new(source, ObjectRef::Event(flashback_event.id()), 0);
        let references = [first, flashback];
        let ordered = ordered_content_references(source, &references);
        assert_eq!(ordered[0].ordinal(), 0);
        assert_eq!(flashback_event.time().start_tick(), Some(5));
        covered.push(29);
    }

    // 30. Derived temporal relations follow endpoints and cannot assert their inverse.
    {
        let early = EventTime::interval(1, 4, TimePrecision::Exact, Certainty::Certain)
            .expect("early interval");
        let late = EventTime::interval(8, 10, TimePrecision::Exact, Certainty::Certain)
            .expect("late interval");
        assert_eq!(early.before(&late), PartialTruth::True);
        assert_eq!(early.after(&late), PartialTruth::False);
        assert_eq!(early.overlaps(&late), PartialTruth::False);
        covered.push(30);
    }

    // 31. Provenance must resolve to an existing document or claim.
    {
        let world_id = WorldId::new();
        let revision = RevisionId::new();
        let subject = person(world_id, "Subject");
        let claim = Claim::new(
            world_id,
            subject.id(),
            "A claim with broken provenance.",
            None,
            None,
            ClaimPolarity::Positive,
            ClaimAuthentication::Canonical,
            None,
            None,
            None,
            None,
            None,
            Some(nirmata_core::DocumentId::new()),
            None,
            None,
            None,
            revision,
        )
        .expect("claim shape");
        let issues = validate_claims(&[claim], &[subject], &[], &HashSet::from([revision]));
        assert_issue(
            &issues,
            "claim.source_document_missing",
            ValidationSeverity::Error,
        );
        covered.push(31);
    }

    assert_eq!(covered, DETERMINISTIC_CASES);
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedSemanticCase {
    case: u8,
    name: String,
    snapshot: Vec<String>,
    request: String,
    draft: RecordedDraft,
    response: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedDraft {
    operation_ids: Vec<String>,
    retcon: String,
    preserves: Vec<String>,
    exception: Option<String>,
}

struct ExpectedIssue {
    id: &'static str,
    severity: ValidationSeverity,
    category: CritiqueCategory,
    attack: CritiqueAttackType,
    operation: &'static str,
    source: &'static str,
}

struct ExpectedSemanticCase {
    case: u8,
    name: &'static str,
    issues: &'static [ExpectedIssue],
    blocked_operations: &'static [&'static str],
    allowed_exception: Option<&'static str>,
}

fn expected_semantic_case(case: u8) -> ExpectedSemanticCase {
    use CritiqueAttackType::{Rebuts, Undercuts};
    use CritiqueCategory::{
        CanonContradiction, ImpossibleKnowledge, InsufficientEvidence, MissingConsequence,
        TemporalConflict, UniverseRule,
    };
    use ValidationSeverity::{Conflict, Warning};

    match case {
        5 => ExpectedSemanticCase {
            case,
            name: "semantic_rule_ignored",
            issues: &[ExpectedIssue {
                id: "semantic-rule",
                severity: Conflict,
                category: UniverseRule,
                attack: Rebuts,
                operation: "05050505-0000-0000-0000-000000000005",
                source: "nirmata://rule/05050505-0505-0505-0505-050505050505",
            }],
            blocked_operations: &["05050505-0000-0000-0000-000000000005"],
            allowed_exception: None,
        },
        6 => ExpectedSemanticCase {
            case,
            name: "belief_is_not_physical_law",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some(
                "The attributed belief remains a perspective, not a universe rule.",
            ),
        },
        7 => ExpectedSemanticCase {
            case,
            name: "impossible_secret_knowledge",
            issues: &[ExpectedIssue {
                id: "secret-access",
                severity: Warning,
                category: ImpossibleKnowledge,
                attack: Undercuts,
                operation: "07070707-0000-0000-0000-000000000007",
                source: "nirmata://claim/07070707-0707-0707-0707-070707070707",
            }],
            blocked_operations: &[],
            allowed_exception: None,
        },
        8 => ExpectedSemanticCase {
            case,
            name: "hidden_markdown_contradiction",
            issues: &[ExpectedIssue {
                id: "hidden-death-conflict",
                severity: Conflict,
                category: TemporalConflict,
                attack: Rebuts,
                operation: "08080808-0000-0000-0000-000000000008",
                source: "nirmata://event/08080808-1111-1111-1111-111111111111",
            }],
            blocked_operations: &["08080808-0000-0000-0000-000000000008"],
            allowed_exception: None,
        },
        9 => ExpectedSemanticCase {
            case,
            name: "valid_rule_exception",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some("The rule explicitly permits the named heir."),
        },
        14 => ExpectedSemanticCase {
            case,
            name: "spatial_transition_missing",
            issues: &[ExpectedIssue {
                id: "spatial-gap",
                severity: Conflict,
                category: CanonContradiction,
                attack: Rebuts,
                operation: "14141414-0000-0000-0000-000000000014",
                source: "nirmata://entity/14141414-1414-1414-1414-141414141414",
            }],
            blocked_operations: &["14141414-0000-0000-0000-000000000014"],
            allowed_exception: None,
        },
        15 => ExpectedSemanticCase {
            case,
            name: "action_conflicts_with_goal",
            issues: &[ExpectedIssue {
                id: "goal-incompatibility",
                severity: Conflict,
                category: MissingConsequence,
                attack: Rebuts,
                operation: "15151515-0000-0000-0000-000000000015",
                source: "nirmata://goal/15151515-1515-1515-1515-151515151515",
            }],
            blocked_operations: &["15151515-0000-0000-0000-000000000015"],
            allowed_exception: None,
        },
        16 => ExpectedSemanticCase {
            case,
            name: "isolated_event_warning",
            issues: &[ExpectedIssue {
                id: "isolated-event",
                severity: Warning,
                category: MissingConsequence,
                attack: Undercuts,
                operation: "16161616-0000-0000-0000-000000000016",
                source: "nirmata://event/16161616-1616-1616-1616-161616161616",
            }],
            blocked_operations: &[],
            allowed_exception: Some(
                "An isolated event may be intentional and only requires acknowledgement.",
            ),
        },
        17 => ExpectedSemanticCase {
            case,
            name: "desire_is_not_knowledge",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some(
                "A desired state does not assert that the actor knows the current state.",
            ),
        },
        18 => ExpectedSemanticCase {
            case,
            name: "explained_discontinuity",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some(
                "The draft explicitly cites the gate transition that explains the discontinuity.",
            ),
        },
        19 => ExpectedSemanticCase {
            case,
            name: "explicit_negation_is_not_unknown",
            issues: &[ExpectedIssue {
                id: "explicit-negation",
                severity: Conflict,
                category: CanonContradiction,
                attack: Rebuts,
                operation: "19191919-0000-0000-0000-000000000019",
                source: "nirmata://claim/19191919-1919-1919-1919-191919191919",
            }],
            blocked_operations: &["19191919-0000-0000-0000-000000000019"],
            allowed_exception: None,
        },
        22 => ExpectedSemanticCase {
            case,
            name: "rebuts_and_undercuts_are_distinct",
            issues: &[
                ExpectedIssue {
                    id: "opposite-conclusion",
                    severity: Conflict,
                    category: CanonContradiction,
                    attack: Rebuts,
                    operation: "22222222-0000-0000-0000-000000000021",
                    source: "nirmata://claim/22222222-2222-2222-2222-222222222222",
                },
                ExpectedIssue {
                    id: "witness-access",
                    severity: Warning,
                    category: InsufficientEvidence,
                    attack: Undercuts,
                    operation: "22222222-0000-0000-0000-000000000022",
                    source: "nirmata://claim/22222222-2222-2222-2222-222222222222",
                },
            ],
            blocked_operations: &["22222222-0000-0000-0000-000000000021"],
            allowed_exception: None,
        },
        24 => ExpectedSemanticCase {
            case,
            name: "reinterpretive_retcon_preserves_perspective",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some(
                "The prior attributed perspective remains in the draft's affected history.",
            ),
        },
        28 => ExpectedSemanticCase {
            case,
            name: "approximate_date_is_not_exact",
            issues: &[ExpectedIssue {
                id: "approximate-date",
                severity: Warning,
                category: TemporalConflict,
                attack: Undercuts,
                operation: "28282828-0000-0000-0000-000000000028",
                source: "nirmata://event/28282828-2828-2828-2828-282828282828",
            }],
            blocked_operations: &[],
            allowed_exception: Some(
                "Approximate evidence may support a warning, not an exact temporal conflict.",
            ),
        },
        32 => ExpectedSemanticCase {
            case,
            name: "defeasible_specific_exception",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some(
                "The healer-during-epidemic rule is more specific than the general closure rule.",
            ),
        },
        33 => ExpectedSemanticCase {
            case,
            name: "open_world_absence_is_not_false",
            issues: &[],
            blocked_operations: &[],
            allowed_exception: Some(
                "Cloak color is not declared closed or exhaustive, so absence stays unknown.",
            ),
        },
        34 => ExpectedSemanticCase {
            case,
            name: "intentional_exception_remains_traceable",
            issues: &[ExpectedIssue {
                id: "intentional-oath-break",
                severity: Conflict,
                category: UniverseRule,
                attack: Rebuts,
                operation: "34343434-0000-0000-0000-000000000034",
                source: "nirmata://rule/34343434-3434-3434-3434-343434343434",
            }],
            blocked_operations: &["34343434-0000-0000-0000-000000000034"],
            allowed_exception: Some(
                "The operation stays blocked until a human records an intentional-exception judgment.",
            ),
        },
        _ => panic!("unexpected semantic regression case {case}"),
    }
}

#[test]
fn recorded_semantic_responses_match_requests_context_drafts_and_expected_issues() {
    let scenarios: Vec<RecordedSemanticCase> = serde_json::from_str(include_str!(
        "fixtures/ai_regression/semantic_critic_responses.json"
    ))
    .expect("recorded semantic fixtures");
    assert_eq!(
        scenarios
            .iter()
            .map(|scenario| scenario.case)
            .collect::<Vec<_>>(),
        SEMANTIC_CASES
    );

    for scenario in scenarios {
        let expected = expected_semantic_case(scenario.case);
        assert_eq!(scenario.case, expected.case);
        assert_eq!(scenario.name, expected.name);
        assert!(
            !scenario.request.trim().is_empty(),
            "case {} request",
            scenario.case
        );
        assert!(
            !scenario.snapshot.is_empty(),
            "case {} snapshot",
            scenario.case
        );
        assert!(
            !scenario.draft.operation_ids.is_empty(),
            "case {} draft",
            scenario.case
        );
        for operation_id in &scenario.draft.operation_ids {
            ChangeOperationId::from_str(operation_id).expect("recorded operation id");
        }
        for preserved in &scenario.draft.preserves {
            assert!(
                scenario.snapshot.contains(preserved),
                "case {} preserves an object outside its snapshot",
                scenario.case
            );
        }
        if scenario.case == 24 {
            assert_eq!(scenario.draft.retcon, "reinterpretive");
            assert!(!scenario.draft.preserves.is_empty());
        }

        let report = parse_critique_report(&scenario.response.to_string())
            .unwrap_or_else(|error| panic!("case {} recorded response: {error}", scenario.case));
        assert_eq!(
            report.issues.len(),
            expected.issues.len(),
            "case {} issue count",
            scenario.case
        );
        for (issue, expected_issue) in report.issues.iter().zip(expected.issues) {
            assert_eq!(
                issue.issue_id.as_str(),
                expected_issue.id,
                "case {} issue id",
                scenario.case
            );
            assert_eq!(
                issue.severity, expected_issue.severity,
                "case {} severity",
                scenario.case
            );
            assert_eq!(
                issue.category, expected_issue.category,
                "case {} category",
                scenario.case
            );
            assert_eq!(
                issue.attack_type,
                Some(expected_issue.attack),
                "case {} attack",
                scenario.case
            );
            assert_eq!(
                issue.affected_operation_ids,
                vec![
                    ChangeOperationId::from_str(expected_issue.operation)
                        .expect("expected operation")
                ],
                "case {} affected operation",
                scenario.case
            );
            assert!(
                scenario
                    .draft
                    .operation_ids
                    .iter()
                    .any(|id| id == expected_issue.operation)
            );
            assert_eq!(
                String::from(issue.evidence[0].source_uri),
                expected_issue.source,
                "case {} evidence source",
                scenario.case
            );
            assert!(
                scenario
                    .snapshot
                    .iter()
                    .any(|source| source == expected_issue.source)
            );
            assert!(!issue.evidence[0].excerpt_md.trim().is_empty());
        }

        let blocked = report
            .issues
            .iter()
            .filter(|issue| issue.severity == ValidationSeverity::Conflict)
            .flat_map(|issue| issue.affected_operation_ids.iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            blocked, expected.blocked_operations,
            "case {} blocked operations",
            scenario.case
        );
        assert_eq!(
            scenario.draft.exception.as_deref(),
            expected.allowed_exception,
            "case {} allowed exception",
            scenario.case
        );
        assert!(
            report
                .issues
                .iter()
                .all(|issue| issue.severity != ValidationSeverity::Error),
            "case {} semantic critic cannot create a hard error",
            scenario.case
        );
    }
}

#[test]
fn case_34_intentional_exception_keeps_conflict_waiver_and_human_judgment_traceable() {
    let path = project_path("ai-regression-intentional-exception");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    WorldStore::create(&path, &world).expect("create store");
    let gate = person(world.id(), "North Gate");
    let existing = canonical_claim(
        world.id(),
        gate.id(),
        world.current_revision(),
        ClaimPolarity::Positive,
    );
    let proposed = canonical_claim(
        world.id(),
        gate.id(),
        world.current_revision(),
        ClaimPolarity::Negative,
    );
    let mut store = WorldStore::open(&path).expect("open store");
    store.insert_entity(&gate).expect("insert gate");
    store
        .insert_claim(&existing)
        .expect("insert existing claim");
    drop(store);

    let mut app = NirmataApp::default();
    app.open_world(path.clone()).expect("open app");
    let mut review = app
        .start_manual_review(ManualReviewInput {
            objective: "Preserve an intentional contradictory report".to_owned(),
            sources: vec![ObjectRef::Claim(existing.id())],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateClaim {
                retcon: RetconKind::Additive,
                after: proposed,
            }],
        })
        .expect("start review");
    assert!(!review.ready_to_confirm());
    assert!(
        review
            .validation_report()
            .conflicts
            .iter()
            .any(|issue| issue.code == "claim.canonical_opposition")
    );

    let operation_id = review.operations()[0].operation_id();
    let rationale = "Intentional contradiction approved for this editorial branch.";
    let judgment = "Keep the conflict visible and preserve both cited claims.";
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::AddWaiver {
            operation_id,
            issue_code: "claim.canonical_opposition".to_owned(),
            rationale: rationale.to_owned(),
        },
    )
    .expect("record intentional exception");
    app.apply_manual_review_action(
        &mut review,
        ManualReviewAction::RecordJudgment {
            operation_id,
            judgment: judgment.to_owned(),
        },
    )
    .expect("record human judgment");

    assert!(review.ready_to_confirm());
    assert!(review.effective_report().conflicts.is_empty());
    assert_eq!(
        review.waivers()[0].issue_code(),
        "claim.canonical_opposition"
    );
    assert_eq!(review.waivers()[0].rationale(), rationale);
    assert_eq!(review.operations()[0].judgment(), Some(judgment));
    assert!(
        review
            .validation_report()
            .conflicts
            .iter()
            .any(|issue| issue.code == "claim.canonical_opposition"),
        "the exception must not erase its evidence"
    );

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}

#[test]
fn all_documented_mvp_cases_have_one_executable_owner() {
    let mut covered = DETERMINISTIC_CASES
        .into_iter()
        .chain(SEMANTIC_CASES)
        .collect::<Vec<_>>();
    covered.sort_unstable();
    assert_eq!(covered, (1..=34).collect::<Vec<_>>());
}
