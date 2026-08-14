import { useQuery } from "@tanstack/react-query";
import { Fragment, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useFieldArray, useForm, useWatch } from "react-hook-form";
import type { Control, UseFormRegister, UseFormSetValue } from "react-hook-form";
import { humanize, objectKindFromUri, pathForUri, setStatus } from "./helpers.js";
import { buildSelectionEditor } from "./editor-model.js";
import { useObjectPicker } from "./object-picker.js";
import { useSession } from "./session-provider.js";
import {
  appActions,
  getAppState,
  useAppState,
} from "./state.js";
import type {
  EditorField,
  ManualDraftRequest,
  ObjectKind,
  SearchObjectKind,
  SearchResult,
  StructuredEditorState,
} from "./types.js";
import {
  resetCurrentEditor,
  saveCurrentDraft,
  selectUri,
} from "./workspace.js";
import { observedScopeQueryKey, openUriQuery, useWorkspaceData } from "./workspace-data.js";

type NamedReference = { uri: string; label: string };
type EditorForm = {
  values: Record<string, string>;
  objective: string;
  assumptionsText: string;
  weekdays: Array<{ name: string }>;
  months: Array<{ name: string; days: string }>;
  participants: Array<NamedReference & { role: string }>;
  causalLinks: Array<NamedReference & { kind: string }>;
  affectedGoals: NamedReference[];
  documentReferences: NamedReference[];
  sources: NamedReference[];
  startDate: CalendarDateParts;
  endDate: CalendarDateParts;
};

type CalendarDateParts = { year: string; month: string; day: string; unit: string };
type ArrayActions = {
  move: (from: number, to: number) => void;
  remove: (index: number) => void;
};

const allReferenceKinds: SearchObjectKind[] = [
  "entity", "relation", "event", "claim", "rule", "goal", "document",
];

const choiceLabels: Record<string, string> = {
  person: "Persona",
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
  unknown: "No especificado",
  instant: "Instante",
  interval: "Intervalo",
  ongoing: "En curso",
  exact: "Exacta",
  day: "Día",
  month: "Mes",
  year: "Año",
  era: "Era",
  none: "Sin objeto",
  entity: "Otra entidad",
  scalar: "Valor textual",
  positive: "Positiva",
  negative: "Negativa",
  canonical: "Canónica",
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
  no_resurrection: "Impedir resurrección",
  active: "Activa",
  achieved: "Lograda",
  abandoned: "Abandonada",
  frustrated: "Frustrada",
  public: "Pública",
  secret: "Secreta",
  non_canonical: "No canónico",
  fixed: "Calendario fijo",
  enables: "Habilita",
  causes: "Causa",
  motivates: "Motiva",
  prevents: "Impide",
  terminates: "Termina",
  reveals: "Revela",
};

const scalarReferences: Record<string, { kinds: SearchObjectKind[]; required?: boolean }> = {
  "relation:source_entity": { kinds: ["entity"], required: true },
  "relation:target_entity": { kinds: ["entity"], required: true },
  "event:location_entity": { kinds: ["entity"] },
  "claim:subject_entity": { kinds: ["entity"], required: true },
  "claim:holder_entity": { kinds: ["entity"] },
  "claim:source_document": { kinds: ["document"] },
  "claim:source_claim": { kinds: ["claim"] },
  "goal:holder_entity": { kinds: ["entity"], required: true },
  "document:author_entity": { kinds: ["entity"] },
  "document:perspective_entity": { kinds: ["entity"] },
};

const specialFields = new Set([
  "calendar_weekdays", "calendar_months", "participants", "affected_goal_ids",
  "causal_links", "content_references", "start_calendar_date", "end_calendar_date",
]);

const advancedFields = new Set([
  "attributes_json", "metadata_json", "parameters_json", "valid_from_tick", "valid_to_tick",
  "start_tick", "end_tick", "calendar_epoch_tick", "calendar_ticks_per_day",
  "period_start_tick", "period_end_tick", "validator_kind", "holder_confidence",
]);

function lines(value: string): string[] {
  return value.split(/\r?\n/u).map((item) => item.trim()).filter(Boolean);
}

function parseDate(value: string): CalendarDateParts {
  const [year = "0", month = "1", day = "1", unit = "0"] = value.split("|", 4);
  return { year, month, day, unit };
}

function referenceUri(value: string, kind?: SearchObjectKind): string {
  const trimmed = value.trim();
  if (!trimmed || trimmed.startsWith("nirmata://") || !kind) return trimmed;
  return `nirmata://${kind}/${trimmed}`;
}

function parseReferenceRows(value: string): NamedReference[] {
  return lines(value).map((line) => {
    const parts = line.split("|").map((item) => item.trim());
    const uri = /^\d+$/u.test(parts[0] ?? "") ? parts[1] ?? "" : parts[0] ?? "";
    return { uri, label: "" };
  }).filter((item) => item.uri);
}

function formDefaults(editor: StructuredEditorState | null): EditorForm {
  const values = editor?.values ?? {};
  return {
    values: { ...values },
    objective: editor?.objective ?? "",
    assumptionsText: editor?.assumptionsText ?? "",
    weekdays: lines(values.calendar_weekdays ?? "").map((name) => ({ name })),
    months: lines(values.calendar_months ?? "").map((row) => {
      const [name = "", days = "1"] = row.split("|", 2);
      return { name, days };
    }),
    participants: lines(values.participants ?? "").map((row) => {
      const [entity = "", role = ""] = row.split("|", 3);
      return { uri: entity, label: "", role };
    }),
    causalLinks: lines(values.causal_links ?? "").map((row) => {
      const [event = "", kind = "causes"] = row.split("|", 2);
      return { uri: event, label: "", kind };
    }),
    affectedGoals: lines(values.affected_goal_ids ?? "").map((goal) => ({
      uri: goal, label: "",
    })),
    documentReferences: parseReferenceRows(values.content_references ?? ""),
    sources: lines(editor?.sourceUrisText ?? "").map((uri) => ({ uri, label: "" })),
    startDate: parseDate(values.start_calendar_date ?? ""),
    endDate: parseDate(values.end_calendar_date ?? ""),
  };
}

function requestFromForm(editor: StructuredEditorState, data: EditorForm): ManualDraftRequest {
  const values = { ...data.values };
  if (editor.objectType === "world") {
    values.calendar_weekdays = data.weekdays.map((row) => row.name.trim()).filter(Boolean).join("\n");
    values.calendar_months = data.months
      .map((row) => `${row.name.trim()}|${row.days.trim()}`)
      .filter((row) => !row.startsWith("|"))
      .join("\n");
  }
  if (editor.objectType === "event") {
    values.participants = data.participants
      .filter((row) => row.uri.trim())
      .map((row, index) => `${row.uri.trim()}|${row.role.trim()}|${index}`)
      .join("\n");
    values.causal_links = data.causalLinks
      .filter((row) => row.uri.trim())
      .map((row) => `${row.uri.trim()}|${row.kind}`)
      .join("\n");
    values.affected_goal_ids = data.affectedGoals.map((row) => row.uri.trim()).filter(Boolean).join("\n");
  }
  if (editor.objectType === "document") {
    values.content_references = data.documentReferences
      .map((row, index) => `${row.uri.trim()}|${index}`)
      .filter((row) => !row.startsWith("|"))
      .join("\n");
  }

  if (editor.objectType === "event") {
    const kind = values.time_kind;
    const calendar = getAppState().session?.world.calendar;
    values.start_calendar_date = kind !== "unknown" && calendar
      ? `${data.startDate.year}|${data.startDate.month}|${data.startDate.day}|${data.startDate.unit || "0"}`
      : "";
    values.end_calendar_date = kind === "interval" && calendar
      ? `${data.endDate.year}|${data.endDate.month}|${data.endDate.day}|${data.endDate.unit || "0"}`
      : "";
    if (values.start_calendar_date) values.start_tick = "";
    if (values.end_calendar_date) values.end_tick = "";
    if (kind === "unknown") {
      values.start_tick = "";
      values.end_tick = "";
    } else if (kind !== "interval") {
      values.end_tick = "";
    }
  }
  if (editor.objectType === "world" && values.calendar_mode !== "fixed") {
    for (const key of ["calendar_name", "calendar_epoch_tick", "calendar_ticks_per_day", "calendar_weekdays", "calendar_months"]) {
      values[key] = "";
    }
  }
  if (editor.objectType === "claim") {
    if (values.object_kind === "none") values.object_value = "";
    if (values.authentication === "canonical") {
      values.holder_entity = "";
      values.modality = "";
      values.holder_confidence = "";
    }
  }
  return {
    objectType: editor.objectType,
    existingUri: editor.existingUri ?? undefined,
    objective: data.objective.trim() || undefined,
    sourceUris: data.sources.map((source) => source.uri.trim()).filter(Boolean),
    assumptions: lines(data.assumptionsText),
    values,
  };
}

export function StructuredEditor({ onPendingReviewsChanged, onStartProposal }: {
  onPendingReviewsChanged?: () => void;
  onStartProposal?: (request: string) => void;
}) {
  const requestObjectPicker = useObjectPicker();
  const state = useAppState();
  const editor = state.structuredEditor;
  const session = useSession();
  const workspaceData = useWorkspaceData();
  const form = useForm<EditorForm>({ defaultValues: formDefaults(editor), shouldUnregister: false });
  const { control, register, reset, setError, clearErrors, setValue, getValues, handleSubmit, formState } = form;
  const weekdays = useFieldArray({ control, name: "weekdays" });
  const months = useFieldArray({ control, name: "months" });
  const participants = useFieldArray({ control, name: "participants" });
  const causalLinks = useFieldArray({ control, name: "causalLinks" });
  const affectedGoals = useFieldArray({ control, name: "affectedGoals" });
  const documentReferences = useFieldArray({ control, name: "documentReferences" });
  const sources = useFieldArray({ control, name: "sources" });
  const previousEditor = useRef(editor);
  const issueSignature = editor?.issues.map((issue) => `${issue.field}:${issue.message}`).join("|") ?? "";

  useEffect(() => {
    const selectedObject = workspaceData.selectedObject;
    const relatedContext = workspaceData.relatedContext;
    if (!session || !workspaceData.selectedUri || !selectedObject.data || !relatedContext.data) return;
    if (state.selectedUri !== workspaceData.selectedUri) return;
    if (state.structuredEditor?.existingUri === workspaceData.selectedUri) return;
    const nextEditor = buildSelectionEditor(selectedObject.data);
    appActions.setStructuredEditor(nextEditor);
    appActions.setSelectedLogicalPath(pathForUri(state.logicalTree, workspaceData.selectedUri));
    appActions.recordRecentUri(workspaceData.selectedUri);
    setStatus(`Selección actualizada: ${nextEditor.title}.`);
  }, [
    session?.world_id,
    state.structuredEditor?.existingUri,
    state.selectedUri,
    workspaceData.relatedContext.data,
    workspaceData.selectedObject.data,
    workspaceData.selectedUri,
  ]);

  useEffect(() => {
    if (previousEditor.current === editor) return;
    previousEditor.current = editor;
    reset(formDefaults(editor));
  }, [editor, reset]);

  useEffect(() => {
    clearErrors();
    if (!editor || editor.issues.length === 0) return;
    for (const issue of editor.issues) {
      const name = issue.field === "sourceUris" ? "sources" : `values.${issue.field}`;
      setError(name as "values", { type: "server", message: issue.message });
    }
    window.requestAnimationFrame(() => {
      const first = editor.issues[0];
      const target = document.querySelector<HTMLElement>(`#editor-panel [data-editor-field="${CSS.escape(first.field)}"]`);
      target?.closest("details")?.setAttribute("open", "");
      (target?.matches("input, textarea, select, button") ? target : target?.querySelector<HTMLElement>("input, textarea, select, button"))?.focus();
    });
  }, [clearErrors, editor, issueSignature, setError]);

  useEffect(() => {
    appActions.setEphemeralWork("editor", "cambios del formulario", formState.isDirty);
    if (formState.isDirty) appActions.setWorkspaceNotice(null);
    return () => appActions.setEphemeralWork("editor", "", false);
  }, [formState.isDirty]);

  if (!editor) {
    return (
      <>
        <div className="panel-header"><div><p className="panel-eyebrow">Formularios manuales</p><h3 id="editor-title">Selecciona o crea un objeto</h3></div></div>
        <div className="panel-body">
          {(workspaceData.selectedObject.isPending || workspaceData.relatedContext.isPending) && workspaceData.selectedUri
            ? <p role="status" className="empty-state">Cargando la selección de esta versión…</p>
            : (workspaceData.selectedObject.isError || workspaceData.relatedContext.isError) && workspaceData.selectedUri
              ? <p role="alert" className="notice warning">No se pudo abrir este objeto en la versión observada.</p>
              : <p className="empty-state">El panel central permite editar la selección o preparar un cambio nuevo.</p>}
        </div>
      </>
    );
  }

  const issueMap = new Map<string, string[]>();
  for (const issue of editor.issues) issueMap.set(issue.field, [...(issueMap.get(issue.field) ?? []), issue.message]);
  const regular = editor.fields.filter((field) => !specialFields.has(field.key) && !advancedFields.has(field.key));
  const advanced = editor.fields.filter((field) => !specialFields.has(field.key) && advancedFields.has(field.key));
  const timeKind = form.watch("values.time_kind");
  const calendarMode = form.watch("values.calendar_mode");
  const objectKind = form.watch("values.object_kind");
  const authentication = form.watch("values.authentication");

  function visible(field: EditorField): boolean {
    if (editor!.objectType === "world" && field.key.startsWith("calendar_") && field.key !== "calendar_mode") {
      return calendarMode === "fixed";
    }
    if (editor!.objectType === "claim" && field.key === "object_value") return objectKind !== "none";
    if (editor!.objectType === "claim" && ["holder_entity", "modality", "holder_confidence"].includes(field.key)) {
      return authentication !== "canonical";
    }
    if (editor!.objectType === "event" && field.key === "start_tick") return timeKind !== "unknown";
    if (editor!.objectType === "event" && field.key === "end_tick") return timeKind === "interval";
    return true;
  }

  async function submit(data: EditorForm) {
    if (await saveCurrentDraft(requestFromForm(editor!, data))) onPendingReviewsChanged?.();
  }

  return (
    <>
      <div className="panel-header">
        <div><p className="panel-eyebrow">Formularios manuales</p><h3 id="editor-title">{editor.title}</h3></div>
        <p className="panel-summary">{editor.mode === "create" ? "Nuevo" : "Editar"} · {kindLabel(editor.objectType)}</p>
      </div>
      <div className="panel-body">
        <form className="editor-layout" onSubmit={handleSubmit(submit)}>
          {state.workspaceNotice && <section className={`notice ${state.workspaceNotice.kind}`}><h4>{state.workspaceNotice.title}</h4><p>{state.workspaceNotice.detail}</p></section>}
          {workspaceData.selectedUri === editor.existingUri && (
            <section className="editor-section"><h4>Acciones</h4><div className="editor-toolbar"><button type="button" className="secondary" disabled={Boolean(session?.read_only)} onClick={() => onStartProposal?.(`Propón un cambio acotado sobre ${editor.title}: `)}>Pedir un cambio sobre la selección</button></div></section>
          )}
          <section className="editor-section"><h4>Lectura activa</h4><p>{editor.description}</p>{editor.logicalPath && <p className="path muted">{editor.logicalPath}</p>}</section>
          <fieldset disabled={Boolean(session?.read_only || formState.isSubmitting)} className="editor-form-fieldset">
            <section className="editor-section">
              <h4>Formulario</h4>
              <div className="editor-fields">
                {regular.filter(visible).map((field) => <EditorControl key={field.key} editor={editor} field={field} control={control} register={register} setValue={setValue} messages={issueMap.get(field.key) ?? []} />)}
                {editor.objectType === "world" && calendarMode === "fixed" && (
                  <>
                    <CalendarRows title="Días de la semana" singular="día" fields={weekdays.fields} register={register} actions={weekdays} append={() => weekdays.append({ name: "" })} messages={issueMap.get("calendar_weekdays") ?? []} />
                    <CalendarRows title="Meses del año" singular="mes" fields={months.fields} register={register} actions={months} append={() => months.append({ name: "", days: "1" })} months messages={issueMap.get("calendar_months") ?? []} />
                  </>
                )}
                {editor.objectType === "event" && <EventDates timeKind={timeKind} control={control} register={register} messages={issueMap} />}
                {editor.objectType === "event" && (
                  <>
                    <Participants fields={participants.fields} register={register} setValue={setValue} actions={participants} append={participants.append} messages={issueMap.get("participants") ?? []} />
                    <ReferenceList title="Metas afectadas" field="affectedGoals" kind="goal" fields={affectedGoals.fields} register={register} setValue={setValue} actions={affectedGoals} append={affectedGoals.append} multiple messages={issueMap.get("affected_goal_ids") ?? []} />
                    <CausalLinks fields={causalLinks.fields} register={register} setValue={setValue} actions={causalLinks} append={causalLinks.append} messages={issueMap.get("causal_links") ?? []} />
                  </>
                )}
                {editor.objectType === "document" && <ReferenceList title="Referencias de contenido" field="documentReferences" fields={documentReferences.fields} register={register} setValue={setValue} actions={documentReferences} append={documentReferences.append} multiple messages={issueMap.get("content_references") ?? []} />}
              </div>
              {advanced.filter(visible).length > 0 && (
                <details className="editor-advanced-options"><summary>Opciones avanzadas</summary><div className="editor-fields advanced-editor-fields">
                  {advanced.filter(visible).map((field) => <EditorControl key={field.key} editor={editor} field={field} control={control} register={register} setValue={setValue} messages={issueMap.get(field.key) ?? []} />)}
                </div></details>
              )}
            </section>
            <section className="editor-section">
              <h4>Preparar cambios</h4>
              <label>Objetivo de la propuesta<input {...register("objective")} name="objective" autoComplete="off" /></label>
              <ReferenceList title="Fuentes internas (opcional)" field="sources" fields={sources.fields} register={register} setValue={setValue} actions={sources} append={sources.append} multiple messages={issueMap.get("sourceUris") ?? []} />
              <label>Supuestos (uno por línea)<textarea {...register("assumptionsText")} name="assumptionsText" rows={4} /></label>
              <div className="form-actions">
                <button type="submit">{formState.isSubmitting ? "Preparando…" : "Preparar cambios"}</button>
                <button type="button" className="secondary" onClick={resetCurrentEditor}>Revertir formulario</button>
              </div>
            </section>
          </fieldset>
          <details className="editor-section editor-technical-details">
            <summary>Detalles técnicos</summary>
            <dl className="meta-list">{editor.metadata.map((item) => <div className="meta-row" key={item.label}><dt>{item.label}</dt><dd>{item.uri ? <button type="button" className="meta-link" onClick={() => void selectUri(item.uri!)}>{item.value}</button> : item.value}</dd></div>)}</dl>
          </details>
        </form>
      </div>
    </>
  );
}

function EditorControl({ editor, field, control, register, setValue, messages }: {
  editor: StructuredEditorState;
  field: EditorField;
  control: Control<EditorForm>;
  register: UseFormRegister<EditorForm>;
  setValue: UseFormSetValue<EditorForm>;
  messages: string[];
}) {
  const requestObjectPicker = useObjectPicker();
  const name = `values.${field.key}` as const;
  const value = useWatch({ control, name }) ?? "";
  const reference = scalarReferences[`${editor.objectType}:${field.key}`]
    ?? (editor.objectType === "claim" && field.key === "object_value" && useWatch({ control, name: "values.object_kind" }) === "entity" ? { kinds: ["entity" as const] } : null);
  const id = `editor-${editor.objectType}-${field.key}`;
  const errorId = `${id}-error`;
  const registration = register(name);
  const inputProps = {
    ...registration,
    id,
    "aria-required": field.required || reference?.required ? true : undefined,
    "aria-invalid": messages.length > 0 ? true : undefined,
    "aria-describedby": messages.length > 0 ? errorId : undefined,
    "data-editor-field": field.key,
  };
  return (
    <div className={`editor-field${value !== editor.baselineValues[field.key] ? " dirty" : ""}`}>
      {reference ? (
        <>
          <span className="field-label">{field.label}</span>
          <div className="field-picker-row">
            <ReferenceName uri={referenceUri(value, reference.kinds[0])} fallback="Sin selección" />
            <button type="button" className="secondary" onClick={(event) => requestObjectPicker({ title: field.label, kinds: [...reference.kinds], multiple: false, returnFocus: event.currentTarget, apply: ([result]) => setValue(name, result.uri, { shouldDirty: true }) })}>Elegir por nombre</button>
            {value && <button type="button" className="ghost" onClick={() => setValue(name, "", { shouldDirty: true })}>Quitar</button>}
          </div>
          <details className="technical-details field-technical-input"><summary>Introducir UUID o URI manualmente</summary><label htmlFor={id}>{field.label}, valor técnico<input {...inputProps} type="text" value={value} /></label></details>
        </>
      ) : (
        <label htmlFor={id}>{field.label}
          {field.control === "textarea" ? <textarea {...inputProps} rows={field.rows ?? 5} value={value} />
            : field.control === "select" ? <select {...inputProps} value={value}>{field.options?.map((option) => <option key={option.value} value={option.value}>{choiceLabels[option.value] ?? option.label}</option>)}</select>
              : <input {...inputProps} type={field.control === "number" ? "number" : "text"} value={value} />}
        </label>
      )}
      {field.help && !reference && <p className="field-help">{field.help}</p>}
      {field.key.endsWith("_md") || field.key === "body_md" ? <MarkdownPreview value={value} /> : null}
      <FieldErrors id={errorId} messages={messages} />
    </div>
  );
}

function CalendarRows({ title, singular, fields, register, actions, append, months: hasDays = false, messages }: {
  title: string;
  singular: string;
  fields: Array<{ id: string }>;
  register: UseFormRegister<EditorForm>;
  actions: ArrayActions;
  append: () => void;
  months?: boolean;
  messages: string[];
}) {
  const fieldKey = hasDays ? "calendar_months" : "calendar_weekdays";
  return (
    <section className="editor-field calendar-builder" data-editor-field={fieldKey} tabIndex={messages.length ? -1 : undefined} aria-invalid={messages.length ? true : undefined} aria-describedby={messages.length ? `${fieldKey}-error` : undefined}>
      <h5>{title}</h5>
      <div className="calendar-builder-list">
        {fields.length === 0 && <p className="muted">Añade al menos un {singular}.</p>}
        {fields.map((field, index) => <div className="calendar-builder-row" key={field.id}>
          <label>Nombre del {singular} {index + 1}<input {...register(`${hasDays ? "months" : "weekdays"}.${index}.name`)} /></label>
          {hasDays && <label>Días<input type="number" min="1" step="1" {...register(`months.${index}.days`)} /></label>}
          <RowActions noun={singular} index={index} count={fields.length} actions={actions} />
        </div>)}
      </div>
      <button type="button" className="secondary" onClick={append}>Agregar {singular}</button>
      <FieldErrors id={`${fieldKey}-error`} messages={messages} />
    </section>
  );
}

function EventDates({ timeKind, control, register, messages }: { timeKind: string; control: Control<EditorForm>; register: UseFormRegister<EditorForm>; messages: Map<string, string[]> }) {
  const calendar = getAppState().session?.world.calendar;
  if (timeKind === "unknown") return <section className="editor-field event-date-builder"><h5>Fecha del acontecimiento</h5><p className="muted">El tiempo queda sin especificar y no necesita fecha.</p></section>;
  if (!calendar) return <section className="editor-field event-date-builder"><h5>Fecha del acontecimiento</h5><p className="notice info">Este mundo no tiene calendario de presentación. La unidad temporal canónica permanece en Detalles técnicos.</p></section>;
  return (
    <section className="editor-field event-date-builder"><h5>Fecha del acontecimiento</h5>
      <DateEndpoint endpoint="startDate" label={timeKind === "instant" ? "Fecha" : "Inicio"} suffix="inicio" calendar={calendar} control={control} register={register} messages={messages.get("start_calendar_date") ?? []} />
      {timeKind === "interval" && <DateEndpoint endpoint="endDate" label="Fin" suffix="fin" calendar={calendar} control={control} register={register} messages={messages.get("end_calendar_date") ?? []} />}
    </section>
  );
}

function DateEndpoint({ endpoint, label, suffix, calendar, control, register, messages }: { endpoint: "startDate" | "endDate"; label: string; suffix: string; calendar: NonNullable<ReturnType<typeof useSession>>["world"]["calendar"] & {}; control: Control<EditorForm>; register: UseFormRegister<EditorForm>; messages: string[] }) {
  const month = useWatch({ control, name: `${endpoint}.month` }) || "1";
  const maxDay = calendar?.months[Number(month) - 1]?.days ?? 1;
  const field = endpoint === "startDate" ? "start_calendar_date" : "end_calendar_date";
  return (
    <fieldset className="event-date-row" data-editor-field={field} aria-invalid={messages.length ? true : undefined} aria-describedby={messages.length ? `${field}-error` : undefined}>
      <legend>{label}</legend>
      <label>Año de {suffix}<input type="number" step="1" {...register(`${endpoint}.year`)} /></label>
      <label>Mes de {suffix}<select {...register(`${endpoint}.month`)}>{calendar?.months.map((item, index) => <option key={`${item.name}-${index}`} value={index + 1}>{item.name}</option>)}</select></label>
      <label>Día de {suffix}<input type="number" min="1" max={maxDay} step="1" {...register(`${endpoint}.day`)} /></label>
      {(calendar?.ticks_per_day ?? 1) > 1 && <label>Unidad del día de {suffix}<input type="number" min="0" max={(calendar?.ticks_per_day ?? 1) - 1} step="1" {...register(`${endpoint}.unit`)} /></label>}
      <FieldErrors id={`${field}-error`} messages={messages} />
    </fieldset>
  );
}

function Participants({ fields, register, setValue, actions, append, messages }: { fields: Array<NamedReference & { id: string; role: string }>; register: UseFormRegister<EditorForm>; setValue: UseFormSetValue<EditorForm>; actions: ArrayActions; append: (value: NamedReference & { role: string }) => void; messages: string[] }) {
  const requestObjectPicker = useObjectPicker();
  return (
    <section className="editor-field composed-list" data-editor-field="participants" tabIndex={messages.length ? -1 : undefined} aria-invalid={messages.length ? true : undefined} aria-describedby={messages.length ? "participants-error" : undefined}>
      <h5>Participantes</h5>
      {fields.map((field, index) => <div className="composed-list-row" key={field.id}>
        <ReferencePicker label={`Participante ${index + 1}`} uri={field.uri} kinds={["entity"]} onChange={(result) => setValue(`participants.${index}.uri`, result.uri, { shouldDirty: true })} />
        <label>Rol<input {...register(`participants.${index}.role`)} /></label>
        <TechnicalReference name={`participants.${index}.uri`} label="Entidad participante" register={register} />
        <RowActions noun="participante" index={index} count={fields.length} actions={actions} />
      </div>)}
      <button type="button" className="secondary" onClick={(event) => requestObjectPicker({ title: "Agregar participante", kinds: ["entity"], multiple: false, returnFocus: event.currentTarget, apply: ([result]) => append({ uri: result.uri, label: cleanLabel(result), role: "" }) })}>Agregar participante</button>
      <FieldErrors id="participants-error" messages={messages} />
    </section>
  );
}

function CausalLinks({ fields, register, setValue, actions, append, messages }: { fields: Array<NamedReference & { id: string; kind: string }>; register: UseFormRegister<EditorForm>; setValue: UseFormSetValue<EditorForm>; actions: ArrayActions; append: (value: NamedReference & { kind: string }) => void; messages: string[] }) {
  const requestObjectPicker = useObjectPicker();
  return (
    <section className="editor-field composed-list" data-editor-field="causal_links" tabIndex={messages.length ? -1 : undefined} aria-invalid={messages.length ? true : undefined} aria-describedby={messages.length ? "causal-links-error" : undefined}>
      <h5>Vínculos causales</h5>
      {fields.map((field, index) => <div className="composed-list-row" key={field.id}>
        <ReferencePicker label={`Acontecimiento ${index + 1}`} uri={field.uri} kinds={["event"]} onChange={(result) => setValue(`causalLinks.${index}.uri`, result.uri, { shouldDirty: true })} />
        <label>Tipo de vínculo<select {...register(`causalLinks.${index}.kind`)}>{["enables", "causes", "motivates", "prevents", "terminates", "reveals"].map((kind) => <option key={kind} value={kind}>{choiceLabels[kind]}</option>)}</select></label>
        <TechnicalReference name={`causalLinks.${index}.uri`} label="Acontecimiento" register={register} />
        <RowActions noun="vínculo" index={index} count={fields.length} actions={actions} />
      </div>)}
      <button type="button" className="secondary" onClick={(event) => requestObjectPicker({ title: "Agregar vínculo causal", kinds: ["event"], multiple: false, returnFocus: event.currentTarget, apply: ([result]) => append({ uri: result.uri, label: cleanLabel(result), kind: "causes" }) })}>Agregar vínculo causal</button>
      <FieldErrors id="causal-links-error" messages={messages} />
    </section>
  );
}

function ReferenceList({ title, field, kind, fields, register, setValue, actions, append, multiple = false, messages }: { title: string; field: "affectedGoals" | "documentReferences" | "sources"; kind?: SearchObjectKind; fields: Array<NamedReference & { id: string }>; register: UseFormRegister<EditorForm>; setValue: UseFormSetValue<EditorForm>; actions: ArrayActions; append: (value: NamedReference | NamedReference[]) => void; multiple?: boolean; messages: string[] }) {
  const requestObjectPicker = useObjectPicker();
  const kinds = kind ? [kind] : allReferenceKinds;
  const backendField = field === "affectedGoals" ? "affected_goal_ids" : field === "documentReferences" ? "content_references" : "sourceUris";
  return (
    <section className="editor-field composed-list" data-editor-field={backendField} tabIndex={messages.length ? -1 : undefined} aria-invalid={messages.length ? true : undefined} aria-describedby={messages.length ? `${field}-error` : undefined}>
      <h5>{title}</h5>
      {fields.length === 0 && <p className="muted">Sin referencias.</p>}
      {fields.map((item, index) => <div className="composed-list-row reference-list-row" key={item.id}>
        <ReferencePicker label={`Referencia ${index + 1}`} uri={item.uri} kinds={kinds} onChange={(result) => setValue(`${field}.${index}.uri`, result.uri, { shouldDirty: true })} />
        <TechnicalReference name={`${field}.${index}.uri`} label="Referencia" register={register} />
        <RowActions noun="referencia" index={index} count={fields.length} actions={actions} />
      </div>)}
      <button type="button" className="secondary" aria-label={`Agregar en ${title} por nombre`} onClick={(event) => requestObjectPicker({ title, kinds, multiple, returnFocus: event.currentTarget, apply: (results) => {
        for (const result of results) {
          if (!fields.some((item) => item.uri === result.uri)) append({ uri: result.uri, label: cleanLabel(result) });
        }
      } })}>Agregar por nombre</button>
      <FieldErrors id={`${field}-error`} messages={messages} />
    </section>
  );
}

function ReferencePicker({ label, uri, kinds, onChange }: { label: string; uri: string; kinds: SearchObjectKind[]; onChange: (result: SearchResult) => void }) {
  const requestObjectPicker = useObjectPicker();
  return <div className="field-picker-row"><span><strong>{label}</strong><br /><ReferenceName uri={referenceUri(uri, kinds[0])} fallback="Sin selección" /></span><button type="button" className="secondary" onClick={(event) => requestObjectPicker({ title: label, kinds, multiple: false, returnFocus: event.currentTarget, apply: ([result]) => onChange(result) })}>Cambiar por nombre</button></div>;
}

function TechnicalReference({ name, label, register }: { name: `participants.${number}.uri` | `causalLinks.${number}.uri` | `affectedGoals.${number}.uri` | `documentReferences.${number}.uri` | `sources.${number}.uri`; label: string; register: UseFormRegister<EditorForm> }) {
  return <details className="technical-details field-technical-input"><summary>Detalles técnicos</summary><label>{label}, UUID o URI<input {...register(name)} /></label></details>;
}

function RowActions({ noun, index, count, actions }: { noun: string; index: number; count: number; actions: ArrayActions }) {
  return <div className="calendar-row-actions"><button type="button" className="secondary" aria-label={`Subir ${noun} ${index + 1}`} disabled={index === 0} onClick={() => actions.move(index, index - 1)}>↑</button><button type="button" className="secondary" aria-label={`Bajar ${noun} ${index + 1}`} disabled={index === count - 1} onClick={() => actions.move(index, index + 1)}>↓</button><button type="button" className="ghost" aria-label={`Quitar ${noun} ${index + 1}`} onClick={() => actions.remove(index)}>Quitar</button></div>;
}

function ReferenceName({ uri, fallback }: { uri: string; fallback: string }) {
  const session = useSession();
  const normalized = uri.startsWith("nirmata://") ? uri : "";
  const query = useQuery({
    ...openUriQuery(session, normalized),
    enabled: Boolean(session && normalized),
    retry: false,
  });
  if (!uri) return <span className="field-picker-status">{fallback}</span>;
  return <span className="field-picker-status">{query.data ? query.data.result.snippet.replace(/[\[\]]/gu, "") : query.isError ? "Referencia no disponible" : "Referencia seleccionada"}</span>;
}

function MarkdownPreview({ value }: { value: string }) {
  const [open, setOpen] = useState(false);
  return <><button type="button" className="secondary markdown-preview-toggle" aria-expanded={open} onClick={() => setOpen((current) => !current)}>{open ? "Ocultar vista previa" : "Mostrar vista previa segura"}</button>{open && <div className="markdown-preview" data-markdown-mode="safe-preview">{safeMarkdown(value)}</div>}</>;
}

function safeMarkdown(value: string): ReactNode {
  if (!value.trim()) return <p className="muted">La vista previa aparecerá aquí.</p>;
  return value.split(/\r?\n/u).map((line, index) => {
    const heading = /^(#{1,4})\s+(.+)$/u.exec(line);
    const item = /^[-*]\s+(.+)$/u.exec(line);
    const content = safeInlineMarkdown(heading?.[2] ?? item?.[1] ?? line);
    if (heading?.[1].length === 1) return <h2 key={index}>{content}</h2>;
    if (heading?.[1].length === 2) return <h3 key={index}>{content}</h3>;
    if (heading) return <h4 key={index}>{content}</h4>;
    if (item) return <li key={index}>{content}</li>;
    return <p key={index}>{content}</p>;
  });
}

function safeInlineMarkdown(value: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /\[([^\]]+)\]\(([^)]+)\)/gu;
  let offset = 0;
  for (const match of value.matchAll(pattern)) {
    nodes.push(value.slice(offset, match.index));
    const label = match[1];
    const target = match[2].trim();
    if (target.startsWith("nirmata://") && objectKindFromUri(target)) {
      nodes.push(<button key={`${match.index}-${target}`} type="button" className="meta-link" title="Abrir referencia interna" onClick={() => void selectUri(target)}>{label}</button>);
    } else if (/^https:\/\//iu.test(target)) {
      nodes.push(<a key={`${match.index}-${target}`} href={target} target="_blank" rel="noreferrer noopener">{label} (enlace externo)</a>);
    } else {
      nodes.push(`${label} (${target})`);
    }
    offset = match.index + match[0].length;
  }
  nodes.push(value.slice(offset));
  return nodes.map((node, index) => <Fragment key={index}>{node}</Fragment>);
}

function FieldErrors({ id, messages }: { id: string; messages: string[] }) {
  if (messages.length === 0) return null;
  return <ul id={id} className="field-error-list">{messages.map((message, index) => <li key={`${message}-${index}`}>{message}</li>)}</ul>;
}

function cleanLabel(result: SearchResult): string {
  return result.snippet.replace(/[\[\]]/gu, "");
}

function kindLabel(kind: ObjectKind): string {
  return ({ world: "Mundo", entity: "Entidad", relation: "Relación", event: "Evento", claim: "Afirmación", rule: "Regla", goal: "Meta", document: "Documento" })[kind] ?? humanize(kind);
}
