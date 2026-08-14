import { useDeferredValue, useState } from "react";
import { humanize } from "./helpers.js";
import { useSession } from "./session-provider.js";
import type { TimelineEventEntry } from "./types.js";
import type { ProposalTemplate } from "./assistant-workspace.js";
import { useWorkspaceData } from "./workspace-data.js";

export function WorldTimeline({ onOpen, onConfigureCalendar, onCreateEvent, onUseTemplate }: { onOpen: (uri: string) => Promise<void>; onConfigureCalendar: () => void; onCreateEvent: () => void; onUseTemplate: (template: ProposalTemplate) => void }) {
  const session = useSession();
  const { timeline } = useWorkspaceData();
  const [query, setQuery] = useState("");
  const [density, setDensity] = useState<"compact" | "comfortable">("comfortable");
  const deferredQuery = useDeferredValue(query.trim().toLocaleLowerCase("es"));
  if (!session) return null;

  const filter = (events: TimelineEventEntry[]) => events.filter((event) =>
    !deferredQuery || event.summary.toLocaleLowerCase("es").includes(deferredQuery)
      || event.kind.toLocaleLowerCase("es").includes(deferredQuery));
  const known = filter(timeline.data?.known ?? []);
  const unknown = filter(timeline.data?.unknown ?? []);
  return (
    <section className={`world-timeline timeline-${density}`} aria-labelledby="timeline-page-title">
      <header className="timeline-page-heading">
        <div><p className="panel-eyebrow">Tiempo del mundo</p><h1 id="timeline-page-title" tabIndex={-1}>Cronología</h1><p>Los acontecimientos se ordenan por su unidad temporal canónica. El tiempo no especificado permanece separado.</p></div>
        <div className="timeline-controls">
          <button type="button" className="secondary" disabled={session.read_only} onClick={onConfigureCalendar}>{session.read_only ? "Configurar calendario (solo lectura)" : "Configurar calendario"}</button>
          <label>Filtrar <input type="search" value={query} onChange={(event) => setQuery(event.currentTarget.value)} placeholder="Resumen o tipo" /></label>
          <label>Densidad <select value={density} onChange={(event) => setDensity(event.currentTarget.value as typeof density)}><option value="comfortable">Cómoda</option><option value="compact">Compacta</option></select></label>
        </div>
      </header>
      {timeline.isPending && <p role="status">Cargando cronología…</p>}
      {timeline.isError && <p role="alert" className="notice warning">No se pudo cargar la cronología. El mundo no cambió.</p>}
      {!timeline.isPending && !timeline.isError && known.length === 0 && unknown.length === 0 && !deferredQuery && (
        <section className="empty-state contextual-empty">
          <h2>La cronología todavía no tiene acontecimientos</h2>
          <p>Crea el primer evento manualmente o prepara una secuencia breve para revisión.</p>
          <div className="pending-actions">
            <button type="button" disabled={session.read_only} onClick={onCreateEvent}>Crear evento</button>
            <button type="button" className="secondary" disabled={session.read_only} onClick={() => onUseTemplate("chronology")}>Usar plantilla Cronología</button>
          </div>
        </section>
      )}
      {!timeline.isPending && !timeline.isError && known.length === 0 && unknown.length === 0 && deferredQuery && <p className="empty-state">No hay acontecimientos con este filtro. La cronología existente no cambió.</p>}
      <TimelineLane title="Tiempo conocido" events={known} onOpen={onOpen} hasCalendar={Boolean(timeline.data?.calendarName)} />
      <TimelineLane title="Tiempo no especificado" events={unknown} onOpen={onOpen} unknown />
    </section>
  );
}

function TimelineLane({ title, events, onOpen, unknown = false, hasCalendar = false }: { title: string; events: TimelineEventEntry[]; onOpen: (uri: string) => Promise<void>; unknown?: boolean; hasCalendar?: boolean }) {
  if (events.length === 0) return null;
  return (
    <section className={`timeline-lane${unknown ? " timeline-unknown" : ""}`}>
      <h2>{title}</h2>
      <ol>
        {events.map((event) => (
          <li key={event.uri}>
            <button type="button" className="timeline-event-card" onClick={() => void onOpen(event.uri)}>
              <span className="timeline-date">{event.startCalendar?.label ?? (unknown ? "Sin fecha" : hasCalendar ? "Fecha fuera del rango de presentación" : "Tiempo conocido sin calendario de presentación")}</span>
              <strong>{event.summary}</strong>
              <span className="badge-row"><span className="badge kind">{humanize(event.kind)}</span><span className="badge context">{timeLabel(event)}</span></span>
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
