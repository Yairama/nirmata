import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { AiActivity, AppState, SearchKind, SearchObjectKind } from "./types.js";

export { invoke, listen };
export const dialog = { open, save };
export const loreFilter = [{ name: "Lore UTF-8", extensions: ["md", "markdown", "txt"] }];
export const kinds: Array<{ value: SearchKind; label: string }> = [
  { value: "all", label: "Todo" },
  { value: "entity", label: "Entidades" },
  { value: "relation", label: "Relaciones" },
  { value: "event", label: "Eventos" },
  { value: "claim", label: "Afirmaciones" },
  { value: "rule", label: "Reglas" },
  { value: "goal", label: "Metas" },
  { value: "document", label: "Documentos" },
];
export const createKinds = kinds.filter((kind) => kind.value !== "all") as Array<{
  value: SearchObjectKind;
  label: string;
}>;

export const state: AppState = {
  session: null,
  logicalTree: null,
  selectedUri: null,
  selectedLogicalPath: null,
  selectedObject: null,
  editorMode: null,
  context: null,
  timeline: null,
  narrative: {
    timeline: null,
    causalThreads: null,
    looseEnds: null,
    exploration: null,
  },
  revisionHistory: null,
  selectedRevisionId: null,
  recentUris: [],
  pendingDrafts: new Map(),
  ephemeralWork: new Map(),
  workspaceNotice: null,
  aiActivity: null,
  aiProviderReady: false,
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

export const closeButton = document.querySelector<HTMLButtonElement>("#close-button")!;
export const worldView = document.querySelector<HTMLElement>("#world-view")!;
export const statusElement = document.querySelector<HTMLElement>("#status")!;
export const error = document.querySelector<HTMLElement>("#error")!;
export const aiBusyBanner = document.querySelector<HTMLElement>("#ai-busy-banner")!;
export const aiBusyMessage = document.querySelector<HTMLElement>("#ai-busy-message")!;
export const aiBusyCancel = document.querySelector<HTMLButtonElement>("#ai-busy-cancel")!;
export const worldName = document.querySelector<HTMLElement>("#world-name")!;
export const worldPath = document.querySelector<HTMLElement>("#world-path")!;
export const worldPremise = document.querySelector<HTMLElement>("#world-premise")!;
export const worldEpoch = document.querySelector<HTMLElement>("#world-epoch")!;
export const worldRevision = document.querySelector<HTMLElement>("#world-revision")!;
export const readScopeLabel = document.querySelector<HTMLElement>("#read-scope-label")!;
export const variantSelect = document.querySelector<HTMLSelectElement>("#variant-select")!;
export const revisionScopeSelect = document.querySelector<HTMLSelectElement>("#revision-scope-select")!;
export const viewActiveHeadButton = document.querySelector<HTMLButtonElement>("#view-active-head")!;
export const createVariantForm = document.querySelector<HTMLFormElement>("#create-variant-form")!;
export const variantNameInput = document.querySelector<HTMLInputElement>("#variant-name")!;
export const renameVariantForm = document.querySelector<HTMLFormElement>("#rename-variant-form")!;
export const renameVariantNameInput = document.querySelector<HTMLInputElement>("#rename-variant-name")!;
export const compareVariantSelect = document.querySelector<HTMLSelectElement>("#compare-variant-select")!;
export const compareVariantsButton = document.querySelector<HTMLButtonElement>("#compare-variants")!;
export const mergeVariantButton = document.querySelector<HTMLButtonElement>("#merge-variant")!;
export const archiveVariantButton = document.querySelector<HTMLButtonElement>("#archive-variant")!;
export const exportVfsSnapshotButton = document.querySelector<HTMLButtonElement>("#export-vfs-snapshot")!;
export const importVfsSnapshotButton = document.querySelector<HTMLButtonElement>("#import-vfs-snapshot")!;
export const variantDiff = document.querySelector<HTMLElement>("#variant-diff")!;
export const editWorldButton = document.querySelector<HTMLButtonElement>("#edit-world-button")!;
export const workspaceShell = document.querySelector<HTMLElement>("#workspace-shell")!;
export const navigationPanel = document.querySelector<HTMLElement>("#navigation-panel")!;
export const contextPanel = document.querySelector<HTMLElement>("#context-panel")!;
export const pendingPanel = document.querySelector<HTMLElement>("#pending-panel")!;
export const toggleNavigationButton = document.querySelector<HTMLButtonElement>("#toggle-navigation")!;
export const toggleContextButton = document.querySelector<HTMLButtonElement>("#toggle-context")!;
export const togglePendingButton = document.querySelector<HTMLButtonElement>("#toggle-pending")!;
export const leftPanelSize = document.querySelector<HTMLInputElement>("#left-panel-size")!;
export const rightPanelSize = document.querySelector<HTMLInputElement>("#right-panel-size")!;
export const bottomPanelSize = document.querySelector<HTMLInputElement>("#bottom-panel-size")!;
export const editorTitle = document.querySelector<HTMLElement>("#editor-title")!;
export const editorSubtitle = document.querySelector<HTMLElement>("#editor-subtitle")!;
export const editorEmpty = document.querySelector<HTMLElement>("#editor-empty")!;
export const editorContent = document.querySelector<HTMLElement>("#editor-content")!;
export const pendingSummary = document.querySelector<HTMLElement>("#pending-summary")!;
export const pendingEmpty = document.querySelector<HTMLElement>("#pending-empty")!;
export const pendingContent = document.querySelector<HTMLElement>("#pending-content")!;
export const assistantQueryMode = document.querySelector<HTMLButtonElement>("#assistant-query-mode")!;
export const assistantProposeMode = document.querySelector<HTMLButtonElement>("#assistant-propose-mode")!;
export const assistantDeepMode = document.querySelector<HTMLButtonElement>("#assistant-deep-mode")!;
export const assistantAuditMode = document.querySelector<HTMLButtonElement>("#assistant-audit-mode")!;
export const assistantContext = document.querySelector<HTMLElement>("#assistant-context")!;
export const assistantForm = document.querySelector<HTMLFormElement>("#assistant-form")!;
export const assistantInput = document.querySelector<HTMLTextAreaElement>("#assistant-input")!;
export const assistantSubmit = document.querySelector<HTMLButtonElement>("#assistant-submit")!;
export const assistantCancel = document.querySelector<HTMLButtonElement>("#assistant-cancel")!;
export const assistantFinalCritique = document.querySelector<HTMLButtonElement>("#assistant-final-critique")!;
export const assistantProgress = document.querySelector<HTMLElement>("#assistant-progress")!;
export const assistantTranscript = document.querySelector<HTMLElement>("#assistant-transcript")!;
export const assistantCredential = document.querySelector<HTMLElement>("#assistant-credential")!;
export const assistantProviderCheck = document.querySelector<HTMLButtonElement>("#assistant-provider-check")!;
export const assistantProviderSettings = document.querySelector<HTMLButtonElement>("#assistant-provider-settings")!;
export const assistantCredentialSettings = document.querySelector<HTMLDetailsElement>("#assistant-credential-settings")!;
export const assistantKeyForm = document.querySelector<HTMLFormElement>("#assistant-key-form")!;
export const assistantKey = document.querySelector<HTMLInputElement>("#assistant-key")!;
export const assistantKeyClear = document.querySelector<HTMLButtonElement>("#assistant-key-clear")!;
export const loreImportSelect = document.querySelector<HTMLButtonElement>("#lore-import-select")!;
export const loreImportExtract = document.querySelector<HTMLButtonElement>("#lore-import-extract")!;
export const loreImportCancel = document.querySelector<HTMLButtonElement>("#lore-import-cancel")!;
export const loreImportReview = document.querySelector<HTMLButtonElement>("#lore-import-review")!;
export const loreImportDelete = document.querySelector<HTMLButtonElement>("#lore-import-delete")!;
export const loreImportStatus = document.querySelector<HTMLElement>("#lore-import-status")!;
export const loreImportContent = document.querySelector<HTMLElement>("#lore-import-content")!;
export const simulationScenarioSelect = document.querySelector<HTMLSelectElement>("#simulation-scenario-select")!;
export const simulationCompareSelect = document.querySelector<HTMLSelectElement>("#simulation-compare-select")!;
export const simulationForm = document.querySelector<HTMLFormElement>("#simulation-form")!;
export const simulationFactions = document.querySelector<HTMLTextAreaElement>("#simulation-factions")!;
export const simulationResources = document.querySelector<HTMLTextAreaElement>("#simulation-resources")!;
export const simulationStocks = document.querySelector<HTMLTextAreaElement>("#simulation-stocks")!;
export const simulationRules = document.querySelector<HTMLTextAreaElement>("#simulation-rules")!;
export const simulationAssumptions = document.querySelector<HTMLTextAreaElement>("#simulation-assumptions")!;
export const simulationMaxSteps = document.querySelector<HTMLInputElement>("#simulation-max-steps")!;
export const simulationNew = document.querySelector<HTMLButtonElement>("#simulation-new")!;
export const simulationDelete = document.querySelector<HTMLButtonElement>("#simulation-delete")!;
export const simulationRun = document.querySelector<HTMLButtonElement>("#simulation-run")!;
export const simulationStatus = document.querySelector<HTMLElement>("#simulation-status")!;
export const simulationResults = document.querySelector<HTMLElement>("#simulation-results")!;
export const narrativeScope = document.querySelector<HTMLSelectElement>("#narrative-scope")!;
export const narrativeTimeline = document.querySelector<HTMLButtonElement>("#narrative-timeline")!;
export const narrativeCausal = document.querySelector<HTMLButtonElement>("#narrative-causal")!;
export const narrativeLooseEnds = document.querySelector<HTMLButtonElement>("#narrative-loose-ends")!;
export const narrativeStartEvents = document.querySelector<HTMLTextAreaElement>("#narrative-start-events")!;
export const narrativeMaxDepth = document.querySelector<HTMLInputElement>("#narrative-max-depth")!;
export const narrativeLimit = document.querySelector<HTMLInputElement>("#narrative-limit")!;
export const narrativeDocumentForm = document.querySelector<HTMLFormElement>("#narrative-document-form")!;
export const narrativeDocumentKind = document.querySelector<HTMLSelectElement>("#narrative-document-kind")!;
export const narrativeDocumentTitle = document.querySelector<HTMLInputElement>("#narrative-document-title")!;
export const narrativeDocumentRequest = document.querySelector<HTMLTextAreaElement>("#narrative-document-request")!;
export const narrativePerspective = document.querySelector<HTMLInputElement>("#narrative-perspective")!;
export const narrativeTick = document.querySelector<HTMLInputElement>("#narrative-tick")!;
export const narrativeCancel = document.querySelector<HTMLButtonElement>("#narrative-cancel")!;
export const narrativeStatus = document.querySelector<HTMLElement>("#narrative-status")!;
export const narrativeResults = document.querySelector<HTMLElement>("#narrative-results")!;

export function beginAiActivity(activity: AiActivity): void {
  state.aiActivity = activity;
  window.dispatchEvent(new CustomEvent("nirmata:ai-activity-changed"));
}

export function endAiActivity(requestId: string): void {
  if (state.aiActivity?.requestId !== requestId) {
    return;
  }
  state.aiActivity = null;
  window.dispatchEvent(new CustomEvent("nirmata:ai-activity-changed"));
}

export function setEphemeralWork(key: string, label: string, present: boolean): void {
  if (present) {
    state.ephemeralWork.set(key, label);
  } else {
    state.ephemeralWork.delete(key);
  }
}

const sessionListeners = new Set<() => void>();

export function setSession(session: AppState["session"]): void {
  state.session = session;
  for (const listener of sessionListeners) {
    listener();
  }
}

export function getSessionSnapshot(): AppState["session"] {
  return state.session;
}

export function subscribeSession(listener: () => void): () => void {
  sessionListeners.add(listener);
  return () => sessionListeners.delete(listener);
}
