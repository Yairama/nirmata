import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useDeferredValue, useEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent } from "react";
import { setConfirmationHandler } from "./confirmation.js";
import type { ConfirmationRequest } from "./confirmation.js";
import { CommandPalette } from "./command-palette.js";
import type { PaletteAction } from "./command-palette.js";
import { SoftwareDialogs } from "./software-dialogs.js";
import type { SoftwareDialog } from "./software-dialogs.js";
import { useSession } from "./session-provider.js";
import type { AiActivity, ReadScope, SearchObjectKind, SearchResult, SearchWorldResponse, Variant } from "./types.js";
import { observeRevision, switchWritingVariant, VersionsWorkspace, viewActiveVersion } from "./versions-workspace.js";
import { openReviewOperationEditor, selectUri, selectUriInScope, startCreatingObject, startEditingWorld } from "./workspace.js";
import { WorldExplorer } from "./world-explorer.js";
import { WorldContext } from "./world-context.js";
import { WorldTimeline } from "./world-timeline.js";
import { showError } from "./helpers.js";
import { PendingReviews, pendingReviewsQueryKey } from "./pending-reviews.js";
import { observedScopeQueryKey, useWorkspaceData } from "./workspace-data.js";
import type { AssistantIntent, ProposalTemplate } from "./assistant-workspace.js";
import type { DesktopActionRequest } from "./desktop-actions.js";

const AssistantWorkspace = lazy(() => import("./assistant-workspace.js").then((module) => ({ default: module.AssistantWorkspace })));
const ImportCenter = lazy(() => import("./import-center.js").then((module) => ({ default: module.ImportCenter })));
const NarrativeWorkspace = lazy(() => import("./narrative-workspace.js").then((module) => ({ default: module.NarrativeWorkspace })));
const SimulationWorkspace = lazy(() => import("./simulation-workspace.js").then((module) => ({ default: module.SimulationWorkspace })));
const StructuredEditor = lazy(() => import("./structured-editor.js").then((module) => ({ default: module.StructuredEditor })));

type WorldArea = "home" | "world" | "chronology" | "assistant" | "narrative" | "simulation" | "imports" | "versions";
type WorkspaceRegion = "explorer" | "editor" | "context";
type WorkspaceSide = "explorer" | "context";
type WorkspaceLayout = {
  explorerWidth: number;
  contextWidth: number;
  explorerCollapsed: boolean;
  contextCollapsed: boolean;
};

const EXPLORER_MIN_WIDTH = 180;
const CONTEXT_MIN_WIDTH = 220;
const PANEL_MAX_WIDTH = 480;
const EDITOR_MIN_WIDTH = 240;
const SPLITTERS_WIDTH = 24;
const WORKSPACE_KEYBOARD_STEP = 16;
const defaultWorkspaceLayout: WorkspaceLayout = {
  explorerWidth: 288,
  contextWidth: 304,
  explorerCollapsed: false,
  contextCollapsed: false,
};

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function workspaceLayoutKey(worldId: string): string {
  return `nirmata.workspace.layout.${worldId}`;
}

function readWorkspaceLayout(worldId: string): WorkspaceLayout {
  try {
    const stored = JSON.parse(localStorage.getItem(workspaceLayoutKey(worldId)) ?? "null") as Partial<WorkspaceLayout> | null;
    return {
      explorerWidth: Number.isFinite(stored?.explorerWidth)
        ? clamp(Number(stored?.explorerWidth), EXPLORER_MIN_WIDTH, PANEL_MAX_WIDTH)
        : defaultWorkspaceLayout.explorerWidth,
      contextWidth: Number.isFinite(stored?.contextWidth)
        ? clamp(Number(stored?.contextWidth), CONTEXT_MIN_WIDTH, PANEL_MAX_WIDTH)
        : defaultWorkspaceLayout.contextWidth,
      explorerCollapsed: stored?.explorerCollapsed === true,
      contextCollapsed: stored?.contextCollapsed === true,
    };
  } catch {
    return { ...defaultWorkspaceLayout };
  }
}

function persistWorkspaceLayout(worldId: string, layout: WorkspaceLayout): void {
  try {
    localStorage.setItem(workspaceLayoutKey(worldId), JSON.stringify(layout));
  } catch {
    // The layout remains usable for the current session in hardened webviews.
  }
}

function fitWorkspaceLayout(layout: WorkspaceLayout, containerWidth: number) {
  const budget = Math.max(0, containerWidth - SPLITTERS_WIDTH - EDITOR_MIN_WIDTH);
  const explorerMinimum = layout.explorerCollapsed ? 0 : EXPLORER_MIN_WIDTH;
  const contextMinimum = layout.contextCollapsed ? 0 : CONTEXT_MIN_WIDTH;
  let explorerWidth = layout.explorerCollapsed ? 0 : clamp(layout.explorerWidth, EXPLORER_MIN_WIDTH, PANEL_MAX_WIDTH);
  let contextWidth = layout.contextCollapsed ? 0 : clamp(layout.contextWidth, CONTEXT_MIN_WIDTH, PANEL_MAX_WIDTH);
  const overflow = Math.max(0, explorerWidth + contextWidth - budget);
  const flexibleWidth = (explorerWidth - explorerMinimum) + (contextWidth - contextMinimum);

  if (overflow > 0 && flexibleWidth > 0) {
    const explorerReduction = Math.min(explorerWidth - explorerMinimum, overflow * (explorerWidth - explorerMinimum) / flexibleWidth);
    explorerWidth -= explorerReduction;
    contextWidth -= Math.min(contextWidth - contextMinimum, overflow - explorerReduction);
  }
  const remainingOverflow = Math.max(0, explorerWidth + contextWidth - budget);
  if (remainingOverflow > 0) {
    const contextReduction = Math.min(contextWidth, remainingOverflow);
    contextWidth -= contextReduction;
    explorerWidth = Math.max(0, explorerWidth - (remainingOverflow - contextReduction));
  }

  explorerWidth = Math.round(explorerWidth);
  contextWidth = Math.round(contextWidth);
  return {
    explorerWidth,
    contextWidth,
    explorerMax: Math.max(explorerWidth, Math.round(Math.min(PANEL_MAX_WIDTH, Math.max(0, budget - contextWidth)))),
    contextMax: Math.max(contextWidth, Math.round(Math.min(PANEL_MAX_WIDTH, Math.max(0, budget - explorerWidth)))),
  };
}

const workspaceRegions: Array<{ id: WorkspaceRegion; label: string; panelId: string }> = [
  { id: "explorer", label: "Explorador", panelId: "navigation-panel" },
  { id: "editor", label: "Editor", panelId: "editor-panel" },
  { id: "context", label: "Contexto", panelId: "context-panel" },
];

const areas: Array<{ id: WorldArea; label: string }> = [
  { id: "home", label: "Inicio" },
  { id: "world", label: "Mundo" },
  { id: "chronology", label: "Cronología" },
  { id: "assistant", label: "Asistente" },
  { id: "narrative", label: "Estudio narrativo" },
  { id: "simulation", label: "Simulación" },
  { id: "imports", label: "Importaciones" },
  { id: "versions", label: "Versiones" },
];

const createActions: Array<{ kind: SearchObjectKind; label: string }> = [
  { kind: "entity", label: "Crear entidad" },
  { kind: "relation", label: "Crear relación" },
  { kind: "event", label: "Crear evento" },
  { kind: "claim", label: "Crear afirmación" },
  { kind: "rule", label: "Crear regla" },
  { kind: "goal", label: "Crear meta" },
  { kind: "document", label: "Crear documento" },
];

const onboardingItems = [
  { id: "premise", label: "Definir la premisa", action: "Editar mundo" },
  { id: "rules", label: "Registrar una regla fundamental", action: "Crear regla" },
  { id: "entities", label: "Crear la primera entidad", action: "Crear entidad" },
  { id: "events", label: "Añadir un acontecimiento", action: "Crear evento" },
  { id: "query", label: "Hacer la primera consulta", action: "Consultar" },
  { id: "propose", label: "Preparar la primera propuesta", action: "Proponer" },
  { id: "applied", label: "Aplicar el primer conjunto de cambios", action: "Se completa desde el historial" },
] as const;

function readOnboarding(worldId: string, hasPremise: boolean): { completed: string[]; dismissed: boolean } {
  try {
    const stored = JSON.parse(localStorage.getItem(`nirmata.onboarding.${worldId}`) ?? "{}") as { completed?: string[]; dismissed?: boolean };
    return {
      completed: Array.from(new Set([...(stored.completed ?? []), ...(hasPremise ? ["premise"] : [])])),
      dismissed: Boolean(stored.dismissed),
    };
  } catch {
    return { completed: hasPremise ? ["premise"] : [], dismissed: false };
  }
}

export function WorldShell({ desktopAction, handoff, onHandoffHandled, aiActivity, onClose }: {
  desktopAction?: DesktopActionRequest | null;
  handoff: { id: number; kind: "proposal"; request: string } | { id: number; kind: "lore-import" } | null;
  onHandoffHandled: () => void;
  aiActivity: AiActivity | null;
  onClose: () => void;
}) {
  const session = useSession();
  const { revisionHistory } = useWorkspaceData();
  const queryClient = useQueryClient();
  const [area, setArea] = useState<WorldArea>("home");
  const mountedAreas = useRef(new Set<WorldArea>(["home"]));
  mountedAreas.current.add(area);
  const [workspaceRegion, setWorkspaceRegion] = useState<WorkspaceRegion>("explorer");
  const [narrowWorkspace, setNarrowWorkspace] = useState(() => window.matchMedia("(max-width: 920px)").matches);
  const [workspaceLayout, setWorkspaceLayout] = useState<WorkspaceLayout>(() => session
    ? readWorkspaceLayout(session.world_id)
    : { ...defaultWorkspaceLayout });
  const workspaceLayoutRef = useRef(workspaceLayout);
  const [workspaceMetrics, setWorkspaceMetrics] = useState(() => fitWorkspaceLayout(workspaceLayout, 1200));
  const workspaceResizeFrame = useRef<number | null>(null);
  const workspaceRef = useRef<HTMLDivElement>(null);
  const workspaceDrag = useRef<{
    side: WorkspaceSide;
    pointerId: number;
    startX: number;
    startWidth: number;
    pendingX: number;
    persistAfterFrame: boolean;
  } | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [dialog, setDialog] = useState<SoftwareDialog>(null);
  const [dialogTrigger, setDialogTrigger] = useState<HTMLElement | null>(null);
  const [pendingCount, setPendingCount] = useState(0);
  const [reviewOpen, setReviewOpen] = useState(false);
  const reviewReturnFocus = useRef<HTMLElement | null>(null);
  const [confirmation, setConfirmation] = useState<(ConfirmationRequest & { resolve: (accepted: boolean) => void }) | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [paletteTrigger, setPaletteTrigger] = useState<HTMLElement | null>(null);
  const [onboarding, setOnboarding] = useState(() => session
    ? readOnboarding(session.world_id, Boolean(session.world.premise_md.trim()))
    : { completed: [] as string[], dismissed: false });
  const areaHeading = useRef<HTMLHeadingElement>(null);
  const initialArea = useRef(true);
  const assistantReturnFocus = useRef<HTMLElement | null>(null);
  const assistantPreviousArea = useRef<WorldArea>("home");
  const assistantIntentId = useRef(0);
  const [assistantIntent, setAssistantIntent] = useState<AssistantIntent | null>(null);
  const [importIntent, setImportIntent] = useState<{ id: number; tab: "lore" | "snapshot" } | null>(null);
  const deferredPaletteQuery = useDeferredValue(paletteQuery.trim());
  const scopeKey = session
    ? observedScopeQueryKey(session)
    : ["world", "closed", "closed", "closed"] as const;
  const paletteResults = useQuery({
    queryKey: [...scopeKey, "palette-search", deferredPaletteQuery],
    queryFn: () => invoke<SearchWorldResponse>("search_world", {
      input: { queryText: deferredPaletteQuery, kind: "all", limit: 20 },
    }),
    enabled: Boolean(session && paletteOpen && deferredPaletteQuery.length >= 2),
    retry: false,
    staleTime: 0,
  });
  const variants = useQuery({
    queryKey: [...scopeKey, "variants"],
    queryFn: () => invoke<Variant[]>("list_variants"),
    enabled: Boolean(session && paletteOpen),
    retry: false,
  });

  useEffect(() => {
    setConfirmationHandler((request) => new Promise((resolve) => setConfirmation({ ...request, resolve })));
    return () => setConfirmationHandler(null);
  }, []);

  useEffect(() => {
    if (!session) return;
    document.body.classList.add("world-session-open");
    return () => {
      document.body.classList.remove("world-session-open");
      delete document.body.dataset.worldArea;
    };
  }, [session]);

  useEffect(() => {
    if (!session) return;
    const next = readWorkspaceLayout(session.world_id);
    workspaceLayoutRef.current = next;
    setWorkspaceLayout(next);
  }, [session?.world_id]);

  useEffect(() => {
    if (!session) return;
    document.body.dataset.worldArea = area;
    if (initialArea.current) {
      initialArea.current = false;
    } else if (area === "home") areaHeading.current?.focus();
  }, [area, session]);

  useEffect(() => {
    const media = window.matchMedia("(max-width: 920px)");
    const sync = () => setNarrowWorkspace(media.matches);
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, []);

  useEffect(() => {
    const workspace = workspaceRef.current;
    if (!workspace || !session) return;

    function applyLayout(layout: WorkspaceLayout) {
      const metrics = fitWorkspaceLayout(layout, workspace!.clientWidth);
      workspace!.style.setProperty("--explorer-panel-width", `${metrics.explorerWidth}px`);
      workspace!.style.setProperty("--context-panel-width", `${metrics.contextWidth}px`);
      setWorkspaceMetrics(metrics);
    }

    applyLayout(workspaceLayoutRef.current);
    const observer = new ResizeObserver(() => applyLayout(workspaceLayoutRef.current));
    observer.observe(workspace);
    return () => observer.disconnect();
  }, [session?.world_id]);

  useEffect(() => () => {
    if (workspaceResizeFrame.current !== null) cancelAnimationFrame(workspaceResizeFrame.current);
  }, []);

  useEffect(() => {
    if (!session || !handoff) return;
    if (handoff.kind === "proposal") openAssistant("propose", handoff.request);
    else {
      setArea("imports");
      setImportIntent({ id: handoff.id, tab: "lore" });
    }
    onHandoffHandled();
  }, [handoff?.id, session?.world_id]);

  useEffect(() => {
    if (!desktopAction || !session) return;
    switch (desktopAction.action) {
      case "edit.world": void configureCalendar(); break;
      case "edit.propose": if (!session.read_only) openAssistant("propose"); break;
      case "view.palette": openPalette(null); break;
      case "view.changes": openReviews(null); break;
      case "view.home": setArea("home"); break;
      case "view.world": setArea("world"); break;
      case "view.chronology": setArea("chronology"); break;
      case "view.assistant": openAssistant("query"); break;
      case "view.narrative": setArea("narrative"); break;
      case "view.simulation": setArea("simulation"); break;
      case "view.imports": setArea("imports"); break;
      case "view.versions": setArea("versions"); break;
      case "settings.open": setDialog("settings"); break;
      case "help.open": setDialog("help"); break;
      case "help.about": setDialog("about"); break;
      case "help.onboarding": showOnboarding(); break;
    }
  }, [desktopAction?.id, session?.world_id]);

  useEffect(() => {
    if (!session) return;
    setOnboarding(readOnboarding(session.world_id, Boolean(session.world.premise_md.trim())));
  }, [session]);

  useEffect(() => () => document.body.classList.remove("review-drawer-open"), []);

  if (!session) return null;
  const worldId = session.world_id;

  function applyWorkspaceLayout(next: WorkspaceLayout, persist: boolean) {
    workspaceLayoutRef.current = next;
    const workspace = workspaceRef.current;
    if (workspace) {
      const metrics = fitWorkspaceLayout(next, workspace.clientWidth);
      workspace.style.setProperty("--explorer-panel-width", `${metrics.explorerWidth}px`);
      workspace.style.setProperty("--context-panel-width", `${metrics.contextWidth}px`);
      setWorkspaceMetrics(metrics);
    }
    setWorkspaceLayout(next);
    if (persist) persistWorkspaceLayout(worldId, next);
  }

  function schedulePointerResize(clientX: number) {
    const drag = workspaceDrag.current;
    if (!drag) return;
    drag.pendingX = clientX;
    if (workspaceResizeFrame.current !== null) return;
    workspaceResizeFrame.current = requestAnimationFrame(() => {
      workspaceResizeFrame.current = null;
      const currentDrag = workspaceDrag.current;
      if (!currentDrag) return;
      const delta = currentDrag.pendingX - currentDrag.startX;
      const maximum = currentDrag.side === "explorer" ? workspaceMetrics.explorerMax : workspaceMetrics.contextMax;
      const minimum = currentDrag.side === "explorer" ? EXPLORER_MIN_WIDTH : CONTEXT_MIN_WIDTH;
      const width = clamp(
        currentDrag.startWidth + (currentDrag.side === "explorer" ? delta : -delta),
        minimum,
        Math.max(minimum, maximum),
      );
      const next = currentDrag.side === "explorer"
        ? { ...workspaceLayoutRef.current, explorerWidth: width, explorerCollapsed: false }
        : { ...workspaceLayoutRef.current, contextWidth: width, contextCollapsed: false };
      applyWorkspaceLayout(next, currentDrag.persistAfterFrame);
      if (currentDrag.persistAfterFrame) workspaceDrag.current = null;
    });
  }

  function startPointerResize(side: WorkspaceSide, event: ReactPointerEvent<HTMLElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    workspaceDrag.current = {
      side,
      pointerId: event.pointerId,
      startX: event.clientX,
      startWidth: side === "explorer" ? workspaceMetrics.explorerWidth : workspaceMetrics.contextWidth,
      pendingX: event.clientX,
      persistAfterFrame: false,
    };
  }

  function movePointerResize(event: ReactPointerEvent<HTMLElement>) {
    if (workspaceDrag.current?.pointerId !== event.pointerId) return;
    event.preventDefault();
    schedulePointerResize(event.clientX);
  }

  function finishPointerResize(event: ReactPointerEvent<HTMLElement>) {
    const drag = workspaceDrag.current;
    if (drag?.pointerId !== event.pointerId) return;
    drag.persistAfterFrame = true;
    schedulePointerResize(event.clientX);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }

  function resizeWithKeyboard(side: WorkspaceSide, event: ReactKeyboardEvent<HTMLElement>) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const width = side === "explorer" ? workspaceMetrics.explorerWidth : workspaceMetrics.contextWidth;
    const maximum = side === "explorer" ? workspaceMetrics.explorerMax : workspaceMetrics.contextMax;
    const collapsed = event.key === "Home";
    const direction = side === "explorer"
      ? event.key === "ArrowRight" ? 1 : -1
      : event.key === "ArrowLeft" ? 1 : -1;
    const preferredWidth = side === "explorer" ? workspaceLayoutRef.current.explorerWidth : workspaceLayoutRef.current.contextWidth;
    const nextWidth = collapsed
      ? preferredWidth
      : event.key === "End"
        ? maximum
        : clamp(width + direction * WORKSPACE_KEYBOARD_STEP, side === "explorer" ? EXPLORER_MIN_WIDTH : CONTEXT_MIN_WIDTH, maximum);
    const next = side === "explorer"
      ? { ...workspaceLayoutRef.current, explorerWidth: nextWidth, explorerCollapsed: collapsed }
      : { ...workspaceLayoutRef.current, contextWidth: nextWidth, contextCollapsed: collapsed };
    applyWorkspaceLayout(next, true);
  }

  function toggleWorkspacePanel(side: WorkspaceSide) {
    const next = side === "explorer"
      ? { ...workspaceLayoutRef.current, explorerCollapsed: !workspaceLayoutRef.current.explorerCollapsed }
      : { ...workspaceLayoutRef.current, contextCollapsed: !workspaceLayoutRef.current.contextCollapsed };
    applyWorkspaceLayout(next, true);
  }

  function openDialog(nextDialog: Exclude<SoftwareDialog, null>, trigger: HTMLElement) {
    setDialogTrigger(trigger);
    setDialog(nextDialog);
  }

  function openReviews(trigger: HTMLElement | null) {
    reviewReturnFocus.current = trigger ?? document.activeElement as HTMLElement | null;
    document.body.classList.add("review-drawer-open");
    setReviewOpen(true);
  }

  function closeReviews(restoreFocus = true) {
    document.body.classList.remove("review-drawer-open");
    setReviewOpen(false);
    if (restoreFocus) window.setTimeout(() => reviewReturnFocus.current?.focus());
  }

  function openSearch() {
    setArea("world");
  }

  function openPalette(trigger: HTMLElement | null) {
    setPaletteTrigger(trigger ?? document.activeElement as HTMLElement | null);
    setPaletteOpen(true);
  }

  async function openPaletteResult(result: SearchResult): Promise<boolean> {
    const opened = await selectUri(result.uri);
    if (opened) {
      setArea("world");
      setPaletteTrigger(null);
    }
    return opened;
  }

  async function openTimelineEvent(uri: string) {
    if (await selectUri(uri)) setArea("world");
  }

  async function openNarrativeObject(uri: string, scope: ReadScope) {
    try {
      if (await selectUriInScope(uri, scope)) setArea("world");
    } catch (value) {
      showError(value);
    }
  }

  async function startCreate(kind: SearchObjectKind) {
    if (await startCreatingObject(kind)) {
      setArea("world");
      if (narrowWorkspace) setWorkspaceRegion("editor");
    }
  }

  async function configureCalendar() {
    if (await startEditingWorld()) {
      setArea("world");
      if (narrowWorkspace) setWorkspaceRegion("editor");
    }
  }

  async function switchVariant(variantId: string) {
    if (await switchWritingVariant(variantId)) {
      await queryClient.invalidateQueries({ queryKey: ["world", worldId] });
      setArea("world");
    }
  }

  async function viewRevision(revisionId: string | null) {
    const changed = revisionId === null
      ? await viewActiveVersion()
      : await observeRevision(revisionId);
    if (changed) {
      await queryClient.invalidateQueries({ queryKey: ["world", worldId] });
      setArea("world");
    }
  }

  function openAssistant(mode: "query" | "propose", request = "", template?: ProposalTemplate) {
    assistantPreviousArea.current = area === "assistant" ? "home" : area;
    assistantReturnFocus.current = document.activeElement as HTMLElement | null;
    assistantIntentId.current += 1;
    setAssistantIntent({ id: assistantIntentId.current, mode, request, template });
    setArea("assistant");
  }

  function closeAssistant() {
    setArea(assistantPreviousArea.current);
    window.setTimeout(() => assistantReturnFocus.current?.focus());
  }

  function chooseArea(nextArea: WorldArea, trigger: HTMLElement) {
    if (nextArea === "assistant") {
      assistantPreviousArea.current = area;
      assistantReturnFocus.current = trigger;
    }
    setArea(nextArea);
  }

  function updateOnboarding(completed: string[], dismissed = onboarding.dismissed) {
    const next = { completed, dismissed };
    setOnboarding(next);
    try {
      localStorage.setItem(`nirmata.onboarding.${worldId}`, JSON.stringify(next));
    } catch {
      // The guide remains usable for the current session in hardened webviews.
    }
  }

  function showOnboarding() {
    setArea("home");
    updateOnboarding(onboarding.completed, false);
    setDialog(null);
  }

  function openBackups() {
    setImportIntent({ id: Date.now(), tab: "snapshot" });
    setArea("imports");
  }

  async function cancelAiActivity() {
    if (!aiActivity) return;
    try {
      await invoke("cancel_ai_request", { requestId: aiActivity.requestId });
    } catch (value) {
      showError(value);
    }
  }

  function runOnboardingAction(item: typeof onboardingItems[number]) {
    const completed = onboarding.completed.includes(item.id)
      ? onboarding.completed
      : [...onboarding.completed, item.id];
    if (item.id !== "applied") updateOnboarding(completed);
    if (item.id === "premise") void configureCalendar();
    if (item.id === "rules") void startCreate("rule");
    if (item.id === "entities") void startCreate("entity");
    if (item.id === "events") void startCreate("event");
    if (item.id === "query") openAssistant("query");
    if (item.id === "propose") openAssistant("propose");
  }

  const currentArea = areas.find((item) => item.id === area)!;
  const firstChangeApplied = (revisionHistory.data?.revisions ?? [])
    .some((revision) => revision.operations.length > 0);
  const onboardingCompleted = Array.from(new Set([
    ...onboarding.completed,
    ...(firstChangeApplied ? ["applied"] : []),
  ]));
  const paletteActions: PaletteAction[] = [
    ...areas.map((item): PaletteAction => ({ id: `area-${item.id}`, label: `Ir a ${item.label}`, group: "Navegar", keywords: [item.label], run: () => setArea(item.id) })),
    { id: "search", label: "Buscar en el canon", group: "Trabajar", keywords: ["objeto", "nombre", "alias"], run: openSearch },
    { id: "ask", label: "Preguntar al asistente", group: "Trabajar", run: () => openAssistant("query") },
    { id: "propose", label: session.read_only ? "Proponer cambios (solo lectura)" : "Proponer cambios", group: "Trabajar", disabled: session.read_only, run: () => openAssistant("propose") },
    { id: "review", label: `Ver cambios pendientes (${pendingCount})`, group: "Trabajar", run: () => openReviews(null) },
    { id: "configure-calendar", label: session.read_only ? "Configurar calendario (solo lectura)" : "Configurar calendario", group: "Trabajar", keywords: ["mundo", "fecha", "meses"], disabled: session.read_only, run: configureCalendar },
    ...createActions.map(({ kind, label }): PaletteAction => ({
      id: `create-${kind}`,
      label: session.read_only ? `${label} (solo lectura)` : label,
      group: "Trabajar",
      disabled: session.read_only,
      keywords: ["nuevo", kind],
      run: () => startCreate(kind),
    })),
    ...(variants.data ?? [])
      .filter((variant) => !variant.archived && variant.id !== session.active_variant.id)
      .map((variant): PaletteAction => ({
        id: `variant-${variant.id}`,
        label: `Escribir en ${variant.name}`,
        group: "Trabajar",
        keywords: ["variante", "versión"],
        run: () => void switchVariant(variant.id),
      })),
    ...(session.read_only ? [{
      id: "revision-current",
      label: "Volver a la versión actual",
      group: "Trabajar" as const,
      keywords: ["versión", "editar"],
      run: () => void viewRevision(null),
    }] : []),
    ...(revisionHistory.data?.revisions ?? []).slice(0, 8).map((revision): PaletteAction => ({
      id: `revision-${revision.revisionId}`,
      label: `Ver versión: ${revision.summary}`,
      group: "Trabajar",
      keywords: ["historia", "solo lectura"],
      run: () => void viewRevision(revision.revisionId),
    })),
    { id: "settings", label: "Abrir Settings", group: "Aplicación", run: () => setDialog("settings") },
    { id: "help", label: "Abrir Centro de ayuda", group: "Aplicación", run: () => setDialog("help") },
    { id: "close", label: "Cerrar mundo", group: "Aplicación", run: onClose },
  ];
  return (
    <>
      <section id="ai-busy-banner" className="ai-busy-banner" role="status" aria-live="polite" hidden={!aiActivity}>
        <div><strong>IA trabajando</strong><p id="ai-busy-message">{aiActivity?.label} Las acciones que cambian o recargan el mundo están pausadas.</p></div>
        <button id="ai-busy-cancel" type="button" className="secondary" disabled={!aiActivity} onClick={() => void cancelAiActivity()}>Cancelar solicitud</button>
      </section>
      <fieldset className="world-busy-boundary" disabled={Boolean(aiActivity)}>
      <div className={`open-shell${collapsed ? " navigation-collapsed" : ""}`}>
      <header className="open-shell-topbar">
        <div className="project-identity">
          <p className="eyebrow">Proyecto</p>
          <strong id="world-name">{session.world.name}</strong>
        </div>
        <dl className="scope-summary">
          <div><dt>Escribiendo en</dt><dd>{session.active_variant.name}</dd></div>
          <div><dt>Viendo</dt><dd>{session.read_only ? "Versión anterior" : "Versión actual"}</dd></div>
        </dl>
        {session.read_only && <span className="read-only-chip">Solo lectura</span>}
        <div className="open-shell-actions">
          <button type="button" className="topbar-search" onClick={(event) => openPalette(event.currentTarget)}>Buscar <kbd>Ctrl K</kbd></button>
          <button type="button" className="ghost" aria-expanded={reviewOpen} onClick={(event) => openReviews(event.currentTarget)}>Cambios <span className="count-badge" aria-label={`${pendingCount} cambios pendientes`}>{pendingCount}</span></button>
          <button type="button" className="ghost" disabled={session.read_only} title={session.read_only ? "El calendario histórico es de solo lectura." : "Editar el calendario del mundo actual"} onClick={configureCalendar}>Calendario</button>
          <button type="button" className="ghost" onClick={(event) => openDialog("settings", event.currentTarget)}>Settings</button>
          <button type="button" className="ghost" onClick={(event) => openDialog("help", event.currentTarget)}>Ayuda</button>
          <button type="button" className="ghost" onClick={onClose}>Cerrar</button>
        </div>
      </header>
      <nav className="open-shell-sidebar" aria-label="Áreas del mundo">
        <button type="button" className="sidebar-collapse ghost" aria-expanded={!collapsed} aria-label={collapsed ? "Expandir navegación" : "Contraer navegación"} onClick={() => setCollapsed((value) => !value)}>
          {collapsed ? ">" : "<"}
        </button>
        {areas.map((item) => (
          <button key={item.id} type="button" aria-current={item.id === area ? "page" : undefined} aria-label={collapsed ? item.label : undefined} title={item.label} onClick={(event) => chooseArea(item.id, event.currentTarget)}>
            <span className="area-mark" aria-hidden="true">{item.label.slice(0, 1)}</span>
            <span className="area-label">{item.label}</span>
          </button>
        ))}
        <button type="button" title="Settings" onClick={(event) => openDialog("settings", event.currentTarget)}>
          <span className="area-mark" aria-hidden="true">S</span>
          <span className="area-label">Settings</span>
        </button>
      </nav>
      <section className="open-world-home" hidden={area !== "home"} aria-labelledby="world-home-title">
        <p className="panel-eyebrow">Inicio del mundo</p>
        <h1 id="world-home-title" ref={areaHeading} tabIndex={-1}>{session.world.name}</h1>
        <p className="world-home-premise">{session.world.premise_md || "Este mundo todavía no tiene una premisa. Empieza por definir sus compromisos fundamentales."}</p>
        <div className="world-home-actions">
          <button type="button" onClick={() => setArea("world")}>Explorar el mundo</button>
          <button type="button" className="secondary" onClick={openSearch}>Buscar en el canon</button>
          <button type="button" className="ghost" onClick={() => setArea("imports")}>Importar material</button>
        </div>
        {!onboarding.dismissed && (
          <section className="world-home-guide" aria-labelledby="world-guide-title">
            <div className="world-guide-heading">
              <div><p className="panel-eyebrow">Guía local · no es canon</p><h2 id="world-guide-title">Construye una base coherente</h2></div>
              <button type="button" className="ghost" onClick={() => updateOnboarding(onboarding.completed, true)}>Ocultar guía</button>
            </div>
            <p>{onboardingCompleted.length} de {onboardingItems.length} pasos completados. El último se verifica desde el historial editorial.</p>
            <ol className="onboarding-list">
              {onboardingItems.map((item) => (
                <li key={item.id}>
                  <span className="onboarding-step-status" aria-label={onboardingCompleted.includes(item.id) ? "Completado" : "Pendiente"}>{onboardingCompleted.includes(item.id) ? "Hecho" : "Pendiente"}</span>
                  <span>{item.label}</span>
                  <button type="button" className="ghost" disabled={item.id === "applied" || session.read_only} onClick={() => runOnboardingAction(item)}>{item.action}</button>
                </li>
              ))}
            </ol>
          </section>
        )}
      </section>
      {area === "chronology" && <WorldTimeline onOpen={openTimelineEvent} onConfigureCalendar={configureCalendar} onCreateEvent={() => startCreate("event")} onUseTemplate={(template) => openAssistant("propose", "", template)} />}
      <Suspense fallback={<p className="area-loading" role="status">Cargando herramienta…</p>}>
        {area === "versions" && <VersionsWorkspace />}
        {mountedAreas.current.has("simulation") && <SimulationWorkspace active={area === "simulation"} onOpenReviews={() => openReviews(null)} />}
        {mountedAreas.current.has("narrative") && <NarrativeWorkspace active={area === "narrative"} onOpen={(uri, scope) => void openNarrativeObject(uri, scope)} onOpenReviews={() => openReviews(null)} onPendingReviewsChanged={() => void queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) })} />}
        {mountedAreas.current.has("assistant") && <AssistantWorkspace active={area === "assistant"} intent={assistantIntent} onClose={closeAssistant} />}
      </Suspense>
      <PendingReviews
        open={reviewOpen}
        onClose={() => closeReviews()}
        onCountChange={setPendingCount}
        onStartProposal={() => openAssistant("propose")}
        onOpenImports={() => setArea("imports")}
        onOpenWorld={() => setArea("world")}
        onEdit={(record, operation) => {
          closeReviews(false);
          setArea("world");
          void openReviewOperationEditor(record, operation);
        }}
      />
      <p className="area-announcement" aria-live="polite">Área actual: {currentArea.label}</p>
      <section id="world-view" aria-labelledby="world-name" aria-busy={Boolean(aiActivity)} hidden={area !== "world" && area !== "imports"}>
        <div className="world-header" hidden>
          <p id="world-path">{session.path}</p>
          <p id="world-premise">{session.world.premise_md || "No especificada"}</p>
          <p id="world-epoch">{session.world.epoch_label || "No especificado"}</p>
        </div>
        <span id="world-revision" hidden>{session.read_only ? "Versión anterior" : "Versión actual"}</span>
        <section id="imports-panel" className="imports-panel" aria-label="Importaciones" hidden={area !== "imports"}>
          <div id="import-center-react-root"><Suspense fallback={<p role="status">Cargando importaciones…</p>}>{mountedAreas.current.has("imports") && <ImportCenter active={area === "imports"} intent={importIntent} onOpenReviews={() => openReviews(null)} />}</Suspense></div>
        </section>
        <div id="workspace-shell" ref={workspaceRef} className="workspace-shell" hidden={area !== "world"}>
          <div className="workspace-region-tabs" role="tablist" aria-label="Región de Mundo">
            {workspaceRegions.map((region) => (
              <button
                key={region.id}
                id={`workspace-region-${region.id}`}
                type="button"
                role="tab"
                aria-selected={workspaceRegion === region.id}
                aria-controls={region.panelId}
                tabIndex={workspaceRegion === region.id ? 0 : -1}
                onClick={() => setWorkspaceRegion(region.id)}
                onKeyDown={(event) => {
                  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
                  event.preventDefault();
                  const current = workspaceRegions.findIndex((item) => item.id === workspaceRegion);
                   const next = event.key === "Home" ? 0
                    : event.key === "End" ? workspaceRegions.length - 1
                      : (current + (event.key === "ArrowRight" ? 1 : -1) + workspaceRegions.length) % workspaceRegions.length;
                   setWorkspaceRegion(workspaceRegions[next].id);
                 }}
               >{region.label}</button>
             ))}
           </div>
          <section
            id="navigation-panel"
            className="panel"
            hidden={narrowWorkspace ? workspaceRegion !== "explorer" : workspaceLayout.explorerCollapsed}
            role={narrowWorkspace ? "tabpanel" : undefined}
            aria-labelledby={narrowWorkspace ? "workspace-region-explorer" : "explorer-title"}
          >
            <div id="explorer-react-root"><WorldExplorer onStartProposal={() => openAssistant("propose")} onEditorOpened={() => { setArea("world"); if (narrowWorkspace) setWorkspaceRegion("editor"); }} /></div>
          </section>
          <section
            id="editor-panel"
            className="panel"
            aria-labelledby={narrowWorkspace ? "workspace-region-editor" : "editor-title"}
            hidden={narrowWorkspace && workspaceRegion !== "editor"}
            role={narrowWorkspace ? "tabpanel" : undefined}
          >
             <div id="structured-editor-react-root"><Suspense fallback={<p role="status">Cargando editor…</p>}>{mountedAreas.current.has("world") && <StructuredEditor onPendingReviewsChanged={() => void queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) })} onStartProposal={(request) => openAssistant("propose", request)} />}</Suspense></div>
          </section>
          <section
            id="context-panel"
            className="panel"
            aria-labelledby={narrowWorkspace ? "workspace-region-context" : "context-title"}
            hidden={narrowWorkspace ? workspaceRegion !== "context" : workspaceLayout.contextCollapsed}
            role={narrowWorkspace ? "tabpanel" : undefined}
          >
            <div id="context-react-root"><WorldContext /></div>
          </section>
          {!narrowWorkspace && (
            <>
              <div className="workspace-splitter workspace-splitter-explorer">
                <div
                  role="separator"
                  tabIndex={0}
                  aria-label="Redimensionar Explorador"
                  aria-orientation="vertical"
                  aria-controls="navigation-panel"
                  aria-valuemin={0}
                  aria-valuemax={workspaceMetrics.explorerMax}
                  aria-valuenow={workspaceMetrics.explorerWidth}
                  aria-valuetext={workspaceLayout.explorerCollapsed ? "Explorador colapsado" : `${workspaceMetrics.explorerWidth} píxeles`}
                  onKeyDown={(event) => resizeWithKeyboard("explorer", event)}
                  onPointerDown={(event) => startPointerResize("explorer", event)}
                  onPointerMove={movePointerResize}
                  onPointerUp={finishPointerResize}
                  onPointerCancel={finishPointerResize}
                />
                <button
                  type="button"
                  className="workspace-splitter-action"
                  aria-controls="navigation-panel"
                  aria-expanded={!workspaceLayout.explorerCollapsed}
                  aria-label={workspaceLayout.explorerCollapsed ? "Restaurar Explorador" : "Colapsar Explorador"}
                  title={workspaceLayout.explorerCollapsed ? "Restaurar Explorador" : "Colapsar Explorador"}
                  onClick={() => toggleWorkspacePanel("explorer")}
                >{workspaceLayout.explorerCollapsed ? ">" : "<"}</button>
              </div>
              <div className="workspace-splitter workspace-splitter-context">
                <div
                  role="separator"
                  tabIndex={0}
                  aria-label="Redimensionar Contexto"
                  aria-orientation="vertical"
                  aria-controls="context-panel"
                  aria-valuemin={0}
                  aria-valuemax={workspaceMetrics.contextMax}
                  aria-valuenow={workspaceMetrics.contextWidth}
                  aria-valuetext={workspaceLayout.contextCollapsed ? "Contexto colapsado" : `${workspaceMetrics.contextWidth} píxeles`}
                  onKeyDown={(event) => resizeWithKeyboard("context", event)}
                  onPointerDown={(event) => startPointerResize("context", event)}
                  onPointerMove={movePointerResize}
                  onPointerUp={finishPointerResize}
                  onPointerCancel={finishPointerResize}
                />
                <button
                  type="button"
                  className="workspace-splitter-action"
                  aria-controls="context-panel"
                  aria-expanded={!workspaceLayout.contextCollapsed}
                  aria-label={workspaceLayout.contextCollapsed ? "Restaurar Contexto" : "Colapsar Contexto"}
                  title={workspaceLayout.contextCollapsed ? "Restaurar Contexto" : "Colapsar Contexto"}
                  onClick={() => toggleWorkspacePanel("context")}
                >{workspaceLayout.contextCollapsed ? "<" : ">"}</button>
              </div>
            </>
          )}
        </div>
      </section>
      <CommandPalette
        open={paletteOpen}
        onOpenChange={(open) => {
          if (open && !paletteTrigger) setPaletteTrigger(document.activeElement as HTMLElement | null);
          setPaletteOpen(open);
          if (!open) setPaletteQuery("");
        }}
        actions={paletteActions}
        returnFocus={paletteTrigger}
        query={paletteQuery}
        onQueryChange={setPaletteQuery}
        results={paletteResults.data?.hits ?? []}
        searching={paletteResults.isFetching}
        searchError={paletteResults.isError}
        onSelectResult={openPaletteResult}
      />
      <SoftwareDialogs active={dialog} onActiveChange={setDialog} returnFocus={dialogTrigger} onOpenBackups={openBackups} onShowOnboarding={showOnboarding} />
      <Dialog.Root open={confirmation !== null} onOpenChange={(open) => {
        if (!open && confirmation) {
          confirmation.resolve(false);
          setConfirmation(null);
        }
      }}>
        <Dialog.Portal>
          <Dialog.Overlay className="dialog-overlay" />
          <Dialog.Content className="confirmation-dialog" aria-describedby="confirmation-detail">
            <Dialog.Title>{confirmation?.title}</Dialog.Title>
            <Dialog.Description id="confirmation-detail">{confirmation?.detail}</Dialog.Description>
            <div className="dialog-actions">
              <button type="button" className={confirmation?.danger ? "danger" : undefined} onClick={() => {
                confirmation?.resolve(true);
                setConfirmation(null);
              }}>{confirmation?.confirmLabel}</button>
              <Dialog.Close asChild><button type="button" className="secondary">Cancelar</button></Dialog.Close>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      </div>
      </fieldset>
    </>
  );
}
