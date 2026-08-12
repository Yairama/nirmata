pub(crate) fn operation_object_snapshot_before(
    operation: &ChangeOperation,
) -> Option<ManualReviewObjectSnapshot> {
    match operation {
        ChangeOperation::UpdateWorld { before, .. } => Some(world_snapshot(before)),
        ChangeOperation::UpdateEntity { before, .. }
        | ChangeOperation::DeleteEntity { before, .. } => Some(entity_snapshot(before)),
        ChangeOperation::UpdateRelation { before, .. }
        | ChangeOperation::DeleteRelation { before, .. } => Some(relation_snapshot(before)),
        ChangeOperation::UpdateEvent { before, .. }
        | ChangeOperation::DeleteEvent { before, .. } => Some(event_snapshot(before)),
        ChangeOperation::UpdateGoal { before, .. } | ChangeOperation::DeleteGoal { before, .. } => {
            Some(goal_snapshot(before))
        }
        ChangeOperation::UpdateRule { before, .. } | ChangeOperation::DeleteRule { before, .. } => {
            Some(rule_snapshot(before))
        }
        ChangeOperation::UpdateClaim { before, .. }
        | ChangeOperation::DeleteClaim { before, .. } => Some(claim_snapshot(before)),
        ChangeOperation::UpdateDocument { before, .. }
        | ChangeOperation::DeleteDocument { before, .. } => Some(document_snapshot(before)),
        ChangeOperation::CreateEntity { .. }
        | ChangeOperation::CreateRelation { .. }
        | ChangeOperation::CreateEvent { .. }
        | ChangeOperation::CreateGoal { .. }
        | ChangeOperation::CreateRule { .. }
        | ChangeOperation::CreateClaim { .. }
        | ChangeOperation::CreateDocument { .. } => None,
    }
}

pub(crate) fn operation_object_snapshot_after(
    operation: &ChangeOperation,
) -> Option<ManualReviewObjectSnapshot> {
    match operation {
        ChangeOperation::UpdateWorld { after, .. } => Some(world_snapshot(after)),
        ChangeOperation::CreateEntity { after, .. }
        | ChangeOperation::UpdateEntity { after, .. } => Some(entity_snapshot(after)),
        ChangeOperation::CreateRelation { after, .. }
        | ChangeOperation::UpdateRelation { after, .. } => Some(relation_snapshot(after)),
        ChangeOperation::CreateEvent { after, .. } | ChangeOperation::UpdateEvent { after, .. } => {
            Some(event_snapshot(after))
        }
        ChangeOperation::CreateGoal { after, .. } | ChangeOperation::UpdateGoal { after, .. } => {
            Some(goal_snapshot(after))
        }
        ChangeOperation::CreateRule { after, .. } | ChangeOperation::UpdateRule { after, .. } => {
            Some(rule_snapshot(after))
        }
        ChangeOperation::CreateClaim { after, .. } | ChangeOperation::UpdateClaim { after, .. } => {
            Some(claim_snapshot(after))
        }
        ChangeOperation::CreateDocument { after, .. }
        | ChangeOperation::UpdateDocument { after, .. } => Some(document_snapshot(after)),
        ChangeOperation::DeleteEntity { .. }
        | ChangeOperation::DeleteRelation { .. }
        | ChangeOperation::DeleteEvent { .. }
        | ChangeOperation::DeleteGoal { .. }
        | ChangeOperation::DeleteRule { .. }
        | ChangeOperation::DeleteClaim { .. }
        | ChangeOperation::DeleteDocument { .. } => None,
    }
}

fn decision_label(decision: OperationDecision) -> &'static str {
    match decision {
        OperationDecision::Accept => "accept",
        OperationDecision::Edit => "edit",
        OperationDecision::Reject => "reject",
    }
}

fn world_snapshot(world: &World) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: world.name().to_owned(),
        object_type: "world".to_owned(),
        target_uri: ObjectRef::World(world.id()).to_string(),
        lines: vec![
            line_item("Premisa", preview(world.premise_md())),
            line_item("Epoch", preview(world.epoch_label())),
            line_item("Revisión", world.current_revision().to_string()),
        ],
    }
}

fn entity_snapshot(entity: &Entity) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: entity.name().to_owned(),
        object_type: "entity".to_owned(),
        target_uri: ObjectRef::Entity(entity.id()).to_string(),
        lines: vec![
            line_item("Tipo", format!("{:?}", entity.kind())),
            line_item("Slug", entity.slug().to_owned()),
            line_item("Resumen", preview(entity.summary())),
            line_item("Cuerpo", preview(entity.body_md())),
        ],
    }
}

fn relation_snapshot(relation: &Relation) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: relation.kind().to_owned(),
        object_type: "relation".to_owned(),
        target_uri: ObjectRef::Relation(relation.id()).to_string(),
        lines: vec![
            line_item(
                "Origen",
                ObjectRef::Entity(relation.source_entity_id()).to_string(),
            ),
            line_item(
                "Destino",
                ObjectRef::Entity(relation.target_entity_id()).to_string(),
            ),
            line_item("Certeza", format!("{:?}", relation.certainty())),
        ],
    }
}

fn event_snapshot(event: &EventAggregate) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: event.event().summary().to_owned(),
        object_type: "event".to_owned(),
        target_uri: ObjectRef::Event(event.event().id()).to_string(),
        lines: vec![
            line_item("Tipo", event.event().kind().to_owned()),
            line_item("Tiempo", format_event_time(event.event().time())),
            line_item(
                "Participantes",
                event.event().participants().len().to_string(),
            ),
            line_item("Causalidad", event.links().len().to_string()),
        ],
    }
}

fn goal_snapshot(goal: &Goal) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: preview(goal.desired_state_md()),
        object_type: "goal".to_owned(),
        target_uri: ObjectRef::Goal(goal.id()).to_string(),
        lines: vec![
            line_item(
                "Holder",
                ObjectRef::Entity(goal.holder_entity_id()).to_string(),
            ),
            line_item("Estado", format!("{:?}", goal.status())),
            line_item("Visibilidad", format!("{:?}", goal.visibility())),
        ],
    }
}

fn rule_snapshot(rule: &Rule) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: preview(rule.statement_md()),
        object_type: "rule".to_owned(),
        target_uri: ObjectRef::Rule(rule.id()).to_string(),
        lines: vec![
            line_item("Tipo", format!("{:?}", rule.kind())),
            line_item("Scope", rule.scope().to_owned()),
            line_item("Severidad", format!("{:?}", rule.severity())),
        ],
    }
}

fn claim_snapshot(claim: &Claim) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: preview(claim.content_md()),
        object_type: "claim".to_owned(),
        target_uri: ObjectRef::Claim(claim.id()).to_string(),
        lines: vec![
            line_item(
                "Sujeto",
                ObjectRef::Entity(claim.subject_entity_id()).to_string(),
            ),
            line_item("Autenticación", format!("{:?}", claim.authentication())),
            line_item("Contenido", preview(claim.content_md())),
        ],
    }
}

fn document_snapshot(document: &DocumentAggregate) -> ManualReviewObjectSnapshot {
    ManualReviewObjectSnapshot {
        title: document.object().title().to_owned(),
        object_type: "document".to_owned(),
        target_uri: ObjectRef::Document(document.object().id()).to_string(),
        lines: vec![
            line_item("Tipo", document.object().kind().to_owned()),
            line_item("Canon", format!("{:?}", document.object().canon_status())),
            line_item("Referencias", document.references().len().to_string()),
            line_item(
                "Detalle de referencias",
                document
                    .references()
                    .iter()
                    .map(|reference| format!("{}|{}", reference.target(), reference.ordinal()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            line_item("Cuerpo", preview(document.object().body_md())),
        ],
    }
}

pub(crate) fn object_snapshot_from_change_value(
    value: &ChangeOperationValue,
) -> ManualReviewObjectSnapshot {
    match value {
        ChangeOperationValue::World(world) => world_snapshot(world),
        ChangeOperationValue::Entity(entity) => entity_snapshot(entity),
        ChangeOperationValue::Relation(relation) => relation_snapshot(relation),
        ChangeOperationValue::Event(event) => event_snapshot(event),
        ChangeOperationValue::Goal(goal) => goal_snapshot(goal),
        ChangeOperationValue::Rule(rule) => rule_snapshot(rule),
        ChangeOperationValue::Claim(claim) => claim_snapshot(claim),
        ChangeOperationValue::Document(document) => document_snapshot(document),
    }
}

fn line_item(label: &str, value: impl Into<String>) -> ManualReviewLineItem {
    ManualReviewLineItem {
        label: label.to_owned(),
        value: value.into(),
    }
}

fn preview(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "—".to_owned()
    } else if trimmed.chars().count() > 120 {
        format!("{}…", trimmed.chars().take(120).collect::<String>())
    } else {
        trimmed.to_owned()
    }
}

fn is_blank_option(value: Option<&str>) -> bool {
    match value {
        None => true,
        Some(item) => item.trim().is_empty(),
    }
}

fn format_event_time(time: &nirmata_core::time::EventTime) -> String {
    match (time.kind(), time.start_tick(), time.end_tick()) {
        (nirmata_core::time::EventTimeKind::Unknown, _, _) => {
            format!("unknown · {:?} · {:?}", time.precision(), time.certainty())
        }
        (nirmata_core::time::EventTimeKind::Instant, Some(start), _) => {
            format!(
                "tick {start} · {:?} · {:?}",
                time.precision(),
                time.certainty()
            )
        }
        (nirmata_core::time::EventTimeKind::Ongoing, Some(start), _) => {
            format!(
                "since {start} · {:?} · {:?}",
                time.precision(),
                time.certainty()
            )
        }
        (nirmata_core::time::EventTimeKind::Interval, Some(start), Some(end)) => {
            format!(
                "ticks {start} → {end} · {:?} · {:?}",
                time.precision(),
                time.certainty()
            )
        }
        _ => format!("{:?}", time.kind()),
    }
}
