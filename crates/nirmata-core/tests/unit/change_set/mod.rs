
use super::*;
use crate::entity::EntityKind;
use crate::event::{EventAggregate, EventLinkKind, EventParticipant};
use crate::goal::{GoalStatus, GoalVisibility};
use crate::relation::RelationDirection;
use crate::rule::{RuleKind, RuleSeverity};
use crate::time::{Certainty, EventTime, TimePrecision};
use serde_json::Value;

fn create_entity_operation(world_id: WorldId) -> ChangeOperation {
    let entity = Entity::new(
        world_id,
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

    ChangeOperation::CreateEntity {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: entity,
    }
}

fn entity_with(
    world_id: WorldId,
    id: crate::EntityId,
    name: &str,
    slug: &str,
    version: u64,
) -> Entity {
    Entity::restore(
        id,
        world_id,
        EntityKind::Person,
        name,
        slug,
        "",
        "",
        "{}",
        vec![],
        version,
        1,
        1,
    )
    .expect("entity")
}

fn create_goal_operation(world_id: WorldId, holder_entity_id: crate::EntityId) -> ChangeOperation {
    let goal = Goal::new(
        world_id,
        holder_entity_id,
        "Protect the gate",
        1,
        GoalStatus::Active,
        None,
        GoalVisibility::Secret,
        None,
    )
    .expect("goal");

    ChangeOperation::CreateGoal {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![
            ObjectRef::Goal(goal.id()),
            ObjectRef::Entity(holder_entity_id),
        ],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: goal,
    }
}

fn update_world_operation(
    world: &World,
    name: &str,
    premise_md: &str,
    epoch_label: &str,
) -> ChangeOperation {
    let after = World::restore(
        world.id(),
        name,
        premise_md,
        epoch_label,
        world.current_revision(),
        world.created_at_ms(),
        world.updated_at_ms() + 1,
    )
    .expect("updated world");

    ChangeOperation::UpdateWorld {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::World(world.id())],
        expected_version: 0,
        retcon: RetconKind::Reinterpretive,
        before: world.clone(),
        after,
    }
}

fn update_entity_operation(before: &Entity, after_name: &str, after_slug: &str) -> ChangeOperation {
    let after = Entity::restore(
        before.id(),
        before.world_id(),
        before.kind(),
        after_name,
        after_slug,
        before.summary(),
        before.body_md(),
        before.attributes_json().as_str(),
        before.aliases().to_vec(),
        before.version() + 1,
        before.created_at_ms(),
        before.updated_at_ms() + 1,
    )
    .expect("updated entity");

    ChangeOperation::UpdateEntity {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(before.id())],
        expected_version: before.version(),
        retcon: RetconKind::Additive,
        before: before.clone(),
        after,
    }
}

fn delete_entity_operation(entity: &Entity) -> ChangeOperation {
    ChangeOperation::DeleteEntity {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: entity.version(),
        retcon: RetconKind::Additive,
        before: entity.clone(),
    }
}

fn snapshot<'a>(
    entities: &'a [Entity],
    relations: &'a [Relation],
) -> ChangeSetValidationSnapshot<'a> {
    ChangeSetValidationSnapshot {
        entities,
        relations,
        goals: &[],
        events: &[],
        event_links: &[],
        rules: &[],
        claims: &[],
        documents: &[],
        content_references: &[],
        revisions: &[],
    }
}

fn snapshot_with_events<'a>(
    entities: &'a [Entity],
    events: &'a [Event],
    event_links: &'a [EventLink],
    rules: &'a [Rule],
) -> ChangeSetValidationSnapshot<'a> {
    ChangeSetValidationSnapshot {
        entities,
        relations: &[],
        goals: &[],
        events,
        event_links,
        rules,
        claims: &[],
        documents: &[],
        content_references: &[],
        revisions: &[],
    }
}

fn event_with(
    world_id: WorldId,
    id: crate::EventId,
    kind: &str,
    tick: Option<i64>,
    participants: Vec<EventParticipant>,
    version: u64,
) -> Event {
    let time = tick.map_or_else(
        || EventTime::unknown(Certainty::Uncertain),
        |tick| EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
    );
    Event::restore(
        id,
        world_id,
        kind,
        "",
        "",
        time,
        None,
        participants,
        vec![],
        version,
        1,
        1,
    )
    .expect("event")
}

fn create_event_operation(after: Event) -> ChangeOperation {
    ChangeOperation::CreateEvent {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Event(after.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: EventAggregate::new(after, vec![]),
    }
}

fn create_event_operation_with_links(after: Event, links: Vec<EventLink>) -> ChangeOperation {
    let mut affected_ids = vec![ObjectRef::Event(after.id())];
    affected_ids.extend(links.iter().flat_map(|link| {
        [
            ObjectRef::Event(link.source_event_id()),
            ObjectRef::Event(link.target_event_id()),
        ]
    }));
    affected_ids.sort_unstable();
    affected_ids.dedup();

    ChangeOperation::CreateEvent {
        operation_id: ChangeOperationId::new(),
        affected_ids,
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: EventAggregate::new(after, links),
    }
}

fn update_event_operation_with_links(
    before: &Event,
    tick: i64,
    links: Vec<EventLink>,
) -> ChangeOperation {
    let after = Event::restore(
        before.id(),
        before.world_id(),
        before.kind(),
        before.summary(),
        before.body_md(),
        EventTime::instant(tick, TimePrecision::Exact, Certainty::Certain),
        before.location_entity_id(),
        before.participants().to_vec(),
        before.affected_goal_ids().to_vec(),
        before.version() + 1,
        before.created_at_ms(),
        before.updated_at_ms() + 1,
    )
    .expect("updated event");

    let mut affected_ids = vec![ObjectRef::Event(before.id())];
    affected_ids.extend(links.iter().flat_map(|link| {
        [
            ObjectRef::Event(link.source_event_id()),
            ObjectRef::Event(link.target_event_id()),
        ]
    }));
    affected_ids.sort_unstable();
    affected_ids.dedup();

    ChangeOperation::UpdateEvent {
        operation_id: ChangeOperationId::new(),
        affected_ids,
        expected_version: before.version(),
        retcon: RetconKind::Additive,
        before: EventAggregate::new(before.clone(), links.clone()),
        after: EventAggregate::new(after, links),
    }
}

fn no_resurrection_rule(world_id: WorldId) -> Rule {
    Rule::new(
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
    .expect("rule")
}

#[test]
fn round_trips_changeset_variants() {
    let world_id = WorldId::new();
    let base_revision = RevisionId::new();
    let world = World::restore(
        world_id,
        "Arcadia",
        "Original premise",
        "First Dawn",
        base_revision,
        1,
        1,
    )
    .expect("world");
    let link_target = crate::EventId::new();
    let linked_event = event_with(world_id, crate::EventId::new(), "storm", Some(5), vec![], 1);
    let link =
        EventLink::new(linked_event.id(), link_target, EventLinkKind::Causes).expect("causal link");
    let event_operation = create_event_operation_with_links(linked_event, vec![link]);
    let draft = ChangeSetDraft::new(
        world_id,
        base_revision,
        "Add Mara",
        vec![],
        vec!["Mara is new".to_owned()],
        vec![
            update_world_operation(&world, "Arcadia Revised", "Updated premise", "Second Dawn"),
            event_operation.clone(),
        ],
        vec![],
    )
    .expect("draft");
    let draft_json = serde_json::to_string(&draft).expect("serialize draft");
    let restored_draft: ChangeSetDraft =
        serde_json::from_str(&draft_json).expect("deserialize draft");
    assert_eq!(restored_draft, draft);

    let change_set = ChangeSet::new(
        world_id,
        base_revision,
        "Add Mara",
        vec![],
        vec!["Mara is new".to_owned()],
        vec![
            update_world_operation(&world, "Arcadia Revised", "Updated premise", "Second Dawn"),
            event_operation,
        ],
        vec![],
    )
    .expect("change set");
    let change_set_json = serde_json::to_string(&change_set).expect("serialize change set");
    let restored_change_set: ChangeSet =
        serde_json::from_str(&change_set_json).expect("deserialize change set");
    assert_eq!(restored_change_set, change_set);
}

#[test]
fn rejects_unknown_fields_and_missing_required_fields() {
    let world_id = WorldId::new();
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Add Mara",
        vec![],
        vec![],
        vec![create_entity_operation(world_id)],
        vec![],
    )
    .expect("draft");

    let mut value = serde_json::to_value(&draft).expect("value");
    value
        .as_object_mut()
        .expect("object")
        .insert("unexpected".to_owned(), Value::Bool(true));
    assert!(serde_json::from_value::<ChangeSetDraft>(value).is_err());

    let mut value = serde_json::to_value(&draft).expect("value");
    value.as_object_mut().expect("object").remove("objective");
    assert!(serde_json::from_value::<ChangeSetDraft>(value).is_err());
}

#[test]
fn rejects_cross_world_operations() {
    let world_id = WorldId::new();
    let other_world_id = WorldId::new();

    let result = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Add Mara",
        vec![],
        vec![],
        vec![create_entity_operation(other_world_id)],
        vec![],
    );

    assert_eq!(
        result,
        Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world"
        ))
    );
}

#[test]
fn rejects_world_updates_that_mutate_revision_or_expected_version() {
    let world_id = WorldId::new();
    let before = World::restore(
        world_id,
        "Arcadia",
        "",
        "First Dawn",
        RevisionId::new(),
        1,
        1,
    )
    .expect("world");
    let after = World::restore(
        world_id,
        "Arcadia",
        "",
        "First Dawn",
        RevisionId::new(),
        1,
        2,
    )
    .expect("world");

    let result = ChangeSetDraft::new(
        world_id,
        before.current_revision(),
        "Invalid world update",
        vec![],
        vec![],
        vec![ChangeOperation::UpdateWorld {
            operation_id: ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::World(world_id)],
            expected_version: 1,
            retcon: RetconKind::Reinterpretive,
            before,
            after,
        }],
        vec![],
    );

    assert_eq!(
        result,
        Err(DomainError::InvalidChangeSetContext(
            "world updates do not use numeric versions"
        ))
    );
}

#[test]
fn reports_cross_world_event_links_in_resulting_state() {
    let world_id = WorldId::new();
    let other_world_id = WorldId::new();
    let local_event = event_with(world_id, crate::EventId::new(), "storm", Some(5), vec![], 1);
    let foreign_event = event_with(
        other_world_id,
        crate::EventId::new(),
        "aftermath",
        Some(6),
        vec![],
        1,
    );
    let link = EventLink::new(local_event.id(), foreign_event.id(), EventLinkKind::Causes)
        .expect("cross world link");
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Link across worlds",
        vec![],
        vec![],
        vec![create_event_operation_with_links(local_event, vec![link])],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&snapshot_with_events(
        &[],
        std::slice::from_ref(&foreign_event),
        &[],
        &[],
    ));

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "reference.cross_world")
    );
}

#[test]
fn reports_missing_entity_reference_in_changeset_validation() {
    let world_id = WorldId::new();
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Add goal",
        vec![],
        vec![],
        vec![create_goal_operation(world_id, crate::EntityId::new())],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "goal.holder_missing")
    );
}

#[test]
fn reports_double_write_conflict() {
    let world_id = WorldId::new();
    let entity = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Rename Mara twice",
        vec![],
        vec![],
        vec![
            update_entity_operation(&entity, "Mara One", "mara-one"),
            update_entity_operation(&entity, "Mara Two", "mara-two"),
        ],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&snapshot(std::slice::from_ref(&entity), &[]));

    assert!(
        report
            .conflicts
            .iter()
            .any(|issue| issue.code == "change_set.operation.double_write")
    );
}

#[test]
fn reports_orphan_delete() {
    let world_id = WorldId::new();
    let mara = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
    let vale = entity_with(world_id, crate::EntityId::new(), "Vale", "vale", 1);
    let relation = Relation::new(
        world_id,
        mara.id(),
        vale.id(),
        "allied_with",
        RelationDirection::Directed,
        None,
        None,
        Certainty::Certain,
        None,
        "{}",
    )
    .expect("relation");
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Delete Mara",
        vec![],
        vec![],
        vec![delete_entity_operation(&mara)],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&snapshot(&[mara, vale], &[relation]));

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.delete_orphan")
    );
}

#[test]
fn reports_missing_dependency_when_reference_is_created_later() {
    let world_id = WorldId::new();
    let entity = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
    let create_entity = ChangeOperation::CreateEntity {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: entity.clone(),
    };
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Add goal before entity",
        vec![],
        vec![],
        vec![create_goal_operation(world_id, entity.id()), create_entity],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "change_set.dependency_order")
    );
}

#[test]
fn allows_creating_an_entity_before_referencing_it() {
    let world_id = WorldId::new();
    let entity = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
    let create_entity = ChangeOperation::CreateEntity {
        operation_id: ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: entity.clone(),
    };
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Add entity then goal",
        vec![],
        vec![],
        vec![create_entity, create_goal_operation(world_id, entity.id())],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&ChangeSetValidationSnapshot::empty());

    assert!(report.is_ok());
}

#[test]
fn blocks_no_resurrection_in_the_resulting_state() {
    let world_id = WorldId::new();
    let mara = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
    let death = event_with(
        world_id,
        crate::EventId::new(),
        "death",
        Some(10),
        vec![EventParticipant::new(mara.id(), "subject", 0).expect("participant")],
        1,
    );
    let return_event = event_with(
        world_id,
        crate::EventId::new(),
        "return",
        Some(20),
        vec![EventParticipant::new(mara.id(), "actor", 0).expect("participant")],
        1,
    );
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Bring Mara back",
        vec![],
        vec![],
        vec![create_event_operation(return_event)],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&snapshot_with_events(
        std::slice::from_ref(&mara),
        std::slice::from_ref(&death),
        &[],
        std::slice::from_ref(&no_resurrection_rule(world_id)),
    ));

    assert!(
        report
            .errors
            .iter()
            .any(|issue| issue.code == "rule.no_resurrection")
    );
}

#[test]
fn blocks_cause_after_effect_in_the_resulting_state() {
    let world_id = WorldId::new();
    let cause = event_with(world_id, crate::EventId::new(), "cause", Some(5), vec![], 1);
    let effect = event_with(
        world_id,
        crate::EventId::new(),
        "effect",
        Some(10),
        vec![],
        1,
    );
    let link = EventLink::new(cause.id(), effect.id(), EventLinkKind::Causes).expect("causal link");
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Move cause after effect",
        vec![],
        vec![],
        vec![update_event_operation_with_links(
            &cause,
            20,
            vec![link.clone()],
        )],
        vec![],
    )
    .expect("draft");

    let report =
        draft.validation_report(&snapshot_with_events(&[], &[cause, effect], &[link], &[]));

    assert!(
        report
            .conflicts
            .iter()
            .any(|issue| issue.code == "causality.cause_after_effect")
    );
}

#[test]
fn leaves_semantic_rules_and_unspecified_time_as_non_blocking() {
    let world_id = WorldId::new();
    let mara = entity_with(world_id, crate::EntityId::new(), "Mara", "mara", 1);
    let unknown_death = event_with(
        world_id,
        crate::EventId::new(),
        "death",
        None,
        vec![EventParticipant::new(mara.id(), "subject", 0).expect("participant")],
        1,
    );
    let travel = event_with(
        world_id,
        crate::EventId::new(),
        "travel",
        Some(20),
        vec![EventParticipant::new(mara.id(), "traveler", 0).expect("participant")],
        1,
    );
    let institutional_rule = Rule::new(
        world_id,
        RuleKind::Institutional,
        "No one may travel alone.",
        "capital",
        RuleSeverity::Advisory,
        None,
        None,
        "{}",
        1,
    )
    .expect("rule");
    let draft = ChangeSetDraft::new(
        world_id,
        RevisionId::new(),
        "Send Mara traveling",
        vec![],
        vec![],
        vec![create_event_operation(travel)],
        vec![],
    )
    .expect("draft");

    let report = draft.validation_report(&snapshot_with_events(
        std::slice::from_ref(&mara),
        std::slice::from_ref(&unknown_death),
        &[],
        &[institutional_rule, no_resurrection_rule(world_id)],
    ));

    assert!(report.is_ok());
    assert!(
        report
            .errors
            .iter()
            .all(|issue| issue.code != "rule.no_resurrection")
    );
}
