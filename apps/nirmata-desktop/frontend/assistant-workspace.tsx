import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useReducer, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { clearError, humanize, labelForUri, showError } from "./helpers.js";
import { showAiCommandError } from "./feedback.js";
import type { AiFailureObservation } from "./feedback.js";
import { useObjectPicker } from "./object-picker.js";
import { pendingReviewsQueryKey } from "./pending-reviews.js";
import { useSession } from "./session-provider.js";
import {
  beginAiActivity,
  endAiActivity,
  listen,
  useAppState,
} from "./state.js";
import type {
  AiConversationHistoryTurn,
  AiProviderDiagnosticStatus,
  AiQueryResponse,
  AiRunSnapshot,
  AssistantConversation,
  AssistantConversationTurn,
  DeepReviewPlan,
  DeepReviewRun,
  ReadScope,
  SearchResult,
  SpecialistRole,
  WorldSession,
} from "./types.js";
import { selectUri, selectUriInScope } from "./workspace.js";
import { Icon } from "./icons.js";

export type AssistantMode = "query" | "propose" | "deep_impact" | "audit";
export type ProposalTemplate = "faction" | "city" | "character" | "conflict" | "chronology" | "consequences";
type ProposalScale = "small" | "medium";
export type AssistantIntent = {
  id: number;
  mode: "query" | "propose";
  request?: string;
  template?: ProposalTemplate;
};

type ProgressEvent = {
  requestId: string;
  progress: { kind: string; delta?: string; role?: string };
};
type ProposalOrigin = AssistantConversationTurn["origin"];
type DeepPlanState = {
  plan: DeepReviewPlan;
  origin: ProposalOrigin;
};
type WorkspaceState = {
  mode: AssistantMode;
  request: string;
  activeRequestId: string | null;
  activeRun: AiRunSnapshot | null;
  activeDeepRun: DeepReviewRun | null;
  deepPlan: DeepPlanState | null;
  activeRunOrigin: ProposalOrigin | null;
  proposalOrigin: ProposalOrigin | null;
  progress: string;
  streamedText: string;
  conversationWorldId: string | null;
  conversations: AssistantConversation[];
  activeConversationId: string | null;
  deleteArmed: boolean;
  confirmingTurnId: string | null;
  view: "conversation" | "run" | "deep_plan" | "deep_run";
};
type WorkspaceAction = { type: "patch"; value: Partial<WorkspaceState> } | { type: "world"; value: WorkspaceState };

const providerStatusKey = ["desktop", "provider-status"] as const;
const maxConversations = 20;
const maxStoredTurns = 50;
const templates: Array<{ id: ProposalTemplate; label: string; detail: string }> = [
  { id: "faction", label: "Facción", detail: "Poder, propósito y vínculos inmediatos." },
  { id: "city", label: "Ciudad", detail: "Función, tensiones y actores relacionados." },
  { id: "character", label: "Personaje", detail: "Motivación, posición y relaciones." },
  { id: "conflict", label: "Conflicto", detail: "Actores, causa y presión concreta." },
  { id: "chronology", label: "Cronología", detail: "Secuencia breve de acontecimientos." },
  { id: "consequences", label: "Consecuencias", detail: "Efectos directos de la selección." },
];

function initialState(): WorkspaceState {
  return {
    mode: "query",
    request: "",
    activeRequestId: null,
    activeRun: null,
    activeDeepRun: null,
    deepPlan: null,
    activeRunOrigin: null,
    proposalOrigin: null,
    progress: "",
    streamedText: "",
    conversationWorldId: null,
    conversations: [],
    activeConversationId: null,
    deleteArmed: false,
    confirmingTurnId: null,
    view: "conversation",
  };
}

function reducer(current: WorkspaceState, action: WorkspaceAction): WorkspaceState {
  return action.type === "world" ? action.value : { ...current, ...action.value };
}

function conversationStorageKey(worldId: string): string {
  return `nirmata.assistant.conversations.${worldId}`;
}

function createConversation(worldId: string): AssistantConversation {
  const now = Date.now();
  return {
    id: crypto.randomUUID(),
    worldId,
    title: "Nueva conversación",
    createdAtMs: now,
    updatedAtMs: now,
    turns: [],
  };
}

function readConversations(worldId: string): AssistantConversation[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(conversationStorageKey(worldId)) ?? "[]") as unknown;
    if (!Array.isArray(parsed)) return [createConversation(worldId)];
    const valid = parsed.filter((value): value is AssistantConversation => {
      if (!value || typeof value !== "object") return false;
      const candidate = value as Partial<AssistantConversation>;
      return typeof candidate.id === "string"
        && candidate.worldId === worldId
        && typeof candidate.title === "string"
        && Array.isArray(candidate.turns)
        && candidate.turns.every((turn) => {
          const stored = turn as Partial<AssistantConversationTurn>;
          return typeof stored.id === "string"
            && typeof stored.request === "string"
            && typeof stored.createdAtMs === "number"
            && Boolean(stored.response
              && Array.isArray(stored.response.items)
              && stored.response.items.every((item) => typeof item.itemId === "string"
                && typeof item.classification === "string"
                && typeof item.markdown === "string"
                && Array.isArray(item.citations)
                && item.citations.every((citation) => typeof citation.quoteMd === "string"
                  && typeof citation.source?.uri === "string"
                  && typeof citation.source?.snippet === "string")))
            && stored.origin?.worldId === worldId
            && typeof stored.origin.baseRevision === "string"
            && typeof stored.origin.contextLabel === "string"
            && typeof stored.origin.readScope?.variantId === "string";
        });
    }).slice(0, maxConversations).map((conversation) => ({
      ...conversation,
      turns: conversation.turns.slice(-maxStoredTurns),
    }));
    return valid.length > 0 ? valid : [createConversation(worldId)];
  } catch {
    return [createConversation(worldId)];
  }
}

function conversationHistory(conversation: AssistantConversation): AiConversationHistoryTurn[] {
  return conversation.turns.slice(-8).map((turn) => ({
    userRequest: turn.request,
    assistantResponse: turn.response.items.map((item) => item.markdown).join("\n\n").slice(0, 8_000),
    sourceUris: Array.from(new Set(turn.response.items.flatMap((item) => item.citations.map((citation) => citation.source.uri)))).slice(0, 24),
  }));
}

function isWriteMode(mode: AssistantMode): boolean {
  return mode === "propose" || mode === "deep_impact";
}

function specialistRoleLabel(role: SpecialistRole): string {
  return ({
    economist: "Economía y recursos",
    historian: "Historia y precedentes",
    political_scientist: "Poder e instituciones",
    anthropologist: "Cultura y sociedad",
    theologian: "Religión y cosmología",
    geographer: "Geografía y territorio",
    temporal_auditor: "Continuidad temporal",
    rules_auditor: "Reglas del mundo",
    causal_auditor: "Causas y consecuencias",
    perspectives_auditor: "Perspectivas y evidencia",
  } as Record<SpecialistRole, string>)[role];
}

function originIsCurrent(session: WorldSession | null, origin: ProposalOrigin): boolean {
  return Boolean(session
    && session.world_id === origin.worldId
    && session.current_revision === origin.baseRevision
    && session.read_scope.variantId === origin.readScope.variantId
    && session.read_scope.revisionId === origin.readScope.revisionId);
}

function currentOrigin(session: WorldSession, selectedUri: string | null): ProposalOrigin {
  return {
    worldId: session.world_id,
    baseRevision: session.current_revision,
    readScope: session.read_scope,
    anchorUri: selectedUri,
    contextLabel: selectedUri ? labelForUri(selectedUri) : "contexto general",
    sourceCount: 0,
  };
}

export function AssistantWorkspace({ active, intent, onClose, onOpenSettings, onOpenReviews }: {
  active: boolean;
  intent: AssistantIntent | null;
  onClose: () => void;
  onOpenSettings: () => void;
  onOpenReviews: () => void;
}) {
  const session = useSession();
  const selectedUri = useAppState().selectedUri;
  const queryClient = useQueryClient();
  const [workspace, dispatch] = useReducer(reducer, undefined, initialState);
  const workspaceRef = useRef(workspace);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const transcriptRef = useRef<HTMLDivElement>(null);
  const aiObservation = useRef<AiFailureObservation>({ startedAtMs: Date.now(), phase: "preparing", receivedCharacters: 0 });
  const handledIntent = useRef<number | null>(null);
  const [templateScale, setTemplateScale] = useState<ProposalScale>("small");
  const [templatesOpen, setTemplatesOpen] = useState(false);
  workspaceRef.current = workspace;

  useEffect(() => {
    if (!active) return;
    const root = document.getElementById("root");
    if (!root) return;
    root.inert = true;
    return () => { root.inert = false; };
  }, [active]);

  const provider = useQuery({
    queryKey: providerStatusKey,
    queryFn: () => invoke<AiProviderDiagnosticStatus>("get_ai_provider_status"),
    enabled: Boolean(session),
    retry: false,
  });
  const providerReady = provider.data?.canCheckConnection === true;
  const activeConversation = workspace.conversations.find((item) => item.id === workspace.activeConversationId) ?? null;
  const running = workspace.activeRequestId !== null;
  const writeBlocked = Boolean(session?.read_only && isWriteMode(workspace.mode));

  useEffect(() => {
    if (!session) {
      dispatch({ type: "world", value: initialState() });
      return;
    }
    const conversations = readConversations(session.world_id);
    dispatch({
      type: "world",
      value: {
        ...initialState(),
        conversationWorldId: session.world_id,
        conversations,
        activeConversationId: conversations[0].id,
      },
    });
  }, [session?.world_id]);

  useEffect(() => {
    if (session?.read_only && isWriteMode(workspace.mode)) {
      dispatch({ type: "patch", value: { mode: "query", proposalOrigin: null, view: "conversation" } });
    }
  }, [session?.read_only, workspace.mode]);

  useEffect(() => {
    if (active) inputRef.current?.focus();
  }, [active]);

  useEffect(() => {
    if (!intent || !session || handledIntent.current === intent.id) return;
    handledIntent.current = intent.id;
    if (intent.mode === "query") {
      dispatch({ type: "patch", value: { mode: "query", proposalOrigin: null, view: "conversation" } });
      window.setTimeout(() => inputRef.current?.focus());
      return;
    }
    if (session.read_only) {
      showError("Vuelve a la versión actual antes de proponer cambios.");
      return;
    }
    dispatch({ type: "patch", value: { mode: "propose", proposalOrigin: null, request: intent.request ?? "" } });
    window.setTimeout(() => inputRef.current?.focus());
    if (intent.template) void prepareTemplate(intent.template);
  }, [intent?.id, session?.world_id]);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const registrations = [
      listen<ProgressEvent>("ai-query-progress", ({ payload }) => {
        const current = workspaceRef.current;
        if (payload.requestId !== current.activeRequestId) return;
        aiObservation.current.phase = payload.progress.kind;
        if (payload.progress.delta) {
          const streamedText = current.streamedText + payload.progress.delta;
          aiObservation.current.receivedCharacters += payload.progress.delta.length;
          workspaceRef.current = { ...current, streamedText, progress: `Recibiendo respuesta… ${streamedText.length} caracteres` };
          dispatch({ type: "patch", value: { streamedText, progress: `Recibiendo respuesta… ${streamedText.length} caracteres` } });
        } else {
          dispatch({ type: "patch", value: { progress: humanize(payload.progress.kind) } });
        }
      }),
      listen<ProgressEvent>("ai-proposal-progress", ({ payload }) => {
        if (payload.requestId === workspaceRef.current.activeRequestId) {
          aiObservation.current.phase = payload.progress.kind;
          dispatch({ type: "patch", value: { progress: humanize(payload.progress.kind) } });
        }
      }),
      listen<ProgressEvent>("deep-review-progress", ({ payload }) => {
        if (payload.requestId !== workspaceRef.current.activeRequestId) return;
        aiObservation.current.phase = payload.progress.kind;
        dispatch({
          type: "patch",
          value: { progress: payload.progress.role ? `${humanize(payload.progress.kind)} · ${humanize(payload.progress.role)}` : humanize(payload.progress.kind) },
        });
      }),
    ];
    void Promise.all(registrations).then((registered) => {
      if (disposed) registered.forEach((unlisten) => unlisten());
      else unlisteners.push(...registered);
    });
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  function persistConversations(conversations: AssistantConversation[]): void {
    if (!workspaceRef.current.conversationWorldId) return;
    try {
      localStorage.setItem(conversationStorageKey(workspaceRef.current.conversationWorldId), JSON.stringify(conversations.slice(0, maxConversations)));
    } catch {
      dispatch({ type: "patch", value: { progress: "El historial sigue disponible en esta sesión, pero no pudo guardarse en este equipo." } });
    }
  }

  function setRunning(requestId: string | null): void {
    const previous = workspaceRef.current.activeRequestId;
    if (requestId) {
      aiObservation.current = { startedAtMs: Date.now(), phase: "preparing", receivedCharacters: 0 };
      beginAiActivity({ requestId, source: "assistant", label: "El asistente está procesando tu solicitud." });
    } else if (previous) {
      endAiActivity(previous);
    }
    workspaceRef.current = { ...workspaceRef.current, activeRequestId: requestId };
    dispatch({ type: "patch", value: { activeRequestId: requestId } });
  }

  async function invalidatePendingReview(run: AiRunSnapshot | null): Promise<void> {
    if (!session || !run?.reviewKey || !run.draft) return;
    await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) });
  }

  function updateMode(mode: AssistantMode): void {
    if (session?.read_only && isWriteMode(mode)) return;
    dispatch({
      type: "patch",
      value: {
        mode,
        proposalOrigin: null,
        confirmingTurnId: null,
        view: "conversation",
        activeRun: null,
        activeRunOrigin: null,
        activeDeepRun: null,
        deepPlan: null,
        progress: "",
      },
    });
    setTemplatesOpen(false);
  }

  function startProposal(request = "", origin: ProposalOrigin | null = null): void {
    if (session?.read_only) {
      showError("Vuelve a la versión actual antes de proponer cambios.");
      return;
    }
    dispatch({ type: "patch", value: { mode: "propose", request, proposalOrigin: origin, confirmingTurnId: null, view: "conversation", activeRun: null, activeDeepRun: null, deepPlan: null, progress: "" } });
    setTemplatesOpen(false);
    window.setTimeout(() => inputRef.current?.focus());
  }

  function newConversation(): AssistantConversation | null {
    if (!session) return null;
    const conversation = createConversation(session.world_id);
    const conversations = [conversation, ...workspaceRef.current.conversations].slice(0, maxConversations);
    workspaceRef.current = { ...workspaceRef.current, conversations, activeConversationId: conversation.id };
    persistConversations(conversations);
    dispatch({
      type: "patch",
      value: {
        mode: "query",
        request: "",
        proposalOrigin: null,
        conversations,
        activeConversationId: conversation.id,
        deleteArmed: false,
        view: "conversation",
      },
    });
    window.setTimeout(() => inputRef.current?.focus());
    return conversation;
  }

  function deleteConversation(): void {
    if (!activeConversation) return;
    if (!workspace.deleteArmed) {
      dispatch({ type: "patch", value: { deleteArmed: true, progress: "La conversación se borrará solo de este equipo; el canon y las propuestas no cambiarán." } });
      return;
    }
    let conversations = workspace.conversations.filter((item) => item.id !== activeConversation.id);
    if (conversations.length === 0 && session) conversations = [createConversation(session.world_id)];
    persistConversations(conversations);
    dispatch({
      type: "patch",
      value: {
        conversations,
        activeConversationId: conversations[0]?.id ?? null,
        deleteArmed: false,
        confirmingTurnId: null,
        progress: "Conversación eliminada. El mundo y sus propuestas no cambiaron.",
      },
    });
    window.setTimeout(() => inputRef.current?.focus());
  }

  async function prepareTemplate(template: ProposalTemplate): Promise<void> {
    if (!session || session.read_only) {
      showError("Vuelve a la versión actual antes de preparar una plantilla.");
      return;
    }
    clearError();
    const origin = currentOrigin(session, selectedUri);
    try {
      const run = await invoke<AiRunSnapshot>("prepare_ai_proposal_template", {
        input: { template, scale: templateScale, anchorUri: selectedUri },
      });
      if (!originIsCurrent(session, origin)) return;
      dispatch({ type: "patch", value: { activeRun: run, activeRunOrigin: origin, activeDeepRun: null, deepPlan: null, view: "run", progress: "Brief preparado localmente. Todavía no se llamó al proveedor." } });
    } catch (value) {
      showAiCommandError(value, aiObservation.current);
    }
  }

  async function continueIntentBrief(run: AiRunSnapshot, edited: {
    objective: string;
    scope: string;
    entities: SearchResult[];
    restrictions: string[];
    scale?: ProposalScale;
  }): Promise<void> {
    if (!session || session.read_only) {
      showError("Vuelve a la versión actual antes de continuar una propuesta.");
      return;
    }
    if (!workspaceRef.current.activeRunOrigin || !originIsCurrent(session, workspaceRef.current.activeRunOrigin)) {
      showError("La versión observada cambió. Prepara de nuevo el brief antes de continuar.");
      return;
    }
    if (!run.intentBrief || !providerReady) return;
    const requestId = crypto.randomUUID();
    setRunning(requestId);
    try {
      const next = await invoke<AiRunSnapshot>("execute_ai_proposal_from_brief", {
        input: {
          requestId,
          runId: run.id,
          objective: edited.objective.trim(),
          scope: edited.scope.trim(),
          entityUris: edited.entities.map((entity) => entity.uri),
          restrictions: edited.restrictions,
          scale: edited.scale ?? null,
        },
      });
      dispatch({ type: "patch", value: { activeRun: next, view: "run" } });
      await invalidatePendingReview(next);
    } catch (value) {
      showAiCommandError(value, aiObservation.current);
    } finally {
      setRunning(null);
    }
  }

  async function submitAssistant(event: FormEvent): Promise<void> {
    event.preventDefault();
    const request = workspace.request.trim();
    if (!request || !session || !providerReady || running) return;
    if (session.read_only && isWriteMode(workspace.mode)) {
      showError("Las propuestas solo pueden iniciarse desde la versión actual.");
      updateMode("query");
      return;
    }
    if (isWriteMode(workspace.mode) && workspace.activeRun?.reviewKey) {
      const current = await invoke<AiRunSnapshot>("read_ai_run", { runId: workspace.activeRun.id });
      dispatch({ type: "patch", value: { activeRun: current } });
      if (!["committed", "cancelled", "failed"].includes(current.status)) {
        showError("Termina o descarta la propuesta activa antes de iniciar otra.");
        return;
      }
    }
    clearError();
    const requestId = crypto.randomUUID();
    dispatch({ type: "patch", value: { streamedText: "", progress: workspace.mode === "query" ? "Preparando consulta…" : "Preparando propuesta…" } });
    setRunning(requestId);
    try {
      if (workspace.mode === "query") {
        let conversation = activeConversation ?? newConversation();
        const previousOrigin = conversation?.turns.at(-1)?.origin;
        if (previousOrigin && !originIsCurrent(session, previousOrigin)) {
          conversation = newConversation();
          dispatch({ type: "patch", value: { progress: "La versión cambió; la consulta continúa en una conversación nueva." } });
        }
        if (!conversation) return;
        const anchorUri = selectedUri;
        const response = await invoke<AiQueryResponse>("execute_ai_query", {
          input: { requestId, request, anchorUri, history: conversationHistory(conversation) },
        });
        const sourceCount = new Set(response.items.flatMap((item) => item.citations.map((citation) => citation.source.uri))).size;
        const origin: ProposalOrigin = {
          worldId: response.snapshot.worldId,
          baseRevision: response.snapshot.baseRevision,
          readScope: response.snapshot.readScope,
          anchorUri,
          contextLabel: anchorUri ? labelForUri(anchorUri) : "contexto general",
          sourceCount,
        };
        const turn: AssistantConversationTurn = { id: crypto.randomUUID(), request, createdAtMs: Date.now(), response, origin };
        const conversations = workspaceRef.current.conversations.map((item) => item.id === conversation!.id
          ? {
              ...item,
              title: item.turns.length === 0 ? (request.length > 48 ? `${request.slice(0, 48).trimEnd()}…` : request) : item.title,
              updatedAtMs: Date.now(),
              turns: [...item.turns, turn].slice(-maxStoredTurns),
            }
          : item);
        persistConversations(conversations);
        dispatch({ type: "patch", value: { conversations, activeConversationId: conversation.id, request: "", view: "conversation" } });
        window.setTimeout(() => {
          const transcript = transcriptRef.current;
          if (typeof transcript?.scrollIntoView === "function") transcript.scrollIntoView({ block: "start" });
        });
      } else if (workspace.mode === "propose") {
        if (workspace.proposalOrigin && !originIsCurrent(session, workspace.proposalOrigin)) {
          showError("La versión observada cambió. Repite la consulta antes de preparar cambios.");
          return;
        }
        const origin = workspace.proposalOrigin ?? currentOrigin(session, selectedUri);
        const run = await invoke<AiRunSnapshot>("execute_ai_proposal", {
          input: { requestId, request, anchorUri: origin.anchorUri },
        });
        dispatch({ type: "patch", value: { activeRun: run, activeRunOrigin: origin, activeDeepRun: null, proposalOrigin: null, view: "run" } });
        await invalidatePendingReview(run);
      } else {
        const origin = currentOrigin(session, selectedUri);
        const plan = await invoke<DeepReviewPlan>("prepare_deep_review", {
          input: { mode: workspace.mode, request, anchorUri: origin.anchorUri },
        });
        dispatch({ type: "patch", value: { deepPlan: { plan, origin }, activeDeepRun: null, view: "deep_plan" } });
      }
      dispatch({ type: "patch", value: { progress: workspace.mode === "query" ? "" : "Propuesta preparada. Revísala antes de aplicarla." } });
    } catch (value) {
      showAiCommandError(value, aiObservation.current);
      dispatch({ type: "patch", value: { progress: "La ejecución terminó sin modificar el canon. Puedes reintentar." } });
    } finally {
      setRunning(null);
    }
  }

  async function executeDeepReview(prepared: DeepPlanState, roles: SpecialistRole[]): Promise<void> {
    if (!session || !originIsCurrent(session, prepared.origin)) {
      showError("La versión observada cambió. Prepara de nuevo la revisión antes de ejecutarla.");
      return;
    }
    if (prepared.plan.mode === "deep_impact" && session.read_only) {
      showError("Vuelve a la versión actual antes de preparar una propuesta profunda.");
      return;
    }
    if (roles.length === 0 || roles.length > prepared.plan.budget.maxSpecialists) {
      showError(`Selecciona entre 1 y ${prepared.plan.budget.maxSpecialists} roles.`);
      return;
    }
    const requestId = crypto.randomUUID();
    setRunning(requestId);
    dispatch({ type: "patch", value: { progress: "Iniciando especialistas confirmados…" } });
    try {
      const run = await invoke<DeepReviewRun>("execute_deep_review", {
        input: {
          requestId,
          mode: prepared.plan.mode,
          request: prepared.plan.request,
          roles,
          anchorUri: prepared.origin.anchorUri,
        },
      });
      let activeRun: AiRunSnapshot | null = null;
      if (run.standardRunId && run.status === "awaiting_review") {
        activeRun = await invoke<AiRunSnapshot>("read_ai_run", { runId: run.standardRunId });
        await invalidatePendingReview(activeRun);
      }
      dispatch({ type: "patch", value: { activeDeepRun: run, activeRun, activeRunOrigin: activeRun ? prepared.origin : null, view: "deep_run" } });
    } catch (value) {
      showAiCommandError(value, aiObservation.current);
    } finally {
      setRunning(null);
    }
  }

  const contextLabel = workspace.mode === "propose" && workspace.proposalOrigin
    ? `Contexto heredado de la consulta: ${workspace.proposalOrigin.contextLabel}.`
    : selectedUri
      ? "La solicitud usará el objeto seleccionado como contexto."
      : "Sin selección: se usará contexto general acotado.";
  const submitLabel = workspace.mode === "query"
    ? "Enviar pregunta"
    : workspace.mode === "propose"
      ? "Preparar propuesta"
      : workspace.mode === "audit"
        ? "Preparar auditoría"
        : "Preparar revisión profunda";
  const composing = workspace.view === "conversation";
  const queryMode = workspace.mode === "query";
  const proposalMode = workspace.mode === "propose";
  const canGoBack = templatesOpen || workspace.view !== "conversation";

  function goBack() {
    if (templatesOpen) {
      setTemplatesOpen(false);
      return;
    }
    dispatch({ type: "patch", value: { view: "conversation", progress: "" } });
    window.setTimeout(() => inputRef.current?.focus());
  }

  return (
    <Dialog.Root open={active} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Portal>
        <Dialog.Overlay className="assistant-sheet-backdrop" />
        <Dialog.Content
          asChild
          aria-describedby={undefined}
          onInteractOutside={(event) => {
            if ((event.target as HTMLElement).closest(".global-feedback")) event.preventDefault();
          }}
        >
      <section id="assistant-panel" className="assistant-panel" aria-labelledby="assistant-title" aria-modal="true">
        <header className="assistant-shell-header">
          <button type="button" className="icon-button" aria-label="Volver en el asistente" title="Volver" disabled={!canGoBack} onClick={goBack}><Icon name="arrow-left" /></button>
          <div className="assistant-heading"><p className="panel-eyebrow">{queryMode ? "Solo lectura" : proposalMode ? "Cambios revisables" : "Análisis avanzado"}</p><Dialog.Title asChild><h2 id="assistant-title">Asistente</h2></Dialog.Title></div>
          <Dialog.Close asChild><button type="button" className="icon-button assistant-sheet-close" aria-label="Cerrar asistente" title="Cerrar asistente"><Icon name="x" /></button></Dialog.Close>
        </header>
        <div className="assistant-scroll-region">
        <div className="assistant-task-tabs" role="tablist" aria-label="Tarea del asistente">
          <button type="button" role="tab" aria-selected={queryMode} disabled={running} onClick={() => updateMode("query")}>Preguntar</button>
          <button type="button" role="tab" aria-selected={proposalMode} disabled={running || Boolean(session?.read_only)} onClick={() => startProposal()}>Proponer un cambio</button>
        </div>
        <p id="assistant-context" className="assistant-context">{contextLabel}</p>
        {queryMode && <details className="assistant-history">
          <summary>Historial de conversaciones</summary>
          <div className="assistant-conversations">
            <label>Conversación
            <select
              id="assistant-conversation-select"
              name="assistant-conversation"
              value={workspace.activeConversationId ?? ""}
              onChange={(event) => dispatch({ type: "patch", value: { mode: "query", proposalOrigin: null, activeConversationId: event.currentTarget.value, deleteArmed: false, confirmingTurnId: null, view: "conversation" } })}
            >
              {workspace.conversations.map((conversation) => <option key={conversation.id} value={conversation.id}>{conversation.title}</option>)}
            </select>
            </label>
            <div className="pending-actions">
              <button type="button" className="secondary" disabled={running} onClick={newConversation}>Nueva</button>
              <button type="button" className="ghost" disabled={running || (workspace.conversations.length === 1 && (activeConversation?.turns.length ?? 0) === 0)} onClick={deleteConversation}>{workspace.deleteArmed ? "Confirmar eliminación" : "Eliminar"}</button>
            </div>
            <p>Se guarda solo en este equipo y no forma parte del canon.</p>
          </div>
        </details>}
        {composing && !provider.isPending && !providerReady && <section className="notice warning assistant-provider-required">
          <h4>Configura la IA una sola vez</h4>
          <p>{provider.data?.message ?? "Falta completar la conexión con Microsoft Foundry."}</p>
          <button type="button" className="secondary" onClick={onOpenSettings}>Abrir Ajustes de IA</button>
        </section>}
        {composing && (queryMode || proposalMode) && <details className="assistant-advanced-modes">
            <summary>Más opciones</summary>
            <div className="assistant-advanced-options">
              <button type="button" className={`assistant-profile${workspace.mode === "deep_impact" ? "" : " secondary"}`} aria-pressed={workspace.mode === "deep_impact"} disabled={running || Boolean(session?.read_only)} onClick={() => updateMode("deep_impact")}>
                <strong>Revisión profunda</strong><span>Especialistas de solo lectura analizan el impacto. La síntesis prepara una propuesta, pero nunca la aplica.</span>
              </button>
              <button type="button" className={`assistant-profile${workspace.mode === "audit" ? "" : " secondary"}`} aria-pressed={workspace.mode === "audit"} disabled={running} onClick={() => updateMode("audit")}>
                <strong>Auditoría del canon</strong><span>Busca problemas y presenta hallazgos orientativos. Es solo lectura y no crea propuestas.</span>
              </button>
            </div>
          </details>}
        {composing && proposalMode && !templatesOpen && <button type="button" className="secondary assistant-template-open" disabled={running} onClick={() => setTemplatesOpen(true)}>Usar una plantilla</button>}
        {composing && proposalMode && templatesOpen && (
          <section id="assistant-template-catalog" className="assistant-template-catalog" aria-labelledby="assistant-template-title">
            <div className="assistant-template-heading">
              <div><p className="panel-eyebrow">Expansión guiada</p><h4 id="assistant-template-title">Empezar desde una plantilla</h4></div>
              <label>Escala
                <select name="assistant-template-scale" value={templateScale} onChange={(event) => setTemplateScale(event.currentTarget.value as ProposalScale)}>
                  <option value="small">Pequeña · máximo 3 operaciones</option>
                  <option value="medium">Mediana · máximo 6 operaciones</option>
                </select>
              </label>
            </div>
            <p className="muted">Preparar el brief es local y no llama al proveedor. Continuar usa la propuesta y revisión habituales.</p>
            <div className="assistant-template-grid">
              {templates.map((template) => <button key={template.id} type="button" className="assistant-template-card secondary" data-template={template.id} disabled={running || Boolean(session?.read_only)} onClick={() => void prepareTemplate(template.id)}><strong>{template.label}</strong><span>{template.detail}</span></button>)}
            </div>
            <button type="button" className="ghost" onClick={() => setTemplatesOpen(false)}>Volver</button>
          </section>
        )}
        {composing && !templatesOpen && <form className="assistant-form" onSubmit={(event) => void submitAssistant(event)}>
          <label>{queryMode ? "Tu pregunta" : proposalMode ? "Cambio que quieres preparar" : "Qué quieres analizar"}
            <textarea id="assistant-input" ref={inputRef} name="assistant-request" rows={3} autoComplete="off" placeholder={queryMode ? "Ej.: ¿Qué sabemos de esta ciudad?" : proposalMode ? "Ej.: Añade una tensión política a esta ciudad." : "Describe el análisis que necesitas."} value={workspace.request} disabled={running} onChange={(event) => dispatch({ type: "patch", value: { request: event.currentTarget.value } })} />
          </label>
          <div className="pending-actions">
            <button id="assistant-submit" type="submit" disabled={running || !providerReady || writeBlocked || !workspace.request.trim()}>{submitLabel}</button>
            {running && <button type="button" className="secondary" onClick={() => { if (workspace.activeRequestId) void invoke("cancel_ai_request", { requestId: workspace.activeRequestId }); }}>Cancelar</button>}
          </div>
        </form>}
        {composing && queryMode && (activeConversation?.turns.length ?? 0) === 0 && !workspace.request && <section className="assistant-example"><p><strong>Ejemplo:</strong> ¿Qué tensiones ya aparecen en el canon?</p><button type="button" className="ghost" onClick={() => { dispatch({ type: "patch", value: { request: "¿Qué tensiones ya aparecen en el canon?" } }); inputRef.current?.focus(); }}>Usar ejemplo</button></section>}
        {workspace.progress && <div className="assistant-progress" aria-live="polite">{workspace.progress}</div>}
        <div ref={transcriptRef} className="assistant-transcript" aria-live="polite">
          {workspace.view === "conversation" && queryMode && <ConversationView
            conversation={activeConversation}
            streamedText={running && workspace.mode === "query" ? workspace.streamedText : ""}
            confirmingTurnId={workspace.confirmingTurnId}
            readOnly={Boolean(session?.read_only)}
            onOpenCitation={(uri, scope) => void selectUriInScope(uri, scope).catch(showError)}
            onConfirm={(turnId) => dispatch({ type: "patch", value: { confirmingTurnId: turnId } })}
            onCancelConfirm={() => dispatch({ type: "patch", value: { confirmingTurnId: null } })}
            onConvert={(turn) => {
              if (!originIsCurrent(session, turn.origin)) {
                showError("La versión observada cambió. Repite la pregunta antes de convertirla en propuesta.");
                return;
              }
              startProposal(turn.response.proposalAction?.request ?? "", turn.origin);
            }}
          />}
          {workspace.view === "run" && workspace.activeRun && <RunView run={workspace.activeRun} providerReady={providerReady} running={running} writeAllowed={Boolean(session && !session.read_only && workspace.activeRunOrigin && originIsCurrent(session, workspace.activeRunOrigin))} onContinue={continueIntentBrief} />}
          {workspace.view === "deep_plan" && workspace.deepPlan && <DeepPlanView key={`${workspace.deepPlan.plan.mode}:${workspace.deepPlan.plan.request}`} prepared={workspace.deepPlan} running={running} onExecute={executeDeepReview} />}
          {workspace.view === "deep_run" && workspace.activeDeepRun && <DeepRunView run={workspace.activeDeepRun} readOnly={Boolean(session?.read_only)} />}
        </div>
        {workspace.view === "run" && workspace.activeRun?.reviewKey && !workspace.activeRun.intentBrief && <div className="assistant-result-actions"><button type="button" onClick={onOpenReviews}>Abrir en Cambios</button><button type="button" className="secondary" onClick={() => startProposal()}>Preparar otra propuesta</button></div>}
        </div>
      </section>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function ConversationView({ conversation, streamedText, confirmingTurnId, readOnly, onOpenCitation, onConfirm, onCancelConfirm, onConvert }: {
  conversation: AssistantConversation | null;
  streamedText: string;
  confirmingTurnId: string | null;
  readOnly: boolean;
  onOpenCitation: (uri: string, scope: ReadScope) => void;
  onConfirm: (turnId: string) => void;
  onCancelConfirm: () => void;
  onConvert: (turn: AssistantConversationTurn) => void;
}) {
  if (!conversation || conversation.turns.length === 0) {
    return streamedText ? <article className="assistant-message"><strong>Recibiendo respuesta</strong><pre>{streamedText}</pre></article> : null;
  }
  return <>{conversation.turns.map((turn) => <QueryTurn key={turn.id} turn={turn} confirming={confirmingTurnId === turn.id} readOnly={readOnly} onOpenCitation={onOpenCitation} onConfirm={() => onConfirm(turn.id)} onCancelConfirm={onCancelConfirm} onConvert={() => onConvert(turn)} />)}{streamedText && <article className="assistant-message"><strong>Stream estructurado en curso</strong><pre>{streamedText}</pre></article>}</>;
}

function QueryTurn({ turn, confirming, readOnly, onOpenCitation, onConfirm, onCancelConfirm, onConvert }: {
  turn: AssistantConversationTurn;
  confirming: boolean;
  readOnly: boolean;
  onOpenCitation: (uri: string, scope: ReadScope) => void;
  onConfirm: () => void;
  onCancelConfirm: () => void;
  onConvert: () => void;
}) {
  const convertRef = useRef<HTMLButtonElement>(null);
  return <>
    <article className="assistant-message user-message"><div className="badge-row"><strong>Tú</strong><span className="muted">{turn.origin.contextLabel} · {new Date(turn.createdAtMs).toLocaleString()}</span></div><p>{turn.request}</p></article>
    {turn.response.items.map((item) => <article className="assistant-message" key={item.itemId}><div className="badge-row"><strong>{humanize(item.classification)}</strong></div><p>{item.markdown}</p>{item.citations.length > 0 && <div className="assistant-sources">{Array.from(new Map(item.citations.map((citation) => [citation.source.uri, citation])).values()).map((citation) => <button key={citation.source.uri} type="button" className="ghost" title={citation.quoteMd} onClick={() => onOpenCitation(citation.source.uri, turn.response.snapshot.readScope)}>{citation.source.snippet || "Abrir fuente"}</button>)}</div>}</article>)}
    {turn.response.proposalAction?.action === "start_proposal" && <article className="assistant-message proposal-action"><p>Esta respuesta puede convertirse en una propuesta revisable. El mundo no cambiará automáticamente.</p>{!confirming ? <button ref={convertRef} type="button" className="secondary" disabled={readOnly} title={readOnly ? "Vuelve a la versión actual para proponer cambios." : ""} onClick={onConfirm}>Convertir en propuesta</button> : <div className="proposal-confirmation"><h4>Confirmar paso a propuesta</h4><blockquote>{turn.response.proposalAction.request}</blockquote><p>Contexto: {turn.origin.contextLabel} · versión actual · {turn.origin.sourceCount} fuentes heredadas</p><p>Esto preparará cambios revisables. No modifica el mundo hasta usar «Aplicar al mundo».</p><div className="pending-actions"><button type="button" className="ghost" autoFocus onClick={() => { onCancelConfirm(); window.setTimeout(() => convertRef.current?.focus()); }}>Cancelar</button><button type="button" className="secondary" onClick={onConvert}>Continuar a Proponer cambios</button></div></div>}</article>}
  </>;
}

function RunView({ run, providerReady, running, writeAllowed, onContinue }: {
  run: AiRunSnapshot;
  providerReady: boolean;
  running: boolean;
  writeAllowed: boolean;
  onContinue: (run: AiRunSnapshot, edited: { objective: string; scope: string; entities: SearchResult[]; restrictions: string[]; scale?: ProposalScale }) => Promise<void>;
}) {
  return <article className="assistant-message proposal"><h4>{run.intentBrief?.objective ?? run.draft?.objective ?? "Propuesta preparada"}</h4>{run.intentBrief ? <IntentBriefForm key={run.id} run={run} providerReady={providerReady} running={running} writeAllowed={writeAllowed} onContinue={onContinue} /> : <p>La propuesta está fuera del canon. Revísala en Cambios antes de decidir si la aplicas.</p>}</article>;
}

function IntentBriefForm({ run, providerReady, running, writeAllowed, onContinue }: {
  run: AiRunSnapshot;
  providerReady: boolean;
  running: boolean;
  writeAllowed: boolean;
  onContinue: (run: AiRunSnapshot, edited: { objective: string; scope: string; entities: SearchResult[]; restrictions: string[]; scale?: ProposalScale }) => Promise<void>;
}) {
  const requestObjectPicker = useObjectPicker();
  const brief = run.intentBrief!;
  const [objective, setObjective] = useState(brief.objective);
  const [scope, setScope] = useState(brief.scope);
  const [entities, setEntities] = useState(brief.entities);
  const [restrictions, setRestrictions] = useState(brief.restrictions.join("\n"));
  const [scale, setScale] = useState<ProposalScale>(brief.scale ?? "small");
  const chooseRef = useRef<HTMLButtonElement>(null);
  const capturedUris = [...run.context.context.canon, ...run.context.context.perspectives, ...run.context.context.desires, ...run.context.context.obligations, ...run.context.context.searchEvidence].map((entry) => entry.uri);
  return <form className="intent-brief-form" onSubmit={(event) => { event.preventDefault(); void onContinue(run, { objective, scope, entities, restrictions: restrictions.split("\n").map((value) => value.trim()).filter(Boolean), scale: brief.template ? scale : undefined }); }}>
    <p className="read-only-callout">{brief.authority}</p><p className="muted">Por qué se prepara este brief: {brief.reason}</p>
    <label>Objetivo<textarea name="intent-objective" rows={2} value={objective} onChange={(event) => setObjective(event.currentTarget.value)} /></label>
    <label>Alcance<textarea name="intent-scope" rows={2} value={scope} onChange={(event) => setScope(event.currentTarget.value)} /></label>
    <fieldset><legend>Entidades por nombre</legend><div className="intent-brief-entities">{entities.length === 0 && <p className="muted">Sin entidades concretas; el mundo permanece como fuente de contexto.</p>}{entities.map((entity) => <button key={entity.uri} type="button" className="ghost" onClick={() => setEntities((current) => current.filter((item) => item.uri !== entity.uri))}>{entity.snippet.replace(/[\[\]]/gu, "")} · quitar</button>)}</div><button ref={chooseRef} type="button" className="secondary" onClick={() => requestObjectPicker({ title: "Entidades del contexto capturado", kinds: ["entity"], multiple: true, allowedUris: capturedUris, returnFocus: chooseRef.current, apply: setEntities })}>Elegir entidades por nombre</button></fieldset>
    <label>Restricciones, una por línea<textarea name="intent-restrictions" rows={4} value={restrictions} onChange={(event) => setRestrictions(event.currentTarget.value)} /></label>
    {brief.template && <label>Escala<select name="intent-scale" value={scale} onChange={(event) => setScale(event.currentTarget.value as ProposalScale)}><option value="small">Pequeña · máximo 3 operaciones</option><option value="medium">Mediana · máximo 6 operaciones</option></select></label>}
    <button type="submit" className="secondary" disabled={!providerReady || running || !writeAllowed} title={!writeAllowed ? "La versión observada cambió. Prepara de nuevo el brief." : providerReady ? "" : "Configura y verifica la IA para continuar. El brief ya está guardado."}>Continuar al proveedor</button>
  </form>;
}

function DeepPlanView({ prepared, running, onExecute }: { prepared: DeepPlanState; running: boolean; onExecute: (prepared: DeepPlanState, roles: SpecialistRole[]) => Promise<void> }) {
  const [roles, setRoles] = useState(prepared.plan.roles);
  const plan = prepared.plan;
  return <article className="assistant-message proposal deep-review"><h4>{plan.mode === "audit" ? "Confirmar auditoría del canon" : "Confirmar revisión profunda"}</h4><p className={plan.mode === "audit" ? "read-only-callout" : "muted"}>{plan.mode === "audit" ? "Resultado orientativo · solo lectura. No se crearán operaciones ni propuestas." : "Los especialistas solo leen el canon. La síntesis irá a Cambios y requerirá «Aplicar al mundo»."}</p><p>{plan.reason}</p><p className="muted">Hasta {plan.budget.maxSpecialists} especialistas · {Math.round(plan.budget.specialistTimeoutMs / 1000)} s por rol · {plan.budget.maxReadToolCalls} lecturas de contexto por rol</p><details className="technical-disclosure"><summary>Detalles técnicos del presupuesto</summary><p>{plan.budget.maxSpecialistCalls} llamadas de especialistas · {plan.budget.maxSynthesisCalls} de síntesis · {plan.budget.specialistMaxOutputTokens} tokens por informe · {plan.budget.maxNestedDelegations} delegaciones</p></details><fieldset><legend>Roles confirmados por el usuario</legend>{plan.roles.map((role) => <label className="deep-role" key={role}><input type="checkbox" value={role} checked={roles.includes(role)} onChange={(event) => setRoles((current) => event.currentTarget.checked ? [...current, role] : current.filter((item) => item !== role))} />{specialistRoleLabel(role)}</label>)}</fieldset><button type="button" className="secondary" disabled={running} onClick={() => void onExecute(prepared, roles)}>Confirmar roles e iniciar</button></article>;
}

function DeepRunView({ run, readOnly }: { run: DeepReviewRun; readOnly: boolean }) {
  const positions = new Map<string, Set<string>>();
  for (const specialist of run.specialists) for (const finding of specialist.report?.findings ?? []) if (finding.decisionPosition) {
    const alternatives = positions.get(finding.decisionPosition.decisionKey) ?? new Set<string>();
    alternatives.add(finding.decisionPosition.alternative);
    positions.set(finding.decisionPosition.decisionKey, alternatives);
  }
  const findingCount = run.auditResult ? Object.values(run.auditResult.validationReport).reduce((total, findings) => total + findings.length, 0) : 0;
  const cards: ReactNode[] = [];
  cards.push(<article className="assistant-message proposal deep-review" key="summary"><h4>{run.mode === "audit" ? "Auditoría del canon" : "Revisión profunda"}</h4><p>Estado: {humanize(run.status)} · versión analizada: {readOnly ? "versión anterior" : "versión actual"}</p><p className={run.mode === "audit" ? "read-only-callout" : "muted"}>{run.mode === "audit" ? "Resultado orientativo · solo lectura. No se creó ninguna propuesta." : "La síntesis no modifica el mundo: debe revisarse en Cambios y aplicarse explícitamente."}</p>{run.error && <p className="assistant-issue conflict">La revisión no pudo completarse. El canon no cambió y puedes volver a intentarlo.</p>}</article>);
  for (const specialist of run.specialists) cards.push(<article className="assistant-message specialist-report" key={specialist.role}><h4>{specialistRoleLabel(specialist.role)} · {humanize(specialist.status)}</h4>{specialist.error && <p className="assistant-issue warning">Este análisis especializado no pudo completarse. Los demás resultados siguen disponibles.</p>}{specialist.report?.findings.map((finding) => <div key={finding.findingId}><p>{finding.summary.markdown}</p>{finding.evidence.map((evidence) => <button key={evidence.sourceUri} type="button" className="ghost" title={evidence.excerptMd} onClick={() => void selectUri(evidence.sourceUri)}>{evidence.sourceUri}</button>)}</div>)}</article>);
  for (const [key, alternatives] of positions) if (alternatives.size > 1) cards.push(<article className="assistant-message assistant-issue conflict" key={key}><h4>Desacuerdo: {humanize(key)}</h4><p>{Array.from(alternatives).join(" / ")}</p></article>);
  if (run.auditResult) cards.push(<article className="assistant-message audit-result" key="audit"><h4>Hallazgos de la auditoría</h4><p>{findingCount} hallazgos consolidados · {run.auditResult.findingIds.length} evidencias de especialistas · ninguna propuesta creada</p></article>);
  if (run.synthesis?.draft) cards.push(<article className="assistant-message proposal" key="synthesis"><h4>Síntesis enviada a Cambios</h4><p>{run.synthesis.draft.operations.length} operaciones · {run.synthesis.draft.decisions.length} decisiones pendientes · todavía requiere revisión y «Aplicar al mundo»</p></article>);
  return <>{cards}</>;
}
