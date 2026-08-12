use nirmata_core::{
    ChangeOperationId, ClaimId, DecisionPointId, DomainError,
    change_set::{ChangeOperation, ChangeSetDraft, MAX_DECISION_ALTERNATIVE_CHARS},
    claim::{ClaimAuthentication, ClaimPolarity},
    document::{DocumentAggregate, ObjectRef},
    entity::EntityKind,
    relation::RelationDirection,
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
pub const MAX_PROPOSAL_DECISION_POINTS: usize = 3;
pub const MAX_SPECIALIST_FINDINGS: usize = 16;
pub const MAX_SPECIALIST_SOURCES: usize = 32;
pub const MAX_FINDING_CONSEQUENCES: usize = 8;
pub const MAX_FINDING_ASSUMPTIONS: usize = 8;
pub const MAX_FINDING_QUESTIONS: usize = 8;
pub const MAX_IMPORT_CANDIDATES: usize = 64;
pub const MAX_IMPORT_CITATIONS: usize = 8;

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistRole {
    Economist,
    Historian,
    PoliticalScientist,
    Anthropologist,
    Theologian,
    Geographer,
    TemporalAuditor,
    RulesAuditor,
    CausalAuditor,
    PerspectivesAuditor,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportCitation {
    pub chunk_id: ContractId,
    pub source_id: ContractId,
    pub source_hash: String,
    pub excerpt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ImportCandidate {
    Entity {
        candidate_id: ContractId,
        name: String,
        entity_kind: EntityKind,
        aliases: Vec<String>,
        summary: String,
        contradiction_key: Option<ContractId>,
        citations: Vec<ImportCitation>,
        technical_confidence: f64,
    },
    Relation {
        candidate_id: ContractId,
        source_name: String,
        target_name: String,
        relation_kind: String,
        direction: RelationDirection,
        contradiction_key: Option<ContractId>,
        citations: Vec<ImportCitation>,
        technical_confidence: f64,
    },
    Event {
        candidate_id: ContractId,
        summary: String,
        body_md: String,
        participant_names: Vec<String>,
        contradiction_key: Option<ContractId>,
        citations: Vec<ImportCitation>,
        technical_confidence: f64,
    },
    Claim {
        candidate_id: ContractId,
        subject_name: String,
        content_md: String,
        predicate_key: Option<String>,
        object_scalar: Option<String>,
        polarity: ClaimPolarity,
        authentication: ClaimAuthentication,
        contradiction_key: Option<ContractId>,
        citations: Vec<ImportCitation>,
        technical_confidence: f64,
    },
    Rule {
        candidate_id: ContractId,
        statement_md: String,
        scope: String,
        contradiction_key: Option<ContractId>,
        citations: Vec<ImportCitation>,
        technical_confidence: f64,
    },
}

impl ImportCandidate {
    pub fn candidate_id(&self) -> &ContractId {
        match self {
            Self::Entity { candidate_id, .. }
            | Self::Relation { candidate_id, .. }
            | Self::Event { candidate_id, .. }
            | Self::Claim { candidate_id, .. }
            | Self::Rule { candidate_id, .. } => candidate_id,
        }
    }

    pub fn citations(&self) -> &[ImportCitation] {
        match self {
            Self::Entity { citations, .. }
            | Self::Relation { citations, .. }
            | Self::Event { citations, .. }
            | Self::Claim { citations, .. }
            | Self::Rule { citations, .. } => citations,
        }
    }

    pub fn technical_confidence(&self) -> f64 {
        match self {
            Self::Entity {
                technical_confidence,
                ..
            }
            | Self::Relation {
                technical_confidence,
                ..
            }
            | Self::Event {
                technical_confidence,
                ..
            }
            | Self::Claim {
                technical_confidence,
                ..
            }
            | Self::Rule {
                technical_confidence,
                ..
            } => *technical_confidence,
        }
    }

    pub fn contradiction_key(&self) -> Option<&ContractId> {
        match self {
            Self::Entity {
                contradiction_key, ..
            }
            | Self::Relation {
                contradiction_key, ..
            }
            | Self::Event {
                contradiction_key, ..
            }
            | Self::Claim {
                contradiction_key, ..
            }
            | Self::Rule {
                contradiction_key, ..
            } => contradiction_key.as_ref(),
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Entity { .. } => "entity",
            Self::Relation { .. } => "relation",
            Self::Event { .. } => "event",
            Self::Claim { .. } => "claim",
            Self::Rule { .. } => "rule",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportExtraction {
    pub candidates: Vec<ImportCandidate>,
}

impl ImportExtraction {
    fn validate(&self) -> Result<(), String> {
        validate_max_items(
            "import_extraction.candidates",
            self.candidates.len(),
            MAX_IMPORT_CANDIDATES,
        )?;
        let mut ids = HashSet::with_capacity(self.candidates.len());
        for candidate in &self.candidates {
            if !ids.insert(candidate.candidate_id().clone()) {
                return Err(format!(
                    "import_extraction repeats candidate_id {}",
                    candidate.candidate_id().as_str()
                ));
            }
            validate_import_candidate(candidate)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistEvidence {
    pub source_uri: ContentUri,
    pub excerpt_md: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistFinding {
    pub finding_id: ContractId,
    pub summary: ReferencedMarkdown,
    pub affected_object_uris: Vec<ContentUri>,
    pub candidate_consequences: Vec<ReferencedMarkdown>,
    pub assumptions: Vec<String>,
    pub evidence: Vec<SpecialistEvidence>,
    pub confidence: f64,
    pub unresolved_questions: Vec<String>,
    pub decision_position: Option<SpecialistDecisionPosition>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistDecisionPosition {
    pub decision_key: ContractId,
    pub alternative: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistReport {
    pub specialist: SpecialistRole,
    pub sources: Vec<ContentUri>,
    pub findings: Vec<SpecialistFinding>,
}

impl SpecialistReport {
    fn validate(&self) -> Result<(), String> {
        validate_max_items(
            "specialist_report.sources",
            self.sources.len(),
            MAX_SPECIALIST_SOURCES,
        )?;
        if self.sources.is_empty() {
            return Err("specialist_report.sources must include at least one source".to_owned());
        }
        validate_unique_items("specialist_report.sources", self.sources.iter().copied())?;
        validate_max_items(
            "specialist_report.findings",
            self.findings.len(),
            MAX_SPECIALIST_FINDINGS,
        )?;
        if self.findings.is_empty() {
            return Err("specialist_report.findings must include at least one finding".to_owned());
        }

        let sources = self.sources.iter().copied().collect::<HashSet<_>>();
        let mut finding_ids = HashSet::with_capacity(self.findings.len());
        for finding in &self.findings {
            if !finding_ids.insert(finding.finding_id.clone()) {
                return Err(format!(
                    "specialist_report.findings repeats finding_id {}",
                    finding.finding_id.as_str()
                ));
            }
            finding.summary.validate(
                "specialist_report.findings.summary",
                MAX_ADVISORY_TEXT_CHARS,
                true,
            )?;
            validate_max_items(
                "specialist_report.findings.affected_object_uris",
                finding.affected_object_uris.len(),
                MAX_RELATED_OBJECT_URIS,
            )?;
            if finding.affected_object_uris.is_empty() {
                return Err(format!(
                    "specialist_report finding {} must cite an affected object",
                    finding.finding_id.as_str()
                ));
            }
            validate_unique_items(
                "specialist_report.findings.affected_object_uris",
                finding.affected_object_uris.iter().copied(),
            )?;
            validate_max_items(
                "specialist_report.findings.candidate_consequences",
                finding.candidate_consequences.len(),
                MAX_FINDING_CONSEQUENCES,
            )?;
            for consequence in &finding.candidate_consequences {
                consequence.validate(
                    "specialist_report.findings.candidate_consequences",
                    MAX_ADVISORY_TEXT_CHARS,
                    true,
                )?;
            }
            validate_text_items(
                "specialist_report.findings.assumptions",
                &finding.assumptions,
                MAX_FINDING_ASSUMPTIONS,
            )?;
            validate_text_items(
                "specialist_report.findings.unresolved_questions",
                &finding.unresolved_questions,
                MAX_FINDING_QUESTIONS,
            )?;
            validate_max_items(
                "specialist_report.findings.evidence",
                finding.evidence.len(),
                MAX_CRITIQUE_EVIDENCE_ITEMS,
            )?;
            if finding.evidence.is_empty() {
                return Err(format!(
                    "specialist_report finding {} must include evidence",
                    finding.finding_id.as_str()
                ));
            }
            for evidence in &finding.evidence {
                if !sources.contains(&evidence.source_uri) {
                    return Err(format!(
                        "specialist_report finding {} cites evidence outside report sources",
                        finding.finding_id.as_str()
                    ));
                }
                validate_required_text(
                    "specialist_report.findings.evidence.excerpt_md",
                    &evidence.excerpt_md,
                )?;
                validate_max_chars(
                    "specialist_report.findings.evidence.excerpt_md",
                    &evidence.excerpt_md,
                    MAX_CITATION_QUOTE_CHARS,
                )?;
            }
            if !finding.confidence.is_finite() || !(0.0..=1.0).contains(&finding.confidence) {
                return Err(format!(
                    "specialist_report finding {} has invalid confidence",
                    finding.finding_id.as_str()
                ));
            }
            if let Some(position) = &finding.decision_position {
                validate_required_text(
                    "specialist_report.findings.decision_position.alternative",
                    &position.alternative,
                )?;
                validate_max_chars(
                    "specialist_report.findings.decision_position.alternative",
                    &position.alternative,
                    MAX_DECISION_ALTERNATIVE_CHARS,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesisOperationOrigin {
    pub operation_id: ChangeOperationId,
    pub finding_ids: Vec<ContractId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SynthesisDecisionOrigin {
    pub decision_point_id: DecisionPointId,
    pub finding_ids: Vec<ContractId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeepSynthesis {
    pub draft: ChangeSetDraft,
    pub operation_origins: Vec<SynthesisOperationOrigin>,
    pub decision_origins: Vec<SynthesisDecisionOrigin>,
}

impl DeepSynthesis {
    fn validate(&self) -> Result<(), String> {
        let operation_ids = self
            .draft
            .operations()
            .iter()
            .map(ChangeOperation::operation_id)
            .collect::<HashSet<_>>();
        if operation_ids.is_empty() {
            return Err("deep_synthesis.draft must include at least one operation".to_owned());
        }
        if self.operation_origins.len() != operation_ids.len() {
            return Err(
                "deep_synthesis.operation_origins must map every draft operation exactly once"
                    .to_owned(),
            );
        }
        let mut mapped_operations = HashSet::with_capacity(self.operation_origins.len());
        for origin in &self.operation_origins {
            if !operation_ids.contains(&origin.operation_id)
                || !mapped_operations.insert(origin.operation_id)
            {
                return Err(
                    "deep_synthesis.operation_origins contains an unknown or duplicate operation"
                        .to_owned(),
                );
            }
            validate_origin_findings(
                "deep_synthesis.operation_origins.finding_ids",
                &origin.finding_ids,
                false,
            )?;
        }

        let decision_ids = self
            .draft
            .decisions()
            .iter()
            .map(|decision| decision.decision_point_id())
            .collect::<HashSet<_>>();
        if self.decision_origins.len() != decision_ids.len() {
            return Err(
                "deep_synthesis.decision_origins must map every draft decision exactly once"
                    .to_owned(),
            );
        }
        let mut mapped_decisions = HashSet::with_capacity(self.decision_origins.len());
        for origin in &self.decision_origins {
            if !decision_ids.contains(&origin.decision_point_id)
                || !mapped_decisions.insert(origin.decision_point_id)
            {
                return Err(
                    "deep_synthesis.decision_origins contains an unknown or duplicate decision"
                        .to_owned(),
                );
            }
            validate_origin_findings(
                "deep_synthesis.decision_origins.finding_ids",
                &origin.finding_ids,
                true,
            )?;
        }
        Ok(())
    }
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

pub fn parse_specialist_report(payload: &str) -> Result<SpecialistReport, StructuredOutputError> {
    parse_contract(payload, "specialist_report", SpecialistReport::validate)
}

pub fn parse_deep_synthesis(payload: &str) -> Result<DeepSynthesis, StructuredOutputError> {
    let synthesis = parse_contract(payload, "deep_synthesis", DeepSynthesis::validate)?;
    let validated_draft =
        reconstruct_change_set_draft(synthesis.draft.clone()).map_err(|error| {
            StructuredOutputError::invalid_content("deep_synthesis", payload, error.to_string())
        })?;
    validate_document_content_references(&validated_draft).map_err(|error| {
        StructuredOutputError::invalid_content("deep_synthesis", payload, error)
    })?;
    Ok(DeepSynthesis {
        draft: validated_draft,
        ..synthesis
    })
}

pub fn parse_import_extraction(payload: &str) -> Result<ImportExtraction, StructuredOutputError> {
    parse_contract(payload, "import_extraction", ImportExtraction::validate)
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
    if validated.decisions().len() > MAX_PROPOSAL_DECISION_POINTS {
        return Err(StructuredOutputError::invalid_content(
            "change_set_draft",
            payload,
            format!(
                "AI proposals cannot include more than {MAX_PROPOSAL_DECISION_POINTS} decision points"
            ),
        ));
    }
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

fn validate_text_items(field: &str, items: &[String], max_items: usize) -> Result<(), String> {
    validate_max_items(field, items.len(), max_items)?;
    for item in items {
        if item.trim().is_empty() {
            return Err(format!("{field} cannot contain empty text"));
        }
        validate_max_chars(field, item, MAX_RESOLUTION_CHARS)?;
    }
    Ok(())
}

fn validate_origin_findings(
    field: &str,
    finding_ids: &[ContractId],
    requires_disagreement: bool,
) -> Result<(), String> {
    if finding_ids.is_empty() {
        return Err(format!("{field} must cite at least one specialist finding"));
    }
    if requires_disagreement && finding_ids.len() < 2 {
        return Err(format!(
            "{field} must cite at least two findings for a decision point"
        ));
    }
    validate_unique_items(field, finding_ids.iter().cloned())
}

fn validate_import_candidate(candidate: &ImportCandidate) -> Result<(), String> {
    let citations = candidate.citations();
    validate_max_items(
        "import_candidate.citations",
        citations.len(),
        MAX_IMPORT_CITATIONS,
    )?;
    if citations.is_empty() {
        return Err(format!(
            "import candidate {} must cite at least one chunk",
            candidate.candidate_id().as_str()
        ));
    }
    if !candidate.technical_confidence().is_finite()
        || !(0.0..=1.0).contains(&candidate.technical_confidence())
    {
        return Err(format!(
            "import candidate {} has invalid technical confidence",
            candidate.candidate_id().as_str()
        ));
    }
    for citation in citations {
        if !citation.source_hash.starts_with("sha256:") || citation.source_hash.len() != 71 {
            return Err("import citation requires a complete sha256 source hash".to_owned());
        }
        validate_required_text("import_candidate.citations.excerpt", &citation.excerpt)?;
        validate_max_chars(
            "import_candidate.citations.excerpt",
            &citation.excerpt,
            MAX_CITATION_QUOTE_CHARS,
        )?;
    }
    match candidate {
        ImportCandidate::Entity {
            name,
            aliases,
            summary,
            ..
        } => {
            validate_required_text("import_candidate.name", name)?;
            validate_max_chars("import_candidate.name", name, MAX_RESOLUTION_CHARS)?;
            validate_text_items("import_candidate.aliases", aliases, MAX_RELATED_OBJECT_URIS)?;
            validate_max_chars("import_candidate.summary", summary, MAX_ADVISORY_TEXT_CHARS)?;
        }
        ImportCandidate::Relation {
            source_name,
            target_name,
            relation_kind,
            ..
        } => {
            validate_required_text("import_candidate.source_name", source_name)?;
            validate_required_text("import_candidate.target_name", target_name)?;
            validate_required_text("import_candidate.relation_kind", relation_kind)?;
        }
        ImportCandidate::Event {
            summary,
            body_md,
            participant_names,
            ..
        } => {
            validate_required_text("import_candidate.summary", summary)?;
            validate_max_chars("import_candidate.body_md", body_md, MAX_ADVISORY_TEXT_CHARS)?;
            validate_text_items(
                "import_candidate.participant_names",
                participant_names,
                MAX_RELATED_OBJECT_URIS,
            )?;
        }
        ImportCandidate::Claim {
            subject_name,
            content_md,
            predicate_key,
            object_scalar,
            ..
        } => {
            validate_required_text("import_candidate.subject_name", subject_name)?;
            validate_required_text("import_candidate.content_md", content_md)?;
            if predicate_key.is_some() != object_scalar.is_some() {
                return Err(
                    "import claim predicate_key and object_scalar must appear together".to_owned(),
                );
            }
        }
        ImportCandidate::Rule {
            statement_md,
            scope,
            ..
        } => {
            validate_required_text("import_candidate.statement_md", statement_md)?;
            validate_required_text("import_candidate.scope", scope)?;
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
