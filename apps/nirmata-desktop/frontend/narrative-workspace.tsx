import { invoke } from "@tauri-apps/api/core";
import * as Tabs from "@radix-ui/react-tabs";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { clearError, formatObjectRef, setStatus, showError } from "./helpers.js";
import { useObjectPicker } from "./object-picker.js";
import { useSession } from "./session-provider.js";
import { beginAiActivity, endAiActivity, listen, setEphemeralWork, useAppState } from "./state.js";
import type {
  AiProviderDiagnosticStatus,
  AiRunSnapshot,
  DocumentObject,
  InternalDocumentKind,
  NarrativeCausalThreads,
  NarrativeContinuityExploration,
  NarrativeContinuityProposal,
  NarrativeContinuitySelection,
  NarrativeLooseEnds,
  NarrativeTimeline,
  ReadScope,
  SearchResult,
  TimelineOverview,
} from "./types.js";
import { useWorkspaceData } from "./workspace-data.js";

type NarrativeTab = "timeline" | "causal" | "loose" | "documents";
type AnalysisScope = "observed" | "current";
type NamedObject = { id: string; uri: string; label: string };
type ProgressEvent = { requestId: string; progress: { kind: string } };
type ContinuityInput =
  | { kind: "loose_end"; code: string; objectUri: string }
  | { kind: "causal_thread"; startEventId: string };
type DocumentPreview = {
  document: DocumentObject;
  perspective: string;
  moment: string;
  referenceCount: number;
};

const documentKinds: Array<{ value: InternalDocumentKind; label: string }> = [
  { value: "chronicle", label: "Crónica" },
  { value: "letter", label: "Carta" },
  { value: "report", label: "Informe" },
  { value: "myth", label: "Mito" },
  { value: "short_story", label: "Historia corta" },
];

function cleanLabel(result: SearchResult): string {
  return result.snippet.replace(/[\[\]]/gu, "").trim() || "Objeto sin nombre";
}

function sourceButtons(uris: string[], scope: ReadScope, onOpen: (uri: string, scope: ReadScope) => void): ReactNode {
  if (uris.length === 0) return null;
  return (
    <div className="narrative-sources" aria-label="Fuentes">
      {uris.map((uri, index) => <button key={uri} type="button" className="ghost" onClick={() => onOpen(uri, scope)}>Abrir fuente {index + 1}</button>)}
    </div>
  );
}

function causalKind(kind: string): string {
  return ({
    enables: "Habilita",
    causes: "Causa",
    motivates: "Motiva",
    prevents: "Impide",
    terminates: "Termina",
    reveals: "Revela",
  } as Record<string, string>)[kind] ?? "Conecta";
}

function looseEndCopy(code: string): { title: string; detail: string } {
  return ({
    active_goal_without_resolution: {
      title: "Meta activa sin resolución",
      detail: "Una meta sigue activa en esta vista y no tiene una resolución afirmada.",
    },
    ongoing_event: {
      title: "Acontecimiento en curso",
      detail: "Un acontecimiento está explícitamente en curso y todavía no tiene final afirmado.",
    },
    disputed_claim: {
      title: "Afirmación disputada",
      detail: "Una afirmación continúa disputada y no ha sido reemplazada en esta vista.",
    },
  } as Record<string, { title: string; detail: string }>)[code] ?? {
    title: "Continuidad por revisar",
    detail: "La evidencia explícita de esta vista sugiere revisar este hilo.",
  };
}

function progressCopy(kind: string): string {
  return ({
    preparing_context: "Reuniendo contexto acotado…",
    calling_model: "Preparando el documento…",
    validating: "Comprobando estructura y fuentes…",
    semantic_critique: "Ejecutando la comprobación final…",
    awaiting_review: "Preparando la revisión…",
  } as Record<string, string>)[kind] ?? "Preparando un resultado revisable…";
}

function documentPreview(run: AiRunSnapshot, perspective: string, moment: string): DocumentPreview | null {
  for (const operation of run.draft?.operations ?? []) {
    if (!operation || typeof operation !== "object" || !("create_document" in operation)) continue;
    const value = (operation as { create_document?: { after?: { object?: DocumentObject; references?: unknown[] } } }).create_document;
    if (!value?.after?.object) continue;
    return {
      document: value.after.object,
      perspective,
      moment,
      referenceCount: value.after.references?.length ?? 0,
    };
  }
  return null;
}

function selectionInput(selection: NarrativeContinuitySelection): ContinuityInput {
  return selection.kind === "loose_end"
    ? { kind: "loose_end", code: selection.code, objectUri: formatObjectRef(selection.objectRef).uri }
    : { kind: "causal_thread", startEventId: selection.startEventId };
}

export function NarrativeWorkspace({
  active,
  onOpen,
  onOpenReviews,
  onPendingReviewsChanged,
}: {
  active: boolean;
  onOpen: (uri: string, scope: ReadScope) => void;
  onOpenReviews: () => void;
  onPendingReviewsChanged: () => void;
}) {
  const requestObjectPicker = useObjectPicker();
  const appState = useAppState();
  const session = useSession();
  const selectedUri = appState.selectedUri;
  const { timeline: moments } = useWorkspaceData();
  const [tab, setTab] = useState<NarrativeTab>("timeline");
  const [analysisScope, setAnalysisScope] = useState<AnalysisScope>("observed");
  const [timelineResult, setTimelineResult] = useState<NarrativeTimeline | null>(null);
  const [causalResult, setCausalResult] = useState<NarrativeCausalThreads | null>(null);
  const [looseResult, setLooseResult] = useState<NarrativeLooseEnds | null>(null);
  const [exploration, setExploration] = useState<NarrativeContinuityExploration | null>(null);
  const [startEvents, setStartEvents] = useState<NamedObject[]>([]);
  const [maxDepth, setMaxDepth] = useState("3");
  const [limit, setLimit] = useState("100");
  const [documentKind, setDocumentKind] = useState<InternalDocumentKind>("chronicle");
  const [documentTitle, setDocumentTitle] = useState("");
  const [documentRequest, setDocumentRequest] = useState("");
  const [perspective, setPerspective] = useState<NamedObject | null>(null);
  const [momentTick, setMomentTick] = useState("");
  const [preview, setPreview] = useState<DocumentPreview | null>(null);
  const [reviewAttached, setReviewAttached] = useState(false);
  const [status, setLocalStatus] = useState("Elige una lectura; las derivaciones no modifican el mundo.");
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);
  const provider = useQuery({
    queryKey: ["desktop", "provider-status"],
    queryFn: () => invoke<AiProviderDiagnosticStatus>("get_ai_provider_status"),
    enabled: Boolean(session),
    retry: false,
  });
  const providerReady = provider.data?.connected === true;
  const requestRef = useRef<string | null>(null);
  const scope: ReadScope | null = !session
    ? null
    : analysisScope === "current"
      ? { variantId: session.active_variant.id, revisionId: null }
      : { ...session.read_scope };
  const knownMoments = (moments.data?.known ?? []).filter((event) => event.time.start_tick !== null);
  const selectedMoment = knownMoments.find((event) => String(event.time.start_tick) === momentTick) ?? null;
  const dirty = timelineResult !== null || causalResult !== null || looseResult !== null || exploration !== null
    || startEvents.length > 0 || documentTitle.trim() !== "" || documentRequest.trim() !== ""
    || perspective !== null || momentTick !== "" || preview !== null;

  function reset() {
    setTimelineResult(null);
    setCausalResult(null);
    setLooseResult(null);
    setExploration(null);
    setStartEvents([]);
    setMaxDepth("3");
    setLimit("100");
    setDocumentKind("chronicle");
    setDocumentTitle("");
    setDocumentRequest("");
    setPerspective(null);
    setMomentTick("");
    setPreview(null);
    setReviewAttached(false);
    setLocalStatus("Elige una lectura; las derivaciones no modifican el mundo.");
    setEphemeralWork("narrative", "derivaciones o formulario narrativo", false);
  }

  useEffect(() => {
    setEphemeralWork("narrative", "derivaciones o formulario narrativo", dirty);
  }, [dirty]);

  useEffect(() => {
    reset();
    return () => setEphemeralWork("narrative", "", false);
  }, [appState.discardRevision]);

  useEffect(() => {
    let disposed = false;
    let unlisten: () => void = () => undefined;
    void listen<ProgressEvent>("ai-proposal-progress", ({ payload }) => {
      if (payload.requestId === requestRef.current) setLocalStatus(progressCopy(payload.progress.kind));
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten();
    };
  }, []);

  useEffect(() => {
    setTimelineResult(null);
    setCausalResult(null);
    setLooseResult(null);
    setExploration(null);
    setStartEvents([]);
    setAnalysisScope("observed");
  }, [session?.world_id, session?.read_scope.variantId, session?.read_scope.revisionId]);

  useEffect(() => {
    if (momentTick && !knownMoments.some((event) => String(event.time.start_tick) === momentTick)) setMomentTick("");
  }, [momentTick, moments.data]);

  if (!session) return null;

  function startAi(label: string): string {
    const requestId = crypto.randomUUID();
    requestRef.current = requestId;
    setActiveRequestId(requestId);
    beginAiActivity({ requestId, source: "narrative", label });
    return requestId;
  }

  function finishAi(requestId: string) {
    endAiActivity(requestId);
    if (requestRef.current === requestId) requestRef.current = null;
    setActiveRequestId(null);
  }

  async function deriveTimeline() {
    if (!scope) return;
    try {
      clearError();
      const result = await invoke<NarrativeTimeline>("derive_narrative_timeline", { input: { scope } });
      setTimelineResult(result);
      setLocalStatus("Orden cronológico y orden del relato derivados sin escritura.");
    } catch (value) { showError(value); }
  }

  async function deriveCausal() {
    if (!scope) return;
    const depth = Number(maxDepth);
    const resultLimit = Number(limit);
    if (!Number.isSafeInteger(depth) || depth < 0 || depth > 3) return showError("La profundidad debe estar entre 0 y 3.");
    if (!Number.isSafeInteger(resultLimit) || resultLimit < 1 || resultLimit > 100) return showError("El límite debe estar entre 1 y 100.");
    try {
      clearError();
      const result = await invoke<NarrativeCausalThreads>("derive_causal_threads", {
        input: { scope, startEventIds: startEvents.length ? startEvents.map((event) => event.id) : null, maxDepth: depth, limit: resultLimit },
      });
      setCausalResult(result);
      setExploration(null);
      setLocalStatus("Relaciones causales derivadas con evidencia navegable.");
    } catch (value) { showError(value); }
  }

  async function deriveLooseEnds() {
    if (!scope) return;
    try {
      clearError();
      const result = await invoke<NarrativeLooseEnds>("derive_loose_ends", { input: { scope } });
      setLooseResult(result);
      setExploration(null);
      setLocalStatus("Cabos abiertos derivados sólo desde estados explícitos y sus fuentes.");
    } catch (value) { showError(value); }
  }

  async function explore(scopeValue: ReadScope, selection: NarrativeContinuitySelection) {
    try {
      clearError();
      const result = await invoke<NarrativeContinuityExploration>("explore_narrative_continuity", {
        input: { scope: scopeValue, selection: selectionInput(selection) },
      });
      setExploration(result);
      setLocalStatus("Alternativas de continuidad listas; todavía no se llamó a IA.");
    } catch (value) { showError(value); }
  }

  async function proposeContinuity(value: NarrativeContinuityExploration, alternativeId: string) {
    if (!session || session.read_only) return showError("Vuelve a la versión actual antes de preparar una propuesta de continuidad.");
    const requestId = startAi("La IA está preparando una propuesta narrativa revisable.");
    setLocalStatus("Preparando una propuesta estándar de continuidad…");
    try {
      clearError();
      const proposal = await invoke<NarrativeContinuityProposal>("propose_narrative_continuity", {
        input: { requestId, scope: value.scope, selection: selectionInput(value.selection), alternativeId },
      });
      if (proposal.run.reviewKey && proposal.run.draft) onPendingReviewsChanged();
      setLocalStatus("Propuesta de continuidad añadida a Cambios.");
      setStatus("La propuesta está en Cambios; el canon todavía no cambió.");
    } catch (error) { showError(error); }
    finally { finishAi(requestId); }
  }

  async function generateDocument(event: FormEvent) {
    event.preventDefault();
    if (!session || session.read_only) return showError("Vuelve a la versión actual antes de preparar un documento.");
    if (!perspective) return showError("Elige una perspectiva por nombre.");
    if (!selectedMoment || selectedMoment.time.start_tick === null) return showError("Elige un momento existente de la cronología.");
    const requestId = startAi("La IA está preparando un documento interno revisable.");
    setPreview(null);
    setReviewAttached(false);
    setLocalStatus("Generando el borrador y su comprobación estándar…");
    try {
      clearError();
      const run = await invoke<AiRunSnapshot>("generate_internal_document", {
        input: {
          requestId,
          documentKind,
          title: documentTitle.trim(),
          request: documentRequest.trim(),
          perspectiveEntityId: perspective.id,
          tick: selectedMoment.time.start_tick,
          anchorUris: selectedUri ? [selectedUri] : [],
        },
      });
      const generated = documentPreview(run, perspective.label, selectedMoment.startCalendar?.label ?? selectedMoment.summary);
      if (!generated) throw new Error("La respuesta no incluyó un documento para previsualizar.");
      if (!run.reviewKey) throw new Error("El documento no incluyó una revisión pendiente.");
      onPendingReviewsChanged();
      setPreview(generated);
      setReviewAttached(true);
      setLocalStatus("Borrador seguro listo y añadido una vez a Cambios.");
      setStatus("El documento está en Cambios; aún no forma parte del canon.");
    } catch (value) { showError(value); }
    finally { finishAi(requestId); }
  }

  const scopeCopy = analysisScope === "current"
    ? `Versión actual de ${session.active_variant.name}`
    : session.read_only ? "Versión observada · solo lectura" : "Versión observada · actual";
  return (
    <section className="narrative-workspace" aria-labelledby="narrative-title" hidden={!active}>
      <header className="narrative-workspace-heading">
        <div><p className="panel-eyebrow narrative-eyebrow">Lectura derivada y citada</p><h1 id="narrative-title" tabIndex={-1}>Estudio narrativo</h1><p>Ordena el relato, sigue causas y revisa continuidad sin convertir inferencias en canon.</p></div>
        <div className="narrative-scope-card">
          <span>Vista analizada</span>
          <strong>{scopeCopy}</strong>
          <div role="group" aria-label="Vista analizada">
            <button type="button" className={analysisScope === "observed" ? "secondary" : "ghost"} aria-pressed={analysisScope === "observed"} onClick={() => setAnalysisScope("observed")}>Observada</button>
            <button type="button" className={analysisScope === "current" ? "secondary" : "ghost"} aria-pressed={analysisScope === "current"} onClick={() => setAnalysisScope("current")}>Actual</button>
          </div>
        </div>
      </header>
      <p className="narrative-status" role="status">{status}</p>
      <Tabs.Root className="narrative-tabs" value={tab} onValueChange={(value) => setTab(value as NarrativeTab)}>
        <Tabs.List aria-label="Herramientas narrativas">
          <Tabs.Trigger value="timeline">Cronología</Tabs.Trigger>
          <Tabs.Trigger value="causal">Causalidad</Tabs.Trigger>
          <Tabs.Trigger value="loose">Cabos abiertos</Tabs.Trigger>
          <Tabs.Trigger value="documents">Documentos</Tabs.Trigger>
        </Tabs.List>

        <Tabs.Content value="timeline" className="narrative-tab-content">
          <div className="narrative-tool-heading"><div><h2>Orden del mundo y del relato</h2><p>Compara cuándo ocurren los hechos con el orden en que las fuentes los cuentan.</p></div><div><button type="button" onClick={() => void deriveTimeline()}>Derivar órdenes</button>{timelineResult && <button type="button" className="ghost" onClick={() => setTimelineResult(null)}>Limpiar resultado</button>}</div></div>
          {!timelineResult && <p className="empty-state">Deriva esta vista para separar tiempo conocido, tiempo no especificado y secuencias narradas.</p>}
          {timelineResult && <TimelineResult value={timelineResult} moments={moments.data ?? null} onOpen={onOpen} />}
        </Tabs.Content>

        <Tabs.Content value="causal" className="narrative-tab-content">
          <div className="narrative-tool-heading"><div><h2>Hilos causales</h2><p>Parte de acontecimientos elegidos por nombre o revisa todos los inicios disponibles.</p></div>{causalResult && <button type="button" className="ghost" onClick={() => { setCausalResult(null); setExploration(null); }}>Limpiar resultado</button>}</div>
          <div className="narrative-causal-controls">
            <div><span className="field-label">Acontecimientos iniciales</span><div className="narrative-chip-list">{startEvents.map((item) => <span key={item.uri}>{item.label}<button type="button" className="ghost" aria-label={`Quitar ${item.label}`} onClick={() => setStartEvents((current) => current.filter((event) => event.uri !== item.uri))}>Quitar</button></span>)}</div><button type="button" className="secondary" onClick={(event) => requestObjectPicker({ title: "Acontecimientos iniciales", kinds: ["event"], multiple: true, returnFocus: event.currentTarget, apply: (results) => setStartEvents((current) => [...current, ...results.filter((result) => !current.some((item) => item.uri === result.uri)).map((result) => ({ id: result.object_id, uri: result.uri, label: cleanLabel(result) }))]) })}>Elegir acontecimientos</button></div>
            <label>Profundidad<select value={maxDepth} onChange={(event) => setMaxDepth(event.currentTarget.value)}><option value="1">Directa</option><option value="2">Dos niveles</option><option value="3">Tres niveles</option></select></label>
            <label>Máximo de relaciones<input type="number" min="1" max="100" step="1" value={limit} onChange={(event) => setLimit(event.currentTarget.value)} /></label>
            <button type="button" onClick={() => void deriveCausal()}>Derivar causalidad</button>
          </div>
          {!causalResult && <p className="empty-state">La lectura sigue enlaces explícitos, evita ciclos y nunca inventa una causa ausente.</p>}
          {causalResult && <CausalResult value={causalResult} names={startEvents} onOpen={onOpen} onExplore={explore} />}
          {exploration && <ContinuityResult value={exploration} disabled={session.read_only || !providerReady || activeRequestId !== null} onOpen={onOpen} onChoose={proposeContinuity} />}
        </Tabs.Content>

        <Tabs.Content value="loose" className="narrative-tab-content">
          <div className="narrative-tool-heading"><div><h2>Cabos abiertos</h2><p>Reúne sólo metas activas, acontecimientos en curso y afirmaciones disputadas explícitas.</p></div><div><button type="button" onClick={() => void deriveLooseEnds()}>Buscar cabos</button>{looseResult && <button type="button" className="ghost" onClick={() => { setLooseResult(null); setExploration(null); }}>Limpiar resultado</button>}</div></div>
          {!looseResult && <p className="empty-state">La ausencia de datos no se presenta como problema de continuidad.</p>}
          {looseResult && <LooseEndsResult value={looseResult} onOpen={onOpen} onExplore={explore} />}
          {exploration && <ContinuityResult value={exploration} disabled={session.read_only || !providerReady || activeRequestId !== null} onOpen={onOpen} onChoose={proposeContinuity} />}
        </Tabs.Content>

        <Tabs.Content value="documents" className="narrative-tab-content">
          <div className="narrative-tool-heading"><div><h2>Documento interno</h2><p>La IA usa una perspectiva y un momento concretos. El resultado será una propuesta, no canon.</p></div>{preview && <button type="button" className="ghost" onClick={() => setPreview(null)}>Limpiar preview</button>}</div>
          {session.read_only && <p className="read-only-callout">Solo lectura: puedes derivar las otras vistas, pero debes volver a la versión actual para preparar un documento.</p>}
          <form className="narrative-document-form" onSubmit={(event) => void generateDocument(event)}>
            <label>Tipo<select value={documentKind} disabled={session.read_only || activeRequestId !== null} onChange={(event) => setDocumentKind(event.currentTarget.value as InternalDocumentKind)}>{documentKinds.map((kind) => <option key={kind.value} value={kind.value}>{kind.label}</option>)}</select></label>
            <label>Título<input maxLength={200} required disabled={session.read_only || activeRequestId !== null} value={documentTitle} onChange={(event) => setDocumentTitle(event.currentTarget.value)} /></label>
            <label className="narrative-request-field">Instrucciones<textarea rows={5} maxLength={20_000} required disabled={session.read_only || activeRequestId !== null} value={documentRequest} onChange={(event) => setDocumentRequest(event.currentTarget.value)} placeholder="Qué debe contar y con qué intención" /></label>
            <div className="narrative-picker-field"><span className="field-label">Perspectiva</span><strong>{perspective?.label ?? "Sin elegir"}</strong><button type="button" className="secondary" disabled={session.read_only || activeRequestId !== null} onClick={(event) => requestObjectPicker({ title: "Perspectiva del documento", kinds: ["entity"], multiple: false, returnFocus: event.currentTarget, apply: ([result]) => { if (result) setPerspective({ id: result.object_id, uri: result.uri, label: cleanLabel(result) }); } })}>{perspective ? "Cambiar perspectiva" : "Elegir por nombre"}</button></div>
            <label>Momento<select required disabled={session.read_only || activeRequestId !== null || moments.isPending} value={momentTick} onChange={(event) => setMomentTick(event.currentTarget.value)}><option value="">Selecciona un acontecimiento fechado</option>{knownMoments.map((event) => <option key={event.uri} value={String(event.time.start_tick)}>{event.startCalendar?.label ?? "Momento registrado"} · {event.summary}</option>)}</select></label>
            {selectedMoment && <details><summary>Detalles técnicos de la fecha</summary><p>Unidad temporal interna: {selectedMoment.time.start_tick}</p></details>}
            {moments.isError && <p role="alert" className="notice warning">No se pudieron cargar los momentos. El formulario se conservó.</p>}
            <button type="submit" disabled={session.read_only || !providerReady || activeRequestId !== null || knownMoments.length === 0}>{activeRequestId ? "Preparando…" : "Generar borrador revisable"}</button>
            {!providerReady && <p className="muted">Verifica la conexión de IA en Ajustes antes de generar.</p>}
          </form>
          {preview && <DocumentPreviewCard value={preview} reviewAttached={reviewAttached} onOpenReviews={onOpenReviews} />}
        </Tabs.Content>
      </Tabs.Root>
    </section>
  );
}

function TimelineResult({ value, moments, onOpen }: { value: NarrativeTimeline; moments: TimelineOverview | null; onOpen: (uri: string, scope: ReadScope) => void }) {
  const dates = new Map([...(moments?.known ?? []), ...(moments?.unknown ?? [])].map((event) => [event.uri, event.startCalendar?.label ?? (event.time.kind === "unknown" ? "Momento no especificado" : "Momento registrado")]));
  const names = new Map([...value.storyTime, ...value.unknownStoryTime].map((event) => [event.event.uri, event.summary]));
  return <div className="narrative-columns">
    <section className="narrative-result"><h3>Orden cronológico</h3>{value.storyTime.map((event) => <article className="narrative-item" key={event.event.uri}><span className="narrative-date">{dates.get(event.event.uri) ?? "Momento registrado"}</span><h4>{event.summary}</h4><button type="button" className="secondary" onClick={() => onOpen(event.event.uri, value.scope)}>Abrir acontecimiento</button>{sourceButtons(event.evidenceUris, value.scope, onOpen)}</article>)}</section>
    <section className="narrative-result"><h3>Tiempo no especificado</h3>{value.unknownStoryTime.length === 0 && <p className="muted">No hay acontecimientos sin fecha en esta vista.</p>}{value.unknownStoryTime.map((event) => <article className="narrative-item" key={event.event.uri}><h4>{event.summary}</h4><button type="button" className="secondary" onClick={() => onOpen(event.event.uri, value.scope)}>Abrir acontecimiento</button>{sourceButtons(event.evidenceUris, value.scope, onOpen)}</article>)}</section>
    <section className="narrative-result"><h3>Orden en que se cuenta</h3>{value.discourseOrder.length === 0 && <p className="muted">No hay fuentes con una secuencia narrativa declarada.</p>}{value.discourseOrder.map((sequence, sourceIndex) => <article className="narrative-item" key={sequence.source.uri}><button type="button" className="ghost" onClick={() => onOpen(sequence.source.uri, value.scope)}>Abrir fuente narrativa {sourceIndex + 1}</button><ol>{sequence.events.map((event) => <li key={`${event.event.uri}-${event.ordinal}`}><span>Parte {event.ordinal + 1}</span><button type="button" className="secondary" onClick={() => onOpen(event.event.uri, value.scope)}>{names.get(event.event.uri) ?? "Abrir acontecimiento citado"}</button>{sourceButtons(event.evidenceUris, value.scope, onOpen)}</li>)}</ol></article>)}</section>
  </div>;
}

function CausalResult({ value, names, onOpen, onExplore }: { value: NarrativeCausalThreads; names: NamedObject[]; onOpen: (uri: string, scope: ReadScope) => void; onExplore: (scope: ReadScope, selection: NarrativeContinuitySelection) => void }) {
  const labels = new Map(names.map((event) => [event.uri, event.label]));
  return <section className="narrative-result"><h3>Hilos encontrados</h3>{value.threads.length === 0 && <p className="empty-state">No se encontraron enlaces causales explícitos con estos criterios.</p>}{value.threads.map((thread, index) => <article className="narrative-item" key={thread.start.uri}><p className="panel-eyebrow">Hilo {index + 1}</p><h4>{labels.get(thread.start.uri) ?? "Acontecimiento inicial"}</h4><button type="button" className="secondary" onClick={() => onOpen(thread.start.uri, value.scope)}>Abrir inicio</button><ol className="causal-link-list">{thread.links.map((link, linkIndex) => <li key={`${link.source.uri}-${link.target.uri}-${linkIndex}`}><strong>{causalKind(link.kind)}</strong><span>Nivel {link.depth}</span><div><button type="button" className="ghost" onClick={() => onOpen(link.source.uri, value.scope)}>Abrir antecedente</button><button type="button" className="ghost" onClick={() => onOpen(link.target.uri, value.scope)}>Abrir consecuencia</button></div>{sourceButtons(link.evidenceUris, value.scope, onOpen)}</li>)}</ol><button type="button" onClick={() => void onExplore(value.scope, { kind: "causal_thread", startEventId: thread.start.uri.split("/").at(-1) ?? "" })}>Explorar continuidad</button></article>)}</section>;
}

function LooseEndsResult({ value, onOpen, onExplore }: { value: NarrativeLooseEnds; onOpen: (uri: string, scope: ReadScope) => void; onExplore: (scope: ReadScope, selection: NarrativeContinuitySelection) => void }) {
  return <section className="narrative-result"><h3>Cabos encontrados</h3>{value.findings.length === 0 && <p className="empty-state">No hay estados explícitos que produzcan cabos abiertos en esta vista.</p>}{value.findings.map((finding, index) => { const copy = looseEndCopy(finding.code); const first = finding.objectRefs[0]; return <article className="narrative-item" key={`${finding.code}-${index}`}><h4>{copy.title}</h4><p>{copy.detail}</p><div className="narrative-object-actions">{finding.objectRefs.map((reference, referenceIndex) => { const uri = formatObjectRef(reference).uri; return <button key={uri} type="button" className="ghost" onClick={() => onOpen(uri, value.scope)}>Abrir objeto relacionado {referenceIndex + 1}</button>; })}</div>{sourceButtons(finding.evidenceUris, value.scope, onOpen)}{first && <button type="button" onClick={() => void onExplore(value.scope, { kind: "loose_end", code: finding.code, objectRef: first })}>Explorar continuidad</button>}</article>; })}</section>;
}

function ContinuityResult({ value, disabled, onOpen, onChoose }: { value: NarrativeContinuityExploration; disabled: boolean; onOpen: (uri: string, scope: ReadScope) => void; onChoose: (value: NarrativeContinuityExploration, alternativeId: string) => void }) {
  return <section className="narrative-result continuity-result"><p className="panel-eyebrow">Continuidad · todavía sin escritura</p><h3>¿Cómo debería continuar este hilo?</h3>{sourceButtons(value.sourceUris, value.scope, onOpen)}<div className="continuity-options">{value.alternatives.map((alternative) => <article className="narrative-alternative" key={alternative.id}><h4>{alternative.title}</h4><p>{alternative.consequence}</p><button type="button" disabled={disabled} onClick={() => void onChoose(value, alternative.id)}>Elegir y preparar propuesta</button></article>)}</div>{disabled && <p className="muted">Vuelve a la versión actual y verifica la IA para preparar una propuesta.</p>}</section>;
}

function DocumentPreviewCard({ value, reviewAttached, onOpenReviews }: { value: DocumentPreview; reviewAttached: boolean; onOpenReviews: () => void }) {
  return <article className="narrative-document-preview" aria-label="Preview del documento"><p className="panel-eyebrow">Borrador generado por IA · aún no es canon</p><h3>{value.document.title}</h3><dl><div><dt>Tipo</dt><dd>{documentKinds.find((kind) => kind.value === value.document.kind)?.label ?? "Documento"}</dd></div><div><dt>Perspectiva</dt><dd>{value.perspective}</dd></div><div><dt>Momento</dt><dd>{value.moment}</dd></div><div><dt>Fuentes citadas</dt><dd>{value.referenceCount}</dd></div></dl><pre className="narrative-document-body">{value.document.body_md}</pre><button type="button" disabled={!reviewAttached} onClick={onOpenReviews}>Abrir revisión en Cambios</button></article>;
}
