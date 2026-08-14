import * as Tabs from "@radix-ui/react-tabs";
import { useAppState } from "./state.js";
import type { JumpLink, RelatedContextEntry, TimelineEventEntry } from "./types.js";
import { buttonStyles, chipStyles, noticeStyles } from "./ui-styles.js";
import { selectUri } from "./workspace.js";
import { useWorkspaceData } from "./workspace-data.js";

const tabTriggerStyles = `${buttonStyles({ variant: "secondary", size: "compact" })} shrink-0 rounded-full px-3 aria-selected:border-accent aria-selected:bg-accent-soft aria-selected:text-accent`;
const contextEntryStyles = "context-entry grid w-full justify-stretch gap-1 rounded-xl border border-transparent bg-transparent p-3 text-left text-ink hover:border-line hover:bg-subtle";

export function WorldContext() {
  const workspaceData = useWorkspaceData();
  const state = useAppState();

  const context = workspaceData.relatedContext.data;
  const links = state.structuredEditor?.links ?? [];
  const warnings = state.structuredEditor?.warnings ?? [];
  const knownEvents = workspaceData.timeline.data?.known ?? [];
  const unknownEvents = workspaceData.timeline.data?.unknown ?? [];
  return (
    <div className="world-context flex h-full min-h-0 flex-col">
      <div className="panel-header flex min-h-16 items-start justify-between gap-4 border-b border-line px-4 py-3">
        <div className="grid gap-1"><p className="panel-eyebrow text-[0.68rem] font-bold uppercase tracking-[0.14em] text-accent">Lectura situada</p><h3 id="context-title">Contexto</h3></div>
        <p className="panel-summary text-xs font-medium text-muted">{context ? `${context.usage.used_objects}/${context.usage.max_objects} objetos` : "Sin selección"}</p>
      </div>
      <Tabs.Root className="context-tabs flex min-h-0 flex-1 flex-col" defaultValue="canon">
        <Tabs.List className="flex gap-1 overflow-x-auto pb-1" aria-label="Secciones de contexto">
          <Tabs.Trigger className={tabTriggerStyles} value="canon">Canon</Tabs.Trigger>
          <Tabs.Trigger className={tabTriggerStyles} value="perspectives">Perspectivas</Tabs.Trigger>
          <Tabs.Trigger className={tabTriggerStyles} value="goals">Metas</Tabs.Trigger>
          <Tabs.Trigger className={tabTriggerStyles} value="timeline">Cronología</Tabs.Trigger>
          <Tabs.Trigger className={tabTriggerStyles} value="warnings">Avisos</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="canon" className="context-scroll min-h-0 flex-1 overflow-auto p-3">
          <ContextGroup title="Relaciones del objeto" entries={links} selectedUri={state.selectedUri} />
          <ContextGroup title="Canon relacionado" entries={context?.canon ?? []} selectedUri={state.selectedUri} />
          <ContextGroup title="Fuentes relacionadas" entries={context?.search_evidence ?? []} selectedUri={state.selectedUri} />
          {links.length === 0 && (context?.canon.length ?? 0) === 0 && (context?.search_evidence.length ?? 0) === 0 && <ContextEmpty text="No hay evidencia canónica adicional alrededor de esta selección." />}
        </Tabs.Content>
        <Tabs.Content value="perspectives" className="context-scroll min-h-0 flex-1 overflow-auto p-3">
          <ContextGroup title="Conocimiento situado" entries={context?.perspectives ?? []} selectedUri={state.selectedUri} />
          {(context?.perspectives.length ?? 0) === 0 && <ContextEmpty text="No hay rumores, creencias o perspectivas relacionadas." />}
        </Tabs.Content>
        <Tabs.Content value="goals" className="context-scroll min-h-0 flex-1 overflow-auto p-3">
          <ContextGroup title="Deseos y metas" entries={context?.desires ?? []} selectedUri={state.selectedUri} />
          <ContextGroup title="Obligaciones" entries={context?.obligations ?? []} selectedUri={state.selectedUri} />
          {(context?.desires.length ?? 0) === 0 && (context?.obligations.length ?? 0) === 0 && <ContextEmpty text="No hay metas u obligaciones relacionadas." />}
        </Tabs.Content>
        <Tabs.Content value="timeline" className="context-scroll min-h-0 flex-1 overflow-auto p-3">
          <TimelineGroup title="Tiempo conocido" events={knownEvents} />
          <TimelineGroup title="Tiempo no especificado" events={unknownEvents} />
          {knownEvents.length === 0 && unknownEvents.length === 0 && <ContextEmpty text="No hay acontecimientos registrados todavía." />}
        </Tabs.Content>
        <Tabs.Content value="warnings" className="context-scroll min-h-0 flex-1 overflow-auto p-3">
          {warnings.map((warning) => <article key={`${warning.title}-${warning.detail}`} className={noticeStyles({ tone: "warning" })}><h4 className="font-semibold">{warning.title}</h4><p>{warning.detail}</p></article>)}
          {warnings.length === 0 && <ContextEmpty text="No hay advertencias para esta selección." />}
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}

function ContextGroup({ title, entries, selectedUri }: { title: string; entries: Array<JumpLink | RelatedContextEntry>; selectedUri: string | null }) {
  if (entries.length === 0) return null;
  return (
    <section className="context-group grid gap-3 border-b border-line py-4">
      <h4>{title}</h4>
      <div className="context-entry-list grid gap-1.5">
        {entries.map((entry) => {
          const result = "result" in entry ? entry.result : null;
          const uri = result?.uri ?? (entry as JumpLink).uri;
          const label = result?.snippet.replace(/[\[\]]/g, "") ?? (entry as JumpLink).label;
          return (
            <button key={`${title}-${uri}`} type="button" className={contextEntryStyles} aria-current={uri === selectedUri ? "true" : undefined} onClick={() => void selectUri(uri)}>
              <strong>{label}</strong>
              {result && <span className="badge-row flex flex-wrap items-center gap-1.5"><span className={chipStyles({ tone: result.authority === "canonical" ? "success" : "perspective" })}>{result.authority === "canonical" ? "Canon" : "Perspectiva"}</span><span className={chipStyles({ tone: "info" })}>{classificationLabel(result.classification)}</span></span>}
            </button>
          );
        })}
      </div>
    </section>
  );
}

function TimelineGroup({ title, events }: { title: string; events: TimelineEventEntry[] }) {
  if (events.length === 0) return null;
  return (
    <section className="context-group grid gap-3 border-b border-line py-4"><h4>{title}</h4><div className="context-entry-list grid gap-1.5">
      {events.map((event) => <button key={event.uri} type="button" className={contextEntryStyles} onClick={() => void selectUri(event.uri)}><strong>{event.summary}</strong><small>{event.startCalendar?.label ?? event.time.kind}</small></button>)}
    </div></section>
  );
}

function ContextEmpty({ text }: { text: string }) {
  return <p className="empty-state grid gap-4 rounded-xl border border-dashed border-line bg-surface p-5 text-sm text-muted">{text}</p>;
}

function classificationLabel(value: RelatedContextEntry["result"]["classification"]): string {
  return ({ fact: "Hecho", perspective: "Perspectiva", inference: "Inferencia", no_evidence: "Sin evidencia", unspecified: "No especificado" })[value];
}
