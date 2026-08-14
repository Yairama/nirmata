import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { showAiCommandError, showCommandError, showSuccess } from "./feedback.js";
import type { AiFailureObservation } from "./feedback.js";
import { humanize } from "./helpers.js";
import { pendingReviewsQueryKey } from "./pending-reviews.js";
import { beginAiActivity, dialog, endAiActivity, invoke, listen, loreFilter } from "./state.js";
import type {
  AiProviderDiagnosticStatus,
  ImportBatchSnapshot,
  ImportCandidate,
  ImportCandidateSnapshot,
  ImportReviewPreparation,
} from "./types.js";
import { useSession } from "./session-provider.js";

type DecisionPoint = ImportReviewPreparation["decisionPoints"][number];
type ProgressEvent = { requestId: string; progress: { kind: string } };

function requestId() {
  return crypto.randomUUID();
}

function candidateText(candidate: ImportCandidate) {
  switch (candidate.kind) {
    case "entity": return candidate.summary;
    case "relation": return candidate.relationKind;
    case "event": return candidate.bodyMd;
    case "claim": return candidate.contentMd;
    case "rule": return candidate.statementMd;
  }
}

function editedCandidate(candidate: ImportCandidate, value: string, authentication?: string): ImportCandidate {
  switch (candidate.kind) {
    case "entity": return { ...candidate, summary: value };
    case "relation": return { ...candidate, relationKind: value };
    case "event": return { ...candidate, bodyMd: value };
    case "claim": return { ...candidate, contentMd: value, authentication: authentication ?? candidate.authentication };
    case "rule": return { ...candidate, statementMd: value };
  }
}

function CandidateFacts({ candidate }: { candidate: ImportCandidate }) {
  switch (candidate.kind) {
    case "entity": return <dl className="lore-candidate-facts"><div><dt>Nombre</dt><dd>{candidate.name}</dd></div><div><dt>Tipo</dt><dd>{humanize(candidate.entityKind)}</dd></div><div><dt>Alias</dt><dd>{candidate.aliases.join(", ") || "Sin alias"}</dd></div></dl>;
    case "relation": return <dl className="lore-candidate-facts"><div><dt>Origen</dt><dd>{candidate.sourceName}</dd></div><div><dt>Destino</dt><dd>{candidate.targetName}</dd></div><div><dt>Dirección</dt><dd>{humanize(candidate.direction)}</dd></div></dl>;
    case "event": return <dl className="lore-candidate-facts"><div><dt>Resumen</dt><dd>{candidate.summary}</dd></div><div><dt>Participantes</dt><dd>{candidate.participantNames.join(", ") || "Sin participantes"}</dd></div></dl>;
    case "claim": return <dl className="lore-candidate-facts"><div><dt>Sujeto</dt><dd>{candidate.subjectName}</dd></div><div><dt>Predicado</dt><dd>{candidate.predicateKey || "Prosa"}</dd></div><div><dt>Objeto</dt><dd>{candidate.objectScalar || "No especificado"}</dd></div><div><dt>Polaridad</dt><dd>{humanize(candidate.polarity)}</dd></div></dl>;
    case "rule": return <dl className="lore-candidate-facts"><div><dt>Alcance</dt><dd>{candidate.scope}</dd></div></dl>;
  }
}

function CandidateCard({ item, batch, onChange }: {
  item: ImportCandidateSnapshot;
  batch: ImportBatchSnapshot;
  onChange: (candidates: ImportCandidateSnapshot[], message: string) => void;
}) {
  const [text, setText] = useState(() => candidateText(item.candidate));
  const [authentication, setAuthentication] = useState(item.candidate.kind === "claim" ? item.candidate.authentication : "canonical");
  const initialIdentity = item.identityDecision === "exact" && item.canonicalUri
    ? `exact:${item.canonicalUri}`
    : item.identityDecision ?? (item.identitySuggestion === "ambiguous" ? "ambiguous" : "new");
  const [identity, setIdentity] = useState(initialIdentity);
  const [saving, setSaving] = useState(false);
  const [openedExcerpt, setOpenedExcerpt] = useState("");

  async function saveEdit() {
    setSaving(true);
    try {
      const candidates = await invoke<ImportCandidateSnapshot[]>("edit_lore_candidate", {
        input: { batchId: batch.id, candidateId: item.id, replacement: editedCandidate(item.candidate, text, authentication) },
      });
      onChange(candidates, "Edición guardada. Siguiente acción: vuelve a seleccionar o rechazar este candidato.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: saveEdit });
    } finally {
      setSaving(false);
    }
  }

  async function decide(selected: boolean) {
    try {
      const identityDecision = !selected
        ? null
        : identity.startsWith("exact:")
          ? { kind: "exact", canonicalUri: identity.slice(6) }
          : { kind: identity };
      const candidates = await invoke<ImportCandidateSnapshot[]>("decide_lore_candidate", {
        input: { batchId: batch.id, decision: { candidateId: item.id, selected, identity: identityDecision } },
      });
      onChange(candidates, selected
        ? "Candidato seleccionado. Siguiente acción: revisa los demás o prepara la revisión."
        : "Candidato rechazado. Siguiente acción: revisa los demás o prepara la revisión.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: () => decide(selected) });
    }
  }

  return (
    <article className="lore-candidate">
      <div className="lore-candidate-heading">
        <div><p className="panel-eyebrow">{humanize(item.candidate.kind)}</p><h4>{item.candidate.kind === "entity" ? item.candidate.name : item.candidate.kind === "event" ? item.candidate.summary : candidateText(item.candidate).slice(0, 70)}</h4></div>
        <span className={`status-chip ${item.status === "selected" ? "success" : "info"}`}>{item.status === "selected" ? "Seleccionado" : item.status === "rejected" ? "Rechazado" : "Por decidir"}</span>
      </div>
      <CandidateFacts candidate={item.candidate} />
      <label>Contenido editable
        <textarea rows={4} value={text} onChange={(event) => setText(event.target.value)} />
      </label>
      {item.candidate.kind === "claim" && <label>Autoridad
        <select value={authentication} onChange={(event) => setAuthentication(event.target.value)}>
          <option value="canonical">Hecho canónico</option>
          <option value="attributed">Afirmación atribuida</option>
        </select>
      </label>}
      <label>Resolución de identidad
        <select value={identity} onChange={(event) => setIdentity(event.target.value)}>
          {item.identityMatches.map((match) => <option key={match.uri} value={`exact:${match.uri}`}>Enlazar con {match.name}</option>)}
          <option value="new">Crear identidad nueva</option>
          {item.identitySuggestion === "ambiguous" && <option value="ambiguous">Mantener ambigua</option>}
        </select>
      </label>
      <div className="lore-citations">
        <strong>Fuentes</strong>
        {item.candidate.citations.map((citation, index) => {
          const source = batch.sources.find((value) => value.id === citation.sourceId);
          const chunk = source?.chunks.find((value) => value.id === citation.chunkId);
          return <blockquote key={`${citation.chunkId}-${index}`}><cite>{source?.fileName ?? "Fuente"}{chunk ? `, líneas ${chunk.lineStart}-${chunk.lineEnd}` : ""}</cite><p>{citation.excerpt}</p><button type="button" className="ghost" onClick={async () => {
            try {
              const location = await invoke<{ chunk: { content: string } }>("open_lore_chunk", { input: { batchId: batch.id, chunkId: citation.chunkId } });
              setOpenedExcerpt(location.chunk.content);
            } catch (error) {
              showCommandError(error);
            }
          }}>Ver fragmento copiado</button></blockquote>;
        })}
        {openedExcerpt && <pre className="lore-preview">{openedExcerpt}</pre>}
      </div>
      <details><summary>Detalles técnicos</summary><p>Confianza de extracción: {(item.candidate.technicalConfidence * 100).toFixed(0)} %. No representa verdad del mundo.</p></details>
      <div className="pending-actions">
        <button type="button" className="secondary" disabled={saving} onClick={saveEdit}>{saving ? "Guardando…" : "Guardar edición"}</button>
        <button type="button" onClick={() => decide(true)}>{item.status === "selected" ? "Actualizar selección" : "Seleccionar"}</button>
        <button type="button" className="ghost" onClick={() => decide(false)}>Rechazar</button>
      </div>
    </article>
  );
}

export function LoreImportWorkspace({ launchId }: { launchId: number | null }) {
  const session = useSession();
  const queryClient = useQueryClient();
  const [batches, setBatches] = useState<ImportBatchSnapshot[]>([]);
  const [batch, setBatch] = useState<ImportBatchSnapshot | null>(null);
  const [candidates, setCandidates] = useState<ImportCandidateSnapshot[]>([]);
  const [decisionPoints, setDecisionPoints] = useState<DecisionPoint[]>([]);
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);
  const [status, setStatus] = useState("Cargando lotes guardados…");
  const [deleteArmed, setDeleteArmed] = useState(false);
  const aiObservation = useRef<AiFailureObservation>({ startedAtMs: Date.now(), phase: "preparing", receivedCharacters: 0 });
  const provider = useQuery({
    queryKey: ["desktop", "provider-status"],
    queryFn: () => invoke<AiProviderDiagnosticStatus>("get_ai_provider_status"),
    enabled: Boolean(session),
    retry: false,
  });
  const providerReady = provider.data?.connected === true;

  async function selectBatch(next: ImportBatchSnapshot) {
    setBatch(next);
    setDeleteArmed(false);
    try {
      const loaded = await invoke<ImportCandidateSnapshot[]>("read_lore_candidates", { input: { batchId: next.id } });
      setCandidates(loaded);
      setStatus(loaded.length > 0 ? "Lote reanudado. Siguiente acción: decide los candidatos pendientes." : "Lote reanudado. Siguiente acción: extraer candidatos.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: () => selectBatch(next) });
    }
  }

  async function loadBatches(preferredId?: string) {
    try {
      const result = await invoke<ImportBatchSnapshot[]>("list_lore_imports");
      const list = Array.isArray(result) ? result : [];
      setBatches(list);
      const selected = list.find((item) => item.id === preferredId) ?? list[0] ?? null;
      if (selected) await selectBatch(selected);
      else {
        setBatch(null);
        setCandidates([]);
        setStatus("Sin lotes guardados. Siguiente acción: selecciona una o más fuentes.");
      }
    } catch (error) {
      setStatus("No se pudieron cargar los lotes guardados.");
      showCommandError(error, { label: "Reintentar", run: loadBatches });
    }
  }

  async function chooseSources() {
    if (session?.read_only) return;
    try {
      const selected = await dialog.open({ multiple: true, directory: false, filters: loreFilter });
      if (selected === null) return;
      const sourceFiles = Array.isArray(selected) ? selected : [selected];
      const created = await invoke<ImportBatchSnapshot>("create_lore_import", { input: { sourceFiles } });
      setBatches((value) => [created, ...value.filter((item) => item.id !== created.id)]);
      setBatch(created);
      setCandidates([]);
      setDecisionPoints([]);
      setStatus(`${sourceFiles.length} fuente${sourceFiles.length === 1 ? "" : "s"} copiada${sourceFiles.length === 1 ? "" : "s"}. Siguiente acción: extraer candidatos.`);
      showSuccess("Lote guardado", "Puedes cerrar el proyecto y reanudar esta importación después.");
    } catch (error) {
      showCommandError(error, { label: "Elegir archivos", run: chooseSources });
    }
  }

  function beginRequest(label: string) {
    const id = requestId();
    aiObservation.current = { startedAtMs: Date.now(), phase: "preparing", receivedCharacters: 0 };
    setActiveRequestId(id);
    beginAiActivity({ requestId: id, source: "lore", label });
    return id;
  }

  function finishRequest(id: string) {
    endAiActivity(id);
    setActiveRequestId(null);
  }

  async function extract() {
    if (!batch) return;
    const id = beginRequest("La IA está extrayendo candidatos del lote de lore.");
    setStatus("Preparando extracción citada…");
    try {
      await invoke("extract_lore_import", { input: { requestId: id, batchId: batch.id } });
      const loaded = await invoke<ImportCandidateSnapshot[]>("read_lore_candidates", { input: { batchId: batch.id } });
      setCandidates(loaded);
      setStatus("Extracción completa. Siguiente acción: edita, selecciona o rechaza cada candidato.");
      showSuccess("Candidatos extraídos", `${loaded.length} hallazgos están listos para decidir.`);
    } catch (error) {
      setStatus("La extracción no terminó. El lote sigue guardado.");
      showAiCommandError(error, aiObservation.current, { label: "Reintentar", run: extract });
    } finally {
      finishRequest(id);
    }
  }

  async function replaceSource(sourceId: string, sourcePath: string) {
    if (!batch) return;
    try {
      const updated = await invoke<ImportBatchSnapshot>("replace_lore_source", { input: { batchId: batch.id, sourceId, sourceFile: sourcePath } });
      setBatch(updated);
      setBatches((value) => value.map((item) => item.id === updated.id ? updated : item));
      setCandidates(await invoke<ImportCandidateSnapshot[]>("read_lore_candidates", { input: { batchId: updated.id } }));
      setStatus("Fuente copiada de nuevo. Siguiente acción: vuelve a extraer sus candidatos.");
      showSuccess("Fuente reemplazada", "Los candidatos derivados de la copia anterior fueron retirados.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: () => replaceSource(sourceId, sourcePath) });
    }
  }

  async function deleteBatch() {
    if (!batch) return;
    if (!deleteArmed) {
      setDeleteArmed(true);
      setStatus(`Confirma eliminar el lote de ${batch.sources.map((source) => source.fileName).join(", ")}. El canon y los originales no cambian.`);
      return;
    }
    try {
      await invoke("delete_lore_import", { input: { batchId: batch.id } });
      if (session) await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) });
      showSuccess("Lote eliminado", "El canon y los archivos originales permanecen intactos.");
      await loadBatches();
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: deleteBatch });
    }
  }

  async function decideFromPoint(point: DecisionPoint, alternative: string) {
    if (!batch) return;
    const item = candidates.find((candidate) => candidate.id === point.candidateId);
    if (!item) return;
    try {
      if (alternative === "mark_canonical" && item.candidate.kind === "claim") {
        const updated = await invoke<ImportCandidateSnapshot[]>("edit_lore_candidate", { input: { batchId: batch.id, candidateId: item.id, replacement: { ...item.candidate, authentication: "canonical" } } });
        setCandidates(updated);
      } else {
        const selected = alternative !== "reject";
        const identity = !selected ? null : alternative === "new" ? { kind: "new" } : { kind: "exact", canonicalUri: alternative };
        const updated = await invoke<ImportCandidateSnapshot[]>("decide_lore_candidate", { input: { batchId: batch.id, decision: { candidateId: item.id, selected, identity } } });
        setCandidates(updated);
      }
      setDecisionPoints((value) => value.filter((candidate) => candidate.candidateId !== point.candidateId));
      setStatus("Decisión guardada. Siguiente acción: prepara nuevamente la revisión.");
    } catch (error) {
      showCommandError(error, { label: "Reintentar", run: () => decideFromPoint(point, alternative) });
    }
  }

  async function prepareReview() {
    if (!batch) return;
    const id = beginRequest("La IA está comprobando la propuesta de importación.");
    try {
      const prepared = await invoke<ImportReviewPreparation>("prepare_lore_import_review", { input: { requestId: id, batchId: batch.id } });
      setDecisionPoints(prepared.decisionPoints);
      if (prepared.decisionPoints.length > 0) {
        setStatus(`${prepared.decisionPoints.length} decisión pendiente. Resuélvela y prepara nuevamente la revisión.`);
        return;
      }
      if (!prepared.reviewKey || !prepared.run) return;
      if (session) await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) });
      setStatus("Revisión preparada. Siguiente acción: abre Cambios para decidir y aplicar.");
      showSuccess("Revisión preparada", "La propuesta sigue fuera del canon hasta que uses Aplicar al mundo.");
    } catch (error) {
      showAiCommandError(error, aiObservation.current, { label: "Reintentar", run: prepareReview });
    } finally {
      finishRequest(id);
    }
  }

  async function cancelRequest() {
    if (!activeRequestId) return;
    try {
      await invoke("cancel_ai_request", { requestId: activeRequestId });
    } catch (error) {
      showCommandError(error);
    }
  }

  useEffect(() => { void loadBatches(); }, [session?.world_id]);
  useEffect(() => {
    if (launchId !== null) void chooseSources();
  }, [launchId]);
  useEffect(() => {
    let disposed = false;
    let removeProgress: (() => void) | undefined;
    void listen<ProgressEvent>("lore-import-progress", ({ payload }) => {
      if (payload.requestId === activeRequestId) {
        aiObservation.current.phase = payload.progress.kind;
        setStatus(`Extracción: ${humanize(payload.progress.kind)}…`);
      }
    }).then((remove) => disposed ? remove() : removeProgress = remove);
    return () => { disposed = true; removeProgress?.(); };
  }, [activeRequestId]);

  if (!session) return null;
  const stale = batch ? batch.variantId !== session.active_variant.id || batch.targetRevision !== session.current_revision : false;
  const selectedCount = candidates.filter((candidate) => candidate.status === "selected").length;

  return (
    <section className="lore-import-workspace" aria-labelledby="lore-import-title">
      <div className="lore-workspace-heading">
        <div><p className="panel-eyebrow">Fuentes locales no confiables</p><h2 id="lore-import-title">Importar lore revisable</h2></div>
        <p role="status" className="panel-summary">{status}</p>
      </div>
      {session.read_only && <p className="notice warning">Estás viendo una versión anterior. Vuelve a la versión actual para crear o continuar lotes.</p>}
      {stale && <p className="notice warning">Este lote pertenece a otra variante o versión. Sus fuentes siguen guardadas, pero debes volver a esa línea o crear un lote nuevo.</p>}
      <div className="lore-toolbar">
        <label>Lote guardado
          <select value={batch?.id ?? ""} onChange={(event) => {
            const selected = batches.find((item) => item.id === event.target.value);
            if (selected) void selectBatch(selected);
          }}>
            <option value="">Sin lote seleccionado</option>
            {batches.map((item) => <option key={item.id} value={item.id}>{new Date(item.createdAtMs).toLocaleString("es")} · {item.sources.map((source) => source.fileName).join(", ")}</option>)}
          </select>
        </label>
        <div className="pending-actions">
          <button type="button" disabled={session.read_only || Boolean(activeRequestId)} onClick={chooseSources}>Nuevo lote…</button>
          <button type="button" className="secondary" disabled={!batch || stale || !providerReady || Boolean(activeRequestId)} onClick={extract}>Extraer candidatos</button>
          <button type="button" className="secondary" disabled={!activeRequestId} onClick={cancelRequest}>Cancelar</button>
          <button type="button" disabled={!batch || stale || selectedCount === 0 || !providerReady || Boolean(activeRequestId)} onClick={prepareReview}>Preparar revisión</button>
          <button type="button" className="ghost" disabled={!batch || Boolean(activeRequestId)} onClick={deleteBatch}>{deleteArmed ? "Confirmar eliminar lote" : "Eliminar lote"}</button>
        </div>
      </div>
      {!batch && <p className="empty-state">Selecciona varios Markdown o textos. Se copian a staging inerte y puedes cerrar el proyecto sin perder el lote.</p>}
      {batch && (
        <>
          <div className="lore-source-grid">
            {batch.sources.map((source) => (
              <article key={source.id} className="lore-source-card">
                <div><h3>{source.fileName}</h3><p>{source.sizeBytes.toLocaleString("es")} bytes · {source.chunks.length} fragmentos</p></div>
                <pre className="lore-preview">{source.preview}</pre>
                <button type="button" className="secondary" disabled={stale || Boolean(activeRequestId)} onClick={() => replaceSource(source.id, source.path)}>Volver a copiar desde el origen</button>
                <details><summary>Detalles técnicos</summary><dl className="technical-facts"><div><dt>Hash</dt><dd>{source.contentHash}</dd></div><div><dt>Ruta original</dt><dd className="path">{source.path}</dd></div></dl></details>
              </article>
            ))}
          </div>
          {decisionPoints.map((point) => (
            <article key={point.candidateId} className="notice warning lore-decision">
              <h3>Decisión necesaria</h3><p>{point.prompt}</p>
              <div className="pending-actions">{point.alternatives.map((alternative) => <button key={alternative} type="button" className={alternative === "reject" ? "ghost" : "secondary"} onClick={() => decideFromPoint(point, alternative)}>{alternative === "new" ? "Crear identidad nueva" : alternative === "mark_canonical" ? "Tratar como hecho canónico" : alternative === "reject" ? "Rechazar candidato" : candidates.flatMap((candidate) => candidate.identityMatches).find((match) => match.uri === alternative)?.name ?? "Enlazar identidad"}</button>)}</div>
            </article>
          ))}
          <div className="lore-candidate-list">
            {candidates.map((item) => <CandidateCard key={item.id} item={item} batch={batch} onChange={(updated, message) => { setCandidates(updated); setDecisionPoints((value) => value.filter((point) => point.candidateId !== item.id)); setStatus(message); }} />)}
          </div>
          {candidates.length === 0 && <p className="empty-state">Las fuentes están guardadas. Extrae candidatos para comenzar la revisión de identidad y contenido.</p>}
        </>
      )}
    </section>
  );
}
