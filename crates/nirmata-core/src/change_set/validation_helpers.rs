fn is_delete_operation(operation: &ChangeOperation) -> bool {
    matches!(
        operation,
        ChangeOperation::DeleteEntity { .. }
            | ChangeOperation::DeleteRelation { .. }
            | ChangeOperation::DeleteEvent { .. }
            | ChangeOperation::DeleteGoal { .. }
            | ChangeOperation::DeleteRule { .. }
            | ChangeOperation::DeleteClaim { .. }
            | ChangeOperation::DeleteDocument { .. }
    )
}

fn validate_create_target(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
    world_id: WorldId,
    state: &ValidationState,
) {
    if object_world_id != world_id {
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
    if object_version != 1 {
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
}

fn validate_update_target(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    same_identity: bool,
    before_world_id: WorldId,
    after_world_id: WorldId,
    before_version: u64,
    after_version: u64,
    world_id: WorldId,
    state: &ValidationState,
) {
    if before_world_id != world_id || after_world_id != world_id {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.world_mismatch",
            ValidationSeverity::Error,
            "operation world does not match change set world",
        ));
    }
    if !same_identity {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.update_identity_changed",
            ValidationSeverity::Error,
            "update operations must preserve the aggregate id",
        ));
    }
    if operation.expected_version() != before_version {
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
                issue_object(primary_ref),
                actual_version,
                operation.expected_version(),
            ) {
                issues.push(with_operation(issue, operation.operation_id()));
            }
        }
    }
    match before_version.checked_add(1) {
        Some(next_version) if after_version != next_version => {
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
}

fn validate_delete_target(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
    world_id: WorldId,
    state: &ValidationState,
) {
    if object_world_id != world_id {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "change_set.operation.world_mismatch",
            ValidationSeverity::Error,
            "operation world does not match change set world",
        ));
    }
    if operation.expected_version() != object_version {
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
                issue_object(primary_ref),
                actual_version,
                operation.expected_version(),
            ) {
                issues.push(with_operation(issue, operation.operation_id()));
            }
            let dependents = state.dependents(primary_ref);
            if !dependents.is_empty() {
                let mut objects = vec![
                    IssueObject::new("change_operation", operation.operation_id()),
                    issue_object(primary_ref),
                ];
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

fn validate_relation_references(
    index: usize,
    operation: &ChangeOperation,
    state: &ValidationState,
    future_creations: &HashMap<ObjectRef, usize>,
    relation: &Relation,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let primary_ref = operation.primary_ref();
    let source_ref = ObjectRef::Entity(relation.source_entity_id());
    let target_ref = ObjectRef::Entity(relation.target_entity_id());

    for (entity_ref, missing_code) in [
        (source_ref, "relation.source_missing"),
        (target_ref, "relation.target_missing"),
    ] {
        match state.object_world(entity_ref) {
            None => issues.push(operation_issue(
                operation,
                primary_ref,
                missing_code,
                ValidationSeverity::Error,
                "relation references an entity that does not exist",
            )),
            Some(reference_world_id) if reference_world_id != relation.world_id() => {
                issues.push(operation_issue(
                    operation,
                    primary_ref,
                    "reference.cross_world",
                    ValidationSeverity::Error,
                    "relation references an entity from another world",
                ));
            }
            Some(_) => {}
        }
    }

    if matches!(
        (relation.valid_from_tick(), relation.valid_to_tick()),
        (Some(start), Some(end)) if start > end
    ) {
        issues.push(operation_issue(
            operation,
            primary_ref,
            "period.inverted",
            ValidationSeverity::Error,
            "relation period starts after it ends",
        ));
    }

    report_future_dependencies(
        &mut issues,
        operation,
        index,
        state,
        future_creations,
        vec![source_ref, target_ref],
    );

    issues
}

fn report_future_dependencies(
    issues: &mut Vec<ValidationIssue>,
    operation: &ChangeOperation,
    index: usize,
    state: &ValidationState,
    future_creations: &HashMap<ObjectRef, usize>,
    references: Vec<ObjectRef>,
) {
    for reference in references {
        if !state.contains(reference)
            && future_creations
                .get(&reference)
                .is_some_and(|future_index| *future_index > index)
        {
            issues.push(ValidationIssue::new(
                "change_set.dependency_order",
                ValidationSeverity::Error,
                vec![
                    IssueObject::new("change_operation", operation.operation_id()),
                    issue_object(operation.primary_ref()),
                    issue_object(reference),
                ],
                "referenced object must exist or be created earlier in the same change set",
            ));
        }
    }
}

fn referenced_event_objects(event: &EventAggregate) -> Vec<ObjectRef> {
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

fn referenced_claim_objects(claim: &Claim) -> Vec<ObjectRef> {
    let mut refs = vec![ObjectRef::Entity(claim.subject_entity_id())];
    if let Some(holder_id) = claim.holder_entity_id() {
        refs.push(ObjectRef::Entity(holder_id));
    }
    if let Some(crate::claim::ClaimObject::Entity(entity_id)) = claim.object() {
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

fn validate_document_references(
    aggregate: &DocumentAggregate,
    state: &ValidationState,
) -> Vec<ValidationIssue> {
    let document = aggregate.object();
    let source = ObjectRef::Document(document.id());
    let mut ordinals = HashSet::new();
    let mut issues = Vec::new();

    for reference in aggregate.references() {
        let objects = vec![issue_object(source), issue_object(reference.target())];
        if reference.source() != source {
            issues.push(ValidationIssue::new(
                "content_reference.source_invalid",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference source must match its document",
            ));
        }
        match state.object_world(reference.target()) {
            None => issues.push(ValidationIssue::new(
                "content_reference.target_missing",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference target does not exist",
            )),
            Some(target_world) if target_world != document.world_id() => {
                issues.push(ValidationIssue::new(
                    "reference.cross_world",
                    ValidationSeverity::Error,
                    objects.clone(),
                    "content reference crosses world boundaries",
                ));
            }
            Some(_) => {}
        }
        if !ordinals.insert(reference.ordinal()) {
            issues.push(ValidationIssue::new(
                "content_reference.ordinal_duplicate",
                ValidationSeverity::Error,
                objects,
                format!(
                    "content ordinal {} is duplicated for its source",
                    reference.ordinal()
                ),
            ));
        }
    }

    issues
}

fn referenced_document_objects(document: &DocumentAggregate) -> Vec<ObjectRef> {
    let document_object = document.object();
    let mut refs = Vec::new();
    if let Some(author_id) = document_object.author_entity_id() {
        refs.push(ObjectRef::Entity(author_id));
    }
    if let Some(perspective_id) = document_object.perspective_entity_id() {
        refs.push(ObjectRef::Entity(perspective_id));
    }
    refs.extend(document_refs_targets(document));
    refs
}

fn document_refs_targets(document: &DocumentAggregate) -> impl Iterator<Item = ObjectRef> + '_ {
    document
        .references()
        .iter()
        .map(|reference| reference.target())
}

fn collect_future_creations(operations: &[ChangeOperation]) -> HashMap<ObjectRef, usize> {
    let mut created = HashMap::new();
    for (index, operation) in operations.iter().enumerate() {
        if let Some(created_ref) = operation.created_ref() {
            created.entry(created_ref).or_insert(index);
        }
    }
    created
}

fn with_operation(mut issue: ValidationIssue, operation_id: ChangeOperationId) -> ValidationIssue {
    let operation_object = IssueObject::new("change_operation", operation_id);
    if !issue.objects.contains(&operation_object) {
        issue.objects.insert(0, operation_object);
    }
    issue
}

fn with_operation_issues(
    operation_id: ChangeOperationId,
    issues: impl IntoIterator<Item = ValidationIssue>,
) -> Vec<ValidationIssue> {
    issues
        .into_iter()
        .map(|issue| with_operation(issue, operation_id))
        .collect()
}

fn operation_issue(
    operation: &ChangeOperation,
    primary_ref: ObjectRef,
    code: &'static str,
    severity: ValidationSeverity,
    message: impl Into<String>,
) -> ValidationIssue {
    ValidationIssue::new(
        code,
        severity,
        vec![
            IssueObject::new("change_operation", operation.operation_id()),
            issue_object(primary_ref),
        ],
        message,
    )
}

fn issue_object(object: ObjectRef) -> IssueObject {
    IssueObject::new(object.kind(), object.to_string())
}

fn validate_non_empty_field(
    value: &str,
    code: &'static str,
    message: &'static str,
    report: &mut ValidationReport,
) {
    if value.trim().is_empty() {
        report.push(ValidationIssue::new(
            code,
            ValidationSeverity::Error,
            vec![],
            message,
        ));
    }
}

fn validate_max_chars(
    value: &str,
    max_chars: usize,
    code: &'static str,
    message: &'static str,
    report: &mut ValidationReport,
) {
    if value.chars().count() > max_chars {
        report.push(ValidationIssue::new(
            code,
            ValidationSeverity::Error,
            vec![],
            message,
        ));
    }
}

fn validate_max_items(
    actual: usize,
    max_items: usize,
    code: &'static str,
    message: &'static str,
    report: &mut ValidationReport,
) {
    if actual > max_items {
        report.push(ValidationIssue::new(
            code,
            ValidationSeverity::Error,
            vec![],
            message,
        ));
    }
}
