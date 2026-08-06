use nirmata_core::{
    ChangeOperationId, ClaimId, DomainError,
    change_set::{ChangeOperation, ChangeSetDraft},
    document::{DocumentAggregate, ObjectRef},
    validation::ValidationSeverity,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, error::Category as JsonErrorCategory};
use std::{collections::HashSet, error::Error, fmt, str::FromStr};

pub const MAX_ADVISORY_ITEMS: usize = 16;
pub const MAX_ADVISORY_CITATIONS: usize = 8;
pub const MAX_ADVISORY_TEXT_CHARS: usize = 2_000;
pub const MAX_CITATION_QUOTE_CHARS: usize = 500;
pub const MAX_PROSE_CONTENT_REFERENCES: usize = 16;
pub const MAX_CRITIQUE_ISSUES: usize = 32;
pub const MAX_CRITIQUE_EVIDENCE_ITEMS: usize = 8;
pub const MAX_RELATED_OBJECT_URIS: usize = 16;
pub const MAX_RESOLUTION_CHARS: usize = 1_000;
pub const MAX_CONTRACT_ID_CHARS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputErrorKind {
    EmptyResponse,
    FreeTextMutation,
    TruncatedJson,
    InvalidJson,
    InvalidShape,
    InvalidContent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredOutputDiagnostic {
    pub char_count: usize,
    pub starts_with: Option<char>,
    pub ends_with: Option<char>,
    pub looks_like_json_object: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredOutputError {
    kind: StructuredOutputErrorKind,
    message: String,
    diagnostic: StructuredOutputDiagnostic,
}

impl StructuredOutputError {
    pub fn kind(&self) -> StructuredOutputErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn diagnostic(&self) -> &StructuredOutputDiagnostic {
        &self.diagnostic
    }

    fn new(kind: StructuredOutputErrorKind, message: impl Into<String>, payload: &str) -> Self {
        Self {
            kind,
            message: message.into(),
            diagnostic: build_diagnostic(payload),
        }
    }

    fn from_json_error(contract: &'static str, payload: &str, error: serde_json::Error) -> Self {
        let kind = match error.classify() {
            JsonErrorCategory::Eof => StructuredOutputErrorKind::TruncatedJson,
            JsonErrorCategory::Syntax | JsonErrorCategory::Io => {
                StructuredOutputErrorKind::InvalidJson
            }
            JsonErrorCategory::Data => StructuredOutputErrorKind::InvalidShape,
        };

        Self::new(
            kind,
            format!("{contract} must be valid structured JSON: {error}"),
            payload,
        )
    }

    fn invalid_content(contract: &'static str, payload: &str, message: impl Into<String>) -> Self {
        Self::new(
            StructuredOutputErrorKind::InvalidContent,
            format!("{contract} is invalid: {}", message.into()),
            payload,
        )
    }
}

impl fmt::Display for StructuredOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl Error for StructuredOutputError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentUri(ObjectRef);

impl ContentUri {
    pub fn object_ref(self) -> ObjectRef {
        self.0
    }
}

impl TryFrom<String> for ContentUri {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ObjectRef::from_str(&value)
            .map(Self)
            .map_err(|_| format!("invalid nirmata content URI: {value}"))
    }
}

impl From<ContentUri> for String {
    fn from(value: ContentUri) -> Self {
        value.0.to_string()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContractId(String);

impl ContractId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContractId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err("contract ids cannot be empty".to_owned());
        }
        if value.chars().count() > MAX_CONTRACT_ID_CHARS {
            return Err(format!(
                "contract ids cannot exceed {MAX_CONTRACT_ID_CHARS} characters"
            ));
        }
        if !is_valid_contract_id(&value) {
            return Err(format!(
                "invalid contract id `{value}`; use lowercase letters, numbers, hyphen or underscore"
            ));
        }
        Ok(Self(value))
    }
}

impl From<ContractId> for String {
    fn from(value: ContractId) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvisoryClassification {
    Fact,
    Perspective,
    Inference,
    NoEvidence,
    Unspecified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferencedMarkdown {
    pub markdown: String,
    pub content_references: Vec<ContentUri>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvisoryCitation {
    pub source_uri: ContentUri,
    pub quote_md: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvisoryItem {
    pub item_id: ContractId,
    pub classification: AdvisoryClassification,
    pub answer: ReferencedMarkdown,
    pub citations: Vec<AdvisoryCitation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvisoryResponse {
    pub items: Vec<AdvisoryItem>,
}

impl AdvisoryResponse {
    fn validate(&self) -> Result<(), String> {
        validate_max_items(
            "advisory_response.items",
            self.items.len(),
            MAX_ADVISORY_ITEMS,
        )?;
        if self.items.is_empty() {
            return Err("advisory_response.items must include at least one item".to_owned());
        }

        let mut seen_ids = HashSet::with_capacity(self.items.len());
        for item in &self.items {
            if !seen_ids.insert(item.item_id.clone()) {
                return Err(format!(
                    "advisory_response.items repeats item_id {}",
                    item.item_id.as_str()
                ));
            }
            item.answer.validate(
                "advisory_response.items.answer",
                MAX_ADVISORY_TEXT_CHARS,
                requires_answer_references(item.classification),
            )?;
            validate_max_items(
                "advisory_response.items.citations",
                item.citations.len(),
                MAX_ADVISORY_CITATIONS,
            )?;
            if requires_citations(item.classification) && item.citations.is_empty() {
                return Err(format!(
                    "advisory_response item {} requires at least one citation",
                    item.item_id.as_str()
                ));
            }
            for citation in &item.citations {
                validate_required_text(
                    "advisory_response.items.citations.quote_md",
                    &citation.quote_md,
                )?;
                validate_max_chars(
                    "advisory_response.items.citations.quote_md",
                    &citation.quote_md,
                    MAX_CITATION_QUOTE_CHARS,
                )?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueCategory {
    CanonContradiction,
    UniverseRule,
    TemporalConflict,
    CausalCycle,
    ImpossibleKnowledge,
    MissingConsequence,
    InsufficientEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueAttackType {
    Rebuts,
    Undercuts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CritiqueEvidence {
    pub source_uri: ContentUri,
    pub excerpt_md: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CritiqueIssue {
    pub issue_id: ContractId,
    pub summary: ReferencedMarkdown,
    pub affected_operation_ids: Vec<ChangeOperationId>,
    pub related_object_uris: Vec<ContentUri>,
    pub evidence: Vec<CritiqueEvidence>,
    pub severity: ValidationSeverity,
    pub category: CritiqueCategory,
    pub attack_type: Option<CritiqueAttackType>,
    pub target_claim_id: Option<ClaimId>,
    pub confidence: f64,
    pub suggested_resolution: Option<ReferencedMarkdown>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CritiqueReport {
    pub issues: Vec<CritiqueIssue>,
}

impl CritiqueReport {
    fn validate(&self) -> Result<(), String> {
        validate_max_items(
            "critique_report.issues",
            self.issues.len(),
            MAX_CRITIQUE_ISSUES,
        )?;
        let mut seen_issue_ids = HashSet::with_capacity(self.issues.len());
        for issue in &self.issues {
            if !seen_issue_ids.insert(issue.issue_id.clone()) {
                return Err(format!(
                    "critique_report.issues repeats issue_id {}",
                    issue.issue_id.as_str()
                ));
            }
            issue.summary.validate(
                "critique_report.issues.summary",
                MAX_ADVISORY_TEXT_CHARS,
                true,
            )?;
            validate_max_items(
                "critique_report.issues.related_object_uris",
                issue.related_object_uris.len(),
                MAX_RELATED_OBJECT_URIS,
            )?;
            validate_unique_items(
                "critique_report.issues.related_object_uris",
                issue.related_object_uris.iter().copied(),
            )?;
            validate_max_items(
                "critique_report.issues.affected_operation_ids",
                issue.affected_operation_ids.len(),
                MAX_RELATED_OBJECT_URIS,
            )?;
            validate_unique_items(
                "critique_report.issues.affected_operation_ids",
                issue.affected_operation_ids.iter().copied(),
            )?;
            if issue.affected_operation_ids.is_empty() {
                return Err(format!(
                    "critique_report issue {} must cite at least one affected operation",
                    issue.issue_id.as_str()
                ));
            }
            if issue.severity == ValidationSeverity::Error {
                return Err(format!(
                    "critique_report issue {} cannot declare a hard error",
                    issue.issue_id.as_str()
                ));
            }
            validate_max_items(
                "critique_report.issues.evidence",
                issue.evidence.len(),
                MAX_CRITIQUE_EVIDENCE_ITEMS,
            )?;
            if issue.evidence.is_empty() {
                return Err(format!(
                    "critique_report issue {} must include evidence",
                    issue.issue_id.as_str()
                ));
            }
            for evidence in &issue.evidence {
                validate_required_text(
                    "critique_report.issues.evidence.excerpt_md",
                    &evidence.excerpt_md,
                )?;
                validate_max_chars(
                    "critique_report.issues.evidence.excerpt_md",
                    &evidence.excerpt_md,
                    MAX_CITATION_QUOTE_CHARS,
                )?;
            }
            if !issue.confidence.is_finite() || !(0.0..=1.0).contains(&issue.confidence) {
                return Err(format!(
                    "critique_report issue {} has invalid confidence",
                    issue.issue_id.as_str()
                ));
            }
            if let Some(resolution) = &issue.suggested_resolution {
                resolution.validate(
                    "critique_report.issues.suggested_resolution",
                    MAX_RESOLUTION_CHARS,
                    true,
                )?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProposedChangeSetDraft(pub ChangeSetDraft);

impl ProposedChangeSetDraft {
    pub fn draft(&self) -> &ChangeSetDraft {
        &self.0
    }

    pub fn into_inner(self) -> ChangeSetDraft {
        self.0
    }
}

pub fn parse_advisory_response(payload: &str) -> Result<AdvisoryResponse, StructuredOutputError> {
    parse_contract(payload, "advisory_response", AdvisoryResponse::validate)
}

pub fn parse_critique_report(payload: &str) -> Result<CritiqueReport, StructuredOutputError> {
    parse_contract(payload, "critique_report", CritiqueReport::validate)
}

pub fn parse_change_set_draft(
    payload: &str,
) -> Result<ProposedChangeSetDraft, StructuredOutputError> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Err(StructuredOutputError::new(
            StructuredOutputErrorKind::EmptyResponse,
            "change_set_draft output cannot be empty",
            payload,
        ));
    }
    if !trimmed.starts_with('{') {
        return Err(StructuredOutputError::new(
            StructuredOutputErrorKind::FreeTextMutation,
            "change_set_draft output must be a JSON object, not free text",
            payload,
        ));
    }

    let raw_value = parse_json_value(payload, "change_set_draft")?;
    let raw_draft: ChangeSetDraft =
        deserialize_value(raw_value.clone(), "change_set_draft", payload)?;
    let validated = reconstruct_change_set_draft(raw_draft).map_err(|error| {
        StructuredOutputError::invalid_content("change_set_draft", payload, error.to_string())
    })?;
    validate_document_content_references(&validated).map_err(|error| {
        StructuredOutputError::invalid_content("change_set_draft", payload, error)
    })?;
    ensure_no_ignored_fields(&raw_value, &validated, "change_set_draft", payload)?;

    Ok(ProposedChangeSetDraft(validated))
}

fn parse_contract<T>(
    payload: &str,
    contract: &'static str,
    validate: impl FnOnce(&T) -> Result<(), String>,
) -> Result<T, StructuredOutputError>
where
    T: DeserializeOwned + Serialize,
{
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Err(StructuredOutputError::new(
            StructuredOutputErrorKind::EmptyResponse,
            format!("{contract} output cannot be empty"),
            payload,
        ));
    }

    let raw_value = parse_json_value(payload, contract)?;
    let parsed: T = deserialize_value(raw_value, contract, payload)?;
    validate(&parsed)
        .map_err(|error| StructuredOutputError::invalid_content(contract, payload, error))?;
    Ok(parsed)
}

fn parse_json_value(payload: &str, contract: &'static str) -> Result<Value, StructuredOutputError> {
    serde_json::from_str(payload)
        .map_err(|error| StructuredOutputError::from_json_error(contract, payload, error))
}

fn deserialize_value<T>(
    value: Value,
    contract: &'static str,
    payload: &str,
) -> Result<T, StructuredOutputError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value)
        .map_err(|error| StructuredOutputError::from_json_error(contract, payload, error))
}

fn reconstruct_change_set_draft(draft: ChangeSetDraft) -> Result<ChangeSetDraft, DomainError> {
    ChangeSetDraft::restore(
        draft.id(),
        draft.world_id(),
        draft.base_revision(),
        draft.objective().to_owned(),
        draft.sources().to_vec(),
        draft.assumptions().to_vec(),
        draft.operations().to_vec(),
        draft.decisions().to_vec(),
    )
}

fn validate_document_content_references(draft: &ChangeSetDraft) -> Result<(), String> {
    for operation in draft.operations() {
        match operation {
            ChangeOperation::CreateDocument {
                operation_id,
                after,
                ..
            }
            | ChangeOperation::UpdateDocument {
                operation_id,
                after,
                ..
            } => validate_generated_document(*operation_id, after)?,
            ChangeOperation::DeleteDocument { .. }
            | ChangeOperation::UpdateWorld { .. }
            | ChangeOperation::CreateEntity { .. }
            | ChangeOperation::UpdateEntity { .. }
            | ChangeOperation::DeleteEntity { .. }
            | ChangeOperation::CreateRelation { .. }
            | ChangeOperation::UpdateRelation { .. }
            | ChangeOperation::DeleteRelation { .. }
            | ChangeOperation::CreateEvent { .. }
            | ChangeOperation::UpdateEvent { .. }
            | ChangeOperation::DeleteEvent { .. }
            | ChangeOperation::CreateGoal { .. }
            | ChangeOperation::UpdateGoal { .. }
            | ChangeOperation::DeleteGoal { .. }
            | ChangeOperation::CreateRule { .. }
            | ChangeOperation::UpdateRule { .. }
            | ChangeOperation::DeleteRule { .. }
            | ChangeOperation::CreateClaim { .. }
            | ChangeOperation::UpdateClaim { .. }
            | ChangeOperation::DeleteClaim { .. } => {}
        }
    }

    Ok(())
}

fn validate_generated_document(
    operation_id: ChangeOperationId,
    document: &DocumentAggregate,
) -> Result<(), String> {
    if document.object().body_md().trim().is_empty() {
        return Ok(());
    }
    if document.references().is_empty() {
        return Err(format!(
            "operation {operation_id} contains generated prose without content references"
        ));
    }
    if document.references().iter().all(|reference| {
        !matches!(
            reference.target(),
            ObjectRef::Entity(_) | ObjectRef::Event(_) | ObjectRef::Rule(_)
        )
    }) {
        return Err(format!(
            "operation {operation_id} must declare at least one entity, event or rule content reference"
        ));
    }

    Ok(())
}

fn ensure_no_ignored_fields<T: Serialize>(
    raw_value: &Value,
    parsed: &T,
    contract: &'static str,
    payload: &str,
) -> Result<(), StructuredOutputError> {
    let canonical = serde_json::to_value(parsed).map_err(|error| {
        StructuredOutputError::invalid_content(
            contract,
            payload,
            format!("could not normalize parsed output: {error}"),
        )
    })?;
    if raw_value != &canonical {
        return Err(StructuredOutputError::invalid_content(
            contract,
            payload,
            "the payload contained unknown, ignored or non-canonical fields",
        ));
    }
    Ok(())
}

impl ReferencedMarkdown {
    fn validate(
        &self,
        field: &'static str,
        max_chars: usize,
        requires_references: bool,
    ) -> Result<(), String> {
        validate_required_text(field, &self.markdown)?;
        validate_max_chars(field, &self.markdown, max_chars)?;
        validate_max_items(
            &format!("{field}.content_references"),
            self.content_references.len(),
            MAX_PROSE_CONTENT_REFERENCES,
        )?;
        if requires_references && self.content_references.is_empty() {
            return Err(format!("{field} must declare content_references"));
        }
        validate_unique_items(
            &format!("{field}.content_references"),
            self.content_references.iter().copied(),
        )?;
        Ok(())
    }
}

fn validate_required_text(field: &'static str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} cannot be empty"));
    }
    Ok(())
}

fn validate_max_chars(field: &str, value: &str, max_chars: usize) -> Result<(), String> {
    if value.chars().count() > max_chars {
        return Err(format!("{field} cannot exceed {max_chars} characters"));
    }
    Ok(())
}

fn validate_max_items(field: &str, count: usize, max_items: usize) -> Result<(), String> {
    if count > max_items {
        return Err(format!("{field} cannot exceed {max_items} items"));
    }
    Ok(())
}

fn validate_unique_items<T>(field: &str, items: impl IntoIterator<Item = T>) -> Result<(), String>
where
    T: Eq + std::hash::Hash,
{
    let mut seen = HashSet::new();
    for item in items {
        if !seen.insert(item) {
            return Err(format!("{field} cannot contain duplicates"));
        }
    }
    Ok(())
}

fn requires_citations(classification: AdvisoryClassification) -> bool {
    requires_answer_references(classification)
}

fn requires_answer_references(classification: AdvisoryClassification) -> bool {
    matches!(
        classification,
        AdvisoryClassification::Fact
            | AdvisoryClassification::Perspective
            | AdvisoryClassification::Inference
    )
}

fn is_valid_contract_id(value: &str) -> bool {
    value.chars().enumerate().all(|(index, character)| {
        matches!(character, 'a'..='z' | '0'..='9' | '-' | '_')
            && !(index == 0 && matches!(character, '-' | '_'))
    })
}

fn build_diagnostic(payload: &str) -> StructuredOutputDiagnostic {
    let trimmed = payload.trim();
    StructuredOutputDiagnostic {
        char_count: trimmed.chars().count(),
        starts_with: trimmed.chars().next(),
        ends_with: trimmed.chars().last(),
        looks_like_json_object: trimmed.starts_with('{'),
    }
}

#[cfg(test)]
#[path = "../tests/contracts/mod.rs"]
mod tests;
