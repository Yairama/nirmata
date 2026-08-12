import {
  badge,
  block,
  button,
  formatEventTime,
  humanize,
  shortId,
} from "./helpers.js";
import {
  contextContent,
  contextEmpty,
  contextSummary,
  state,
} from "./state.js";
import type {
  JumpLink,
  RelatedContextEntry,
  TimelineEventEntry,
} from "./types.js";
import { selectUri } from "./workspace.js";

function renderLinkedEntry(entry: JumpLink | RelatedContextEntry): HTMLButtonElement {
  const item = button("", "linked-button");
  const title = document.createElement("div");
  title.className = "linked-button-title";
  const snippet = document.createElement("p");
  snippet.className = "linked-button-snippet";
  const meta = block("badge-row");
  if ("result" in entry) {
    title.textContent = entry.result.snippet;
    snippet.textContent = `${humanize(entry.result.object_type)} · ${shortId(entry.result.object_id)}`;
    meta.append(
      badge(humanize(entry.stage), "context"),
      badge(humanize(entry.result.classification), "info"),
    );
    item.addEventListener("click", () => {
      void selectUri(entry.result.uri);
    });
    item.setAttribute("aria-current", String(entry.result.uri === state.selectedUri));
  } else {
    title.textContent = entry.label;
    snippet.textContent = entry.uri;
    item.addEventListener("click", () => {
      void selectUri(entry.uri);
    });
    item.setAttribute("aria-current", String(entry.uri === state.selectedUri));
  }

  item.append(title, meta, snippet);
  return item;
}

function renderContextSection(title: string, entries: Array<JumpLink | RelatedContextEntry>): HTMLDivElement {
  const section = block("context-section");
  const heading = document.createElement("h4");
  heading.textContent = title;
  const list = block("context-list");
  list.append(...entries.map(renderLinkedEntry));
  section.append(heading, list);
  return section;
}

export function renderContext(): void {
  if (!state.editorMode && !state.timeline) {
    contextSummary.textContent = state.selectedLogicalPath ?? "";
    contextEmpty.hidden = false;
    contextContent.hidden = true;
    contextContent.replaceChildren();
    return;
  }

  const summaryParts: string[] = [];
  if (state.context) {
    summaryParts.push(`${state.context.usage.used_objects}/${state.context.usage.max_objects} objetos`);
  } else if (state.selectedLogicalPath) {
    summaryParts.push(state.selectedLogicalPath);
  }
  if (state.timeline) {
    summaryParts.push(
      `${state.timeline.known.length + state.timeline.unknown.length} evento${
        state.timeline.known.length + state.timeline.unknown.length === 1 ? "" : "s"
      }`,
    );
  }
  contextSummary.textContent = summaryParts.join(" · ");
  const wrapper = block("editor-layout");
  if (state.editorMode && state.editorMode.warnings.length > 0) {
    const warningSection = block("context-section");
    const heading = document.createElement("h4");
    heading.textContent = "Advertencias";
    const list = block("warning-list");
    for (const warning of state.editorMode.warnings) {
      const card = block("warning-card");
      const title = document.createElement("h4");
      title.textContent = warning.title;
      const detail = document.createElement("p");
      detail.textContent = warning.detail;
      card.append(title, detail);
      list.append(card);
    }
    warningSection.append(heading, list);
    wrapper.append(warningSection);
  }

  if (state.editorMode && state.editorMode.links.length > 0) {
    wrapper.append(renderContextSection("Navegación de relaciones", state.editorMode.links));
  }

  if (state.context) {
    const groups: Array<[string, RelatedContextEntry[]]> = [
      ["Canon", state.context.canon],
      ["Perspectivas", state.context.perspectives],
      ["Deseos", state.context.desires],
      ["Obligaciones", state.context.obligations],
      ["Búsqueda relacionada", state.context.search_evidence],
    ];

    for (const [title, entries] of groups) {
      if (entries.length > 0) {
        wrapper.append(renderContextSection(title, entries));
      }
    }
  }

  wrapper.append(renderTimelineSection());

  if (wrapper.childElementCount === 0) {
    const empty = block("context-section");
    const title = document.createElement("h4");
    title.textContent = "Sin contexto adicional";
    const detail = document.createElement("p");
    detail.textContent =
      state.context?.absence?.classification === "no_evidence"
        ? "No hay evidencia adicional alrededor de esta selección."
        : "No se encontraron relaciones adicionales para esta selección.";
    empty.append(title, detail);
    wrapper.append(empty);
  }

  contextContent.replaceChildren(wrapper);
  contextEmpty.hidden = true;
  contextContent.hidden = false;
}

function renderTimelineGroup(title: string, events: TimelineEventEntry[]): HTMLDivElement {
  const section = block("context-section");
  const heading = document.createElement("h4");
  heading.textContent = title;
  const list = block("timeline-list");
  for (const event of events) {
    const item = button("", "linked-button");
    item.setAttribute("aria-current", String(event.uri === state.selectedUri));
    const eventTitle = document.createElement("div");
    eventTitle.className = "linked-button-title";
    eventTitle.textContent = event.summary;
    const meta = block("badge-row");
    meta.append(badge(humanize(event.kind), "kind"), badge(humanize(event.time.kind), "context"));
    const detail = document.createElement("p");
    detail.className = "linked-button-snippet";
    const calendar = [event.startCalendar?.label, event.endCalendar?.label]
      .filter((value): value is string => Boolean(value))
      .join(" → ");
    detail.textContent = calendar
      ? `${formatEventTime(event.time)} · ${calendar}`
      : formatEventTime(event.time);
    item.append(eventTitle, meta, detail);
    item.addEventListener("click", () => {
      void selectUri(event.uri);
    });
    list.append(item);
  }
  section.append(heading, list);
  return section;
}

export function renderTimelineSection(): HTMLDivElement {
  const section = block("context-section");
  const heading = document.createElement("h4");
  heading.textContent = "Timeline";
  section.append(heading);
  if (!state.timeline || (state.timeline.known.length === 0 && state.timeline.unknown.length === 0)) {
    const detail = document.createElement("p");
    detail.textContent = "No hay eventos registrados todavía.";
    section.append(detail);
    return section;
  }

  if (state.timeline.known.length > 0) {
    section.append(renderTimelineGroup("Ticks conocidos", state.timeline.known));
  }
  if (state.timeline.unknown.length > 0) {
    section.append(renderTimelineGroup("Tiempo unknown", state.timeline.unknown));
  }
  return section;
}
