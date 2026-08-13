import { invoke } from "@tauri-apps/api/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useDeferredValue, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { CommandPalette } from "./command-palette.js";
import type { PaletteAction } from "./command-palette.js";
import { SoftwareDialogs } from "./software-dialogs.js";
import type { SoftwareDialog } from "./software-dialogs.js";
import { useSession } from "./session-provider.js";
import { closeButton, pendingPanel, state } from "./state.js";
import type { RevisionHistorySnapshot, SearchObjectKind, SearchResult, SearchWorldResponse, Variant } from "./types.js";
import { observeRevision, switchWritingVariant, viewActiveVersion } from "./variant-ui.js";
import { selectUri, startCreatingObject } from "./workspace.js";
import { WorldExplorer } from "./world-explorer.js";
import { ObjectPicker } from "./object-picker.js";
import { WorldContext } from "./world-context.js";
import { WorldTimeline } from "./world-timeline.js";
import { ImportCenter } from "./import-center.js";

type WorldArea = "home" | "world" | "chronology" | "assistant" | "narrative" | "simulation" | "imports" | "versions";

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

const areaPanels: Record<WorldArea, string[]> = {
  home: [],
  world: ["workspace-shell"],
  chronology: [],
  assistant: ["assistant-panel"],
  narrative: ["narrative-panel"],
  simulation: ["simulation-panel"],
  imports: ["imports-panel"],
  versions: ["variant-bar"],
};

const managedPanels = [
  "assistant-panel",
  "narrative-panel",
  "imports-panel",
  "simulation-panel",
  "workspace-shell",
  "variant-bar",
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
  { id: "premise", label: "Definir la premisa" },
  { id: "rules", label: "Registrar una regla fundamental" },
  { id: "places", label: "Crear un lugar o facción" },
  { id: "characters", label: "Crear un personaje y su meta" },
  { id: "events", label: "Añadir un acontecimiento" },
  { id: "query", label: "Hacer la primera consulta" },
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

export function WorldShell() {
  const session = useSession();
  const queryClient = useQueryClient();
  const [area, setArea] = useState<WorldArea>("home");
  const [collapsed, setCollapsed] = useState(false);
  const [dialog, setDialog] = useState<SoftwareDialog>(null);
  const [dialogTrigger, setDialogTrigger] = useState<HTMLElement | null>(null);
  const [pendingCount, setPendingCount] = useState(0);
  const [reviewOpen, setReviewOpen] = useState(false);
  const reviewReturnFocus = useRef<HTMLElement | null>(null);
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
  const deferredPaletteQuery = useDeferredValue(paletteQuery.trim());
  const scopeKey = session
    ? [session.world_id, session.active_variant.id, session.read_scope.revisionId ?? session.current_revision]
    : ["closed", "closed", "closed"];
  const paletteResults = useQuery({
    queryKey: ["world", ...scopeKey, "palette-search", deferredPaletteQuery],
    queryFn: () => invoke<SearchWorldResponse>("search_world", {
      input: { queryText: deferredPaletteQuery, kind: "all", limit: 20 },
    }),
    enabled: Boolean(session && paletteOpen && deferredPaletteQuery.length >= 2),
    retry: false,
    staleTime: 0,
  });
  const variants = useQuery({
    queryKey: ["world", ...scopeKey, "variants"],
    queryFn: () => invoke<Variant[]>("list_variants"),
    enabled: Boolean(session && paletteOpen),
    retry: false,
  });
  const revisionHistory = useQuery({
    queryKey: ["world", ...scopeKey, "revision-history"],
    queryFn: () => invoke<RevisionHistorySnapshot>("list_revision_history"),
    enabled: Boolean(session && paletteOpen),
    retry: false,
  });

  useEffect(() => {
    if (!session) return;
    document.body.classList.add("world-session-open");
    return () => {
      document.body.classList.remove("world-session-open");
      delete document.body.dataset.worldArea;
    };
  }, [session]);

  useEffect(() => {
    const worldId = session?.world_id;
    return () => {
      if (worldId) queryClient.removeQueries({ queryKey: ["world", worldId] });
    };
  }, [queryClient, session?.world_id]);

  useEffect(() => {
    if (!session) return;
    document.body.dataset.worldArea = area;
    for (const panelId of managedPanels) {
      const panel = document.getElementById(panelId);
      if (panel) panel.hidden = !areaPanels[area].includes(panelId);
    }
    document.querySelector<HTMLElement>(".world-header")!.hidden = true;
    document.querySelector<HTMLElement>(".layout-toolbar")!.hidden = true;
    if (initialArea.current) {
      initialArea.current = false;
    } else {
      const target = area === "home"
        ? areaHeading.current
        : document.getElementById(areaPanels[area][0] ?? "")?.querySelector<HTMLElement>("h2, h3");
      if (target) {
        target.tabIndex = -1;
        target.focus();
      }
    }
  }, [area, session]);

  useEffect(() => {
    function syncPending() {
      setPendingCount(state.pendingDrafts.size);
    }
    syncPending();
    window.addEventListener("nirmata:pending-changed", syncPending);
    return () => window.removeEventListener("nirmata:pending-changed", syncPending);
  }, []);

  useEffect(() => {
    if (!session) return;
    setOnboarding(readOnboarding(session.world_id, Boolean(session.world.premise_md.trim())));
  }, [session]);

  useEffect(() => {
    function showOnboarding() {
      setArea("home");
      setOnboarding((value) => ({ ...value, dismissed: false }));
      setDialog(null);
    }
    window.addEventListener("nirmata:show-onboarding", showOnboarding);
    return () => window.removeEventListener("nirmata:show-onboarding", showOnboarding);
  }, []);

  useEffect(() => {
    function openRequestedArea(event: Event) {
      const requested = (event as CustomEvent<{ area?: WorldArea }>).detail?.area;
      if (requested && areas.some((item) => item.id === requested)) setArea(requested);
    }
    window.addEventListener("nirmata:open-area", openRequestedArea);
    return () => window.removeEventListener("nirmata:open-area", openRequestedArea);
  }, []);

  if (!session) return null;
  const worldId = session.world_id;

  function openDialog(nextDialog: Exclude<SoftwareDialog, null>, trigger: HTMLElement) {
    setDialogTrigger(trigger);
    setDialog(nextDialog);
  }

  function openReviews(trigger: HTMLElement | null) {
    reviewReturnFocus.current = trigger ?? document.activeElement as HTMLElement | null;
    document.body.classList.add("review-drawer-open");
    pendingPanel.hidden = false;
    setReviewOpen(true);
  }

  function closeReviews() {
    document.body.classList.remove("review-drawer-open");
    pendingPanel.hidden = true;
    setReviewOpen(false);
    window.setTimeout(() => reviewReturnFocus.current?.focus());
  }

  useEffect(() => () => document.body.classList.remove("review-drawer-open"), []);

  function openSearch() {
    setArea("world");
    window.setTimeout(() => document.querySelector<HTMLInputElement>(".world-explorer-search")?.focus());
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

  function startCreate(kind: SearchObjectKind) {
    if (startCreatingObject(kind)) setArea("world");
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

  function openAssistant(mode: "query" | "propose") {
    assistantPreviousArea.current = area === "assistant" ? "home" : area;
    assistantReturnFocus.current = document.activeElement as HTMLElement | null;
    setArea("assistant");
    if (mode === "propose") {
      window.setTimeout(() => window.dispatchEvent(new CustomEvent("nirmata:start-proposal")));
    } else {
      window.setTimeout(() => document.querySelector<HTMLTextAreaElement>("#assistant-input")?.focus());
    }
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

  const currentArea = areas.find((item) => item.id === area)!;
  const explorerHost = document.querySelector<HTMLElement>("#explorer-react-root");
  const contextHost = document.querySelector<HTMLElement>("#context-react-root");
  const importCenterHost = document.querySelector<HTMLElement>("#import-center-react-root");
  const paletteActions: PaletteAction[] = [
    ...areas.map((item): PaletteAction => ({ id: `area-${item.id}`, label: `Ir a ${item.label}`, group: "Navegar", keywords: [item.label], run: () => setArea(item.id) })),
    { id: "search", label: "Buscar en el canon", group: "Trabajar", keywords: ["objeto", "nombre", "alias"], run: openSearch },
    { id: "ask", label: "Preguntar al asistente", group: "Trabajar", run: () => openAssistant("query") },
    { id: "propose", label: session.read_only ? "Proponer cambios (solo lectura)" : "Proponer cambios", group: "Trabajar", disabled: session.read_only, run: () => openAssistant("propose") },
    { id: "review", label: `Ver cambios pendientes (${pendingCount})`, group: "Trabajar", run: () => openReviews(null) },
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
    { id: "close", label: "Cerrar mundo", group: "Aplicación", run: () => closeButton.click() },
  ];
  return (
    <div className={`open-shell${collapsed ? " navigation-collapsed" : ""}`}>
      <header className="open-shell-topbar">
        <div className="project-identity">
          <p className="eyebrow">Proyecto</p>
          <strong>{session.world.name}</strong>
        </div>
        <dl className="scope-summary">
          <div><dt>Escribiendo en</dt><dd>{session.active_variant.name}</dd></div>
          <div><dt>Viendo</dt><dd>{session.read_only ? "Versión anterior" : "Versión actual"}</dd></div>
        </dl>
        {session.read_only && <span className="read-only-chip">Solo lectura</span>}
        <div className="open-shell-actions">
          <button type="button" className="topbar-search" onClick={(event) => openPalette(event.currentTarget)}>Buscar <kbd>Ctrl K</kbd></button>
          <button type="button" className="ghost" aria-expanded={reviewOpen} onClick={(event) => openReviews(event.currentTarget)}>Cambios <span className="count-badge" aria-label={`${pendingCount} cambios pendientes`}>{pendingCount}</span></button>
          <button type="button" className="ghost" onClick={(event) => openDialog("settings", event.currentTarget)}>Settings</button>
          <button type="button" className="ghost" onClick={(event) => openDialog("help", event.currentTarget)}>Ayuda</button>
          <button type="button" className="ghost" onClick={() => closeButton.click()}>Cerrar</button>
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
            <p>{onboarding.completed.length} de {onboardingItems.length} pasos marcados en este equipo.</p>
            <ol className="onboarding-list">
              {onboardingItems.map((item) => (
                <li key={item.id}>
                  <label>
                    <input
                      type="checkbox"
                      checked={onboarding.completed.includes(item.id)}
                      onChange={(event) => updateOnboarding(event.currentTarget.checked
                        ? [...onboarding.completed, item.id]
                        : onboarding.completed.filter((id) => id !== item.id))}
                    />
                    <span>{item.label}</span>
                  </label>
                </li>
              ))}
            </ol>
          </section>
        )}
      </section>
      {area === "chronology" && <WorldTimeline onOpen={openTimelineEvent} />}
      {area === "assistant" && (
        <>
          <button type="button" className="assistant-sheet-backdrop" aria-label="Cerrar asistente" onClick={closeAssistant} />
          <button type="button" className="assistant-sheet-close ghost" onClick={closeAssistant}>Cerrar asistente</button>
        </>
      )}
      {reviewOpen && (
        <>
          <button type="button" className="review-drawer-backdrop" aria-label="Cerrar cambios pendientes" onClick={closeReviews} />
          <button type="button" className="review-drawer-close ghost" onClick={closeReviews}>Cerrar cambios</button>
        </>
      )}
      <p className="area-announcement" aria-live="polite">Área actual: {currentArea.label}</p>
      {explorerHost && createPortal(<WorldExplorer />, explorerHost)}
      {contextHost && createPortal(<WorldContext />, contextHost)}
      {importCenterHost && createPortal(<ImportCenter active={area === "imports"} />, importCenterHost)}
      <ObjectPicker />
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
      <SoftwareDialogs active={dialog} onActiveChange={setDialog} returnFocus={dialogTrigger} />
    </div>
  );
}
