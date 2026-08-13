import { buildCreateEditor } from "./editor-create.js";
import { clearError, humanize, setStatus, showError } from "./helpers.js";
import { renderPending } from "./render-pending.js";
import {
  dialog,
  beginAiActivity,
  endAiActivity,
  invoke,
  listen,
  loreFilter,
  loreImportCancel,
  loreImportContent,
  loreImportDelete,
  loreImportExtract,
  loreImportReview,
  loreImportSelect,
  loreImportStatus,
  setEphemeralWork,
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

type ImportDecisionPoint = ImportReviewPreparation["decisionPoints"][number];
type ProgressEvent = {
  requestId: string;
  progress: { kind: string };
};

let batch: ImportBatchSnapshot | null = null;
let candidates: ImportCandidateSnapshot[] = [];
let activeRequestId: string | null = null;
let decisionPoints: ImportDecisionPoint[] = [];
let statusMessage: string | null = null;
let batchReviewKey: string | null = null;

function setRunning(nextRequestId: string | null, label = ""): void {
  const previousRequestId = activeRequestId;
  activeRequestId = nextRequestId;
  if (nextRequestId) {
    beginAiActivity({ requestId: nextRequestId, source: "lore", label });
  } else if (previousRequestId) {
    endAiActivity(previousRequestId);
  }
  render();
}

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

function editedCandidate(
  candidate: ImportCandidate,
  value: string,
  authentication?: string,
): ImportCandidate {
  switch (candidate.kind) {
    case "entity": return { ...candidate, summary: value };
    case "relation": return { ...candidate, relationKind: value };
    case "event": return { ...candidate, bodyMd: value };
    case "claim": return { ...candidate, contentMd: value, authentication: authentication ?? candidate.authentication };
    case "rule": return { ...candidate, statementMd: value };
  }
}

function render(): void {
  const readOnly = state.session?.read_only ?? false;
  loreImportContent.replaceChildren();
  loreImportSelect.disabled = readOnly || activeRequestId !== null;
  loreImportExtract.disabled = readOnly || !batch || !state.aiProviderReady || activeRequestId !== null;
  loreImportCancel.disabled = activeRequestId === null;
  loreImportReview.disabled = readOnly
    || candidates.every((candidate) => candidate.status !== "selected")
    || !state.aiProviderReady
    || activeRequestId !== null;
  loreImportDelete.disabled = !batch || activeRequestId !== null;
  loreImportStatus.textContent = statusMessage ?? (batch
    ? candidates.length === 0
      ? `${batch.sources.length} fuente${batch.sources.length === 1 ? "" : "s"} preparada${batch.sources.length === 1 ? "" : "s"}. Siguiente acción: extraer candidatos.`
      : decisionPoints.length > 0
        ? `${decisionPoints.length} decisión pendiente. Resuélvela antes de abrir la revisión.`
        : `${candidates.length} candidatos. Selecciona o rechaza cada hallazgo y abre la revisión.`
    : readOnly
      ? "Solo lectura: vuelve a la versión actual para importar material."
      : "Sin lote activo. Siguiente acción: seleccionar una o más fuentes.");
  if (!batch) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "Selecciona fuentes locales. HTML, macros, enlaces e instrucciones permanecen texto inerte.";
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

  for (const point of decisionPoints) {
    const card = document.createElement("article");
    card.className = "notice warning lore-decision";
    const heading = document.createElement("h4");
    heading.textContent = "Decisión necesaria";
    const prompt = document.createElement("p");
    prompt.textContent = point.prompt;
    const actions = document.createElement("div");
    actions.className = "pending-actions";
    for (const alternative of point.alternatives) {
      const action = document.createElement("button");
      action.type = "button";
      action.className = alternative === "reject" ? "ghost" : "secondary";
      action.textContent = decisionLabel(alternative);
      action.addEventListener("click", () => void resolveDecision(point, alternative));
      actions.append(action);
    }
    card.append(heading, prompt, actions);
    loreImportContent.append(card);
  }

  for (const item of candidates) {
    const card = document.createElement("article");
    card.className = "lore-candidate";
    card.dataset.candidateId = item.id;
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
    const authentication = item.candidate.kind === "claim"
      ? document.createElement("select")
      : null;
    if (authentication && item.candidate.kind === "claim") {
      for (const [value, label] of [["canonical", "Hecho canónico"], ["attributed", "Afirmación atribuida"]]) {
        const option = document.createElement("option");
        option.value = value!;
        option.textContent = label!;
        option.selected = item.candidate.authentication === value;
        authentication.append(option);
      }
      authentication.setAttribute("aria-label", "Autoridad de la afirmación candidata");
    }
    save.addEventListener("click", async () => {
      try {
        candidates = await invoke<ImportCandidateSnapshot[]>("edit_lore_candidate", {
          input: {
            batchId: batch!.id,
            candidateId: item.id,
            replacement: editedCandidate(item.candidate, editor.value, authentication?.value),
          },
        });
        decisionPoints = decisionPoints.filter((point) => point.candidateId !== item.id);
        statusMessage = "Candidato actualizado. Siguiente acción: volver a preparar la revisión.";
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
    actions.append(save);
    if (authentication) actions.append(authentication);
    actions.append(identity, select, reject);
    card.append(heading, confidence, editor, citations, actions);
    loreImportContent.append(card);
  }
}

function decisionLabel(alternative: string): string {
  if (alternative === "new") return "Crear identidad nueva";
  if (alternative === "mark_canonical") return "Tratar como hecho canónico";
  if (alternative === "reject") return "Rechazar candidato";
  const match = candidates
    .flatMap((candidate) => candidate.identityMatches)
    .find((identity) => identity.uri === alternative);
  return match ? `Enlazar con ${match.name}` : alternative;
}

async function resolveDecision(point: ImportDecisionPoint, alternative: string): Promise<void> {
  const item = candidates.find((candidate) => candidate.id === point.candidateId);
  if (!item || !batch) return;
  try {
    if (alternative === "reject") {
      await decide(item, "new", false);
      return;
    }
    if (alternative === "mark_canonical" && item.candidate.kind === "claim") {
      candidates = await invoke<ImportCandidateSnapshot[]>("edit_lore_candidate", {
        input: {
          batchId: batch.id,
          candidateId: item.id,
          replacement: { ...item.candidate, authentication: "canonical" },
        },
      });
      decisionPoints = decisionPoints.filter((candidate) => candidate.candidateId !== item.id);
      statusMessage = "Afirmación marcada como canónica. Siguiente acción: volver a preparar la revisión.";
      render();
      return;
    }
    await decide(item, alternative === "new" ? "new" : `exact:${alternative}`, true);
  } catch (value) {
    showError(value);
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
    decisionPoints = decisionPoints.filter((point) => point.candidateId !== item.id);
    statusMessage = "Decisión guardada. Siguiente acción: preparar la revisión cuando termines.";
    render();
  } catch (value) { showError(value); }
}

loreImportSelect.addEventListener("click", async () => {
  if (state.session?.read_only) {
    showError("Vuelve a la versión actual antes de importar material.");
    return;
  }
  try {
    clearError();
    const selected = await dialog.open({ multiple: true, directory: false, filters: loreFilter });
    if (selected === null) return;
    const sourceFiles = Array.isArray(selected) ? selected : [selected];
    batch = await invoke<ImportBatchSnapshot>("create_lore_import", { input: { sourceFiles } });
    setEphemeralWork("lore", "lote de importación activo", true);
    candidates = [];
    decisionPoints = [];
    batchReviewKey = null;
    statusMessage = `${sourceFiles.length} fuente${sourceFiles.length === 1 ? "" : "s"} copiada${sourceFiles.length === 1 ? "" : "s"} a staging inerte. Siguiente acción: extraer candidatos.`;
    render();
  } catch (value) { showError(value); }
});

loreImportExtract.addEventListener("click", async () => {
  if (!batch) return;
  setRunning(requestId(), "La IA está extrayendo candidatos de las fuentes importadas.");
  setStatus("Extrayendo candidatos citados…");
  try {
    await invoke("extract_lore_import", { input: { requestId: activeRequestId, batchId: batch.id } });
    candidates = await invoke<ImportCandidateSnapshot[]>("read_lore_candidates", { input: { batchId: batch.id } });
    statusMessage = "Extracción completa. Siguiente acción: editar, seleccionar o rechazar candidatos.";
    setStatus("Candidatos listos para editar, seleccionar o rechazar.");
  } catch (value) {
    statusMessage = "La extracción no terminó. Siguiente acción: revisar configuración y reintentar.";
    showError(value);
  }
  setRunning(null);
});

loreImportCancel.addEventListener("click", async () => {
  if (!activeRequestId) return;
  try { await invoke("cancel_ai_request", { requestId: activeRequestId }); } catch (value) { showError(value); }
});

loreImportDelete.addEventListener("click", async () => {
  if (!batch) return;
  try {
    await invoke("delete_lore_import", { input: { batchId: batch.id } });
    if (batchReviewKey) state.pendingDrafts.delete(batchReviewKey);
    batch = null;
    candidates = [];
    decisionPoints = [];
    batchReviewKey = null;
    statusMessage = null;
    setEphemeralWork("lore", "", false);
    render();
    renderPending();
    setStatus("Lote eliminado; canon y original permanecen intactos.");
  } catch (value) { showError(value); }
});

loreImportReview.addEventListener("click", async () => {
  if (!batch) return;
  setRunning(requestId(), "La IA está preparando la revisión del lote importado.");
  try {
    const prepared = await invoke<ImportReviewPreparation>("prepare_lore_import_review", {
      input: { requestId: activeRequestId, batchId: batch.id },
    });
    decisionPoints = prepared.decisionPoints;
    if (prepared.decisionPoints.length > 0) {
      statusMessage = `${prepared.decisionPoints.length} decisión pendiente. Elige una acción para continuar.`;
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
    batchReviewKey = review.reviewKey;
    statusMessage = "Revisión estándar preparada. Siguiente acción: revisar y aplicar o descartar.";
    renderPending();
    setStatus("Propuesta de importación abierta en revisión.");
  } catch (value) { showError(value); }
  finally { setRunning(null); }
});

window.addEventListener("nirmata:scope-changed", render);
window.addEventListener("nirmata:ai-provider-changed", render);
void listen<ProgressEvent>("lore-import-progress", ({ payload }) => {
  if (payload.requestId === activeRequestId) {
    statusMessage = `Extracción: ${humanize(payload.progress.kind)}…`;
    render();
  }
});
void listen<ProgressEvent>("ai-proposal-progress", ({ payload }) => {
  if (payload.requestId === activeRequestId) {
    statusMessage = `Preparando revisión: ${humanize(payload.progress.kind)}…`;
    render();
  }
});
window.addEventListener("nirmata:discard-ephemeral-work", () => {
  batch = null;
  candidates = [];
  decisionPoints = [];
  batchReviewKey = null;
  statusMessage = null;
  setEphemeralWork("lore", "", false);
  render();
});
window.addEventListener("nirmata:start-lore-import", () => {
  document.querySelector("#lore-import-panel")?.scrollIntoView({ block: "start" });
  loreImportSelect.focus();
  loreImportSelect.click();
});
render();
