import { invoke } from "@tauri-apps/api/core";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { clearError, formatTimestamp, humanize, labelForUri, setStatus, showError } from "./helpers.js";
import { pendingReviewsQueryKey } from "./pending-reviews.js";
import { appActions, getAppState } from "./state.js";
import type {
  ManualReviewObjectSnapshot,
  MergeReviewResult,
  ObjectRef,
  ReadScope,
  RevisionAuditOperationSnapshot,
  RevisionHistoryEntrySnapshot,
  RevisionHistorySnapshot,
  Variant,
  VariantComparison,
  VariantDiff,
  VariantSummary,
  WorldSession,
} from "./types.js";
import { confirmDiscardPending, selectUriInScope } from "./workspace.js";
import { observedScopeQueryKey, useWorkspaceData } from "./workspace-data.js";
import { buttonStyles, chipStyles, cn, noticeStyles } from "./ui-styles.js";

const hiddenComparisonFields = new Set([
  "id", "world_id", "worldId", "version", "created_at_ms", "createdAtMs", "updated_at_ms", "updatedAtMs",
  "current_revision", "currentRevision", "registered_revision_id", "registeredRevisionId", "superseded_revision_id",
  "supersededRevisionId",
]);

const eyebrowStyles = "panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent";
const emptyStateStyles = "empty-state grid gap-4 rounded-xl border border-dashed border-line bg-surface p-5 text-sm text-muted";
const versionsSectionStyles = "versions-section mt-4 grid gap-4 rounded-2xl border border-line bg-surface p-5";
const versionSnapshotStyles = "grid gap-2 rounded-xl border border-line bg-canvas p-4";

export async function switchWritingVariant(variantId: string): Promise<boolean> {
  if (!await confirmDiscardPending("workspace")) return false;
  try {
    clearError();
    const session = await invoke<WorldSession>("switch_variant", { input: { variantId } });
    appActions.setSession(session);
    appActions.discardEphemeralWork();
    appActions.setStructuredEditor(null);
    setStatus("Navegación actualizada.");
    return true;
  } catch (value) {
    showError(value);
    return false;
  }
}

async function observeScope(scope: ReadScope): Promise<boolean> {
  if (!await confirmDiscardPending("editor")) return false;
  try {
    appActions.setSession(await invoke<WorldSession>("set_read_scope", { input: { scope } }));
    appActions.setStructuredEditor(null);
    setStatus("Navegación actualizada.");
    return true;
  } catch (value) {
    showError(value);
    return false;
  }
}

export async function observeRevision(revisionId: string | null): Promise<boolean> {
  const session = getAppState().session;
  if (!session) return false;
  return observeScope({ variantId: session.read_scope.variantId, revisionId });
}

export async function viewActiveVersion(): Promise<boolean> {
  if (!await confirmDiscardPending("editor")) return false;
  try {
    appActions.setSession(await invoke<WorldSession>("view_active_head"));
    appActions.setStructuredEditor(null);
    setStatus("Navegación actualizada.");
    return true;
  } catch (value) {
    showError(value);
    return false;
  }
}

function objectUri(reference: ObjectRef): string {
  const [kind, id] = Object.entries(reference)[0] ?? [];
  return kind && id ? `nirmata://${kind}/${id}` : "";
}

function affectedReferenceLabel(reference: ObjectRef): string {
  const uri = objectUri(reference);
  const resolved = labelForUri(uri);
  if (resolved !== uri) return resolved;
  const kind = Object.keys(reference)[0] ?? "object";
  const label = {
    world: "mundo",
    entity: "entidad",
    relation: "relación",
    event: "evento",
    claim: "afirmación",
    rule: "regla",
    goal: "meta",
    document: "documento",
  }[kind] ?? "objeto";
  return `Referencia a ${label}`;
}

function unwrapValue(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  const entries = Object.entries(record);
  if (entries.length === 1 && entries[0][1] && typeof entries[0][1] === "object" && !Array.isArray(entries[0][1])) {
    return entries[0][1] as Record<string, unknown>;
  }
  return record;
}

function displayValue(value: unknown): string {
  if (value === null || value === undefined || value === "") return "Sin especificar";
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) return value.length === 0 ? "Ninguno" : value.map(displayValue).join(", ");
  return JSON.stringify(value);
}

function changedFields(difference: VariantDiff): Array<{ key: string; before: string; after: string }> {
  const before = unwrapValue(difference.before) ?? {};
  const after = unwrapValue(difference.after) ?? {};
  return Array.from(new Set([...Object.keys(before), ...Object.keys(after)]))
    .filter((key) => !hiddenComparisonFields.has(key))
    .filter((key) => JSON.stringify(before[key]) !== JSON.stringify(after[key]))
    .map((key) => ({ key, before: displayValue(before[key]), after: displayValue(after[key]) }));
}

function differenceTitle(difference: VariantDiff): string {
  const value = unwrapValue(difference.after) ?? unwrapValue(difference.before);
  const candidate = value?.name ?? value?.title ?? value?.summary ?? value?.content_md ?? value?.statement_md ?? value?.desired_state_md;
  return typeof candidate === "string" && candidate.trim() ? candidate : humanize(Object.keys(difference.objectRef)[0] ?? "objeto");
}

function sourceLabel(source: VariantDiff["leftSource"]): string {
  if (!source) return "Objeto inicial o importado antes del historial disponible";
  const audit = {
    manual_review: "Edición manual",
    merge: "Cambios entre variantes",
    import: "Importación",
    undo: "Deshacer",
  }[source.auditSource] ?? humanize(source.auditSource);
  const retcon = {
    additive: "Aditivo",
    reinterpretive: "Reinterpretación",
    replacement: "Reemplazo",
  }[source.retcon];
  return `${audit} · ${retcon}`;
}

function SnapshotCard({ title, snapshot }: { title: string; snapshot: ManualReviewObjectSnapshot | null }) {
  if (!snapshot) return <article className={cn("version-object-snapshot empty", versionSnapshotStyles, "border-dashed text-muted")}><h4>{title}</h4><p>El objeto no existe en esta versión.</p></article>;
  return (
    <article className={cn("version-object-snapshot", versionSnapshotStyles)}>
      <p className={eyebrowStyles}>{title}</p>
      <h4>{snapshot.title}</h4>
      <dl className="version-field-list my-4 grid gap-2.5 [&_dd]:mt-0.5 [&_dd]:break-words [&_dt]:text-xs [&_dt]:font-bold [&_dt]:uppercase [&_dt]:tracking-wide [&_dt]:text-muted">
        {snapshot.lines.map((line) => <div key={`${line.label}-${line.value}`}><dt>{line.label}</dt><dd>{line.value}</dd></div>)}
      </dl>
    </article>
  );
}

function AuditOperation({ operation, revisionId, variantId }: { operation: RevisionAuditOperationSnapshot; revisionId: string; variantId: string }) {
  return (
    <article className={cn("version-audit-operation", versionSnapshotStyles)}>
      <header>
        <div><p className={eyebrowStyles}>{humanize(operation.source)}</p><h4>{operation.after?.title ?? operation.before?.title ?? "Objeto modificado"}</h4></div>
        <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => void selectUriInScope(operation.targetUri, { variantId, revisionId })}>Abrir en esta versión</button>
      </header>
      <div className="version-snapshot-grid grid grid-cols-2 gap-3 max-mobile:grid-cols-1">
        <SnapshotCard title="Antes" snapshot={operation.before} />
        <SnapshotCard title="Después" snapshot={operation.after} />
      </div>
    </article>
  );
}

export function VersionsWorkspace() {
  const session = getAppState().session!;
  const queryClient = useQueryClient();
  const { revisionHistory: history } = useWorkspaceData();
  const scopeKey = observedScopeQueryKey(session);
  const summaries = useQuery({
    queryKey: [...scopeKey, "variant-summaries"],
    queryFn: () => invoke<VariantSummary[]>("list_variant_summaries"),
    retry: false,
  });
  const [selectedVariantId, setSelectedVariantId] = useState("");
  const [selectedRevisionId, setSelectedRevisionId] = useState<string | null>(session.read_scope.revisionId);
  const [historyFilter, setHistoryFilter] = useState("");
  const [comparison, setComparison] = useState<VariantComparison | null>(null);
  const [newVariantName, setNewVariantName] = useState("");
  const [renameValue, setRenameValue] = useState("");
  const [archiveConfirm, setArchiveConfirm] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const variants = summaries.data ?? [];
  const observed = variants.find((item) => item.variant.id === session.read_scope.variantId);
  const selectedVariant = variants.find((item) => item.variant.id === selectedVariantId);
  const revisions = history.data?.revisions ?? [];
  const selectedRevision = revisions.find((item) => item.revisionId === selectedRevisionId) ?? revisions[0] ?? null;
  const normalizedFilter = historyFilter.trim().toLocaleLowerCase("es");
  const filteredRevisions = revisions.filter((entry) => !normalizedFilter || entry.summary.toLocaleLowerCase("es").includes(normalizedFilter)
    || entry.operations.some((operation) => [operation.before?.title, operation.after?.title, operation.targetUri]
      .some((value) => value?.toLocaleLowerCase("es").includes(normalizedFilter))));

  useEffect(() => {
    if (!selectedVariantId) {
      setSelectedVariantId(variants.find((item) => !item.variant.archived && item.variant.id !== session.read_scope.variantId)?.variant.id ?? "");
    }
  }, [selectedVariantId, session.read_scope.variantId, variants]);

  useEffect(() => {
    if (!selectedRevisionId && revisions[0]) setSelectedRevisionId(revisions[0].revisionId);
  }, [revisions, selectedRevisionId]);

  async function refresh() {
    await queryClient.invalidateQueries({ queryKey: ["world", session.world_id] });
  }

  async function run(action: () => Promise<void>) {
    setBusy(true);
    clearError();
    try {
      await action();
    } catch (value) {
      showError(value);
    } finally {
      setBusy(false);
    }
  }

  async function createVariant() {
    const name = newVariantName.trim();
    if (!name || !observed) return;
    const fromRevisionId = selectedRevision?.revisionId ?? session.read_scope.revisionId ?? observed.variant.headRevisionId;
    await run(async () => {
      await invoke<Variant>("create_variant", { input: { name, fromRevisionId } });
      setNewVariantName("");
      await refresh();
      setStatus(`Variante ${name} creada desde la versión seleccionada. Sigues escribiendo en ${session.active_variant.name}.`);
    });
  }

  async function renameActive() {
    const name = renameValue.trim();
    if (!name) return;
    await run(async () => {
      const variant = await invoke<Variant>("rename_variant", { input: { variantId: session.active_variant.id, name } });
      appActions.setSession({ ...session, active_variant: variant });
      setRenameValue("");
      await refresh();
      setStatus(`Variante renombrada a ${variant.name}.`);
    });
  }

  async function archiveVariant(summary: VariantSummary, allowReferenced = false) {
    await run(async () => {
      try {
        await invoke("archive_variant", { input: { variantId: summary.variant.id, allowReferenced } });
      } catch (value) {
        if (!allowReferenced && String(value).includes("descendants or import references")) {
          setArchiveConfirm(summary.variant.id);
          return;
        }
        throw value;
      }
      setArchiveConfirm(null);
      setComparison(null);
      await refresh();
      setStatus(`Variante ${summary.variant.name} archivada.`);
    });
  }

  async function compareSelected() {
    if (!selectedVariant) return;
    await run(async () => {
      const right: ReadScope = { variantId: selectedVariant.variant.id, revisionId: null };
      setComparison(await invoke<VariantComparison>("compare_variant_scopes", { input: { left: session.read_scope, right } }));
      setStatus(`Comparación preparada entre ${observed?.variant.name ?? "la versión observada"} y ${selectedVariant.variant.name}.`);
    });
  }

  async function prepareMerge() {
    if (!selectedVariant || selectedVariant.variant.id === session.active_variant.id || session.read_only) return;
    await run(async () => {
      const result = await invoke<MergeReviewResult>("prepare_variant_merge", {
        input: { scope: { variantId: selectedVariant.variant.id, revisionId: null } },
      });
      await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) });
      setStatus(`${result.automaticOperationIds.length} cambios independientes y ${result.decisionOperationIds.length} decisiones pendientes en Cambios.`);
    });
  }

  async function undo(entry: RevisionHistoryEntrySnapshot) {
    if (!await confirmDiscardPending("editor")) return;
    await run(async () => {
      const next = await invoke<WorldSession>("undo_revision", { input: { revisionId: entry.revisionId } });
      appActions.setSession(next);
      appActions.setWorkspaceNotice({
        kind: "info",
        title: "Cambio deshecho",
        detail: `Se creó una nueva versión que revierte “${entry.summary}”; el historial anterior permanece intacto.`,
      });
      appActions.setStructuredEditor(null);
      await refresh();
      setStatus("El cambio se deshizo creando una nueva versión.");
    });
  }

  return (
    <section className="versions-workspace min-h-0 min-w-0 overflow-auto bg-canvas p-5 [grid-column:2] [grid-row:2] lg:p-7 max-mobile:[grid-column:1] max-mobile:[grid-row:3]" aria-labelledby="versions-title">
      <header className="versions-heading flex items-start justify-between gap-4">
        <div><p className={eyebrowStyles}>Linajes editoriales del mundo</p><h1 id="versions-title" tabIndex={-1}>Versiones</h1><p>Explora alternativas, compara su canon y prepara cambios sin alterar la variante de origen.</p></div>
        <dl className="versions-scope-summary m-0 flex min-w-[min(100%,25rem)] items-stretch justify-between gap-4 border border-l-[0.3rem] border-line border-l-accent bg-raised px-4 py-3 max-mobile:flex-col [&>div]:min-w-0 [&_dd]:mt-0.5 [&_dd]:break-words [&_dt]:text-xs [&_dt]:font-bold [&_dt]:uppercase [&_dt]:tracking-wide [&_dt]:text-muted">
          <div><dt>Escribiendo en</dt><dd>{session.active_variant.name}</dd></div>
          <div><dt>Viendo</dt><dd>{session.read_only ? `${observed?.variant.name ?? "Otra variante"} · solo lectura` : "Versión actual"}</dd></div>
        </dl>
      </header>

      {(summaries.isLoading || history.isLoading) && <p className={noticeStyles({ tone: "info" })}>Cargando versiones…</p>}
      {(summaries.isError || history.isError) && <p className={noticeStyles({ tone: "warning" })} role="alert">No se pudo cargar el historial de versiones. Reintenta sin cerrar el mundo.</p>}

      <section className={versionsSectionStyles} aria-labelledby="variant-list-title">
        <div className="versions-section-heading flex items-start justify-between gap-4"><div><p className={eyebrowStyles}>Variantes</p><h2 id="variant-list-title">Líneas del mundo</h2></div>{session.read_only && <button type="button" onClick={() => void run(async () => { await viewActiveVersion(); await refresh(); })}>Volver a la versión actual</button>}</div>
        <div className="variant-card-grid grid grid-cols-2 gap-3 max-mobile:grid-cols-1">
          {variants.map((summary) => {
            const isActive = summary.variant.id === session.active_variant.id;
            const isObserved = summary.variant.id === session.read_scope.variantId;
            return (
              <article className={cn("variant-card min-w-0 grid gap-3 rounded-xl border border-line border-t-4 border-t-accent bg-raised p-4", summary.variant.archived && "archived border-t-line-strong bg-raised")} key={summary.variant.id}>
                <header className="flex items-center justify-between gap-4"><div><h3>{summary.variant.name}</h3><div className="badge-row flex flex-wrap items-center gap-1.5">{isActive && <span className={chipStyles({ tone: "success" })}>Activa para escribir</span>}{isObserved && <span className={chipStyles({ tone: "info" })}>En pantalla</span>}{summary.variant.archived && <span className={chipStyles({ tone: "perspective" })}>Archivada</span>}</div></div></header>
                <dl className="variant-card-meta grid gap-0 [&>div]:grid [&>div]:grid-cols-[minmax(7rem,0.4fr)_minmax(0,1fr)] [&>div]:gap-4 [&>div]:border-b [&>div]:border-line [&>div]:py-2 [&>div]:text-sm [&_dd]:min-w-0 [&_dd]:break-words [&_dt]:text-muted">
                  <div><dt>Origen</dt><dd>{summary.originVariantName ? `${summary.originVariantName}: ${summary.originSummary}` : "Variante inicial del proyecto"}</dd></div>
                  <div><dt>Creada</dt><dd>{formatTimestamp(summary.originCreatedAtMs)}</dd></div>
                  <div><dt>Última versión</dt><dd>{summary.latestSummary}</dd></div>
                  <div><dt>Actualizada</dt><dd>{formatTimestamp(summary.latestCreatedAtMs)}</dd></div>
                </dl>
                {!summary.variant.archived && <div className="variant-card-actions flex flex-wrap items-center gap-2">{!isActive && <button type="button" disabled={busy} onClick={() => void run(async () => { if (await switchWritingVariant(summary.variant.id)) await refresh(); })}>Escribir aquí</button>}{!isObserved && <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={busy} onClick={() => void run(async () => { await observeScope({ variantId: summary.variant.id, revisionId: null }); await refresh(); })}>Ver última versión</button>}{!isActive && (archiveConfirm === summary.variant.id ? <><p>Esta línea tiene descendientes o importaciones. Archivarla no los elimina.</p><button type="button" className={buttonStyles({ variant: "danger" })} onClick={() => void archiveVariant(summary, true)}>Confirmar archivo</button><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setArchiveConfirm(null)}>Cancelar</button></> : <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => void archiveVariant(summary)}>Archivar</button>)}</div>}
              </article>
            );
          })}
        </div>
        <div className="version-admin-grid grid grid-cols-2 gap-3 max-mobile:grid-cols-1">
          <form className="grid gap-3 rounded-xl border border-line bg-raised p-4" onSubmit={(event) => { event.preventDefault(); void createVariant(); }}><h3>Crear desde la versión seleccionada</h3><p>Origen: {selectedRevision?.summary ?? observed?.latestSummary ?? "Versión actual"}. La variante donde escribes no cambiará.</p><label>Nombre<input name="new-variant-name" autoComplete="off" value={newVariantName} maxLength={80} required onChange={(event) => setNewVariantName(event.currentTarget.value)} /></label><button disabled={busy} type="submit">Crear variante</button></form>
          <form className="grid gap-3 rounded-xl border border-line bg-raised p-4" onSubmit={(event) => { event.preventDefault(); void renameActive(); }}><h3>Renombrar línea activa</h3><p>Actualmente: {session.active_variant.name}</p><label>Nuevo nombre<input name="active-variant-name" autoComplete="off" value={renameValue} maxLength={80} required onChange={(event) => setRenameValue(event.currentTarget.value)} /></label><button disabled={busy} className={buttonStyles({ variant: "secondary" })} type="submit">Renombrar</button></form>
        </div>
      </section>

      <section className={versionsSectionStyles} aria-labelledby="comparison-title">
        <div className="versions-section-heading flex items-start justify-between gap-4"><div><p className={eyebrowStyles}>Comparar y traer</p><h2 id="comparison-title">Dos líneas, una decisión explícita</h2></div></div>
        <div className="comparison-controls flex flex-wrap items-end gap-3"><label>Comparar con<select value={selectedVariantId} onChange={(event) => { setSelectedVariantId(event.currentTarget.value); setComparison(null); }}>{variants.filter((item) => !item.variant.archived && item.variant.id !== session.read_scope.variantId).map((item) => <option key={item.variant.id} value={item.variant.id}>{item.variant.name}</option>)}</select></label><button type="button" className={buttonStyles({ variant: "secondary" })} disabled={!selectedVariant || busy} onClick={() => void compareSelected()}>Mostrar diferencias</button></div>
        {selectedVariant && <div className="merge-intent mt-3 flex items-center justify-between gap-4 border-l-4 border-l-accent"><div><strong>Traer cambios de {selectedVariant.variant.name} hacia {session.active_variant.name}</strong><p>{session.active_variant.name} recibirá una propuesta revisable. {selectedVariant.variant.name} permanecerá sin cambios.</p></div><button type="button" disabled={busy || session.read_only || selectedVariant.variant.id === session.active_variant.id} onClick={() => void prepareMerge()}>Preparar en Cambios</button></div>}
        {session.read_only && <p className={noticeStyles({ tone: "warning" })}>Estás viendo una versión de solo lectura. Vuelve a la versión actual antes de preparar cambios.</p>}
        {comparison && <div className="variant-comparison mt-4 grid gap-3" aria-live="polite"><header className="comparison-side-headings grid grid-cols-2 gap-3 max-mobile:grid-cols-1"><div><span>Versión observada</span><strong>{observed?.variant.name}</strong></div><div><span>Versión comparada</span><strong>{selectedVariant?.variant.name}</strong></div></header>{comparison.differences.length === 0 ? <p className={emptyStateStyles}>No hay diferencias entre estas versiones.</p> : comparison.differences.map((difference) => <article className="comparison-difference grid gap-3 rounded-xl border border-line bg-raised p-4" key={objectUri(difference.objectRef)}><header><div><p className={eyebrowStyles}>{humanize(difference.kind)}</p><h3>{differenceTitle(difference)}</h3></div><span>{difference.affectedReferences.length} referencia{difference.affectedReferences.length === 1 ? "" : "s"} afectada{difference.affectedReferences.length === 1 ? "" : "s"}</span></header><div className="comparison-fields grid grid-cols-2 gap-3 max-mobile:grid-cols-1">{changedFields(difference).map((field) => <div className="comparison-field grid gap-2 rounded-xl border border-line bg-canvas p-4" key={field.key}><h4>{humanize(field.key)}</h4><p><span>{observed?.variant.name}</span>{field.before}</p><p><span>{selectedVariant?.variant.name}</span>{field.after}</p></div>)}</div><div className="comparison-provenance mt-3 grid grid-cols-2 gap-2 text-sm text-muted max-mobile:grid-cols-1"><p><strong>{observed?.variant.name}:</strong> {sourceLabel(difference.leftSource)}</p><p><strong>{selectedVariant?.variant.name}:</strong> {sourceLabel(difference.rightSource)}</p></div>{difference.affectedReferences.length > 0 && <div className="comparison-references my-3 flex flex-wrap gap-2 [&>strong]:basis-full [&>span]:break-words [&>span]:border [&>span]:border-line [&>span]:bg-raised [&>span]:px-2 [&>span]:py-1"><strong>Referencias afectadas</strong>{difference.affectedReferences.map((reference) => <span key={objectUri(reference)}>{affectedReferenceLabel(reference)}</span>)}</div>}<div className="variant-card-actions flex flex-wrap items-center gap-2"><button type="button" className={buttonStyles({ variant: "ghost" })} disabled={difference.before === null} onClick={() => void selectUriInScope(objectUri(difference.objectRef), difference.leftScope)}>Abrir en {observed?.variant.name}</button><button type="button" className={buttonStyles({ variant: "ghost" })} disabled={difference.after === null} onClick={() => void selectUriInScope(objectUri(difference.objectRef), difference.rightScope)}>Abrir en {selectedVariant?.variant.name}</button></div></article>)}</div>}
      </section>

      <section className={versionsSectionStyles} aria-labelledby="history-title">
        <div className="versions-section-heading flex items-start justify-between gap-4"><div><p className={eyebrowStyles}>Historia editorial</p><h2 id="history-title">Versiones de {observed?.variant.name ?? session.active_variant.name}</h2><p>Deshacer solo está disponible para el último cambio lógico de la línea activa. Siempre crea una versión nueva y nunca borra historia.</p></div><label>Filtrar por objeto o resumen<input type="search" value={historyFilter} onChange={(event) => setHistoryFilter(event.currentTarget.value)} /></label></div>
        {revisions.length === 0 ? <p className={emptyStateStyles}>Esta línea todavía no tiene cambios aplicados.</p> : <div className="revision-layout grid grid-cols-[minmax(16rem,0.65fr)_minmax(0,1.35fr)] gap-4 max-mobile:grid-cols-1"><ol className="revision-list grid max-h-[60dvh] gap-2 overflow-auto">{filteredRevisions.map((entry) => <li key={entry.revisionId}><button type="button" aria-current={entry.revisionId === selectedRevision?.revisionId ? "true" : undefined} onClick={() => setSelectedRevisionId(entry.revisionId)}><strong>{entry.summary}</strong><span>{formatTimestamp(entry.createdAtMs)} · {entry.operations.length} objeto{entry.operations.length === 1 ? "" : "s"}</span><span>{entry.isCurrentHead ? "Versión actual" : entry.isCurrentUndoTarget ? "Se puede deshacer" : entry.undoneRevisionId ? "Versión inversa" : "Versión anterior"}</span></button></li>)}</ol>{selectedRevision && <article className="revision-detail min-w-0"><header><div><p className={eyebrowStyles}>{humanize(selectedRevision.author)}</p><h3>{selectedRevision.summary}</h3><p>{formatTimestamp(selectedRevision.createdAtMs)} · {selectedRevision.operations.length} operación{selectedRevision.operations.length === 1 ? "" : "es"}</p></div><button type="button" className={buttonStyles({ variant: "secondary" })} disabled={!selectedRevision.isCurrentUndoTarget || busy} onClick={() => void undo(selectedRevision)}>Deshacer creando otra versión</button></header>{selectedRevision.undoneRevisionId && <p className={noticeStyles({ tone: "info" })}>Esta versión invierte un cambio anterior y conserva ambos estados en la historia.</p>}{selectedRevision.operations.map((operation) => <AuditOperation key={operation.operationId} operation={operation} revisionId={selectedRevision.revisionId} variantId={session.read_scope.variantId} />)}</article>}</div>}
      </section>
    </section>
  );
}
