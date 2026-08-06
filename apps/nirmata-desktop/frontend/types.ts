export type World = {
  id: string;
  name: string;
  premise_md: string;
  epoch_label: string;
  current_revision: string;
  created_at_ms: number;
  updated_at_ms: number;
};

export type WorldSession = {
  path: string;
  world_id: string;
  current_revision: string;
  world: World;
};

export type SearchAuthority = "canonical" | "perspective";
export type SearchClassification =
  | "fact"
  | "perspective"
  | "inference"
  | "no_evidence"
  | "unspecified";
export type ContextStage = "selection" | "relation" | "temporal" | "goal" | "perspective" | "search";
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
  | { kind: "reject"; operationId: string }
  | { kind: "add_waiver"; operationId: string; issueCode: string; rationale: string };

export type TimelineEventEntry = {
  uri: string;
  summary: string;
  kind: string;
  time: EventTime;
};

export type TimelineOverview = {
  known: TimelineEventEntry[];
  unknown: TimelineEventEntry[];
};

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

export type DialogFilter = {
  name: string;
  extensions: string[];
};

export type TauriApi = {
  core: {
    invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  };
  dialog: {
    open(options: { multiple: false; directory: false; filters: DialogFilter[] }): Promise<string | null>;
    save(options: { defaultPath: string; filters: DialogFilter[] }): Promise<string | null>;
  };
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
};

export type WorkspaceNotice = {
  kind: "info" | "warning";
  title: string;
  detail: string;
};

export type AppState = {
  session: WorldSession | null;
  queryText: string;
  activeKind: SearchKind;
  searchHits: SearchResult[];
  logicalTree: LogicalVfsDirectory | null;
  selectedUri: string | null;
  selectedLogicalPath: string | null;
  selectedObject: OpenUriResponse | null;
  editorMode: EditorMode | null;
  context: RelatedContextResponse | null;
  timeline: TimelineOverview | null;
  revisionHistory: RevisionHistorySnapshot | null;
  selectedRevisionId: string | null;
  recentUris: string[];
  pendingDrafts: Map<string, PendingDraftRecord>;
  workspaceNotice: WorkspaceNotice | null;
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

declare global {
  interface Window {
    __TAURI__: TauriApi;
  }
}
