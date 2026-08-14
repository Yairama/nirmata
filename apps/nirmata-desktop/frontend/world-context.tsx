import * as Tabs from "@radix-ui/react-tabs";
import { useAppState } from "./state.js";
import type { JumpLink, RelatedContextEntry, TimelineEventEntry } from "./types.js";
import { selectUri } from "./workspace.js";
import { useWorkspaceData } from "./workspace-data.js";

export function WorldContext() {
  const workspaceData = useWorkspaceData();
  const state = useAppState();

  const context = workspaceData.relatedContext.data;
  const links = state.structuredEditor?.links ?? [];
  const warnings = state.structuredEditor?.warnings ?? [];
  const knownEvents = workspaceData.timeline.data?.known ?? [];
  const unknownEvents = workspaceData.timeline.data?.unknown ?? [];
  return (
    <div className="world-context">
      <div className="panel-header">
        <div><p className="panel-eyebrow">Lectura situada</p><h3 id="context-title">Contexto</h3></div>
        <p className="panel-summary">{context ? `${context.usage.used_objects}/${context.usage.max_objects} objetos` : "Sin selección"}</p>
      </div>
      <Tabs.Root className="context-tabs" defaultValue="canon">
        <Tabs.List aria-label="Secciones de contexto">
          <Tabs.Trigger value="canon">Canon</Tabs.Trigger>
          <Tabs.Trigger value="perspectives">Perspectivas</Tabs.Trigger>
          <Tabs.Trigger value="goals">Metas</Tabs.Trigger>
          <Tabs.Trigger value="timeline">Cronología</Tabs.Trigger>
          <Tabs.Trigger value="warnings">Avisos</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="canon" className="context-scroll">
          <ContextGroup title="Relaciones del objeto" entries={links} selectedUri={state.selectedUri} />
          <ContextGroup title="Canon relacionado" entries={context?.canon ?? []} selectedUri={state.selectedUri} />
          <ContextGroup title="Fuentes relacionadas" entries={context?.search_evidence ?? []} selectedUri={state.selectedUri} />
          {links.length === 0 && (context?.canon.length ?? 0) === 0 && (context?.search_evidence.length ?? 0) === 0 && <ContextEmpty text="No hay evidencia canónica adicional alrededor de esta selección." />}
        </Tabs.Content>
        <Tabs.Content value="perspectives" className="context-scroll">
          <ContextGroup title="Conocimiento situado" entries={context?.perspectives ?? []} selectedUri={state.selectedUri} />
          {(context?.perspectives.length ?? 0) === 0 && <ContextEmpty text="No hay rumores, creencias o perspectivas relacionadas." />}
        </Tabs.Content>
        <Tabs.Content value="goals" className="context-scroll">
          <ContextGroup title="Deseos y metas" entries={context?.desires ?? []} selectedUri={state.selectedUri} />
          <ContextGroup title="Obligaciones" entries={context?.obligations ?? []} selectedUri={state.selectedUri} />
          {(context?.desires.length ?? 0) === 0 && (context?.obligations.length ?? 0) === 0 && <ContextEmpty text="No hay metas u obligaciones relacionadas." />}
        </Tabs.Content>
        <Tabs.Content value="timeline" className="context-scroll">
          <TimelineGroup title="Tiempo conocido" events={knownEvents} />
          <TimelineGroup title="Tiempo no especificado" events={unknownEvents} />
          {knownEvents.length === 0 && unknownEvents.length === 0 && <ContextEmpty text="No hay acontecimientos registrados todavía." />}
        </Tabs.Content>
        <Tabs.Content value="warnings" className="context-scroll">
          {warnings.map((warning) => <article key={`${warning.title}-${warning.detail}`} className="notice warning"><h4>{warning.title}</h4><p>{warning.detail}</p></article>)}
          {warnings.length === 0 && <ContextEmpty text="No hay advertencias para esta selección." />}
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
}

function ContextGroup({ title, entries, selectedUri }: { title: string; entries: Array<JumpLink | RelatedContextEntry>; selectedUri: string | null }) {
  if (entries.length === 0) return null;
  return (
    <section className="context-group">
      <h4>{title}</h4>
      <div className="context-entry-list">
        {entries.map((entry) => {
          const result = "result" in entry ? entry.result : null;
          const uri = result?.uri ?? (entry as JumpLink).uri;
          const label = result?.snippet.replace(/[\[\]]/g, "") ?? (entry as JumpLink).label;
          return (
            <button key={`${title}-${uri}`} type="button" className="context-entry" aria-current={uri === selectedUri ? "true" : undefined} onClick={() => void selectUri(uri)}>
              <strong>{label}</strong>
              {result && <span className="badge-row"><span className={`badge ${result.authority === "canonical" ? "ready" : "context"}`}>{result.authority === "canonical" ? "Canon" : "Perspectiva"}</span><span className="badge info">{classificationLabel(result.classification)}</span></span>}
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
    <section className="context-group"><h4>{title}</h4><div className="context-entry-list">
      {events.map((event) => <button key={event.uri} type="button" className="context-entry" onClick={() => void selectUri(event.uri)}><strong>{event.summary}</strong><small>{event.startCalendar?.label ?? event.time.kind}</small></button>)}
    </div></section>
  );
}

function ContextEmpty({ text }: { text: string }) {
  return <p className="empty-state">{text}</p>;
}

function classificationLabel(value: RelatedContextEntry["result"]["classification"]): string {
  return ({ fact: "Hecho", perspective: "Perspectiva", inference: "Inferencia", no_evidence: "Sin evidencia", unspecified: "No especificado" })[value];
}
