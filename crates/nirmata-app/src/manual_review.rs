use crate::AppError;
use nirmata_core::{
    ChangeOperationId, RevisionId, World, WorldId,
    change_set::{ChangeOperation, ChangeSetDraft, DecisionPoint, RetconKind},
    claim::{Claim, ClaimObject},
    document::{Document, DocumentAggregate, ObjectRef},
    entity::Entity,
    event::{Event, EventAggregate},
    goal::Goal,
    relation::Relation,
    rule::Rule,
    validation::{IssueObject, ValidationIssue, ValidationReport, ValidationSeverity},
};
use nirmata_store::{
    ChangeOperationValue, ChangeSetWaiver, CommittedChangeSetRecord, OperationAudit,
    OperationDecision, StoreError, StoredRevision, WorldStore,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

const DEFAULT_REPLACEMENT_KEEP: &str = "Keep current canon";
const DEFAULT_REPLACEMENT_APPLY: &str = "Apply replacement";
const HIGH_IMPACT_AFFECTED_OBJECTS_THRESHOLD: usize = 4;

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
            judgment: Some("Undo requested from revision history.".to_owned()),
        })
        .collect();
    ManualReviewSession::rebuild(original_draft, reviewed_operations, vec![], store)
}

fn rebuilt_draft(
    original_draft: &ChangeSetDraft,
    operations: &[ManualReviewOperation],
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

    let decisions = original_draft
        .decisions()
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
