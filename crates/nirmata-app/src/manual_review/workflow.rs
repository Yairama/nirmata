pub(crate) fn create_undo_review(
    world_id: WorldId,
    base_revision: RevisionId,
    target_revision: &StoredRevision,
    committed: &CommittedChangeSetRecord,
    store: &WorldStore,
    now_ms: i64,
) -> Result<ManualReviewSession, AppError> {
    let audits = committed
        .audits()
        .iter()
        .map(|audit| (audit.operation_id(), audit))
        .collect::<HashMap<_, _>>();
    let draft_operations = committed
        .change_set()
        .operations()
        .iter()
        .rev()
        .map(|operation| {
            let audit = audits
                .get(&operation.operation_id())
                .copied()
                .ok_or_else(|| invalid_undo("committed change set is missing an audit record"))?;
            inverse_operation(operation, audit, store, now_ms)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let operations = draft_operations
        .into_iter()
        .map(|operation| operation.into_change_operation(ChangeOperationId::new()))
        .collect::<Vec<_>>();
    let decisions = operations
        .iter()
        .filter_map(undo_decision)
        .collect::<Result<Vec<_>, _>>()?;
    let original_draft = ChangeSetDraft::new(
        world_id,
        base_revision,
        format!(
            "Undo revision {}: {}",
            target_revision.id(),
            target_revision.summary()
        ),
        committed.change_set().sources().to_vec(),
        vec![],
        operations,
        decisions.clone(),
    )?;
    let reviewed_operations = original_draft
        .operations()
        .iter()
        .cloned()
        .map(|operation| ManualReviewOperation {
            original: operation.clone(),
            current: operation,
            decision: OperationDecision::Accept,
            judgment: Some("Undo requested from revision history.".to_owned()),
        })
        .collect();
    ManualReviewSession::rebuild(
        original_draft,
        reviewed_operations,
        decisions,
        vec![],
        store,
    )
}

fn rebuilt_draft(
    original_draft: &ChangeSetDraft,
    operations: &[ManualReviewOperation],
    decisions: &[DecisionPoint],
) -> Result<ChangeSetDraft, AppError> {
    let selected_operation_ids: HashSet<_> = operations
        .iter()
        .filter(|operation| operation.is_selected())
        .map(ManualReviewOperation::operation_id)
        .collect();
    let selected_operations = operations
        .iter()
        .filter(|operation| operation.is_selected())
        .map(|operation| operation.current.clone())
        .collect();

    let decisions = decisions
        .iter()
        .filter_map(|decision| {
            let operation_ids: Vec<_> = decision
                .operation_ids()
                .iter()
                .copied()
                .filter(|operation_id| selected_operation_ids.contains(operation_id))
                .collect();
            if operation_ids.is_empty() {
                return None;
            }

            Some(nirmata_core::change_set::DecisionPoint::restore(
                decision.decision_point_id(),
                operation_ids,
                decision.prompt().to_owned(),
                decision.alternatives().to_vec(),
                decision.replacement_target(),
                decision.reason().map(str::to_owned),
                decision.resolved_alternative().map(str::to_owned),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ChangeSetDraft::restore(
        original_draft.id(),
        original_draft.world_id(),
        original_draft.base_revision(),
        original_draft.objective().to_owned(),
        original_draft.sources().to_vec(),
        original_draft.assumptions().to_vec(),
        selected_operations,
        decisions,
    )?)
}

fn rebase_draft_revision(
    draft: &ChangeSetDraft,
    base_revision: RevisionId,
) -> Result<ChangeSetDraft, AppError> {
    Ok(ChangeSetDraft::restore(
        draft.id(),
        draft.world_id(),
        base_revision,
        draft.objective().to_owned(),
        draft.sources().to_vec(),
        draft.assumptions().to_vec(),
        draft.operations().to_vec(),
        draft.decisions().to_vec(),
    )?)
}

fn find_operation_mut(
    operations: &mut [ManualReviewOperation],
    operation_id: ChangeOperationId,
) -> Result<&mut ManualReviewOperation, AppError> {
    operations
        .iter_mut()
        .find(|operation| operation.operation_id() == operation_id)
        .ok_or(AppError::UnknownReviewOperation(operation_id))
}

fn find_issue<'a>(
    report: &'a ValidationReport,
    operation_id: ChangeOperationId,
    issue_code: &str,
) -> Option<&'a ValidationIssue> {
    report
        .errors
        .iter()
        .chain(report.conflicts.iter())
        .chain(report.warnings.iter())
        .chain(report.info.iter())
        .find(|issue| issue.code == issue_code && issue_has_operation(issue, operation_id))
}

fn retain_applicable_waivers(
    waivers: Vec<ChangeSetWaiver>,
    operations: &[ManualReviewOperation],
    report: &ValidationReport,
) -> Vec<ChangeSetWaiver> {
    let selected_operations: HashSet<_> = operations
        .iter()
        .filter(|operation| operation.is_selected())
        .map(ManualReviewOperation::operation_id)
        .collect();

    waivers
        .into_iter()
        .filter(|waiver| {
            selected_operations.contains(&waiver.operation_id())
                && find_issue(report, waiver.operation_id(), waiver.issue_code())
                    .is_some_and(|issue| issue.severity != ValidationSeverity::Error)
        })
        .collect()
}

fn apply_waivers(report: &ValidationReport, waivers: &[ChangeSetWaiver]) -> ValidationReport {
    ValidationReport {
        errors: report.errors.clone(),
        conflicts: report
            .conflicts
            .iter()
            .filter(|issue| !issue_is_waived(issue, waivers))
            .cloned()
            .collect(),
        warnings: report
            .warnings
            .iter()
            .filter(|issue| !issue_is_waived(issue, waivers))
            .cloned()
            .collect(),
        info: report
            .info
            .iter()
            .filter(|issue| !issue_is_waived(issue, waivers))
            .cloned()
            .collect(),
    }
}

fn issue_is_waived(issue: &ValidationIssue, waivers: &[ChangeSetWaiver]) -> bool {
    waivers.iter().any(|waiver| {
        waiver.issue_code() == issue.code && issue_has_operation(issue, waiver.operation_id())
    })
}

fn issue_has_operation(issue: &ValidationIssue, operation_id: ChangeOperationId) -> bool {
    let operation_id = operation_id.to_string();
    issue
        .objects
        .iter()
        .any(|object| object.kind == "change_operation" && object.id == operation_id)
}

fn parse_operation_id(value: &str) -> Result<ChangeOperationId, AppError> {
    ChangeOperationId::from_str(value).map_err(|_| {
        AppError::Storage(StoreError::InvalidChangeSet(format!(
            "invalid manual review operation id: {value}"
        )))
    })
}

fn operation_snapshot(
    review: &ManualReviewSession,
    operation: &ManualReviewOperation,
) -> ManualReviewOperationSnapshot {
    let issues = report_for_operation(&review.validation_report, operation.operation_id());
    let effective_issues = report_for_operation(&review.effective_report, operation.operation_id());
    let primary_ref = operation.current().primary_ref();
    let risk = operation_risk_snapshot(review, operation);
    let hide_suggested_resolution = risk.requires_judgment && is_blank_option(operation.judgment());
    ManualReviewOperationSnapshot {
        operation_id: operation.operation_id().to_string(),
        decision: decision_label(operation.decision()).to_owned(),
        selected: operation.is_selected(),
        severity: report_severity(&effective_issues),
        target_uri: primary_ref.to_string(),
        dependencies: operation
            .current()
            .affected_ids()
            .iter()
            .copied()
            .filter(|reference| *reference != primary_ref)
            .map(|reference| reference.to_string())
            .collect(),
        before: operation_object_snapshot_before(operation.current()),
        after: operation_object_snapshot_after(operation.current()),
        issues,
        effective_issues,
        waivers: review
            .waivers()
            .iter()
            .filter(|waiver| waiver.operation_id() == operation.operation_id())
            .map(|waiver| ManualReviewWaiverSnapshot {
                issue_code: waiver.issue_code().to_owned(),
                rationale: waiver.rationale().to_owned(),
                created_at_ms: waiver.created_at_ms(),
            })
            .collect(),
        decision_points: review
            .draft()
            .decisions()
            .iter()
            .filter(|decision| decision.operation_ids().contains(&operation.operation_id()))
            .map(|decision| ManualReviewDecisionPointSnapshot {
                decision_point_id: decision.decision_point_id().to_string(),
                prompt: decision.prompt().to_owned(),
                alternatives: decision.alternatives().to_vec(),
                replacement_target: decision
                    .replacement_target()
                    .map(|target| target.to_string()),
                suggestion_available: decision.reason().is_some()
                    || decision.resolved_alternative().is_some(),
                suggestion_hidden: hide_suggested_resolution
                    && (decision.reason().is_some() || decision.resolved_alternative().is_some()),
                reason: if hide_suggested_resolution {
                    None
                } else {
                    decision.reason().map(str::to_owned)
                },
                resolved_alternative: if hide_suggested_resolution {
                    None
                } else {
                    decision.resolved_alternative().map(str::to_owned)
                },
            })
            .collect(),
        risk,
    }
}

fn operation_risk_snapshot(
    review: &ManualReviewSession,
    operation: &ManualReviewOperation,
) -> ManualReviewRiskSnapshot {
    let triggers = operation_risk_triggers(review, operation);
    let suggested_resolution_available = review
        .draft()
        .decisions()
        .iter()
        .filter(|decision| decision.operation_ids().contains(&operation.operation_id()))
        .any(|decision| decision.reason().is_some() || decision.resolved_alternative().is_some());
    let requires_judgment = operation.is_selected() && !triggers.is_empty();
    let judgment = operation.judgment().map(str::to_owned);
    let suggested_resolution_hidden = requires_judgment && is_blank_option(judgment.as_deref());

    ManualReviewRiskSnapshot {
        requires_judgment,
        judgment,
        suggested_resolution_available,
        suggested_resolution_hidden: suggested_resolution_hidden && suggested_resolution_available,
        triggers,
    }
}

fn operation_requires_judgment(
    review: &ManualReviewSession,
    operation: &ManualReviewOperation,
) -> bool {
    operation_risk_snapshot(review, operation).requires_judgment
}

fn operation_risk_triggers(
    review: &ManualReviewSession,
    operation: &ManualReviewOperation,
) -> Vec<ManualReviewRiskTriggerSnapshot> {
    let issues = report_for_operation(&review.validation_report, operation.operation_id());
    let mut triggers = Vec::new();

    if operation.current().retcon() == RetconKind::Replacement {
        triggers.push(ManualReviewRiskTriggerSnapshot {
            code: "replacement".to_owned(),
            title: "Replacement".to_owned(),
            detail: "La operación reemplaza canon existente y requiere juicio humano antes de aplicar la recomendación.".to_owned(),
        });
    }

    if !issues.errors.is_empty() || !issues.conflicts.is_empty() {
        triggers.push(ManualReviewRiskTriggerSnapshot {
            code: "hard_conflict".to_owned(),
            title: "Conflicto duro".to_owned(),
            detail: first_issue_message(&issues),
        });
    }

    if operation.current().affected_ids().len() >= HIGH_IMPACT_AFFECTED_OBJECTS_THRESHOLD {
        triggers.push(ManualReviewRiskTriggerSnapshot {
            code: "broad_impact".to_owned(),
            title: "Impacto amplio".to_owned(),
            detail: format!(
                "La operación toca {} objetos relacionados en el snapshot afectado.",
                operation.current().affected_ids().len()
            ),
        });
    }

    triggers
}

fn report_for_operation(
    report: &ValidationReport,
    operation_id: ChangeOperationId,
) -> ValidationReport {
    ValidationReport {
        errors: report
            .errors
            .iter()
            .filter(|issue| issue_has_operation(issue, operation_id))
            .cloned()
            .collect(),
        conflicts: report
            .conflicts
            .iter()
            .filter(|issue| issue_has_operation(issue, operation_id))
            .cloned()
            .collect(),
        warnings: report
            .warnings
            .iter()
            .filter(|issue| issue_has_operation(issue, operation_id))
            .cloned()
            .collect(),
        info: report
            .info
            .iter()
            .filter(|issue| issue_has_operation(issue, operation_id))
            .cloned()
            .collect(),
    }
}

fn report_severity(report: &ValidationReport) -> ValidationSeverity {
    if !report.errors.is_empty() {
        ValidationSeverity::Error
    } else if !report.conflicts.is_empty() {
        ValidationSeverity::Conflict
    } else if !report.warnings.is_empty() {
        ValidationSeverity::Warning
    } else {
        ValidationSeverity::Info
    }
}

fn first_issue_message(report: &ValidationReport) -> String {
    report
        .errors
        .iter()
        .chain(report.conflicts.iter())
        .chain(report.warnings.iter())
        .chain(report.info.iter())
        .next()
        .map(|issue| issue.message.clone())
        .unwrap_or_else(|| "La revisión requiere otra validación.".to_owned())
}

fn parse_decision_point_id(value: &str) -> Result<DecisionPointId, AppError> {
    DecisionPointId::from_str(value).map_err(|_| {
        AppError::Storage(StoreError::InvalidChangeSet(format!(
            "invalid manual review decision point id: {value}"
        )))
    })
}
