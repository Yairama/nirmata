
use super::*;
use crate::claim::{ClaimModality, ClaimObject, ClaimPolarity};
use crate::document::{DocumentCanonStatus, ordered_content_references};
use crate::entity::EntityKind;
use crate::event::{EventLinkKind, EventParticipant};
use crate::goal::{GoalStatus, GoalVisibility};
use crate::relation::RelationDirection;
use crate::time::{Certainty, EventTime, TimePrecision};
use crate::{DomainError, EntityId};

fn entity(world_id: WorldId, name: &str) -> Entity {
    Entity::new(
        world_id,
        EntityKind::Person,
        name,
        name.to_lowercase(),
        "",
        "",
        "{}",
        vec![],
        1,
    )
    .expect("entity")
}

fn event(world_id: WorldId, tick: i64, participants: Vec<EventParticipant>) -> Event {
    Event::new(
        world_id,
        "event",
        "",
        "",
        EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
        None,
        participants,
        vec![],
        1,
    )
    .expect("event")
}

#[test]
fn validates_references_duplicates_and_self_relation_declarations() {
    let world_id = WorldId::new();
    let mara = entity(world_id, "Mara");
    let vale = entity(world_id, "Vale");
    let entities = [mara.clone(), vale.clone()];
    let missing = EntityId::new();
    let relation = Relation::new(
        world_id,
        mara.id(),
        missing,
        "mirrors",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation shape");
    assert!(
        validate_relations(&[relation], &entities, &[])
            .iter()
            .any(|issue| issue.code == "relation.target_missing")
    );

    let self_relation = Relation::new(
        world_id,
        mara.id(),
        mara.id(),
        "remembers_self",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation shape");
    assert!(
        validate_relations(std::slice::from_ref(&self_relation), &entities, &[])
            .iter()
            .any(|issue| issue.code == "relation.self_not_allowed")
    );
    assert!(validate_relations(&[self_relation], &entities, &["remembers_self"]).is_empty());

    let first = Relation::new(
        world_id,
        mara.id(),
        vale.id(),
        "allied_with",
        RelationDirection::Undirected,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation");
    let reversed = Relation::new(
        world_id,
        vale.id(),
        mara.id(),
        "allied_with",
        RelationDirection::Undirected,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation");
    assert!(
        validate_relations(&[first, reversed], &[mara, vale], &[])
            .iter()
            .any(|issue| issue.code == "relation.duplicate")
    );

    let active = Relation::new(
        world_id,
        entities[0].id(),
        entities[1].id(),
        "holds_role",
        RelationDirection::Directed,
        Some(5),
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("active relation");
    assert_eq!(relation_active_at(&active, 4), PartialTruth::False);
    assert_eq!(relation_active_at(&active, 5), PartialTruth::True);
}

#[test]
fn validates_goal_event_and_causal_time() {
    let world_id = WorldId::new();
    let mara = entity(world_id, "Mara");
    let goal = Goal::new(
        world_id,
        mara.id(),
        "Open the gate",
        1,
        GoalStatus::Active,
        None,
        GoalVisibility::Secret,
        None,
    )
    .expect("goal");
    assert!(validate_goals(std::slice::from_ref(&goal), std::slice::from_ref(&mara)).is_empty());
    assert!(
        validate_goals(std::slice::from_ref(&goal), &[])
            .iter()
            .any(|issue| issue.code == "goal.holder_missing")
    );

    let cause = event(
        world_id,
        20,
        vec![EventParticipant::new(mara.id(), "actor", 0).expect("participant")],
    );
    let effect = event(world_id, 10, vec![]);
    assert!(validate_events(&[cause.clone(), effect.clone()], &[mara], &[goal]).is_empty());
    let link = EventLink::new(cause.id(), effect.id(), EventLinkKind::Causes).expect("causal link");
    assert!(
        validate_event_links(&[link], &[cause, effect])
            .iter()
            .any(|issue| issue.code == "causality.cause_after_effect")
    );

    let missing_link = EventLink::new(
        crate::EventId::new(),
        crate::EventId::new(),
        EventLinkKind::Causes,
    )
    .expect("causal link shape");
    assert!(
        validate_event_links(&[missing_link], &[])
            .iter()
            .any(|issue| issue.code == "causality.event_missing")
    );
}

#[test]
fn detects_lifecycle_conflicts_but_leaves_unknown_time_unspecified() {
    let world_id = WorldId::new();
    let mara = entity(world_id, "Mara");
    let birth = event(world_id, 20, vec![]);
    let death = event(world_id, 10, vec![]);
    let later = event(world_id, 30, vec![]);
    let issues = validate_lifecycle(&mara, Some(&birth), Some(&death), &[&later]);

    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "lifecycle.death_before_birth")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "lifecycle.participation_after_death")
    );

    let unknown = Event::new(
        world_id,
        "unknown",
        "",
        "",
        EventTime::unknown(Certainty::Uncertain),
        None,
        vec![],
        vec![],
        1,
    )
    .expect("unknown event");
    assert!(validate_lifecycle(&mara, None, Some(&unknown), &[&later]).is_empty());
}

#[test]
fn separates_claim_contexts_and_detects_only_canonical_opposition() {
    let world_id = WorldId::new();
    let subject = entity(world_id, "Gate");
    let holder = entity(world_id, "Witness");
    let revision = RevisionId::new();
    let revisions = HashSet::from([revision]);

    let canonical = |polarity| {
        Claim::new(
            world_id,
            subject.id(),
            "The gate state.",
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
            None,
            revision,
        )
        .expect("canonical claim")
    };
    let positive = canonical(ClaimPolarity::Positive);
    let negative = canonical(ClaimPolarity::Negative);
    let issues = validate_claims(
        &[positive.clone(), negative],
        &[subject.clone(), holder.clone()],
        &[],
        &revisions,
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "claim.canonical_opposition")
    );

    let rumor = Claim::new(
        world_id,
        subject.id(),
        "The gate is not open.",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Negative,
        ClaimAuthentication::Attributed,
        Some(holder.id()),
        Some(ClaimModality::Belief),
        Some("rumor".to_owned()),
        None,
        None,
        None,
        None,
        Some(0.5),
        None,
        revision,
    )
    .expect("rumor");
    assert!(
        validate_claims(&[positive, rumor], &[subject, holder], &[], &revisions,)
            .iter()
            .all(|issue| issue.code != "claim.canonical_opposition")
    );
}

#[test]
fn validates_content_targets_and_stable_discourse_order() {
    let world_id = WorldId::new();
    let mara = entity(world_id, "Mara");
    let document = Document::new(
        world_id,
        "Chronicle",
        "chronicle",
        Some(mara.id()),
        None,
        DocumentCanonStatus::Canonical,
        "",
        1,
    )
    .expect("document");
    let source = ObjectRef::Document(document.id());
    let valid = ContentReference::new(source, ObjectRef::Entity(mara.id()), 1);
    let missing = ContentReference::new(source, ObjectRef::Entity(EntityId::new()), 0);
    let issues = validate_content_references(
        &[valid.clone(), missing],
        &[],
        &[mara],
        &[],
        &[],
        &[],
        &[],
        &[document],
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "content_reference.target_missing")
    );

    let refs = vec![
        valid,
        ContentReference::new(source, ObjectRef::Entity(EntityId::new()), 0),
    ];
    let ordered = ordered_content_references(source, &refs);
    assert_eq!(ordered[0].ordinal(), 0);
    assert_eq!(ordered[1].ordinal(), 1);
}

#[test]
fn reports_version_mismatch() {
    let issue = validate_expected_version(IssueObject::new("entity", EntityId::new()), 2, 1)
        .expect("version mismatch");
    assert_eq!(issue.code, "version.mismatch");
    assert_eq!(
        crate::Period::new(Some(2), Some(1)),
        Err(DomainError::InvalidPeriod)
    );
}
