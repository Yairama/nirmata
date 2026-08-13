import {
  dedupeLinks,
  formatClaimObject,
  formatEventTime,
  formatObjectRef,
  formatPeriod,
  formatReferenceLabel,
  humanize,
  linesToText,
  normalizeText,
  pathForUri,
  previewText,
  serializeContentReferences,
  shortId,
  warningsForResult,
} from "./helpers.js";
import { state } from "./state.js";
import type {
  EditorControl,
  EditorField,
  EditorMeta,
  EditorMode,
  JumpLink,
  ObjectKind,
  OpenUriResponse,
  WarningItem,
} from "./types.js";

function createField(
  key: string,
  label: string,
  control: EditorControl,
  value: string,
  options: Partial<EditorField> = {},
): EditorField {
  return { key, label, control, value, ...options };
}

export {
  buildSelectionEditor,
  buildWorldEditor,
  cloneEditorMode,
  createEditorMode,
  createField,
  currentWorldUri,
  enumOptions,
};

function enumOptions(values: string[]): Array<{ value: string; label: string }> {
  return values.map((value) => ({ value, label: humanize(value) }));
}

function createEditorMode(definition: {
  mode: "create" | "update";
  objectType: ObjectKind;
  existingUri: string | null;
  targetUri: string | null;
  title: string;
  subtitle: string;
  description: string;
  logicalPath: string | null;
  fields: EditorField[];
  metadata: EditorMeta[];
  warnings: WarningItem[];
  links: JumpLink[];
  objective: string;
  sourceUrisText: string;
  assumptionsText: string;
}): EditorMode {
  const values = Object.fromEntries(definition.fields.map((field) => [field.key, field.value]));
  return {
    ...definition,
    values,
    baselineValues: { ...values },
    baselineObjective: definition.objective,
    baselineSourceUrisText: definition.sourceUrisText,
    baselineAssumptionsText: definition.assumptionsText,
    issues: [],
    reviewEdit: null,
  };
}

function cloneEditorMode(mode: EditorMode): EditorMode {
  return {
    ...mode,
    fields: mode.fields.map((field) => ({ ...field })),
    values: { ...mode.values },
    baselineValues: { ...mode.baselineValues },
    metadata: mode.metadata.map((item) => ({ ...item })),
    warnings: mode.warnings.map((item) => ({ ...item })),
    links: mode.links.map((item) => ({ ...item })),
    issues: mode.issues.map((item) => ({ ...item })),
    reviewEdit: mode.reviewEdit ? { ...mode.reviewEdit } : null,
  };
}

function currentWorldUri(): string | null {
  return state.session ? `nirmata://world/${state.session.world.id}` : null;
}

function buildWorldEditor(): EditorMode | null {
  if (!state.session) {
    return null;
  }
  const world = state.session.world;
  const worldUri = currentWorldUri()!;
  const editor = createEditorMode({
    mode: "update",
    objectType: "world",
    existingUri: worldUri,
    targetUri: worldUri,
    title: world.name,
    subtitle: "Mundo",
    description: previewText(world.premise_md, "Edita metadatos del mundo activo como draft."),
    logicalPath: "/world",
    objective: `Actualizar mundo ${world.name}`,
    sourceUrisText: worldUri,
    assumptionsText: "",
    fields: worldFields(world),
    metadata: [
      { label: "URI", value: worldUri },
      { label: "Revisión base", value: world.current_revision },
      { label: "Actualizado", value: new Date(world.updated_at_ms).toLocaleString("es") },
    ],
    warnings: [],
    links: [],
  });
  return editor;
}

function buildSelectionEditor(selection: OpenUriResponse): EditorMode {
  const { result, object } = selection;
  const warnings = warningsForResult(result);
  const sourceUrisText = result.uri;

  if ("world" in object) {
    return buildWorldEditor() ?? createEditorMode({
      mode: "update",
      objectType: "world",
      existingUri: result.uri,
      targetUri: result.uri,
      title: object.world.name,
      subtitle: "Mundo",
      description: previewText(object.world.premise_md, "Mundo activo"),
      logicalPath: "/world",
      objective: `Actualizar mundo ${object.world.name}`,
      sourceUrisText,
      assumptionsText: "",
      fields: worldFields(object.world),
      metadata: [{ label: "URI", value: result.uri }],
      warnings,
      links: [],
    });
  }

  if ("entity" in object) {
    const entity = object.entity;
    const editor = createEditorMode({
      mode: "update",
      objectType: "entity",
      existingUri: result.uri,
      targetUri: result.uri,
      title: entity.name,
      subtitle: humanize(entity.kind),
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Actualizar entidad ${entity.name}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("kind", "Tipo", "select", entity.kind, {
          required: true,
          options: enumOptions(["person", "place", "faction", "culture", "resource", "concept"]),
        }),
        createField("name", "Nombre", "text", entity.name, { required: true }),
        createField("slug", "Slug", "text", entity.slug, { required: true }),
        createField("aliases", "Nombres alternativos (uno por línea)", "textarea", linesToText(entity.aliases), {
          rows: 4,
        }),
        createField("summary", "Resumen", "textarea", entity.summary, { rows: 4 }),
        createField("body_md", "Cuerpo Markdown", "textarea", entity.body_md, { rows: 7 }),
        createField("attributes_json", "Atributos JSON", "textarea", entity.attributes_json, {
          rows: 6,
          help: "Debe ser un objeto JSON.",
        }),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Versión", value: String(entity.version) },
        { label: "Actualizado", value: new Date(entity.updated_at_ms).toLocaleString("es") },
      ],
      warnings,
      links: [],
    });
    return editor;
  }

  if ("relation" in object) {
    const relation = object.relation;
    const links: JumpLink[] = [
      {
        uri: `nirmata://entity/${relation.source_entity_id}`,
        label: `Entidad origen · ${shortId(relation.source_entity_id)}`,
      },
      {
        uri: `nirmata://entity/${relation.target_entity_id}`,
        label: `Entidad destino · ${shortId(relation.target_entity_id)}`,
      },
    ];
    const editor = createEditorMode({
      mode: "update",
      objectType: "relation",
      existingUri: result.uri,
      targetUri: result.uri,
      title: relation.kind,
      subtitle: humanize(relation.direction),
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Actualizar relación ${relation.kind}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("source_entity", "Entidad origen", "text", relation.source_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("target_entity", "Entidad destino", "text", relation.target_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("kind", "Tipo de relación", "text", relation.kind, { required: true }),
        createField("direction", "Dirección", "select", relation.direction, {
          required: true,
          options: enumOptions(["directed", "undirected"]),
        }),
        createField("certainty", "Certeza", "select", relation.certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("valid_from_tick", "Tick inicio", "number", relation.valid_from_tick?.toString() ?? ""),
        createField("valid_to_tick", "Tick fin", "number", relation.valid_to_tick?.toString() ?? ""),
        createField("source_reference", "Procedencia", "textarea", relation.source_reference ?? "", {
          rows: 3,
        }),
        createField("metadata_json", "Metadata JSON", "textarea", relation.metadata_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Versión", value: String(relation.version) },
        {
          label: "Periodo",
          value:
            relation.valid_from_tick === null && relation.valid_to_tick === null
              ? "Siempre"
              : `Ticks ${relation.valid_from_tick ?? "¿?"} → ${relation.valid_to_tick ?? "¿?"}`,
        },
      ],
      warnings:
        relation.certainty !== "certain"
          ? [
              ...warnings,
              {
                title: "Relación incierta",
                detail: `La relación está marcada como ${humanize(relation.certainty).toLowerCase()}.`,
              },
            ]
          : warnings,
      links: dedupeLinks(links),
    });
    return editor;
  }

  if ("event" in object) {
    const aggregate = object.event;
    const event = aggregate.event;
    const links: JumpLink[] = [];
    if (event.location_entity_id) {
      links.push({
        uri: `nirmata://entity/${event.location_entity_id}`,
        label: `Lugar · ${shortId(event.location_entity_id)}`,
      });
    }
    for (const participant of event.participants) {
      links.push({
        uri: `nirmata://entity/${participant.entity_id}`,
        label: `${humanize(participant.role)} · ${shortId(participant.entity_id)}`,
      });
    }
    for (const goalId of event.affected_goal_ids) {
      links.push({ uri: `nirmata://goal/${goalId}`, label: `Meta · ${shortId(goalId)}` });
    }
    for (const link of aggregate.links) {
      links.push({
        uri: `nirmata://event/${link.target_event_id}`,
        label: `${humanize(link.kind)} · ${shortId(link.target_event_id)}`,
      });
    }
    const eventWarnings = [...warnings];
    if (event.time.kind === "unknown") {
      eventWarnings.push({
        title: "Tiempo desconocido",
        detail: "Este evento no tiene anclaje temporal preciso.",
      });
    }
    if (event.time.certainty !== "certain") {
      eventWarnings.push({
        title: "Tiempo incierto",
        detail: `La cronología está marcada como ${humanize(event.time.certainty).toLowerCase()}.`,
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "event",
      existingUri: result.uri,
      targetUri: result.uri,
      title: normalizeText(event.summary, event.kind),
      subtitle: humanize(event.kind),
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Actualizar evento ${normalizeText(event.summary, event.kind)}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("kind", "Tipo", "text", event.kind, { required: true }),
        createField("summary", "Resumen", "textarea", event.summary, { rows: 3, required: true }),
        createField("body_md", "Cuerpo Markdown", "textarea", event.body_md, { rows: 6 }),
        createField("time_kind", "Tipo de tiempo", "select", event.time.kind, {
          required: true,
          options: enumOptions(["unknown", "instant", "interval", "ongoing"]),
        }),
        createField("time_precision", "Precisión", "select", event.time.precision, {
          required: true,
          options: enumOptions(["exact", "day", "month", "year", "era", "unknown"]),
        }),
        createField("time_certainty", "Certeza temporal", "select", event.time.certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("start_tick", "Tick inicio", "number", event.time.start_tick?.toString() ?? ""),
        createField("end_tick", "Tick fin", "number", event.time.end_tick?.toString() ?? ""),
        createField("start_calendar_date", "Fecha inicio (año|mes|día|sub-tick)", "text", ""),
        createField("end_calendar_date", "Fecha fin (año|mes|día|sub-tick)", "text", ""),
        createField("location_entity", "Entidad lugar", "text", event.location_entity_id ?? "", {
          help: "UUID o nirmata://entity/...",
        }),
        createField(
          "participants",
          "Participantes (entidad|rol|ordinal)",
          "textarea",
          linesToText(
            event.participants.map(
              (participant) => `${participant.entity_id}|${participant.role}|${participant.ordinal}`,
            ),
          ),
          { rows: 5 },
        ),
        createField(
          "affected_goal_ids",
          "Metas afectadas (una por línea)",
          "textarea",
          linesToText(event.affected_goal_ids),
          { rows: 3, help: "UUID o nirmata://goal/..." },
        ),
        createField(
          "causal_links",
          "Causalidad (evento|kind)",
          "textarea",
          linesToText(aggregate.links.map((link) => `nirmata://event/${link.target_event_id}|${link.kind}`)),
          {
            rows: 4,
            help: "Una línea por vínculo: nirmata://event/<id>|causes",
          },
        ),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Tiempo", value: formatEventTime(event.time) },
        { label: "Versión", value: String(event.version) },
      ],
      warnings: eventWarnings,
      links: dedupeLinks(links),
    });
    return editor;
  }

  if ("claim" in object) {
    const claim = object.claim;
    const links: JumpLink[] = [
      {
        uri: `nirmata://entity/${claim.subject_entity_id}`,
        label: `Sujeto · ${shortId(claim.subject_entity_id)}`,
      },
    ];
    if (claim.holder_entity_id) {
      links.push({
        uri: `nirmata://entity/${claim.holder_entity_id}`,
        label: `Holder · ${shortId(claim.holder_entity_id)}`,
      });
    }
    if (claim.source_document_id) {
      links.push({
        uri: `nirmata://document/${claim.source_document_id}`,
        label: `Documento fuente · ${shortId(claim.source_document_id)}`,
      });
    }
    if (claim.source_claim_id) {
      links.push({
        uri: `nirmata://claim/${claim.source_claim_id}`,
        label: `Claim fuente · ${shortId(claim.source_claim_id)}`,
      });
    }
    if (claim.object && "entity" in claim.object) {
      links.push({
        uri: `nirmata://entity/${claim.object.entity}`,
        label: `Objeto · ${shortId(claim.object.entity)}`,
      });
    }
    const claimWarnings = [...warnings];
    if (claim.authentication !== "canonical") {
      claimWarnings.push({
        title: "Claim no canónico",
        detail: `La autenticación actual es ${humanize(claim.authentication).toLowerCase()}.`,
      });
    }
    if (claim.superseded_revision_id !== null) {
      claimWarnings.push({
        title: "Claim reemplazado",
        detail: "Este claim ya tiene una revisión posterior que lo supera.",
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "claim",
      existingUri: result.uri,
      targetUri: result.uri,
      title: normalizeText(claim.predicate_key, "Claim"),
      subtitle: humanize(claim.authentication),
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Actualizar afirmación ${previewText(claim.content_md, "afirmación")}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("subject_entity", "Entidad sujeto", "text", claim.subject_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("content_md", "Contenido Markdown", "textarea", claim.content_md, { rows: 5 }),
        createField("predicate_key", "Tipo de afirmación", "text", claim.predicate_key ?? ""),
        createField(
          "object_kind",
          "Tipo de objeto",
          "select",
          claim.object === null ? "none" : "entity" in claim.object ? "entity" : "scalar",
          {
            options: [
            { value: "none", label: "Sin objeto" },
            { value: "entity", label: "Otra entidad" },
            { value: "scalar", label: "Valor textual" },
            ],
          },
        ),
        createField(
          "object_value",
          "Objeto",
          "text",
          claim.object === null ? "" : "entity" in claim.object ? claim.object.entity : claim.object.scalar,
          { help: "UUID/URI para entity, o texto para scalar." },
        ),
        createField("polarity", "Polaridad", "select", claim.polarity, {
          required: true,
          options: enumOptions(["positive", "negative"]),
        }),
        createField("authentication", "Autenticación", "select", claim.authentication, {
          required: true,
          options: enumOptions(["canonical", "attributed", "disputed"]),
        }),
        createField("holder_entity", "Quien sostiene la afirmación", "text", claim.holder_entity_id ?? "", {
          help: "UUID o nirmata://entity/...",
        }),
        createField("modality", "Modalidad", "select", claim.modality ?? "", {
          options: [
            { value: "", label: "Sin modalidad" },
            ...enumOptions(["assertion", "belief", "hypothesis", "counterfactual"]),
          ],
        }),
        createField("register", "Registro", "text", claim.register ?? ""),
        createField("epistemic_basis", "Base epistémica", "textarea", claim.epistemic_basis ?? "", {
          rows: 3,
        }),
        createField("source", "Procedencia textual", "textarea", claim.source ?? "", { rows: 3 }),
        createField("source_document", "Documento fuente", "text", claim.source_document_id ?? "", {
          help: "UUID o nirmata://document/...",
        }),
        createField("source_claim", "Claim fuente", "text", claim.source_claim_id ?? "", {
          help: "UUID o nirmata://claim/...",
        }),
        createField(
          "holder_confidence",
          "Confianza declarada",
          "number",
          claim.holder_confidence?.toString() ?? "",
        ),
        createField("period_start_tick", "Periodo inicio", "number", claim.period?.start_tick?.toString() ?? ""),
        createField("period_end_tick", "Periodo fin", "number", claim.period?.end_tick?.toString() ?? ""),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Objeto", value: formatClaimObject(claim.object) },
        { label: "Periodo", value: formatPeriod(claim.period) },
        { label: "Versión", value: String(claim.version) },
      ],
      warnings: claimWarnings,
      links: dedupeLinks(links),
    });
    return editor;
  }

  if ("rule" in object) {
    const rule = object.rule;
    const ruleWarnings = [...warnings];
    if (rule.severity === "hard") {
      ruleWarnings.push({
        title: "Regla dura",
        detail: "Puede bloquear cambios durante una revisión posterior.",
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "rule",
      existingUri: result.uri,
      targetUri: result.uri,
      title: previewText(rule.statement_md, "Regla"),
      subtitle: humanize(rule.kind),
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Actualizar regla ${previewText(rule.statement_md, "regla")}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("kind", "Tipo", "select", rule.kind, {
          required: true,
          options: enumOptions(["constitutive", "generative", "institutional", "authorial"]),
        }),
        createField("statement_md", "Regla Markdown", "textarea", rule.statement_md, {
          rows: 4,
          required: true,
        }),
        createField("scope", "Alcance", "text", rule.scope, { required: true }),
        createField("severity", "Severidad", "select", rule.severity, {
          required: true,
          options: enumOptions(["advisory", "hard"]),
        }),
        createField("validator_kind", "Validador", "select", rule.validator_kind ?? "", {
          options: [
            { value: "", label: "Sin validador" },
            { value: "no_resurrection", label: "No resurrection" },
          ],
        }),
        createField("source", "Procedencia", "textarea", rule.source ?? "", { rows: 3 }),
        createField("parameters_json", "Parámetros JSON", "textarea", rule.parameters_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Versión", value: String(rule.version) },
        { label: "Actualizado", value: new Date(rule.updated_at_ms).toLocaleString("es") },
      ],
      warnings: ruleWarnings,
      links: [],
    });
    return editor;
  }

  if ("goal" in object) {
    const goal = object.goal;
    const goalWarnings = [...warnings];
    if (goal.visibility === "secret") {
      goalWarnings.push({
        title: "Meta secreta",
        detail: "El contexto puede depender de perspectivas y no solo de canon.",
      });
    }
    const editor = createEditorMode({
      mode: "update",
      objectType: "goal",
      existingUri: result.uri,
      targetUri: result.uri,
      title: previewText(goal.desired_state_md, "Meta"),
      subtitle: humanize(goal.status),
      description: result.snippet,
      logicalPath: pathForUri(state.logicalTree, result.uri),
      objective: `Actualizar meta ${previewText(goal.desired_state_md, "meta")}`,
      sourceUrisText,
      assumptionsText: "",
      fields: [
        createField("holder_entity", "Titular de la meta", "text", goal.holder_entity_id, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("desired_state_md", "Estado deseado", "textarea", goal.desired_state_md, {
          rows: 4,
          required: true,
        }),
        createField("priority", "Prioridad", "number", String(goal.priority), { required: true }),
        createField("status", "Estado", "select", goal.status, {
          required: true,
          options: enumOptions(["active", "achieved", "abandoned", "frustrated"]),
        }),
        createField("visibility", "Visibilidad", "select", goal.visibility, {
          required: true,
          options: enumOptions(["public", "secret"]),
        }),
        createField("source", "Procedencia", "textarea", goal.source ?? "", { rows: 3 }),
        createField("period_start_tick", "Periodo inicio", "number", goal.period?.start_tick?.toString() ?? ""),
        createField("period_end_tick", "Periodo fin", "number", goal.period?.end_tick?.toString() ?? ""),
      ],
      metadata: [
        { label: "URI", value: result.uri },
        { label: "Periodo", value: formatPeriod(goal.period) },
        {
          label: "Holder",
          value: shortId(goal.holder_entity_id),
          uri: `nirmata://entity/${goal.holder_entity_id}`,
        },
      ],
      warnings: goalWarnings,
      links: [
        {
          uri: `nirmata://entity/${goal.holder_entity_id}`,
          label: `Holder · ${shortId(goal.holder_entity_id)}`,
        },
      ],
    });
    return editor;
  }

  const aggregate = object.document;
  const documentObject = aggregate.object;
  const links: JumpLink[] = aggregate.references.map((reference) => ({
    uri: formatObjectRef(reference.target).uri,
    label: formatReferenceLabel(reference.target),
  }));
  if (documentObject.author_entity_id) {
    links.push({
      uri: `nirmata://entity/${documentObject.author_entity_id}`,
      label: `Autor · ${shortId(documentObject.author_entity_id)}`,
    });
  }
  if (documentObject.perspective_entity_id) {
    links.push({
      uri: `nirmata://entity/${documentObject.perspective_entity_id}`,
      label: `Perspectiva · ${shortId(documentObject.perspective_entity_id)}`,
    });
  }
  const documentWarnings = [...warnings];
  if (documentObject.canon_status !== "canonical") {
    documentWarnings.push({
      title: "Documento no canónico",
      detail: "Su contenido debe leerse como perspectiva o apoyo, no como hecho duro.",
    });
  }
  const editor = createEditorMode({
    mode: "update",
    objectType: "document",
    existingUri: result.uri,
    targetUri: result.uri,
    title: documentObject.title,
    subtitle: humanize(documentObject.kind),
    description: result.snippet,
    logicalPath: pathForUri(state.logicalTree, result.uri),
    objective: `Actualizar documento ${documentObject.title}`,
    sourceUrisText,
    assumptionsText: "",
    fields: [
      createField("title", "Título", "text", documentObject.title, { required: true }),
      createField("kind", "Tipo", "text", documentObject.kind, { required: true }),
      createField("author_entity", "Autor", "text", documentObject.author_entity_id ?? "", {
        help: "UUID o nirmata://entity/...",
      }),
      createField("perspective_entity", "Perspectiva", "text", documentObject.perspective_entity_id ?? "", {
        help: "UUID o nirmata://entity/...",
      }),
      createField("canon_status", "Canon", "select", documentObject.canon_status, {
        required: true,
        options: enumOptions(["canonical", "non_canonical"]),
      }),
      createField("body_md", "Cuerpo Markdown", "textarea", documentObject.body_md, { rows: 8 }),
      createField(
        "content_references",
        "Referencias de contenido",
        "textarea",
        serializeContentReferences(aggregate.references),
        { rows: 6, help: "Agrega, abre y reordena referencias sin alterar el tiempo de los eventos." },
      ),
    ],
    metadata: [
      { label: "URI", value: result.uri },
      { label: "Referencias", value: String(aggregate.references.length) },
      { label: "Versión", value: String(documentObject.version) },
    ],
    warnings: documentWarnings,
    links: dedupeLinks(links),
  });
  return editor;
}

function worldFields(world: import("./types.js").World): EditorField[] {
  const calendar = world.calendar ?? null;
  return [
    createField("name", "Nombre", "text", world.name, { required: true }),
    createField("premise_md", "Premisa", "textarea", world.premise_md, { rows: 6 }),
    createField("epoch_label", "Etiqueta del epoch", "text", world.epoch_label),
    createField("calendar_mode", "Calendario", "select", calendar ? "fixed" : "none", {
      options: [
        { value: "none", label: "Sin calendario" },
        { value: "fixed", label: "Calendario fijo" },
      ],
    }),
    createField("calendar_name", "Nombre del calendario", "text", calendar?.name ?? ""),
    createField("calendar_epoch_tick", "Tick del epoch", "text", String(calendar?.epoch_tick ?? 0)),
    createField("calendar_ticks_per_day", "Ticks por día", "text", String(calendar?.ticks_per_day ?? 1)),
    createField(
      "calendar_weekdays",
      "Weekdays (uno por línea)",
      "textarea",
      calendar?.weekday_names.join("\n") ?? "",
      { rows: 4 },
    ),
    createField(
      "calendar_months",
      "Meses (nombre|días)",
      "textarea",
      calendar?.months.map((month) => `${month.name}|${month.days}`).join("\n") ?? "",
      { rows: 5 },
    ),
  ];
}
