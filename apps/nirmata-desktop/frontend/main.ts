type World = {
  id: string;
  name: string;
  premise_md: string;
  epoch_label: string;
  current_revision: string;
  created_at_ms: number;
  updated_at_ms: number;
};

type WorldSession = {
  path: string;
  world_id: string;
  current_revision: string;
  world: World;
};

type SearchAuthority = "canonical" | "perspective";
type SearchClassification =
  | "fact"
  | "perspective"
  | "inference"
  | "no_evidence"
  | "unspecified";
type ContextStage = "selection" | "relation" | "temporal" | "goal" | "perspective" | "search";
type SearchKind = "all" | "entity" | "relation" | "event" | "claim" | "rule" | "goal" | "document";
type SearchObjectKind = Exclude<SearchKind, "all">;
type ObjectKind = SearchObjectKind | "world";
type ValidationSeverity = "error" | "conflict" | "warning" | "info";

type SearchResult = {
  object_ref: ObjectRef;
  object_type: ObjectKind;
  object_id: string;
  uri: string;
  snippet: string;
  authority: SearchAuthority;
  classification: SearchClassification;
  provenance: string;
};

type SearchAbsence = {
  classification: SearchClassification;
  provenance: string;
};

type SearchWorldResponse = {
  hits: SearchResult[];
  absence: SearchAbsence | null;
};

type RelatedContextEntry = {
  result: SearchResult;
  stage: ContextStage;
};

type ContextBudgetUsage = {
  max_objects: number;
  max_chars: number;
  used_objects: number;
  used_chars: number;
};

type RelatedContextResponse = {
  canon: RelatedContextEntry[];
  perspectives: RelatedContextEntry[];
  desires: RelatedContextEntry[];
  obligations: RelatedContextEntry[];
  search_evidence: RelatedContextEntry[];
  usage: ContextBudgetUsage;
  absence: SearchAbsence | null;
};

type ValidationIssue = {
  code: string;
  severity: ValidationSeverity;
  objects: Array<{ kind: string; id: string }>;
  message: string;
};

type ValidationReport = {
  errors: ValidationIssue[];
  conflicts: ValidationIssue[];
  warnings: ValidationIssue[];
  info: ValidationIssue[];
};

type ObjectRef =
  | { world: string }
  | { entity: string }
  | { relation: string }
  | { event: string }
  | { claim: string }
  | { rule: string }
  | { goal: string }
  | { document: string };

type ContentReference = {
  source: ObjectRef;
  target: ObjectRef;
  ordinal: number;
};

type EventTime = {
  kind: "unknown" | "instant" | "interval" | "ongoing";
  start_tick: number | null;
  end_tick: number | null;
  precision: "exact" | "day" | "month" | "year" | "era" | "unknown";
  certainty: "certain" | "approximate" | "uncertain" | "approximate_uncertain";
};

type Period = {
  start_tick: number | null;
  end_tick: number | null;
};

type Entity = {
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

type Relation = {
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

type EventParticipant = {
  entity_id: string;
  role: string;
  ordinal: number;
};

type EventLink = {
  source_event_id: string;
  target_event_id: string;
  kind: string;
};

type EventObject = {
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

type EventAggregate = {
  event: EventObject;
  links: EventLink[];
};

type ClaimObject = { entity: string } | { scalar: string };

type Claim = {
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

type Rule = {
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

type Goal = {
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

type DocumentObject = {
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

type CanonAggregate<T> = {
  object: T;
  references: ContentReference[];
};

type ResolvedObject =
  | { world: World }
  | { entity: Entity }
  | { relation: Relation }
  | { event: EventAggregate }
  | { claim: Claim }
  | { rule: Rule }
  | { goal: Goal }
  | { document: CanonAggregate<DocumentObject> };

type OpenUriResponse = {
  result: SearchResult;
  object: ResolvedObject;
};

type LogicalVfsObject = {
  type: "object";
  name: string;
  object: ObjectRef;
  uri: string;
};

type LogicalVfsDirectory = {
  type?: "directory";
  name: string;
  children: LogicalVfsNode[];
};

type LogicalVfsNode = LogicalVfsObject | ({ type: "directory" } & LogicalVfsDirectory);

type ManualFieldIssue = {
  field: string;
  message: string;
};

type ManualDraftPreview = {
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

type ManualDraftResponse = {
  draft: ManualDraftPreview | null;
  review: ManualReviewSnapshot | null;
  fieldIssues: ManualFieldIssue[];
};

type ManualReviewLineItem = {
  label: string;
  value: string;
};

type ManualReviewObjectSnapshot = {
  title: string;
  objectType: string;
  targetUri: string;
  lines: ManualReviewLineItem[];
};

type ManualReviewWaiverSnapshot = {
  issueCode: string;
  rationale: string;
  createdAtMs: number;
};

type ManualReviewFreshnessSnapshot = {
  status: "current" | "stale" | "refresh_restart_required";
  currentRevision: string;
  canRevalidate: boolean;
  message: string;
};

type ManualReviewRiskTriggerSnapshot = {
  code: string;
  title: string;
  detail: string;
};

type ManualReviewRiskSnapshot = {
  requiresJudgment: boolean;
  judgment: string | null;
  suggestedResolutionAvailable: boolean;
  suggestedResolutionHidden: boolean;
  triggers: ManualReviewRiskTriggerSnapshot[];
};

type ManualReviewDecisionPointSnapshot = {
  decisionPointId: string;
  prompt: string;
  alternatives: string[];
  replacementTarget: string | null;
  suggestionAvailable: boolean;
  suggestionHidden: boolean;
  reason: string | null;
  resolvedAlternative: string | null;
};

type ManualReviewOperationSnapshot = {
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

type ManualReviewSnapshot = {
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

type ManualReviewActionRequest =
  | { kind: "accept"; operationId: string }
  | { kind: "record_judgment"; operationId: string; judgment: string }
  | { kind: "reject"; operationId: string }
  | { kind: "add_waiver"; operationId: string; issueCode: string; rationale: string };

type TimelineEventEntry = {
  uri: string;
  summary: string;
  kind: string;
  time: EventTime;
};

type TimelineOverview = {
  known: TimelineEventEntry[];
  unknown: TimelineEventEntry[];
};

type RevisionAuditOperationSnapshot = {
  operationId: string;
  targetUri: string;
  decision: "accept" | "edit" | "reject";
  source: string;
  decidedAtMs: number;
  before: ManualReviewObjectSnapshot | null;
  after: ManualReviewObjectSnapshot | null;
  waivers: ManualReviewWaiverSnapshot[];
};

type RevisionHistoryEntrySnapshot = {
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

type RevisionHistorySnapshot = {
  currentHeadRevisionId: string;
  undoTargetRevisionId: string | null;
  revisions: RevisionHistoryEntrySnapshot[];
};

type ManualDraftRequest = {
  objectType: ObjectKind;
  existingUri?: string;
  objective?: string;
  sourceUris: string[];
  assumptions: string[];
  values: Record<string, string>;
};

type DialogFilter = {
  name: string;
  extensions: string[];
};

type TauriApi = {
  core: {
    invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  };
  dialog: {
    open(options: { multiple: false; directory: false; filters: DialogFilter[] }): Promise<string | null>;
    save(options: { defaultPath: string; filters: DialogFilter[] }): Promise<string | null>;
  };
};

type EditorControl = "text" | "textarea" | "number" | "select";

type EditorField = {
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

type EditorMeta = {
  label: string;
  value: string;
  uri?: string;
};

type WarningItem = {
  title: string;
  detail: string;
};

type JumpLink = {
  uri: string;
  label: string;
};

type ReviewEditContext = {
  reviewKey: string;
  operationId: string;
};

type EditorMode = {
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

type PendingDraftRecord = {
  preview: ManualDraftPreview;
  review: ManualReviewSnapshot;
  editor: EditorMode;
};

type WorkspaceNotice = {
  kind: "info" | "warning";
  title: string;
  detail: string;
};

type AppState = {
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

export {};

declare global {
  interface Window {
    __TAURI__: TauriApi;
  }
}

const { invoke } = window.__TAURI__.core;
const dialog = window.__TAURI__.dialog;
const filter = [{ name: "Proyecto Nirmata", extensions: ["nirmata"] }];
const kinds: Array<{ value: SearchKind; label: string }> = [
  { value: "all", label: "Todo" },
  { value: "entity", label: "Entidades" },
  { value: "relation", label: "Relaciones" },
  { value: "event", label: "Eventos" },
  { value: "claim", label: "Claims" },
  { value: "rule", label: "Reglas" },
  { value: "goal", label: "Metas" },
  { value: "document", label: "Documentos" },
];
const createKinds = kinds.filter((kind) => kind.value !== "all") as Array<{
  value: SearchObjectKind;
  label: string;
}>;

const state: AppState = {
  session: null,
  queryText: "",
  activeKind: "all",
  searchHits: [],
  logicalTree: null,
  selectedUri: null,
  selectedLogicalPath: null,
  selectedObject: null,
  editorMode: null,
  context: null,
  timeline: null,
  revisionHistory: null,
  selectedRevisionId: null,
  recentUris: [],
  pendingDrafts: new Map(),
  workspaceNotice: null,
  panels: {
    leftCollapsed: false,
    rightCollapsed: false,
    bottomCollapsed: false,
    leftWidth: 24,
    rightWidth: 22,
    bottomHeight: 15,
  },
  navigationRequestId: 0,
  selectionRequestId: 0,
};

const createForm = document.querySelector<HTMLFormElement>("#create-form")!;
const nameInput = document.querySelector<HTMLInputElement>("#name")!;
const premiseInput = document.querySelector<HTMLTextAreaElement>("#premise")!;
const epochInput = document.querySelector<HTMLInputElement>("#epoch-label")!;
const pathInput = document.querySelector<HTMLInputElement>("#create-path")!;
const chooseCreatePath = document.querySelector<HTMLButtonElement>("#choose-create-path")!;
const openButton = document.querySelector<HTMLButtonElement>("#open-button")!;
const closeButton = document.querySelector<HTMLButtonElement>("#close-button")!;
const closedView = document.querySelector<HTMLElement>("#closed-view")!;
const worldView = document.querySelector<HTMLElement>("#world-view")!;
const statusElement = document.querySelector<HTMLElement>("#status")!;
const error = document.querySelector<HTMLElement>("#error")!;
const worldName = document.querySelector<HTMLElement>("#world-name")!;
const worldPath = document.querySelector<HTMLElement>("#world-path")!;
const worldPremise = document.querySelector<HTMLElement>("#world-premise")!;
const worldEpoch = document.querySelector<HTMLElement>("#world-epoch")!;
const worldRevision = document.querySelector<HTMLElement>("#world-revision")!;
const editWorldButton = document.querySelector<HTMLButtonElement>("#edit-world-button")!;
const workspaceShell = document.querySelector<HTMLElement>("#workspace-shell")!;
const navigationPanel = document.querySelector<HTMLElement>("#navigation-panel")!;
const contextPanel = document.querySelector<HTMLElement>("#context-panel")!;
const pendingPanel = document.querySelector<HTMLElement>("#pending-panel")!;
const toggleNavigationButton = document.querySelector<HTMLButtonElement>("#toggle-navigation")!;
const toggleContextButton = document.querySelector<HTMLButtonElement>("#toggle-context")!;
const togglePendingButton = document.querySelector<HTMLButtonElement>("#toggle-pending")!;
const leftPanelSize = document.querySelector<HTMLInputElement>("#left-panel-size")!;
const rightPanelSize = document.querySelector<HTMLInputElement>("#right-panel-size")!;
const bottomPanelSize = document.querySelector<HTMLInputElement>("#bottom-panel-size")!;
const uriForm = document.querySelector<HTMLFormElement>("#uri-form")!;
const uriInput = document.querySelector<HTMLInputElement>("#uri-input")!;
const searchForm = document.querySelector<HTMLFormElement>("#search-form")!;
const searchInput = document.querySelector<HTMLInputElement>("#search-input")!;
const resultSummary = document.querySelector<HTMLElement>("#result-summary")!;
const kindFilters = document.querySelector<HTMLElement>("#kind-filters")!;
const recentsList = document.querySelector<HTMLElement>("#recents-list")!;
const recentsEmpty = document.querySelector<HTMLElement>("#recents-empty")!;
const treeRoot = document.querySelector<HTMLElement>("#tree-root")!;
const treeEmpty = document.querySelector<HTMLElement>("#tree-empty")!;
const resultsList = document.querySelector<HTMLElement>("#results-list")!;
const resultsEmpty = document.querySelector<HTMLElement>("#results-empty")!;
const editorTitle = document.querySelector<HTMLElement>("#editor-title")!;
const editorSubtitle = document.querySelector<HTMLElement>("#editor-subtitle")!;
const editorEmpty = document.querySelector<HTMLElement>("#editor-empty")!;
const editorContent = document.querySelector<HTMLElement>("#editor-content")!;
const contextSummary = document.querySelector<HTMLElement>("#context-summary")!;
const contextEmpty = document.querySelector<HTMLElement>("#context-empty")!;
const contextContent = document.querySelector<HTMLElement>("#context-content")!;
const pendingSummary = document.querySelector<HTMLElement>("#pending-summary")!;
const pendingEmpty = document.querySelector<HTMLElement>("#pending-empty")!;
const pendingContent = document.querySelector<HTMLElement>("#pending-content")!;

function setStatus(message: string): void {
  statusElement.textContent = message;
}

function setMarkdownText(element: HTMLElement, value: string | null | undefined, fallback: string): void {
  element.dataset.markdownMode = "plain-text";
  element.textContent = normalizeText(value, fallback);
}

function commandMessage(value: unknown): string {
  if (typeof value === "object" && value !== null && "message" in value) {
    return String((value as { message: unknown }).message);
  }
  return String(value);
}

function commandCode(value: unknown): string | null {
  if (typeof value === "object" && value !== null && "code" in value) {
    return String((value as { code: unknown }).code);
  }
  return null;
}

function showError(value: unknown): void {
  error.textContent = commandMessage(value);
  error.hidden = false;
}

function clearError(): void {
  error.hidden = true;
  error.textContent = "";
}

function button(text: string, className?: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.textContent = text;
  if (className) {
    element.className = className;
  }
  return element;
}

function block(className?: string): HTMLDivElement {
  const element = document.createElement("div");
  if (className) {
    element.className = className;
  }
  return element;
}

function badge(text: string, className?: string): HTMLSpanElement {
  const element = document.createElement("span");
  element.className = ["badge", className].filter(Boolean).join(" ");
  element.textContent = text;
  return element;
}

function shortId(value: string): string {
  return value.length > 8 ? value.slice(0, 8) : value;
}

function humanize(value: string): string {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function normalizeText(value: string | null | undefined, fallback = "No especificado"): string {
  const trimmed = value?.trim() ?? "";
  return trimmed.length > 0 ? trimmed : fallback;
}

function previewText(value: string | null | undefined, fallback: string): string {
  const text = normalizeText(value, fallback);
  return text.length > 120 ? `${text.slice(0, 120).trimEnd()}…` : text;
}

function formatObjectRef(reference: ObjectRef): { kind: ObjectKind; id: string; uri: string } {
  if ("world" in reference) {
    return { kind: "world", id: reference.world, uri: `nirmata://world/${reference.world}` };
  }
  if ("entity" in reference) {
    return { kind: "entity", id: reference.entity, uri: `nirmata://entity/${reference.entity}` };
  }
  if ("relation" in reference) {
    return {
      kind: "relation",
      id: reference.relation,
      uri: `nirmata://relation/${reference.relation}`,
    };
  }
  if ("event" in reference) {
    return { kind: "event", id: reference.event, uri: `nirmata://event/${reference.event}` };
  }
  if ("claim" in reference) {
    return { kind: "claim", id: reference.claim, uri: `nirmata://claim/${reference.claim}` };
  }
  if ("rule" in reference) {
    return { kind: "rule", id: reference.rule, uri: `nirmata://rule/${reference.rule}` };
  }
  if ("goal" in reference) {
    return { kind: "goal", id: reference.goal, uri: `nirmata://goal/${reference.goal}` };
  }
  return {
    kind: "document",
    id: reference.document,
    uri: `nirmata://document/${reference.document}`,
  };
}

function formatReferenceLabel(reference: ObjectRef): string {
  const parts = formatObjectRef(reference);
  return `${humanize(parts.kind)} · ${shortId(parts.id)}`;
}

function formatPeriod(period: Period | null): string {
  if (period === null) {
    return "Sin periodo";
  }
  const start = period.start_tick ?? "¿?";
  const end = period.end_tick ?? "¿?";
  return start === end ? `Tick ${start}` : `Ticks ${start} → ${end}`;
}

function formatEventTime(time: EventTime): string {
  if (time.kind === "unknown") {
    return `Tiempo desconocido · ${humanize(time.certainty)}`;
  }
  const start = time.start_tick ?? "¿?";
  const end = time.end_tick ?? "¿?";
  const range =
    time.kind === "instant"
      ? `Tick ${start}`
      : time.kind === "ongoing"
        ? `Desde ${start}`
        : `Ticks ${start} → ${end}`;
  return `${range} · ${humanize(time.precision)} · ${humanize(time.certainty)}`;
}

function formatTimestamp(value: number): string {
  return new Date(value).toLocaleString("es");
}

function retainedDraftHint(): string {
  return state.pendingDrafts.size > 0 ? " El draft sigue visible en el panel inferior." : "";
}

function formatClaimObject(object: ClaimObject | null): string {
  if (object === null) {
    return "Sin objeto";
  }
  if ("entity" in object) {
    return `Entidad ${shortId(object.entity)}`;
  }
  return object.scalar;
}

function linesToText(values: string[]): string {
  return values.join("\n");
}

function splitLines(value: string): string[] {
  return value
    .split(/\r?\n/u)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

type EditableContentReference = {
  targetUri: string;
  ordinal: number;
};

function serializeContentReferences(references: Array<EditableContentReference | ContentReference>): string {
  return references
    .map((reference, index) => {
      if ("targetUri" in reference) {
        return `${reference.targetUri}|${reference.ordinal ?? index}`;
      }
      return `${formatObjectRef(reference.target).uri}|${reference.ordinal}`;
    })
    .join("\n");
}

function parseContentReferences(value: string): EditableContentReference[] {
  return value
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line, index) => {
      const parts = line.split("|").map((part) => part.trim());
      if (parts.length === 1) {
        return { targetUri: parts[0], ordinal: index };
      }
      const [left, right] = parts;
      const leftNumber = Number(left);
      const rightNumber = Number(right);
      if (Number.isInteger(leftNumber) && !Number.isInteger(rightNumber)) {
        return { targetUri: right, ordinal: leftNumber };
      }
      if (!Number.isInteger(leftNumber) && Number.isInteger(rightNumber)) {
        return { targetUri: left, ordinal: rightNumber };
      }
      return { targetUri: left, ordinal: index };
    });
}

function normalizeContentReferences(references: EditableContentReference[]): EditableContentReference[] {
  return references.map((reference, index) => ({
    targetUri: reference.targetUri.trim(),
    ordinal: index,
  }));
}

function firstReviewIssue(report: ValidationReport): ValidationIssue | null {
  return report.errors[0] ?? report.conflicts[0] ?? report.warnings[0] ?? report.info[0] ?? null;
}

function dedupeLinks(links: JumpLink[]): JumpLink[] {
  const seen = new Set<string>();
  const unique: JumpLink[] = [];
  for (const link of links) {
    if (seen.has(link.uri)) {
      continue;
    }
    seen.add(link.uri);
    unique.push(link);
  }
  return unique;
}

function objectKindFromUri(uri: string): ObjectKind | null {
  const matched = /^nirmata:\/\/(world|entity|relation|event|claim|rule|goal|document)\//u.exec(uri);
  return matched ? (matched[1] as ObjectKind) : null;
}

function pathForUri(tree: LogicalVfsDirectory | null, uri: string): string | null {
  if (!tree) {
    return null;
  }
  for (const child of tree.children) {
    const path = pathForNode(child, uri, "");
    if (path) {
      return path;
    }
  }
  return null;
}

function pathForNode(node: LogicalVfsNode, uri: string, prefix: string): string | null {
  if (node.type === "object") {
    return node.uri === uri ? `${prefix}/${node.name}` : null;
  }
  const nextPrefix = `${prefix}/${node.name}`;
  for (const child of node.children) {
    const path = pathForNode(child, uri, nextPrefix);
    if (path) {
      return path;
    }
  }
  return null;
}

function firstUriFromTree(tree: LogicalVfsDirectory | null): string | null {
  if (!tree) {
    return null;
  }
  return firstUriFromNodes(tree.children);
}

function firstUriFromNodes(nodes: LogicalVfsNode[]): string | null {
  for (const node of nodes) {
    if (node.type === "object") {
      return node.uri;
    }
    const nested = firstUriFromNodes(node.children);
    if (nested) {
      return nested;
    }
  }
  return null;
}

function labelForUri(uri: string): string {
  const pending = state.pendingDrafts.get(uri);
  if (pending) {
    return pending.preview.title;
  }
  if (state.selectedObject?.result.uri === uri) {
    return state.selectedObject.result.snippet;
  }
  const hit = state.searchHits.find((item) => item.uri === uri);
  if (hit) {
    return hit.snippet;
  }
  const path = pathForUri(state.logicalTree, uri);
  if (path) {
    const parts = path.split("/").filter(Boolean);
    return parts[parts.length - 1] ?? uri;
  }
  return uri;
}

function selectedRevisionEntry(): RevisionHistoryEntrySnapshot | null {
  const revisions = state.revisionHistory?.revisions ?? [];
  if (revisions.length === 0) {
    return null;
  }
  return (
    revisions.find((entry) => entry.revisionId === state.selectedRevisionId)
    ?? revisions.find((entry) => entry.isCurrentUndoTarget)
    ?? revisions[0]
  );
}

function warningsForResult(result: SearchResult): WarningItem[] {
  switch (result.classification) {
    case "perspective":
      return [
        {
          title: "Resultado perspectival",
          detail: "La selección proviene de una perspectiva situada y puede no ser canon directo.",
        },
      ];
    case "inference":
      return [
        {
          title: "Resultado inferido",
          detail: "La selección depende de inferencias; revisa el contexto antes de asumirla como canon.",
        },
      ];
    case "no_evidence":
      return [
        {
          title: "Sin evidencia",
          detail: "La búsqueda no encontró respaldo suficiente para este resultado.",
        },
      ];
    default:
      return [];
  }
}

function createField(
  key: string,
  label: string,
  control: EditorControl,
  value: string,
  options: Partial<EditorField> = {},
): EditorField {
  return { key, label, control, value, ...options };
}

function enumOptions(values: string[]): Array<{ value: string; label: string }> {
  return values.map((value) => ({ value, label: humanize(value) }));
}

function createEditorMode(definition: {
  mode: "create" | "update";
  objectType: ObjectKind;
  existingUri: string | null;
  targetUri: string | null;
  title: string;
  subtitle: string;
  description: string;
  logicalPath: string | null;
  fields: EditorField[];
  metadata: EditorMeta[];
  warnings: WarningItem[];
  links: JumpLink[];
  objective: string;
  sourceUrisText: string;
  assumptionsText: string;
}): EditorMode {
  const values = Object.fromEntries(definition.fields.map((field) => [field.key, field.value]));
  return {
    ...definition,
    values,
    baselineValues: { ...values },
    baselineObjective: definition.objective,
    baselineSourceUrisText: definition.sourceUrisText,
    baselineAssumptionsText: definition.assumptionsText,
    issues: [],
    reviewEdit: null,
  };
}

function cloneEditorMode(mode: EditorMode): EditorMode {
  return {
    ...mode,
    fields: mode.fields.map((field) => ({ ...field })),
    values: { ...mode.values },
    baselineValues: { ...mode.baselineValues },
    metadata: mode.metadata.map((item) => ({ ...item })),
    warnings: mode.warnings.map((item) => ({ ...item })),
    links: mode.links.map((item) => ({ ...item })),
    issues: mode.issues.map((item) => ({ ...item })),
    reviewEdit: mode.reviewEdit ? { ...mode.reviewEdit } : null,
  };
}

function restorePendingValues(editor: EditorMode, existing: PendingDraftRecord | undefined): EditorMode {
  if (!existing) {
    return editor;
  }
  const restored = cloneEditorMode(existing.editor);
  restored.metadata = editor.metadata;
  restored.warnings = editor.warnings;
  restored.links = editor.links;
  restored.logicalPath = existing.preview.logicalPath || editor.logicalPath;
  restored.title = editor.title;
  restored.subtitle = editor.subtitle;
  restored.description = editor.description;
  return restored;
}

function currentWorldUri(): string | null {
  return state.session ? `nirmata://world/${state.session.world.id}` : null;
}

function buildWorldEditor(): EditorMode | null {
  if (!state.session) {
    return null;
  }
  const world = state.session.world;
  const worldUri = currentWorldUri()!;
  const editor = createEditorMode({
    mode: "update",
    objectType: "world",
    existingUri: worldUri,
    targetUri: worldUri,
    title: world.name,
    subtitle: `Mundo · ${shortId(world.id)}`,
    description: previewText(world.premise_md, "Edita metadatos del mundo activo como draft."),
    logicalPath: "/world",
    objective: `Update world ${world.name}`,
    sourceUrisText: worldUri,
    assumptionsText: "",
    fields: [
      createField("name", "Nombre", "text", world.name, { required: true }),
      createField("premise_md", "Premisa", "textarea", world.premise_md, { rows: 6 }),
      createField("epoch_label", "Etiqueta del epoch", "text", world.epoch_label),
    ],
    metadata: [
      { label: "URI", value: worldUri },
      { label: "Revisión base", value: world.current_revision },
      { label: "Actualizado", value: new Date(world.updated_at_ms).toLocaleString("es") },
    ],
    warnings: [],
    links: [],
  });
  return restorePendingValues(editor, state.pendingDrafts.get(worldUri));
}

function buildSelectionEditor(selection: OpenUriResponse): EditorMode {
  const { result, object } = selection;
  const warnings = warningsForResult(result);
  const sourceUrisText = result.uri;

  if ("world" in object) {
    return buildWorldEditor() ?? createEditorMode({
      mode: "update",
      objectType: "world",
      existingUri: result.uri,
      targetUri: result.uri,
      title: object.world.name,
      subtitle: `Mundo · ${shortId(object.world.id)}`,
      description: previewText(object.world.premise_md, "Mundo activo"),
      logicalPath: "/world",
      objective: `Update world ${object.world.name}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("name", "Nombre", "text", object.world.name, { required: true }),
        createField("premise_md", "Premisa", "textarea", object.world.premise_md, { rows: 6 }),
        createField("epoch_label", "Etiqueta del epoch", "text", object.world.epoch_label),
      ],
      metadata: [{ label: "URI", value: result.uri }],
      warnings,
      links: [],
    });
  }

  if ("entity" in object) {
    const entity = object.entity;
    const editor = createEditorMode({
      mode: "update",
      objectType: "entity",
      existingUri: result.uri,
      targetUri: result.uri,
      title: entity.name,
      subtitle: `${humanize(entity.kind)} · ${shortId(entity.id)}`,
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Update entity ${entity.name}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("kind", "Tipo", "select", entity.kind, {
          required: true,
          options: enumOptions(["person", "place", "faction", "culture", "resource", "concept"]),
        }),
        createField("name", "Nombre", "text", entity.name, { required: true }),
        createField("slug", "Slug", "text", entity.slug, { required: true }),
        createField("aliases", "Aliases (uno por línea)", "textarea", linesToText(entity.aliases), {
          rows: 4,
        }),
        createField("summary", "Resumen", "textarea", entity.summary, { rows: 4 }),
        createField("body_md", "Cuerpo Markdown", "textarea", entity.body_md, { rows: 7 }),
        createField("attributes_json", "Atributos JSON", "textarea", entity.attributes_json, {
          rows: 6,
          help: "Debe ser un objeto JSON.",
        }),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Versión", value: String(entity.version) },
        { label: "Actualizado", value: new Date(entity.updated_at_ms).toLocaleString("es") },
      ],
      warnings,
      links: [],
    });
    return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
  }

  if ("relation" in object) {
    const relation = object.relation;
    const links: JumpLink[] = [
      {
        uri: `nirmata://entity/${relation.source_entity_id}`,
        label: `Entidad origen · ${shortId(relation.source_entity_id)}`,
      },
      {
        uri: `nirmata://entity/${relation.target_entity_id}`,
        label: `Entidad destino · ${shortId(relation.target_entity_id)}`,
      },
    ];
    const editor = createEditorMode({
      mode: "update",
      objectType: "relation",
      existingUri: result.uri,
      targetUri: result.uri,
      title: relation.kind,
      subtitle: `${humanize(relation.direction)} · ${shortId(relation.id)}`,
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Update relation ${relation.kind}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("source_entity", "Entidad origen", "text", relation.source_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("target_entity", "Entidad destino", "text", relation.target_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("kind", "Tipo de relación", "text", relation.kind, { required: true }),
        createField("direction", "Dirección", "select", relation.direction, {
          required: true,
          options: enumOptions(["directed", "undirected"]),
        }),
        createField("certainty", "Certeza", "select", relation.certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("valid_from_tick", "Tick inicio", "number", relation.valid_from_tick?.toString() ?? ""),
        createField("valid_to_tick", "Tick fin", "number", relation.valid_to_tick?.toString() ?? ""),
        createField("source_reference", "Procedencia", "textarea", relation.source_reference ?? "", {
          rows: 3,
        }),
        createField("metadata_json", "Metadata JSON", "textarea", relation.metadata_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Versión", value: String(relation.version) },
        {
          label: "Periodo",
          value:
            relation.valid_from_tick === null && relation.valid_to_tick === null
              ? "Siempre"
              : `Ticks ${relation.valid_from_tick ?? "¿?"} → ${relation.valid_to_tick ?? "¿?"}`,
        },
      ],
      warnings:
        relation.certainty !== "certain"
          ? [
              ...warnings,
              {
                title: "Relación incierta",
                detail: `La relación está marcada como ${humanize(relation.certainty).toLowerCase()}.`,
              },
            ]
          : warnings,
      links: dedupeLinks(links),
    });
    return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
  }

  if ("event" in object) {
    const aggregate = object.event;
    const event = aggregate.event;
    const links: JumpLink[] = [];
    if (event.location_entity_id) {
      links.push({
        uri: `nirmata://entity/${event.location_entity_id}`,
        label: `Lugar · ${shortId(event.location_entity_id)}`,
      });
    }
    for (const participant of event.participants) {
      links.push({
        uri: `nirmata://entity/${participant.entity_id}`,
        label: `${humanize(participant.role)} · ${shortId(participant.entity_id)}`,
      });
    }
    for (const goalId of event.affected_goal_ids) {
      links.push({ uri: `nirmata://goal/${goalId}`, label: `Meta · ${shortId(goalId)}` });
    }
    for (const link of aggregate.links) {
      links.push({
        uri: `nirmata://event/${link.target_event_id}`,
        label: `${humanize(link.kind)} · ${shortId(link.target_event_id)}`,
      });
    }
    const eventWarnings = [...warnings];
    if (event.time.kind === "unknown") {
      eventWarnings.push({
        title: "Tiempo desconocido",
        detail: "Este evento no tiene anclaje temporal preciso.",
      });
    }
    if (event.time.certainty !== "certain") {
      eventWarnings.push({
        title: "Tiempo incierto",
        detail: `La cronología está marcada como ${humanize(event.time.certainty).toLowerCase()}.`,
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "event",
      existingUri: result.uri,
      targetUri: result.uri,
      title: normalizeText(event.summary, event.kind),
      subtitle: `${humanize(event.kind)} · ${shortId(event.id)}`,
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Update event ${normalizeText(event.summary, event.kind)}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("kind", "Tipo", "text", event.kind, { required: true }),
        createField("summary", "Resumen", "textarea", event.summary, { rows: 3, required: true }),
        createField("body_md", "Cuerpo Markdown", "textarea", event.body_md, { rows: 6 }),
        createField("time_kind", "Tipo de tiempo", "select", event.time.kind, {
          required: true,
          options: enumOptions(["unknown", "instant", "interval", "ongoing"]),
        }),
        createField("time_precision", "Precisión", "select", event.time.precision, {
          required: true,
          options: enumOptions(["exact", "day", "month", "year", "era", "unknown"]),
        }),
        createField("time_certainty", "Certeza temporal", "select", event.time.certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("start_tick", "Tick inicio", "number", event.time.start_tick?.toString() ?? ""),
        createField("end_tick", "Tick fin", "number", event.time.end_tick?.toString() ?? ""),
        createField("location_entity", "Entidad lugar", "text", event.location_entity_id ?? "", {
          help: "UUID o nirmata://entity/...",
        }),
        createField(
          "participants",
          "Participantes (entidad|rol|ordinal)",
          "textarea",
          linesToText(
            event.participants.map(
              (participant) => `${participant.entity_id}|${participant.role}|${participant.ordinal}`,
            ),
          ),
          { rows: 5 },
        ),
        createField(
          "affected_goal_ids",
          "Metas afectadas (una por línea)",
          "textarea",
          linesToText(event.affected_goal_ids),
          { rows: 3, help: "UUID o nirmata://goal/..." },
        ),
        createField(
          "causal_links",
          "Causalidad (evento|kind)",
          "textarea",
          linesToText(aggregate.links.map((link) => `nirmata://event/${link.target_event_id}|${link.kind}`)),
          {
            rows: 4,
            help: "Una línea por vínculo: nirmata://event/<id>|causes",
          },
        ),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Tiempo", value: formatEventTime(event.time) },
        { label: "Versión", value: String(event.version) },
      ],
      warnings: eventWarnings,
      links: dedupeLinks(links),
    });
    return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
  }

  if ("claim" in object) {
    const claim = object.claim;
    const links: JumpLink[] = [
      {
        uri: `nirmata://entity/${claim.subject_entity_id}`,
        label: `Sujeto · ${shortId(claim.subject_entity_id)}`,
      },
    ];
    if (claim.holder_entity_id) {
      links.push({
        uri: `nirmata://entity/${claim.holder_entity_id}`,
        label: `Holder · ${shortId(claim.holder_entity_id)}`,
      });
    }
    if (claim.source_document_id) {
      links.push({
        uri: `nirmata://document/${claim.source_document_id}`,
        label: `Documento fuente · ${shortId(claim.source_document_id)}`,
      });
    }
    if (claim.source_claim_id) {
      links.push({
        uri: `nirmata://claim/${claim.source_claim_id}`,
        label: `Claim fuente · ${shortId(claim.source_claim_id)}`,
      });
    }
    if (claim.object && "entity" in claim.object) {
      links.push({
        uri: `nirmata://entity/${claim.object.entity}`,
        label: `Objeto · ${shortId(claim.object.entity)}`,
      });
    }
    const claimWarnings = [...warnings];
    if (claim.authentication !== "canonical") {
      claimWarnings.push({
        title: "Claim no canónico",
        detail: `La autenticación actual es ${humanize(claim.authentication).toLowerCase()}.`,
      });
    }
    if (claim.superseded_revision_id !== null) {
      claimWarnings.push({
        title: "Claim reemplazado",
        detail: "Este claim ya tiene una revisión posterior que lo supera.",
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "claim",
      existingUri: result.uri,
      targetUri: result.uri,
      title: normalizeText(claim.predicate_key, "Claim"),
      subtitle: `${humanize(claim.authentication)} · ${shortId(claim.id)}`,
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Update claim ${previewText(claim.content_md, "claim")}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("subject_entity", "Entidad sujeto", "text", claim.subject_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("content_md", "Contenido Markdown", "textarea", claim.content_md, { rows: 5 }),
        createField("predicate_key", "Predicate key", "text", claim.predicate_key ?? ""),
        createField(
          "object_kind",
          "Tipo de objeto",
          "select",
          claim.object === null ? "none" : "entity" in claim.object ? "entity" : "scalar",
          {
            options: [
              { value: "none", label: "None" },
              { value: "entity", label: "Entity" },
              { value: "scalar", label: "Scalar" },
            ],
          },
        ),
        createField(
          "object_value",
          "Objeto",
          "text",
          claim.object === null ? "" : "entity" in claim.object ? claim.object.entity : claim.object.scalar,
          { help: "UUID/URI para entity, o texto para scalar." },
        ),
        createField("polarity", "Polaridad", "select", claim.polarity, {
          required: true,
          options: enumOptions(["positive", "negative"]),
        }),
        createField("authentication", "Autenticación", "select", claim.authentication, {
          required: true,
          options: enumOptions(["canonical", "attributed", "disputed"]),
        }),
        createField("holder_entity", "Holder", "text", claim.holder_entity_id ?? "", {
          help: "UUID o nirmata://entity/...",
        }),
        createField("modality", "Modalidad", "select", claim.modality ?? "", {
          options: [
            { value: "", label: "Sin modalidad" },
            ...enumOptions(["assertion", "belief", "hypothesis", "counterfactual"]),
          ],
        }),
        createField("register", "Registro", "text", claim.register ?? ""),
        createField("epistemic_basis", "Base epistémica", "textarea", claim.epistemic_basis ?? "", {
          rows: 3,
        }),
        createField("source", "Procedencia textual", "textarea", claim.source ?? "", { rows: 3 }),
        createField("source_document", "Documento fuente", "text", claim.source_document_id ?? "", {
          help: "UUID o nirmata://document/...",
        }),
        createField("source_claim", "Claim fuente", "text", claim.source_claim_id ?? "", {
          help: "UUID o nirmata://claim/...",
        }),
        createField(
          "holder_confidence",
          "Confianza holder",
          "number",
          claim.holder_confidence?.toString() ?? "",
        ),
        createField("period_start_tick", "Periodo inicio", "number", claim.period?.start_tick?.toString() ?? ""),
        createField("period_end_tick", "Periodo fin", "number", claim.period?.end_tick?.toString() ?? ""),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Objeto", value: formatClaimObject(claim.object) },
        { label: "Periodo", value: formatPeriod(claim.period) },
        { label: "Versión", value: String(claim.version) },
      ],
      warnings: claimWarnings,
      links: dedupeLinks(links),
    });
    return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
  }

  if ("rule" in object) {
    const rule = object.rule;
    const ruleWarnings = [...warnings];
    if (rule.severity === "hard") {
      ruleWarnings.push({
        title: "Regla dura",
        detail: "Puede bloquear cambios durante una revisión posterior.",
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "rule",
      existingUri: result.uri,
      targetUri: result.uri,
      title: previewText(rule.statement_md, "Regla"),
      subtitle: `${humanize(rule.kind)} · ${shortId(rule.id)}`,
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Update rule ${previewText(rule.statement_md, "rule")}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("kind", "Tipo", "select", rule.kind, {
          required: true,
          options: enumOptions(["constitutive", "generative", "institutional", "authorial"]),
        }),
        createField("statement_md", "Regla Markdown", "textarea", rule.statement_md, {
          rows: 4,
          required: true,
        }),
        createField("scope", "Scope", "text", rule.scope, { required: true }),
        createField("severity", "Severidad", "select", rule.severity, {
          required: true,
          options: enumOptions(["advisory", "hard"]),
        }),
        createField("validator_kind", "Validador", "select", rule.validator_kind ?? "", {
          options: [
            { value: "", label: "Sin validador" },
            { value: "no_resurrection", label: "No resurrection" },
          ],
        }),
        createField("source", "Procedencia", "textarea", rule.source ?? "", { rows: 3 }),
        createField("parameters_json", "Parámetros JSON", "textarea", rule.parameters_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Versión", value: String(rule.version) },
        { label: "Actualizado", value: new Date(rule.updated_at_ms).toLocaleString("es") },
      ],
      warnings: ruleWarnings,
      links: [],
    });
    return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
  }

  if ("goal" in object) {
    const goal = object.goal;
    const goalWarnings = [...warnings];
    if (goal.visibility === "secret") {
      goalWarnings.push({
        title: "Meta secreta",
        detail: "El contexto puede depender de perspectivas y no solo de canon.",
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "goal",
      existingUri: result.uri,
      targetUri: result.uri,
      title: previewText(goal.desired_state_md, "Meta"),
      subtitle: `${humanize(goal.status)} · ${shortId(goal.id)}`,
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Update goal ${previewText(goal.desired_state_md, "goal")}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("holder_entity", "Holder", "text", goal.holder_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("desired_state_md", "Estado deseado", "textarea", goal.desired_state_md, {
          rows: 4,
          required: true,
        }),
        createField("priority", "Prioridad", "number", String(goal.priority), { required: true }),
        createField("status", "Estado", "select", goal.status, {
          required: true,
          options: enumOptions(["active", "achieved", "abandoned", "frustrated"]),
        }),
        createField("visibility", "Visibilidad", "select", goal.visibility, {
          required: true,
          options: enumOptions(["public", "secret"]),
        }),
        createField("source", "Procedencia", "textarea", goal.source ?? "", { rows: 3 }),
        createField("period_start_tick", "Periodo inicio", "number", goal.period?.start_tick?.toString() ?? ""),
        createField("period_end_tick", "Periodo fin", "number", goal.period?.end_tick?.toString() ?? ""),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Periodo", value: formatPeriod(goal.period) },
        {
          label: "Holder",
          value: shortId(goal.holder_entity_id),
          uri: `nirmata://entity/${goal.holder_entity_id}`,
        },
      ],
      warnings: goalWarnings,
      links: [
        {
          uri: `nirmata://entity/${goal.holder_entity_id}`,
          label: `Holder · ${shortId(goal.holder_entity_id)}`,
        },
      ],
    });
    return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
  }

  const aggregate = object.document;
  const documentObject = aggregate.object;
  const links: JumpLink[] = aggregate.references.map((reference) => ({
    uri: formatObjectRef(reference.target).uri,
    label: formatReferenceLabel(reference.target),
  }));
  if (documentObject.author_entity_id) {
    links.push({
      uri: `nirmata://entity/${documentObject.author_entity_id}`,
      label: `Autor · ${shortId(documentObject.author_entity_id)}`,
    });
  }
  if (documentObject.perspective_entity_id) {
    links.push({
      uri: `nirmata://entity/${documentObject.perspective_entity_id}`,
      label: `Perspectiva · ${shortId(documentObject.perspective_entity_id)}`,
    });
  }
  const documentWarnings = [...warnings];
  if (documentObject.canon_status !== "canonical") {
    documentWarnings.push({
      title: "Documento no canónico",
      detail: "Su contenido debe leerse como perspectiva o apoyo, no como hecho duro.",
    });
  }
  const editor = createEditorMode({
    mode: "update",
    objectType: "document",
    existingUri: result.uri,
    targetUri: result.uri,
    title: documentObject.title,
    subtitle: `${humanize(documentObject.kind)} · ${shortId(documentObject.id)}`,
    description: result.snippet,
    logicalPath: pathForUri(state.logicalTree, result.uri),
    objective: `Update document ${documentObject.title}`,
    sourceUrisText,
    assumptionsText: "",
    fields: [
      createField("title", "Título", "text", documentObject.title, { required: true }),
      createField("kind", "Tipo", "text", documentObject.kind, { required: true }),
      createField("author_entity", "Autor", "text", documentObject.author_entity_id ?? "", {
        help: "UUID o nirmata://entity/...",
      }),
      createField("perspective_entity", "Perspectiva", "text", documentObject.perspective_entity_id ?? "", {
        help: "UUID o nirmata://entity/...",
      }),
      createField("canon_status", "Canon", "select", documentObject.canon_status, {
        required: true,
        options: enumOptions(["canonical", "non_canonical"]),
      }),
      createField("body_md", "Cuerpo Markdown", "textarea", documentObject.body_md, { rows: 8 }),
      createField(
        "content_references",
        "Referencias de contenido",
        "textarea",
        serializeContentReferences(aggregate.references),
        { rows: 6, help: "Agrega, abre y reordena referencias sin alterar el tiempo de los eventos." },
      ),
    ],
    metadata: [
      { label: "URI", value: result.uri },
      { label: "Referencias", value: String(aggregate.references.length) },
      { label: "Versión", value: String(documentObject.version) },
    ],
    warnings: documentWarnings,
    links: dedupeLinks(links),
  });
  return restorePendingValues(editor, state.pendingDrafts.get(result.uri));
}

function defaultCreateValues(kind: SearchObjectKind): Record<string, string> {
  switch (kind) {
    case "entity":
      return {
        kind: "person",
        name: "",
        slug: "",
        aliases: "",
        summary: "",
        body_md: "",
        attributes_json: "{}",
      };
    case "relation":
      return {
        source_entity: state.selectedUri && objectKindFromUri(state.selectedUri) === "entity" ? state.selectedUri : "",
        target_entity: "",
        kind: "",
        direction: "directed",
        certainty: "certain",
        valid_from_tick: "",
        valid_to_tick: "",
        source_reference: "",
        metadata_json: "{}",
      };
    case "event":
      return {
        kind: "",
        summary: "",
        body_md: "",
        time_kind: "unknown",
        time_precision: "unknown",
        time_certainty: "certain",
        start_tick: "",
        end_tick: "",
        location_entity:
          objectKindFromUri(state.selectedUri ?? "") === "entity" ? state.selectedUri ?? "" : "",
        participants: "",
        affected_goal_ids: "",
        causal_links: "",
      };
    case "claim":
      return {
        subject_entity:
          objectKindFromUri(state.selectedUri ?? "") === "entity" ? state.selectedUri ?? "" : "",
        content_md: "",
        predicate_key: "",
        object_kind: "none",
        object_value: "",
        polarity: "positive",
        authentication: "canonical",
        holder_entity: "",
        modality: "",
        register: "",
        epistemic_basis: "",
        source: "",
        source_document: "",
        source_claim: "",
        holder_confidence: "",
        period_start_tick: "",
        period_end_tick: "",
      };
    case "rule":
      return {
        kind: "constitutive",
        statement_md: "",
        scope: "world",
        severity: "advisory",
        validator_kind: "",
        source: "",
        parameters_json: "{}",
      };
    case "goal":
      return {
        holder_entity:
          objectKindFromUri(state.selectedUri ?? "") === "entity" ? state.selectedUri ?? "" : "",
        desired_state_md: "",
        priority: "0",
        status: "active",
        visibility: "public",
        source: "",
        period_start_tick: "",
        period_end_tick: "",
      };
    case "document":
      return {
        title: "",
        kind: "chronicle",
        author_entity:
          objectKindFromUri(state.selectedUri ?? "") === "entity" ? state.selectedUri ?? "" : "",
        perspective_entity:
          objectKindFromUri(state.selectedUri ?? "") === "entity" ? state.selectedUri ?? "" : "",
        canon_status: "canonical",
        body_md: "",
        content_references: "",
      };
  }
}

function buildCreateEditor(kind: SearchObjectKind): EditorMode {
  const values = defaultCreateValues(kind);
  const sourceUrisText = state.selectedUri ?? "";
  const fields: EditorField[] = [];

  switch (kind) {
    case "entity":
      fields.push(
        createField("kind", "Tipo", "select", values.kind, {
          required: true,
          options: enumOptions(["person", "place", "faction", "culture", "resource", "concept"]),
        }),
        createField("name", "Nombre", "text", values.name, { required: true }),
        createField("slug", "Slug", "text", values.slug, { required: true }),
        createField("aliases", "Aliases (uno por línea)", "textarea", values.aliases, { rows: 4 }),
        createField("summary", "Resumen", "textarea", values.summary, { rows: 4 }),
        createField("body_md", "Cuerpo Markdown", "textarea", values.body_md, { rows: 7 }),
        createField("attributes_json", "Atributos JSON", "textarea", values.attributes_json, {
          rows: 6,
          help: "Debe ser un objeto JSON.",
        }),
      );
      break;
    case "relation":
      fields.push(
        createField("source_entity", "Entidad origen", "text", values.source_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("target_entity", "Entidad destino", "text", values.target_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("kind", "Tipo de relación", "text", values.kind, { required: true }),
        createField("direction", "Dirección", "select", values.direction, {
          required: true,
          options: enumOptions(["directed", "undirected"]),
        }),
        createField("certainty", "Certeza", "select", values.certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("valid_from_tick", "Tick inicio", "number", values.valid_from_tick),
        createField("valid_to_tick", "Tick fin", "number", values.valid_to_tick),
        createField("source_reference", "Procedencia", "textarea", values.source_reference, { rows: 3 }),
        createField("metadata_json", "Metadata JSON", "textarea", values.metadata_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      );
      break;
    case "event":
      fields.push(
        createField("kind", "Tipo", "text", values.kind, { required: true }),
        createField("summary", "Resumen", "textarea", values.summary, { rows: 3, required: true }),
        createField("body_md", "Cuerpo Markdown", "textarea", values.body_md, { rows: 6 }),
        createField("time_kind", "Tipo de tiempo", "select", values.time_kind, {
          required: true,
          options: enumOptions(["unknown", "instant", "interval", "ongoing"]),
        }),
        createField("time_precision", "Precisión", "select", values.time_precision, {
          required: true,
          options: enumOptions(["exact", "day", "month", "year", "era", "unknown"]),
        }),
        createField("time_certainty", "Certeza temporal", "select", values.time_certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("start_tick", "Tick inicio", "number", values.start_tick),
        createField("end_tick", "Tick fin", "number", values.end_tick),
        createField("location_entity", "Entidad lugar", "text", values.location_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("participants", "Participantes (entidad|rol|ordinal)", "textarea", values.participants, {
          rows: 5,
        }),
        createField(
          "affected_goal_ids",
          "Metas afectadas (una por línea)",
          "textarea",
          values.affected_goal_ids,
          { rows: 3, help: "UUID o nirmata://goal/..." },
        ),
        createField("causal_links", "Causalidad (evento|kind)", "textarea", values.causal_links, {
          rows: 4,
          help: "Una línea por vínculo: nirmata://event/<id>|causes",
        }),
      );
      break;
    case "claim":
      fields.push(
        createField("subject_entity", "Entidad sujeto", "text", values.subject_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("content_md", "Contenido Markdown", "textarea", values.content_md, { rows: 5 }),
        createField("predicate_key", "Predicate key", "text", values.predicate_key),
        createField("object_kind", "Tipo de objeto", "select", values.object_kind, {
          options: [
            { value: "none", label: "None" },
            { value: "entity", label: "Entity" },
            { value: "scalar", label: "Scalar" },
          ],
        }),
        createField("object_value", "Objeto", "text", values.object_value, {
          help: "UUID/URI para entity, o texto para scalar.",
        }),
        createField("polarity", "Polaridad", "select", values.polarity, {
          required: true,
          options: enumOptions(["positive", "negative"]),
        }),
        createField("authentication", "Autenticación", "select", values.authentication, {
          required: true,
          options: enumOptions(["canonical", "attributed", "disputed"]),
        }),
        createField("holder_entity", "Holder", "text", values.holder_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("modality", "Modalidad", "select", values.modality, {
          options: [
            { value: "", label: "Sin modalidad" },
            ...enumOptions(["assertion", "belief", "hypothesis", "counterfactual"]),
          ],
        }),
        createField("register", "Registro", "text", values.register),
        createField("epistemic_basis", "Base epistémica", "textarea", values.epistemic_basis, {
          rows: 3,
        }),
        createField("source", "Procedencia textual", "textarea", values.source, { rows: 3 }),
        createField("source_document", "Documento fuente", "text", values.source_document, {
          help: "UUID o nirmata://document/...",
        }),
        createField("source_claim", "Claim fuente", "text", values.source_claim, {
          help: "UUID o nirmata://claim/...",
        }),
        createField("holder_confidence", "Confianza holder", "number", values.holder_confidence),
        createField("period_start_tick", "Periodo inicio", "number", values.period_start_tick),
        createField("period_end_tick", "Periodo fin", "number", values.period_end_tick),
      );
      break;
    case "rule":
      fields.push(
        createField("kind", "Tipo", "select", values.kind, {
          required: true,
          options: enumOptions(["constitutive", "generative", "institutional", "authorial"]),
        }),
        createField("statement_md", "Regla Markdown", "textarea", values.statement_md, {
          rows: 4,
          required: true,
        }),
        createField("scope", "Scope", "text", values.scope, { required: true }),
        createField("severity", "Severidad", "select", values.severity, {
          required: true,
          options: enumOptions(["advisory", "hard"]),
        }),
        createField("validator_kind", "Validador", "select", values.validator_kind, {
          options: [
            { value: "", label: "Sin validador" },
            { value: "no_resurrection", label: "No resurrection" },
          ],
        }),
        createField("source", "Procedencia", "textarea", values.source, { rows: 3 }),
        createField("parameters_json", "Parámetros JSON", "textarea", values.parameters_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      );
      break;
    case "goal":
      fields.push(
        createField("holder_entity", "Holder", "text", values.holder_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("desired_state_md", "Estado deseado", "textarea", values.desired_state_md, {
          rows: 4,
          required: true,
        }),
        createField("priority", "Prioridad", "number", values.priority, { required: true }),
        createField("status", "Estado", "select", values.status, {
          required: true,
          options: enumOptions(["active", "achieved", "abandoned", "frustrated"]),
        }),
        createField("visibility", "Visibilidad", "select", values.visibility, {
          required: true,
          options: enumOptions(["public", "secret"]),
        }),
        createField("source", "Procedencia", "textarea", values.source, { rows: 3 }),
        createField("period_start_tick", "Periodo inicio", "number", values.period_start_tick),
        createField("period_end_tick", "Periodo fin", "number", values.period_end_tick),
      );
      break;
    case "document":
      fields.push(
        createField("title", "Título", "text", values.title, { required: true }),
        createField("kind", "Tipo", "text", values.kind, { required: true }),
        createField("author_entity", "Autor", "text", values.author_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("perspective_entity", "Perspectiva", "text", values.perspective_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("canon_status", "Canon", "select", values.canon_status, {
          required: true,
          options: enumOptions(["canonical", "non_canonical"]),
        }),
        createField("body_md", "Cuerpo Markdown", "textarea", values.body_md, { rows: 8 }),
        createField(
          "content_references",
          "Referencias de contenido",
          "textarea",
          values.content_references,
          { rows: 6, help: "Usa el editor inferior para añadir, abrir y reordenar referencias." },
        ),
      );
      break;
  }

  return createEditorMode({
    mode: "create",
    objectType: kind,
    existingUri: null,
    targetUri: null,
    title: `Nuevo ${humanize(kind).toLowerCase()}`,
    subtitle: `Draft manual · ${humanize(kind)}`,
    description: `Completa un formulario mínimo para proponer un nuevo ${humanize(kind).toLowerCase()}.`,
    logicalPath: null,
    fields,
    metadata: state.session ? [{ label: "Mundo", value: state.session.world.name }] : [],
    warnings: [],
    links: [],
    objective: `Create ${kind}`,
    sourceUrisText,
    assumptionsText: "",
  });
}

function issueMap(issues: ManualFieldIssue[]): Map<string, string[]> {
  const grouped = new Map<string, string[]>();
  for (const issue of issues) {
    const list = grouped.get(issue.field) ?? [];
    list.push(issue.message);
    grouped.set(issue.field, list);
  }
  return grouped;
}

function editorIsDirty(mode: EditorMode | null): boolean {
  if (!mode) {
    return false;
  }
  return (
    JSON.stringify(mode.values) !== JSON.stringify(mode.baselineValues)
    || mode.objective !== mode.baselineObjective
    || mode.sourceUrisText !== mode.baselineSourceUrisText
    || mode.assumptionsText !== mode.baselineAssumptionsText
  );
}

function hasPendingWork(): boolean {
  return state.pendingDrafts.size > 0 || editorIsDirty(state.editorMode);
}

function applyLayoutState(): void {
  workspaceShell.style.setProperty(
    "--left-panel-width",
    state.panels.leftCollapsed ? "0px" : `${state.panels.leftWidth}rem`,
  );
  workspaceShell.style.setProperty(
    "--right-panel-width",
    state.panels.rightCollapsed ? "0px" : `${state.panels.rightWidth}rem`,
  );
  workspaceShell.style.setProperty(
    "--bottom-panel-height",
    state.panels.bottomCollapsed ? "0px" : `${state.panels.bottomHeight}rem`,
  );

  navigationPanel.hidden = state.panels.leftCollapsed;
  contextPanel.hidden = state.panels.rightCollapsed;
  pendingPanel.hidden = state.panels.bottomCollapsed;

  toggleNavigationButton.textContent = state.panels.leftCollapsed
    ? "Mostrar navegación"
    : "Ocultar navegación";
  toggleContextButton.textContent = state.panels.rightCollapsed
    ? "Mostrar contexto"
    : "Ocultar contexto";
  togglePendingButton.textContent = state.panels.bottomCollapsed
    ? "Mostrar drafts"
    : "Ocultar drafts";

  toggleNavigationButton.setAttribute("aria-expanded", String(!state.panels.leftCollapsed));
  toggleContextButton.setAttribute("aria-expanded", String(!state.panels.rightCollapsed));
  togglePendingButton.setAttribute("aria-expanded", String(!state.panels.bottomCollapsed));

  leftPanelSize.disabled = state.panels.leftCollapsed;
  rightPanelSize.disabled = state.panels.rightCollapsed;
  bottomPanelSize.disabled = state.panels.bottomCollapsed;
  leftPanelSize.value = String(state.panels.leftWidth);
  rightPanelSize.value = String(state.panels.rightWidth);
  bottomPanelSize.value = String(state.panels.bottomHeight);
}

function renderKindFilters(): void {
  kindFilters.replaceChildren(
    ...kinds.map((kind) => {
      const item = button(kind.label, "kind-filter secondary");
      item.setAttribute("aria-pressed", String(state.activeKind === kind.value));
      item.addEventListener("click", () => {
        state.activeKind = kind.value;
        void refreshNavigation();
      });
      return item;
    }),
  );
}

function renderRecents(): void {
  const items = state.recentUris.slice(0, 8);
  recentsEmpty.hidden = items.length > 0;
  recentsList.replaceChildren(
    ...items.map((uri) => {
      const item = button(labelForUri(uri), "linked-button");
      item.setAttribute("aria-current", String(uri === state.selectedUri));
      const snippet = document.createElement("p");
      snippet.className = "linked-button-snippet";
      snippet.textContent = pathForUri(state.logicalTree, uri) ?? uri;
      const meta = block("badge-row");
      const kind = objectKindFromUri(uri);
      if (kind) {
        meta.append(badge(humanize(kind), "kind"));
      }
      item.append(meta, snippet);
      item.addEventListener("click", () => {
        void selectUri(uri);
      });
      return item;
    }),
  );
}

function renderTree(): void {
  const tree = state.logicalTree;
  if (!tree || tree.children.length === 0) {
    treeRoot.replaceChildren();
    treeEmpty.hidden = false;
    return;
  }
  treeEmpty.hidden = true;
  treeRoot.replaceChildren(renderTreeNodes(tree.children));
}

function renderTreeNodes(nodes: LogicalVfsNode[]): HTMLOListElement {
  const list = document.createElement("ol");
  list.className = "tree-list";
  for (const node of nodes) {
    const item = document.createElement("li");
    if (node.type === "object") {
      const objectButton = button(node.name, "tree-button");
      objectButton.setAttribute("aria-current", String(node.uri === state.selectedUri));
      objectButton.addEventListener("click", () => {
        void selectUri(node.uri);
      });
      const meta = document.createElement("div");
      meta.className = "tree-path";
      meta.textContent = node.uri;
      item.append(objectButton, meta);
    } else {
      const label = document.createElement("div");
      label.className = "tree-label";
      label.textContent = node.name;
      const branch = document.createElement("div");
      branch.className = "tree-branch";
      branch.append(renderTreeNodes(node.children));
      item.append(label, branch);
    }
    list.append(item);
  }
  return list;
}

function renderResults(): void {
  const summaryParts = [`${state.searchHits.length} resultado${state.searchHits.length === 1 ? "" : "s"}`];
  if (state.activeKind !== "all") {
    summaryParts.push(humanize(state.activeKind).toLowerCase());
  }
  if (state.queryText.trim()) {
    summaryParts.push(`para “${state.queryText.trim()}”`);
  }
  resultSummary.textContent = summaryParts.join(" · ");

  resultsEmpty.hidden = state.searchHits.length > 0;
  resultsList.replaceChildren(
    ...state.searchHits.map((result) => {
      const item = button("", "result-item");
      item.setAttribute("role", "option");
      item.setAttribute("aria-current", String(result.uri === state.selectedUri));
      item.setAttribute("aria-selected", String(result.uri === state.selectedUri));
      const title = document.createElement("div");
      title.className = "result-item-title";
      title.textContent = result.snippet;
      const meta = block("badge-row");
      meta.append(
        badge(humanize(result.object_type), "kind"),
        badge(humanize(result.classification), result.classification === "no_evidence" ? "warning" : "info"),
        badge(humanize(result.authority), "context"),
      );
      const hint = document.createElement("p");
      hint.className = "result-item-snippet";
      hint.textContent = `${shortId(result.object_id)} · ${result.provenance}`;
      item.append(title, meta, hint);
      item.addEventListener("click", () => {
        void selectUri(result.uri);
      });
      return item;
    }),
  );
}

function renderNotice(notice: WorkspaceNotice): HTMLDivElement {
  const card = block(`notice ${notice.kind}`);
  const title = document.createElement("h4");
  title.textContent = notice.title;
  const detail = document.createElement("p");
  detail.textContent = notice.detail;
  card.append(title, detail);
  return card;
}

function renderDocumentReferenceField(
  mode: EditorMode,
  field: EditorField,
  wrapper: HTMLDivElement,
  messages: string[],
): HTMLDivElement {
  const container = block("document-reference-editor");
  const references = normalizeContentReferences(parseContentReferences(mode.values[field.key] ?? ""));
  const list = block("document-reference-list");

  const sync = (nextReferences: EditableContentReference[]): void => {
    mode.values[field.key] = serializeContentReferences(normalizeContentReferences(nextReferences));
    mode.issues = [];
    state.workspaceNotice = null;
    wrapper.classList.toggle("dirty", mode.values[field.key] !== mode.baselineValues[field.key]);
    renderWorkspace();
  };

  if (references.length === 0) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = "Sin referencias. Añade objetivos tipados para mantener el orden del discurso.";
    list.append(empty);
  } else {
    for (const [index, reference] of references.entries()) {
      const card = block("document-reference-card");
      const heading = block("document-reference-heading");
      const title = document.createElement("strong");
      title.textContent = objectKindFromUri(reference.targetUri)
        ? formatReferenceLabel(parseObjectRefFromUri(reference.targetUri)!)
        : reference.targetUri;
      const meta = block("badge-row");
      meta.append(
        badge(`Ordinal ${reference.ordinal}`, "context"),
        badge(objectKindFromUri(reference.targetUri) ? humanize(objectKindFromUri(reference.targetUri)!) : "URI", "kind"),
      );
      heading.append(title, meta);
      const hint = document.createElement("p");
      hint.className = "muted";
      hint.textContent = reference.targetUri;
      const actions = block("inline-actions");
      const openButton = button("Abrir target", "ghost");
      openButton.disabled = objectKindFromUri(reference.targetUri) === null;
      openButton.addEventListener("click", () => {
        void selectUri(reference.targetUri);
      });
      const upButton = button("↑", "secondary");
      upButton.disabled = index === 0;
      upButton.addEventListener("click", () => {
        const next = [...references];
        [next[index - 1], next[index]] = [next[index], next[index - 1]];
        sync(next);
      });
      const downButton = button("↓", "secondary");
      downButton.disabled = index === references.length - 1;
      downButton.addEventListener("click", () => {
        const next = [...references];
        [next[index], next[index + 1]] = [next[index + 1], next[index]];
        sync(next);
      });
      const removeButton = button("Eliminar", "secondary");
      removeButton.addEventListener("click", () => {
        sync(references.filter((_, itemIndex) => itemIndex !== index));
      });
      actions.append(openButton, upButton, downButton, removeButton);
      card.append(heading, hint, actions);
      list.append(card);
    }
  }

  const addSection = block("document-reference-add");
  const addLabel = document.createElement("label");
  addLabel.textContent = "Nueva referencia";
  const addInput = document.createElement("input");
  addInput.type = "text";
  addInput.placeholder = "nirmata://event/...";
  addLabel.append(addInput);
  const addButton = button("Añadir referencia", "secondary");
  addButton.addEventListener("click", () => {
    const targetUri = addInput.value.trim();
    if (!targetUri) {
      return;
    }
    sync([...references, { targetUri, ordinal: references.length }]);
  });
  addSection.append(addLabel, addButton);

  container.append(list, addSection);
  if (field.help) {
    const help = document.createElement("p");
    help.className = "field-help";
    help.textContent = field.help;
    container.append(help);
  }
  if (messages.length > 0) {
    const errorList = document.createElement("ul");
    errorList.className = "field-error-list";
    for (const message of messages) {
      const item = document.createElement("li");
      item.textContent = message;
      errorList.append(item);
    }
    container.append(errorList);
  }
  return container;
}

function parseObjectRefFromUri(uri: string): ObjectRef | null {
  const [, kind, id] = /^nirmata:\/\/(world|entity|relation|event|claim|rule|goal|document)\/(.+)$/u.exec(
    uri,
  ) ?? [null, null, null];
  if (!kind || !id) {
    return null;
  }
  return { [kind]: id } as ObjectRef;
}

function renderEditor(): void {
  if (!state.editorMode) {
    editorTitle.textContent = "Selecciona o crea un objeto";
    editorSubtitle.textContent = "";
    editorEmpty.hidden = false;
    editorContent.hidden = true;
    editorContent.replaceChildren();
    return;
  }

  const mode = state.editorMode;
  editorTitle.textContent = mode.title;
  editorSubtitle.textContent = `${mode.mode === "create" ? "Nuevo" : "Editar"} · ${humanize(mode.objectType)} · ${mode.subtitle}`;
  const layout = block("editor-layout");
  if (state.workspaceNotice) {
    layout.append(renderNotice(state.workspaceNotice));
  }

  const toolbar = block("editor-section");
  const toolbarTitle = document.createElement("h4");
  toolbarTitle.textContent = "Modo";
  const actions = block("editor-toolbar");
  if (state.selectedObject) {
    const editSelection = button("Editar selección", "secondary");
    editSelection.disabled = mode.mode === "update" && mode.existingUri === state.selectedObject.result.uri;
    editSelection.addEventListener("click", () => {
      state.editorMode = buildSelectionEditor(state.selectedObject!);
      renderWorkspace();
    });
    actions.append(editSelection);
  }
  const kindSelect = document.createElement("select");
  for (const option of createKinds) {
    const item = document.createElement("option");
    item.value = option.value;
    item.textContent = option.label;
    if (option.value === mode.objectType && mode.mode === "create") {
      item.selected = true;
    }
    kindSelect.append(item);
  }
  const newButton = button("Nuevo formulario", "secondary");
  newButton.addEventListener("click", () => {
    state.workspaceNotice = null;
    state.editorMode = buildCreateEditor(kindSelect.value as SearchObjectKind);
    renderWorkspace();
  });
  actions.append(kindSelect, newButton);
  toolbar.append(toolbarTitle, actions);
  layout.append(toolbar);

  const summary = block("editor-section");
  const summaryTitle = document.createElement("h4");
  summaryTitle.textContent = "Lectura activa";
  const summaryText = document.createElement("p");
  summaryText.textContent = mode.description;
  summary.append(summaryTitle, summaryText);
  if (mode.logicalPath) {
    const path = document.createElement("p");
    path.className = "path muted";
    path.textContent = mode.logicalPath;
    summary.append(path);
  }
  layout.append(summary);

  const fieldIssues = issueMap(mode.issues);
  const formSection = block("editor-section");
  const formTitle = document.createElement("h4");
  formTitle.textContent = "Formulario";
  const fieldsWrapper = block("editor-fields");
  for (const field of mode.fields) {
    const wrapper = block("editor-field");
    if (mode.values[field.key] !== mode.baselineValues[field.key]) {
      wrapper.classList.add("dirty");
    }
    const messages = fieldIssues.get(field.key) ?? [];
    if (mode.objectType === "document" && field.key === "content_references") {
      const label = document.createElement("label");
      label.textContent = field.label;
      wrapper.append(label, renderDocumentReferenceField(mode, field, wrapper, messages));
      fieldsWrapper.append(wrapper);
      continue;
    }
    const label = document.createElement("label");
    label.textContent = field.label;
    let control: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement;
    switch (field.control) {
      case "textarea": {
        const textarea = document.createElement("textarea");
        textarea.rows = field.rows ?? 5;
        control = textarea;
        break;
      }
      case "select": {
        const select = document.createElement("select");
        for (const option of field.options ?? []) {
          const element = document.createElement("option");
          element.value = option.value;
          element.textContent = option.label;
          if (option.value === mode.values[field.key]) {
            element.selected = true;
          }
          select.append(element);
        }
        control = select;
        break;
      }
      default: {
        const input = document.createElement("input");
        input.type = field.control === "number" ? "number" : "text";
        control = input;
      }
    }
    control.name = field.key;
    control.value = mode.values[field.key] ?? "";
    if (field.placeholder && "placeholder" in control) {
      control.placeholder = field.placeholder;
    }
    if (field.required) {
      control.setAttribute("aria-required", "true");
    }
    control.addEventListener("input", () => {
      mode.values[field.key] = control.value;
      mode.issues = [];
      state.workspaceNotice = null;
      wrapper.classList.toggle("dirty", mode.values[field.key] !== mode.baselineValues[field.key]);
    });
    label.append(control);
    wrapper.append(label);
    if (field.help) {
      const help = document.createElement("p");
      help.className = "field-help";
      help.textContent = field.help;
      wrapper.append(help);
    }
    if (messages.length > 0) {
      const list = document.createElement("ul");
      list.className = "field-error-list";
      for (const message of messages) {
        const item = document.createElement("li");
        item.textContent = message;
        list.append(item);
      }
      wrapper.append(list);
    }
    fieldsWrapper.append(wrapper);
  }
  formSection.append(formTitle, fieldsWrapper);
  layout.append(formSection);

  const workflow = block("editor-section");
  const workflowTitle = document.createElement("h4");
  workflowTitle.textContent = "Workflow manual";
  const objectiveLabel = document.createElement("label");
  objectiveLabel.textContent = "Objetivo del draft";
  const objectiveInput = document.createElement("input");
  objectiveInput.type = "text";
  objectiveInput.value = mode.objective;
  objectiveInput.addEventListener("input", () => {
    mode.objective = objectiveInput.value;
    mode.issues = [];
  });
  objectiveLabel.append(objectiveInput);
  const sourceLabel = document.createElement("label");
  sourceLabel.textContent = "Fuentes (URIs, una por línea)";
  const sourceTextarea = document.createElement("textarea");
  sourceTextarea.rows = 4;
  sourceTextarea.value = mode.sourceUrisText;
  sourceTextarea.addEventListener("input", () => {
    mode.sourceUrisText = sourceTextarea.value;
    mode.issues = [];
  });
  sourceLabel.append(sourceTextarea);
  const assumptionsLabel = document.createElement("label");
  assumptionsLabel.textContent = "Supuestos (uno por línea)";
  const assumptionsTextarea = document.createElement("textarea");
  assumptionsTextarea.rows = 4;
  assumptionsTextarea.value = mode.assumptionsText;
  assumptionsTextarea.addEventListener("input", () => {
    mode.assumptionsText = assumptionsTextarea.value;
    mode.issues = [];
  });
  assumptionsLabel.append(assumptionsTextarea);
  const formActions = block("form-actions");
  const saveButton = button("Guardar draft", "primary");
  saveButton.addEventListener("click", () => {
    void saveCurrentDraft();
  });
  const resetButton = button("Revertir formulario", "secondary");
  resetButton.addEventListener("click", () => {
    resetCurrentEditor();
  });
  formActions.append(saveButton, resetButton);
  workflow.append(workflowTitle, objectiveLabel, sourceLabel, assumptionsLabel, formActions);
  layout.append(workflow);

  const metaSection = block("editor-section");
  const metaTitle = document.createElement("h4");
  metaTitle.textContent = "Metadata";
  const metaList = document.createElement("dl");
  metaList.className = "meta-list";
  for (const item of mode.metadata) {
    const row = block("meta-row");
    const term = document.createElement("dt");
    term.textContent = item.label;
    const definition = document.createElement("dd");
    if (item.uri) {
      const link = button(item.value, "meta-link");
      link.addEventListener("click", () => {
        void selectUri(item.uri!);
      });
      definition.append(link);
    } else {
      definition.textContent = item.value;
    }
    row.append(term, definition);
    metaList.append(row);
  }
  metaSection.append(metaTitle, metaList);
  layout.append(metaSection);

  editorContent.replaceChildren(layout);
  editorEmpty.hidden = true;
  editorContent.hidden = false;
}

function renderLinkedEntry(entry: JumpLink | RelatedContextEntry): HTMLButtonElement {
  const item = button("", "linked-button");
  const title = document.createElement("div");
  title.className = "linked-button-title";
  const snippet = document.createElement("p");
  snippet.className = "linked-button-snippet";
  const meta = block("badge-row");
  if ("result" in entry) {
    title.textContent = entry.result.snippet;
    snippet.textContent = `${humanize(entry.result.object_type)} · ${shortId(entry.result.object_id)}`;
    meta.append(
      badge(humanize(entry.stage), "context"),
      badge(humanize(entry.result.classification), "info"),
    );
    item.addEventListener("click", () => {
      void selectUri(entry.result.uri);
    });
    item.setAttribute("aria-current", String(entry.result.uri === state.selectedUri));
  } else {
    title.textContent = entry.label;
    snippet.textContent = entry.uri;
    item.addEventListener("click", () => {
      void selectUri(entry.uri);
    });
    item.setAttribute("aria-current", String(entry.uri === state.selectedUri));
  }
  item.append(title, meta, snippet);
  return item;
}

function renderContextSection(title: string, entries: Array<JumpLink | RelatedContextEntry>): HTMLDivElement {
  const section = block("context-section");
  const heading = document.createElement("h4");
  heading.textContent = title;
  const list = block("context-list");
  list.append(...entries.map(renderLinkedEntry));
  section.append(heading, list);
  return section;
}

function renderContext(): void {
  if (!state.editorMode && !state.timeline) {
    contextSummary.textContent = state.selectedLogicalPath ?? "";
    contextEmpty.hidden = false;
    contextContent.hidden = true;
    contextContent.replaceChildren();
    return;
  }

  const summaryParts: string[] = [];
  if (state.context) {
    summaryParts.push(`${state.context.usage.used_objects}/${state.context.usage.max_objects} objetos`);
  } else if (state.selectedLogicalPath) {
    summaryParts.push(state.selectedLogicalPath);
  }
  if (state.timeline) {
    summaryParts.push(
      `${state.timeline.known.length + state.timeline.unknown.length} evento${
        state.timeline.known.length + state.timeline.unknown.length === 1 ? "" : "s"
      }`,
    );
  }
  contextSummary.textContent = summaryParts.join(" · ");
  const wrapper = block("editor-layout");
  if (state.editorMode && state.editorMode.warnings.length > 0) {
    const warningSection = block("context-section");
    const heading = document.createElement("h4");
    heading.textContent = "Advertencias";
    const list = block("warning-list");
    for (const warning of state.editorMode.warnings) {
      const card = block("warning-card");
      const title = document.createElement("h4");
      title.textContent = warning.title;
      const detail = document.createElement("p");
      detail.textContent = warning.detail;
      card.append(title, detail);
      list.append(card);
    }
    warningSection.append(heading, list);
    wrapper.append(warningSection);
  }

  if (state.editorMode && state.editorMode.links.length > 0) {
    wrapper.append(renderContextSection("Navegación de relaciones", state.editorMode.links));
  }

  if (state.context) {
    const groups: Array<[string, RelatedContextEntry[]]> = [
      ["Canon", state.context.canon],
      ["Perspectivas", state.context.perspectives],
      ["Deseos", state.context.desires],
      ["Obligaciones", state.context.obligations],
      ["Búsqueda relacionada", state.context.search_evidence],
    ];

    for (const [title, entries] of groups) {
      if (entries.length > 0) {
        wrapper.append(renderContextSection(title, entries));
      }
    }
  }

  wrapper.append(renderTimelineSection());

  if (wrapper.childElementCount === 0) {
    const empty = block("context-section");
    const title = document.createElement("h4");
    title.textContent = "Sin contexto adicional";
    const detail = document.createElement("p");
    detail.textContent =
      state.context?.absence?.classification === "no_evidence"
        ? "No hay evidencia adicional alrededor de esta selección."
        : "No se encontraron relaciones adicionales para esta selección.";
    empty.append(title, detail);
    wrapper.append(empty);
  }

  contextContent.replaceChildren(wrapper);
  contextEmpty.hidden = true;
  contextContent.hidden = false;
}

function issueEntries(report: ValidationReport): Array<[string, ValidationIssue[]]> {
  return [
    ["Errores", report.errors],
    ["Conflictos", report.conflicts],
    ["Advertencias", report.warnings],
    ["Info", report.info],
  ];
}

function syncPendingReviewRecord(record: PendingDraftRecord, review: ManualReviewSnapshot): void {
  record.review = review;
  record.preview.objective = review.objective;
  record.preview.sourceUris = [...review.sources];
  record.preview.assumptions = [...review.assumptions];
  record.preview.validationReport = review.validationReport;
  record.preview.readyToConfirm = review.readyToConfirm;
}

async function updateReviewAction(
  record: PendingDraftRecord,
  action: ManualReviewActionRequest,
): Promise<void> {
  clearError();
  setStatus("Revalidando revisión manual…");
  try {
    const review = await invoke<ManualReviewSnapshot>("apply_manual_review_action", {
      input: {
        reviewKey: record.review.reviewKey,
        action,
      },
    });
    syncPendingReviewRecord(record, review);
    renderWorkspace();
    setStatus("Revisión actualizada.");
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

async function addWaiver(
  record: PendingDraftRecord,
  operation: ManualReviewOperationSnapshot,
  issue: ValidationIssue,
): Promise<void> {
  const rationale = window.prompt(`Razón del waiver para ${issue.code}:`, "");
  if (!rationale || !rationale.trim()) {
    return;
  }
  await updateReviewAction(record, {
    kind: "add_waiver",
    operationId: operation.operationId,
    issueCode: issue.code,
    rationale: rationale.trim(),
  });
}

async function recordJudgment(
  record: PendingDraftRecord,
  operation: ManualReviewOperationSnapshot,
): Promise<void> {
  const judgment = window.prompt(
    "Registra tu lectura breve antes de revelar la resolución sugerida:",
    operation.risk.judgment ?? "",
  );
  if (!judgment || !judgment.trim()) {
    return;
  }
  await updateReviewAction(record, {
    kind: "record_judgment",
    operationId: operation.operationId,
    judgment: judgment.trim(),
  });
}

async function readPendingDraft(record: PendingDraftRecord): Promise<void> {
  const review = await invoke<ManualReviewSnapshot>("read_manual_review", {
    input: { reviewKey: record.review.reviewKey },
  });
  syncPendingReviewRecord(record, review);
}

async function revalidatePendingDraft(record: PendingDraftRecord): Promise<void> {
  clearError();
  setStatus("Revalidando contra la cabeza vigente…");
  try {
    const review = await invoke<ManualReviewSnapshot>("revalidate_manual_review", {
      input: { reviewKey: record.review.reviewKey },
    });
    syncPendingReviewRecord(record, review);
    renderWorkspace();
    setStatus(
      review.freshness.status === "current"
        ? "Revisión manual revalidada."
        : "La revisión volvió a cambiar durante el refresco.",
    );
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

async function confirmPendingDraft(record: PendingDraftRecord): Promise<void> {
  clearError();
  setStatus("Confirmando ChangeSet…");
  try {
    const session = await invoke<WorldSession>("confirm_manual_review", {
      input: { reviewKey: record.review.reviewKey },
    });
    state.session = session;
    state.pendingDrafts.delete(record.preview.draftKey);
    state.workspaceNotice = {
      kind: "info",
      title: "ChangeSet persistido",
      detail: `La revisión ${session.current_revision} quedó aplicada en una única transacción.`,
    };
    state.selectedUri = record.preview.targetUri;
    state.selectedObject = null;
    state.context = null;
    state.selectedLogicalPath = null;
    await refreshNavigation();
    await loadSelection(record.preview.targetUri, true);
    setStatus(`ChangeSet confirmado: ${record.preview.title}.`);
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function renderReviewObjectSnapshot(
  title: string,
  snapshot: ManualReviewObjectSnapshot | null,
): HTMLDivElement | null {
  if (!snapshot) {
    return null;
  }
  const section = block("review-object");
  const heading = document.createElement("h5");
  heading.textContent = title;
  const objectTitle = document.createElement("strong");
  objectTitle.textContent = snapshot.title;
  const openButton = button("Abrir", "ghost");
  openButton.addEventListener("click", () => {
    void selectUri(snapshot.targetUri);
  });
  const header = block("subsection-header");
  header.append(objectTitle, openButton);
  const list = document.createElement("dl");
  list.className = "meta-list";
  for (const line of snapshot.lines) {
    const row = block("meta-row");
    const term = document.createElement("dt");
    term.textContent = line.label;
    const definition = document.createElement("dd");
    definition.textContent = line.value;
    row.append(term, definition);
    list.append(row);
  }
  section.append(heading, header, list);
  return section;
}

function renderTimelineGroup(title: string, events: TimelineEventEntry[]): HTMLDivElement {
  const section = block("context-section");
  const heading = document.createElement("h4");
  heading.textContent = title;
  const list = block("timeline-list");
  for (const event of events) {
    const item = button("", "linked-button");
    item.setAttribute("aria-current", String(event.uri === state.selectedUri));
    const eventTitle = document.createElement("div");
    eventTitle.className = "linked-button-title";
    eventTitle.textContent = event.summary;
    const meta = block("badge-row");
    meta.append(badge(humanize(event.kind), "kind"), badge(humanize(event.time.kind), "context"));
    const detail = document.createElement("p");
    detail.className = "linked-button-snippet";
    detail.textContent = formatEventTime(event.time);
    item.append(eventTitle, meta, detail);
    item.addEventListener("click", () => {
      void selectUri(event.uri);
    });
    list.append(item);
  }
  section.append(heading, list);
  return section;
}

function renderTimelineSection(): HTMLDivElement {
  const section = block("context-section");
  const heading = document.createElement("h4");
  heading.textContent = "Timeline";
  section.append(heading);
  if (!state.timeline || (state.timeline.known.length === 0 && state.timeline.unknown.length === 0)) {
    const detail = document.createElement("p");
    detail.textContent = "No hay eventos registrados todavía.";
    section.append(detail);
    return section;
  }
  if (state.timeline.known.length > 0) {
    section.append(renderTimelineGroup("Ticks conocidos", state.timeline.known));
  }
  if (state.timeline.unknown.length > 0) {
    section.append(renderTimelineGroup("Tiempo unknown", state.timeline.unknown));
  }
  return section;
}

function renderRevisionAuditOperation(operation: RevisionAuditOperationSnapshot): HTMLDivElement {
  const operationCard = block("review-operation-card");
  const operationTitle = document.createElement("h5");
  operationTitle.textContent = operation.after?.title ?? operation.before?.title ?? operation.targetUri;
  const operationMeta = block("badge-row");
  operationMeta.append(
    badge(humanize(operation.decision), "info"),
    badge(operation.source, "context"),
    badge(formatTimestamp(operation.decidedAtMs), "context"),
  );
  operationCard.append(operationTitle, operationMeta);

  const openTarget = button("Abrir objetivo", "ghost");
  openTarget.addEventListener("click", () => {
    void selectUri(operation.targetUri);
  });
  operationCard.append(openTarget);

  const objectGrid = block("review-object-grid");
  const before = renderReviewObjectSnapshot("Antes", operation.before);
  const after = renderReviewObjectSnapshot("Después", operation.after);
  if (before) {
    objectGrid.append(before);
  }
  if (after) {
    objectGrid.append(after);
  }
  if (objectGrid.childElementCount > 0) {
    operationCard.append(objectGrid);
  }

  if (operation.waivers.length > 0) {
    const waiverSection = block("review-issue-group");
    const waiverTitle = document.createElement("h5");
    waiverTitle.textContent = "Waivers";
    const waiverList = document.createElement("ul");
    waiverList.className = "review-issue-list";
    for (const waiver of operation.waivers) {
      const item = document.createElement("li");
      item.textContent = `${waiver.issueCode}: ${waiver.rationale}`;
      waiverList.append(item);
    }
    waiverSection.append(waiverTitle, waiverList);
    operationCard.append(waiverSection);
  }

  return operationCard;
}

async function undoRevisionFromHistory(entry: RevisionHistoryEntrySnapshot): Promise<void> {
  clearError();
  setStatus(`Deshaciendo revisión ${shortId(entry.revisionId)}…`);
  try {
    const session = await invoke<WorldSession>("undo_revision", {
      input: { revisionId: entry.revisionId },
    });
    state.session = session;
    state.workspaceNotice = {
      kind: "info",
      title: "Undo aplicado",
      detail: `Se creó una nueva revisión que revierte ${shortId(entry.revisionId)} sin perder la auditoría local.`,
    };
    await refreshNavigation();
    if (state.selectedUri) {
      await loadSelection(state.selectedUri, true);
    }
    setStatus(`Undo confirmado para ${shortId(entry.revisionId)}.`);
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function renderRevisionHistory(): HTMLDivElement | null {
  if (!state.revisionHistory) {
    return null;
  }

  const card = block("pending-card");
  const title = document.createElement("h4");
  title.textContent = "Historial local de revisiones";
  const detail = document.createElement("p");
  detail.className = "muted";
  detail.textContent = state.revisionHistory.revisions.length === 0
    ? `La cabeza actual es ${shortId(state.revisionHistory.currentHeadRevisionId)} y todavía no hay ChangeSets confirmados.`
    : `${state.revisionHistory.revisions.length} revisión${state.revisionHistory.revisions.length === 1 ? "" : "es"} confirmada${state.revisionHistory.revisions.length === 1 ? "" : "s"} · cabeza ${shortId(state.revisionHistory.currentHeadRevisionId)}.`;
  card.append(title, detail);

  if (state.revisionHistory.revisions.length === 0) {
    return card;
  }

  const list = block("review-source-list");
  for (const entry of state.revisionHistory.revisions) {
    const item = button("", "linked-button");
    item.setAttribute("aria-current", String(entry.revisionId === state.selectedRevisionId));
    const entryTitle = document.createElement("div");
    entryTitle.className = "linked-button-title";
    entryTitle.textContent = entry.summary;
    const meta = block("badge-row");
    meta.append(
      badge(shortId(entry.revisionId), "context"),
      badge(entry.isCurrentHead ? "Cabeza" : entry.isCurrentUndoTarget ? "Undo disponible" : entry.undoneRevisionId ? "Undo" : entry.author, entry.isCurrentUndoTarget ? "ready" : "info"),
    );
    const snippet = document.createElement("p");
    snippet.className = "linked-button-snippet";
    snippet.textContent = `${formatTimestamp(entry.createdAtMs)} · ${entry.operations.length} operación${entry.operations.length === 1 ? "" : "es"} · ${entry.waivers.length} waiver${entry.waivers.length === 1 ? "" : "s"}`;
    item.append(entryTitle, meta, snippet);
    item.addEventListener("click", () => {
      state.selectedRevisionId = entry.revisionId;
      renderWorkspace();
    });
    list.append(item);
  }
  card.append(list);

  const selected = selectedRevisionEntry();
  if (!selected) {
    return card;
  }

  const selectedCard = block("review-operation-card");
  const selectedTitle = document.createElement("h5");
  selectedTitle.textContent = `Revisión ${shortId(selected.revisionId)}`;
  const selectedMeta = block("badge-row");
  selectedMeta.append(
    badge(selected.author, "context"),
    badge(formatTimestamp(selected.createdAtMs), "context"),
    badge(selected.isCurrentHead ? "Cabeza actual" : selected.isCurrentUndoTarget ? "Undo disponible" : "Histórica", selected.isCurrentUndoTarget ? "ready" : "info"),
  );
  const summary = document.createElement("p");
  summary.textContent = selected.summary;
  const ids = document.createElement("p");
  ids.className = "muted";
  ids.textContent = `change_set ${shortId(selected.changeSetId)} · parent ${selected.parentRevisionId ? shortId(selected.parentRevisionId) : "root"}`;
  selectedCard.append(selectedTitle, selectedMeta, summary, ids);

  if (selected.undoneRevisionId) {
    const undoneNotice = block("notice info");
    const undoneTitle = document.createElement("h4");
    undoneTitle.textContent = "Undo registrado";
    const undoneDetail = document.createElement("p");
    undoneDetail.textContent = `Esta revisión revierte ${shortId(selected.undoneRevisionId)} y mantiene la auditoría before/after accesible.`;
    undoneNotice.append(undoneTitle, undoneDetail);
    selectedCard.append(undoneNotice);
  }

  if (selected.waivers.length > 0) {
    const waiverSection = block("review-issue-group");
    const waiverTitle = document.createElement("h5");
    waiverTitle.textContent = "Waivers registrados";
    const waiverList = document.createElement("ul");
    waiverList.className = "review-issue-list";
    for (const waiver of selected.waivers) {
      const item = document.createElement("li");
      item.textContent = `${waiver.issueCode}: ${waiver.rationale}`;
      waiverList.append(item);
    }
    waiverSection.append(waiverTitle, waiverList);
    selectedCard.append(waiverSection);
  }

  if (selected.operations.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "La revisión no expone operaciones auditables.";
    selectedCard.append(empty);
  } else {
    for (const operation of selected.operations) {
      selectedCard.append(renderRevisionAuditOperation(operation));
    }
  }

  const actions = block("pending-actions");
  const undo = button("Deshacer esta revisión", "secondary");
  undo.disabled = !selected.isCurrentUndoTarget;
  undo.title = selected.isCurrentUndoTarget
    ? ""
    : state.revisionHistory.undoTargetRevisionId
      ? `Primero debes deshacer ${shortId(state.revisionHistory.undoTargetRevisionId)}.`
      : "No hay revisiones lógicas para deshacer.";
  undo.addEventListener("click", () => {
    void undoRevisionFromHistory(selected);
  });
  actions.append(undo);
  selectedCard.append(actions);
  card.append(selectedCard);
  return card;
}

function renderPending(): void {
  const drafts = Array.from(state.pendingDrafts.values()).sort((left, right) =>
    left.preview.title.localeCompare(right.preview.title, "es"),
  );
  const revisionCount = state.revisionHistory?.revisions.length ?? 0;
  pendingSummary.textContent = [
    drafts.length === 0
      ? null
      : `${drafts.length} ChangeSet${drafts.length === 1 ? "" : "s"} pendiente${drafts.length === 1 ? "" : "s"}`,
    revisionCount === 0
      ? null
      : `${revisionCount} revisión${revisionCount === 1 ? "" : "es"} local${revisionCount === 1 ? "" : "es"}`,
  ].filter(Boolean).join(" · ") || "Sin cambios pendientes.";

  const wrapper = block("pending-list");
  if (state.editorMode && state.editorMode.issues.length > 0) {
    const issuesCard = block("pending-card");
    const title = document.createElement("h4");
    title.textContent = "Último intento sin guardar";
    const detail = document.createElement("p");
    detail.className = "muted";
    detail.textContent = `${state.editorMode.issues.length} validación(es) por campo.`;
    const list = document.createElement("ul");
    for (const issue of state.editorMode.issues) {
      const item = document.createElement("li");
      item.textContent = `${issue.field}: ${issue.message}`;
      list.append(item);
    }
    issuesCard.append(title, detail, list);
    wrapper.append(issuesCard);
  }

  for (const record of drafts) {
    const card = block("pending-card");
    const title = document.createElement("h4");
    title.textContent = record.preview.title;
    const meta = block("badge-row");
    meta.append(
      badge(humanize(record.preview.objectType), "kind"),
      badge(record.preview.mode === "create" ? "Create" : "Update", "info"),
      badge(
        record.review.freshness.status === "current" ? "Revisión vigente" : "Draft stale",
        record.review.freshness.status === "current" ? "ready" : "warning",
      ),
      badge(
        record.review.readyToConfirm ? "Listo para confirmar" : "Bloqueado",
        record.review.readyToConfirm ? "ready" : "warning",
      ),
    );
    const hint = document.createElement("p");
    hint.className = "muted";
    hint.textContent = `${record.preview.logicalPath} · ${record.preview.targetUri} · base ${record.review.baseRevision}`;
    const objective = document.createElement("p");
    objective.textContent = record.review.objective;
    card.append(title, meta, hint, objective);

    if (record.review.freshness.status !== "current") {
      const freshnessNotice = block("notice warning");
      const freshnessTitle = document.createElement("h4");
      freshnessTitle.textContent =
        record.review.freshness.status === "refresh_restart_required"
          ? "Revalidación interrumpida"
          : "Draft obsoleto";
      const freshnessDetail = document.createElement("p");
      freshnessDetail.textContent = record.review.freshness.message;
      freshnessNotice.append(freshnessTitle, freshnessDetail);
      if (record.review.freshness.canRevalidate) {
        const revalidate = button("Revalidar contra la cabeza vigente", "secondary");
        revalidate.addEventListener("click", () => {
          void revalidatePendingDraft(record);
        });
        freshnessNotice.append(revalidate);
      }
      card.append(freshnessNotice);
    }

    if (record.review.sources.length > 0) {
      const sources = block("review-source-list");
      for (const source of record.review.sources) {
        const sourceButton = button(labelForUri(source), "ghost");
        sourceButton.addEventListener("click", () => {
          void selectUri(source);
        });
        sources.append(sourceButton);
      }
      card.append(sources);
    }

    const reviewIssue = firstReviewIssue(record.review.effectiveReport);
    if (reviewIssue) {
      const notice = block(`notice ${reviewIssue.severity === "error" ? "warning" : "info"}`);
      const noticeTitle = document.createElement("h4");
      noticeTitle.textContent = record.review.readyToConfirm
        ? "Revisión validada"
        : "Confirmación deshabilitada";
      const detail = document.createElement("p");
      detail.textContent = reviewIssue.message;
      notice.append(noticeTitle, detail);
      card.append(notice);
    }

    for (const operation of record.review.operations) {
      const operationCard = block("review-operation-card");
      const operationTitle = document.createElement("h5");
      operationTitle.textContent = operation.after?.title ?? operation.before?.title ?? operation.targetUri;
      const operationMeta = block("badge-row");
      operationMeta.append(
        badge(humanize(operation.severity), operation.severity),
        badge(operation.selected ? "Seleccionada" : "Excluida", operation.selected ? "ready" : "warning"),
        badge(humanize(operation.decision), "info"),
      );
      operationCard.append(operationTitle, operationMeta);

      const objectGrid = block("review-object-grid");
      const before = renderReviewObjectSnapshot("Antes", operation.before);
      const after = renderReviewObjectSnapshot("Después", operation.after);
      if (before) {
        objectGrid.append(before);
      }
      if (after) {
        objectGrid.append(after);
      }
      if (objectGrid.childElementCount > 0) {
        operationCard.append(objectGrid);
      }

      if (operation.dependencies.length > 0) {
        const dependencySection = block("review-issue-group");
        const dependencyTitle = document.createElement("h5");
        dependencyTitle.textContent = "Dependencias";
        const dependencyList = block("review-source-list");
        for (const dependency of operation.dependencies) {
          const dependencyButton = button(labelForUri(dependency), "ghost");
          dependencyButton.addEventListener("click", () => {
            void selectUri(dependency);
          });
          dependencyList.append(dependencyButton);
        }
        dependencySection.append(dependencyTitle, dependencyList);
        operationCard.append(dependencySection);
      }

      if (operation.decisionPoints.length > 0) {
        const decisionSection = block("review-issue-group");
        const decisionTitle = document.createElement("h5");
        decisionTitle.textContent = "DecisionPoints";
        const decisionList = block("warning-list");
        for (const decision of operation.decisionPoints) {
          const decisionCard = block("warning-card");
          const decisionHeading = document.createElement("h4");
          decisionHeading.textContent = decision.prompt;
          const detail = document.createElement("p");
          detail.textContent = decision.suggestionHidden
            ? "Registra primero tu juicio para revelar la resolución sugerida."
            : [
                decision.resolvedAlternative ? `Resuelta: ${decision.resolvedAlternative}` : null,
                decision.reason ? `Razón: ${decision.reason}` : null,
              ]
                .filter(Boolean)
                .join(" · ");
          const alternatives = document.createElement("p");
          alternatives.className = "muted";
          alternatives.textContent = decision.alternatives.join(" · ");
          decisionCard.append(decisionHeading, detail, alternatives);
          decisionList.append(decisionCard);
        }
        decisionSection.append(decisionTitle, decisionList);
        operationCard.append(decisionSection);
      }

      if (operation.risk.triggers.length > 0) {
        const riskSection = block("review-issue-group");
        const riskTitle = document.createElement("h5");
        riskTitle.textContent = "Fricción de alto riesgo";
        const riskList = document.createElement("ul");
        riskList.className = "review-issue-list";
        for (const trigger of operation.risk.triggers) {
          const item = document.createElement("li");
          item.textContent = `${trigger.title}: ${trigger.detail}`;
          riskList.append(item);
        }
        riskSection.append(riskTitle, riskList);
        const judgment = document.createElement("p");
        judgment.className = "muted";
        judgment.textContent = operation.risk.judgment
          ? `Juicio registrado: ${operation.risk.judgment}`
          : operation.risk.suggestedResolutionAvailable
            ? "Debes registrar un juicio breve antes de ver la resolución sugerida."
            : "Registra un juicio breve para dejar constancia antes de confirmar este cambio.";
        riskSection.append(judgment);
        if (!operation.risk.judgment) {
          const judgmentButton = button("Registrar juicio", "secondary");
          judgmentButton.addEventListener("click", () => {
            void recordJudgment(record, operation);
          });
          riskSection.append(judgmentButton);
        }
        operationCard.append(riskSection);
      }

      for (const [label, issues] of issueEntries(operation.effectiveIssues)) {
        if (issues.length === 0) {
          continue;
        }
        const group = block("review-issue-group");
        const groupTitle = document.createElement("h5");
        groupTitle.textContent = label;
        const list = document.createElement("ul");
        list.className = "review-issue-list";
        for (const issue of issues) {
          const item = document.createElement("li");
          const message = document.createElement("span");
          message.textContent = issue.message;
          item.append(message);
          if (issue.severity !== "error") {
            const waiveButton = button("Registrar waiver", "ghost");
            waiveButton.addEventListener("click", () => {
              void addWaiver(record, operation, issue);
            });
            item.append(waiveButton);
          }
          list.append(item);
        }
        group.append(groupTitle, list);
        operationCard.append(group);
      }

      if (operation.waivers.length > 0) {
        const waiverSection = block("review-issue-group");
        const waiverTitle = document.createElement("h5");
        waiverTitle.textContent = "Waivers";
        const waiverList = document.createElement("ul");
        waiverList.className = "review-issue-list";
        for (const waiver of operation.waivers) {
          const item = document.createElement("li");
          item.textContent = `${waiver.issueCode}: ${waiver.rationale}`;
          waiverList.append(item);
        }
        waiverSection.append(waiverTitle, waiverList);
        operationCard.append(waiverSection);
      }

      const operationActions = block("pending-actions");
      const toggleSelection = button(operation.selected ? "Excluir operación" : "Incluir operación", "secondary");
      toggleSelection.addEventListener("click", () => {
        void updateReviewAction(record, {
          kind: operation.selected ? "reject" : "accept",
          operationId: operation.operationId,
        });
      });
      const editButton = button("Editar operación", "ghost");
      editButton.addEventListener("click", () => {
        void openReviewOperationEditor(record, operation);
      });
      operationActions.append(toggleSelection, editButton);
      operationCard.append(operationActions);
      card.append(operationCard);
    }

    const actions = block("pending-actions");
    const reopen = button("Abrir formulario", "ghost");
    reopen.addEventListener("click", () => {
      void openPendingDraft(record);
    });
    actions.append(reopen);
    if (record.editor.existingUri) {
      const openCanon = button("Abrir canon", "ghost");
      openCanon.addEventListener("click", () => {
        void selectUri(record.editor.existingUri!);
      });
      actions.append(openCanon);
    }
    const confirm = button("Confirmar ChangeSet", "primary");
    confirm.disabled = !record.review.readyToConfirm;
    confirm.title = record.review.readyToConfirm
      ? ""
      : record.review.freshness.status !== "current"
        ? record.review.freshness.message
        : firstReviewIssue(record.review.effectiveReport)?.message ?? "Hay errores, conflictos, dependencias rotas o falta registrar juicio.";
    confirm.addEventListener("click", () => {
      void confirmPendingDraft(record);
    });
    actions.append(confirm);
    const discard = button("Descartar", "secondary");
    discard.addEventListener("click", () => {
      state.pendingDrafts.delete(record.preview.draftKey);
      if (state.editorMode?.targetUri === record.preview.draftKey) {
        state.editorMode.issues = [];
      }
      renderWorkspace();
    });
    actions.append(discard);
    card.append(actions);
    wrapper.append(card);
  }

  const history = renderRevisionHistory();
  if (history) {
    wrapper.append(history);
  }

  if (wrapper.childElementCount === 0) {
    pendingEmpty.hidden = false;
    pendingContent.hidden = true;
    pendingContent.replaceChildren();
    return;
  }

  pendingContent.replaceChildren(wrapper);
  pendingEmpty.hidden = true;
  pendingContent.hidden = false;
}

async function openPendingDraft(record: PendingDraftRecord): Promise<void> {
  if (record.editor.existingUri) {
    await loadSelection(record.editor.existingUri, false);
  }
  state.workspaceNotice = null;
  state.editorMode = cloneEditorMode(record.editor);
  renderWorkspace();
}

function renderWorkspace(): void {
  if (!state.session) {
    closedView.hidden = false;
    worldView.hidden = true;
    return;
  }

  closedView.hidden = true;
  worldView.hidden = false;
  worldName.textContent = state.session.world.name;
  worldPath.textContent = state.session.path;
  setMarkdownText(worldPremise, state.session.world.premise_md, "No especificada");
  worldEpoch.textContent = normalizeText(state.session.world.epoch_label, "No especificado");
  worldRevision.textContent = state.session.world.current_revision;
  searchInput.value = state.queryText;
  uriInput.value = state.selectedUri ?? uriInput.value;

  applyLayoutState();
  renderKindFilters();
  renderRecents();
  renderTree();
  renderResults();
  renderEditor();
  renderContext();
  renderPending();
}

function resetWorkspaceState(): void {
  state.queryText = "";
  state.activeKind = "all";
  state.searchHits = [];
  state.logicalTree = null;
  state.selectedUri = null;
  state.selectedLogicalPath = null;
  state.selectedObject = null;
  state.editorMode = null;
  state.context = null;
  state.timeline = null;
  state.revisionHistory = null;
  state.selectedRevisionId = null;
  state.recentUris = [];
  state.pendingDrafts.clear();
  state.workspaceNotice = null;
  state.navigationRequestId = 0;
  state.selectionRequestId = 0;
}

function openSession(session: WorldSession): void {
  state.session = session;
  resetWorkspaceState();
  renderWorkspace();
  void refreshNavigation();
}

function closeSession(): void {
  state.session = null;
  resetWorkspaceState();
  renderWorkspace();
  setStatus("Mundo cerrado.");
}

function pushRecent(uri: string): void {
  state.recentUris = [uri, ...state.recentUris.filter((item) => item !== uri)].slice(0, 8);
}

function applyCommandStateError(value: unknown, fallback?: string): void {
  const code = commandCode(value);
  const message = commandMessage(value);
  switch (code) {
    case "object_not_found":
      state.workspaceNotice = {
        kind: "warning",
        title: "Objeto eliminado",
        detail: `La URI seleccionada ya no existe en el mundo actual. ${message}`,
      };
      state.selectedObject = null;
      state.context = null;
      state.selectedLogicalPath = null;
      if (state.selectedUri && state.pendingDrafts.has(state.selectedUri)) {
        state.editorMode = cloneEditorMode(state.pendingDrafts.get(state.selectedUri)!.editor);
      }
      setStatus("La selección ya no existe en canon.");
      break;
    case "no_world_open":
      state.workspaceNotice = {
        kind: "warning",
        title: "Mundo cerrado",
        detail: "La sesión activa ya no existe y la interfaz volvió al estado inicial.",
      };
      closeSession();
      break;
    case "manual_review_stale":
      state.workspaceNotice = {
        kind: "warning",
        title: "Draft obsoleto",
        detail: `${message || "La revisión quedó detrás de la cabeza actual; revalida antes de confirmar."}${retainedDraftHint()}`,
      };
      setStatus("La revisión manual requiere revalidación.");
      break;
    case "manual_review_not_ready":
    case "manual_review_revalidation_failed":
    case "validation_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "Commit rechazado por validación",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El ChangeSet sigue pendiente de revisión.");
      break;
    case "project_locked":
      state.workspaceNotice = {
        kind: "warning",
        title: "Archivo bloqueado",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("No se pudo escribir porque el archivo está en uso.");
      break;
    case "constraint_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "Constraint de SQLite rechazó el cambio",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El draft se conservó para corregirlo o reintentarlo.");
      break;
    case "file_not_found":
      state.workspaceNotice = {
        kind: "warning",
        title: "Archivo movido o inexistente",
        detail: `El archivo .nirmata activo ya no se encuentra en la ruta original. ${message}`,
      };
      setStatus("");
      break;
    case "file_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "Error de archivo",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("");
      break;
    case "invalid_project_path":
      state.workspaceNotice = {
        kind: "warning",
        title: "Ruta inválida",
        detail: "Selecciona un archivo local con extensión .nirmata.",
      };
      setStatus(fallback ?? "");
      break;
    case "invalid_object_uri":
    case "invalid_revision_id":
      state.workspaceNotice = {
        kind: "warning",
        title: "URI o revisión inválida",
        detail: message || "Usa URIs estables nirmata://kind/uuid y revisiones UUID visibles en el historial.",
      };
      setStatus(fallback ?? "");
      break;
    case "undo_target_invalid":
    case "undo_conflict":
      state.workspaceNotice = {
        kind: "warning",
        title: "Undo no disponible",
        detail: message,
      };
      setStatus("El historial local indica qué revisión puede deshacerse ahora.");
      break;
    default:
      showError(value);
      if (fallback) {
        setStatus(fallback);
      }
      return;
  }
  clearError();
  renderWorkspace();
}

async function refreshNavigation(): Promise<void> {
  if (!state.session) {
    return;
  }
  clearError();
  setStatus("Actualizando navegación…");
  const requestId = ++state.navigationRequestId;
  try {
    const [session, response, logicalTree, timeline, revisionHistory] = await Promise.all([
      invoke<WorldSession | null>("get_current_world"),
      invoke<SearchWorldResponse>("search_world", {
        input: {
          queryText: state.queryText,
          kind: state.activeKind,
          limit: 200,
        },
      }),
      invoke<LogicalVfsDirectory>("read_logical_vfs"),
      invoke<TimelineOverview>("list_timeline_events"),
      invoke<RevisionHistorySnapshot>("list_revision_history"),
    ]);
    if (requestId !== state.navigationRequestId) {
      return;
    }

    if (session) {
      state.session = session;
    }
    const previousPath = state.selectedLogicalPath;
    state.searchHits = response.hits;
    state.logicalTree = logicalTree;
    state.timeline = timeline;
    state.revisionHistory = revisionHistory;
    state.selectedRevisionId =
      revisionHistory.revisions.find((entry) => entry.revisionId === state.selectedRevisionId)?.revisionId
      ?? revisionHistory.undoTargetRevisionId
      ?? revisionHistory.revisions[0]?.revisionId
      ?? null;

    if (state.selectedUri) {
      const nextPath = pathForUri(logicalTree, state.selectedUri);
      if (nextPath && previousPath && nextPath !== previousPath) {
        state.workspaceNotice = {
          kind: "info",
          title: "Archivo lógico movido",
          detail: `La selección conservó su URI estable y ahora vive en ${nextPath}.`,
        };
      }
      state.selectedLogicalPath = nextPath;
      if (!nextPath) {
        await loadSelection(state.selectedUri, true);
      }
    } else {
      const nextSelection = state.searchHits[0]?.uri ?? firstUriFromTree(logicalTree);
      if (nextSelection) {
        await loadSelection(nextSelection, true);
        return;
      }
      if (!state.editorMode) {
        state.editorMode = buildCreateEditor("entity");
      }
    }

    const pending = Array.from(state.pendingDrafts.values());
    if (pending.length > 0) {
      await Promise.all(pending.map((record) => readPendingDraft(record)));
      if (requestId !== state.navigationRequestId) {
        return;
      }
    }

    renderWorkspace();
    setStatus(state.queryText.trim() ? "Búsqueda y árbol actualizados." : "Navegación actualizada.");
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

async function loadSelection(uri: string, keepNotice: boolean): Promise<void> {
  clearError();
  const requestId = ++state.selectionRequestId;
  setStatus("Cargando selección…");
  try {
    const [selectedObject, context] = await Promise.all([
      invoke<OpenUriResponse>("open_uri", { uri }),
      invoke<RelatedContextResponse>("get_related_context", { input: { uri } }),
    ]);
    if (requestId !== state.selectionRequestId) {
      return;
    }

    state.selectedUri = uri;
    state.selectedLogicalPath = pathForUri(state.logicalTree, uri);
    state.selectedObject = selectedObject;
    state.context = context;
    pushRecent(uri);
    if (!keepNotice) {
      state.workspaceNotice = null;
    }
    state.editorMode = buildSelectionEditor(selectedObject);
    renderWorkspace();
    setStatus(`Selección actualizada: ${state.editorMode.title}.`);
  } catch (value) {
    if (requestId !== state.selectionRequestId) {
      return;
    }
    state.selectedUri = uri;
    applyCommandStateError(value, "");
  }
}

async function selectUri(uri: string): Promise<void> {
  if (state.selectedUri === uri && state.selectedObject && state.context) {
    return;
  }
  await loadSelection(uri, false);
}

function applyManualRequestToEditor(
  editor: EditorMode,
  request: ManualDraftRequest,
  reviewEdit: ReviewEditContext | null,
): EditorMode {
  const next = cloneEditorMode(editor);
  next.objective = request.objective ?? "";
  next.sourceUrisText = request.sourceUris.join("\n");
  next.assumptionsText = request.assumptions.join("\n");
  next.values = { ...next.values, ...request.values };
  next.fields = next.fields.map((field) => ({
    ...field,
    value: request.values[field.key] ?? "",
  }));
  next.baselineValues = { ...next.values };
  next.baselineObjective = next.objective;
  next.baselineSourceUrisText = next.sourceUrisText;
  next.baselineAssumptionsText = next.assumptionsText;
  next.issues = [];
  next.reviewEdit = reviewEdit;
  return next;
}

async function openReviewOperationEditor(
  record: PendingDraftRecord,
  operation: ManualReviewOperationSnapshot,
): Promise<void> {
  clearError();
  setStatus("Abriendo edición de operación…");
  try {
    const request = await invoke<ManualDraftRequest>("begin_manual_review_edit", {
      input: {
        reviewKey: record.review.reviewKey,
        operationId: operation.operationId,
      },
    });

    let editor: EditorMode | null;
    if (request.existingUri) {
      await loadSelection(request.existingUri, false);
      editor = state.editorMode ? cloneEditorMode(state.editorMode) : null;
    } else {
      editor = buildCreateEditor(request.objectType as SearchObjectKind);
    }

    if (!editor) {
      setStatus("");
      return;
    }

    state.workspaceNotice = {
      kind: "info",
      title: "Edición por operación",
      detail: "El formulario reutiliza el workflow existente y revalidará el ChangeSet al guardar.",
    };
    state.editorMode = applyManualRequestToEditor(editor, request, {
      reviewKey: record.review.reviewKey,
      operationId: operation.operationId,
    });
    renderWorkspace();
    setStatus("Operación lista para editar.");
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function currentManualRequest(): ManualDraftRequest | null {
  const editor = state.editorMode;
  if (!editor) {
    return null;
  }
  return {
    objectType: editor.objectType,
    existingUri: editor.existingUri ?? undefined,
    objective: editor.objective.trim() || undefined,
    sourceUris: splitLines(editor.sourceUrisText),
    assumptions: splitLines(editor.assumptionsText),
    values: { ...editor.values },
  };
}

async function saveCurrentDraft(): Promise<void> {
  const editor = state.editorMode;
  const request = currentManualRequest();
  if (!editor || !request) {
    return;
  }
  if (editor.reviewEdit) {
    clearError();
    setStatus("Revalidando operación editada…");
    try {
      const response = await invoke<ManualDraftResponse>("apply_manual_review_edit", {
        input: {
          reviewKey: editor.reviewEdit.reviewKey,
          operationId: editor.reviewEdit.operationId,
          request,
        },
      });
      if (!state.editorMode) {
        return;
      }
      state.editorMode.issues = response.fieldIssues;
      if (!response.review) {
        state.workspaceNotice = {
          kind: "warning",
          title: "Edición incompleta",
          detail: "Corrige los campos marcados antes de revalidar la operación.",
        };
        renderWorkspace();
        setStatus("La operación editada requiere correcciones.");
        return;
      }
      const record = state.pendingDrafts.get(editor.reviewEdit.reviewKey);
      if (record) {
        syncPendingReviewRecord(record, response.review);
        if (record.preview.targetUri === request.existingUri || record.review.operations.length === 1) {
          record.editor = applyManualRequestToEditor(cloneEditorMode(editor), request, null);
        }
      }
      state.workspaceNotice = {
        kind: "info",
        title: "Operación revalidada",
        detail: "El diff y el reporte del panel inferior ya reflejan la edición aplicada.",
      };
      state.editorMode = null;
      renderWorkspace();
      setStatus("Operación actualizada.");
      return;
    } catch (value) {
      applyCommandStateError(value, "");
      return;
    }
  }
  clearError();
  setStatus("Construyendo ChangeSetDraft…");
  try {
    const response = await invoke<ManualDraftResponse>("preview_manual_draft", { input: request });
    if (!state.editorMode) {
      return;
    }
    state.editorMode.issues = response.fieldIssues;
    if (!response.draft) {
      state.workspaceNotice = {
        kind: "warning",
        title: "Draft no creado",
        detail: "Corrige las validaciones por campo antes de generar el ChangeSetDraft.",
      };
      renderWorkspace();
      setStatus("El draft manual requiere correcciones.");
      return;
    }
    if (!response.review) {
      throw new Error("La revisión manual no quedó disponible para este draft.");
    }

    const updatedEditor = cloneEditorMode(state.editorMode);
    updatedEditor.targetUri = response.draft.targetUri;
    updatedEditor.logicalPath = response.draft.logicalPath;
    updatedEditor.issues = [];
    state.pendingDrafts.set(response.draft.draftKey, {
      preview: response.draft,
      review: response.review,
      editor: updatedEditor,
    });
    state.workspaceNotice = {
      kind: "info",
      title: "Draft creado",
      detail: "El formulario generó un ChangeSetDraft pendiente. Revisa validaciones en el panel inferior.",
    };
    renderWorkspace();
    setStatus(`Draft listo: ${response.draft.title}.`);
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function resetCurrentEditor(): void {
  if (!state.editorMode) {
    return;
  }
  if (state.editorMode.mode === "create") {
    state.editorMode = buildCreateEditor(
      state.editorMode.objectType === "world" ? "entity" : state.editorMode.objectType,
    );
  } else if (state.selectedObject) {
    state.editorMode = buildSelectionEditor(state.selectedObject);
  } else if (state.editorMode.objectType === "world") {
    state.editorMode = buildWorldEditor();
  }
  state.workspaceNotice = null;
  renderWorkspace();
}

function confirmDiscardPending(): boolean {
  if (!hasPendingWork()) {
    return true;
  }
  return window.confirm(
    "Hay drafts manuales no revisados o cambios locales sin guardar. ¿Descartar y continuar?",
  );
}

chooseCreatePath.addEventListener("click", async () => {
  try {
    clearError();
    const selected = await dialog.save({
      defaultPath: `${nameInput.value.trim() || "mi-mundo"}.nirmata`,
      filters: filter,
    });
    if (selected !== null) {
      pathInput.value = selected.toLowerCase().endsWith(".nirmata") ? selected : `${selected}.nirmata`;
    }
  } catch (value) {
    showError(value);
  }
});

createForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!pathInput.value) {
    showError("Elige primero dónde guardar el archivo .nirmata.");
    return;
  }

  clearError();
  setStatus("Creando mundo…");
  try {
    const session = await invoke<WorldSession>("create_world", {
      input: {
        path: pathInput.value,
        name: nameInput.value,
        premise_md: premiseInput.value,
        epoch_label: epochInput.value,
      },
    });
    openSession(session);
  } catch (value) {
    showError(value);
    setStatus("");
  }
});

openButton.addEventListener("click", async () => {
  try {
    clearError();
    const selected = await dialog.open({
      multiple: false,
      directory: false,
      filters: filter,
    });
    if (selected === null) {
      return;
    }

    setStatus("Abriendo mundo…");
    openSession(await invoke<WorldSession>("open_world", { path: selected }));
  } catch (value) {
    showError(value);
    setStatus("");
  }
});

closeButton.addEventListener("click", async () => {
  if (!confirmDiscardPending()) {
    return;
  }
  clearError();
  setStatus("Cerrando mundo…");
  try {
    await invoke("close_world");
    closeSession();
  } catch (value) {
    applyCommandStateError(value, "");
  }
});

editWorldButton.addEventListener("click", () => {
  const uri = currentWorldUri();
  if (!uri) {
    return;
  }
  void selectUri(uri);
});

toggleNavigationButton.addEventListener("click", () => {
  state.panels.leftCollapsed = !state.panels.leftCollapsed;
  applyLayoutState();
});

toggleContextButton.addEventListener("click", () => {
  state.panels.rightCollapsed = !state.panels.rightCollapsed;
  applyLayoutState();
});

togglePendingButton.addEventListener("click", () => {
  state.panels.bottomCollapsed = !state.panels.bottomCollapsed;
  applyLayoutState();
});

leftPanelSize.addEventListener("input", () => {
  state.panels.leftWidth = Number(leftPanelSize.value);
  applyLayoutState();
});

rightPanelSize.addEventListener("input", () => {
  state.panels.rightWidth = Number(rightPanelSize.value);
  applyLayoutState();
});

bottomPanelSize.addEventListener("input", () => {
  state.panels.bottomHeight = Number(bottomPanelSize.value);
  applyLayoutState();
});

searchInput.addEventListener("input", () => {
  state.queryText = searchInput.value;
});

searchForm.addEventListener("submit", (event) => {
  event.preventDefault();
  void refreshNavigation();
});

uriForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const uri = uriInput.value.trim();
  if (!uri) {
    return;
  }
  void selectUri(uri);
});

resultsList.addEventListener("keydown", (event) => {
  const options = Array.from(resultsList.querySelectorAll<HTMLButtonElement>('[role="option"]'));
  if (options.length === 0) {
    return;
  }
  const currentIndex = options.findIndex((option) => option === document.activeElement);
  const activeIndex =
    currentIndex >= 0
      ? currentIndex
      : options.findIndex((option) => option.getAttribute("aria-current") === "true");
  let nextIndex = activeIndex >= 0 ? activeIndex : 0;
  switch (event.key) {
    case "ArrowDown":
      nextIndex = Math.min(options.length - 1, nextIndex + 1);
      break;
    case "ArrowUp":
      nextIndex = Math.max(0, nextIndex - 1);
      break;
    case "Home":
      nextIndex = 0;
      break;
    case "End":
      nextIndex = options.length - 1;
      break;
    default:
      return;
  }
  event.preventDefault();
  options[nextIndex]?.focus();
  options[nextIndex]?.click();
});

window.addEventListener("beforeunload", (event) => {
  if (!hasPendingWork()) {
    return;
  }
  event.preventDefault();
  event.returnValue = "";
});

renderWorkspace();

void invoke<WorldSession | null>("get_current_world")
  .then((session) => {
    if (session !== null) {
      openSession(session);
      return;
    }
    setStatus("Crea o abre un mundo para comenzar.");
  })
  .catch(showError);
