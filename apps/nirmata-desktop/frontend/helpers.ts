import { clearFeedback, commandErrorCopy, showCommandError } from "./feedback.js";

function setStatus(message: string): void {
  appActions.setStatus(message);
}

function setMarkdownText(element: HTMLElement, value: string | null | undefined, fallback: string): void {
  element.dataset.markdownMode = "plain-text";
  element.textContent = normalizeText(value, fallback);
}

function commandMessage(value: unknown): string {
  return commandErrorCopy(value).detail;
}

function commandCode(value: unknown): string | null {
  if (typeof value === "object" && value !== null && "code" in value) {
    return String((value as { code: unknown }).code);
  }
  return null;
}

function validationIssueMessage(issue: ValidationIssue): string {
  const exact: Record<string, string> = {
    "change_set.objective_empty": "Explica el objetivo de esta propuesta.",
    "change_set.operations_empty": "La propuesta no contiene operaciones para aplicar.",
    "change_set.operation.target_missing": "El objeto que se intenta modificar ya no existe.",
    "change_set.operation.target_exists": "Ya existe un objeto con esa identidad.",
    "change_set.operation.world_revision_changed": "El mundo avanzó desde que se preparó esta operación.",
    "change_set.dependency_order": "Ordena primero las operaciones de las que depende este cambio.",
    "change_set.delete_orphan": "Este objeto todavía es utilizado por otros elementos del canon. Abre las dependencias y elimina o cambia esas referencias antes de confirmar.",
    "change_set.replacement_decision_unresolved": "Elige si conservar el objeto actual o aplicar su eliminación.",
    "reference.cross_world": "La referencia pertenece a otro mundo.",
    "version.mismatch": "La versión esperada ya no coincide con la actual.",
    "period.inverted": "El final del período ocurre antes que el inicio.",
    "event.time_invalid": "El tiempo del acontecimiento no es válido.",
  };
  if (exact[issue.code]) return exact[issue.code];
  if (issue.code.includes("missing")) return "Falta un dato o una referencia necesaria para este cambio.";
  if (issue.code.includes("duplicate")) return "Este cambio repite un dato que debe ser único.";
  if (issue.code.includes("empty")) return "Completa la información requerida antes de aplicar.";
  if (issue.code.includes("too_many")) return "Reduce la cantidad de elementos incluidos en este cambio.";
  if (issue.code.includes("too_long")) return "Acorta este contenido antes de aplicar.";
  if (issue.code.includes("invalid")) return "Este valor no cumple las reglas del mundo.";
  return issue.severity === "error"
    ? "Este cambio incumple una regla obligatoria y todavía no puede aplicarse."
    : "Revisa esta condición antes de aplicar el cambio.";
}

function showError(value: unknown): void {
  showCommandError(value);
}

function clearError(): void {
  clearFeedback();
}

function button(text: string, className?: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.type = "button";
  element.textContent = text;
  if (className) {
    element.className = className;
  }
  return element;
}

function block(className?: string): HTMLDivElement {
  const element = document.createElement("div");
  if (className) {
    element.className = className;
  }
  return element;
}

function badge(text: string, className?: string): HTMLSpanElement {
  const element = document.createElement("span");
  element.className = ["badge", className].filter(Boolean).join(" ");
  element.textContent = text;
  return element;
}

function shortId(value: string): string {
  return value.length > 8 ? value.slice(0, 8) : value;
}

function humanize(value: string): string {
  const labels: Record<string, string> = {
    fact: "Hecho",
    perspective: "Perspectiva",
    inference: "Inferencia",
    no_evidence: "Sin evidencia",
    unspecified: "No especificado",
    entity: "Entidad",
    relation: "Relación",
    event: "Evento",
    claim: "Afirmación",
    rule: "Regla",
    goal: "Meta",
    document: "Documento",
    world: "Mundo",
    create: "Crear",
    update: "Modificar",
    created: "Creado",
    deleted: "Eliminado",
    renamed: "Renombrado",
    edited: "Modificado",
    relation_diverged: "Relación divergente",
    additive: "Completa información",
    reinterpretive: "Añade otra interpretación",
    replacement: "Sustituye información",
    warning: "Advertencia",
    conflict: "Conflicto",
    error: "Error",
    info: "Información",
    accept: "Aceptar",
    reject: "Rechazar",
    current: "Vigente",
    completed: "Completado",
    cancelled: "Cancelado",
    failed: "Fallido",
    person: "Personaje",
    place: "Lugar",
    faction: "Facción",
    culture: "Cultura",
    resource: "Recurso",
    concept: "Concepto",
    directed: "Dirigida",
    undirected: "No dirigida",
    certain: "Cierta",
    approximate: "Aproximada",
    uncertain: "Incierta",
    approximate_uncertain: "Aproximada e incierta",
    unknown: "Desconocido",
    instant: "Instante",
    interval: "Intervalo",
    ongoing: "En curso",
    exact: "Exacta",
    day: "Día",
    month: "Mes",
    year: "Año",
    era: "Era",
    positive: "Positiva",
    negative: "Negativa",
    canonical: "Canónico",
    attributed: "Atribuida",
    disputed: "Disputada",
    assertion: "Afirmación",
    belief: "Creencia",
    hypothesis: "Hipótesis",
    counterfactual: "Contrafactual",
    constitutive: "Constitutiva",
    generative: "Generativa",
    institutional: "Institucional",
    authorial: "Autoral",
    advisory: "Orientativa",
    hard: "Obligatoria",
    active: "Activa",
    achieved: "Cumplida",
    abandoned: "Abandonada",
    frustrated: "Frustrada",
    public: "Pública",
    secret: "Secreta",
    non_canonical: "No canónico",
    chronicle: "Crónica",
    letter: "Carta",
    report: "Informe",
    myth: "Mito",
    short_story: "Historia corta",
  };
  if (labels[value]) {
    return labels[value];
  }
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function normalizeText(value: string | null | undefined, fallback = "No especificado"): string {
  const trimmed = value?.trim() ?? "";
  return trimmed.length > 0 ? trimmed : fallback;
}

function previewText(value: string | null | undefined, fallback: string): string {
  const text = normalizeText(value, fallback);
  return text.length > 120 ? `${text.slice(0, 120).trimEnd()}…` : text;
}

function formatObjectRef(reference: ObjectRef): { kind: ObjectKind; id: string; uri: string } {
  if ("world" in reference) {
    return { kind: "world", id: reference.world, uri: `nirmata://world/${reference.world}` };
  }
  if ("entity" in reference) {
    return { kind: "entity", id: reference.entity, uri: `nirmata://entity/${reference.entity}` };
  }
  if ("relation" in reference) {
    return {
      kind: "relation",
      id: reference.relation,
      uri: `nirmata://relation/${reference.relation}`,
    };
  }
  if ("event" in reference) {
    return { kind: "event", id: reference.event, uri: `nirmata://event/${reference.event}` };
  }
  if ("claim" in reference) {
    return { kind: "claim", id: reference.claim, uri: `nirmata://claim/${reference.claim}` };
  }
  if ("rule" in reference) {
    return { kind: "rule", id: reference.rule, uri: `nirmata://rule/${reference.rule}` };
  }
  if ("goal" in reference) {
    return { kind: "goal", id: reference.goal, uri: `nirmata://goal/${reference.goal}` };
  }
  return {
    kind: "document",
    id: reference.document,
    uri: `nirmata://document/${reference.document}`,
  };
}

function formatReferenceLabel(reference: ObjectRef): string {
  const parts = formatObjectRef(reference);
  return `${humanize(parts.kind)} · ${shortId(parts.id)}`;
}

function formatPeriod(period: Period | null): string {
  if (period === null) {
    return "Sin periodo";
  }
  const start = period.start_tick ?? "¿?";
  const end = period.end_tick ?? "¿?";
  return start === end ? `Tick ${start}` : `Ticks ${start} → ${end}`;
}

function formatEventTime(time: EventTime): string {
  if (time.kind === "unknown") {
    return `Tiempo desconocido · ${humanize(time.certainty)}`;
  }
  const start = time.start_tick ?? "¿?";
  const end = time.end_tick ?? "¿?";
  const range =
    time.kind === "instant"
      ? `Tick ${start}`
      : time.kind === "ongoing"
        ? `Desde ${start}`
        : `Ticks ${start} → ${end}`;
  return `${range} · ${humanize(time.precision)} · ${humanize(time.certainty)}`;
}

function formatTimestamp(value: number): string {
  return new Date(value).toLocaleString("es");
}

function retainedDraftHint(): string {
  return " La propuesta sigue guardada en Cambios.";
}

function formatClaimObject(object: ClaimObject | null): string {
  if (object === null) {
    return "Sin objeto";
  }
  if ("entity" in object) {
    return `Entidad ${shortId(object.entity)}`;
  }
  return object.scalar;
}

function linesToText(values: string[]): string {
  return values.join("\n");
}

function splitLines(value: string): string[] {
  return value
    .split(/\r?\n/u)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

type EditableContentReference = {
  targetUri: string;
  ordinal: number;
};

function serializeContentReferences(references: Array<EditableContentReference | ContentReference>): string {
  return references
    .map((reference, index) => {
      if ("targetUri" in reference) {
        return `${reference.targetUri}|${reference.ordinal ?? index}`;
      }
      return `${formatObjectRef(reference.target).uri}|${reference.ordinal}`;
    })
    .join("\n");
}

function parseContentReferences(value: string): EditableContentReference[] {
  return value
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line, index) => {
      const parts = line.split("|").map((part) => part.trim());
      if (parts.length === 1) {
        return { targetUri: parts[0], ordinal: index };
      }
      const [left, right] = parts;
      const leftNumber = Number(left);
      const rightNumber = Number(right);
      if (Number.isInteger(leftNumber) && !Number.isInteger(rightNumber)) {
        return { targetUri: right, ordinal: leftNumber };
      }
      if (!Number.isInteger(leftNumber) && Number.isInteger(rightNumber)) {
        return { targetUri: left, ordinal: rightNumber };
      }
      return { targetUri: left, ordinal: index };
    });
}

function normalizeContentReferences(references: EditableContentReference[]): EditableContentReference[] {
  return references.map((reference, index) => ({
    targetUri: reference.targetUri.trim(),
    ordinal: index,
  }));
}

function firstReviewIssue(report: ValidationReport): ValidationIssue | null {
  return report.errors[0] ?? report.conflicts[0] ?? report.warnings[0] ?? report.info[0] ?? null;
}

function dedupeLinks(links: JumpLink[]): JumpLink[] {
  const seen = new Set<string>();
  const unique: JumpLink[] = [];
  for (const link of links) {
    if (seen.has(link.uri)) {
      continue;
    }
    seen.add(link.uri);
    unique.push(link);
  }
  return unique;
}

function objectKindFromUri(uri: string): ObjectKind | null {
  const matched = /^nirmata:\/\/(world|entity|relation|event|claim|rule|goal|document)\//u.exec(uri);
  return matched ? (matched[1] as ObjectKind) : null;
}

function pathForUri(tree: LogicalVfsDirectory | null, uri: string): string | null {
  if (!tree) {
    return null;
  }
  for (const child of tree.children) {
    const path = pathForNode(child, uri, "");
    if (path) {
      return path;
    }
  }
  return null;
}

function pathForNode(node: LogicalVfsNode, uri: string, prefix: string): string | null {
  if (node.type === "object") {
    return node.uri === uri ? `${prefix}/${node.name}` : null;
  }
  const nextPrefix = `${prefix}/${node.name}`;
  for (const child of node.children) {
    const path = pathForNode(child, uri, nextPrefix);
    if (path) {
      return path;
    }
  }
  return null;
}

function firstUriFromTree(tree: LogicalVfsDirectory | null): string | null {
  if (!tree) {
    return null;
  }
  return firstUriFromNodes(tree.children);
}

function firstUriFromNodes(nodes: LogicalVfsNode[]): string | null {
  for (const node of nodes) {
    if (node.type === "object") {
      return node.uri;
    }
    const nested = firstUriFromNodes(node.children);
    if (nested) {
      return nested;
    }
  }
  return null;
}

function labelForUri(uri: string): string {
  const state = getAppState();
  if (uri.startsWith("nirmata://world/")) {
    return state.session?.world.name ?? "Mundo";
  }
  const path = pathForUri(state.logicalTree, uri);
  if (path) {
    const parts = path.split("/").filter(Boolean);
    return parts[parts.length - 1] ?? uri;
  }
  return uri;
}

function warningsForResult(result: SearchResult): WarningItem[] {
  switch (result.classification) {
    case "perspective":
      return [
        {
          title: "Resultado perspectival",
          detail: "La selección proviene de una perspectiva situada y puede no ser canon directo.",
        },
      ];
    case "inference":
      return [
        {
          title: "Resultado inferido",
          detail: "La selección depende de inferencias; revisa el contexto antes de asumirla como canon.",
        },
      ];
    case "no_evidence":
      return [
        {
          title: "Sin evidencia",
          detail: "La búsqueda no encontró respaldo suficiente para este resultado.",
        },
      ];
    default:
      return [];
  }

}
import { appActions, getAppState } from "./state.js";
import type {
  ClaimObject,
  ContentReference,
  EventTime,
  JumpLink,
  LogicalVfsDirectory,
  LogicalVfsNode,
  ObjectKind,
  ObjectRef,
  Period,
  SearchResult,
  ValidationIssue,
  ValidationReport,
  WarningItem,
} from "./types.js";

export {
  badge,
  block,
  button,
  clearError,
  commandCode,
  commandMessage,
  dedupeLinks,
  firstReviewIssue,
  firstUriFromTree,
  formatClaimObject,
  formatEventTime,
  formatObjectRef,
  formatPeriod,
  formatReferenceLabel,
  formatTimestamp,
  humanize,
  labelForUri,
  linesToText,
  normalizeContentReferences,
  normalizeText,
  objectKindFromUri,
  parseContentReferences,
  pathForUri,
  previewText,
  retainedDraftHint,
  serializeContentReferences,
  setMarkdownText,
  setStatus,
  shortId,
  showError,
  splitLines,
  warningsForResult,
  validationIssueMessage,
};
export type { EditableContentReference };
