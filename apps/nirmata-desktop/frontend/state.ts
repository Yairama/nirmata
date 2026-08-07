import type { AppState, SearchKind, SearchObjectKind } from "./types.js";

export const { invoke } = window.__TAURI__.core;
export const dialog = window.__TAURI__.dialog;
export const listen = window.__TAURI__.event.listen;
export const filter = [{ name: "Proyecto Nirmata", extensions: ["nirmata"] }];
export const kinds: Array<{ value: SearchKind; label: string }> = [
  { value: "all", label: "Todo" },
  { value: "entity", label: "Entidades" },
  { value: "relation", label: "Relaciones" },
  { value: "event", label: "Eventos" },
  { value: "claim", label: "Claims" },
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

export const createForm = document.querySelector<HTMLFormElement>("#create-form")!;
export const nameInput = document.querySelector<HTMLInputElement>("#name")!;
export const premiseInput = document.querySelector<HTMLTextAreaElement>("#premise")!;
export const epochInput = document.querySelector<HTMLInputElement>("#epoch-label")!;
export const pathInput = document.querySelector<HTMLInputElement>("#create-path")!;
export const chooseCreatePath = document.querySelector<HTMLButtonElement>("#choose-create-path")!;
export const openButton = document.querySelector<HTMLButtonElement>("#open-button")!;
export const closeButton = document.querySelector<HTMLButtonElement>("#close-button")!;
export const closedView = document.querySelector<HTMLElement>("#closed-view")!;
export const worldView = document.querySelector<HTMLElement>("#world-view")!;
export const statusElement = document.querySelector<HTMLElement>("#status")!;
export const error = document.querySelector<HTMLElement>("#error")!;
export const worldName = document.querySelector<HTMLElement>("#world-name")!;
export const worldPath = document.querySelector<HTMLElement>("#world-path")!;
export const worldPremise = document.querySelector<HTMLElement>("#world-premise")!;
export const worldEpoch = document.querySelector<HTMLElement>("#world-epoch")!;
export const worldRevision = document.querySelector<HTMLElement>("#world-revision")!;
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
export const uriForm = document.querySelector<HTMLFormElement>("#uri-form")!;
export const uriInput = document.querySelector<HTMLInputElement>("#uri-input")!;
export const searchForm = document.querySelector<HTMLFormElement>("#search-form")!;
export const searchInput = document.querySelector<HTMLInputElement>("#search-input")!;
export const resultSummary = document.querySelector<HTMLElement>("#result-summary")!;
export const kindFilters = document.querySelector<HTMLElement>("#kind-filters")!;
export const recentsList = document.querySelector<HTMLElement>("#recents-list")!;
export const recentsEmpty = document.querySelector<HTMLElement>("#recents-empty")!;
export const treeRoot = document.querySelector<HTMLElement>("#tree-root")!;
export const treeEmpty = document.querySelector<HTMLElement>("#tree-empty")!;
export const resultsList = document.querySelector<HTMLElement>("#results-list")!;
export const resultsEmpty = document.querySelector<HTMLElement>("#results-empty")!;
export const editorTitle = document.querySelector<HTMLElement>("#editor-title")!;
export const editorSubtitle = document.querySelector<HTMLElement>("#editor-subtitle")!;
export const editorEmpty = document.querySelector<HTMLElement>("#editor-empty")!;
export const editorContent = document.querySelector<HTMLElement>("#editor-content")!;
export const contextSummary = document.querySelector<HTMLElement>("#context-summary")!;
export const contextEmpty = document.querySelector<HTMLElement>("#context-empty")!;
export const contextContent = document.querySelector<HTMLElement>("#context-content")!;
export const pendingSummary = document.querySelector<HTMLElement>("#pending-summary")!;
export const pendingEmpty = document.querySelector<HTMLElement>("#pending-empty")!;
export const pendingContent = document.querySelector<HTMLElement>("#pending-content")!;
export const assistantQueryMode = document.querySelector<HTMLButtonElement>("#assistant-query-mode")!;
export const assistantProposeMode = document.querySelector<HTMLButtonElement>("#assistant-propose-mode")!;
export const assistantContext = document.querySelector<HTMLElement>("#assistant-context")!;
export const assistantForm = document.querySelector<HTMLFormElement>("#assistant-form")!;
export const assistantInput = document.querySelector<HTMLTextAreaElement>("#assistant-input")!;
export const assistantSubmit = document.querySelector<HTMLButtonElement>("#assistant-submit")!;
export const assistantCancel = document.querySelector<HTMLButtonElement>("#assistant-cancel")!;
export const assistantFinalCritique = document.querySelector<HTMLButtonElement>("#assistant-final-critique")!;
export const assistantProgress = document.querySelector<HTMLElement>("#assistant-progress")!;
export const assistantTranscript = document.querySelector<HTMLElement>("#assistant-transcript")!;
export const assistantCredential = document.querySelector<HTMLElement>("#assistant-credential")!;
export const assistantKeyForm = document.querySelector<HTMLFormElement>("#assistant-key-form")!;
export const assistantKey = document.querySelector<HTMLInputElement>("#assistant-key")!;
export const assistantKeyClear = document.querySelector<HTMLButtonElement>("#assistant-key-clear")!;
