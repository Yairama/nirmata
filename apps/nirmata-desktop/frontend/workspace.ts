import {
  buildSelectionEditor,
  buildWorldEditor,
  cloneEditorMode,
} from "./editor-model.js";
import { buildCreateEditor } from "./editor-create.js";
import {
  clearError,
  commandCode,
  commandMessage,
  firstUriFromTree,
  normalizeText,
  pathForUri,
  retainedDraftHint,
  setMarkdownText,
  setStatus,
  showError,
  splitLines,
} from "./helpers.js";
import { renderContext } from "./render-context.js";
import { renderEditor } from "./render-editor.js";
import {
  renderKindFilters,
  renderRecents,
  renderResults,
  renderTree,
} from "./render-navigation.js";
import {
  readPendingDraft,
  renderPending,
  syncPendingReviewRecord,
} from "./render-pending.js";
import {
  bottomPanelSize,
  closedView,
  contextPanel,
  editWorldButton,
  invoke,
  leftPanelSize,
  navigationPanel,
  pendingPanel,
  rightPanelSize,
  searchInput,
  state,
  toggleContextButton,
  toggleNavigationButton,
  togglePendingButton,
  uriInput,
  workspaceShell,
  worldEpoch,
  worldName,
  worldPath,
  worldPremise,
  worldRevision,
  worldView,
} from "./state.js";
import type {
  EditorMode,
  LogicalVfsDirectory,
  ManualDraftRequest,
  ManualDraftResponse,
  ManualReviewOperationSnapshot,
  OpenUriResponse,
  PendingDraftRecord,
  RelatedContextResponse,
  ReviewEditContext,
  RevisionHistorySnapshot,
  SearchObjectKind,
  SearchWorldResponse,
  TimelineOverview,
  WorldSession,
} from "./types.js";

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

export function hasPendingWork(): boolean {
  return state.pendingDrafts.size > 0 || editorIsDirty(state.editorMode);
}

export function applyLayoutState(): void {
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

export function renderWorkspace(): void {
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
  editWorldButton.disabled = state.session.read_only;
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
  state.narrative.timeline = null;
  state.narrative.causalThreads = null;
  state.narrative.looseEnds = null;
  state.narrative.exploration = null;
  state.revisionHistory = null;
  state.selectedRevisionId = null;
  state.recentUris = [];
  state.pendingDrafts.clear();
  state.workspaceNotice = null;
  state.navigationRequestId = 0;
  state.selectionRequestId = 0;
}

export function openSession(session: WorldSession): void {
  state.session = session;
  resetWorkspaceState();
  renderWorkspace();
  window.dispatchEvent(new CustomEvent("nirmata:scope-changed"));
  void refreshNavigation();
}

export function closeSession(): void {
  state.session = null;
  resetWorkspaceState();
  renderWorkspace();
  setStatus("Mundo cerrado.");
}

function pushRecent(uri: string): void {
  state.recentUris = [uri, ...state.recentUris.filter((item) => item !== uri)].slice(0, 8);
}

export function applyCommandStateError(value: unknown, fallback?: string): void {
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
    case "storage_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "La transacción no pudo completarse",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El canon no cambió. El draft se conservó y puedes reintentar.");
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

export async function refreshNavigation(): Promise<void> {
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
    window.dispatchEvent(new CustomEvent("nirmata:scope-changed"));

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

export async function loadSelection(uri: string, keepNotice: boolean): Promise<void> {
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

export async function selectUri(uri: string): Promise<void> {
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

export async function openReviewOperationEditor(
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

export async function saveCurrentDraft(): Promise<void> {
  if (state.session?.read_only) {
    showError("La revisión observada es de solo lectura. Vuelve a la cabeza activa para editar.");
    return;
  }
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

export function resetCurrentEditor(): void {
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

export function confirmDiscardPending(): boolean {
  if (!hasPendingWork()) {
    return true;
  }

  return window.confirm(
    "Hay drafts manuales no revisados o cambios locales sin guardar. ¿Descartar y continuar?",
  );
}
