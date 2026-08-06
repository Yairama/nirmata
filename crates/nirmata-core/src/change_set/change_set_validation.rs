fn validate_change_set_parts(
    world_id: WorldId,
    base_revision: RevisionId,
    objective: &str,
    sources: &[ObjectRef],
    assumptions: &[String],
    operations: &[ChangeOperation],
    decisions: &[DecisionPoint],
    snapshot: &ChangeSetValidationSnapshot<'_>,
) -> ValidationReport {
    let mut report = ValidationReport::new();

    validate_non_empty_field(
        objective,
        "change_set.objective_empty",
        "change set objective cannot be empty",
        &mut report,
    );
    validate_max_chars(
        objective,
        MAX_CHANGE_SET_OBJECTIVE_CHARS,
        "change_set.objective_too_long",
        "change set objective exceeds the allowed size",
        &mut report,
    );
    validate_max_items(
        sources.len(),
        MAX_CHANGE_SET_SOURCES,
        "change_set.sources_too_many",
        "change set includes too many source references",
        &mut report,
    );
    validate_max_items(
        assumptions.len(),
        MAX_CHANGE_SET_ASSUMPTIONS,
        "change_set.assumptions_too_many",
        "change set includes too many assumptions",
        &mut report,
    );
    if operations.is_empty() {
        report.push(ValidationIssue::new(
            "change_set.operations_empty",
            ValidationSeverity::Error,
            vec![],
            "change set must include at least one operation",
        ));
    }
    validate_max_items(
        operations.len(),
        MAX_CHANGE_SET_OPERATIONS,
        "change_set.operations_too_many",
        "change set includes too many operations",
        &mut report,
    );
    validate_max_items(
        decisions.len(),
        MAX_CHANGE_SET_DECISIONS,
        "change_set.decisions_too_many",
        "change set includes too many decision points",
        &mut report,
    );

    let mut seen_sources = HashSet::with_capacity(sources.len());
    for source in sources {
        if !seen_sources.insert(*source) {
            report.push(ValidationIssue::new(
                "change_set.source_duplicate",
                ValidationSeverity::Error,
                vec![issue_object(*source)],
                "change set sources cannot repeat the same object",
            ));
        }
    }

    let mut seen_assumptions = HashSet::with_capacity(assumptions.len());
    for assumption in assumptions {
        validate_non_empty_field(
            assumption,
            "change_set.assumption_empty",
            "assumptions cannot be empty",
            &mut report,
        );
        validate_max_chars(
            assumption,
            MAX_CHANGE_SET_ASSUMPTION_CHARS,
            "change_set.assumption_too_long",
            "assumption exceeds the allowed size",
            &mut report,
        );
        if !seen_assumptions.insert(assumption.trim()) {
            report.push(ValidationIssue::new(
                "change_set.assumption_duplicate",
                ValidationSeverity::Error,
                vec![],
                "change set assumptions cannot repeat the same value",
            ));
        }
    }

    let mut operation_ids = HashSet::with_capacity(operations.len());
    let mut replacement_operations = HashSet::new();
    for operation in operations {
        if !operation_ids.insert(operation.operation_id()) {
            report.push(ValidationIssue::new(
                "change_set.operation_id_duplicate",
                ValidationSeverity::Error,
                vec![IssueObject::new(
                    "change_operation",
                    operation.operation_id(),
                )],
                "change set repeats an operation id",
            ));
        }
        if operation.retcon() == RetconKind::Replacement {
            replacement_operations.insert(operation.operation_id());
        }
    }

    let mut state = ValidationState::from_snapshot(snapshot, world_id, base_revision);
    let mut decision_ids = HashSet::with_capacity(decisions.len());
    let mut replacement_decisions = HashSet::new();
    let mut resolved_replacement_decisions = HashSet::new();
    for decision in decisions {
        let decision_object = IssueObject::new("decision_point", decision.decision_point_id());
        let touches_replacement = decision
            .operation_ids()
            .iter()
            .any(|operation_id| replacement_operations.contains(operation_id));
        if !decision_ids.insert(decision.decision_point_id()) {
            report.push(ValidationIssue::new(
                "change_set.decision_id_duplicate",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "change set repeats a decision point id",
            ));
        }

        validate_non_empty_field(
            decision.prompt(),
            "change_set.decision_prompt_empty",
            "decision prompt cannot be empty",
            &mut report,
        );
        validate_max_chars(
            decision.prompt(),
            MAX_DECISION_PROMPT_CHARS,
            "change_set.decision_prompt_too_long",
            "decision prompt exceeds the allowed size",
            &mut report,
        );
        if decision.operation_ids().is_empty() {
            report.push(ValidationIssue::new(
                "change_set.decision_operations_empty",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "decision point must reference at least one operation",
            ));
        }
        if decision.alternatives().len() < 2 {
            report.push(ValidationIssue::new(
                "change_set.decision_alternatives_too_few",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "decision point must expose at least two alternatives",
            ));
        }
        validate_max_items(
            decision.alternatives().len(),
            MAX_DECISION_ALTERNATIVES,
            "change_set.decision_alternatives_too_many",
            "decision point exposes too many alternatives",
            &mut report,
        );

        let mut seen_operation_ids = HashSet::with_capacity(decision.operation_ids().len());
        for operation_id in decision.operation_ids() {
            if !operation_ids.contains(operation_id) {
                report.push(ValidationIssue::new(
                    "change_set.decision_operation_missing",
                    ValidationSeverity::Error,
                    vec![
                        decision_object.clone(),
                        IssueObject::new("change_operation", operation_id),
                    ],
                    "decision point references an unknown operation",
                ));
            }
            if !seen_operation_ids.insert(*operation_id) {
                report.push(ValidationIssue::new(
                    "change_set.decision_operation_duplicate",
                    ValidationSeverity::Error,
                    vec![
                        decision_object.clone(),
                        IssueObject::new("change_operation", operation_id),
                    ],
                    "decision point repeats the same operation id",
                ));
            }
            if replacement_operations.contains(operation_id) {
                replacement_decisions.insert(*operation_id);
                if decision.resolved_alternative().is_some() {
                    resolved_replacement_decisions.insert(*operation_id);
                }
            }
        }

        let mut seen_alternatives = HashSet::with_capacity(decision.alternatives().len());
        for alternative in decision.alternatives() {
            validate_non_empty_field(
                alternative,
                "change_set.decision_alternative_empty",
                "decision alternative cannot be empty",
                &mut report,
            );
            validate_max_chars(
                alternative,
                MAX_DECISION_ALTERNATIVE_CHARS,
                "change_set.decision_alternative_too_long",
                "decision alternative exceeds the allowed size",
                &mut report,
            );
            if !seen_alternatives.insert(alternative.trim()) {
                report.push(ValidationIssue::new(
                    "change_set.decision_alternative_duplicate",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "decision point cannot repeat the same alternative",
                ));
            }
        }

        if decision.resolved_alternative().is_some_and(|alternative| {
            !decision
                .alternatives()
                .iter()
                .any(|value| value == alternative)
        }) {
            report.push(ValidationIssue::new(
                "change_set.decision_resolution_unknown",
                ValidationSeverity::Error,
                vec![decision_object.clone()],
                "resolved alternative must match one of the decision alternatives",
            ));
        }

        if touches_replacement {
            match decision.replacement_target() {
                None => report.push(ValidationIssue::new(
                    "change_set.replacement_target_missing",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "replacement decisions must identify the canon they replace",
                )),
                Some(target) => match state.object_world(target) {
                    None => report.push(ValidationIssue::new(
                        "change_set.replacement_target_unknown",
                        ValidationSeverity::Error,
                        vec![decision_object.clone(), issue_object(target)],
                        "replacement target must exist in the validated snapshot",
                    )),
                    Some(target_world_id) if target_world_id != world_id => {
                        report.push(ValidationIssue::new(
                            "change_set.replacement_target_cross_world",
                            ValidationSeverity::Error,
                            vec![decision_object.clone(), issue_object(target)],
                            "replacement target belongs to another world",
                        ))
                    }
                    Some(_) => {}
                },
            }

            match decision.reason() {
                None => report.push(ValidationIssue::new(
                    "change_set.replacement_reason_missing",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "replacement decisions must include a reason",
                )),
                Some(reason) if reason.chars().count() > MAX_REPLACEMENT_REASON_CHARS => report
                    .push(ValidationIssue::new(
                        "change_set.replacement_reason_too_long",
                        ValidationSeverity::Error,
                        vec![decision_object.clone()],
                        "replacement reason exceeds the allowed size",
                    )),
                Some(_) => {}
            }

            if decision.resolved_alternative().is_none() {
                report.push(ValidationIssue::new(
                    "change_set.replacement_decision_unresolved",
                    ValidationSeverity::Error,
                    vec![decision_object.clone()],
                    "replacement decisions must be resolved before validation succeeds",
                ));
            }
        }
    }

    for operation_id in replacement_operations {
        if !replacement_decisions.contains(&operation_id) {
            report.push(ValidationIssue::new(
                "change_set.replacement_decision_missing",
                ValidationSeverity::Error,
                vec![IssueObject::new("change_operation", operation_id)],
                "replacement operations require a decision point",
            ));
        } else if !resolved_replacement_decisions.contains(&operation_id) {
            report.push(ValidationIssue::new(
                "change_set.replacement_decision_unresolved",
                ValidationSeverity::Error,
                vec![IssueObject::new("change_operation", operation_id)],
                "replacement operations require a resolved decision point",
            ));
        }
    }

    let future_creations = collect_future_creations(operations);
    let mut written_objects = HashMap::with_capacity(operations.len());
    let mut resulting_state_scope = ResultingStateValidationScope::default();

    for (index, operation) in operations.iter().enumerate() {
        let mut issues = validate_operation(
            index,
            world_id,
            operation,
            &state,
            &future_creations,
            &mut written_objects,
        );
        let can_apply = issues.iter().all(|issue| {
            !matches!(
                issue.severity,
                ValidationSeverity::Error | ValidationSeverity::Conflict
            )
        });
        report.extend(issues.drain(..));
        if can_apply {
            resulting_state_scope.observe(operation);
            state.apply(operation);
        }
    }

    report.extend(validate_resulting_state(&state, &resulting_state_scope));

    for source in sources {
        if !state.contains(*source) {
            report.push(ValidationIssue::new(
                "change_set.source_missing",
                ValidationSeverity::Error,
                vec![issue_object(*source)],
                "source reference does not exist in the validated snapshot",
            ));
        }
    }

    report
}
