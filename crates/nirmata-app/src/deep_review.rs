use crate::{
    AiContextSnapshot, AiProposalProgress, AiProviderConfig, AiRequestOptions, AppError,
    ContextBundleRequest,
    ai::{AiModeClient, map_capability_error, serialize_payload},
};
use futures_util::{StreamExt, stream::FuturesUnordered};
use nirmata_ai::{
    AiError, CancellationToken, RequestOptions,
    capabilities::CapabilityError,
    contracts::{ContractId, DeepSynthesis, SpecialistReport, SpecialistRole},
};
use nirmata_core::{
    ChangeSetId, RevisionId, WorldId,
    change_set::ChangeOperation,
    document::ObjectRef,
    validation::{IssueObject, ValidationIssue, ValidationReport, ValidationSeverity},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
    time::{Duration, Instant},
};

pub const MAX_SPECIALISTS: usize = 4;
pub const MAX_CONTEXT_EXPANSIONS: u8 = 2;
pub const MAX_READ_TOOL_CALLS: u8 = 6;
pub const MAX_NESTED_DELEGATIONS: u8 = 0;
pub const SPECIALIST_MAX_OUTPUT_TOKENS: u32 = 2_048;
pub const SYNTHESIS_MAX_OUTPUT_TOKENS: u32 = 4_096;
pub const SPECIALIST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepReviewMode {
    DeepImpact,
    Audit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistSelectionSource {
    Explicit,
    RuleBased,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReviewBudget {
    pub max_specialists: usize,
    pub max_specialist_calls: usize,
    pub max_synthesis_calls: usize,
    pub max_context_expansions: u8,
    pub max_read_tool_calls: u8,
    pub max_nested_delegations: u8,
    pub specialist_max_output_tokens: u32,
    pub synthesis_max_output_tokens: u32,
    pub specialist_timeout_ms: u64,
}

impl Default for DeepReviewBudget {
    fn default() -> Self {
        Self {
            max_specialists: MAX_SPECIALISTS,
            max_specialist_calls: MAX_SPECIALISTS,
            max_synthesis_calls: 1,
            max_context_expansions: MAX_CONTEXT_EXPANSIONS,
            max_read_tool_calls: MAX_READ_TOOL_CALLS,
            max_nested_delegations: MAX_NESTED_DELEGATIONS,
            specialist_max_output_tokens: SPECIALIST_MAX_OUTPUT_TOKENS,
            synthesis_max_output_tokens: SYNTHESIS_MAX_OUTPUT_TOKENS,
            specialist_timeout_ms: SPECIALIST_TIMEOUT.as_millis() as u64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReviewPlan {
    pub mode: DeepReviewMode,
    pub request: String,
    pub roles: Vec<SpecialistRole>,
    pub selection_source: SpecialistSelectionSource,
    pub reason: String,
    pub budget: DeepReviewBudget,
    pub confirmed: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DeepReviewRunId(ChangeSetId);

impl DeepReviewRunId {
    fn new() -> Self {
        Self(ChangeSetId::new())
    }
}

impl fmt::Display for DeepReviewRunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for DeepReviewRunId {
    type Err = <ChangeSetId as FromStr>::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ChangeSetId::from_str(value).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeepReviewStatus {
    Running,
    Synthesizing,
    AwaitingReview,
    CompletedAudit,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistRunStatus {
    Pending,
    Running,
    Completed,
    TimedOut,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialistRunResult {
    pub role: SpecialistRole,
    pub status: SpecialistRunStatus,
    pub report: Option<SpecialistReport>,
    pub error: Option<String>,
    pub elapsed_ms: u64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReviewRun {
    pub id: DeepReviewRunId,
    pub world_id: WorldId,
    pub base_revision: RevisionId,
    pub mode: DeepReviewMode,
    pub request: String,
    pub context: AiContextSnapshot,
    pub plan: DeepReviewPlan,
    pub status: DeepReviewStatus,
    pub specialists: Vec<SpecialistRunResult>,
    pub synthesis: Option<DeepSynthesis>,
    pub audit_result: Option<DeepAuditResult>,
    pub standard_run_id: Option<crate::AiRunId>,
    pub error: Option<String>,
}

impl DeepReviewRun {
    pub(crate) fn running(plan: DeepReviewPlan, context: AiContextSnapshot) -> Self {
        Self {
            id: DeepReviewRunId::new(),
            world_id: context.world_id,
            base_revision: context.base_revision,
            mode: plan.mode,
            request: plan.request.clone(),
            specialists: plan
                .roles
                .iter()
                .copied()
                .map(|role| SpecialistRunResult {
                    role,
                    status: SpecialistRunStatus::Pending,
                    report: None,
                    error: None,
                    elapsed_ms: 0,
                    input_tokens: None,
                    output_tokens: None,
                })
                .collect(),
            context,
            plan,
            status: DeepReviewStatus::Running,
            synthesis: None,
            audit_result: None,
            standard_run_id: None,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepAuditResult {
    pub validation_report: ValidationReport,
    pub finding_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeepReviewProgress {
    SpecialistStarted {
        role: SpecialistRole,
    },
    SpecialistFinished {
        role: SpecialistRole,
        status: SpecialistRunStatus,
    },
    Synthesizing,
    ValidatingSynthesis,
    HandingToStandardReview,
    Completed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpecialistInput {
    mode: DeepReviewMode,
    specialist: SpecialistRole,
    task: String,
    snapshot: AiContextSnapshot,
    context_object_ids: Vec<String>,
    read_tools: [&'static str; 6],
    budget: DeepReviewBudget,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SynthesisInput {
    mode: DeepReviewMode,
    request: String,
    snapshot: AiContextSnapshot,
    reports: Vec<SpecialistReport>,
}

impl DeepReviewPlan {
    pub fn confirm(mut self, roles: Vec<SpecialistRole>) -> Result<Self, AppError> {
        validate_roles(self.mode, &roles)?;
        self.roles = roles;
        self.selection_source = SpecialistSelectionSource::Explicit;
        self.confirmed = true;
        Ok(self)
    }

    pub fn validate_for_execution(&self) -> Result<(), AppError> {
        if !self.confirmed {
            return Err(AppError::InvalidDeepReview(
                "deep review roles must be explicitly confirmed before execution".to_owned(),
            ));
        }
        if self.budget != DeepReviewBudget::default() {
            return Err(AppError::InvalidDeepReview(
                "deep review budgets are fixed by the application".to_owned(),
            ));
        }
        validate_roles(self.mode, &self.roles)
    }
}

pub fn specialist_capabilities() -> [&'static str; 6] {
    [
        "get_entity",
        "get_related_events",
        "get_rules",
        "get_claims",
        "get_goals",
        "search_canon",
    ]
}

pub fn validate_specialist_tool(tool: &str) -> Result<(), AppError> {
    if specialist_capabilities().contains(&tool) {
        Ok(())
    } else {
        Err(AppError::InvalidDeepReview(format!(
            "specialists cannot use tool `{tool}`; only fixed read tools are allowed"
        )))
    }
}

impl crate::NirmataApp {
    pub fn prepare_deep_review(
        &self,
        mode: DeepReviewMode,
        request: impl Into<String>,
        explicit_roles: Option<Vec<SpecialistRole>>,
        context_request: &ContextBundleRequest,
    ) -> Result<DeepReviewPlan, AppError> {
        if mode == DeepReviewMode::DeepImpact {
            crate::app::ensure_active_write_scope(
                self.active.as_ref().ok_or(AppError::NoWorldOpen)?,
            )?;
        }
        let request = request.into();
        if request.trim().is_empty() {
            return Err(AppError::InvalidDeepReview(
                "deep review request cannot be empty".to_owned(),
            ));
        }
        let (roles, selection_source, reason) = match explicit_roles {
            Some(roles) => (
                roles,
                SpecialistSelectionSource::Explicit,
                "Roles selected explicitly by the user.".to_owned(),
            ),
            None => select_roles_by_rule(mode, &request),
        };
        validate_roles(mode, &roles)?;
        let _ = self.build_ai_context_snapshot(context_request)?;
        Ok(DeepReviewPlan {
            mode,
            request,
            roles,
            selection_source,
            reason,
            budget: DeepReviewBudget::default(),
            confirmed: false,
        })
    }

    pub fn read_deep_review_run(&self, run_id: DeepReviewRunId) -> Result<DeepReviewRun, AppError> {
        self.deep_review_runs
            .get(&run_id)
            .cloned()
            .ok_or_else(|| AppError::DeepReviewRunNotFound(run_id.to_string()))
    }

    pub async fn execute_deep_review<F>(
        &mut self,
        provider: &AiProviderConfig,
        plan: DeepReviewPlan,
        context_request: &ContextBundleRequest,
        cancellation: CancellationToken,
        on_progress: F,
    ) -> Result<DeepReviewRun, AppError>
    where
        F: FnMut(DeepReviewProgress) + Send,
    {
        if plan.mode == DeepReviewMode::DeepImpact {
            crate::app::ensure_active_write_scope(
                self.active.as_ref().ok_or(AppError::NoWorldOpen)?,
            )?;
        }
        let client = self.provider_client(provider)?;
        self.execute_deep_review_with(&client, plan, context_request, cancellation, on_progress)
            .await
    }

    pub(crate) async fn execute_deep_review_with<C, F>(
        &mut self,
        client: &C,
        plan: DeepReviewPlan,
        context_request: &ContextBundleRequest,
        cancellation: CancellationToken,
        mut on_progress: F,
    ) -> Result<DeepReviewRun, AppError>
    where
        C: AiModeClient,
        F: FnMut(DeepReviewProgress) + Send,
    {
        plan.validate_for_execution()?;
        if plan.mode == DeepReviewMode::DeepImpact {
            crate::app::ensure_active_write_scope(
                self.active.as_ref().ok_or(AppError::NoWorldOpen)?,
            )?;
        }
        let snapshot = self.build_ai_context_snapshot(context_request)?;
        let mut run = DeepReviewRun::running(plan, snapshot.clone());
        let run_id = run.id;
        self.deep_review_runs.insert(run_id, run.clone());

        let context_object_ids = snapshot.context_object_ids();
        let mut pending = FuturesUnordered::new();
        for (index, role) in run.plan.roles.iter().copied().enumerate() {
            let payload = serialize_payload(
                &SpecialistInput {
                    mode: run.mode,
                    specialist: role,
                    task: run.request.clone(),
                    snapshot: snapshot.clone(),
                    context_object_ids: context_object_ids.clone(),
                    read_tools: specialist_capabilities(),
                    budget: run.plan.budget,
                },
                "specialist",
            )?;
            let ids = context_object_ids.clone();
            let token = cancellation.clone();
            on_progress(DeepReviewProgress::SpecialistStarted { role });
            run.specialists[index].status = SpecialistRunStatus::Running;
            pending.push(async move {
                if token.is_cancelled() {
                    return (
                        index,
                        role,
                        Duration::ZERO,
                        Err(CapabilityError::Ai(AiError::RequestCancelled)),
                    );
                }
                let started = Instant::now();
                let result = client
                    .run_specialist(
                        payload,
                        ids,
                        RequestOptions::new(SPECIALIST_TIMEOUT).with_cancellation(token),
                    )
                    .await;
                (index, role, started.elapsed(), result)
            });
        }

        while let Some((index, role, elapsed, result)) = pending.next().await {
            let specialist = &mut run.specialists[index];
            specialist.elapsed_ms = elapsed.as_millis().try_into().unwrap_or(u64::MAX);
            match result {
                Ok(invocation) => {
                    match validate_specialist_report(role, &context_object_ids, &invocation.output)
                    {
                        Ok(()) => {
                            specialist.status = SpecialistRunStatus::Completed;
                            specialist.input_tokens = invocation
                                .metadata
                                .usage
                                .as_ref()
                                .and_then(|usage| usage.input_tokens);
                            specialist.output_tokens = invocation
                                .metadata
                                .usage
                                .as_ref()
                                .and_then(|usage| usage.output_tokens);
                            specialist.report = Some(invocation.output);
                        }
                        Err(error) => {
                            specialist.status = SpecialistRunStatus::Failed;
                            specialist.error = Some(error.to_string());
                        }
                    }
                }
                Err(error) => {
                    specialist.status = match error {
                        CapabilityError::Ai(AiError::RequestTimedOut(_)) => {
                            SpecialistRunStatus::TimedOut
                        }
                        CapabilityError::Ai(AiError::RequestCancelled) => {
                            SpecialistRunStatus::Cancelled
                        }
                        _ => SpecialistRunStatus::Failed,
                    };
                    specialist.error = Some(map_capability_error(error).to_string());
                }
            }
            on_progress(DeepReviewProgress::SpecialistFinished {
                role,
                status: specialist.status,
            });
        }

        let reports = run
            .specialists
            .iter()
            .filter_map(|result| result.report.clone())
            .collect::<Vec<_>>();
        if reports.is_empty() {
            run.status = if cancellation.is_cancelled() {
                DeepReviewStatus::Cancelled
            } else {
                DeepReviewStatus::Failed
            };
            run.error = Some("all specialists failed; no proposal was created".to_owned());
            self.deep_review_runs.insert(run_id, run.clone());
            on_progress(DeepReviewProgress::Completed);
            return Ok(run);
        }

        if run.mode == DeepReviewMode::Audit {
            run.audit_result = Some(build_audit_result(&reports));
            run.status = DeepReviewStatus::CompletedAudit;
            self.deep_review_runs.insert(run_id, run.clone());
            on_progress(DeepReviewProgress::Completed);
            return Ok(run);
        }

        if cancellation.is_cancelled() {
            run.status = DeepReviewStatus::Cancelled;
            run.error = Some("deep review was cancelled before synthesis".to_owned());
            self.deep_review_runs.insert(run_id, run.clone());
            on_progress(DeepReviewProgress::Completed);
            return Ok(run);
        }

        run.status = DeepReviewStatus::Synthesizing;
        on_progress(DeepReviewProgress::Synthesizing);
        let synthesis_payload = serialize_payload(
            &SynthesisInput {
                mode: run.mode,
                request: run.request.clone(),
                snapshot: snapshot.clone(),
                reports: reports.clone(),
            },
            "deep synthesis",
        )?;
        let synthesis = client
            .run_synthesis(
                synthesis_payload,
                context_object_ids.clone(),
                RequestOptions::new(SPECIALIST_TIMEOUT).with_cancellation(cancellation.clone()),
            )
            .await;
        let synthesis = match synthesis {
            Ok(synthesis) => synthesis,
            Err(error) => {
                run.status = if matches!(error, CapabilityError::Ai(AiError::RequestCancelled)) {
                    DeepReviewStatus::Cancelled
                } else {
                    DeepReviewStatus::Failed
                };
                run.error = Some(map_capability_error(error).to_string());
                self.deep_review_runs.insert(run_id, run.clone());
                on_progress(DeepReviewProgress::Completed);
                return Ok(run);
            }
        };

        on_progress(DeepReviewProgress::ValidatingSynthesis);
        if let Err(error) = validate_synthesis(&snapshot, &reports, &synthesis.output) {
            run.status = DeepReviewStatus::Failed;
            run.error = Some(error.to_string());
            self.deep_review_runs.insert(run_id, run.clone());
            on_progress(DeepReviewProgress::Completed);
            return Ok(run);
        }
        run.synthesis = Some(synthesis.output.clone());

        on_progress(DeepReviewProgress::HandingToStandardReview);
        let standard_run = self
            .hand_deep_synthesis_to_standard_review(
                client,
                run.request.clone(),
                snapshot,
                synthesis,
                context_request,
                AiRequestOptions::new(SPECIALIST_TIMEOUT).with_cancellation(cancellation),
                |progress| match progress {
                    AiProposalProgress::Validating => {
                        on_progress(DeepReviewProgress::ValidatingSynthesis)
                    }
                    AiProposalProgress::CallingCritic => {
                        on_progress(DeepReviewProgress::HandingToStandardReview)
                    }
                    _ => {}
                },
            )
            .await;
        match standard_run {
            Ok(standard_run) => {
                run.standard_run_id = Some(standard_run.id);
                run.status = DeepReviewStatus::AwaitingReview;
            }
            Err(error) => {
                run.status = if matches!(error, AppError::Ai(AiError::RequestCancelled)) {
                    DeepReviewStatus::Cancelled
                } else {
                    DeepReviewStatus::Failed
                };
                run.error = Some(error.to_string());
            }
        }
        self.deep_review_runs.insert(run_id, run.clone());
        on_progress(DeepReviewProgress::Completed);
        Ok(run)
    }
}

fn validate_specialist_report(
    role: SpecialistRole,
    context_object_ids: &[String],
    report: &SpecialistReport,
) -> Result<(), AppError> {
    if report.specialist != role {
        return Err(AppError::InvalidDeepReview(format!(
            "specialist response role {:?} does not match assigned role {:?}",
            report.specialist, role
        )));
    }
    let allowed = context_object_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut cited = Vec::new();
    cited.extend(report.sources.iter().copied());
    for finding in &report.findings {
        cited.extend(finding.summary.content_references.iter().copied());
        cited.extend(finding.affected_object_uris.iter().copied());
        cited.extend(
            finding
                .candidate_consequences
                .iter()
                .flat_map(|consequence| consequence.content_references.iter().copied()),
        );
        cited.extend(finding.evidence.iter().map(|evidence| evidence.source_uri));
    }
    if let Some(uri) = cited
        .into_iter()
        .map(String::from)
        .find(|uri| !allowed.contains(uri))
    {
        return Err(AppError::InvalidDeepReview(format!(
            "specialist report cites {uri} outside the immutable snapshot"
        )));
    }
    Ok(())
}

fn validate_synthesis(
    snapshot: &AiContextSnapshot,
    reports: &[SpecialistReport],
    synthesis: &DeepSynthesis,
) -> Result<(), AppError> {
    if synthesis.draft.world_id() != snapshot.world_id
        || synthesis.draft.base_revision() != snapshot.base_revision
    {
        return Err(AppError::InvalidDeepReview(
            "synthesis draft does not use the immutable deep-review snapshot".to_owned(),
        ));
    }
    let allowed_sources = snapshot
        .context_object_ids()
        .into_iter()
        .collect::<BTreeSet<_>>();
    if let Some(source) = synthesis
        .draft
        .sources()
        .iter()
        .map(ToString::to_string)
        .find(|source| !allowed_sources.contains(source))
    {
        return Err(AppError::InvalidDeepReview(format!(
            "synthesis cites source {source} outside the immutable snapshot"
        )));
    }
    if synthesis
        .draft
        .sources()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != synthesis.draft.sources().len()
    {
        return Err(AppError::InvalidDeepReview(
            "synthesis draft contains duplicate sources".to_owned(),
        ));
    }

    let operation_ids = synthesis
        .draft
        .operations()
        .iter()
        .map(ChangeOperation::operation_id)
        .collect::<BTreeSet<_>>();
    let mapped_operations = synthesis
        .operation_origins
        .iter()
        .map(|origin| origin.operation_id)
        .collect::<BTreeSet<_>>();
    if operation_ids != mapped_operations
        || mapped_operations.len() != synthesis.operation_origins.len()
    {
        return Err(AppError::InvalidDeepReview(
            "synthesis must map every unique operation exactly once".to_owned(),
        ));
    }
    let decision_ids = synthesis
        .draft
        .decisions()
        .iter()
        .map(|decision| decision.decision_point_id())
        .collect::<BTreeSet<_>>();
    let mapped_decisions = synthesis
        .decision_origins
        .iter()
        .map(|origin| origin.decision_point_id)
        .collect::<BTreeSet<_>>();
    if decision_ids != mapped_decisions
        || mapped_decisions.len() != synthesis.decision_origins.len()
    {
        return Err(AppError::InvalidDeepReview(
            "synthesis must map every unique decision point exactly once".to_owned(),
        ));
    }

    let mut findings = BTreeMap::<ContractId, Option<(ContractId, String)>>::new();
    for report in reports {
        for finding in &report.findings {
            let position = finding
                .decision_position
                .as_ref()
                .map(|position| (position.decision_key.clone(), position.alternative.clone()));
            if findings
                .insert(finding.finding_id.clone(), position)
                .is_some()
            {
                return Err(AppError::InvalidDeepReview(format!(
                    "specialist finding id {} is duplicated across reports",
                    finding.finding_id.as_str()
                )));
            }
        }
    }
    for origin in &synthesis.operation_origins {
        if origin.finding_ids.is_empty()
            || origin
                .finding_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .len()
                != origin.finding_ids.len()
        {
            return Err(AppError::InvalidDeepReview(
                "each operation must cite one or more unique specialist findings".to_owned(),
            ));
        }
        for finding_id in &origin.finding_ids {
            if !findings.contains_key(finding_id) {
                return Err(AppError::InvalidDeepReview(format!(
                    "operation origin cites unknown finding {}",
                    finding_id.as_str()
                )));
            }
        }
    }
    for origin in &synthesis.decision_origins {
        if origin.finding_ids.len() < 2
            || origin
                .finding_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .len()
                != origin.finding_ids.len()
        {
            return Err(AppError::InvalidDeepReview(
                "each decision point must cite at least two unique specialist findings".to_owned(),
            ));
        }
        for finding_id in &origin.finding_ids {
            if !findings.contains_key(finding_id) {
                return Err(AppError::InvalidDeepReview(format!(
                    "decision origin cites unknown finding {}",
                    finding_id.as_str()
                )));
            }
        }
    }

    let mut disagreements = BTreeMap::<ContractId, BTreeMap<String, BTreeSet<ContractId>>>::new();
    for (finding_id, position) in &findings {
        if let Some((key, alternative)) = position {
            disagreements
                .entry(key.clone())
                .or_default()
                .entry(alternative.clone())
                .or_default()
                .insert(finding_id.clone());
        }
    }
    for (key, alternatives) in disagreements
        .into_iter()
        .filter(|(_, alternatives)| alternatives.len() > 1)
    {
        let required = alternatives
            .into_values()
            .flatten()
            .collect::<BTreeSet<_>>();
        let preserved = synthesis.decision_origins.iter().any(|origin| {
            let cited = origin.finding_ids.iter().cloned().collect::<BTreeSet<_>>();
            required.is_subset(&cited)
        });
        if !preserved {
            return Err(AppError::InvalidDeepReview(format!(
                "synthesis silently resolved specialist disagreement {}",
                key.as_str()
            )));
        }
    }
    Ok(())
}

fn build_audit_result(reports: &[SpecialistReport]) -> DeepAuditResult {
    let mut issues = Vec::new();
    let mut finding_ids = Vec::new();
    for report in reports {
        for finding in &report.findings {
            finding_ids.push(finding.finding_id.as_str().to_owned());
            issues.push(ValidationIssue {
                code: format!("deep.audit.{}", finding.finding_id.as_str()),
                severity: ValidationSeverity::Warning,
                objects: finding
                    .affected_object_uris
                    .iter()
                    .map(|uri| issue_object(uri.object_ref()))
                    .collect(),
                message: finding.summary.markdown.clone(),
            });
        }
    }
    DeepAuditResult {
        validation_report: ValidationReport::from_issues(issues),
        finding_ids,
    }
}

fn issue_object(object: ObjectRef) -> IssueObject {
    let kind = object.kind();
    let id = object
        .to_string()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    IssueObject {
        kind: kind.to_owned(),
        id,
    }
}

fn select_roles_by_rule(
    mode: DeepReviewMode,
    request: &str,
) -> (Vec<SpecialistRole>, SpecialistSelectionSource, String) {
    if mode == DeepReviewMode::Audit {
        return (
            vec![
                SpecialistRole::TemporalAuditor,
                SpecialistRole::RulesAuditor,
                SpecialistRole::CausalAuditor,
                SpecialistRole::PerspectivesAuditor,
            ],
            SpecialistSelectionSource::RuleBased,
            "Audit mode uses the four closed read-only audit roles.".to_owned(),
        );
    }

    let normalized = request.to_lowercase();
    let mut roles = Vec::new();
    add_role_for_markers(
        &mut roles,
        SpecialistRole::Economist,
        &normalized,
        &["recurso", "comerc", "econom", "mina", "escasez"],
    );
    add_role_for_markers(
        &mut roles,
        SpecialistRole::Historian,
        &normalized,
        &["guerra", "suces", "histori", "dinast"],
    );
    add_role_for_markers(
        &mut roles,
        SpecialistRole::PoliticalScientist,
        &normalized,
        &["guerra", "suces", "polít", "polit", "gobierno", "poder"],
    );
    add_role_for_markers(
        &mut roles,
        SpecialistRole::Anthropologist,
        &normalized,
        &["ritual", "tabú", "tabu", "costumbre", "cultura"],
    );
    add_role_for_markers(
        &mut roles,
        SpecialistRole::Theologian,
        &normalized,
        &["ritual", "relig", "teolog", "dios", "culto"],
    );
    add_role_for_markers(
        &mut roles,
        SpecialistRole::Geographer,
        &normalized,
        &["geograf", "territ", "frontera", "río", "rio", "ruta"],
    );
    if roles.is_empty() {
        roles.push(SpecialistRole::Historian);
    }
    roles.truncate(MAX_SPECIALISTS);
    (
        roles,
        SpecialistSelectionSource::RuleBased,
        "Roles suggested by deterministic domain keywords; user confirmation is still required."
            .to_owned(),
    )
}

fn add_role_for_markers(
    roles: &mut Vec<SpecialistRole>,
    role: SpecialistRole,
    request: &str,
    markers: &[&str],
) {
    if markers.iter().any(|marker| request.contains(marker)) && !roles.contains(&role) {
        roles.push(role);
    }
}

fn validate_roles(mode: DeepReviewMode, roles: &[SpecialistRole]) -> Result<(), AppError> {
    if roles.is_empty() {
        return Err(AppError::InvalidDeepReview(
            "deep review requires at least one specialist".to_owned(),
        ));
    }
    if roles.len() > MAX_SPECIALISTS {
        return Err(AppError::InvalidDeepReview(format!(
            "deep review cannot exceed {MAX_SPECIALISTS} specialists"
        )));
    }
    let unique = roles.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != roles.len() {
        return Err(AppError::InvalidDeepReview(
            "deep review cannot repeat a specialist role".to_owned(),
        ));
    }
    let valid_mode = roles.iter().all(|role| match mode {
        DeepReviewMode::DeepImpact => !matches!(
            role,
            SpecialistRole::TemporalAuditor
                | SpecialistRole::RulesAuditor
                | SpecialistRole::CausalAuditor
                | SpecialistRole::PerspectivesAuditor
        ),
        DeepReviewMode::Audit => matches!(
            role,
            SpecialistRole::TemporalAuditor
                | SpecialistRole::RulesAuditor
                | SpecialistRole::CausalAuditor
                | SpecialistRole::PerspectivesAuditor
        ),
    });
    if !valid_mode {
        return Err(AppError::InvalidDeepReview(
            "selected specialist roles do not belong to the requested deep-review mode".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/deep_review.rs"]
mod tests;
