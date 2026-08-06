fn validate_operation(
    index: usize,
    world_id: WorldId,
    operation: &ChangeOperation,
    state: &ValidationState,
    future_creations: &HashMap<ObjectRef, usize>,
    written_objects: &mut HashMap<ObjectRef, ChangeOperationId>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let operation_object = IssueObject::new("change_operation", operation.operation_id());
    let primary_ref = operation.primary_ref();
    let primary_object = issue_object(primary_ref);

    if let Some(previous_operation_id) = written_objects.get(&primary_ref) {
        issues.push(ValidationIssue::new(
            "change_set.operation.double_write",
            ValidationSeverity::Conflict,
            vec![
                IssueObject::new("change_operation", previous_operation_id),
                operation_object.clone(),
                primary_object.clone(),
            ],
            "multiple operations write the same object",
        ));
    } else {
        written_objects.insert(primary_ref, operation.operation_id());
    }

    issues.extend(validate_operation_metadata(
        operation,
        &operation_object,
        &primary_object,
    ));

    match operation {
        ChangeOperation::UpdateWorld { before, after, .. } => {
            if before.id() != world_id || after.id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if before.id() != after.id() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_identity_changed",
                    ValidationSeverity::Error,
                    "update operations must preserve the aggregate id",
                ));
            }
            if operation.expected_version() != 0 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_expected_version",
                    ValidationSeverity::Error,
                    "world updates do not use numeric versions",
                ));
            }
            if before.current_revision() != after.current_revision() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_revision_changed",
                    ValidationSeverity::Error,
                    "world updates must preserve the base revision inside the operation payload",
                ));
            }
        }
        ChangeOperation::CreateEntity { after, .. } => {
            if after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if operation.expected_version() != 0 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_expected_version",
                    ValidationSeverity::Error,
                    "create operations must expect version 0",
                ));
            }
            if after.version() != 1 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_initial_version",
                    ValidationSeverity::Error,
                    "created aggregates must start at version 1",
                ));
            }
            if state.contains(primary_ref) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_exists",
                    ValidationSeverity::Error,
                    "create operation targets an object that already exists",
                ));
            }
            if state.has_entity_slug(after.world_id(), after.slug(), Some(after.id())) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.entity.duplicate_slug",
                    ValidationSeverity::Error,
                    "entity slug must remain unique within its world",
                ));
            }
        }
        ChangeOperation::UpdateEntity { before, after, .. } => {
            if before.world_id() != world_id || after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if before.id() != after.id() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_identity_changed",
                    ValidationSeverity::Error,
                    "update operations must preserve the aggregate id",
                ));
            }
            if operation.expected_version() != before.version() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_expected_version",
                    ValidationSeverity::Error,
                    "update operations must expect the current version",
                ));
            }
            match state.version(primary_ref) {
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_missing",
                    ValidationSeverity::Error,
                    "update operation targets an object that does not exist",
                )),
                Some(actual_version) => {
                    if let Some(issue) = validate_expected_version(
                        primary_object.clone(),
                        actual_version,
                        operation.expected_version(),
                    ) {
                        issues.push(with_operation(issue, operation.operation_id()));
                    }
                }
            }
            match before.version().checked_add(1) {
                Some(next_version) if after.version() != next_version => {
                    issues.push(operation_issue(
                        operation,
                        primary_ref,
                        "change_set.operation.update_version_increment",
                        ValidationSeverity::Error,
                        "updated aggregates must increment version by one",
                    ));
                }
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_version_increment",
                    ValidationSeverity::Error,
                    "updated aggregate version overflowed",
                )),
                Some(_) => {}
            }
            if state.has_entity_slug(after.world_id(), after.slug(), Some(after.id())) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.entity.duplicate_slug",
                    ValidationSeverity::Error,
                    "entity slug must remain unique within its world",
                ));
            }
        }
        ChangeOperation::DeleteEntity { before, .. } => {
            if before.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if operation.expected_version() != before.version() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.delete_expected_version",
                    ValidationSeverity::Error,
                    "delete operations must expect the current version",
                ));
            }
            match state.version(primary_ref) {
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_missing",
                    ValidationSeverity::Error,
                    "delete operation targets an object that does not exist",
                )),
                Some(actual_version) => {
                    if let Some(issue) = validate_expected_version(
                        primary_object.clone(),
                        actual_version,
                        operation.expected_version(),
                    ) {
                        issues.push(with_operation(issue, operation.operation_id()));
                    }
                    let dependents = state.dependents(primary_ref);
                    if !dependents.is_empty() {
                        let mut objects = vec![operation_object.clone(), primary_object.clone()];
                        objects.extend(dependents);
                        issues.push(ValidationIssue::new(
                            "change_set.delete_orphan",
                            ValidationSeverity::Error,
                            objects,
                            "delete operation would leave orphaned references",
                        ));
                    }
                }
            }
        }
        ChangeOperation::CreateRelation { after, .. } => {
            if after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if operation.expected_version() != 0 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_expected_version",
                    ValidationSeverity::Error,
                    "create operations must expect version 0",
                ));
            }
            if after.version() != 1 {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.create_initial_version",
                    ValidationSeverity::Error,
                    "created aggregates must start at version 1",
                ));
            }
            if state.contains(primary_ref) {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_exists",
                    ValidationSeverity::Error,
                    "create operation targets an object that already exists",
                ));
            }
            issues.extend(validate_relation_references(
                index,
                operation,
                state,
                future_creations,
                after,
            ));
        }
        ChangeOperation::UpdateRelation { before, after, .. } => {
            if before.world_id() != world_id || after.world_id() != world_id {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.world_mismatch",
                    ValidationSeverity::Error,
                    "operation world does not match change set world",
                ));
            }
            if before.id() != after.id() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_identity_changed",
                    ValidationSeverity::Error,
                    "update operations must preserve the aggregate id",
                ));
            }
            if operation.expected_version() != before.version() {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_expected_version",
                    ValidationSeverity::Error,
                    "update operations must expect the current version",
                ));
            }
            match state.version(primary_ref) {
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.target_missing",
                    ValidationSeverity::Error,
                    "update operation targets an object that does not exist",
                )),
                Some(actual_version) => {
                    if let Some(issue) = validate_expected_version(
                        primary_object.clone(),
                        actual_version,
                        operation.expected_version(),
                    ) {
                        issues.push(with_operation(issue, operation.operation_id()));
                    }
                }
            }
            match before.version().checked_add(1) {
                Some(next_version) if after.version() != next_version => {
                    issues.push(operation_issue(
                        operation,
                        primary_ref,
                        "change_set.operation.update_version_increment",
                        ValidationSeverity::Error,
                        "updated aggregates must increment version by one",
                    ));
                }
                None => issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "change_set.operation.update_version_increment",
                    ValidationSeverity::Error,
                    "updated aggregate version overflowed",
                )),
                Some(_) => {}
            }
            issues.extend(validate_relation_references(
                index,
                operation,
                state,
                future_creations,
                after,
            ));
        }
        ChangeOperation::DeleteRelation { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateEvent { after, .. } => {
            let event = after.event();
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                event.world_id(),
                event.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_events(
                    std::slice::from_ref(event),
                    &state.entity_values(),
                    &state.goal_values(),
                ),
            ));
            let mut scoped_events = state.event_values();
            scoped_events.push(event.clone());
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_event_links(after.links(), &scoped_events),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_event_objects(after),
            );
        }
        ChangeOperation::UpdateEvent { before, after, .. } => {
            let before_event = before.event();
            let after_event = after.event();
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before_event.id() == after_event.id(),
                before_event.world_id(),
                after_event.world_id(),
                before_event.version(),
                after_event.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_events(
                    std::slice::from_ref(after_event),
                    &state.entity_values(),
                    &state.goal_values(),
                ),
            ));
            let mut scoped_events = state.event_values();
            scoped_events.retain(|event| event.id() != before_event.id());
            scoped_events.push(after_event.clone());
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_event_links(after.links(), &scoped_events),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_event_objects(after),
            );
        }
        ChangeOperation::DeleteEvent { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.event().world_id(),
                before.event().version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateGoal { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.world_id(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_goals(std::slice::from_ref(after), &state.entity_values()),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                vec![ObjectRef::Entity(after.holder_entity_id())],
            );
        }
        ChangeOperation::UpdateGoal { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.id() == after.id(),
                before.world_id(),
                after.world_id(),
                before.version(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_goals(std::slice::from_ref(after), &state.entity_values()),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                vec![ObjectRef::Entity(after.holder_entity_id())],
            );
        }
        ChangeOperation::DeleteGoal { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateRule { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.world_id(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_rules(std::slice::from_ref(after)),
            ));
        }
        ChangeOperation::UpdateRule { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.id() == after.id(),
                before.world_id(),
                after.world_id(),
                before.version(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_rules(std::slice::from_ref(after)),
            ));
        }
        ChangeOperation::DeleteRule { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateClaim { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.world_id(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_claims(
                    std::slice::from_ref(after),
                    &state.entity_values(),
                    &state.document_values(),
                    &state.revisions,
                ),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_claim_objects(after),
            );
        }
        ChangeOperation::UpdateClaim { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.id() == after.id(),
                before.world_id(),
                after.world_id(),
                before.version(),
                after.version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_claims(
                    std::slice::from_ref(after),
                    &state.entity_values(),
                    &state.document_values(),
                    &state.revisions,
                ),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_claim_objects(after),
            );
        }
        ChangeOperation::DeleteClaim { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.world_id(),
                before.version(),
                world_id,
                state,
            );
        }
        ChangeOperation::CreateDocument { after, .. } => {
            validate_create_target(
                &mut issues,
                operation,
                primary_ref,
                after.object().world_id(),
                after.object().version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_documents(std::slice::from_ref(after.object()), &state.entity_values()),
            ));
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_document_references(after, state),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_document_objects(after),
            );
        }
        ChangeOperation::UpdateDocument { before, after, .. } => {
            validate_update_target(
                &mut issues,
                operation,
                primary_ref,
                before.object().id() == after.object().id(),
                before.object().world_id(),
                after.object().world_id(),
                before.object().version(),
                after.object().version(),
                world_id,
                state,
            );
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_documents(std::slice::from_ref(after.object()), &state.entity_values()),
            ));
            issues.extend(with_operation_issues(
                operation.operation_id(),
                validate_document_references(after, state),
            ));
            report_future_dependencies(
                &mut issues,
                operation,
                index,
                state,
                future_creations,
                referenced_document_objects(after),
            );
        }
        ChangeOperation::DeleteDocument { before, .. } => {
            validate_delete_target(
                &mut issues,
                operation,
                primary_ref,
                before.object().world_id(),
                before.object().version(),
                world_id,
                state,
            );
        }
    }

    issues
}

fn validate_operation_metadata(
    operation: &ChangeOperation,
    operation_object: &IssueObject,
    primary_object: &IssueObject,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if operation.affected_ids().is_empty() {
        issues.push(ValidationIssue::new(
            "change_set.operation.affected_empty",
            ValidationSeverity::Error,
            vec![operation_object.clone(), primary_object.clone()],
            "operation must affect at least one object",
        ));
    }
    if operation.affected_ids().len() > MAX_OPERATION_AFFECTED_IDS {
        issues.push(ValidationIssue::new(
            "change_set.operation.affected_too_many",
            ValidationSeverity::Error,
            vec![operation_object.clone(), primary_object.clone()],
            "operation affects too many objects",
        ));
    }
    if !operation.affected_ids().contains(&operation.primary_ref()) {
        issues.push(ValidationIssue::new(
            "change_set.operation.primary_missing",
            ValidationSeverity::Error,
            vec![operation_object.clone(), primary_object.clone()],
            "operation must include its primary object id in affected_ids",
        ));
    }

    let mut seen = HashSet::with_capacity(operation.affected_ids().len());
    for affected_id in operation.affected_ids() {
        if !seen.insert(*affected_id) {
            issues.push(ValidationIssue::new(
                "change_set.operation.affected_duplicate",
                ValidationSeverity::Error,
                vec![operation_object.clone(), issue_object(*affected_id)],
                "operation repeats an affected object id",
            ));
        }
    }

    match operation.retcon() {
        RetconKind::Additive if is_delete_operation(operation) => {
            issues.push(ValidationIssue::new(
                "change_set.retcon.additive_delete",
                ValidationSeverity::Error,
                vec![operation_object.clone(), primary_object.clone()],
                "additive retcons cannot delete canon",
            ));
        }
        RetconKind::Reinterpretive if is_delete_operation(operation) => {
            issues.push(ValidationIssue::new(
                "change_set.retcon.reinterpretive_delete",
                ValidationSeverity::Error,
                vec![operation_object.clone(), primary_object.clone()],
                "reinterpretive retcons must preserve prior canon",
            ));
        }
        RetconKind::Replacement => {}
        _ => {}
    }

    issues
}

