import { useDeferredValue, useState } from "react";
import { humanize } from "./helpers.js";
import { useSession } from "./session-provider.js";
import type { TimelineEventEntry } from "./types.js";
import type { ProposalTemplate } from "./assistant-workspace.js";
import { buttonStyles, chipStyles, cn, noticeStyles } from "./ui-styles.js";
import { useWorkspaceData } from "./workspace-data.js";

type TimelineDensity = "compact" | "comfortable";

const timelineDensityStyles: Record<TimelineDensity, string> = {
  compact: "timeline-compact [&_.timeline-event-card]:py-2",
  comfortable: "timeline-comfortable",
};

export function WorldTimeline({ onOpen, onConfigureCalendar, onCreateEvent, onUseTemplate }: { onOpen: (uri: string) => Promise<void>; onConfigureCalendar: () => void; onCreateEvent: () => void; onUseTemplate: (template: ProposalTemplate) => void }) {
  const session = useSession();
  const { timeline } = useWorkspaceData();
  const [query, setQuery] = useState("");
  const [density, setDensity] = useState<TimelineDensity>("comfortable");
  const deferredQuery = useDeferredValue(query.trim().toLocaleLowerCase("es"));
  if (!session) return null;

  const filter = (events: TimelineEventEntry[]) => events.filter((event) =>
    !deferredQuery || event.summary.toLocaleLowerCase("es").includes(deferredQuery)
      || event.kind.toLocaleLowerCase("es").includes(deferredQuery));
  const known = filter(timeline.data?.known ?? []);
  const unknown = filter(timeline.data?.unknown ?? []);
  return (
    <section className={cn("world-timeline min-h-0 min-w-0 overflow-auto bg-canvas p-5 [grid-column:2] [grid-row:2] lg:p-7 max-mobile:[grid-column:1] max-mobile:[grid-row:3]", timelineDensityStyles[density])} aria-labelledby="timeline-page-title">
      <header className="timeline-page-heading flex items-start justify-between gap-4">
        <div><p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Tiempo del mundo</p><h1 id="timeline-page-title" tabIndex={-1}>Cronología</h1><p>Los acontecimientos se ordenan por su unidad temporal canónica. El tiempo no especificado permanece separado.</p></div>
        <div className="timeline-controls grid min-w-[min(100%,25rem)] grid-cols-[minmax(0,1fr)_8rem] gap-2.5 [&>button]:col-span-full [&>button]:justify-self-start">
          <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={session.read_only} onClick={onConfigureCalendar}>{session.read_only ? "Configurar calendario (solo lectura)" : "Configurar calendario"}</button>
          <label>Filtrar <input type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Resumen o tipo" /></label>
          <label>Densidad <select value={density} onChange={(event) => setDensity(event.currentTarget.value as typeof density)}><option value="comfortable">Cómoda</option><option value="compact">Compacta</option></select></label>
        </div>
      </header>
      {timeline.isPending && <p role="status">Cargando cronología…</p>}
      {timeline.isError && <p role="alert" className={noticeStyles({ tone: "warning" })}>No se pudo cargar la cronología. El mundo no cambió.</p>}
      {!timeline.isPending && !timeline.isError && known.length === 0 && unknown.length === 0 && !deferredQuery && (
        <section className="empty-state grid gap-4 rounded-xl border border-dashed border-line bg-surface p-5 text-sm text-muted">
          <h2>La cronología todavía no tiene acontecimientos</h2>
          <p>Crea el primer evento manualmente o prepara una secuencia breve para revisión.</p>
          <div className="pending-actions flex flex-wrap items-center gap-2">
            <button type="button" disabled={session.read_only} onClick={onCreateEvent}>Crear evento</button>
            <button type="button" className={buttonStyles({ variant: "secondary" })} disabled={session.read_only} onClick={() => onUseTemplate("chronology")}>Usar plantilla Cronología</button>
          </div>
        </section>
      )}
      {!timeline.isPending && !timeline.isError && known.length === 0 && unknown.length === 0 && deferredQuery && <p className="empty-state grid gap-4 rounded-xl border border-dashed border-line bg-surface p-5 text-sm text-muted">No hay acontecimientos con este filtro. La cronología existente no cambió.</p>}
      <TimelineLane title="Tiempo conocido" events={known} onOpen={onOpen} hasCalendar={Boolean(timeline.data?.calendarName)} />
      <TimelineLane title="Tiempo no especificado" events={unknown} onOpen={onOpen} unknown />
    </section>
  );
}

function TimelineLane({ title, events, onOpen, unknown = false, hasCalendar = false }: { title: string; events: TimelineEventEntry[]; onOpen: (uri: string) => Promise<void>; unknown?: boolean; hasCalendar?: boolean }) {
  if (events.length === 0) return null;
  return (
    <section className={cn("timeline-lane mt-6", unknown && "timeline-unknown")}>
      <h2 className="text-base">{title}</h2>
      <ol className={cn("m-0 grid list-none gap-2.5 border-l-2 border-line py-0 pl-4", unknown && "border-dashed")}>
        {events.map((event) => (
          <li key={event.uri}>
            <button type="button" className="timeline-event-card mt-3 grid gap-3 rounded-xl border border-line bg-raised p-4" onClick={() => void onOpen(event.uri)}>
              <span className="timeline-date font-mono text-xs text-muted">{event.startCalendar?.label ?? (unknown ? "Sin fecha" : hasCalendar ? "Fecha fuera del rango de presentación" : "Tiempo conocido sin calendario de presentación")}</span>
              <strong>{event.summary}</strong>
              <span className="badge-row flex flex-wrap items-center gap-1.5"><span className={chipStyles({ tone: "kind" })}>{humanize(event.kind)}</span><span className={chipStyles({ tone: "perspective" })}>{timeLabel(event)}</span></span>
            </button>
          </li>
        ))}
      </ol>
    </section>
  );
}

function timeLabel(event: TimelineEventEntry): string {
  if (event.time.kind === "ongoing") return "En curso";
  if (event.time.certainty.includes("approximate")) return "Aproximado";
  if (event.time.certainty.includes("uncertain")) return "Incierto";
  return humanize(event.time.kind);
}
