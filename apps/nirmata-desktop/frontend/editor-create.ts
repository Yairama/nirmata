import {
  createEditorMode,
  createField,
  currentWorldUri,
  enumOptions,
} from "./editor-model.js";
import { humanize, objectKindFromUri } from "./helpers.js";
import { state } from "./state.js";
import type {
  EditorField,
  EditorMode,
  SearchObjectKind,
} from "./types.js";

function defaultCreateValues(kind: SearchObjectKind): Record<string, string> {
  const selectedEntityUri =
    state.selectedUri && objectKindFromUri(state.selectedUri) === "entity" ? state.selectedUri : "";
  switch (kind) {
    case "entity":
      return {
        kind: "person",
        name: "",
        slug: "",
        aliases: "",
        summary: "",
        body_md: "",
        attributes_json: "{}",
      };
    case "relation":
      return {
        source_entity: selectedEntityUri,
        target_entity: "",
        kind: "",
        direction: "directed",
        certainty: "certain",
        valid_from_tick: "",
        valid_to_tick: "",
        source_reference: "",
        metadata_json: "{}",
      };
    case "event":
      return {
        kind: "",
        summary: "",
        body_md: "",
        time_kind: "unknown",
        time_precision: "unknown",
        time_certainty: "certain",
        start_tick: "",
        end_tick: "",
        location_entity: selectedEntityUri,
        participants: "",
        affected_goal_ids: "",
        causal_links: "",
      };
    case "claim":
      return {
        subject_entity: selectedEntityUri,
        content_md: "",
        predicate_key: "",
        object_kind: "none",
        object_value: "",
        polarity: "positive",
        authentication: "canonical",
        holder_entity: "",
        modality: "",
        register: "",
        epistemic_basis: "",
        source: "",
        source_document: "",
        source_claim: "",
        holder_confidence: "",
        period_start_tick: "",
        period_end_tick: "",
      };
    case "rule":
      return {
        kind: "constitutive",
        statement_md: "",
        scope: "world",
        severity: "advisory",
        validator_kind: "",
        source: "",
        parameters_json: "{}",
      };
    case "goal":
      return {
        holder_entity: selectedEntityUri,
        desired_state_md: "",
        priority: "0",
        status: "active",
        visibility: "public",
        source: "",
        period_start_tick: "",
        period_end_tick: "",
      };
    case "document":
      return {
        title: "",
        kind: "chronicle",
        author_entity: selectedEntityUri,
        perspective_entity: selectedEntityUri,
        canon_status: "canonical",
        body_md: "",
        content_references: "",
      };
  }

}

export function buildCreateEditor(kind: SearchObjectKind): EditorMode {
  const values = defaultCreateValues(kind);
  const sourceUrisText = state.selectedUri ?? "";
  const fields: EditorField[] = [];

  switch (kind) {
    case "entity":
      fields.push(
        createField("kind", "Tipo", "select", values.kind, {
          required: true,
          options: enumOptions(["person", "place", "faction", "culture", "resource", "concept"]),
        }),
        createField("name", "Nombre", "text", values.name, { required: true }),
        createField("slug", "Slug", "text", values.slug, { required: true }),
        createField("aliases", "Aliases (uno por línea)", "textarea", values.aliases, { rows: 4 }),
        createField("summary", "Resumen", "textarea", values.summary, { rows: 4 }),
        createField("body_md", "Cuerpo Markdown", "textarea", values.body_md, { rows: 7 }),
        createField("attributes_json", "Atributos JSON", "textarea", values.attributes_json, {
          rows: 6,
          help: "Debe ser un objeto JSON.",
        }),
      );
      break;
    case "relation":
      fields.push(
        createField("source_entity", "Entidad origen", "text", values.source_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("target_entity", "Entidad destino", "text", values.target_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("kind", "Tipo de relación", "text", values.kind, { required: true }),
        createField("direction", "Dirección", "select", values.direction, {
          required: true,
          options: enumOptions(["directed", "undirected"]),
        }),
        createField("certainty", "Certeza", "select", values.certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("valid_from_tick", "Tick inicio", "number", values.valid_from_tick),
        createField("valid_to_tick", "Tick fin", "number", values.valid_to_tick),
        createField("source_reference", "Procedencia", "textarea", values.source_reference, { rows: 3 }),
        createField("metadata_json", "Metadata JSON", "textarea", values.metadata_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      );
      break;
    case "event":
      fields.push(
        createField("kind", "Tipo", "text", values.kind, { required: true }),
        createField("summary", "Resumen", "textarea", values.summary, { rows: 3, required: true }),
        createField("body_md", "Cuerpo Markdown", "textarea", values.body_md, { rows: 6 }),
        createField("time_kind", "Tipo de tiempo", "select", values.time_kind, {
          required: true,
          options: enumOptions(["unknown", "instant", "interval", "ongoing"]),
        }),
        createField("time_precision", "Precisión", "select", values.time_precision, {
          required: true,
          options: enumOptions(["exact", "day", "month", "year", "era", "unknown"]),
        }),
        createField("time_certainty", "Certeza temporal", "select", values.time_certainty, {
          required: true,
          options: enumOptions(["certain", "approximate", "uncertain", "approximate_uncertain"]),
        }),
        createField("start_tick", "Tick inicio", "number", values.start_tick),
        createField("end_tick", "Tick fin", "number", values.end_tick),
        createField("location_entity", "Entidad lugar", "text", values.location_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("participants", "Participantes (entidad|rol|ordinal)", "textarea", values.participants, {
          rows: 5,
        }),
        createField(
          "affected_goal_ids",
          "Metas afectadas (una por línea)",
          "textarea",
          values.affected_goal_ids,
          { rows: 3, help: "UUID o nirmata://goal/..." },
        ),
        createField("causal_links", "Causalidad (evento|kind)", "textarea", values.causal_links, {
          rows: 4,
          help: "Una línea por vínculo: nirmata://event/<id>|causes",
        }),
      );
      break;
    case "claim":
      fields.push(
        createField("subject_entity", "Entidad sujeto", "text", values.subject_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("content_md", "Contenido Markdown", "textarea", values.content_md, { rows: 5 }),
        createField("predicate_key", "Predicate key", "text", values.predicate_key),
        createField("object_kind", "Tipo de objeto", "select", values.object_kind, {
          options: [
            { value: "none", label: "None" },
            { value: "entity", label: "Entity" },
            { value: "scalar", label: "Scalar" },
          ],
        }),
        createField("object_value", "Objeto", "text", values.object_value, {
          help: "UUID/URI para entity, o texto para scalar.",
        }),
        createField("polarity", "Polaridad", "select", values.polarity, {
          required: true,
          options: enumOptions(["positive", "negative"]),
        }),
        createField("authentication", "Autenticación", "select", values.authentication, {
          required: true,
          options: enumOptions(["canonical", "attributed", "disputed"]),
        }),
        createField("holder_entity", "Holder", "text", values.holder_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("modality", "Modalidad", "select", values.modality, {
          options: [
            { value: "", label: "Sin modalidad" },
            ...enumOptions(["assertion", "belief", "hypothesis", "counterfactual"]),
          ],
        }),
        createField("register", "Registro", "text", values.register),
        createField("epistemic_basis", "Base epistémica", "textarea", values.epistemic_basis, {
          rows: 3,
        }),
        createField("source", "Procedencia textual", "textarea", values.source, { rows: 3 }),
        createField("source_document", "Documento fuente", "text", values.source_document, {
          help: "UUID o nirmata://document/...",
        }),
        createField("source_claim", "Claim fuente", "text", values.source_claim, {
          help: "UUID o nirmata://claim/...",
        }),
        createField("holder_confidence", "Confianza holder", "number", values.holder_confidence),
        createField("period_start_tick", "Periodo inicio", "number", values.period_start_tick),
        createField("period_end_tick", "Periodo fin", "number", values.period_end_tick),
      );
      break;
    case "rule":
      fields.push(
        createField("kind", "Tipo", "select", values.kind, {
          required: true,
          options: enumOptions(["constitutive", "generative", "institutional", "authorial"]),
        }),
        createField("statement_md", "Regla Markdown", "textarea", values.statement_md, {
          rows: 4,
          required: true,
        }),
        createField("scope", "Scope", "text", values.scope, { required: true }),
        createField("severity", "Severidad", "select", values.severity, {
          required: true,
          options: enumOptions(["advisory", "hard"]),
        }),
        createField("validator_kind", "Validador", "select", values.validator_kind, {
          options: [
            { value: "", label: "Sin validador" },
            { value: "no_resurrection", label: "No resurrection" },
          ],
        }),
        createField("source", "Procedencia", "textarea", values.source, { rows: 3 }),
        createField("parameters_json", "Parámetros JSON", "textarea", values.parameters_json, {
          rows: 5,
          help: "Debe ser un objeto JSON.",
        }),
      );
      break;
    case "goal":
      fields.push(
        createField("holder_entity", "Holder", "text", values.holder_entity, {
          required: true,
          help: "UUID o nirmata://entity/...",
        }),
        createField("desired_state_md", "Estado deseado", "textarea", values.desired_state_md, {
          rows: 4,
          required: true,
        }),
        createField("priority", "Prioridad", "number", values.priority, { required: true }),
        createField("status", "Estado", "select", values.status, {
          required: true,
          options: enumOptions(["active", "achieved", "abandoned", "frustrated"]),
        }),
        createField("visibility", "Visibilidad", "select", values.visibility, {
          required: true,
          options: enumOptions(["public", "secret"]),
        }),
        createField("source", "Procedencia", "textarea", values.source, { rows: 3 }),
        createField("period_start_tick", "Periodo inicio", "number", values.period_start_tick),
        createField("period_end_tick", "Periodo fin", "number", values.period_end_tick),
      );
      break;
    case "document":
      fields.push(
        createField("title", "Título", "text", values.title, { required: true }),
        createField("kind", "Tipo", "text", values.kind, { required: true }),
        createField("author_entity", "Autor", "text", values.author_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("perspective_entity", "Perspectiva", "text", values.perspective_entity, {
          help: "UUID o nirmata://entity/...",
        }),
        createField("canon_status", "Canon", "select", values.canon_status, {
          required: true,
          options: enumOptions(["canonical", "non_canonical"]),
        }),
        createField("body_md", "Cuerpo Markdown", "textarea", values.body_md, { rows: 8 }),
        createField(
          "content_references",
          "Referencias de contenido",
          "textarea",
          values.content_references,
          { rows: 6, help: "Usa el editor inferior para añadir, abrir y reordenar referencias." },
        ),
      );
      break;
  }


  return createEditorMode({
    mode: "create",
    objectType: kind,
    existingUri: null,
    targetUri: null,
    title: `Nuevo ${humanize(kind).toLowerCase()}`,
    subtitle: `Draft manual · ${humanize(kind)}`,
    description: `Completa un formulario mínimo para proponer un nuevo ${humanize(kind).toLowerCase()}.`,
    logicalPath: null,
    fields,
    metadata: state.session ? [{ label: "Mundo", value: state.session.world.name }] : [],
    warnings: [],
    links: [],
    objective: `Create ${kind}`,
    sourceUrisText,
    assumptionsText: "",
  });
}
