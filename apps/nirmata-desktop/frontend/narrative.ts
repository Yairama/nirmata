import { attachAiReview } from "./render-pending.js";
import { button, formatEventTime, formatObjectRef, humanize, showError } from "./helpers.js";
import {
  invoke,
  listen,
  beginAiActivity,
  endAiActivity,
  narrativeCancel,
  narrativeCausal,
  narrativeDocumentForm,
  narrativeDocumentKind,
  narrativeDocumentRequest,
  narrativeDocumentTitle,
  narrativeLimit,
  narrativeLooseEnds,
  narrativeMaxDepth,
  narrativePerspective,
  narrativeResults,
  narrativeScope,
  narrativeStartEvents,
  narrativeStatus,
  narrativeTick,
  narrativeTimeline,
  setEphemeralWork,
  state,
} from "./state.js";
import type {
  InternalDocumentKind,
  NarrativeCausalThreads,
  NarrativeContinuityExploration,
  NarrativeContinuityProposal,
  NarrativeContinuitySelection,
  NarrativeLooseEnds,
  NarrativeTimeline,
  ObjectRef,
  ReadScope,
  WorldSession,
  AiRunSnapshot,
} from "./types.js";
import { selectUriInScope } from "./workspace.js";

type NarrativeSelectionInput =
  | { kind: "loose_end"; code: string; objectUri: string }
  | { kind: "causal_thread"; startEventId: string };

type ProgressEvent = {
  requestId: string;
  progress: { kind: string };
};

let activeRequestId: string | null = null;
let formDirty = false;

function syncEphemeralState(): void {
  setEphemeralWork(
    "narrative",
    "derivaciones o formulario narrativo",
    formDirty
      || state.narrative.timeline !== null
      || state.narrative.causalThreads !== null
      || state.narrative.looseEnds !== null
      || state.narrative.exploration !== null,
  );
}

function selectedScope(): ReadScope | null {
  const session = state.session;
  if (!session) return null;
  return narrativeScope.value === "active"
    ? { variantId: session.active_variant.id, revisionId: null }
    : { ...session.read_scope };
}

function renderScopeOptions(): void {
  const session = state.session;
  narrativeScope.replaceChildren();
  if (!session) return;
  const observed = document.createElement("option");
  observed.value = "observed";
  observed.textContent = session.read_only
    ? "Versión observada · solo lectura"
    : "Versión actual";
  const active = document.createElement("option");
  active.value = "active";
  active.textContent = `Versión actual de ${session.active_variant.name}`;
  narrativeScope.append(observed, active);
}

function setRunning(requestId: string | null): void {
  const previousRequestId = activeRequestId;
  activeRequestId = requestId;
  if (requestId) {
    syncAvailability();
    beginAiActivity({
      requestId,
      source: "narrative",
      label: "La IA está preparando un resultado narrativo revisable.",
    });
  } else if (previousRequestId) {
    endAiActivity(previousRequestId);
    syncAvailability();
  } else {
    syncAvailability();
  }
}

function syncAvailability(): void {
  const session = state.session;
  const busy = activeRequestId !== null;
  const unavailable = !session || busy;
  narrativeTimeline.disabled = unavailable;
  narrativeCausal.disabled = unavailable;
  narrativeLooseEnds.disabled = unavailable;
  narrativeScope.disabled = unavailable;
  narrativeCancel.disabled = !busy;
  narrativeDocumentForm
    .querySelectorAll<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement | HTMLButtonElement>(
      "input, textarea, select, button",
    )
    .forEach((control) => {
      control.disabled = unavailable || Boolean(session?.read_only) || !state.aiProviderReady;
    });
  if (session?.read_only) {
    narrativeStatus.textContent = "Solo lectura: derivaciones habilitadas; documento y propuestas bloqueados.";
  } else if (!session) {
    narrativeStatus.textContent = "Abre un mundo para derivar narrativa.";
  }
}

function section(title: string): HTMLElement {
  const card = document.createElement("section");
  card.className = "narrative-result";
  const heading = document.createElement("h4");
  heading.textContent = title;
  card.append(heading);
  return card;
}

async function openScopedUri(uri: string, scope: ReadScope): Promise<void> {
  await selectUriInScope(uri, scope);
}

function uriButton(uri: string, scope: ReadScope, label = uri): HTMLButtonElement {
  const open = button(label, "ghost");
  open.addEventListener("click", () => void openScopedUri(uri, scope).catch(showError));
  return open;
}

function appendEvidence(parent: HTMLElement, uris: string[], scope: ReadScope): void {
  if (uris.length === 0) return;
  const sources = document.createElement("div");
  sources.className = "narrative-sources";
  for (const uri of uris) sources.append(uriButton(uri, scope));
  parent.append(sources);
}

function renderTimelineResult(value: NarrativeTimeline): HTMLElement {
  const card = section("Orden cronológico y orden en que se cuenta");
  const columns = document.createElement("div");
  columns.className = "narrative-columns";
  const story = section(`Orden cronológico · ${value.storyTime.length}`);
  for (const event of value.storyTime) {
    const item = document.createElement("article");
    item.className = "narrative-item";
    const summary = document.createElement("p");
    summary.textContent = `${formatEventTime(event.time)} · ${event.summary}`;
    item.append(summary, uriButton(event.event.uri, value.scope, "Abrir evento"));
    appendEvidence(item, event.evidenceUris, value.scope);
    story.append(item);
  }
  const unknown = section(`Tiempo desconocido · ${value.unknownStoryTime.length}`);
  for (const event of value.unknownStoryTime) {
    const item = document.createElement("article");
    item.className = "narrative-item";
    const summary = document.createElement("p");
    summary.textContent = event.summary;
    item.append(summary, uriButton(event.event.uri, value.scope, "Abrir evento"));
    appendEvidence(item, event.evidenceUris, value.scope);
    unknown.append(item);
  }
  const discourse = section(`Orden en que se cuenta · ${value.discourseOrder.length} fuentes`);
  for (const sequence of value.discourseOrder) {
    const item = document.createElement("article");
    item.className = "narrative-item";
    item.append(uriButton(sequence.source.uri, value.scope, `Fuente ${sequence.source.uri}`));
    for (const event of sequence.events) {
      const row = document.createElement("p");
      row.textContent = `#${event.ordinal} · ${event.event.uri}`;
      item.append(row, uriButton(event.event.uri, value.scope, "Abrir evento"));
      appendEvidence(item, event.evidenceUris, value.scope);
    }
    discourse.append(item);
  }
  columns.append(story, unknown, discourse);
  card.append(columns);
  return card;
}

function causalSelection(startUri: string): NarrativeContinuitySelection {
  return { kind: "causal_thread", startEventId: startUri.split("/").at(-1) ?? "" };
}

function renderCausalResult(value: NarrativeCausalThreads): HTMLElement {
  const card = section(`Hilos causales · profundidad ${value.maxDepth} · límite ${value.limit}`);
  for (const thread of value.threads) {
    const item = document.createElement("article");
    item.className = "narrative-item";
    const heading = document.createElement("h5");
    heading.textContent = `Inicio ${thread.start.uri}`;
    item.append(heading, uriButton(thread.start.uri, value.scope, "Abrir inicio"));
    for (const link of thread.links) {
      const row = document.createElement("p");
      row.textContent = `Nivel ${link.depth} · ${humanize(link.kind)} · ${link.source.uri} → ${link.target.uri}`;
      item.append(row);
      appendEvidence(item, link.evidenceUris, value.scope);
    }
    const explore = button("Explorar continuidad", "secondary");
    explore.addEventListener("click", () => void exploreContinuity(value.scope, causalSelection(thread.start.uri)));
    item.append(explore);
    card.append(item);
  }
  return card;
}

function renderLooseEndsResult(value: NarrativeLooseEnds): HTMLElement {
  const card = section(`Cabos abiertos · ${value.findings.length}`);
  for (const finding of value.findings) {
    const item = document.createElement("article");
    item.className = "narrative-item";
    const heading = document.createElement("h5");
    heading.textContent = finding.code;
    const message = document.createElement("p");
    message.textContent = finding.message;
    item.append(heading, message);
    for (const reference of finding.objectRefs) {
      const resolved = formatObjectRef(reference);
      item.append(uriButton(resolved.uri, value.scope, resolved.uri));
    }
    appendEvidence(item, finding.evidenceUris, value.scope);
    const first = finding.objectRefs[0];
    if (first) {
      const explore = button("Explorar continuidad", "secondary");
      explore.addEventListener("click", () => void exploreContinuity(value.scope, {
        kind: "loose_end",
        code: finding.code,
        objectRef: first,
      }));
      item.append(explore);
    }
    card.append(item);
  }
  return card;
}

function selectionInput(selection: NarrativeContinuitySelection): NarrativeSelectionInput {
  return selection.kind === "loose_end"
    ? { kind: "loose_end", code: selection.code, objectUri: formatObjectRef(selection.objectRef).uri }
    : { kind: "causal_thread", startEventId: selection.startEventId };
}

function renderExploration(value: NarrativeContinuityExploration): HTMLElement {
  const card = section("Continuidad: alternativas antes de proponer");
  const question = document.createElement("p");
  question.textContent = value.question;
  card.append(question);
  appendEvidence(card, value.sourceUris, value.scope);
  for (const alternative of value.alternatives) {
    const item = document.createElement("article");
    item.className = "narrative-alternative";
    const title = document.createElement("h5");
    title.textContent = alternative.title;
    const consequence = document.createElement("p");
    consequence.textContent = alternative.consequence;
    const choose = button("Elegir y preparar propuesta", "secondary");
    choose.disabled = Boolean(state.session?.read_only)
      || activeRequestId !== null
      || !state.aiProviderReady;
    choose.addEventListener("click", () => void proposeContinuity(value, alternative.id));
    item.append(title, consequence, choose);
    card.append(item);
  }
  return card;
}

function renderResults(): void {
  const values: HTMLElement[] = [];
  if (state.narrative.timeline) values.push(renderTimelineResult(state.narrative.timeline));
  if (state.narrative.causalThreads) values.push(renderCausalResult(state.narrative.causalThreads));
  if (state.narrative.looseEnds) values.push(renderLooseEndsResult(state.narrative.looseEnds));
  if (state.narrative.exploration) values.push(renderExploration(state.narrative.exploration));
  if (values.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "Las derivaciones no escriben canon. Documento y continuidad siempre entran en cambios pendientes.";
    narrativeResults.replaceChildren(empty);
  } else {
    narrativeResults.replaceChildren(...values);
  }
  syncEphemeralState();
}

function integer(input: HTMLInputElement, label: string, min: number, max: number): number {
  const value = Number(input.value);
  if (!Number.isSafeInteger(value) || value < min || value > max) {
    throw new Error(`${label} debe ser un entero entre ${min} y ${max}.`);
  }
  return value;
}

function eventIds(): string[] | null {
  const values = narrativeStartEvents.value.split(/\r?\n/u).map((value) => value.trim()).filter(Boolean);
  return values.length > 0 ? values : null;
}

async function deriveTimeline(): Promise<void> {
  const scope = selectedScope();
  if (!scope) return;
  try {
    state.narrative.timeline = await invoke<NarrativeTimeline>("derive_narrative_timeline", {
      input: { scope },
    });
    narrativeStatus.textContent = "Orden cronológico y relato derivados sin escritura.";
    renderResults();
  } catch (value) { showError(value); }
}

async function deriveCausal(): Promise<void> {
  const scope = selectedScope();
  if (!scope) return;
  try {
    state.narrative.causalThreads = await invoke<NarrativeCausalThreads>("derive_causal_threads", {
      input: {
        scope,
        startEventIds: eventIds(),
        maxDepth: integer(narrativeMaxDepth, "Profundidad", 0, 3),
        limit: integer(narrativeLimit, "Límite", 0, 100),
      },
    });
    narrativeStatus.textContent = "Hilos causales derivados con evidencia navegable.";
    renderResults();
  } catch (value) { showError(value); }
}

async function deriveLooseEnds(): Promise<void> {
  const scope = selectedScope();
  if (!scope) return;
  try {
    state.narrative.looseEnds = await invoke<NarrativeLooseEnds>("derive_loose_ends", {
      input: { scope },
    });
    narrativeStatus.textContent = "Cabos abiertos derivados con su regla heurística y fuentes visibles.";
    renderResults();
  } catch (value) { showError(value); }
}

async function exploreContinuity(scope: ReadScope, selection: NarrativeContinuitySelection): Promise<void> {
  try {
    state.narrative.exploration = await invoke<NarrativeContinuityExploration>(
      "explore_narrative_continuity",
      { input: { scope, selection: selectionInput(selection) } },
    );
    narrativeStatus.textContent = "Alternativas de continuidad listas; todavía no se llamó a IA.";
    renderResults();
  } catch (value) { showError(value); }
}

async function proposeContinuity(
  exploration: NarrativeContinuityExploration,
  alternativeId: string,
): Promise<void> {
  if (state.session?.read_only) {
    showError("Vuelve a la versión actual antes de preparar una propuesta de continuidad.");
    return;
  }
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  narrativeStatus.textContent = "Preparando propuesta estándar de continuidad…";
  try {
    const proposal = await invoke<NarrativeContinuityProposal>("propose_narrative_continuity", {
      input: {
        requestId,
        scope: exploration.scope,
        selection: selectionInput(exploration.selection),
        alternativeId,
      },
    });
    await attachAiReview(proposal.run, "Continuidad narrativa");
    narrativeStatus.textContent = "Propuesta de continuidad añadida a cambios pendientes.";
  } catch (value) { showError(value); }
  finally { setRunning(null); renderResults(); }
}

async function generateDocument(): Promise<void> {
  const session = state.session;
  if (!session || session.read_only) {
    showError("Vuelve a la versión actual antes de generar un documento interno.");
    return;
  }
  const tick = integer(narrativeTick, "Tick", Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER);
  const requestId = crypto.randomUUID();
  setRunning(requestId);
  narrativeStatus.textContent = "Generando documento y crítica estándar…";
  try {
    const run = await invoke<AiRunSnapshot>("generate_internal_document", {
      input: {
        requestId,
        documentKind: narrativeDocumentKind.value as InternalDocumentKind,
        title: narrativeDocumentTitle.value,
        request: narrativeDocumentRequest.value,
        perspectiveEntityId: narrativePerspective.value,
        tick,
        anchorUris: state.selectedUri ? [state.selectedUri] : [],
      },
    });
    await attachAiReview(run, `Documento interno · ${narrativeDocumentTitle.value.trim()}`);
    narrativeStatus.textContent = "Documento añadido como propuesta pendiente de revisión.";
  } catch (value) { showError(value); }
  finally { setRunning(null); }
}

narrativeTimeline.addEventListener("click", () => void deriveTimeline());
narrativeCausal.addEventListener("click", () => void deriveCausal());
narrativeLooseEnds.addEventListener("click", () => void deriveLooseEnds());
narrativeDocumentForm.addEventListener("submit", (event) => {
  event.preventDefault();
  void generateDocument();
});
narrativeDocumentForm.addEventListener("input", () => {
  formDirty = true;
  syncEphemeralState();
});
narrativeStartEvents.addEventListener("input", () => {
  formDirty = true;
  syncEphemeralState();
});
narrativeCancel.addEventListener("click", () => {
  if (activeRequestId) void invoke("cancel_ai_request", { requestId: activeRequestId });
});
void listen<ProgressEvent>("ai-proposal-progress", ({ payload }) => {
  if (payload.requestId === activeRequestId) {
    narrativeStatus.textContent = humanize(payload.progress.kind);
  }
});
window.addEventListener("nirmata:scope-changed", () => {
  renderScopeOptions();
  syncAvailability();
  renderResults();
});
window.addEventListener("nirmata:ai-provider-changed", () => {
  syncAvailability();
  renderResults();
});
window.addEventListener("nirmata:discard-ephemeral-work", () => {
  formDirty = false;
  state.narrative.timeline = null;
  state.narrative.causalThreads = null;
  state.narrative.looseEnds = null;
  state.narrative.exploration = null;
  narrativeDocumentForm.reset();
  narrativeStartEvents.value = "";
  renderResults();
});
renderScopeOptions();
syncAvailability();
renderResults();
