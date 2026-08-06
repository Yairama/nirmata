fn broken_dependency_issues(operations: &[ManualReviewOperation]) -> Vec<ValidationIssue> {
    let created_by_operation: HashMap<_, _> = operations
        .iter()
        .filter_map(|operation| {
            created_ref(operation.current()).map(|reference| (reference, operation))
        })
        .collect();
    let selected_ids: HashSet<_> = operations
        .iter()
        .filter(|operation| operation.is_selected())
        .map(ManualReviewOperation::operation_id)
        .collect();
    let mut seen = HashSet::new();
    let mut issues = Vec::new();

    for operation in operations
        .iter()
        .filter(|operation| operation.is_selected())
    {
        for dependency in referenced_refs(operation.current()) {
            let Some(required_operation) = created_by_operation.get(&dependency) else {
                continue;
            };
            if selected_ids.contains(&required_operation.operation_id())
                || required_operation.operation_id() == operation.operation_id()
            {
                continue;
            }

            let key = (
                operation.operation_id(),
                required_operation.operation_id(),
                dependency,
            );
            if !seen.insert(key) {
                continue;
            }

            issues.push(ValidationIssue {
                code: "manual_review.dependency_broken".to_owned(),
                severity: ValidationSeverity::Error,
                objects: vec![
                    IssueObject::new("change_operation", operation.operation_id()),
                    IssueObject::new("change_operation", required_operation.operation_id()),
                    IssueObject::new(dependency.kind(), dependency.to_string()),
                ],
                message: "selected operation depends on another operation that was rejected"
                    .to_owned(),
            });
        }
    }

    issues
}

pub(crate) fn annotate_report_with_change_operations(
    report: &mut ValidationReport,
    operations: &[ChangeOperation],
) {
    annotate_issue_list_with_change_operations(&mut report.errors, operations);
    annotate_issue_list_with_change_operations(&mut report.conflicts, operations);
    annotate_issue_list_with_change_operations(&mut report.warnings, operations);
    annotate_issue_list_with_change_operations(&mut report.info, operations);
}

fn annotate_report_with_operations(
    report: &mut ValidationReport,
    operations: &[ManualReviewOperation],
) {
    annotate_issue_list(&mut report.errors, operations);
    annotate_issue_list(&mut report.conflicts, operations);
    annotate_issue_list(&mut report.warnings, operations);
    annotate_issue_list(&mut report.info, operations);
}

