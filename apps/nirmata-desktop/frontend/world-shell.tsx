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
import { Icon } from "./icons.js";
import type { IconName } from "./icons.js";
import { buttonStyles, chipStyles, cn } from "./ui-styles.js";

const AssistantWorkspace = lazy(() => import("./assistant-workspace.js").then((module) => ({ default: module.AssistantWorkspace })));
const ImportCenter = lazy(() => import("./import-center.js").then((module) => ({ default: module.ImportCenter })));
const NarrativeWorkspace = lazy(() => import("./narrative-workspace.js").then((module) => ({ default: module.NarrativeWorkspace })));
const SimulationWorkspace = lazy(() => import("./simulation-workspace.js").then((module) => ({ default: module.SimulationWorkspace })));
const StructuredEditor = lazy(() => import("./structured-editor.js").then((module) => ({ default: module.StructuredEditor })));

type WorldArea = "home" | "world" | "chronology" | "narrative" | "simulation" | "imports" | "versions";
type NavigationArea = WorldArea | "assistant";
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
  { id: "explorer", label: "Explorar", panelId: "navigation-panel" },
  { id: "editor", label: "Editar", panelId: "editor-panel" },
  { id: "context", label: "Contexto", panelId: "context-panel" },
];

const areas: Array<{ id: NavigationArea; label: string; icon: IconName }> = [
  { id: "home", label: "Inicio", icon: "home" },
  { id: "world", label: "Mundo", icon: "globe" },
  { id: "chronology", label: "Cronología", icon: "clock" },
  { id: "assistant", label: "Asistente", icon: "sparkles" },
  { id: "narrative", label: "Estudio narrativo", icon: "book" },
  { id: "simulation", label: "Simulación", icon: "flask" },
  { id: "imports", label: "Importaciones", icon: "download" },
  { id: "versions", label: "Versiones", icon: "history" },
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

function readOnboarding(worldId: string): { dismissed: boolean } {
  try {
    const stored = JSON.parse(localStorage.getItem(`nirmata.onboarding.${worldId}`) ?? "{}") as { dismissed?: boolean };
    return { dismissed: Boolean(stored.dismissed) };
  } catch {
    return { dismissed: false };
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
  const [narrowWorkspace, setNarrowWorkspace] = useState(() => window.matchMedia("(max-width: 1180px)").matches);
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
  const [settingsInitialTab, setSettingsInitialTab] = useState<"general" | "ai">("general");
  const [dialogTrigger, setDialogTrigger] = useState<HTMLElement | null>(null);
  const [pendingCount, setPendingCount] = useState(0);
  const [reviewOpen, setReviewOpen] = useState(false);
  const reviewReturnFocus = useRef<HTMLElement | null>(null);
  const [confirmation, setConfirmation] = useState<(ConfirmationRequest & { resolve: (accepted: boolean) => void }) | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [paletteTrigger, setPaletteTrigger] = useState<HTMLElement | null>(null);
  const [onboarding, setOnboarding] = useState(() => session
    ? readOnboarding(session.world_id)
    : { dismissed: false });
  const areaHeading = useRef<HTMLHeadingElement>(null);
  const homeRef = useRef<HTMLElement>(null);
  const initialArea = useRef(true);
  const assistantReturnFocus = useRef<HTMLElement | null>(null);
  const [assistantOpen, setAssistantOpen] = useState(false);
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
    document.body.classList.add("world-session-open", "h-dvh", "overflow-hidden");
    return () => {
      document.body.classList.remove("world-session-open", "h-dvh", "overflow-hidden");
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
    if (initialArea.current) {
      initialArea.current = false;
    } else if (area === "home") {
      if (homeRef.current) {
        homeRef.current.scrollTop = 0;
        homeRef.current.scrollLeft = 0;
      }
      areaHeading.current?.focus({ preventScroll: true });
    }
  }, [area, session]);

  useEffect(() => {
    const media = window.matchMedia("(max-width: 1180px)");
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
      case "settings.open": setSettingsInitialTab("general"); setDialog("settings"); break;
      case "help.open": setDialog("help"); break;
      case "help.about": setDialog("about"); break;
      case "help.onboarding": showOnboarding(); break;
    }
  }, [desktopAction?.id, session?.world_id]);

  useEffect(() => {
    if (!session) return;
    setOnboarding(readOnboarding(session.world_id));
  }, [session]);

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
    trigger.closest("details")?.removeAttribute("open");
    if (nextDialog === "settings") setSettingsInitialTab("general");
    setDialogTrigger(trigger);
    setDialog(nextDialog);
  }

  function openReviews(trigger: HTMLElement | null) {
    reviewReturnFocus.current = trigger ?? document.activeElement as HTMLElement | null;
    setReviewOpen(true);
  }

  function closeReviews(restoreFocus = true) {
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
    assistantReturnFocus.current = document.activeElement as HTMLElement | null;
    assistantIntentId.current += 1;
    setAssistantIntent({ id: assistantIntentId.current, mode, request, template });
    setAssistantOpen(true);
  }

  function closeAssistant() {
    setAssistantOpen(false);
    window.setTimeout(() => assistantReturnFocus.current?.focus());
  }

  function openAssistantSettings() {
    setAssistantOpen(false);
    setSettingsInitialTab("ai");
    setDialogTrigger(assistantReturnFocus.current);
    setDialog("settings");
  }

  function openAssistantReviews() {
    setAssistantOpen(false);
    openReviews(null);
  }

  function chooseArea(nextArea: NavigationArea, trigger: HTMLElement) {
    if (nextArea === "assistant") {
      openAssistant("query");
      assistantReturnFocus.current = trigger;
      return;
    }
    setArea(nextArea);
  }

  function updateOnboarding(dismissed: boolean) {
    const next = { dismissed };
    setOnboarding(next);
    try {
      localStorage.setItem(`nirmata.onboarding.${worldId}`, JSON.stringify(next));
    } catch {
      // The guide remains usable for the current session in hardened webviews.
    }
  }

  function showOnboarding() {
    setArea("home");
    updateOnboarding(false);
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

  const currentArea = areas.find((item) => item.id === area)!;
  const premise = session.world.premise_md
    .replace(/[*_`#>~]+/gu, "")
    .replace(/\s+/gu, " ")
    .trim() || "Este mundo todavía no tiene una premisa. Define en una frase qué lo hace único.";
  const paletteActions: PaletteAction[] = [
    ...areas.filter((item) => item.id !== "assistant").map((item): PaletteAction => ({ id: `area-${item.id}`, label: `Ir a ${item.label}`, group: "Navegar", keywords: [item.label], run: () => setArea(item.id as WorldArea) })),
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
    { id: "settings", label: "Abrir Ajustes", group: "Aplicación", run: () => { setSettingsInitialTab("general"); setDialog("settings"); } },
    { id: "help", label: "Abrir Centro de ayuda", group: "Aplicación", run: () => setDialog("help") },
    { id: "close", label: "Cerrar mundo", group: "Aplicación", run: onClose },
  ];
  return (
    <>
      <section id="ai-busy-banner" className="ai-busy-banner fixed inset-x-0 top-0 z-[100] flex min-h-16 items-center justify-between gap-4 border-b border-warning bg-warning-soft px-5 py-3" role="status" aria-live="polite" hidden={!aiActivity}>
        <div><strong>IA trabajando</strong><p id="ai-busy-message">{aiActivity?.label} Las acciones que cambian o recargan el mundo están pausadas.</p></div>
        <button id="ai-busy-cancel" type="button" className={buttonStyles({ variant: "secondary" })} disabled={!aiActivity} onClick={() => void cancelAiActivity()}>Cancelar solicitud</button>
      </section>
      <fieldset className="world-busy-boundary contents" disabled={Boolean(aiActivity)}>
      <div className={cn(
        "open-shell grid h-dvh w-full grid-cols-[14.5rem_minmax(0,1fr)] grid-rows-[4rem_minmax(0,1fr)] overflow-hidden bg-canvas max-mobile:grid-cols-[minmax(0,1fr)] max-mobile:grid-rows-[auto_auto_minmax(0,1fr)]",
        collapsed && "navigation-collapsed grid-cols-[4.5rem_minmax(0,1fr)] max-mobile:grid-cols-[minmax(0,1fr)]",
      )}>
      <header className="open-shell-topbar z-20 col-span-full row-start-1 grid grid-cols-[minmax(10rem,auto)_minmax(16rem,1fr)_auto_auto] items-center gap-6 border-b border-line bg-surface px-4 max-mobile:col-start-1 max-mobile:row-start-1 max-mobile:flex max-mobile:min-h-14 max-mobile:flex-wrap max-mobile:gap-2 max-mobile:px-3 max-mobile:py-2">
        <div className="project-identity min-w-0 max-mobile:mr-auto">
          <p className="eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Proyecto</p>
          <strong id="world-name" className="block truncate font-serif text-base">{session.world.name}</strong>
        </div>
        <dl className="scope-summary flex min-w-0 gap-6 max-mobile:order-3 max-mobile:w-full max-mobile:border-t max-mobile:border-line max-mobile:pt-2 max-compact:gap-3">
          <div className="min-w-0"><dt className="text-[0.62rem] font-bold uppercase tracking-wider text-muted">Escribiendo en</dt><dd className="truncate text-xs font-semibold">{session.active_variant.name}</dd></div>
          <div className="min-w-0"><dt className="text-[0.62rem] font-bold uppercase tracking-wider text-muted">Viendo</dt><dd className="truncate text-xs font-semibold">{session.read_only ? "Versión anterior" : "Versión actual"}</dd></div>
        </dl>
        {session.read_only && <span className={chipStyles({ kind: "readOnly" })}>Solo lectura</span>}
        <div className="open-shell-actions flex items-center justify-end gap-2 max-mobile:ml-auto">
          <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "topbar-search min-h-9 max-mobile:size-9 max-mobile:min-h-9 max-mobile:rounded-full max-mobile:p-0")} onClick={(event) => openPalette(event.currentTarget)}><Icon name="search" /> <span className="max-mobile:hidden">Buscar</span> <kbd className="ml-2 text-[0.65rem] font-normal text-muted max-mobile:hidden">Ctrl K</kbd></button>
          <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "topbar-changes min-h-9 max-mobile:size-9 max-mobile:min-h-9 max-mobile:rounded-full max-mobile:p-0")} aria-label={`Cambios ${pendingCount} cambios pendientes`} aria-expanded={reviewOpen} onClick={(event) => openReviews(event.currentTarget)}><Icon name="layers" /> <span className="max-mobile:hidden">Cambios</span> <span className={chipStyles({ kind: "count" })} aria-hidden="true">{pendingCount}</span></button>
          <details className="topbar-more group relative">
            <summary className="flex size-9 list-none items-center justify-center rounded-full border border-line group-open:bg-subtle" aria-label="Más acciones" title="Más acciones"><Icon name="more" /><span className="sr-only">Más</span></summary>
            <div className="absolute right-0 top-11 z-50 grid min-w-48 gap-1 rounded-xl border border-line bg-raised p-2 shadow-overlay">
              <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "justify-start border-0")} disabled={session.read_only} title={session.read_only ? "El calendario histórico es de solo lectura." : "Editar el calendario del mundo actual"} onClick={(event) => { event.currentTarget.closest("details")?.removeAttribute("open"); void configureCalendar(); }}>Editar calendario</button>
              <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "justify-start border-0")} onClick={(event) => openDialog("settings", event.currentTarget)}>Ajustes</button>
              <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "justify-start border-0")} onClick={(event) => openDialog("help", event.currentTarget)}>Ayuda</button>
              <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "justify-start border-0")} onClick={(event) => { event.currentTarget.closest("details")?.removeAttribute("open"); onClose(); }}>Cerrar mundo</button>
            </div>
          </details>
        </div>
      </header>
      <nav className="open-shell-sidebar z-10 col-start-1 row-start-2 flex min-h-0 flex-col gap-1 border-r border-line bg-surface px-2 py-3 max-mobile:row-start-2 max-mobile:flex-row max-mobile:overflow-x-auto max-mobile:border-r-0 max-mobile:border-b max-mobile:px-2 max-mobile:py-1.5" aria-label="Áreas del mundo">
        <button type="button" className={cn(buttonStyles({ variant: "ghost" }), "sidebar-collapse mb-2 ml-auto max-mobile:hidden")} aria-expanded={!collapsed} aria-label={collapsed ? "Expandir navegación" : "Contraer navegación"} onClick={() => setCollapsed((value) => !value)}>
          <Icon name={collapsed ? "chevron-right" : "chevron-left"} />
        </button>
        {areas.map((item) => (
          <button key={item.id} type="button" className={cn(
            "min-h-11 justify-start border-0 bg-transparent px-3 text-muted enabled:hover:bg-subtle enabled:hover:text-ink aria-[current=page]:bg-accent-soft aria-[current=page]:text-accent max-mobile:min-h-10 max-mobile:shrink-0 max-mobile:justify-center max-mobile:px-3 max-compact:size-10 max-compact:px-0",
            collapsed && "justify-center px-0 max-mobile:px-3 max-compact:px-0",
          )} aria-current={item.id !== "assistant" && item.id === area ? "page" : undefined} aria-haspopup={item.id === "assistant" ? "dialog" : undefined} aria-expanded={item.id === "assistant" ? assistantOpen : undefined} aria-label={collapsed ? item.label : undefined} title={item.label} onClick={(event) => chooseArea(item.id, event.currentTarget)}>
            <span className="area-mark flex size-6 shrink-0 items-center justify-center" aria-hidden="true"><Icon name={item.icon} /></span>
            <span className={cn("area-label min-w-0 truncate max-compact:hidden", collapsed && "hidden max-mobile:inline max-compact:hidden")}>{item.label}</span>
          </button>
        ))}
      </nav>
      <section ref={homeRef} className="open-world-home col-start-2 row-start-2 grid min-h-0 min-w-0 justify-items-start gap-6 overflow-auto p-8 [align-content:safe_center] lg:p-14 max-mobile:col-start-1 max-mobile:row-start-3 max-mobile:content-start max-mobile:p-6" hidden={area !== "home"} aria-labelledby="world-home-title">
        <p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Inicio del mundo</p>
        <h1 id="world-home-title" className="max-w-5xl text-6xl lg:text-7xl max-mobile:text-5xl" ref={areaHeading} tabIndex={-1}>{session.world.name}</h1>
        <p className="world-home-premise max-w-3xl font-serif text-xl leading-relaxed text-muted">{premise}</p>
        <div className="world-home-actions flex flex-wrap gap-2">
          <button type="button" onClick={() => setArea("world")}>Abrir Mundo</button>
          <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={session.read_only} onClick={() => void startCreate("entity")}>Crear entidad</button>
          <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={session.read_only} onClick={() => void startCreate("event")}>Crear evento</button>
          <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => openAssistant("query")}>Preguntar</button>
        </div>
        {!onboarding.dismissed && (
          <section className="world-home-guide grid max-w-3xl gap-2 border-l-2 border-accent bg-surface px-5 py-4" aria-labelledby="world-guide-title">
            <div className="world-guide-heading flex items-start justify-between gap-4">
              <div><p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Flujo básico</p><h2 id="world-guide-title">Tú decides qué entra al mundo</h2></div>
              <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => updateOnboarding(true)}>Ocultar guía</button>
            </div>
            <p>Edita o crea, prepara una propuesta, revísala en <strong>Cambios</strong> y usa <strong>Aplicar al mundo</strong> solo cuando estés conforme.</p>
          </section>
        )}
      </section>
      {area === "chronology" && <WorldTimeline onOpen={openTimelineEvent} onConfigureCalendar={configureCalendar} onCreateEvent={() => startCreate("event")} onUseTemplate={(template) => openAssistant("propose", "", template)} />}
      <Suspense fallback={<p className="area-loading col-start-2 row-start-2 min-h-0 min-w-0 overflow-auto max-mobile:col-start-1 max-mobile:row-start-3" role="status">Cargando herramienta…</p>}>
        {area === "versions" && <VersionsWorkspace />}
        {mountedAreas.current.has("simulation") && <SimulationWorkspace active={area === "simulation"} onOpenReviews={() => openReviews(null)} />}
        {mountedAreas.current.has("narrative") && <NarrativeWorkspace active={area === "narrative"} onOpen={(uri, scope) => void openNarrativeObject(uri, scope)} onOpenReviews={() => openReviews(null)} onPendingReviewsChanged={() => void queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) })} />}
        {assistantIntent && <AssistantWorkspace active={assistantOpen} intent={assistantIntent} onClose={closeAssistant} onOpenSettings={openAssistantSettings} onOpenReviews={openAssistantReviews} />}
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
      <p className="area-announcement sr-only" aria-live="polite">Área actual: {currentArea.label}</p>
      <section id="world-view" className="col-start-2 row-start-2 min-h-0 min-w-0 overflow-hidden bg-canvas max-mobile:col-start-1 max-mobile:row-start-3" aria-labelledby="world-name" aria-busy={Boolean(aiActivity)} hidden={area !== "world" && area !== "imports"}>
        <div hidden>
          <p id="world-path">{session.path}</p>
          <p id="world-premise">{session.world.premise_md || "No especificada"}</p>
          <p id="world-epoch">{session.world.epoch_label || "No especificado"}</p>
        </div>
        <span id="world-revision" hidden>{session.read_only ? "Versión anterior" : "Versión actual"}</span>
        <section id="imports-panel" className="imports-panel h-full overflow-auto p-5" aria-label="Importaciones" hidden={area !== "imports"}>
          <div id="import-center-react-root"><Suspense fallback={<p role="status">Cargando importaciones…</p>}>{mountedAreas.current.has("imports") && <ImportCenter active={area === "imports"} intent={importIntent} onOpenReviews={() => openReviews(null)} />}</Suspense></div>
        </section>
        <div id="workspace-shell" ref={workspaceRef} className="workspace-shell grid h-full min-h-0 min-w-0 grid-cols-[var(--explorer-panel-width,18rem)_0.75rem_minmax(15rem,1fr)_0.75rem_var(--context-panel-width,19rem)] gap-0 p-3 [grid-template-areas:'explorer_split-explorer_editor_split-context_context'] max-workspace:grid-cols-[minmax(0,1fr)] max-workspace:grid-rows-[auto_minmax(0,1fr)] max-workspace:gap-2 max-workspace:[grid-template-areas:'tabs'_'active'] max-mobile:p-2" hidden={area !== "world"}>
          <div className="workspace-region-tabs hidden max-workspace:grid max-workspace:grid-cols-3 max-workspace:gap-1 max-workspace:rounded-xl max-workspace:bg-subtle max-workspace:p-1 max-workspace:[grid-area:tabs]" role="tablist" aria-label="Región de Mundo">
            {workspaceRegions.map((region) => (
              <button
                key={region.id}
                id={`workspace-region-${region.id}`}
                 type="button"
                 className="max-workspace:border-0 max-workspace:bg-transparent max-workspace:text-ink max-workspace:aria-[selected=true]:bg-raised max-workspace:aria-[selected=true]:text-accent max-workspace:aria-[selected=true]:shadow-sm"
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
            className="panel flex min-h-0 min-w-0 flex-col overflow-hidden rounded-2xl border border-line bg-surface shadow-sm [grid-area:explorer] max-workspace:[grid-area:active]"
            hidden={narrowWorkspace ? workspaceRegion !== "explorer" : workspaceLayout.explorerCollapsed}
            role={narrowWorkspace ? "tabpanel" : undefined}
            aria-labelledby={narrowWorkspace ? "workspace-region-explorer" : "explorer-title"}
          >
            <div id="explorer-react-root"><WorldExplorer onStartProposal={() => openAssistant("propose")} onEditorOpened={() => { setArea("world"); if (narrowWorkspace) setWorkspaceRegion("editor"); }} /></div>
          </section>
          <section
            id="editor-panel"
            className="panel flex min-h-0 min-w-0 flex-col overflow-hidden rounded-2xl border border-line bg-surface shadow-sm [grid-area:editor] max-workspace:[grid-area:active]"
            aria-labelledby={narrowWorkspace ? "workspace-region-editor" : "editor-title"}
            hidden={narrowWorkspace && workspaceRegion !== "editor"}
            role={narrowWorkspace ? "tabpanel" : undefined}
          >
             <div id="structured-editor-react-root" className="flex h-full min-h-0 flex-col"><Suspense fallback={<p role="status">Cargando editor…</p>}>{mountedAreas.current.has("world") && <StructuredEditor onPendingReviewsChanged={() => void queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) })} onStartProposal={(request) => openAssistant("propose", request)} />}</Suspense></div>
          </section>
          <section
            id="context-panel"
            className="panel flex min-h-0 min-w-0 flex-col overflow-hidden rounded-2xl border border-line bg-surface shadow-sm [grid-area:context] max-workspace:[grid-area:active]"
            aria-labelledby={narrowWorkspace ? "workspace-region-context" : "context-title"}
            hidden={narrowWorkspace ? workspaceRegion !== "context" : workspaceLayout.contextCollapsed}
            role={narrowWorkspace ? "tabpanel" : undefined}
          >
            <div id="context-react-root"><WorldContext /></div>
          </section>
          {!narrowWorkspace && (
            <>
              <div className="workspace-splitter workspace-splitter-explorer relative flex items-center justify-center [grid-area:split-explorer] max-workspace:hidden">
                <div
                  className="absolute inset-y-0 w-full cursor-col-resize after:absolute after:bottom-4 after:left-1/2 after:top-4 after:w-px after:-translate-x-1/2 after:bg-line after:content-['']"
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
                  className="workspace-splitter-action relative z-10 size-7 min-h-7 rounded-full border-line bg-surface p-0 text-muted shadow-sm"
                  aria-controls="navigation-panel"
                  aria-expanded={!workspaceLayout.explorerCollapsed}
                  aria-label={workspaceLayout.explorerCollapsed ? "Restaurar Explorador" : "Colapsar Explorador"}
                  title={workspaceLayout.explorerCollapsed ? "Restaurar Explorador" : "Colapsar Explorador"}
                  onClick={() => toggleWorkspacePanel("explorer")}
                ><Icon name={workspaceLayout.explorerCollapsed ? "chevron-right" : "chevron-left"} /></button>
              </div>
              <div className="workspace-splitter workspace-splitter-context relative flex items-center justify-center [grid-area:split-context] max-workspace:hidden">
                <div
                  className="absolute inset-y-0 w-full cursor-col-resize after:absolute after:bottom-4 after:left-1/2 after:top-4 after:w-px after:-translate-x-1/2 after:bg-line after:content-['']"
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
                  className="workspace-splitter-action relative z-10 size-7 min-h-7 rounded-full border-line bg-surface p-0 text-muted shadow-sm"
                  aria-controls="context-panel"
                  aria-expanded={!workspaceLayout.contextCollapsed}
                  aria-label={workspaceLayout.contextCollapsed ? "Restaurar Contexto" : "Colapsar Contexto"}
                  title={workspaceLayout.contextCollapsed ? "Restaurar Contexto" : "Colapsar Contexto"}
                  onClick={() => toggleWorkspacePanel("context")}
                ><Icon name={workspaceLayout.contextCollapsed ? "chevron-left" : "chevron-right"} /></button>
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
      <SoftwareDialogs active={dialog} onActiveChange={setDialog} returnFocus={dialogTrigger} onOpenBackups={openBackups} onShowOnboarding={showOnboarding} settingsInitialTab={settingsInitialTab} />
      <Dialog.Root open={confirmation !== null} onOpenChange={(open) => {
        if (!open && confirmation) {
          confirmation.resolve(false);
          setConfirmation(null);
        }
      }}>
        <Dialog.Portal>
          <Dialog.Overlay className="dialog-overlay fixed inset-0 z-40 bg-overlay" />
          <Dialog.Content className="confirmation-dialog fixed left-1/2 top-1/2 z-50 grid max-h-[calc(100dvh-2rem)] w-[min(42rem,calc(100vw-2rem))] max-w-lg -translate-x-1/2 -translate-y-1/2 gap-4 overflow-auto rounded-2xl border border-line bg-raised p-6 shadow-overlay outline-none" aria-describedby="confirmation-detail">
            <Dialog.Title>{confirmation?.title}</Dialog.Title>
            <Dialog.Description id="confirmation-detail">{confirmation?.detail}</Dialog.Description>
            <div className="dialog-actions flex flex-wrap items-center gap-2">
              <button type="button" className={buttonStyles({ variant: confirmation?.danger ? "danger" : "primary" })} onClick={() => {
                confirmation?.resolve(true);
                setConfirmation(null);
              }}>{confirmation?.confirmLabel}</button>
              <Dialog.Close asChild><button type="button" className={buttonStyles({ variant: "secondary" })}>Cancelar</button></Dialog.Close>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      </div>
      </fieldset>
    </>
  );
}
