#[derive(Clone, Debug, PartialEq)]
pub struct ManualReviewInput {
    pub objective: String,
    pub sources: Vec<ObjectRef>,
    pub assumptions: Vec<String>,
    pub operations: Vec<DraftOperationInput>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DraftOperationInput {
    UpdateWorld {
        retcon: RetconKind,
        before: World,
        after: World,
    },
    CreateEntity {
        retcon: RetconKind,
        after: Entity,
    },
    UpdateEntity {
        retcon: RetconKind,
        before: Entity,
        after: Entity,
    },
    DeleteEntity {
        retcon: RetconKind,
        before: Entity,
    },
    CreateRelation {
        retcon: RetconKind,
        after: Relation,
    },
    UpdateRelation {
        retcon: RetconKind,
        before: Relation,
        after: Relation,
    },
    DeleteRelation {
        retcon: RetconKind,
        before: Relation,
    },
    CreateEvent {
        retcon: RetconKind,
        after: EventAggregate,
    },
    UpdateEvent {
        retcon: RetconKind,
        before: EventAggregate,
        after: EventAggregate,
    },
    DeleteEvent {
        retcon: RetconKind,
        before: EventAggregate,
    },
    CreateGoal {
        retcon: RetconKind,
        after: Goal,
    },
    UpdateGoal {
        retcon: RetconKind,
        before: Goal,
        after: Goal,
    },
    DeleteGoal {
        retcon: RetconKind,
        before: Goal,
    },
    CreateRule {
        retcon: RetconKind,
        after: Rule,
    },
    UpdateRule {
        retcon: RetconKind,
        before: Rule,
        after: Rule,
    },
    DeleteRule {
        retcon: RetconKind,
        before: Rule,
    },
    CreateClaim {
        retcon: RetconKind,
        after: Claim,
    },
    UpdateClaim {
        retcon: RetconKind,
        before: Claim,
        after: Claim,
    },
    DeleteClaim {
        retcon: RetconKind,
        before: Claim,
    },
    CreateDocument {
        retcon: RetconKind,
        after: DocumentAggregate,
    },
    UpdateDocument {
        retcon: RetconKind,
        before: DocumentAggregate,
        after: DocumentAggregate,
    },
    DeleteDocument {
        retcon: RetconKind,
        before: DocumentAggregate,
    },
}

impl DraftOperationInput {
    pub fn with_retcon(self, retcon: RetconKind) -> Self {
        match self {
            Self::UpdateWorld { before, after, .. } => Self::UpdateWorld {
                retcon,
                before,
                after,
            },
            Self::CreateEntity { after, .. } => Self::CreateEntity { retcon, after },
            Self::UpdateEntity { before, after, .. } => Self::UpdateEntity {
                retcon,
                before,
                after,
            },
            Self::DeleteEntity { before, .. } => Self::DeleteEntity { retcon, before },
            Self::CreateRelation { after, .. } => Self::CreateRelation { retcon, after },
            Self::UpdateRelation { before, after, .. } => Self::UpdateRelation {
                retcon,
                before,
                after,
            },
            Self::DeleteRelation { before, .. } => Self::DeleteRelation { retcon, before },
            Self::CreateEvent { after, .. } => Self::CreateEvent { retcon, after },
            Self::UpdateEvent { before, after, .. } => Self::UpdateEvent {
                retcon,
                before,
                after,
            },
            Self::DeleteEvent { before, .. } => Self::DeleteEvent { retcon, before },
            Self::CreateGoal { after, .. } => Self::CreateGoal { retcon, after },
            Self::UpdateGoal { before, after, .. } => Self::UpdateGoal {
                retcon,
                before,
                after,
            },
            Self::DeleteGoal { before, .. } => Self::DeleteGoal { retcon, before },
            Self::CreateRule { after, .. } => Self::CreateRule { retcon, after },
            Self::UpdateRule { before, after, .. } => Self::UpdateRule {
                retcon,
                before,
                after,
            },
            Self::DeleteRule { before, .. } => Self::DeleteRule { retcon, before },
            Self::CreateClaim { after, .. } => Self::CreateClaim { retcon, after },
            Self::UpdateClaim { before, after, .. } => Self::UpdateClaim {
                retcon,
                before,
                after,
            },
            Self::DeleteClaim { before, .. } => Self::DeleteClaim { retcon, before },
            Self::CreateDocument { after, .. } => Self::CreateDocument { retcon, after },
            Self::UpdateDocument { before, after, .. } => Self::UpdateDocument {
                retcon,
                before,
                after,
            },
            Self::DeleteDocument { before, .. } => Self::DeleteDocument { retcon, before },
        }
    }

    fn into_change_operation(self, operation_id: ChangeOperationId) -> ChangeOperation {
        match self {
            Self::UpdateWorld {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateWorld {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::World(before.id()), []),
                expected_version: 0,
                retcon,
                before,
                after,
            },
            Self::CreateEntity { retcon, after } => ChangeOperation::CreateEntity {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Entity(after.id()), []),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateEntity {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateEntity {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Entity(before.id()), []),
                expected_version: before.version(),
                retcon,
                before,
                after,
            },
            Self::DeleteEntity { retcon, before } => ChangeOperation::DeleteEntity {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Entity(before.id()), []),
                expected_version: before.version(),
                retcon,
                before,
            },
            Self::CreateRelation { retcon, after } => ChangeOperation::CreateRelation {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Relation(after.id()), relation_refs(&after)),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateRelation {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateRelation {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Relation(before.id()),
                    relation_refs(&before)
                        .into_iter()
                        .chain(relation_refs(&after)),
                ),
                expected_version: before.version(),
                retcon,
                before,
                after,
            },
            Self::DeleteRelation { retcon, before } => ChangeOperation::DeleteRelation {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Relation(before.id()), relation_refs(&before)),
                expected_version: before.version(),
                retcon,
                before,
            },
            Self::CreateEvent { retcon, after } => ChangeOperation::CreateEvent {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Event(after.event().id()), event_refs(&after)),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateEvent {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateEvent {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Event(before.event().id()),
                    event_refs(&before).into_iter().chain(event_refs(&after)),
                ),
                expected_version: before.event().version(),
                retcon,
                before,
                after,
            },
            Self::DeleteEvent { retcon, before } => ChangeOperation::DeleteEvent {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Event(before.event().id()),
                    event_refs(&before),
                ),
                expected_version: before.event().version(),
                retcon,
                before,
            },
            Self::CreateGoal { retcon, after } => ChangeOperation::CreateGoal {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Goal(after.id()), goal_refs(&after)),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateGoal {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateGoal {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Goal(before.id()),
                    goal_refs(&before).into_iter().chain(goal_refs(&after)),
                ),
                expected_version: before.version(),
                retcon,
                before,
                after,
            },
            Self::DeleteGoal { retcon, before } => ChangeOperation::DeleteGoal {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Goal(before.id()), goal_refs(&before)),
                expected_version: before.version(),
                retcon,
                before,
            },
            Self::CreateRule { retcon, after } => ChangeOperation::CreateRule {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Rule(after.id()), []),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateRule {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateRule {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Rule(before.id()), []),
                expected_version: before.version(),
                retcon,
                before,
                after,
            },
            Self::DeleteRule { retcon, before } => ChangeOperation::DeleteRule {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Rule(before.id()), []),
                expected_version: before.version(),
                retcon,
                before,
            },
            Self::CreateClaim { retcon, after } => ChangeOperation::CreateClaim {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Claim(after.id()), claim_refs(&after)),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateClaim {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateClaim {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Claim(before.id()),
                    claim_refs(&before).into_iter().chain(claim_refs(&after)),
                ),
                expected_version: before.version(),
                retcon,
                before,
                after,
            },
            Self::DeleteClaim { retcon, before } => ChangeOperation::DeleteClaim {
                operation_id,
                affected_ids: dedupe_refs(ObjectRef::Claim(before.id()), claim_refs(&before)),
                expected_version: before.version(),
                retcon,
                before,
            },
            Self::CreateDocument { retcon, after } => ChangeOperation::CreateDocument {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Document(after.object().id()),
                    document_refs(&after),
                ),
                expected_version: 0,
                retcon,
                after,
            },
            Self::UpdateDocument {
                retcon,
                before,
                after,
            } => ChangeOperation::UpdateDocument {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Document(before.object().id()),
                    document_refs(&before)
                        .into_iter()
                        .chain(document_refs(&after)),
                ),
                expected_version: before.object().version(),
                retcon,
                before,
                after,
            },
            Self::DeleteDocument { retcon, before } => ChangeOperation::DeleteDocument {
                operation_id,
                affected_ids: dedupe_refs(
                    ObjectRef::Document(before.object().id()),
                    document_refs(&before),
                ),
                expected_version: before.object().version(),
                retcon,
                before,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ManualReviewAction {
    Accept {
        operation_id: ChangeOperationId,
    },
    Edit {
        operation_id: ChangeOperationId,
        replacement: DraftOperationInput,
    },
    RecordJudgment {
        operation_id: ChangeOperationId,
        judgment: String,
    },
    Reject {
        operation_id: ChangeOperationId,
    },
    AddWaiver {
        operation_id: ChangeOperationId,
        issue_code: String,
        rationale: String,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ManualReviewActionRequest {
    Accept {
        operation_id: String,
    },
    RecordJudgment {
        operation_id: String,
        judgment: String,
    },
    Reject {
        operation_id: String,
    },
    AddWaiver {
        operation_id: String,
        issue_code: String,
        rationale: String,
    },
}

impl ManualReviewActionRequest {
    pub fn into_action(self) -> Result<ManualReviewAction, AppError> {
        Ok(match self {
            Self::Accept { operation_id } => ManualReviewAction::Accept {
                operation_id: parse_operation_id(&operation_id)?,
            },
            Self::RecordJudgment {
                operation_id,
                judgment,
            } => ManualReviewAction::RecordJudgment {
                operation_id: parse_operation_id(&operation_id)?,
                judgment,
            },
            Self::Reject { operation_id } => ManualReviewAction::Reject {
                operation_id: parse_operation_id(&operation_id)?,
            },
            Self::AddWaiver {
                operation_id,
                issue_code,
                rationale,
            } => ManualReviewAction::AddWaiver {
                operation_id: parse_operation_id(&operation_id)?,
                issue_code,
                rationale,
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewLineItem {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewObjectSnapshot {
    pub title: String,
    pub object_type: String,
    pub target_uri: String,
    pub lines: Vec<ManualReviewLineItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewWaiverSnapshot {
    pub issue_code: String,
    pub rationale: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualReviewFreshnessStatus {
    Current,
    Stale,
    RefreshRestartRequired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewFreshnessSnapshot {
    pub status: ManualReviewFreshnessStatus,
    pub current_revision: String,
    pub can_revalidate: bool,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewRiskTriggerSnapshot {
    pub code: String,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewRiskSnapshot {
    pub requires_judgment: bool,
    pub judgment: Option<String>,
    pub suggested_resolution_available: bool,
    pub suggested_resolution_hidden: bool,
    pub triggers: Vec<ManualReviewRiskTriggerSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewDecisionPointSnapshot {
    pub decision_point_id: String,
    pub prompt: String,
    pub alternatives: Vec<String>,
    pub replacement_target: Option<String>,
    pub suggestion_available: bool,
    pub suggestion_hidden: bool,
    pub reason: Option<String>,
    pub resolved_alternative: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewOperationSnapshot {
    pub operation_id: String,
    pub decision: String,
    pub selected: bool,
    pub severity: ValidationSeverity,
    pub target_uri: String,
    pub dependencies: Vec<String>,
    pub before: Option<ManualReviewObjectSnapshot>,
    pub after: Option<ManualReviewObjectSnapshot>,
    pub issues: ValidationReport,
    pub effective_issues: ValidationReport,
    pub waivers: Vec<ManualReviewWaiverSnapshot>,
    pub decision_points: Vec<ManualReviewDecisionPointSnapshot>,
    pub risk: ManualReviewRiskSnapshot,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualReviewSnapshot {
    pub review_key: String,
    pub objective: String,
    pub sources: Vec<String>,
    pub assumptions: Vec<String>,
    pub base_revision: String,
    pub operations: Vec<ManualReviewOperationSnapshot>,
    pub validation_report: ValidationReport,
    pub effective_report: ValidationReport,
    pub ready_to_confirm: bool,
    pub freshness: ManualReviewFreshnessSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ManualReviewOperation {
    original: ChangeOperation,
    current: ChangeOperation,
    decision: OperationDecision,
    judgment: Option<String>,
}

impl ManualReviewOperation {
    pub fn operation_id(&self) -> ChangeOperationId {
        self.original.operation_id()
    }

    pub fn original(&self) -> &ChangeOperation {
        &self.original
    }

    pub fn current(&self) -> &ChangeOperation {
        &self.current
    }

    pub fn decision(&self) -> OperationDecision {
        self.decision
    }

    pub fn judgment(&self) -> Option<&str> {
        self.judgment.as_deref()
    }

    pub fn is_selected(&self) -> bool {
        self.decision != OperationDecision::Reject
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ManualReviewSession {
    original_draft: ChangeSetDraft,
    draft: ChangeSetDraft,
    operations: Vec<ManualReviewOperation>,
    validation_report: ValidationReport,
    effective_report: ValidationReport,
    waivers: Vec<ChangeSetWaiver>,
    ready_to_confirm: bool,
}

impl ManualReviewSession {
    pub(crate) fn create(
        world_id: WorldId,
        base_revision: RevisionId,
        input: ManualReviewInput,
        store: &WorldStore,
    ) -> Result<Self, AppError> {
        let operations: Vec<_> = input
            .operations
            .into_iter()
            .map(|operation| operation.into_change_operation(ChangeOperationId::new()))
            .collect();
        let decisions = default_replacement_decisions(&operations)?;
        let original_draft = ChangeSetDraft::new(
            world_id,
            base_revision,
            input.objective,
            input.sources,
            input.assumptions,
            operations,
            decisions,
        )?;
        let reviewed_operations = original_draft
            .operations()
            .iter()
            .cloned()
            .map(|operation| ManualReviewOperation {
                original: operation.clone(),
                current: operation,
                decision: OperationDecision::Accept,
                judgment: None,
            })
            .collect();
        Self::rebuild(original_draft, reviewed_operations, vec![], store)
    }

    pub(crate) fn apply_action(
        &self,
        action: ManualReviewAction,
        decided_at_ms: i64,
        store: &WorldStore,
    ) -> Result<Self, AppError> {
        let mut operations = self.operations.clone();
        let mut waivers = self.waivers.clone();

        match action {
            ManualReviewAction::Accept { operation_id } => {
                let operation = find_operation_mut(&mut operations, operation_id)?;
                operation.current = operation.original.clone();
                operation.decision = OperationDecision::Accept;
            }
            ManualReviewAction::Edit {
                operation_id,
                replacement,
            } => {
                let operation = find_operation_mut(&mut operations, operation_id)?;
                operation.current = replacement.into_change_operation(operation_id);
                operation.decision = OperationDecision::Edit;
                operation.judgment = None;
            }
            ManualReviewAction::RecordJudgment {
                operation_id,
                judgment,
            } => {
                let operation = find_operation_mut(&mut operations, operation_id)?;
                operation.judgment = Some(judgment.trim().to_owned());
            }
            ManualReviewAction::Reject { operation_id } => {
                let operation = find_operation_mut(&mut operations, operation_id)?;
                operation.decision = OperationDecision::Reject;
                operation.judgment = None;
                waivers.retain(|waiver| waiver.operation_id() != operation_id);
            }
            ManualReviewAction::AddWaiver {
                operation_id,
                issue_code,
                rationale,
            } => {
                let issue = find_issue(&self.validation_report, operation_id, &issue_code)
                    .ok_or_else(|| AppError::ReviewIssueNotFound {
                        operation_id,
                        issue_code: issue_code.clone(),
                    })?;
                if issue.severity == ValidationSeverity::Error {
                    return Err(AppError::CannotWaiveHardIssue {
                        operation_id,
                        issue_code,
                    });
                }

                waivers.retain(|waiver| {
                    waiver.operation_id() != operation_id || waiver.issue_code() != issue_code
                });
                waivers.push(ChangeSetWaiver::new(
                    operation_id,
                    issue_code,
                    rationale,
                    decided_at_ms,
                )?);
            }
        }

        Self::rebuild(self.original_draft.clone(), operations, waivers, store)
    }

    pub fn original_draft(&self) -> &ChangeSetDraft {
        &self.original_draft
    }

    pub fn draft(&self) -> &ChangeSetDraft {
        &self.draft
    }

    pub fn operations(&self) -> &[ManualReviewOperation] {
        &self.operations
    }

    pub fn validation_report(&self) -> &ValidationReport {
        &self.validation_report
    }

    pub fn effective_report(&self) -> &ValidationReport {
        &self.effective_report
    }

    pub fn waivers(&self) -> &[ChangeSetWaiver] {
        &self.waivers
    }

    pub fn ready_to_confirm(&self) -> bool {
        self.ready_to_confirm && self.pending_high_risk_judgments().is_empty()
    }

    pub fn can_confirm_against(&self, current_revision: RevisionId) -> bool {
        self.draft.base_revision() == current_revision && self.ready_to_confirm()
    }

    pub fn revalidate_at_revision(
        &self,
        base_revision: RevisionId,
        store: &WorldStore,
    ) -> Result<Self, AppError> {
        Self::rebuild(
            rebase_draft_revision(&self.original_draft, base_revision)?,
            self.operations.clone(),
            self.waivers.clone(),
            store,
        )
    }

    pub fn snapshot(
        &self,
        review_key: &str,
        freshness: ManualReviewFreshnessSnapshot,
    ) -> ManualReviewSnapshot {
        let ready_to_confirm =
            freshness.status == ManualReviewFreshnessStatus::Current && self.ready_to_confirm();
        ManualReviewSnapshot {
            review_key: review_key.to_owned(),
            objective: self.draft.objective().to_owned(),
            sources: self
                .draft
                .sources()
                .iter()
                .map(ToString::to_string)
                .collect(),
            assumptions: self.draft.assumptions().to_vec(),
            base_revision: self.draft.base_revision().to_string(),
            operations: self
                .operations
                .iter()
                .map(|operation| operation_snapshot(self, operation))
                .collect(),
            validation_report: self.validation_report.clone(),
            effective_report: self.effective_report.clone(),
            ready_to_confirm,
            freshness,
        }
    }

    fn pending_high_risk_judgments(&self) -> Vec<ChangeOperationId> {
        self.operations
            .iter()
            .filter(|operation| {
                operation.is_selected() && operation_requires_judgment(self, operation)
            })
            .filter(|operation| is_blank_option(operation.judgment()))
            .map(ManualReviewOperation::operation_id)
            .collect()
    }

    fn rebuild(
        original_draft: ChangeSetDraft,
        operations: Vec<ManualReviewOperation>,
        waivers: Vec<ChangeSetWaiver>,
        store: &WorldStore,
    ) -> Result<Self, AppError> {
        let draft = rebuilt_draft(&original_draft, &operations)?;
        let mut validation_report = store.validate_change_set_draft(&draft)?;
        validation_report.extend(broken_dependency_issues(&operations));
        annotate_report_with_operations(&mut validation_report, &operations);
        let waivers = retain_applicable_waivers(waivers, &operations, &validation_report);
        let effective_report = apply_waivers(&validation_report, &waivers);
        let ready_to_confirm = effective_report.is_ok();

        Ok(Self {
            original_draft,
            draft,
            operations,
            validation_report,
            effective_report,
            waivers,
            ready_to_confirm,
        })
    }
}

fn default_replacement_decisions(
    operations: &[ChangeOperation],
) -> Result<Vec<DecisionPoint>, AppError> {
    operations
        .iter()
        .filter(|operation| operation.retcon() == RetconKind::Replacement)
        .map(default_replacement_decision)
        .collect()
}

fn default_replacement_decision(operation: &ChangeOperation) -> Result<DecisionPoint, AppError> {
    let target = match operation {
        ChangeOperation::CreateEntity { .. }
        | ChangeOperation::CreateRelation { .. }
        | ChangeOperation::CreateEvent { .. }
        | ChangeOperation::CreateGoal { .. }
        | ChangeOperation::CreateRule { .. }
        | ChangeOperation::CreateClaim { .. }
        | ChangeOperation::CreateDocument { .. } => {
            return Err(nirmata_core::DomainError::InvalidChangeSetContext(
                "replacement create operations require explicit decision metadata",
            )
            .into());
        }
        _ => operation.primary_ref(),
    };

    DecisionPoint::new_replacement(
        vec![operation.operation_id()],
        format!("Should {target} replace the current canon?"),
        vec![
            DEFAULT_REPLACEMENT_KEEP.to_owned(),
            DEFAULT_REPLACEMENT_APPLY.to_owned(),
        ],
        target,
        format!("Manual review confirmed replacing {target}."),
        DEFAULT_REPLACEMENT_APPLY,
    )
    .map_err(Into::into)
}
