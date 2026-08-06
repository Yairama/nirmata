fn inverse_operation(
    operation: &ChangeOperation,
    audit: &OperationAudit,
    store: &WorldStore,
    now_ms: i64,
) -> Result<DraftOperationInput, AppError> {
    match operation {
        ChangeOperation::UpdateWorld { retcon, before, .. } => {
            let current = current_world(store, before)?;
            Ok(DraftOperationInput::UpdateWorld {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_world(before, current.current_revision(), now_ms)?,
            })
        }
        ChangeOperation::CreateEntity { retcon, after, .. } => {
            let current = current_entity(store, after.id(), after)?;
            Ok(DraftOperationInput::DeleteEntity {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateEntity { retcon, before, .. } => {
            let current = current_entity(store, before.id(), entity_after(audit)?)?;
            Ok(DraftOperationInput::UpdateEntity {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_entity(before, current.version() + 1, now_ms)?,
            })
        }
        ChangeOperation::DeleteEntity { retcon, .. } => Ok(DraftOperationInput::CreateEntity {
            retcon: undo_restore_retcon(*retcon),
            after: restored_entity(entity_before(audit)?, 1, now_ms)?,
        }),
        ChangeOperation::CreateRelation { retcon, after, .. } => {
            let current = current_relation(store, after.id(), after)?;
            Ok(DraftOperationInput::DeleteRelation {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateRelation { retcon, before, .. } => {
            let current = current_relation(store, before.id(), relation_after(audit)?)?;
            Ok(DraftOperationInput::UpdateRelation {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_relation(before, current.version() + 1)?,
            })
        }
        ChangeOperation::DeleteRelation { retcon, .. } => Ok(DraftOperationInput::CreateRelation {
            retcon: undo_restore_retcon(*retcon),
            after: restored_relation(relation_before(audit)?, 1)?,
        }),
        ChangeOperation::CreateEvent { retcon, after, .. } => {
            let current = current_event(store, after.event().id(), after)?;
            Ok(DraftOperationInput::DeleteEvent {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateEvent { retcon, before, .. } => {
            let current = current_event(store, before.event().id(), event_after(audit)?)?;
            Ok(DraftOperationInput::UpdateEvent {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_event(before, current.event().version() + 1, now_ms)?,
            })
        }
        ChangeOperation::DeleteEvent { retcon, .. } => Ok(DraftOperationInput::CreateEvent {
            retcon: undo_restore_retcon(*retcon),
            after: restored_event(event_before(audit)?, 1, now_ms)?,
        }),
        ChangeOperation::CreateGoal { retcon, after, .. } => {
            let current = current_goal(store, after.id(), after)?;
            Ok(DraftOperationInput::DeleteGoal {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateGoal { retcon, before, .. } => {
            let current = current_goal(store, before.id(), goal_after(audit)?)?;
            Ok(DraftOperationInput::UpdateGoal {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_goal(before, current.version() + 1)?,
            })
        }
        ChangeOperation::DeleteGoal { retcon, .. } => Ok(DraftOperationInput::CreateGoal {
            retcon: undo_restore_retcon(*retcon),
            after: restored_goal(goal_before(audit)?, 1)?,
        }),
        ChangeOperation::CreateRule { retcon, after, .. } => {
            let current = current_rule(store, after.id(), after)?;
            Ok(DraftOperationInput::DeleteRule {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateRule { retcon, before, .. } => {
            let current = current_rule(store, before.id(), rule_after(audit)?)?;
            Ok(DraftOperationInput::UpdateRule {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_rule(before, current.version() + 1, now_ms)?,
            })
        }
        ChangeOperation::DeleteRule { retcon, .. } => Ok(DraftOperationInput::CreateRule {
            retcon: undo_restore_retcon(*retcon),
            after: restored_rule(rule_before(audit)?, 1, now_ms)?,
        }),
        ChangeOperation::CreateClaim { retcon, after, .. } => {
            let current = current_claim(store, after.id(), after)?;
            Ok(DraftOperationInput::DeleteClaim {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateClaim { retcon, before, .. } => {
            let current = current_claim(store, before.id(), claim_after(audit)?)?;
            Ok(DraftOperationInput::UpdateClaim {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_claim(before, current.version() + 1)?,
            })
        }
        ChangeOperation::DeleteClaim { retcon, .. } => Ok(DraftOperationInput::CreateClaim {
            retcon: undo_restore_retcon(*retcon),
            after: restored_claim(claim_before(audit)?, 1)?,
        }),
        ChangeOperation::CreateDocument { retcon, after, .. } => {
            let current = current_document(store, after.object().id(), after)?;
            Ok(DraftOperationInput::DeleteDocument {
                retcon: undo_delete_retcon(*retcon),
                before: current,
            })
        }
        ChangeOperation::UpdateDocument { retcon, before, .. } => {
            let current = current_document(store, before.object().id(), document_after(audit)?)?;
            Ok(DraftOperationInput::UpdateDocument {
                retcon: undo_restore_retcon(*retcon),
                before: current.clone(),
                after: restored_document(before, current.object().version() + 1, now_ms)?,
            })
        }
        ChangeOperation::DeleteDocument { retcon, .. } => Ok(DraftOperationInput::CreateDocument {
            retcon: undo_restore_retcon(*retcon),
            after: restored_document(document_before(audit)?, 1, now_ms)?,
        }),
    }
}

fn current_entity(
    store: &WorldStore,
    id: nirmata_core::EntityId,
    fallback: &Entity,
) -> Result<Entity, AppError> {
    Ok(store.get_entity(id)?.unwrap_or_else(|| fallback.clone()))
}

fn current_world(store: &WorldStore, fallback: &World) -> Result<World, AppError> {
    Ok(store.load_world().unwrap_or_else(|_| fallback.clone()))
}

fn current_relation(
    store: &WorldStore,
    id: nirmata_core::RelationId,
    fallback: &Relation,
) -> Result<Relation, AppError> {
    Ok(store.get_relation(id)?.unwrap_or_else(|| fallback.clone()))
}

fn current_event(
    store: &WorldStore,
    id: nirmata_core::EventId,
    fallback: &EventAggregate,
) -> Result<EventAggregate, AppError> {
    Ok(store
        .get_event(id)?
        .map(|aggregate| aggregate.clone())
        .unwrap_or_else(|| fallback.clone()))
}

fn current_goal(
    store: &WorldStore,
    id: nirmata_core::GoalId,
    fallback: &Goal,
) -> Result<Goal, AppError> {
    Ok(store.get_goal(id)?.unwrap_or_else(|| fallback.clone()))
}

fn current_rule(
    store: &WorldStore,
    id: nirmata_core::RuleId,
    fallback: &Rule,
) -> Result<Rule, AppError> {
    Ok(store.get_rule(id)?.unwrap_or_else(|| fallback.clone()))
}

fn current_claim(
    store: &WorldStore,
    id: nirmata_core::ClaimId,
    fallback: &Claim,
) -> Result<Claim, AppError> {
    Ok(store.get_claim(id)?.unwrap_or_else(|| fallback.clone()))
}

fn current_document(
    store: &WorldStore,
    id: nirmata_core::DocumentId,
    fallback: &DocumentAggregate,
) -> Result<DocumentAggregate, AppError> {
    Ok(store.get_document(id)?.unwrap_or_else(|| fallback.clone()))
}

fn restored_entity(snapshot: &Entity, version: u64, now_ms: i64) -> Result<Entity, AppError> {
    Entity::restore(
        snapshot.id(),
        snapshot.world_id(),
        snapshot.kind(),
        snapshot.name(),
        snapshot.slug(),
        snapshot.summary().to_owned(),
        snapshot.body_md().to_owned(),
        snapshot.attributes_json().as_str().to_owned(),
        snapshot.aliases().to_vec(),
        version,
        snapshot.created_at_ms(),
        now_ms,
    )
    .map_err(Into::into)
}

fn restored_relation(snapshot: &Relation, version: u64) -> Result<Relation, AppError> {
    Relation::restore(
        snapshot.id(),
        snapshot.world_id(),
        snapshot.source_entity_id(),
        snapshot.target_entity_id(),
        snapshot.kind(),
        snapshot.direction(),
        snapshot.valid_from_tick(),
        snapshot.valid_to_tick(),
        snapshot.certainty(),
        snapshot.source_reference().map(str::to_owned),
        snapshot.metadata_json().as_str().to_owned(),
        version,
    )
    .map_err(Into::into)
}

fn restored_world(
    snapshot: &World,
    current_revision: RevisionId,
    now_ms: i64,
) -> Result<World, AppError> {
    World::restore(
        snapshot.id(),
        snapshot.name(),
        snapshot.premise_md(),
        snapshot.epoch_label(),
        current_revision,
        snapshot.created_at_ms(),
        now_ms,
    )
    .map_err(Into::into)
}

fn restored_event(
    snapshot: &EventAggregate,
    version: u64,
    now_ms: i64,
) -> Result<EventAggregate, AppError> {
    Ok(EventAggregate::new(
        Event::restore(
            snapshot.event().id(),
            snapshot.event().world_id(),
            snapshot.event().kind(),
            snapshot.event().summary(),
            snapshot.event().body_md(),
            snapshot.event().time().clone(),
            snapshot.event().location_entity_id(),
            snapshot.event().participants().to_vec(),
            snapshot.event().affected_goal_ids().to_vec(),
            version,
            snapshot.event().created_at_ms(),
            now_ms,
        )
        .map_err(AppError::from)?,
        snapshot.links().to_vec(),
    ))
}

fn restored_goal(snapshot: &Goal, version: u64) -> Result<Goal, AppError> {
    Goal::restore(
        snapshot.id(),
        snapshot.world_id(),
        snapshot.holder_entity_id(),
        snapshot.desired_state_md(),
        snapshot.priority(),
        snapshot.status(),
        snapshot.period(),
        snapshot.visibility(),
        snapshot.source().map(str::to_owned),
        version,
    )
    .map_err(Into::into)
}

fn restored_rule(snapshot: &Rule, version: u64, now_ms: i64) -> Result<Rule, AppError> {
    Rule::restore(
        snapshot.id(),
        snapshot.world_id(),
        snapshot.kind(),
        snapshot.statement_md(),
        snapshot.scope(),
        snapshot.severity(),
        snapshot.source().map(str::to_owned),
        snapshot.validator_kind(),
        snapshot.parameters_json().as_str().to_owned(),
        version,
        snapshot.created_at_ms(),
        now_ms,
    )
    .map_err(Into::into)
}

fn restored_claim(snapshot: &Claim, version: u64) -> Result<Claim, AppError> {
    Claim::restore(
        snapshot.id(),
        snapshot.world_id(),
        snapshot.subject_entity_id(),
        snapshot.content_md(),
        snapshot.predicate_key().map(str::to_owned),
        snapshot.object().cloned(),
        snapshot.polarity(),
        snapshot.authentication(),
        snapshot.holder_entity_id(),
        snapshot.modality(),
        snapshot.register().map(str::to_owned),
        snapshot.epistemic_basis().map(str::to_owned),
        snapshot.source().map(str::to_owned),
        snapshot.source_document_id(),
        snapshot.source_claim_id(),
        snapshot.holder_confidence(),
        snapshot.period(),
        snapshot.registered_revision_id(),
        snapshot.superseded_revision_id(),
        version,
    )
    .map_err(Into::into)
}

fn restored_document(
    snapshot: &DocumentAggregate,
    version: u64,
    now_ms: i64,
) -> Result<DocumentAggregate, AppError> {
    Document::restore(
        snapshot.object().id(),
        snapshot.object().world_id(),
        snapshot.object().title(),
        snapshot.object().kind(),
        snapshot.object().author_entity_id(),
        snapshot.object().perspective_entity_id(),
        snapshot.object().canon_status(),
        snapshot.object().body_md(),
        version,
        snapshot.object().created_at_ms(),
        now_ms,
    )
    .map(|document| DocumentAggregate::new(document, snapshot.references().to_vec()))
    .map_err(Into::into)
}

fn entity_before(audit: &OperationAudit) -> Result<&Entity, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Entity(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous entity state",
        )),
    }
}

fn entity_after(audit: &OperationAudit) -> Result<&Entity, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Entity(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the current entity state",
        )),
    }
}

fn relation_before(audit: &OperationAudit) -> Result<&Relation, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Relation(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous relation state",
        )),
    }
}

fn relation_after(audit: &OperationAudit) -> Result<&Relation, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Relation(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the current relation state",
        )),
    }
}

fn event_before(audit: &OperationAudit) -> Result<&EventAggregate, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Event(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous event state",
        )),
    }
}

fn event_after(audit: &OperationAudit) -> Result<&EventAggregate, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Event(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the current event state",
        )),
    }
}

fn goal_before(audit: &OperationAudit) -> Result<&Goal, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Goal(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous goal state",
        )),
    }
}

fn goal_after(audit: &OperationAudit) -> Result<&Goal, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Goal(value)) => Ok(value),
        _ => Err(invalid_undo("undo audit is missing the current goal state")),
    }
}

fn rule_before(audit: &OperationAudit) -> Result<&Rule, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Rule(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous rule state",
        )),
    }
}

fn rule_after(audit: &OperationAudit) -> Result<&Rule, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Rule(value)) => Ok(value),
        _ => Err(invalid_undo("undo audit is missing the current rule state")),
    }
}

fn claim_before(audit: &OperationAudit) -> Result<&Claim, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Claim(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous claim state",
        )),
    }
}

fn claim_after(audit: &OperationAudit) -> Result<&Claim, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Claim(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the current claim state",
        )),
    }
}

fn document_before(audit: &OperationAudit) -> Result<&DocumentAggregate, AppError> {
    match audit.before() {
        Some(ChangeOperationValue::Document(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the previous document state",
        )),
    }
}

fn document_after(audit: &OperationAudit) -> Result<&DocumentAggregate, AppError> {
    match audit.after() {
        Some(ChangeOperationValue::Document(value)) => Ok(value),
        _ => Err(invalid_undo(
            "undo audit is missing the current document state",
        )),
    }
}

fn invalid_undo(details: impl Into<String>) -> AppError {
    AppError::Storage(StoreError::InvalidChangeSet(details.into()))
}

fn undo_delete_retcon(_original: RetconKind) -> RetconKind {
    RetconKind::Replacement
}

fn undo_restore_retcon(_original: RetconKind) -> RetconKind {
    RetconKind::Additive
}

fn undo_decision(operation: &ChangeOperation) -> Option<Result<DecisionPoint, AppError>> {
    if operation.retcon() != RetconKind::Replacement {
        return None;
    }

    Some(
        DecisionPoint::new_replacement(
            vec![operation.operation_id()],
            format!(
                "Should {} be removed to complete the undo?",
                operation.primary_ref()
            ),
            vec![
                "Keep current canon".to_owned(),
                "Undo latest logical commit".to_owned(),
            ],
            operation.primary_ref(),
            "Linear undo reverts the latest logical commit.".to_owned(),
            "Undo latest logical commit".to_owned(),
        )
        .map_err(Into::into),
    )
}

fn annotate_issue_list(issues: &mut [ValidationIssue], operations: &[ManualReviewOperation]) {
    for issue in issues {
        if issue
            .objects
            .iter()
            .any(|object| object.kind == "change_operation")
        {
            continue;
        }

        let matching_operation_ids: Vec<_> = operations
            .iter()
            .filter(|operation| operation.is_selected())
            .filter(|operation| issue_matches_operation_context(issue, operation.current()))
            .map(ManualReviewOperation::operation_id)
            .collect();

        for operation_id in matching_operation_ids.into_iter().rev() {
            issue
                .objects
                .insert(0, IssueObject::new("change_operation", operation_id));
        }
    }
}

fn annotate_issue_list_with_change_operations(
    issues: &mut [ValidationIssue],
    operations: &[ChangeOperation],
) {
    for issue in issues {
        if issue
            .objects
            .iter()
            .any(|object| object.kind == "change_operation")
        {
            continue;
        }

        let matching_operation_ids: Vec<_> = operations
            .iter()
            .filter(|operation| issue_matches_operation_context(issue, operation))
            .map(ChangeOperation::operation_id)
            .collect();

        for operation_id in matching_operation_ids.into_iter().rev() {
            issue
                .objects
                .insert(0, IssueObject::new("change_operation", operation_id));
        }
    }
}

fn issue_matches_operation_context(issue: &ValidationIssue, operation: &ChangeOperation) -> bool {
    issue
        .objects
        .iter()
        .any(|object| matches_object_ref(object, operation.primary_ref()))
        || operation.affected_ids().iter().any(|reference| {
            issue
                .objects
                .iter()
                .any(|object| matches_object_ref(object, *reference))
        })
}

fn matches_object_ref(object: &IssueObject, reference: ObjectRef) -> bool {
    object.kind == reference.kind()
        && (object.id == reference.to_string() || object.id == raw_object_id(reference))
}

fn raw_object_id(reference: ObjectRef) -> String {
    match reference {
        ObjectRef::World(id) => id.to_string(),
        ObjectRef::Entity(id) => id.to_string(),
        ObjectRef::Relation(id) => id.to_string(),
        ObjectRef::Event(id) => id.to_string(),
        ObjectRef::Claim(id) => id.to_string(),
        ObjectRef::Rule(id) => id.to_string(),
        ObjectRef::Goal(id) => id.to_string(),
        ObjectRef::Document(id) => id.to_string(),
    }
}

fn created_ref(operation: &ChangeOperation) -> Option<ObjectRef> {
    match operation {
        ChangeOperation::CreateEntity { after, .. } => Some(ObjectRef::Entity(after.id())),
        ChangeOperation::CreateRelation { after, .. } => Some(ObjectRef::Relation(after.id())),
        ChangeOperation::CreateEvent { after, .. } => Some(ObjectRef::Event(after.event().id())),
        ChangeOperation::CreateGoal { after, .. } => Some(ObjectRef::Goal(after.id())),
        ChangeOperation::CreateRule { after, .. } => Some(ObjectRef::Rule(after.id())),
        ChangeOperation::CreateClaim { after, .. } => Some(ObjectRef::Claim(after.id())),
        ChangeOperation::CreateDocument { after, .. } => {
            Some(ObjectRef::Document(after.object().id()))
        }
        _ => None,
    }
}

fn referenced_refs(operation: &ChangeOperation) -> Vec<ObjectRef> {
    match operation {
        ChangeOperation::CreateEntity { .. }
        | ChangeOperation::UpdateEntity { .. }
        | ChangeOperation::DeleteEntity { .. }
        | ChangeOperation::UpdateWorld { .. }
        | ChangeOperation::CreateRule { .. }
        | ChangeOperation::UpdateRule { .. }
        | ChangeOperation::DeleteRule { .. } => vec![],
        ChangeOperation::CreateRelation { after, .. } => relation_refs(after),
        ChangeOperation::UpdateRelation { after, .. } => relation_refs(after),
        ChangeOperation::DeleteRelation { before, .. } => relation_refs(before),
        ChangeOperation::CreateEvent { after, .. } => event_refs(after),
        ChangeOperation::UpdateEvent { after, .. } => event_refs(after),
        ChangeOperation::DeleteEvent { before, .. } => event_refs(before),
        ChangeOperation::CreateGoal { after, .. } => goal_refs(after),
        ChangeOperation::UpdateGoal { after, .. } => goal_refs(after),
        ChangeOperation::DeleteGoal { before, .. } => goal_refs(before),
        ChangeOperation::CreateClaim { after, .. } => claim_refs(after),
        ChangeOperation::UpdateClaim { after, .. } => claim_refs(after),
        ChangeOperation::DeleteClaim { before, .. } => claim_refs(before),
        ChangeOperation::CreateDocument { after, .. } => document_refs(after),
        ChangeOperation::UpdateDocument { after, .. } => document_refs(after),
        ChangeOperation::DeleteDocument { before, .. } => document_refs(before),
    }
}

fn relation_refs(relation: &Relation) -> Vec<ObjectRef> {
    vec![
        ObjectRef::Entity(relation.source_entity_id()),
        ObjectRef::Entity(relation.target_entity_id()),
    ]
}

fn goal_refs(goal: &Goal) -> Vec<ObjectRef> {
    vec![ObjectRef::Entity(goal.holder_entity_id())]
}

fn event_refs(event: &EventAggregate) -> Vec<ObjectRef> {
    let mut refs = Vec::new();
    if let Some(location_id) = event.event().location_entity_id() {
        refs.push(ObjectRef::Entity(location_id));
    }
    refs.extend(
        event
            .event()
            .participants()
            .iter()
            .map(|participant| ObjectRef::Entity(participant.entity_id())),
    );
    refs.extend(
        event
            .event()
            .affected_goal_ids()
            .iter()
            .copied()
            .map(ObjectRef::Goal),
    );
    refs.extend(
        event
            .links()
            .iter()
            .map(|link| ObjectRef::Event(link.target_event_id())),
    );
    refs
}

fn claim_refs(claim: &Claim) -> Vec<ObjectRef> {
    let mut refs = vec![ObjectRef::Entity(claim.subject_entity_id())];
    if let Some(holder_id) = claim.holder_entity_id() {
        refs.push(ObjectRef::Entity(holder_id));
    }
    if let Some(ClaimObject::Entity(entity_id)) = claim.object() {
        refs.push(ObjectRef::Entity(*entity_id));
    }
    if let Some(document_id) = claim.source_document_id() {
        refs.push(ObjectRef::Document(document_id));
    }
    if let Some(claim_id) = claim.source_claim_id() {
        refs.push(ObjectRef::Claim(claim_id));
    }
    refs
}

fn document_refs(document: &DocumentAggregate) -> Vec<ObjectRef> {
    let document_object = document.object();
    let mut refs = Vec::new();
    if let Some(author_id) = document_object.author_entity_id() {
        refs.push(ObjectRef::Entity(author_id));
    }
    if let Some(perspective_id) = document_object.perspective_entity_id() {
        refs.push(ObjectRef::Entity(perspective_id));
    }
    refs.extend(
        document
            .references()
            .iter()
            .map(|reference| reference.target()),
    );
    refs
}

fn dedupe_refs(primary: ObjectRef, refs: impl IntoIterator<Item = ObjectRef>) -> Vec<ObjectRef> {
    let mut deduped = vec![primary];
    let mut seen = HashSet::from([primary]);
    for reference in refs {
        if seen.insert(reference) {
            deduped.push(reference);
        }
    }
    deduped
}
