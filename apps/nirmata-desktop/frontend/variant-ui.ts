import { buildWorldEditor } from "./editor-model.js";
import { clearError, formatTimestamp, humanize, setStatus, shortId, showError } from "./helpers.js";
import { renderPending } from "./render-pending.js";
import {
  compareVariantSelect,
  compareVariantsButton,
  createVariantForm,
  archiveVariantButton,
  dialog,
  exportVfsSnapshotButton,
  importVfsSnapshotButton,
  invoke,
  mergeVariantButton,
  readScopeLabel,
  renameVariantForm,
  renameVariantNameInput,
  revisionScopeSelect,
  setSession,
  state,
  variantDiff,
  variantNameInput,
  variantSelect,
  viewActiveHeadButton,
} from "./state.js";
import type {
  MergeReviewResult,
  ExportSnapshotResult,
  ImportSnapshotResult,
  PendingDraftRecord,
  ReadScope,
  Variant,
  VariantComparison,
  VariantDiff,
  WorldSession,
} from "./types.js";
import {
  confirmDiscardPending,
  refreshNavigation,
  selectUriInScope,
} from "./workspace.js";

let variants: Variant[] = [];
let refreshing = false;

function objectUri(diff: VariantDiff): string {
  const [kind, id] = Object.entries(diff.objectRef)[0] ?? [];
  return kind && id ? `nirmata://${kind}/${id}` : "";
}

function currentScope(): ReadScope | null {
  return state.session?.read_scope ?? null;
}

async function openDiffSide(diff: VariantDiff, scope: ReadScope): Promise<void> {
  const uri = objectUri(diff);
  if (uri) {
    await selectUriInScope(uri, scope);
  }
}

function renderComparison(comparison: VariantComparison): void {
  if (comparison.differences.length === 0) {
    variantDiff.textContent = "No hay diferencias entre las versiones seleccionadas.";
    return;
  }
  const list = document.createElement("ul");
  for (const difference of comparison.differences) {
    const item = document.createElement("li");
    const label = document.createElement("span");
    label.textContent = humanize(difference.kind);
    const provenance = document.createElement("small");
    const source = difference.rightSource ?? difference.leftSource;
    provenance.textContent = source
      ? `Tipo de cambio: ${humanize(source.retcon)}`
      : "Objeto inicial o importado antes del historial disponible.";
    const references = document.createElement("small");
    references.textContent = difference.affectedReferences.length > 0
      ? `${difference.affectedReferences.length} referencia(s) afectada(s).`
      : "Sin referencias estructuradas afectadas.";
    const left = document.createElement("button");
    left.type = "button";
    left.className = "ghost";
    const leftVariant = variants.find((variant) => variant.id === difference.leftScope.variantId);
    const rightVariant = variants.find((variant) => variant.id === difference.rightScope.variantId);
    left.textContent = `Abrir ${leftVariant?.name ?? "origen"}`;
    left.disabled = difference.before === null;
    left.addEventListener("click", () => void openDiffSide(difference, difference.leftScope));
    const right = document.createElement("button");
    right.type = "button";
    right.className = "ghost";
    right.textContent = `Abrir ${rightVariant?.name ?? "destino"}`;
    right.disabled = difference.after === null;
    right.addEventListener("click", () => void openDiffSide(difference, difference.rightScope));
    item.append(label, provenance, references, left, right);
    list.append(item);
  }
  variantDiff.replaceChildren(list);
}

function mergeRecord(result: MergeReviewResult): PendingDraftRecord {
  const editor = buildWorldEditor();
  if (!editor) {
    throw new Error("No hay mundo activo para presentar el merge.");
  }
  editor.title = "Traer cambios de otra variante";
  editor.description = "La propuesta se aplicará en la variante activa; la variante de origen permanecerá intacta.";
  editor.fields = [];
  editor.values = {};
  editor.baselineValues = {};
  return {
    preview: {
      draftKey: result.review.reviewKey,
      targetUri: result.review.reviewKey,
      objectType: "world",
      mode: "update",
      title: "Traer cambios de otra variante",
      objective: result.review.objective,
      sourceUris: result.review.sources,
      assumptions: result.review.assumptions,
      logicalPath: "/",
      validationReport: result.review.validationReport,
      readyToConfirm: result.review.readyToConfirm,
    },
    review: result.review,
    editor,
  };
}

function snapshotRecord(result: ImportSnapshotResult): PendingDraftRecord {
  const editor = buildWorldEditor();
  if (!editor) {
    throw new Error("No hay mundo activo para presentar el snapshot.");
  }
  editor.title = "Importación de snapshot";
  editor.description = "Cambios externos revisables; importar nunca escribe canon directamente.";
  editor.fields = [];
  editor.values = {};
  editor.baselineValues = {};
  return {
    preview: {
      draftKey: result.review.reviewKey,
      targetUri: result.review.reviewKey,
      objectType: "world",
      mode: "update",
      title: "Importación de snapshot",
      objective: result.review.objective,
      sourceUris: result.review.sources,
      assumptions: result.review.assumptions,
      logicalPath: result.path,
      validationReport: result.review.validationReport,
      readyToConfirm: result.review.readyToConfirm,
    },
    review: result.review,
    editor,
  };
}

async function refreshVariantPanel(): Promise<void> {
  if (!state.session || refreshing) {
    return;
  }
  refreshing = true;
  try {
    variants = await invoke<Variant[]>("list_variants");
    const active = state.session.active_variant;
    const scope = currentScope()!;
    const observed = variants.find((variant) => variant.id === scope.variantId) ?? active;
    variantSelect.replaceChildren(...variants.filter((variant) => !variant.archived).map((variant) => {
      const option = document.createElement("option");
      option.value = variant.id;
      option.textContent = variant.name;
      option.selected = variant.id === active.id;
      return option;
    }));
    compareVariantSelect.replaceChildren(...variants.filter((variant) => !variant.archived && variant.id !== observed.id).map((variant) => {
      const option = document.createElement("option");
      option.value = variant.id;
      option.textContent = variant.name;
      return option;
    }));
    const head = document.createElement("option");
    head.value = "";
    head.textContent = "Versión actual";
    revisionScopeSelect.replaceChildren(head);
    for (const revision of state.revisionHistory?.revisions ?? []) {
      const option = document.createElement("option");
      option.value = revision.revisionId;
      option.textContent = `${formatTimestamp(revision.createdAtMs)} · ${revision.summary}`;
      option.selected = scope.revisionId === revision.revisionId;
      revisionScopeSelect.append(option);
    }
    const readOnly = state.session.read_only;
    const viewedRevision = state.revisionHistory?.revisions.find((revision) =>
      revision.revisionId === scope.revisionId
    );
    readScopeLabel.textContent = readOnly
      ? `Escribiendo en: ${active.name} · Viendo: ${observed.name}, ${viewedRevision?.summary ?? "versión anterior"} · Solo lectura`
      : `Escribiendo en: ${active.name} · Viendo: versión actual`;
    readScopeLabel.classList.toggle("read-only", readOnly);
    viewActiveHeadButton.disabled = !readOnly;
    mergeVariantButton.disabled = readOnly || !compareVariantSelect.value;
    compareVariantsButton.disabled = !compareVariantSelect.value;
    archiveVariantButton.disabled = !compareVariantSelect.value;
    importVfsSnapshotButton.disabled = readOnly;
  } catch (value) {
    showError(value);
  } finally {
    refreshing = false;
  }
}

variantSelect.addEventListener("change", async () => {
  const switched = await switchWritingVariant(variantSelect.value);
  if (!switched) variantSelect.value = state.session?.active_variant.id ?? "";
});

export async function switchWritingVariant(variantId: string): Promise<boolean> {
  if (!confirmDiscardPending("workspace")) {
    return false;
  }
  try {
    clearError();
    const session = await invoke<WorldSession>("switch_variant", {
      input: { variantId },
    });
    setSession(session);
    state.pendingDrafts.clear();
    window.dispatchEvent(new CustomEvent("nirmata:discard-ephemeral-work"));
    state.editorMode = null;
    variantDiff.replaceChildren();
    await refreshNavigation();
    return true;
  } catch (value) {
    showError(value);
    return false;
  }
}

revisionScopeSelect.addEventListener("change", async () => {
  const changed = await observeRevision(revisionScopeSelect.value || null);
  if (!changed) {
    revisionScopeSelect.value = state.session?.read_scope.revisionId ?? "";
  }
});

export async function observeRevision(revisionId: string | null): Promise<boolean> {
  if (!state.session || !confirmDiscardPending("editor")) return false;
  try {
    const scope: ReadScope = {
      variantId: state.session!.read_scope.variantId,
      revisionId,
    };
    setSession(await invoke<WorldSession>("set_read_scope", { input: { scope } }));
    state.editorMode = null;
    await refreshNavigation();
    return true;
  } catch (value) {
    showError(value);
    return false;
  }
}

viewActiveHeadButton.addEventListener("click", async () => {
  await viewActiveVersion();
});

export async function viewActiveVersion(): Promise<boolean> {
  if (!confirmDiscardPending("editor")) return false;
  try {
    setSession(await invoke<WorldSession>("view_active_head"));
    state.editorMode = null;
    await refreshNavigation();
    return true;
  } catch (value) {
    showError(value);
    return false;
  }
}

createVariantForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const name = variantNameInput.value.trim();
  if (!name || !state.session) {
    return;
  }
  try {
    const observed = variants.find((variant) => variant.id === state.session!.read_scope.variantId);
    const fromRevisionId = state.session.read_scope.revisionId
      ?? observed?.headRevisionId
      ?? state.session.active_variant.headRevisionId;
    await invoke<Variant>("create_variant", { input: { name, fromRevisionId } });
    variantNameInput.value = "";
    await refreshVariantPanel();
    setStatus("Variante creada sin cambiar la versión donde estás escribiendo.");
  } catch (value) {
    showError(value);
  }
});

renameVariantForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const name = renameVariantNameInput.value.trim();
  const active = state.session?.active_variant;
  if (!name || !active) {
    return;
  }
  try {
    const variant = await invoke<Variant>("rename_variant", {
      input: { variantId: active.id, name },
    });
    setSession({ ...state.session!, active_variant: variant });
    renameVariantNameInput.value = "";
    await refreshVariantPanel();
    setStatus(`Variante renombrada a ${variant.name}.`);
  } catch (value) {
    showError(value);
  }
});

archiveVariantButton.addEventListener("click", async () => {
  const variant = variants.find((item) => item.id === compareVariantSelect.value);
  if (!variant) {
    return;
  }
  try {
    await invoke("archive_variant", {
      input: { variantId: variant.id, allowReferenced: false },
    });
  } catch (value) {
    if (!String(value).includes("descendants or import references")
      || !window.confirm("La variante tiene descendientes o importaciones. ¿Archivarla igualmente?")) {
      showError(value);
      return;
    }
    await invoke("archive_variant", {
      input: { variantId: variant.id, allowReferenced: true },
    });
  }
  variantDiff.replaceChildren();
  await refreshVariantPanel();
  setStatus(`Variante ${variant.name} archivada.`);
});

exportVfsSnapshotButton.addEventListener("click", async () => {
  if (!state.session) {
    return;
  }
  const parentDirectory = await dialog.open({ multiple: false, directory: true });
  if (!parentDirectory) {
    return;
  }
  const fallback = `snapshot-${shortId(state.session.read_scope.revisionId ?? state.session.active_variant.headRevisionId)}`;
  const snapshotName = window.prompt("Nombre del directorio de snapshot", fallback)?.trim();
  if (!snapshotName) {
    return;
  }
  try {
    const result = await invoke<ExportSnapshotResult>("export_vfs_snapshot", {
      input: { parentDirectory, snapshotName },
    });
    setStatus(`Snapshot ${result.variant} exportado: ${result.objectCount} objetos en ${result.path}.`);
  } catch (value) {
    showError(value);
  }
});

importVfsSnapshotButton.addEventListener("click", async () => {
  if (!state.session || state.session.read_only) {
    showError("Vuelve a la versión actual antes de importar una copia estructurada.");
    return;
  }
  const snapshotDirectory = await dialog.open({ multiple: false, directory: true });
  if (!snapshotDirectory) {
    return;
  }
  try {
    const result = await invoke<ImportSnapshotResult>("import_vfs_snapshot", {
      input: { snapshotDirectory },
    });
    state.pendingDrafts.set(result.review.reviewKey, snapshotRecord(result));
    renderPending();
    setStatus(
      `Snapshot revisable: ${result.createdCount} altas, ${result.updatedCount} cambios y ${result.deletedCount} bajas.`,
    );
  } catch (value) {
    showError(value);
  }
});

compareVariantsButton.addEventListener("click", async () => {
  const source = variants.find((variant) => variant.id === compareVariantSelect.value);
  const left = currentScope();
  if (!source || !left) {
    return;
  }
  try {
    const right: ReadScope = { variantId: source.id, revisionId: null };
    renderComparison(await invoke<VariantComparison>("compare_variant_scopes", {
      input: { left, right },
    }));
  } catch (value) {
    showError(value);
  }
});

mergeVariantButton.addEventListener("click", async () => {
  const source = variants.find((variant) => variant.id === compareVariantSelect.value);
  if (!source || state.session?.read_only) {
    return;
  }
  try {
    const result = await invoke<MergeReviewResult>("prepare_variant_merge", {
      input: { scope: { variantId: source.id, revisionId: null } },
    });
    state.pendingDrafts.set(result.review.reviewKey, mergeRecord(result));
    renderPending();
    setStatus(
      `${result.automaticOperationIds.length} operaciones independientes y ${result.decisionOperationIds.length} decisiones manuales.`,
    );
  } catch (value) {
    showError(value);
  }
});

window.addEventListener("nirmata:scope-changed", () => void refreshVariantPanel());
void refreshVariantPanel();
