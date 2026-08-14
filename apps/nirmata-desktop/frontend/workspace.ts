import {
  buildWorldEditor,
  cloneStructuredEditorState,
  currentWorldUri,
} from "./editor-model.js";
import { buildCreateEditor } from "./editor-create.js";
import { requestConfirmation } from "./confirmation.js";
import {
  clearError,
  commandCode,
  commandMessage,
  retainedDraftHint,
  setStatus,
  showError,
  splitLines,
} from "./helpers.js";
import {
  appActions,
  getAppState,
  invoke,
} from "./state.js";
import type {
  StructuredEditorState,
  ManualDraftRequest,
  ManualDraftResponse,
  ManualReviewOperationSnapshot,
  PendingReviewSnapshot,
  ReadScope,
  ReviewEditContext,
  SearchObjectKind,
  WorkspaceNotice,
  WorldSession,
} from "./types.js";

export function editorIsDirty(mode: StructuredEditorState | null): boolean {
  if (!mode) {
    return false;
  }

  return (
    !sameEditorValues(mode.values, mode.baselineValues)
    || mode.objective !== mode.baselineObjective
    || mode.sourceUrisText !== mode.baselineSourceUrisText
    || mode.assumptionsText !== mode.baselineAssumptionsText
  );
}

function sameEditorValues(left: Record<string, string>, right: Record<string, string>): boolean {
  const keys = new Set([...Object.keys(left), ...Object.keys(right)]);
  for (const key of keys) {
    if ((left[key] ?? "") !== (right[key] ?? "")) return false;
  }
  return true;
}

export function hasPendingWork(): boolean {
  const state = getAppState();
  return Object.keys(state.ephemeralWork).length > 0
    || editorIsDirty(state.structuredEditor);
}

export function openSession(session: WorldSession): void {
  appActions.resetWorkspace(session, "");
}

export function closeSession(): void {
  appActions.resetWorkspace(null, "Mundo cerrado.");
}

export function applyCommandStateError(value: unknown, fallback?: string): void {
  const code = commandCode(value);
  const message = commandMessage(value);
  let notice: WorkspaceNotice | null = null;
  switch (code) {
    case "object_not_found":
      notice = {
        kind: "warning",
        title: "Objeto eliminado",
        detail: `La referencia seleccionada ya no existe en el mundo actual. ${message}`,
      };
      appActions.setSelectedLogicalPath(null);
      appActions.setStructuredEditor(null);
      setStatus("La selección ya no existe en canon.");
      break;
    case "no_world_open":
      notice = {
        kind: "warning",
        title: "Sesión no disponible",
        detail: "El backend no reconoce la sesión. El trabajo local se conserva hasta que decidas volver al inicio.",
      };
      setStatus("La acción no se ejecutó; comprueba la sesión antes de continuar.");
      break;
    case "manual_review_stale":
      notice = {
        kind: "warning",
        title: "Propuesta desactualizada",
        detail: `${message || "La propuesta quedó detrás de la versión actual; vuelve a comprobarla antes de aplicarla."}${retainedDraftHint()}`,
      };
      setStatus("La revisión manual requiere revalidación.");
      break;
    case "manual_review_not_ready":
    case "manual_review_revalidation_failed":
    case "validation_error":
      notice = {
        kind: "warning",
        title: "No se puede aplicar todavía",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El conjunto de cambios sigue pendiente de revisión.");
      break;
    case "project_locked":
      notice = {
        kind: "warning",
        title: "Archivo bloqueado",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("No se pudo escribir porque el archivo está en uso.");
      break;
    case "app_busy":
      notice = {
        kind: "warning",
        title: "IA trabajando",
        detail: "Espera a que termine la solicitud activa o cancélala antes de cambiar el mundo.",
      };
      setStatus("La acción no se ejecutó; el mundo y el trabajo local se conservaron.");
      break;
    case "constraint_error":
      notice = {
        kind: "warning",
        title: "El almacenamiento rechazó el cambio",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("La propuesta se conservó para corregirla o reintentarlo.");
      break;
    case "storage_error":
      notice = {
        kind: "warning",
        title: "La transacción no pudo completarse",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("El canon no cambió. La propuesta se conservó y puedes reintentar.");
      break;
    case "file_not_found":
      notice = {
        kind: "warning",
        title: "Archivo movido o inexistente",
        detail: `El archivo .nirmata activo ya no se encuentra en la ruta original. ${message}`,
      };
      setStatus("");
      break;
    case "file_error":
      notice = {
        kind: "warning",
        title: "Error de archivo",
        detail: `${message}${retainedDraftHint()}`,
      };
      setStatus("");
      break;
    case "invalid_project_path":
      notice = {
        kind: "warning",
        title: "Ruta inválida",
        detail: "Selecciona un archivo local con extensión .nirmata.",
      };
      setStatus(fallback ?? "");
      break;
    case "invalid_object_uri":
    case "invalid_revision_id":
      notice = {
        kind: "warning",
        title: "Referencia o versión inválida",
        detail: "Vuelve a elegir el objeto o la versión desde la interfaz.",
      };
      setStatus(fallback ?? "");
      break;
    case "undo_target_invalid":
    case "undo_conflict":
      notice = {
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
  appActions.setWorkspaceNotice(notice);
  showError(value);
}

export async function selectUri(uri: string): Promise<boolean> {
  const state = getAppState();
  if (
    state.selectedUri === uri
    && state.structuredEditor?.existingUri === uri
  ) {
    return true;
  }
  if (!await confirmDiscardPending("editor")) {
    return false;
  }
  clearError();
  appActions.setWorkspaceNotice(null);
  appActions.selectUri(uri);
  setStatus("Cargando selección…");
  return true;
}

export async function startCreatingObject(kind: SearchObjectKind): Promise<boolean> {
  const state = getAppState();
  if (state.session?.read_only) {
    showError("Vuelve a la versión actual antes de crear objetos.");
    return false;
  }
  if (!await confirmDiscardPending("editor")) {
    return false;
  }
  appActions.setSelectedUri(null);
  appActions.setWorkspaceNotice(null);
  appActions.setStructuredEditor(buildCreateEditor(kind));
  return true;
}

export async function startEditingWorld(): Promise<boolean> {
  const state = getAppState();
  if (state.session?.read_only) {
    showError("El calendario de una versión anterior es de solo lectura. Vuelve a la versión actual para configurarlo.");
    return false;
  }
  if (!await confirmDiscardPending("editor")) {
    return false;
  }
  const editor = buildWorldEditor();
  if (!editor) {
    return false;
  }
  appActions.setSelectedUri(currentWorldUri());
  appActions.setWorkspaceNotice(null);
  appActions.setStructuredEditor(editor);
  return true;
}

export async function selectUriInScope(uri: string, scope: ReadScope): Promise<boolean> {
  const session = getAppState().session;
  if (!session) {
    return false;
  }
  const current = session.read_scope;
  if (current.variantId === scope.variantId && current.revisionId === scope.revisionId) {
    return selectUri(uri);
  }
  if (!await confirmDiscardPending("editor")) {
    return false;
  }
  appActions.setSession(await invoke<WorldSession>("set_read_scope", { input: { scope } }));
  appActions.setWorkspaceNotice(null);
  appActions.selectUri(uri);
  setStatus("Navegación actualizada.");
  return true;
}

function applyManualRequestToEditor(
  editor: StructuredEditorState,
  request: ManualDraftRequest,
  reviewEdit: ReviewEditContext | null,
): StructuredEditorState {
  const next = cloneStructuredEditorState(editor);
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
  record: PendingReviewSnapshot,
  operation: ManualReviewOperationSnapshot,
): Promise<void> {
  if (!await confirmDiscardPending("editor")) {
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

    const editor = buildPendingReviewEditor(request);

    if (!editor) {
      setStatus("");
      return;
    }

    appActions.setWorkspaceNotice({
      kind: "info",
      title: "Edición por operación",
      detail: "El formulario reutiliza el flujo existente y volverá a validar el conjunto de cambios al guardar.",
    });
    appActions.setSelectedUri(null);
    appActions.setStructuredEditor(applyManualRequestToEditor(editor, request, {
      reviewKey: record.review.reviewKey,
      operationId: operation.operationId,
    }));
    setStatus("Operación lista para editar.");
  } catch (value) {
    applyCommandStateError(value, "");
  }
}

function currentManualRequest(): ManualDraftRequest | null {
  const editor = getAppState().structuredEditor;
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

export async function saveCurrentDraft(submittedRequest?: ManualDraftRequest): Promise<boolean> {
  const state = getAppState();
  if (state.session?.read_only) {
    showError("La versión observada es de solo lectura. Vuelve a la versión actual para editar.");
    return false;
  }
  const editor = state.structuredEditor;
  const request = submittedRequest ?? currentManualRequest();
  if (!editor || !request) return false;
  if (editor.reviewEdit) {
    clearError();
    setStatus("Revalidando operación editada…");
    try {
      const response = await invoke<ManualDraftResponse>("apply_manual_review_edit", {
        input: { reviewKey: editor.reviewEdit.reviewKey, operationId: editor.reviewEdit.operationId, request },
      });
      if (!getAppState().structuredEditor) return false;
      appActions.updateStructuredEditor((current) => ({ ...current, issues: response.fieldIssues }));
      if (!response.review) {
        appActions.setWorkspaceNotice({ kind: "warning", title: "Edición incompleta", detail: "Corrige los campos marcados antes de revalidar la operación." });
        setStatus("La operación editada requiere correcciones.");
        return false;
      }
      appActions.setWorkspaceNotice({ kind: "info", title: "Operación revalidada", detail: "El diff y el reporte de Cambios ya reflejan la edición aplicada." });
      appActions.setStructuredEditor(null);
      setStatus("Operación actualizada.");
      return true;
    } catch (value) {
      applyCommandStateError(value, "");
      return false;
    }
  }
  clearError();
  setStatus("Preparando conjunto de cambios…");
  try {
    const response = await invoke<ManualDraftResponse>("preview_manual_draft", { input: request });
    if (!getAppState().structuredEditor) return false;
    appActions.updateStructuredEditor((current) => ({ ...current, issues: response.fieldIssues }));
    if (!response.draft) {
      appActions.setWorkspaceNotice({ kind: "warning", title: "Propuesta no creada", detail: "Corrige los campos marcados antes de preparar el conjunto de cambios." });
      setStatus("La propuesta requiere correcciones.");
      return false;
    }
    if (!response.review) throw new Error("La revisión manual no quedó disponible para este draft.");
    appActions.setWorkspaceNotice({ kind: "info", title: "Propuesta preparada", detail: "El conjunto de cambios está listo para revisarse en Cambios." });
    appActions.setStructuredEditor(null);
    setStatus(`Propuesta lista: ${response.draft.title}.`);
    return true;
  } catch (value) {
    applyCommandStateError(value, "");
    return false;
  }
}

export function resetCurrentEditor(): void {
  const state = getAppState();
  if (!state.structuredEditor) return;
  const editor = cloneStructuredEditorState(state.structuredEditor);
  editor.values = { ...editor.baselineValues };
  editor.objective = editor.baselineObjective;
  editor.sourceUrisText = editor.baselineSourceUrisText;
  editor.assumptionsText = editor.baselineAssumptionsText;
  editor.issues = [];
  appActions.setStructuredEditor(editor);
  appActions.setWorkspaceNotice(null);
}

export function buildPendingReviewEditor(request: ManualDraftRequest): StructuredEditorState {
  const base = request.objectType === "world"
    ? buildWorldEditor() ?? buildCreateEditor("entity")
    : buildCreateEditor(request.objectType as SearchObjectKind);
  const editor = applyManualRequestToEditor(base, request, null);
  editor.mode = request.existingUri ? "update" : "create";
  editor.existingUri = request.existingUri ?? null;
  editor.targetUri = request.existingUri ?? null;
  return editor;
}

export async function confirmDiscardPending(scope: "editor" | "workspace" = "workspace"): Promise<boolean> {
  const state = getAppState();
  const shouldConfirm = editorIsDirty(state.structuredEditor)
    || Boolean(state.ephemeralWork.editor)
    || (scope === "workspace" && Object.keys(state.ephemeralWork).length > 0);
  if (!shouldConfirm) return true;
  const volatileItems = Object.values(state.ephemeralWork);
  return requestConfirmation({
    title: scope === "workspace" ? "Descartar trabajo de sesión" : "Descartar cambios del formulario",
    detail: scope === "workspace"
      ? volatileItems.length > 0
        ? `Hay trabajo de sesión que se perderá: ${volatileItems.join(", ")}. También se descartará cualquier formulario sin guardar.`
        : "Hay cambios locales sin guardar que se perderán."
      : "Hay cambios sin guardar en el formulario.",
    confirmLabel: "Descartar y continuar",
    danger: true,
  });
}
