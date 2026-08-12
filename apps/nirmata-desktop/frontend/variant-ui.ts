import { buildWorldEditor } from "./editor-model.js";
import { clearError, humanize, setStatus, shortId, showError } from "./helpers.js";
import { renderPending } from "./render-pending.js";
import {
  compareVariantSelect,
  compareVariantsButton,
  createVariantForm,
  invoke,
  mergeVariantButton,
  readScopeLabel,
  revisionScopeSelect,
  state,
  variantDiff,
  variantNameInput,
  variantSelect,
  viewActiveHeadButton,
} from "./state.js";
import type {
  MergeReviewResult,
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
  renderWorkspace,
  selectUri,
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
  if (!confirmDiscardPending()) {
    return;
  }
  const session = await invoke<WorldSession>("set_read_scope", { input: { scope } });
  state.session = session;
  state.editorMode = null;
  await refreshNavigation();
  const uri = objectUri(diff);
  if (uri) {
    await selectUri(uri);
  }
}

function renderComparison(comparison: VariantComparison): void {
  if (comparison.differences.length === 0) {
    variantDiff.textContent = "Sin diferencias por ID estable.";
    return;
  }
  const list = document.createElement("ul");
  for (const difference of comparison.differences) {
    const item = document.createElement("li");
    const label = document.createElement("span");
    label.textContent = `${humanize(difference.kind)} · ${objectUri(difference)}`;
    const left = document.createElement("button");
    left.type = "button";
    left.className = "ghost";
    left.textContent = "Abrir izquierda";
    left.disabled = difference.before === null;
    left.addEventListener("click", () => void openDiffSide(difference, difference.leftScope));
    const right = document.createElement("button");
    right.type = "button";
    right.className = "ghost";
    right.textContent = "Abrir derecha";
    right.disabled = difference.after === null;
    right.addEventListener("click", () => void openDiffSide(difference, difference.rightScope));
    item.append(label, left, right);
    list.append(item);
  }
  variantDiff.replaceChildren(list);
}

function mergeRecord(result: MergeReviewResult): PendingDraftRecord {
  const editor = buildWorldEditor();
  if (!editor) {
    throw new Error("No hay mundo activo para presentar el merge.");
  }
  editor.title = "Merge de variantes";
  editor.description = "ChangeSet normal sobre la cabeza destino; la fuente permanece intacta.";
  editor.fields = [];
  editor.values = {};
  editor.baselineValues = {};
  return {
    preview: {
      draftKey: result.review.reviewKey,
      targetUri: result.review.reviewKey,
      objectType: "world",
      mode: "update",
      title: "Merge de variantes",
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
      option.textContent = `${variant.name} · ${shortId(variant.headRevisionId)}`;
      option.selected = variant.id === active.id;
      return option;
    }));
    compareVariantSelect.replaceChildren(...variants.filter((variant) => !variant.archived && variant.id !== observed.id).map((variant) => {
      const option = document.createElement("option");
      option.value = variant.id;
      option.textContent = `${variant.name} · ${shortId(variant.headRevisionId)}`;
      return option;
    }));
    const head = document.createElement("option");
    head.value = "";
    head.textContent = `Cabeza · ${shortId(observed.headRevisionId)}`;
    revisionScopeSelect.replaceChildren(head);
    for (const revision of state.revisionHistory?.revisions ?? []) {
      const option = document.createElement("option");
      option.value = revision.revisionId;
      option.textContent = `${shortId(revision.revisionId)} · ${revision.summary}`;
      option.selected = scope.revisionId === revision.revisionId;
      revisionScopeSelect.append(option);
    }
    const readOnly = state.session.read_only;
    readScopeLabel.textContent = readOnly
      ? `Solo lectura: ${observed.name} / ${shortId(scope.revisionId ?? observed.headRevisionId)}`
      : `Escritura activa: ${active.name} / ${shortId(active.headRevisionId)}`;
    readScopeLabel.classList.toggle("read-only", readOnly);
    viewActiveHeadButton.disabled = !readOnly;
    mergeVariantButton.disabled = readOnly || !compareVariantSelect.value;
    compareVariantsButton.disabled = !compareVariantSelect.value;
  } catch (value) {
    showError(value);
  } finally {
    refreshing = false;
  }
}

variantSelect.addEventListener("change", async () => {
  if (!confirmDiscardPending()) {
    variantSelect.value = state.session?.active_variant.id ?? "";
    return;
  }
  try {
    clearError();
    const session = await invoke<WorldSession>("switch_variant", {
      input: { variantId: variantSelect.value },
    });
    state.session = session;
    state.pendingDrafts.clear();
    state.editorMode = null;
    variantDiff.replaceChildren();
    await refreshNavigation();
  } catch (value) {
    showError(value);
  }
});

revisionScopeSelect.addEventListener("change", async () => {
  if (!confirmDiscardPending()) {
    return;
  }
  try {
    const revisionId = revisionScopeSelect.value || null;
    const scope: ReadScope = {
      variantId: state.session!.read_scope.variantId,
      revisionId,
    };
    state.session = await invoke<WorldSession>("set_read_scope", { input: { scope } });
    state.editorMode = null;
    await refreshNavigation();
  } catch (value) {
    showError(value);
  }
});

viewActiveHeadButton.addEventListener("click", async () => {
  try {
    state.session = await invoke<WorldSession>("view_active_head");
    await refreshNavigation();
  } catch (value) {
    showError(value);
  }
});

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
    setStatus("Variante creada sin cambiar la cabeza activa.");
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
