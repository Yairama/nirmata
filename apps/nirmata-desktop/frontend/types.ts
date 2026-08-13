export type World = {
  id: string;
  name: string;
  premise_md: string;
  epoch_label: string;
  calendar?: WorldCalendar | null;
  current_revision: string;
  created_at_ms: number;
  updated_at_ms: number;
};

export type WorldSession = {
  path: string;
  world_id: string;
  current_revision: string;
  world: World;
  active_variant: Variant;
  read_scope: ReadScope;
  read_only: boolean;
};
export type CalendarMonth = {
  name: string;
  days: number;
};

export type WorldCalendar = {
  name: string;
  epoch_tick: number;
  ticks_per_day: number;
  weekday_names: string[];
  months: CalendarMonth[];
};

export type ReadScope = {
  variantId: string;
  revisionId: string | null;
};

export type Variant = {
  id: string;
  worldId: string;
  name: string;
  headRevisionId: string;
  archived: boolean;
  createdFromRevisionId: string;
  createdAtMs: number;
};

export type VariantDiff = {
  objectRef: ObjectRef;
  kind: "created" | "deleted" | "renamed" | "edited" | "relation_diverged";
  before: unknown | null;
  after: unknown | null;
  leftScope: ReadScope;
  rightScope: ReadScope;
  leftSource: VariantDiffSource | null;
  rightSource: VariantDiffSource | null;
  affectedReferences: ObjectRef[];
};

export type VariantDiffSource = {
  revisionId: string;
  changeSetId: string;
  operationId: string;
  retcon: "additive" | "reinterpretive" | "replacement";
  auditSource: string;
  scope: ReadScope;
};

export type VariantComparison = {
  left: ReadScope;
  right: ReadScope;
  differences: VariantDiff[];
};

export type MergeReviewResult = {
  sourceScope: ReadScope;
  destinationScope: ReadScope;
  commonAncestorRevision: string;
  automaticOperationIds: string[];
  decisionOperationIds: string[];
  review: ManualReviewSnapshot;
};

export type SimulationResource = {
  resourceId: string;
  unit: string;
};

export type SimulationStock = {
  factionId: string;
  resourceId: string;
  quantity: number;
  capacity: number;
};

export type SimulationRule =
  | { kind: "production"; faction_id: string; resource_id: string; amount: number }
  | { kind: "consumption"; faction_id: string; resource_id: string; amount: number }
  | {
      kind: "transfer";
      from_faction_id: string;
      to_faction_id: string;
      resource_id: string;
      amount: number;
    };

export type SimulationScenarioInput = {
  worldId: string;
  variantId: string;
  baseRevision: string;
  factions: string[];
  resources: SimulationResource[];
  stocks: SimulationStock[];
  rules: SimulationRule[];
  maxSteps: number;
  assumptions: string[];
};

export type SimulationScenario = SimulationScenarioInput & {
  id: string;
};

export type SimulationTransition = {
  step: number;
  ruleIndex: number;
  rule: SimulationRule;
  before: SimulationStock[];
  after: SimulationStock[];
  requested: number;
  applied: number;
  shortage: number;
};

export type SimulationRun = {
  scenarioId: string;
  worldId: string;
  variantId: string;
  baseRevision: string;
  stepsCompleted: number;
  assumptions: string[];
  transitions: SimulationTransition[];
  finalStocks: SimulationStock[];
};

export type SimulationTransitionSelection =
  | { kind: "create_event"; step: number; ruleIndex: number; summary: string; tick: number | null }
  | {
      kind: "create_claim";
      step: number;
      ruleIndex: number;
      subjectEntityId: string;
      content: string;
      tick: number | null;
    };

export type SearchAuthority = "canonical" | "perspective";
export type SearchClassification =
  | "fact"
  | "perspective"
  | "inference"
  | "no_evidence"
  | "unspecified";
export type ContextStage = "selection" | "relation" | "temporal" | "goal" | "perspective" | "search" | "semantic";
export type SearchKind = "all" | "entity" | "relation" | "event" | "claim" | "rule" | "goal" | "document";
export type SearchObjectKind = Exclude<SearchKind, "all">;
export type ObjectKind = SearchObjectKind | "world";
export type ValidationSeverity = "error" | "conflict" | "warning" | "info";

export type SearchResult = {
  object_ref: ObjectRef;
  object_type: ObjectKind;
  object_id: string;
  uri: string;
  snippet: string;
  authority: SearchAuthority;
  classification: SearchClassification;
  provenance: string;
  stage: string;
  score: number;
  rank: number;
  score_explanation: string;
};

export type SearchAbsence = {
  classification: SearchClassification;
  provenance: string;
};

export type SearchWorldResponse = {
  hits: SearchResult[];
  absence: SearchAbsence | null;
};

export type RelatedContextEntry = {
  result: SearchResult;
  stage: ContextStage;
};

export type ContextBudgetUsage = {
  max_objects: number;
  max_chars: number;
  used_objects: number;
  used_chars: number;
};

export type RelatedContextResponse = {
  canon: RelatedContextEntry[];
  perspectives: RelatedContextEntry[];
  desires: RelatedContextEntry[];
  obligations: RelatedContextEntry[];
  search_evidence: RelatedContextEntry[];
  usage: ContextBudgetUsage;
  absence: SearchAbsence | null;
};

export type ValidationIssue = {
  code: string;
  severity: ValidationSeverity;
  objects: Array<{ kind: string; id: string }>;
  message: string;
};

export type ValidationReport = {
  errors: ValidationIssue[];
  conflicts: ValidationIssue[];
  warnings: ValidationIssue[];
  info: ValidationIssue[];
};

export type ObjectRef =
  | { world: string }
  | { entity: string }
  | { relation: string }
  | { event: string }
  | { claim: string }
  | { rule: string }
  | { goal: string }
  | { document: string };

export type ContentReference = {
  source: ObjectRef;
  target: ObjectRef;
  ordinal: number;
};

export type EventTime = {
  kind: "unknown" | "instant" | "interval" | "ongoing";
  start_tick: number | null;
  end_tick: number | null;
  precision: "exact" | "day" | "month" | "year" | "era" | "unknown";
  certainty: "certain" | "approximate" | "uncertain" | "approximate_uncertain";
};

export type Period = {
  start_tick: number | null;
  end_tick: number | null;
};

export type Entity = {
  id: string;
  world_id: string;
  kind: string;
  name: string;
  slug: string;
  summary: string;
  body_md: string;
  attributes_json: string;
  aliases: string[];
  version: number;
  created_at_ms: number;
  updated_at_ms: number;
};

export type Relation = {
  id: string;
  world_id: string;
  source_entity_id: string;
  target_entity_id: string;
  kind: string;
  direction: "directed" | "undirected";
  valid_from_tick: number | null;
  valid_to_tick: number | null;
  certainty: "certain" | "approximate" | "uncertain" | "approximate_uncertain";
  source_reference: string | null;
  metadata_json: string;
  version: number;
};

export type EventParticipant = {
  entity_id: string;
  role: string;
  ordinal: number;
};

export type EventLink = {
  source_event_id: string;
  target_event_id: string;
  kind: string;
};

export type EventObject = {
  id: string;
  world_id: string;
  kind: string;
  summary: string;
  body_md: string;
  time: EventTime;
  location_entity_id: string | null;
  participants: EventParticipant[];
  affected_goal_ids: string[];
  version: number;
  created_at_ms: number;
  updated_at_ms: number;
};

export type EventAggregate = {
  event: EventObject;
  links: EventLink[];
};

export type ClaimObject = { entity: string } | { scalar: string };

export type Claim = {
  id: string;
  world_id: string;
  subject_entity_id: string;
  content_md: string;
  predicate_key: string | null;
  object: ClaimObject | null;
  polarity: "positive" | "negative";
  authentication: "canonical" | "attributed" | "disputed";
  holder_entity_id: string | null;
  modality: "assertion" | "belief" | "hypothesis" | "counterfactual" | null;
  register: string | null;
  epistemic_basis: string | null;
  source: string | null;
  source_document_id: string | null;
  source_claim_id: string | null;
  holder_confidence: number | null;
  period: Period | null;
  registered_revision_id: string;
  superseded_revision_id: string | null;
  version: number;
};

export type Rule = {
  id: string;
  world_id: string;
  kind: string;
  statement_md: string;
  scope: string;
  severity: "advisory" | "hard";
  source: string | null;
  validator_kind: string | null;
  parameters_json: string;
  version: number;
  created_at_ms: number;
  updated_at_ms: number;
};

export type Goal = {
  id: string;
  world_id: string;
  holder_entity_id: string;
  desired_state_md: string;
  priority: number;
  status: "active" | "achieved" | "abandoned" | "frustrated";
  period: Period | null;
  visibility: "public" | "secret";
  source: string | null;
  version: number;
};

export type DocumentObject = {
  id: string;
  world_id: string;
  title: string;
  kind: string;
  author_entity_id: string | null;
  perspective_entity_id: string | null;
  canon_status: "canonical" | "non_canonical";
  body_md: string;
  version: number;
  created_at_ms: number;
  updated_at_ms: number;
};

export type CanonAggregate<T> = {
  object: T;
  references: ContentReference[];
};

export type ResolvedObject =
  | { world: World }
  | { entity: Entity }
  | { relation: Relation }
  | { event: EventAggregate }
  | { claim: Claim }
  | { rule: Rule }
  | { goal: Goal }
  | { document: CanonAggregate<DocumentObject> };

export type OpenUriResponse = {
  result: SearchResult;
  object: ResolvedObject;
};

export type LogicalVfsObject = {
  type: "object";
  name: string;
  object: ObjectRef;
  uri: string;
};

export type LogicalVfsDirectory = {
  type?: "directory";
  name: string;
  children: LogicalVfsNode[];
};

export type LogicalVfsNode = LogicalVfsObject | ({ type: "directory" } & LogicalVfsDirectory);

export type ManualFieldIssue = {
  field: string;
  message: string;
};

export type ManualDraftPreview = {
  draftKey: string;
  targetUri: string;
  objectType: ObjectKind;
  mode: "create" | "update";
  title: string;
  objective: string;
  sourceUris: string[];
  assumptions: string[];
  logicalPath: string;
  validationReport: ValidationReport;
  readyToConfirm: boolean;
};

export type ManualDraftResponse = {
  draft: ManualDraftPreview | null;
  review: ManualReviewSnapshot | null;
  fieldIssues: ManualFieldIssue[];
};

export type ManualReviewLineItem = {
  label: string;
  value: string;
};

export type ManualReviewObjectSnapshot = {
  title: string;
  objectType: string;
  targetUri: string;
  lines: ManualReviewLineItem[];
};

export type ManualReviewWaiverSnapshot = {
  issueCode: string;
  rationale: string;
  createdAtMs: number;
};

export type ManualReviewFreshnessSnapshot = {
  status: "current" | "stale" | "refresh_restart_required";
  currentRevision: string;
  canRevalidate: boolean;
  message: string;
};

export type ManualReviewRiskTriggerSnapshot = {
  code: string;
  title: string;
  detail: string;
};

export type ManualReviewRiskSnapshot = {
  requiresJudgment: boolean;
  judgment: string | null;
  suggestedResolutionAvailable: boolean;
  suggestedResolutionHidden: boolean;
  triggers: ManualReviewRiskTriggerSnapshot[];
};

export type ManualReviewDecisionPointSnapshot = {
  decisionPointId: string;
  prompt: string;
  alternatives: string[];
  replacementTarget: string | null;
  suggestionAvailable: boolean;
  suggestionHidden: boolean;
  reason: string | null;
  resolvedAlternative: string | null;
};

export type ManualReviewOperationSnapshot = {
  operationId: string;
  decision: "accept" | "edit" | "reject";
  selected: boolean;
  severity: ValidationSeverity;
  targetUri: string;
  dependencies: string[];
  before: ManualReviewObjectSnapshot | null;
  after: ManualReviewObjectSnapshot | null;
  issues: ValidationReport;
  effectiveIssues: ValidationReport;
  waivers: ManualReviewWaiverSnapshot[];
  decisionPoints: ManualReviewDecisionPointSnapshot[];
  risk: ManualReviewRiskSnapshot;
};

export type ManualReviewSnapshot = {
  reviewKey: string;
  objective: string;
  sources: string[];
  assumptions: string[];
  baseRevision: string;
  operations: ManualReviewOperationSnapshot[];
  validationReport: ValidationReport;
  effectiveReport: ValidationReport;
  readyToConfirm: boolean;
  freshness: ManualReviewFreshnessSnapshot;
};

export type ManualReviewActionRequest =
  | { kind: "accept"; operationId: string }
  | { kind: "record_judgment"; operationId: string; judgment: string }
  | { kind: "resolve_decision"; decisionPointId: string; alternative: string }
  | { kind: "reject"; operationId: string }
  | { kind: "add_waiver"; operationId: string; issueCode: string; rationale: string };

export type TimelineEventEntry = {
  uri: string;
  summary: string;
  kind: string;
  time: EventTime;
  startCalendar: CalendarTickPresentation | null;
  endCalendar: CalendarTickPresentation | null;
};

export type CalendarTickPresentation = {
  tick: number;
  label: string;
  dateInput: string;
};

export type TimelineOverview = {
  known: TimelineEventEntry[];
  unknown: TimelineEventEntry[];
};

export type NarrativeObjectReference = {
  objectRef: ObjectRef;
  uri: string;
};

export type NarrativeTimelineEvent = {
  event: NarrativeObjectReference;
  summary: string;
  time: EventTime;
  evidenceUris: string[];
};

export type NarrativeTimeline = {
  scope: ReadScope;
  storyTime: NarrativeTimelineEvent[];
  unknownStoryTime: NarrativeTimelineEvent[];
  discourseOrder: Array<{
    source: NarrativeObjectReference;
    events: Array<{
      event: NarrativeObjectReference;
      ordinal: number;
      evidenceUris: string[];
    }>;
  }>;
};

export type NarrativeCausalThreads = {
  scope: ReadScope;
  maxDepth: number;
  limit: number;
  threads: Array<{
    start: NarrativeObjectReference;
    links: Array<{
      depth: number;
      kind: string;
      source: NarrativeObjectReference;
      target: NarrativeObjectReference;
      evidenceUris: string[];
    }>;
  }>;
};

export type NarrativeLooseEnds = {
  scope: ReadScope;
  findings: Array<{
    code: string;
    message: string;
    objectRefs: ObjectRef[];
    evidenceUris: string[];
  }>;
};

export type NarrativeContinuitySelection =
  | { kind: "loose_end"; code: string; objectRef: ObjectRef }
  | { kind: "causal_thread"; startEventId: string };

export type NarrativeContinuityExploration = {
  scope: ReadScope;
  selection: NarrativeContinuitySelection;
  question: string;
  alternatives: Array<{
    id: string;
    title: string;
    consequence: string;
    proposalRequest: string;
  }>;
  sourceUris: string[];
};

export type NarrativeContinuityProposal = {
  exploration: NarrativeContinuityExploration;
  selectedAlternativeId: string;
  run: AiRunSnapshot;
};

export type InternalDocumentKind = "chronicle" | "letter" | "report" | "myth" | "short_story";

export type RevisionAuditOperationSnapshot = {
  operationId: string;
  targetUri: string;
  decision: "accept" | "edit" | "reject";
  source: string;
  decidedAtMs: number;
  before: ManualReviewObjectSnapshot | null;
  after: ManualReviewObjectSnapshot | null;
  waivers: ManualReviewWaiverSnapshot[];
};

export type RevisionHistoryEntrySnapshot = {
  revisionId: string;
  parentRevisionId: string | null;
  changeSetId: string;
  author: string;
  summary: string;
  createdAtMs: number;
  undoneRevisionId: string | null;
  isCurrentHead: boolean;
  isCurrentUndoTarget: boolean;
  operations: RevisionAuditOperationSnapshot[];
  waivers: ManualReviewWaiverSnapshot[];
};

export type RevisionHistorySnapshot = {
  currentHeadRevisionId: string;
  undoTargetRevisionId: string | null;
  revisions: RevisionHistoryEntrySnapshot[];
};

export type ManualDraftRequest = {
  objectType: ObjectKind;
  existingUri?: string;
  objective?: string;
  sourceUris: string[];
  assumptions: string[];
  values: Record<string, string>;
};

export type ProviderCredentialStatus = {
  configured: boolean;
  source: "missing" | "system_secure_store" | "session_environment" | "session_memory";
  persistence: "none" | "system_secure_store" | "session";
  secureStoreAvailable: boolean;
  limitation: string | null;
};

export type ExportSnapshotResult = {
  path: string;
  worldId: string;
  baseRevision: string;
  logicalHash: string;
  objectCount: number;
  variantId: string;
  variant: string;
};

export type ImportSnapshotResult = {
  path: string;
  logicalHash: string;
  objectCount: number;
  createdCount: number;
  updatedCount: number;
  deletedCount: number;
  review: ManualReviewSnapshot;
};

export type AiQueryItem = {
  itemId: string;
  classification: "fact" | "perspective" | "inference" | "no_evidence" | "unspecified";
  markdown: string;
  contentReferences: SearchResult[];
  citations: Array<{ quoteMd: string; source: SearchResult }>;
};

export type AiQueryResponse = {
  request: string;
  snapshot: {
    worldId: string;
    baseRevision: string;
    readScope: ReadScope;
  };
  items: AiQueryItem[];
  proposalAction: {
    action: "start_proposal";
    label: string;
    request: string;
  } | null;
};

export type AiRunSnapshot = {
  id: string;
  baseRevision: string;
  request: string;
  status:
    | "running"
    | "intent_brief_ready"
    | "awaiting_review"
    | "awaiting_final_critique"
    | "ready_to_commit"
    | "committed"
    | "failed"
    | "cancelled";
  draft: {
    objective: string;
    assumptions: string[];
    operations: unknown[];
    decisions: Array<{
      decision_point_id: string;
      prompt: string;
      alternatives: string[];
      resolved_alternative: string | null;
    }>;
  } | null;
  validationReport: ValidationReport | null;
  critiqueReport: {
    issues: Array<{
      issueId: string;
      summary: { markdown: string };
      severity: ValidationSeverity;
      affectedOperationIds: string[];
      evidence: Array<{ sourceUri: string; excerptMd: string }>;
    }>;
  } | null;
  repairCount: number;
  reviewKey: string | null;
  intentBrief: {
    userRequest: string;
    objective: string;
    scope: string;
    entities: SearchResult[];
    restrictions: string[];
    reason: string;
  } | null;
  error: string | null;
};

export type ImportCitation = {
  chunkId: string;
  sourceId: string;
  sourceHash: string;
  excerpt: string;
};

type ImportCandidateBase = {
  candidateId: string;
  contradictionKey: string | null;
  citations: ImportCitation[];
  technicalConfidence: number;
};

export type ImportCandidate =
  | (ImportCandidateBase & { kind: "entity"; name: string; entityKind: string; aliases: string[]; summary: string })
  | (ImportCandidateBase & { kind: "relation"; sourceName: string; targetName: string; relationKind: string; direction: string })
  | (ImportCandidateBase & { kind: "event"; summary: string; bodyMd: string; participantNames: string[] })
  | (ImportCandidateBase & { kind: "claim"; subjectName: string; contentMd: string; predicateKey: string | null; objectScalar: string | null; polarity: string; authentication: string })
  | (ImportCandidateBase & { kind: "rule"; statementMd: string; scope: string });

export type ImportChunkSnapshot = {
  id: string;
  sourceId: string;
  sourceHash: string;
  ordinal: number;
  byteStart: number;
  byteEnd: number;
  lineStart: number;
  lineEnd: number;
  heading: string | null;
  content: string;
};

export type ImportBatchSnapshot = {
  id: string;
  worldId: string;
  variantId: string;
  targetRevision: string;
  status: string;
  sources: Array<{
    id: string;
    path: string;
    fileName: string;
    format: "markdown" | "text";
    contentHash: string;
    sizeBytes: number;
    status: string;
    preview: string;
    chunks: ImportChunkSnapshot[];
  }>;
};

export type ImportCandidateSnapshot = {
  id: string;
  candidate: ImportCandidate;
  resolvedSourceCandidateId: string | null;
  resolvedTargetCandidateId: string | null;
  status: "pending" | "selected" | "rejected";
  identitySuggestion: "exact" | "ambiguous" | "new";
  identityMatches: Array<{ uri: string; name: string }>;
  identityDecision: "exact" | "ambiguous" | "new" | null;
  canonicalUri: string | null;
};

export type ImportReviewPreparation = {
  batchId: string;
  run: AiRunSnapshot | null;
  reviewKey: string | null;
  decisionPoints: Array<{ candidateId: string; prompt: string; alternatives: string[] }>;
  traces: Array<{ candidateId: string; operationUri: string; chunkIds: string[] }>;
};

export type SpecialistRole =
  | "economist"
  | "historian"
  | "political_scientist"
  | "anthropologist"
  | "theologian"
  | "geographer"
  | "temporal_auditor"
  | "rules_auditor"
  | "causal_auditor"
  | "perspectives_auditor";

export type DeepReviewPlan = {
  mode: "deep_impact" | "audit";
  request: string;
  roles: SpecialistRole[];
  selectionSource: "explicit" | "rule_based";
  reason: string;
  confirmed: boolean;
  budget: {
    maxSpecialists: number;
    maxSpecialistCalls: number;
    maxSynthesisCalls: number;
    maxContextExpansions: number;
    maxReadToolCalls: number;
    maxNestedDelegations: number;
    specialistMaxOutputTokens: number;
    synthesisMaxOutputTokens: number;
    specialistTimeoutMs: number;
  };
};

export type SpecialistFinding = {
  findingId: string;
  summary: { markdown: string; contentReferences: string[] };
  affectedObjectUris: string[];
  candidateConsequences: Array<{ markdown: string; contentReferences: string[] }>;
  assumptions: string[];
  evidence: Array<{ sourceUri: string; excerptMd: string }>;
  confidence: number;
  unresolvedQuestions: string[];
  decisionPosition: { decisionKey: string; alternative: string } | null;
};

export type DeepReviewRun = {
  id: string;
  baseRevision: string;
  mode: "deep_impact" | "audit";
  request: string;
  status: "running" | "synthesizing" | "awaiting_review" | "completed_audit" | "failed" | "cancelled";
  plan: DeepReviewPlan;
  specialists: Array<{
    role: SpecialistRole;
    status: "pending" | "running" | "completed" | "timed_out" | "failed" | "cancelled";
    report: { specialist: SpecialistRole; sources: string[]; findings: SpecialistFinding[] } | null;
    error: string | null;
    elapsedMs: number;
    inputTokens: number | null;
    outputTokens: number | null;
  }>;
  synthesis: {
    draft: AiRunSnapshot["draft"];
    operationOrigins: Array<{ operationId: string; findingIds: string[] }>;
    decisionOrigins: Array<{ decisionPointId: string; findingIds: string[] }>;
  } | null;
  auditResult: { validationReport: ValidationReport; findingIds: string[] } | null;
  standardRunId: string | null;
  error: string | null;
};

export type EditorControl = "text" | "textarea" | "number" | "select";

export type EditorField = {
  key: string;
  label: string;
  control: EditorControl;
  value: string;
  required?: boolean;
  placeholder?: string;
  rows?: number;
  options?: Array<{ value: string; label: string }>;
  help?: string;
};

export type EditorMeta = {
  label: string;
  value: string;
  uri?: string;
};

export type WarningItem = {
  title: string;
  detail: string;
};

export type JumpLink = {
  uri: string;
  label: string;
};

export type ReviewEditContext = {
  reviewKey: string;
  operationId: string;
};

export type EditorMode = {
  mode: "create" | "update";
  objectType: ObjectKind;
  existingUri: string | null;
  targetUri: string | null;
  title: string;
  subtitle: string;
  description: string;
  logicalPath: string | null;
  fields: EditorField[];
  values: Record<string, string>;
  baselineValues: Record<string, string>;
  objective: string;
  baselineObjective: string;
  sourceUrisText: string;
  baselineSourceUrisText: string;
  assumptionsText: string;
  baselineAssumptionsText: string;
  metadata: EditorMeta[];
  warnings: WarningItem[];
  links: JumpLink[];
  issues: ManualFieldIssue[];
  reviewEdit: ReviewEditContext | null;
};

export type PendingDraftRecord = {
  preview: ManualDraftPreview;
  review: ManualReviewSnapshot;
  editor: EditorMode;
  aiRunId?: string;
};

export type WorkspaceNotice = {
  kind: "info" | "warning";
  title: string;
  detail: string;
};

export type AiActivity = {
  requestId: string;
  source: "assistant" | "lore" | "narrative" | "unknown";
  label: string;
};

export type AiActivitySnapshot = {
  busy: boolean;
  requestIds: string[];
};

export type RecentProject = {
  path: string;
  name: string;
  worldId: string;
  lastOpenedMs: number;
};

export type AiProviderDiagnosticStatus = {
  state:
    | "credential_missing"
    | "endpoint_missing"
    | "endpoint_invalid"
    | "model_missing"
    | "connection_unchecked"
    | "connected";
  message: string;
  canCheckConnection: boolean;
  connected: boolean;
  credential: ProviderCredentialStatus;
};

export type AppState = {
  session: WorldSession | null;
  logicalTree: LogicalVfsDirectory | null;
  selectedUri: string | null;
  selectedLogicalPath: string | null;
  selectedObject: OpenUriResponse | null;
  editorMode: EditorMode | null;
  context: RelatedContextResponse | null;
  timeline: TimelineOverview | null;
  narrative: {
    timeline: NarrativeTimeline | null;
    causalThreads: NarrativeCausalThreads | null;
    looseEnds: NarrativeLooseEnds | null;
    exploration: NarrativeContinuityExploration | null;
  };
  revisionHistory: RevisionHistorySnapshot | null;
  selectedRevisionId: string | null;
  recentUris: string[];
  pendingDrafts: Map<string, PendingDraftRecord>;
  ephemeralWork: Map<string, string>;
  workspaceNotice: WorkspaceNotice | null;
  aiActivity: AiActivity | null;
  aiProviderReady: boolean;
  panels: {
    leftCollapsed: boolean;
    rightCollapsed: boolean;
    bottomCollapsed: boolean;
    leftWidth: number;
    rightWidth: number;
    bottomHeight: number;
  };
  navigationRequestId: number;
  selectionRequestId: number;
};
