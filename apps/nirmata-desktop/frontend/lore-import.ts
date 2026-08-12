import { buildCreateEditor } from "./editor-create.js";
import { clearError, humanize, setStatus, showError } from "./helpers.js";
import { renderPending } from "./render-pending.js";
import {
  dialog,
  invoke,
  loreFilter,
  loreImportCancel,
  loreImportContent,
  loreImportDelete,
  loreImportExtract,
  loreImportReview,
  loreImportSelect,
  loreImportStatus,
  state,
} from "./state.js";
import type {
  ImportBatchSnapshot,
  ImportCandidate,
  ImportCandidateSnapshot,
  ImportReviewPreparation,
  ManualDraftPreview,
  ManualReviewSnapshot,
} from "./types.js";

let batch: ImportBatchSnapshot | null = null;
let candidates: ImportCandidateSnapshot[] = [];
let activeRequestId: string | null = null;

function requestId(): string {
  return globalThis.crypto.randomUUID();
}

function candidateText(candidate: ImportCandidate): string {
  switch (candidate.kind) {
    case "entity": return candidate.summary;
    case "relation": return candidate.relationKind;
    case "event": return candidate.bodyMd;
    case "claim": return candidate.contentMd;
    case "rule": return candidate.statementMd;
  }
}

function editedCandidate(candidate: ImportCandidate, value: string): ImportCandidate {
  switch (candidate.kind) {
    case "entity": return { ...candidate, summary: value };
    case "relation": return { ...candidate, relationKind: value };
    case "event": return { ...candidate, bodyMd: value };
    case "claim": return { ...candidate, contentMd: value };
    case "rule": return { ...candidate, statementMd: value };
  }
}

function render(): void {
  const readOnly = state.session?.read_only ?? false;
  loreImportContent.replaceChildren();
  loreImportSelect.disabled = readOnly || activeRequestId !== null;
  loreImportExtract.disabled = readOnly || !batch || activeRequestId !== null;
  loreImportCancel.disabled = activeRequestId === null;
  loreImportReview.disabled = readOnly
    || candidates.every((candidate) => candidate.status !== "selected")
    || activeRequestId !== null;
  loreImportDelete.disabled = !batch || activeRequestId !== null;
  loreImportStatus.textContent = batch
    ? `${batch.sources.length} fuente · ${candidates.length} candidatos · variante ${batch.variantId.slice(0, 8)} · base ${batch.targetRevision.slice(0, 8)}`
    : readOnly
      ? "Solo lectura: vuelve a la cabeza activa para importar lore."
      : "Sin lote activo.";
  if (!batch) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "Selecciona una fuente local. HTML, macros, enlaces e instrucciones permanecen texto inerte.";
    loreImportContent.append(empty);
    return;
  }

  for (const source of batch.sources) {
    const details = document.createElement("details");
    details.open = true;
    const summary = document.createElement("summary");
    summary.textContent = `${source.fileName} · ${source.sizeBytes} bytes · ${source.contentHash.slice(0, 20)}…`;
    const preview = document.createElement("pre");
    preview.className = "lore-preview";
    preview.textContent = source.preview;
    const chunkList = document.createElement("div");
    chunkList.className = "citation-list";
    for (const chunk of source.chunks) {
      const open = document.createElement("button");
      open.type = "button";
      open.className = "ghost";
      open.textContent = `L${chunk.lineStart}-${chunk.lineEnd} · bytes ${chunk.byteStart}-${chunk.byteEnd}`;
      open.addEventListener("click", async () => {
        try {
          const location = await invoke<{ sourcePath: string; originalMatchesHash: boolean; chunk: { content: string } }>("open_lore_chunk", {
            input: { batchId: batch!.id, chunkId: chunk.id },
          });
          preview.textContent = location.chunk.content;
          preview.title = `${location.sourcePath} · original ${location.originalMatchesHash ? "sin cambios" : "cambió desde la ingesta"}`;
        } catch (value) { showError(value); }
      });
      chunkList.append(open);
    }
    details.append(summary, preview, chunkList);
    loreImportContent.append(details);
  }

  for (const item of candidates) {
    const card = document.createElement("article");
    card.className = "lore-candidate";
    const heading = document.createElement("h4");
    heading.textContent = `${humanize(item.candidate.kind)} · ${item.candidate.candidateId}`;
    const confidence = document.createElement("p");
    confidence.className = "muted";
    confidence.textContent = `Confianza técnica ${(item.candidate.technicalConfidence * 100).toFixed(0)} %; no es confianza diegética.`;
    const editor = document.createElement("textarea");
    editor.rows = 3;
    editor.value = candidateText(item.candidate);
    const save = document.createElement("button");
    save.type = "button";
    save.className = "secondary";
    save.textContent = "Guardar edición candidata";
    save.addEventListener("click", async () => {
      try {
        candidates = await invoke<ImportCandidateSnapshot[]>("edit_lore_candidate", {
          input: { batchId: batch!.id, candidateId: item.id, replacement: editedCandidate(item.candidate, editor.value) },
        });
        render();
      } catch (value) { showError(value); }
    });
    const identity = document.createElement("select");
    const choices = item.identityMatches.map((match) => ({ value: `exact:${match.uri}`, label: `Enlazar ${match.name}` }));
    choices.push({ value: "new", label: "Crear identidad nueva" });
    if (item.identitySuggestion === "ambiguous") choices.push({ value: "ambiguous", label: "Mantener ambigua" });
    for (const choice of choices) {
      const option = document.createElement("option");
      option.value = choice.value;
      option.textContent = choice.label;
      option.selected = choice.value.startsWith(item.identityDecision ?? item.identitySuggestion);
      identity.append(option);
    }
    const select = document.createElement("button");
    select.type = "button";
    select.textContent = item.status === "selected" ? "Seleccionado" : "Seleccionar";
    select.addEventListener("click", () => void decide(item, identity.value, true));
    const reject = document.createElement("button");
    reject.type = "button";
    reject.className = "ghost";
    reject.textContent = item.status === "rejected" ? "Rechazado" : "Rechazar";
    reject.addEventListener("click", () => void decide(item, identity.value, false));
    const citations = document.createElement("p");
    citations.className = "path";
    citations.textContent = `Chunks: ${item.candidate.citations.map((citation) => citation.chunkId).join(", ")}`;
    const actions = document.createElement("div");
    actions.className = "pending-actions";
    actions.append(save, identity, select, reject);
    card.append(heading, confidence, editor, citations, actions);
    loreImportContent.append(card);
  }
}

async function decide(item: ImportCandidateSnapshot, value: string, selected: boolean): Promise<void> {
  try {
    const identity = !selected
      ? null
      : value.startsWith("exact:")
        ? { kind: "exact", canonicalUri: value.slice(6) }
        : { kind: value };
    candidates = await invoke<ImportCandidateSnapshot[]>("decide_lore_candidate", {
      input: { batchId: batch!.id, decision: { candidateId: item.id, selected, identity } },
    });
    render();
  } catch (value) { showError(value); }
}

loreImportSelect.addEventListener("click", async () => {
  if (state.session?.read_only) {
    showError("Vuelve a la cabeza activa antes de importar lore.");
    return;
  }
  try {
    clearError();
    const selected = await dialog.open({ multiple: false, directory: false, filters: loreFilter });
    if (selected === null) return;
    batch = await invoke<ImportBatchSnapshot>("create_lore_import", { input: { sourceFile: selected } });
    candidates = [];
    render();
  } catch (value) { showError(value); }
});

loreImportExtract.addEventListener("click", async () => {
  if (!batch) return;
  activeRequestId = requestId();
  render();
  setStatus("Extrayendo candidatos citados…");
  try {
    await invoke("extract_lore_import", { input: { requestId: activeRequestId, batchId: batch.id } });
    candidates = await invoke<ImportCandidateSnapshot[]>("read_lore_candidates", { input: { batchId: batch.id } });
    setStatus("Candidatos listos para editar, seleccionar o rechazar.");
  } catch (value) { showError(value); }
  activeRequestId = null;
  render();
});

loreImportCancel.addEventListener("click", async () => {
  if (!activeRequestId) return;
  try { await invoke("cancel_ai_request", { requestId: activeRequestId }); } catch (value) { showError(value); }
});

loreImportDelete.addEventListener("click", async () => {
  if (!batch) return;
  try {
    await invoke("delete_lore_import", { input: { batchId: batch.id } });
    batch = null;
    candidates = [];
    render();
    setStatus("Lote eliminado; canon y original permanecen intactos.");
  } catch (value) { showError(value); }
});

loreImportReview.addEventListener("click", async () => {
  if (!batch) return;
  activeRequestId = requestId();
  render();
  try {
    const prepared = await invoke<ImportReviewPreparation>("prepare_lore_import_review", {
      input: { requestId: activeRequestId, batchId: batch.id },
    });
    if (prepared.decisionPoints.length > 0) {
      loreImportStatus.textContent = prepared.decisionPoints.map((point) => point.prompt).join(" · ");
      return;
    }
    if (!prepared.reviewKey || !prepared.run) return;
    const review = await invoke<ManualReviewSnapshot>("read_manual_review", { input: { reviewKey: prepared.reviewKey } });
    const first = review.operations[0];
    const objectType = (first?.after?.objectType ?? first?.before?.objectType ?? "entity") as ManualDraftPreview["objectType"];
    state.pendingDrafts.set(review.reviewKey, {
      preview: {
        draftKey: review.reviewKey,
        targetUri: first?.targetUri ?? review.reviewKey,
        objectType,
        mode: first?.before ? "update" : "create",
        title: "Importación de lore",
        objective: review.objective,
        sourceUris: review.sources,
        assumptions: review.assumptions,
        logicalPath: "Importación revisada",
        validationReport: review.validationReport,
        readyToConfirm: review.readyToConfirm,
      },
      review,
      editor: buildCreateEditor(objectType === "world" ? "entity" : objectType),
      aiRunId: prepared.run.id,
    });
    renderPending();
    setStatus("ChangeSet de importación abierto en revisión estándar.");
  } catch (value) { showError(value); }
  finally { activeRequestId = null; render(); }
});

window.addEventListener("nirmata:scope-changed", render);
render();
