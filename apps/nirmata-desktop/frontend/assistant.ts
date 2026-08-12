import { button, clearError, humanize, setStatus, showError } from "./helpers.js";
import { attachAiReview } from "./render-pending.js";
import {
  assistantCancel,
  assistantContext,
  assistantCredential,
  assistantDeepMode,
  assistantAuditMode,
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
  DeepReviewPlan,
  DeepReviewRun,
  ProviderCredentialStatus,
  SpecialistRole,
  WorldSession,
} from "./types.js";
import { renderWorkspace, selectUri } from "./workspace.js";

type AssistantMode = "query" | "propose" | "deep_impact" | "audit";
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

function isWriteMode(value: AssistantMode): boolean {
  return value === "propose" || value === "deep_impact";
}

function updateAvailability(): void {
  const readOnly = state.session?.read_only ?? false;
  assistantProposeMode.disabled = readOnly || activeRequestId !== null;
  assistantDeepMode.disabled = readOnly || activeRequestId !== null;
  assistantQueryMode.disabled = activeRequestId !== null;
  assistantAuditMode.disabled = activeRequestId !== null;
  assistantSubmit.disabled = activeRequestId !== null
    || !credentialConfigured
    || (readOnly && isWriteMode(mode));
  assistantFinalCritique.disabled = readOnly || activeRequestId !== null || !activeRun;
}

function updateMode(next: AssistantMode): void {
  if (state.session?.read_only && isWriteMode(next)) {
    next = "query";
  }
  mode = next;
  assistantQueryMode.setAttribute("aria-pressed", String(mode === "query"));
  assistantProposeMode.setAttribute("aria-pressed", String(mode === "propose"));
  assistantDeepMode.setAttribute("aria-pressed", String(mode === "deep_impact"));
  assistantAuditMode.setAttribute("aria-pressed", String(mode === "audit"));
  assistantQueryMode.className = mode === "query" ? "" : "secondary";
  assistantProposeMode.className = mode === "propose" ? "" : "secondary";
  assistantDeepMode.className = mode === "deep_impact" ? "" : "secondary";
  assistantAuditMode.className = mode === "audit" ? "" : "secondary";
  assistantSubmit.textContent = mode === "query"
    ? "Consultar"
    : mode === "propose"
      ? "Generar propuesta"
      : "Preparar roles";
  updateAvailability();
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
  assistantKeyClear.disabled = !status.configured;
  if (status.limitation) {
    assistantCredential.title = status.limitation;
  }
  updateAvailability();
}

function setRunning(requestId: string | null): void {
  activeRequestId = requestId;
  assistantCancel.disabled = requestId === null;
  assistantInput.disabled = requestId !== null;
  updateAvailability();
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
        source.addEventListener("click", () => void (async () => {
          const session = await invoke<WorldSession>("set_read_scope", {
            input: { scope: response.snapshot.readScope },
          });
          state.session = session;
          renderWorkspace();
          await selectUri(citation.source.uri);
        })().catch(showError));
        sources.append(source);
      }
      card.append(sources);
    }
    assistantTranscript.append(card);
  }
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
        await attachAiReview(activeRun);
      });
      warning.append(judge);
    }
    card.append(warning);
  }
  assistantTranscript.append(card);
  assistantFinalCritique.hidden = !run.reviewKey || run.status === "ready_to_commit" || run.status === "committed";
}

function renderDeepPlan(plan: DeepReviewPlan): void {
  assistantTranscript.replaceChildren();
  const card = document.createElement("article");
  card.className = "assistant-message proposal deep-review";
  const title = document.createElement("h4");
  title.textContent = plan.mode === "audit" ? "Confirmar auditoría profunda" : "Confirmar revisión profunda";
  const reason = document.createElement("p");
  reason.textContent = plan.reason;
  const budget = document.createElement("p");
  budget.className = "muted";
  budget.textContent = `${plan.budget.maxSpecialistCalls} llamadas especialistas máx. · ${plan.budget.specialistMaxOutputTokens} tokens/informe · ${plan.budget.maxReadToolCalls} tools de lectura · ${plan.budget.maxNestedDelegations} delegaciones · ${Math.round(plan.budget.specialistTimeoutMs / 1000)} s/rol`;
  const roles = document.createElement("fieldset");
  const legend = document.createElement("legend");
  legend.textContent = "Roles confirmados por el usuario";
  roles.append(legend);
  for (const role of plan.roles) {
    const label = document.createElement("label");
    label.className = "deep-role";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.value = role;
    checkbox.checked = true;
    label.append(checkbox, document.createTextNode(humanize(role)));
    roles.append(label);
  }
  const confirm = button("Confirmar roles e iniciar", "secondary");
  confirm.addEventListener("click", () => {
    const selected = Array.from(roles.querySelectorAll<HTMLInputElement>('input[type="checkbox"]:checked'))
      .map((input) => input.value as SpecialistRole);
    if (selected.length === 0 || selected.length > plan.budget.maxSpecialists) {
      showError(`Selecciona entre 1 y ${plan.budget.maxSpecialists} roles.`);
      return;
    }
    void executeDeepReview(plan, selected);
  });
  card.append(title, reason, budget, roles, confirm);
  assistantTranscript.append(card);
}

function renderDeepRun(run: DeepReviewRun): void {
  assistantTranscript.replaceChildren();
  const summary = document.createElement("article");
  summary.className = "assistant-message proposal deep-review";
  const title = document.createElement("h4");
  title.textContent = run.mode === "audit" ? "Auditoría profunda" : "Revisión profunda";
  const status = document.createElement("p");
  status.textContent = `Estado: ${humanize(run.status)} · revisión base ${run.baseRevision}`;
  summary.append(title, status);
  if (run.error) {
    const error = document.createElement("p");
    error.className = "assistant-issue conflict";
    error.textContent = run.error;
    summary.append(error);
  }
  assistantTranscript.append(summary);

  const positions = new Map<string, Set<string>>();
  for (const specialist of run.specialists) {
    const card = document.createElement("article");
    card.className = "assistant-message specialist-report";
    const heading = document.createElement("h4");
    heading.textContent = `${humanize(specialist.role)} · ${humanize(specialist.status)}`;
    card.append(heading);
    if (specialist.error) {
      const failure = document.createElement("p");
      failure.className = "assistant-issue warning";
      failure.textContent = specialist.error;
      card.append(failure);
    }
    for (const finding of specialist.report?.findings ?? []) {
      const text = document.createElement("p");
      text.textContent = finding.summary.markdown;
      card.append(text);
      for (const evidence of finding.evidence) {
        const source = button(evidence.sourceUri, "ghost");
        source.title = evidence.excerptMd;
        source.addEventListener("click", () => void selectUri(evidence.sourceUri));
        card.append(source);
      }
      if (finding.decisionPosition) {
        const alternatives = positions.get(finding.decisionPosition.decisionKey) ?? new Set<string>();
        alternatives.add(finding.decisionPosition.alternative);
        positions.set(finding.decisionPosition.decisionKey, alternatives);
      }
    }
    assistantTranscript.append(card);
  }
  for (const [key, alternatives] of positions) {
    if (alternatives.size < 2) continue;
    const disagreement = document.createElement("article");
    disagreement.className = "assistant-message assistant-issue conflict";
    const heading = document.createElement("h4");
    heading.textContent = `Desacuerdo: ${humanize(key)}`;
    const detail = document.createElement("p");
    detail.textContent = Array.from(alternatives).join(" / ");
    disagreement.append(heading, detail);
    assistantTranscript.append(disagreement);
  }
  if (run.synthesis?.draft) {
    const synthesis = document.createElement("article");
    synthesis.className = "assistant-message proposal";
    const heading = document.createElement("h4");
    heading.textContent = "Síntesis completa entregada a revisión estándar";
    const detail = document.createElement("p");
    detail.textContent = `${run.synthesis.draft.operations.length} operaciones · ${run.synthesis.draft.decisions.length} decisiones pendientes`;
    synthesis.append(heading, detail);
    assistantTranscript.append(synthesis);
  }
}

async function executeDeepReview(plan: DeepReviewPlan, roles: SpecialistRole[]): Promise<void> {
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  assistantProgress.textContent = "Iniciando especialistas confirmados…";
  try {
    const run = await invoke<DeepReviewRun>("execute_deep_review", {
      input: {
        requestId,
        mode: plan.mode,
        request: plan.request,
        roles,
        anchorUri: state.selectedUri,
      },
    });
    renderDeepRun(run);
    if (run.standardRunId && run.status === "awaiting_review") {
      activeRun = await invoke<AiRunSnapshot>("read_ai_run", { runId: run.standardRunId });
      await attachAiReview(activeRun);
      assistantFinalCritique.hidden = !activeRun.reviewKey;
    } else {
      activeRun = null;
      assistantFinalCritique.hidden = true;
    }
  } catch (value) {
    showError(value);
  } finally {
    setRunning(null);
  }
}

async function continueIntentBrief(run: AiRunSnapshot): Promise<void> {
  if (state.session?.read_only) {
    showError("Vuelve a la cabeza activa antes de continuar una propuesta.");
    return;
  }
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
    await attachAiReview(activeRun);
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
  if (state.session.read_only && isWriteMode(mode)) {
    showError("Las propuestas solo pueden iniciarse desde la cabeza activa.");
    updateMode("query");
    return;
  }
  if (mode !== "query" && activeRun?.reviewKey) {
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
    } else if (mode === "propose") {
      activeRun = await invoke<AiRunSnapshot>("execute_ai_proposal", {
        input: { requestId, request, anchorUri: state.selectedUri },
      });
      renderRun(activeRun);
      await attachAiReview(activeRun);
    } else {
      const plan = await invoke<DeepReviewPlan>("prepare_deep_review", {
        input: { mode, request, anchorUri: state.selectedUri },
      });
      renderDeepPlan(plan);
    }
    assistantProgress.textContent = "Ejecución completada.";
  } catch (value) {
    showError(value);
    assistantProgress.textContent = "La ejecución terminó sin modificar el canon. Puedes reintentar.";
  } finally {
    setRunning(null);
  }
}

assistantQueryMode.addEventListener("click", () => updateMode("query"));
assistantProposeMode.addEventListener("click", () => updateMode("propose"));
assistantDeepMode.addEventListener("click", () => updateMode("deep_impact"));
assistantAuditMode.addEventListener("click", () => updateMode("audit"));
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
  if (!activeRun || state.session?.read_only) {
    return;
  }
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  try {
    activeRun = await invoke<AiRunSnapshot>("revalidate_ai_run", {
      input: { requestId, runId: activeRun.id, anchorUri: state.selectedUri },
    });
    renderRun(activeRun);
    await attachAiReview(activeRun);
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
void listen<ProgressEvent>("deep-review-progress", ({ payload }) => {
  if (payload.requestId === activeRequestId) {
    const role = (payload.progress as { role?: string }).role;
    assistantProgress.textContent = role
      ? `${humanize(payload.progress.kind)} · ${humanize(role)}`
      : humanize(payload.progress.kind);
  }
});

window.setInterval(() => {
  updateContextLabel();
  if (state.session?.read_only && isWriteMode(mode)) {
    updateMode("query");
  } else {
    updateAvailability();
  }
}, 400);
window.addEventListener("nirmata:ai-review-attached", (event) => {
  activeRun = (event as CustomEvent<AiRunSnapshot>).detail;
  renderRun(activeRun);
  updateAvailability();
});
updateMode("query");
updateContextLabel();
void refreshCredential().catch(showError);
