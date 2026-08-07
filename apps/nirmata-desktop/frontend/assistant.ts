import { buildCreateEditor } from "./editor-create.js";
import { cloneEditorMode } from "./editor-model.js";
import { button, clearError, humanize, objectKindFromUri, setStatus, showError } from "./helpers.js";
import {
  assistantCancel,
  assistantContext,
  assistantCredential,
  assistantFinalCritique,
  assistantForm,
  assistantInput,
  assistantKey,
  assistantKeyClear,
  assistantKeyForm,
  assistantProgress,
  assistantProposeMode,
  assistantQueryMode,
  assistantSubmit,
  assistantTranscript,
  invoke,
  listen,
  state,
} from "./state.js";
import type {
  AiQueryResponse,
  AiRunSnapshot,
  ManualReviewSnapshot,
  ObjectKind,
  PendingDraftRecord,
  ProviderCredentialStatus,
  SearchObjectKind,
} from "./types.js";
import { renderWorkspace, selectUri } from "./workspace.js";

type AssistantMode = "query" | "propose";
type ProgressEvent = {
  requestId: string;
  progress: { kind: string; delta?: string };
};

let mode: AssistantMode = "query";
let activeRequestId: string | null = null;
let activeRun: AiRunSnapshot | null = null;
let credentialConfigured = false;
let streamedText = "";
let streamElement: HTMLElement | null = null;

function updateMode(next: AssistantMode): void {
  mode = next;
  assistantQueryMode.setAttribute("aria-pressed", String(mode === "query"));
  assistantProposeMode.setAttribute("aria-pressed", String(mode === "propose"));
  assistantQueryMode.className = mode === "query" ? "" : "secondary";
  assistantProposeMode.className = mode === "propose" ? "" : "secondary";
  assistantSubmit.textContent = mode === "query" ? "Consultar" : "Generar propuesta";
}

function updateContextLabel(): void {
  assistantContext.textContent = state.selectedUri
    ? `Contexto anclado en ${state.selectedUri}`
    : "Sin selección: se usará contexto general acotado.";
}

async function refreshCredential(): Promise<void> {
  const status = await invoke<ProviderCredentialStatus>("get_provider_credential_status");
  credentialConfigured = status.configured;
  assistantCredential.textContent = status.configured
    ? `Credencial configurada · ${humanize(status.persistence)}`
    : "Falta credencial: las acciones de IA están deshabilitadas.";
  assistantCredential.className = `credential-status ${status.configured ? "ready" : "warning"}`;
  assistantSubmit.disabled = !status.configured || activeRequestId !== null;
  assistantKeyClear.disabled = !status.configured;
  if (status.limitation) {
    assistantCredential.title = status.limitation;
  }
}

function setRunning(requestId: string | null): void {
  activeRequestId = requestId;
  assistantCancel.disabled = requestId === null;
  assistantSubmit.disabled = requestId !== null || !credentialConfigured;
  assistantInput.disabled = requestId !== null;
}

function renderQuery(response: AiQueryResponse): void {
  assistantTranscript.replaceChildren();
  for (const item of response.items) {
    const card = document.createElement("article");
    card.className = "assistant-message";
    const heading = document.createElement("div");
    heading.className = "badge-row";
    const classification = document.createElement("strong");
    classification.textContent = humanize(item.classification);
    heading.append(classification);
    const answer = document.createElement("p");
    answer.textContent = item.markdown;
    card.append(heading, answer);
    if (item.citations.length > 0) {
      const sources = document.createElement("div");
      sources.className = "assistant-sources";
      for (const citation of item.citations) {
        const source = button(citation.source.snippet || citation.source.uri, "ghost");
        source.title = citation.quoteMd;
        source.addEventListener("click", () => void selectUri(citation.source.uri));
        sources.append(source);
      }
      card.append(sources);
    }
    assistantTranscript.append(card);
  }
}

function fallbackEditor(targetUri: string) {
  const kind = objectKindFromUri(targetUri) as ObjectKind;
  if (state.editorMode) {
    return cloneEditorMode(state.editorMode);
  }
  return buildCreateEditor((kind === "world" ? "entity" : kind) as SearchObjectKind);
}

async function attachReview(run: AiRunSnapshot): Promise<void> {
  if (!run.reviewKey || !run.draft) {
    return;
  }
  const review = await invoke<ManualReviewSnapshot>("read_manual_review", {
    input: { reviewKey: run.reviewKey },
  });
  review.readyToConfirm = run.status === "ready_to_commit" && review.readyToConfirm;
  const targetUri = review.operations[0]?.targetUri ?? run.reviewKey;
  const kind = objectKindFromUri(targetUri) as ObjectKind;
  const record: PendingDraftRecord = {
    preview: {
      draftKey: run.reviewKey,
      targetUri,
      objectType: kind,
      mode: state.selectedUri === targetUri ? "update" : "create",
      title: "Propuesta de IA",
      objective: review.objective,
      sourceUris: review.sources,
      assumptions: review.assumptions,
      logicalPath: targetUri,
      validationReport: review.validationReport,
      readyToConfirm: review.readyToConfirm,
    },
    review,
    editor: fallbackEditor(targetUri),
    aiRunId: run.id,
  };
  state.pendingDrafts.set(run.reviewKey, record);
  state.panels.bottomCollapsed = false;
  renderWorkspace();
}

function renderRun(run: AiRunSnapshot): void {
  assistantTranscript.replaceChildren();
  const card = document.createElement("article");
  card.className = "assistant-message proposal";
  const title = document.createElement("h4");
  title.textContent = run.intentBrief?.objective ?? run.draft?.objective ?? "Ejecución de propuesta";
  const status = document.createElement("p");
  status.textContent = `Estado: ${humanize(run.status)} · reparaciones: ${run.repairCount}`;
  card.append(title, status);
  if (run.intentBrief) {
    const scope = document.createElement("p");
    scope.textContent = run.intentBrief.scope;
    assistantInput.value = run.intentBrief.objective;
    const useBrief = button("Continuar brief editado", "secondary");
    useBrief.addEventListener("click", () => void continueIntentBrief(run));
    card.append(scope, useBrief);
  }
  for (const issue of run.critiqueReport?.issues ?? []) {
    const warning = document.createElement("div");
    warning.className = `assistant-issue ${issue.severity}`;
    const summary = document.createElement("p");
    summary.textContent = `${humanize(issue.severity)}: ${issue.summary.markdown}`;
    warning.append(summary);
    for (const evidence of issue.evidence) {
      const source = button(evidence.sourceUri, "ghost");
      source.title = evidence.excerptMd;
      source.addEventListener("click", () => void selectUri(evidence.sourceUri));
      warning.append(source);
    }
    if (issue.severity === "conflict" && run.reviewKey) {
      const judge = button("Registrar decisión humana", "secondary");
      judge.addEventListener("click", async () => {
        const judgment = window.prompt("Explica por qué aceptas o corregirás este hallazgo:", "");
        if (!judgment?.trim()) {
          return;
        }
        activeRun = await invoke<AiRunSnapshot>("acknowledge_ai_critique", {
          input: { runId: run.id, issueId: issue.issueId, judgment: judgment.trim() },
        });
        renderRun(activeRun);
        await attachReview(activeRun);
      });
      warning.append(judge);
    }
    card.append(warning);
  }
  assistantTranscript.append(card);
  assistantFinalCritique.hidden = !run.reviewKey || run.status === "ready_to_commit" || run.status === "committed";
}

async function continueIntentBrief(run: AiRunSnapshot): Promise<void> {
  if (!run.intentBrief || !credentialConfigured) {
    return;
  }
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  try {
    activeRun = await invoke<AiRunSnapshot>("execute_ai_proposal_from_brief", {
      input: {
        requestId,
        userRequest: run.intentBrief.userRequest,
        objective: assistantInput.value.trim() || run.intentBrief.objective,
        scope: run.intentBrief.scope,
        entityUris: run.intentBrief.entities.map((entity) => entity.uri),
        restrictions: run.intentBrief.restrictions,
        reason: run.intentBrief.reason,
        anchorUri: state.selectedUri,
      },
    });
    renderRun(activeRun);
    await attachReview(activeRun);
  } catch (value) {
    showError(value);
  } finally {
    setRunning(null);
  }
}

async function submitAssistant(): Promise<void> {
  const request = assistantInput.value.trim();
  if (!request || !state.session || !credentialConfigured) {
    return;
  }
  if (mode === "propose" && activeRun?.reviewKey) {
    activeRun = await invoke<AiRunSnapshot>("read_ai_run", { runId: activeRun.id });
    if (activeRun.status !== "committed" && activeRun.status !== "cancelled" && activeRun.status !== "failed") {
      showError("Termina o descarta la propuesta activa antes de iniciar otra.");
      return;
    }
  }
  clearError();
  streamedText = "";
  streamElement = null;
  if (mode === "query") {
    assistantTranscript.replaceChildren();
    const streamCard = document.createElement("article");
    streamCard.className = "assistant-message";
    const label = document.createElement("strong");
    label.textContent = "Stream estructurado en curso";
    streamElement = document.createElement("pre");
    streamCard.append(label, streamElement);
    assistantTranscript.append(streamCard);
  }
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  assistantProgress.textContent = mode === "query" ? "Preparando consulta…" : "Preparando propuesta…";
  try {
    if (mode === "query") {
      const response = await invoke<AiQueryResponse>("execute_ai_query", {
        input: { requestId, request, anchorUri: state.selectedUri },
      });
      renderQuery(response);
    } else {
      activeRun = await invoke<AiRunSnapshot>("execute_ai_proposal", {
        input: { requestId, request, anchorUri: state.selectedUri },
      });
      renderRun(activeRun);
      await attachReview(activeRun);
    }
    assistantProgress.textContent = "Ejecución completada.";
  } catch (value) {
    showError(value);
    assistantProgress.textContent = "La ejecución terminó sin modificar el canon.";
  } finally {
    setRunning(null);
  }
}

assistantQueryMode.addEventListener("click", () => updateMode("query"));
assistantProposeMode.addEventListener("click", () => updateMode("propose"));
assistantForm.addEventListener("submit", (event) => {
  event.preventDefault();
  void submitAssistant();
});
assistantCancel.addEventListener("click", () => {
  if (activeRequestId) {
    void invoke("cancel_ai_request", { requestId: activeRequestId });
  }
});
assistantFinalCritique.addEventListener("click", async () => {
  if (!activeRun) {
    return;
  }
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  try {
    activeRun = await invoke<AiRunSnapshot>("revalidate_ai_run", {
      input: { requestId, runId: activeRun.id, anchorUri: state.selectedUri },
    });
    renderRun(activeRun);
    await attachReview(activeRun);
  } catch (value) {
    showError(value);
  } finally {
    setRunning(null);
  }
});
assistantKeyForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const apiKey = assistantKey.value.trim();
  if (!apiKey) {
    return;
  }
  await invoke("set_provider_api_key", { apiKey });
  assistantKey.value = "";
  await refreshCredential();
});
assistantKeyClear.addEventListener("click", async () => {
  await invoke("clear_provider_api_key");
  await refreshCredential();
});

void listen<ProgressEvent>("ai-query-progress", ({ payload }) => {
  if (payload.requestId !== activeRequestId) {
    return;
  }
  if (payload.progress.delta) {
    streamedText += payload.progress.delta;
    if (streamElement) {
      streamElement.textContent = streamedText;
    }
    assistantProgress.textContent = `Recibiendo respuesta… ${streamedText.length} caracteres`;
  } else {
    assistantProgress.textContent = humanize(payload.progress.kind);
  }
});
void listen<ProgressEvent>("ai-proposal-progress", ({ payload }) => {
  if (payload.requestId === activeRequestId) {
    assistantProgress.textContent = humanize(payload.progress.kind);
  }
});

window.setInterval(updateContextLabel, 400);
updateMode("query");
updateContextLabel();
void refreshCredential().catch(showError);
