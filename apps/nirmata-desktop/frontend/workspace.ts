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
  normalizeText,
  pathForUri,
  retainedDraftHint,
  setMarkdownText,
  setStatus,
  showError,
  splitLines,
} from "./helpers.js";
import { renderEditor } from "./render-editor.js";
import {
  readPendingDraft,
  renderPending,
  syncPendingReviewRecord,
} from "./render-pending.js";
import {
  bottomPanelSize,
  contextPanel,
  editWorldButton,
  invoke,
  leftPanelSize,
  navigationPanel,
  pendingPanel,
  rightPanelSize,
  setSession,
  state,
  statusElement,
  toggleContextButton,
  toggleNavigationButton,
  togglePendingButton,
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
  ReadScope,
  ReviewEditContext,
  RevisionHistorySnapshot,
  SearchObjectKind,
  SearchWorldResponse,
  TimelineOverview,
  WorldSession,
} from "./types.js";

export function editorIsDirty(mode: EditorMode | null): boolean {
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
  return state.pendingDrafts.size > 0
    || state.ephemeralWork.size > 0
    || editorIsDirty(state.editorMode);
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
  pendingPanel.hidden = !document.body.classList.contains("review-drawer-open");

  toggleNavigationButton.textContent = state.panels.leftCollapsed
    ? "Mostrar navegación"
    : "Ocultar navegación";
  toggleContextButton.textContent = state.panels.rightCollapsed
    ? "Mostrar contexto"
    : "Ocultar contexto";
  togglePendingButton.textContent = state.panels.bottomCollapsed
    ? "Mostrar cambios"
    : "Ocultar cambios";

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
    worldView.hidden = true;
    statusElement.hidden = true;
    return;
  }

  worldView.hidden = false;
  statusElement.hidden = false;
  worldName.textContent = state.session.world.name;
  worldPath.textContent = state.session.path;
  setMarkdownText(worldPremise, state.session.world.premise_md, "No especificada");
  worldEpoch.textContent = normalizeText(state.session.world.epoch_label, "No especificado");
  worldRevision.textContent = state.session.read_only ? "Versión anterior" : "Versión actual";
  editWorldButton.disabled = state.session.read_only;
  applyLayoutState();
  renderEditor();
  window.dispatchEvent(new CustomEvent("nirmata:context-changed"));
  renderPending();
}

function resetWorkspaceState(): void {
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
  state.ephemeralWork.clear();
  state.workspaceNotice = null;
  state.navigationRequestId = 0;
  state.selectionRequestId = 0;
}

export function openSession(session: WorldSession): void {
  resetWorkspaceState();
  setSession(session);
  renderWorkspace();
  window.dispatchEvent(new CustomEvent("nirmata:scope-changed"));
  void refreshNavigation();
}

export function closeSession(): void {
  window.dispatchEvent(new CustomEvent("nirmata:discard-ephemeral-work"));
  resetWorkspaceState();
  setSession(null);
  renderWorkspace();
  setStatus("Mundo cerrado.");
}

function pushRecent(uri: string): void {
  state.recentUris = [uri, ...state.recentUris.filter((item) => item !== uri)].slice(0, 8);
  window.dispatchEvent(new CustomEvent("nirmata:selection-changed"));
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
        title: "Propuesta desactualizada",
        detail: `${message || "La propuesta quedó detrás de la versión actual; vuelve a comprobarla antes de aplicarla."}${retainedDraftHint()}`,
      };
      setStatus("La revisión manual requiere revalidación.");
      break;
    case "manual_review_not_ready":
    case "manual_review_revalidation_failed":
    case "validation_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "No se puede aplicar todavía",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El conjunto de cambios sigue pendiente de revisión.");
      break;
    case "project_locked":
      state.workspaceNotice = {
        kind: "warning",
        title: "Archivo bloqueado",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("No se pudo escribir porque el archivo está en uso.");
      break;
    case "app_busy":
      state.workspaceNotice = {
        kind: "warning",
        title: "IA trabajando",
        detail: "Espera a que termine la solicitud activa o cancélala antes de cambiar el mundo.",
      };
      setStatus("La acción no se ejecutó; el mundo y el trabajo local se conservaron.");
      break;
    case "constraint_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "El almacenamiento rechazó el cambio",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("La propuesta se conservó para corregirla o reintentarlo.");
      break;
    case "storage_error":
      state.workspaceNotice = {
        kind: "warning",
        title: "La transacción no pudo completarse",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El canon no cambió. La propuesta se conservó y puedes reintentar.");
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
        title: "Deshacer no disponible",
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
    const [session, timeline, revisionHistory] = await Promise.all([
      invoke<WorldSession | null>("get_current_world"),
      invoke<TimelineOverview>("list_timeline_events"),
      invoke<RevisionHistorySnapshot>("list_revision_history"),
    ]);
    if (requestId !== state.navigationRequestId) {
      return;
    }

    if (session) {
      setSession(session);
    }
    state.timeline = timeline;
    state.revisionHistory = revisionHistory;
    state.selectedRevisionId =
      revisionHistory.revisions.find((entry) => entry.revisionId === state.selectedRevisionId)?.revisionId
      ?? revisionHistory.undoTargetRevisionId
      ?? revisionHistory.revisions[0]?.revisionId
      ?? null;
    window.dispatchEvent(new CustomEvent("nirmata:scope-changed"));

    if (state.selectedUri) {
      await loadSelection(state.selectedUri, true);
      return;
    }

    const pending = Array.from(state.pendingDrafts.values());
    if (pending.length > 0) {
      await Promise.all(pending.map((record) => readPendingDraft(record)));
      if (requestId !== state.navigationRequestId) {
        return;
      }
    }

    renderWorkspace();
    setStatus("Navegación actualizada.");
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

export async function selectUri(uri: string): Promise<boolean> {
  if (state.selectedUri === uri && state.selectedObject && state.context) {
    return true;
  }
  if (!confirmDiscardPending("editor")) {
    return false;
  }
  await loadSelection(uri, false);
  return true;
}

export function startCreatingObject(kind: SearchObjectKind): boolean {
  if (state.session?.read_only) {
    showError("Vuelve a la versión actual antes de crear objetos.");
    return false;
  }
  if (!confirmDiscardPending("editor")) {
    return false;
  }
  state.workspaceNotice = null;
  state.editorMode = buildCreateEditor(kind);
  renderWorkspace();
  return true;
}

export async function selectUriInScope(uri: string, scope: ReadScope): Promise<boolean> {
  const session = state.session;
  if (!session) {
    return false;
  }
  const current = session.read_scope;
  if (current.variantId === scope.variantId && current.revisionId === scope.revisionId) {
    return selectUri(uri);
  }
  if (!confirmDiscardPending("editor")) {
    return false;
  }
  setSession(await invoke<WorldSession>("set_read_scope", { input: { scope } }));
  state.editorMode = null;
  await refreshNavigation();
  await loadSelection(uri, false);
  return true;
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
  if (!confirmDiscardPending("editor")) {
    return;
  }
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
      detail: "El formulario reutiliza el flujo existente y volverá a validar el conjunto de cambios al guardar.",
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
    showError("La versión observada es de solo lectura. Vuelve a la versión actual para editar.");
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
  setStatus("Preparando conjunto de cambios…");
  try {
    const response = await invoke<ManualDraftResponse>("preview_manual_draft", { input: request });
    if (!state.editorMode) {
      return;
    }
    state.editorMode.issues = response.fieldIssues;
    if (!response.draft) {
      state.workspaceNotice = {
        kind: "warning",
        title: "Propuesta no creada",
        detail: "Corrige los campos marcados antes de preparar el conjunto de cambios.",
      };
      renderWorkspace();
      setStatus("La propuesta requiere correcciones.");
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
      title: "Propuesta preparada",
      detail: "El conjunto de cambios está listo para revisarse en el panel inferior.",
    };
    state.editorMode = null;
    renderWorkspace();
    setStatus(`Propuesta lista: ${response.draft.title}.`);
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

export function confirmDiscardPending(scope: "editor" | "workspace" = "workspace"): boolean {
  const shouldConfirm = editorIsDirty(state.editorMode)
    || (scope === "workspace" && (state.pendingDrafts.size > 0 || state.ephemeralWork.size > 0));
  if (!shouldConfirm) {
    return true;
  }
  const previousFocus = document.activeElement instanceof HTMLElement
    ? document.activeElement
    : null;
  const volatileItems = Array.from(state.ephemeralWork.values());
  const accepted = window.confirm(
    scope === "workspace"
      ? volatileItems.length > 0
        ? `Hay trabajo de sesión que se perderá: ${volatileItems.join(", ")}. También se descartará cualquier formulario o revisión local pendiente. ¿Continuar?`
        : "Hay cambios locales o revisiones pendientes. ¿Descartar y continuar?"
      : "Hay cambios sin guardar en el formulario. ¿Descartar y continuar?",
  );
  if (!accepted) {
    previousFocus?.focus();
  }
  return accepted;
}
