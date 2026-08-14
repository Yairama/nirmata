import { invoke } from "@tauri-apps/api/core";
import * as Dialog from "@radix-ui/react-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import {
  clearError,
  firstReviewIssue,
  humanize,
  labelForUri,
  objectKindFromUri,
  setStatus,
  validationIssueMessage,
} from "./helpers.js";
import { appActions, beginAiActivity, endAiActivity, getAppState, useAppState } from "./state.js";
import type {
  AiRunSnapshot,
  ManualReviewActionRequest,
  ManualReviewDecisionPointSnapshot,
  ManualReviewObjectSnapshot,
  ManualReviewOperationSnapshot,
  ManualReviewSnapshot,
  PendingReviewOrigin,
  PendingReviewSnapshot,
  ValidationIssue,
  ValidationReport,
  WorldSession,
} from "./types.js";
import {
  applyCommandStateError,
  confirmDiscardPending,
  selectUri,
  startCreatingObject,
} from "./workspace.js";
import { Icon } from "./icons.js";
import { buttonStyles, chipStyles, cn, noticeStyles } from "./ui-styles.js";

type PendingReviewsProps = {
  open: boolean;
  onClose: () => void;
  onCountChange: (count: number) => void;
  onEdit: (record: PendingReviewSnapshot, operation: ManualReviewOperationSnapshot) => void;
  onStartProposal: () => void;
  onOpenImports: () => void;
  onOpenWorld: () => void;
};

const originLabels: Record<PendingReviewOrigin, string> = {
  manual: "Edición manual",
  ai: "IA",
  lore_import: "Importación",
  simulation: "Simulación",
  versions_merge: "Versiones",
  snapshot: "Snapshot",
};

const issueGroups: Array<[string, keyof ValidationReport]> = [
  ["Errores", "errors"],
  ["Conflictos", "conflicts"],
  ["Advertencias", "warnings"],
  ["Información", "info"],
];

export function pendingReviewsQueryKey(session: WorldSession) {
  return ["world", session.world_id, session.active_variant.id, session.current_revision, "pending-reviews"] as const;
}

function manualReviewQueryKey(session: WorldSession, reviewKey: string) {
  return [...pendingReviewsQueryKey(session), "manual-review", reviewKey] as const;
}

export function PendingReviews({ open, onClose, onCountChange, onEdit, onStartProposal, onOpenImports, onOpenWorld }: PendingReviewsProps) {
  const session = useAppState().session;
  const queryClient = useQueryClient();
  const pending = useQuery({
    queryKey: session ? pendingReviewsQueryKey(session) : ["world", "closed", "pending-reviews"],
    queryFn: () => invoke<PendingReviewSnapshot[]>("list_pending_reviews"),
    enabled: Boolean(session),
    retry: false,
    staleTime: 0,
  });

  useEffect(() => {
    onCountChange(pending.data?.length ?? 0);
  }, [onCountChange, pending.data?.length]);

  if (!session) return null;
  return (
    <ReviewDrawer
      open={open}
      onClose={onClose}
      records={pending.data ?? []}
      loading={pending.isPending}
      failed={pending.isError}
      onRetry={() => void pending.refetch()}
      onEdit={onEdit}
      onStartProposal={onStartProposal}
      onOpenImports={onOpenImports}
      onOpenWorld={onOpenWorld}
    />
  );
}

export function ReviewDrawer({ open, onClose, records, loading, failed, onRetry, onEdit, onStartProposal, onOpenImports, onOpenWorld }: {
  open: boolean;
  onClose: () => void;
  records: PendingReviewSnapshot[];
  loading: boolean;
  failed: boolean;
  onRetry: () => void;
  onEdit: PendingReviewsProps["onEdit"];
  onStartProposal: () => void;
  onOpenImports: () => void;
  onOpenWorld: () => void;
}) {
  const sorted = [...records].sort((left, right) => left.title.localeCompare(right.title, "es"));
  useEffect(() => {
    if (!open) return;
    const root = document.getElementById("root");
    if (!root) return;
    root.inert = true;
    return () => { root.inert = false; };
  }, [open]);
  return (
    <Dialog.Root open={open} onOpenChange={(value) => { if (!value) onClose(); }}>
      <Dialog.Portal>
      <Dialog.Overlay className="review-drawer-backdrop fixed inset-0 top-16 z-40 bg-overlay max-mobile:top-0" />
      <Dialog.Content
        asChild
        aria-describedby={undefined}
        onInteractOutside={(event) => {
          if ((event.target as HTMLElement).closest(".global-feedback")) event.preventDefault();
        }}
      >
      <section id="pending-panel" className="panel fixed bottom-0 right-0 top-16 z-50 flex min-h-0 w-[min(64rem,calc(100vw-5rem))] min-w-0 flex-col overflow-hidden border-l border-line bg-surface shadow-overlay outline-none max-mobile:inset-0 max-mobile:w-full max-mobile:border-l-0" aria-labelledby="pending-title" aria-modal="true">
        <div className="panel-header flex min-h-16 items-start justify-between gap-4 border-b border-line px-4 py-3 pr-16 max-mobile:pr-14">
          <div className="grid gap-1"><p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Revisión manual</p><Dialog.Title asChild><h3 id="pending-title">Cambios pendientes</h3></Dialog.Title></div>
          <p className="panel-summary text-xs font-medium text-muted">{sorted.length === 0 ? "Sin cambios pendientes." : `${sorted.length} propuesta${sorted.length === 1 ? "" : "s"} pendiente${sorted.length === 1 ? "" : "s"}`}</p>
        </div>
        <Dialog.Close asChild><button type="button" className={cn("review-drawer-close absolute right-4 top-3.5 z-10", buttonStyles({ variant: "icon" }))} aria-label="Cerrar cambios" title="Cerrar cambios"><Icon name="x" /></button></Dialog.Close>
        <div className="panel-body min-h-0 flex-1 overflow-auto p-4">
          {loading && <p role="status" className={noticeStyles({ tone: "info" })}>Recuperando revisiones guardadas…</p>}
          {failed && <section className={noticeStyles({ tone: "warning" })} role="alert"><h4 className="font-semibold">No se pudieron cargar los cambios</h4><p>Las revisiones siguen guardadas. Reintenta sin cerrar el mundo.</p><button type="button" className={buttonStyles({ variant: "secondary" })} onClick={onRetry}>Reintentar</button></section>}
          {!loading && !failed && sorted.length === 0 && <EmptyReviews onStartProposal={onStartProposal} onOpenImports={onOpenImports} onOpenWorld={onOpenWorld} />}
          {sorted.length > 0 && <div className="pending-list grid gap-4">{sorted.map((record) => <PendingReviewCard key={record.review.reviewKey} record={record} open={open} onEdit={onEdit} />)}</div>}
        </div>
      </section>
      </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function EmptyReviews({ onStartProposal, onOpenImports, onOpenWorld }: { onStartProposal: () => void; onOpenImports: () => void; onOpenWorld: () => void }) {
  const session = useAppState().session;
  return (
    <section className="empty-state grid gap-4 rounded-xl border border-dashed border-line bg-surface p-5 text-sm text-muted">
      <p>No hay cambios pendientes. Puedes preparar una edición manual, proponer con IA o importar material; ninguna acción aplica canon por sí sola.</p>
      <div className="pending-actions flex flex-wrap items-center gap-2">
        <button type="button" disabled={Boolean(session?.read_only)} onClick={() => void startCreatingObject("entity").then((opened) => { if (opened) onOpenWorld(); })}>Crear cambio manual</button>
        <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={Boolean(session?.read_only)} onClick={onStartProposal}>Proponer con IA</button>
        <button type="button" className={buttonStyles({ variant: "ghost" })} onClick={onOpenImports}>Importar material</button>
      </div>
    </section>
  );
}

function PendingReviewCard({ record, open, onEdit }: { record: PendingReviewSnapshot; open: boolean; onEdit: PendingReviewsProps["onEdit"] }) {
  const session = getAppState().session!;
  const queryClient = useQueryClient();
  const [discardArmed, setDiscardArmed] = useState(false);
  const reviewQuery = useQuery({
    queryKey: manualReviewQueryKey(session, record.review.reviewKey),
    queryFn: () => invoke<ManualReviewSnapshot>("read_manual_review", { input: { reviewKey: record.review.reviewKey } }),
    enabled: open,
    initialData: record.review,
    retry: false,
    staleTime: 0,
  });
  const review = reviewQuery.data;

  async function refreshReview(next?: ManualReviewSnapshot) {
    if (next) queryClient.setQueryData(manualReviewQueryKey(session, review.reviewKey), next);
    await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) });
  }

  const action = useMutation({
    mutationFn: (request: ManualReviewActionRequest) => invoke<ManualReviewSnapshot>("apply_manual_review_action", {
      input: { reviewKey: review.reviewKey, action: request },
    }),
    onSuccess: async (next) => {
      await refreshReview(next);
      setStatus("Revisión actualizada.");
    },
    onError: (value) => applyCommandStateError(value, "La propuesta se conservó para reintentar."),
  });
  const revalidate = useMutation({
    mutationFn: () => invoke<ManualReviewSnapshot>("revalidate_manual_review", { input: { reviewKey: review.reviewKey } }),
    onSuccess: async (next) => {
      await refreshReview(next);
      setStatus(next.freshness.status === "current" ? "Revisión manual revalidada." : "La revisión volvió a cambiar durante el refresco.");
    },
    onError: (value) => applyCommandStateError(value, "La propuesta se conservó para reintentar."),
  });
  const discard = useMutation({
    mutationFn: async () => {
      if (record.aiRunId) await invoke("discard_ai_run", { runId: record.aiRunId });
      else await invoke("discard_manual_review", { input: { reviewKey: review.reviewKey } });
    },
    onSuccess: async () => {
      if (getAppState().structuredEditor?.reviewEdit?.reviewKey === review.reviewKey) appActions.setStructuredEditor(null);
      await refreshReview();
      setStatus("Propuesta descartada. El mundo no cambió.");
    },
    onError: (value) => applyCommandStateError(value, "No se pudo descartar la propuesta. La tarjeta se conservó."),
  });
  const apply = useMutation({
    mutationFn: async () => {
      if (!await confirmDiscardPending("editor")) throw new Error("review_apply_cancelled");
      clearError();
      setStatus("Aplicando conjunto de cambios…");
      return invoke<WorldSession>("confirm_manual_review", { input: { reviewKey: review.reviewKey } });
    },
    onSuccess: async (next) => {
      appActions.setSession(next);
      appActions.setWorkspaceNotice({ kind: "info", title: "Cambios aplicados", detail: `La revisión ${next.current_revision} quedó aplicada en una única transacción.` });
      const targetUri = review.operations[0]?.targetUri ?? review.reviewKey;
      const deleted = review.operations[0]?.before !== null && review.operations[0]?.after === null;
      if (deleted) appActions.forgetUri(targetUri);
      else appActions.selectUri(targetUri);
      await queryClient.invalidateQueries({ queryKey: ["world", next.world_id] });
      setStatus(`Cambios aplicados: ${record.title}.`);
    },
    onError: (value) => {
      if (value instanceof Error && value.message === "review_apply_cancelled") return;
      applyCommandStateError(value, "El canon no cambió. La propuesta se conservó y puedes reintentar.");
    },
  });

  const targetUri = review.operations[0]?.targetUri ?? review.reviewKey;
  const deleting = review.operations[0]?.before !== null && review.operations[0]?.after === null;
  const objectType = review.operations[0]?.after?.objectType ?? review.operations[0]?.before?.objectType ?? objectKindFromUri(targetUri) ?? "world";
  const firstIssue = firstReviewIssue(review.effectiveReport);
  const canApply = review.readyToConfirm && review.freshness.status === "current" && !session.read_only;
  const busy = action.isPending || revalidate.isPending || discard.isPending || apply.isPending;
  return (
    <article className="pending-card grid gap-4 rounded-2xl border border-line bg-raised p-5 shadow-sm">
      <h4 className="font-serif text-xl">{record.title}</h4>
      <div className="badge-row flex flex-wrap items-center gap-1.5">
        <span className={chipStyles({ tone: "perspective" })}>{originLabels[record.origin]}</span>
        <span className={chipStyles({ tone: "kind" })}>{humanize(objectType)}</span>
        <span className={chipStyles({ tone: deleting ? "error" : "info" })}>{deleting ? "Eliminar" : record.editorRequest.existingUri ? "Modificar" : "Crear"}</span>
        <span className={chipStyles({ tone: review.freshness.status === "current" ? "success" : "warning" })}>{review.freshness.status === "current" ? "Revisión vigente" : "Propuesta desactualizada"}</span>
        <span className={chipStyles({ tone: review.readyToConfirm ? "success" : "warning" })}>{review.readyToConfirm ? "Listo para confirmar" : "Bloqueado"}</span>
      </div>
      {record.merge ? <><p className="muted text-muted">{record.merge.sourceName} permanece sin cambios · destino: {record.merge.destinationName}</p><div className="badge-row flex flex-wrap items-center gap-1.5"><span className={chipStyles({ tone: "success" })}>{record.merge.automaticCount} cambios independientes</span><span className={chipStyles({ tone: record.merge.decisionCount > 0 ? "warning" : "info" })}>{record.merge.decisionCount} decisiones manuales</span></div></> : <details className="technical-details border-t border-line pt-3"><summary>Detalles técnicos</summary><p className="muted text-muted">{targetUri} · base {review.baseRevision}</p></details>}
      <p>{record.merge ? `${record.merge.destinationName} recibirá una propuesta revisable; ${record.merge.sourceName} permanecerá sin cambios.` : review.objective}</p>
      {review.freshness.status !== "current" && <section className={noticeStyles({ tone: "warning" })}><h4 className="font-semibold">{review.freshness.status === "refresh_restart_required" ? "Revalidación interrumpida" : "Propuesta desactualizada"}</h4><p>{review.freshness.message}</p>{review.freshness.canRevalidate && <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={busy || session.read_only} onClick={() => revalidate.mutate()}>{revalidate.isPending ? "Comprobando…" : "Actualizar y volver a comprobar"}</button>}</section>}
      {review.sources.length > 0 && <ReviewLinks label="Fuentes" uris={review.sources} review={review} />}
      {firstIssue && <section className={noticeStyles({ tone: firstIssue.severity === "error" ? "warning" : "info" })}><h4 className="font-semibold">{review.readyToConfirm ? "Revisión validada" : "Confirmación deshabilitada"}</h4><p>{validationIssueMessage(firstIssue)}</p></section>}
      {record.aiRunId && <AiFinalCritique runId={record.aiRunId} reviewKey={review.reviewKey} open={open} />}
      {review.operations.map((operation) => <ReviewOperation key={operation.operationId} record={record} review={review} operation={operation} busy={busy} onAction={(request) => action.mutateAsync(request)} onEdit={() => onEdit(record, operation)} />)}
      {!discardArmed ? <div className="pending-actions flex flex-wrap items-center gap-2"><button type="button" disabled={!canApply || busy} title={applyDisabledReason(review, session.read_only)} onClick={() => apply.mutate()}>{apply.isPending ? "Aplicando…" : "Aplicar al mundo"}</button><button type="button" className={buttonStyles({ variant: "secondary" })} disabled={busy} onClick={() => setDiscardArmed(true)}>Descartar propuesta</button></div> : <section className={noticeStyles({ tone: "warning" })}><p>¿Descartar “{record.title}”? El mundo no cambiará.</p><div className="pending-actions flex flex-wrap items-center gap-2"><button type="button" className={buttonStyles({ variant: "danger" })} autoFocus disabled={discard.isPending} onClick={() => discard.mutate()}>{discard.isPending ? "Descartando propuesta…" : "Sí, descartar propuesta"}</button><button type="button" className={buttonStyles({ variant: "secondary" })} disabled={discard.isPending} onClick={() => setDiscardArmed(false)}>Conservar propuesta</button></div></section>}
    </article>
  );
}

function ReviewOperation({ record, review, operation, busy, onAction, onEdit }: {
  record: PendingReviewSnapshot;
  review: ManualReviewSnapshot;
  operation: ManualReviewOperationSnapshot;
  busy: boolean;
  onAction: (action: ManualReviewActionRequest) => Promise<ManualReviewSnapshot>;
  onEdit: () => void;
}) {
  const [judgmentOpen, setJudgmentOpen] = useState(false);
  const [judgment, setJudgment] = useState(operation.risk.judgment ?? "");
  const [waiverIssue, setWaiverIssue] = useState<ValidationIssue | null>(null);
  const [rationale, setRationale] = useState("");
  const title = operation.after?.title ?? operation.before?.title ?? "Objeto modificado";
  const deleting = operation.before !== null && operation.after === null;

  async function submitJudgment(event: FormEvent) {
    event.preventDefault();
    if (!judgment.trim()) return;
    await onAction({ kind: "record_judgment", operationId: operation.operationId, judgment: judgment.trim() });
    setJudgmentOpen(false);
  }

  async function submitWaiver(event: FormEvent) {
    event.preventDefault();
    if (!waiverIssue || !rationale.trim()) return;
    await onAction({ kind: "add_waiver", operationId: operation.operationId, issueCode: waiverIssue.code, rationale: rationale.trim() });
    setWaiverIssue(null);
    setRationale("");
  }

  return (
    <section className="review-operation-card grid gap-4 rounded-xl border border-line bg-surface p-4" aria-labelledby={`review-operation-${operation.operationId}`}>
      <h5 id={`review-operation-${operation.operationId}`}>{title}</h5>
      <div className="badge-row flex flex-wrap items-center gap-1.5"><span className={chipStyles({ tone: operation.severity })}>{humanize(operation.severity)}</span><span className={chipStyles({ tone: operation.selected ? "success" : "warning" })}>{operation.selected ? "Seleccionada" : "Excluida"}</span><span className={chipStyles({ tone: "info" })}>{humanize(operation.decision)}</span></div>
      <div className="review-object-grid grid grid-cols-2 gap-3 max-workspace:grid-cols-1"><ReviewObject title="Antes" snapshot={operation.before} /><ReviewObject title="Después" snapshot={operation.after} /></div>
      {operation.dependencies.length > 0 && <section className="review-issue-group grid gap-3 border-t border-line pt-4"><h5>Dependencias</h5><ReviewLinks label="Dependencias" uris={operation.dependencies} review={review} /></section>}
      {operation.decisionPoints.length > 0 && <section className="review-issue-group grid gap-3 border-t border-line pt-4"><h5>Decisiones pendientes</h5><div className="warning-list grid gap-2 pl-5 text-sm">{operation.decisionPoints.map((decision) => <DecisionPoint key={decision.decisionPointId} record={record} operationTitle={title} deleting={deleting} decision={decision} busy={busy} onAction={onAction} />)}</div></section>}
      {operation.risk.triggers.length > 0 && <section className="review-issue-group grid gap-3 border-t border-line pt-4"><h5>Fricción de alto riesgo</h5><ul className="review-issue-list grid gap-2 pl-5 text-sm">{operation.risk.triggers.map((trigger) => <li key={trigger.code}>{trigger.title}: {trigger.detail}</li>)}</ul><p className="muted text-muted">{operation.risk.judgment ? `Juicio registrado: ${operation.risk.judgment}` : operation.risk.suggestedResolutionAvailable ? "Debes registrar un juicio breve antes de ver la resolución sugerida." : "Registra un juicio breve para dejar constancia antes de confirmar este cambio."}</p>{!operation.risk.judgment && !judgmentOpen && <button type="button" className={buttonStyles({ variant: "secondary" })} onClick={() => setJudgmentOpen(true)}>Registrar juicio</button>}{judgmentOpen && <form className="inline-review-form grid gap-3 rounded-xl border border-line bg-canvas p-4" onSubmit={(event) => void submitJudgment(event)}><label htmlFor={`judgment-${operation.operationId}`}>Tu lectura antes de revelar la resolución sugerida<textarea id={`judgment-${operation.operationId}`} name="review-judgment" required autoFocus value={judgment} onChange={(event) => setJudgment(event.currentTarget.value)} /></label><div className="pending-actions flex flex-wrap items-center gap-2"><button type="submit" disabled={busy || !judgment.trim()}>Guardar juicio</button><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setJudgmentOpen(false)}>Cancelar</button></div></form>}</section>}
      {issueGroups.map(([label, key]) => operation.effectiveIssues[key].length > 0 && <section className="review-issue-group grid gap-3 border-t border-line pt-4" key={key}><h5>{label}</h5><ul className="review-issue-list grid gap-2 pl-5 text-sm">{operation.effectiveIssues[key].map((issue) => <li key={`${issue.code}-${validationIssueMessage(issue)}`}><span>{validationIssueMessage(issue)}</span>{issue.severity !== "error" && <button type="button" className={buttonStyles({ variant: "ghost" })} disabled={busy} onClick={() => { setWaiverIssue(issue); setRationale(""); }}>Aceptar advertencia con motivo</button>}</li>)}</ul>{waiverIssue && operation.effectiveIssues[key].includes(waiverIssue) && <form className="inline-review-form grid gap-3 rounded-xl border border-line bg-canvas p-4" onSubmit={(event) => void submitWaiver(event)}><label htmlFor={`waiver-${operation.operationId}-${waiverIssue.code}`}>Motivo para aceptar {waiverIssue.code}<textarea id={`waiver-${operation.operationId}-${waiverIssue.code}`} name="waiver-rationale" required autoFocus value={rationale} onChange={(event) => setRationale(event.currentTarget.value)} /></label><div className="pending-actions flex flex-wrap items-center gap-2"><button type="submit" disabled={busy || !rationale.trim()}>Guardar motivo</button><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setWaiverIssue(null)}>Cancelar</button></div></form>}</section>)}
      {operation.waivers.length > 0 && <section className="review-issue-group grid gap-3 border-t border-line pt-4"><h5>Advertencias aceptadas con motivo</h5><ul className="review-issue-list grid gap-2 pl-5 text-sm">{operation.waivers.map((waiver) => <li key={`${waiver.issueCode}-${waiver.createdAtMs}`}>{waiver.issueCode}: {waiver.rationale}</li>)}</ul></section>}
      <div className="pending-actions flex flex-wrap items-center gap-2"><button type="button" className={buttonStyles({ variant: "secondary" })} disabled={busy || operation.selected} onClick={() => void onAction({ kind: "accept", operationId: operation.operationId })}>Aceptar operación</button><button type="button" className={buttonStyles({ variant: "secondary" })} disabled={busy || !operation.selected} onClick={() => void onAction({ kind: "reject", operationId: operation.operationId })}>Rechazar operación</button>{!deleting && <button type="button" className={buttonStyles({ variant: "ghost" })} disabled={busy} onClick={onEdit}>Editar cambio</button>}{operation.before && <button type="button" className={buttonStyles({ variant: "ghost" })} title="Abre el estado vigente sin aplicar esta propuesta." onClick={() => void selectUri(operation.targetUri)}>Ver objeto actual</button>}</div>
    </section>
  );
}

function ReviewObject({ title, snapshot }: { title: string; snapshot: ManualReviewObjectSnapshot | null }) {
  if (!snapshot) return null;
  const isTechnical = (label: string) => label.startsWith("Detalles técnicos") || ["SLUG", "ID", "URI"].includes(label.toLocaleUpperCase("es"));
  const regular = snapshot.lines.filter((line) => !isTechnical(line.label));
  const technical = snapshot.lines.filter((line) => isTechnical(line.label));
  return <article className="review-object min-w-0 rounded-xl border border-line bg-canvas p-4"><h5>{title}</h5><div className="subsection-header flex items-center justify-between gap-3"><strong>{snapshot.title}</strong><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => void selectUri(snapshot.targetUri)}>Abrir</button></div><dl className="meta-list grid gap-0">{regular.map((line) => <div className="meta-row grid grid-cols-[minmax(7rem,0.4fr)_minmax(0,1fr)] gap-4 border-b border-line py-2 text-sm" key={`${line.label}-${line.value}`}><dt className="text-muted">{line.label}</dt><dd className="min-w-0 break-words">{line.label.toLocaleUpperCase("es") === "TIPO" ? humanize(line.value.toLocaleLowerCase("es")) : line.value}</dd></div>)}</dl>{technical.length > 0 && <details className="technical-details border-t border-line pt-3"><summary>Detalles técnicos</summary><dl className="meta-list grid gap-0">{technical.map((line) => <div className="meta-row grid grid-cols-[minmax(7rem,0.4fr)_minmax(0,1fr)] gap-4 border-b border-line py-2 text-sm" key={`${line.label}-${line.value}`}><dt className="text-muted">{line.label.replace("Detalles técnicos del ", "")}</dt><dd className="min-w-0 break-words">{line.value}</dd></div>)}</dl></details>}</article>;
}

function ReviewLinks({ label, uris, review }: { label: string; uris: string[]; review: ManualReviewSnapshot }) {
  return <div className="review-source-list flex flex-wrap gap-2" aria-label={label}>{uris.map((uri) => <button key={uri} type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => void selectUri(uri)}>{reviewLabelForUri(review, uri)}</button>)}</div>;
}

function DecisionPoint({ record, operationTitle, deleting, decision, busy, onAction }: { record: PendingReviewSnapshot; operationTitle: string; deleting: boolean; decision: ManualReviewDecisionPointSnapshot; busy: boolean; onAction: (action: ManualReviewActionRequest) => Promise<ManualReviewSnapshot> }) {
  return <article className={cn("warning-card", noticeStyles({ tone: "warning" }))}><h4 className="font-semibold">{record.merge ? `¿Qué versión debe conservarse para ${operationTitle}?` : deleting ? `¿Eliminar ${operationTitle} del canon?` : decision.prompt}</h4><p>{decision.suggestionHidden ? "Registra primero tu juicio para revelar la resolución sugerida." : [decision.resolvedAlternative ? `Resuelta: ${decisionAlternativeLabel(record, decision.resolvedAlternative, deleting)}` : null, decision.reason && !deleting ? `Razón: ${decision.reason}` : null].filter(Boolean).join(" · ")}</p><div className="pending-actions flex flex-wrap items-center gap-2">{decision.alternatives.map((alternative) => <button key={alternative} type="button" className={buttonStyles({ variant: decision.resolvedAlternative === alternative ? "secondary" : "ghost" })} disabled={busy || decision.suggestionHidden} onClick={() => void onAction({ kind: "resolve_decision", decisionPointId: decision.decisionPointId, alternative })}>{decisionAlternativeLabel(record, alternative, deleting)}</button>)}</div></article>;
}

function AiFinalCritique({ runId, reviewKey, open }: { runId: string; reviewKey: string; open: boolean }) {
  const session = getAppState().session!;
  const queryClient = useQueryClient();
  const [issueId, setIssueId] = useState<string | null>(null);
  const [judgment, setJudgment] = useState("");
  const run = useQuery({ queryKey: [...pendingReviewsQueryKey(session), "ai-run", runId], queryFn: () => invoke<AiRunSnapshot>("read_ai_run", { runId }), enabled: open, retry: false, staleTime: 0 });
  const refresh = async (next: AiRunSnapshot) => {
    queryClient.setQueryData([...pendingReviewsQueryKey(session), "ai-run", runId], next);
    await queryClient.invalidateQueries({ queryKey: manualReviewQueryKey(session, reviewKey) });
    await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(session) });
  };
  const critique = useMutation({
    mutationFn: async () => {
      const requestId = crypto.randomUUID();
      beginAiActivity({ requestId, source: "assistant", label: "La IA está ejecutando la crítica final." });
      try { return await invoke<AiRunSnapshot>("revalidate_ai_run", { input: { requestId, runId, anchorUri: getAppState().selectedUri } }); }
      finally { endAiActivity(requestId); }
    },
    onSuccess: refresh,
    onError: (value) => applyCommandStateError(value, "La propuesta se conservó; la crítica final puede reintentarse."),
  });
  const acknowledge = useMutation({
    mutationFn: () => invoke<AiRunSnapshot>("acknowledge_ai_critique", { input: { runId, issueId, judgment: judgment.trim() } }),
    onSuccess: async (next) => { await refresh(next); setIssueId(null); setJudgment(""); },
    onError: (value) => applyCommandStateError(value, "La propuesta se conservó para revisar el hallazgo."),
  });
  if (run.isError) return <section className={noticeStyles({ tone: "warning" })}><h4 className="font-semibold">Crítica final no disponible</h4><p>La propuesta permanece guardada y puede recuperarse desde el asistente.</p></section>;
  if (!run.data) return <section className={noticeStyles({ tone: "info" })}><h4 className="font-semibold">Crítica final</h4><p>Cargando comprobación semántica…</p></section>;
  const pendingCritique = run.data.status === "awaiting_final_critique" || run.data.status === "awaiting_review";
  return <section className={noticeStyles({ tone: pendingCritique ? "warning" : "info" })}><h4 className="font-semibold">Crítica final</h4><p>{run.data.status === "ready_to_commit" ? "La crítica final está vigente para esta propuesta." : "Ejecuta la crítica final antes de aplicar la propuesta de IA."}</p>{run.data.critiqueReport?.issues.map((issue) => <article className={cn("assistant-issue rounded-lg border-l-2 p-3 text-sm", issue.severity === "error" && "error border-danger bg-danger-soft", issue.severity === "conflict" && "conflict border-conflict bg-conflict-soft", issue.severity === "warning" && "warning border-warning bg-warning-soft", issue.severity === "info" && "info border-info bg-info-soft")} key={issue.issueId}><p>{humanize(issue.severity)}: {issue.summary.markdown}</p>{issue.evidence.map((evidence) => <button key={`${issue.issueId}-${evidence.sourceUri}`} type="button" className={buttonStyles({ variant: "ghost" })} title={evidence.excerptMd} onClick={() => void selectUri(evidence.sourceUri)}>Abrir evidencia</button>)}{issue.severity === "conflict" && issueId !== issue.issueId && <button type="button" className={buttonStyles({ variant: "secondary" })} onClick={() => setIssueId(issue.issueId)}>Registrar decisión humana</button>}{issueId === issue.issueId && <form className="inline-review-form grid gap-3 rounded-xl border border-line bg-canvas p-4" onSubmit={(event) => { event.preventDefault(); if (judgment.trim()) acknowledge.mutate(); }}><label htmlFor={`critique-${issue.issueId}`}>Por qué aceptas o corregirás este hallazgo<textarea id={`critique-${issue.issueId}`} name="critique-judgment" required autoFocus value={judgment} onChange={(event) => setJudgment(event.currentTarget.value)} /></label><div className="pending-actions flex flex-wrap items-center gap-2"><button type="submit" disabled={!judgment.trim() || acknowledge.isPending}>Guardar decisión</button><button type="button" className={buttonStyles({ variant: "ghost" })} onClick={() => setIssueId(null)}>Cancelar</button></div></form>}</article>)}{pendingCritique && <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={critique.isPending || session.read_only} onClick={() => critique.mutate()}>{critique.isPending ? "Ejecutando crítica…" : "Revalidar crítica final"}</button>}</section>;
}

function reviewLabelForUri(review: ManualReviewSnapshot, uri: string): string {
  const operation = review.operations.find((item) => item.targetUri === uri);
  return operation?.after?.title ?? operation?.before?.title ?? labelForUri(uri);
}

function decisionAlternativeLabel(record: PendingReviewSnapshot, alternative: string, deleting = false): string {
  if (deleting && alternative === "Keep current canon") return "Conservar en el canon";
  if (deleting && alternative === "Apply replacement") return "Confirmar eliminación";
  if (!record.merge) return humanize(alternative);
  if (alternative === "keep_destination") return `Conservar lo que ya existe en ${record.merge.destinationName}`;
  if (alternative === "take_source") return `Traer la versión de ${record.merge.sourceName}`;
  return humanize(alternative);
}

function applyDisabledReason(review: ManualReviewSnapshot, readOnly: boolean): string {
  if (readOnly) return "Vuelve a la versión actual antes de aplicar cambios.";
  if (review.freshness.status !== "current") return review.freshness.message;
  const issue = firstReviewIssue(review.effectiveReport);
  return review.readyToConfirm ? "" : issue ? validationIssueMessage(issue) : "Hay errores, conflictos, dependencias rotas o falta registrar juicio.";
}
