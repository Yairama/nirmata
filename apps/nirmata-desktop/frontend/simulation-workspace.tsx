import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { Dispatch, FormEvent, SetStateAction } from "react";
import { buildCreateEditor } from "./editor-create.js";
import { clearError, setStatus, showError } from "./helpers.js";
import { useObjectPicker } from "./object-picker.js";
import { pendingReviewsQueryKey } from "./pending-reviews.js";
import { getAppState, setEphemeralWork, useAppState } from "./state.js";
import type {
  ManualReviewSnapshot,
  SearchObjectKind,
  SearchResult,
  SimulationRule,
  SimulationRun,
  SimulationScenario,
  SimulationScenarioInput,
  SimulationStock,
  SimulationTransition,
  SimulationTransitionSelection,
} from "./types.js";

type NamedObject = { id: string; label: string };
type ResourceRow = NamedObject & { unit: string };
type StockRow = { factionId: string; resourceId: string; quantity: string; capacity: string };
type RuleRow = {
  kind: "production" | "consumption" | "transfer";
  factionId: string;
  destinationId: string;
  resourceId: string;
  amount: string;
};
type ScenarioForm = {
  name: string;
  factions: NamedObject[];
  resources: ResourceRow[];
  stocks: StockRow[];
  rules: RuleRow[];
  assumptions: string;
  maxSteps: string;
};
type PromotionDraft = {
  selected: boolean;
  kind: "create_event" | "create_claim";
  summary: string;
  subjectEntityId: string;
  content: string;
  tick: string;
};

const emptyForm: ScenarioForm = {
  name: "",
  factions: [],
  resources: [],
  stocks: [],
  rules: [],
  assumptions: "",
  maxSteps: "1",
};

function cleanLabel(result: SearchResult): string {
  return result.snippet.replace(/[\[\]]/gu, "").trim() || "Objeto sin nombre";
}

function integer(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) throw new Error(`${label} debe ser un número entero.`);
  return parsed;
}

function ruleFaction(rule: SimulationRule): string {
  return rule.kind === "transfer" ? rule.from_faction_id : rule.faction_id;
}

function formFromScenario(scenario: SimulationScenario, knownLabels: Map<string, string>): ScenarioForm {
  const label = (id: string, fallback: string) => knownLabels.get(id) ?? fallback;
  const factions = scenario.factions.map((id, index) => ({ id, label: label(id, `Facción ${index + 1}`) }));
  const resources = scenario.resources.map((resource, index) => ({
    id: resource.resourceId,
    label: label(resource.resourceId, `Recurso ${index + 1}`),
    unit: resource.unit,
  }));
  return {
    name: scenario.name,
    factions,
    resources,
    stocks: scenario.stocks.map((stock) => ({
      factionId: stock.factionId,
      resourceId: stock.resourceId,
      quantity: String(stock.quantity),
      capacity: String(stock.capacity),
    })),
    rules: scenario.rules.map((rule) => ({
      kind: rule.kind,
      factionId: ruleFaction(rule),
      destinationId: rule.kind === "transfer" ? rule.to_faction_id : "",
      resourceId: rule.resource_id,
      amount: String(rule.amount),
    })),
    assumptions: scenario.assumptions.join("\n"),
    maxSteps: String(scenario.maxSteps),
  };
}

function scenarioInput(form: ScenarioForm, selected: SimulationScenario | null): SimulationScenarioInput {
  const session = getAppState().session;
  if (!session) throw new Error("Abre un mundo antes de crear un escenario.");
  const name = form.name.trim();
  if (!name) throw new Error("Escribe un nombre para distinguir el escenario.");
  if (form.factions.length === 0) throw new Error("Selecciona al menos una facción.");
  if (form.resources.length === 0) throw new Error("Selecciona al menos un recurso.");
  if (form.stocks.length === 0) throw new Error("Añade al menos una existencia inicial.");
  if (form.rules.length === 0) throw new Error("Añade al menos una regla.");
  const maxSteps = integer(form.maxSteps, "El máximo de pasos");
  if (maxSteps < 1 || maxSteps > 1000) throw new Error("El máximo de pasos debe estar entre 1 y 1000.");
  return {
    name,
    worldId: selected?.worldId ?? session.world_id,
    variantId: selected?.variantId ?? session.read_scope.variantId,
    baseRevision: selected?.baseRevision ?? session.world.current_revision,
    factions: form.factions.map((faction) => faction.id),
    resources: form.resources.map((resource) => {
      if (!resource.unit.trim()) throw new Error(`Indica la unidad de ${resource.label}.`);
      return { resourceId: resource.id, unit: resource.unit.trim() };
    }),
    stocks: form.stocks.map((stock, index) => ({
      factionId: stock.factionId,
      resourceId: stock.resourceId,
      quantity: integer(stock.quantity, `Cantidad de la existencia ${index + 1}`),
      capacity: integer(stock.capacity, `Capacidad de la existencia ${index + 1}`),
    })),
    rules: form.rules.map((rule, index): SimulationRule => {
      const amount = integer(rule.amount, `Cantidad de la regla ${index + 1}`);
      if (rule.kind === "transfer") {
        return {
          kind: "transfer",
          from_faction_id: rule.factionId,
          to_faction_id: rule.destinationId,
          resource_id: rule.resourceId,
          amount,
        };
      }
      return { kind: rule.kind, faction_id: rule.factionId, resource_id: rule.resourceId, amount };
    }),
    maxSteps,
    assumptions: form.assumptions.split(/\r?\n/gu).map((value) => value.trim()).filter(Boolean),
  };
}

function promotionBlockReason(scenario: SimulationScenario): string | null {
  const session = getAppState().session;
  if (!session) return "No hay un mundo abierto.";
  if (session.read_only) return "Vuelve a la versión actual para preparar cambios.";
  if (scenario.worldId !== session.world_id) return "El escenario pertenece a otro mundo.";
  if (scenario.variantId !== session.active_variant.id || scenario.variantId !== session.read_scope.variantId) {
    return "El escenario pertenece a otra variante.";
  }
  if (scenario.baseRevision !== session.world.current_revision) return "El escenario usa una versión anterior.";
  return null;
}

function SelectObjectButton({ title, onApply }: { title: string; onApply: (result: SearchResult) => void }) {
  const requestObjectPicker = useObjectPicker();
  return (
    <button
      type="button"
      className="secondary"
      onClick={(event) => requestObjectPicker({
        title,
        kinds: ["entity"],
        multiple: false,
        returnFocus: event.currentTarget,
        apply: ([result]) => { if (result) onApply(result); },
      })}
    >
      Elegir por nombre
    </button>
  );
}

function SimulationForm({ form, setForm, selected, onSaved, onDeleted }: {
  form: ScenarioForm;
  setForm: Dispatch<SetStateAction<ScenarioForm>>;
  selected: SimulationScenario | null;
  onSaved: (scenario: SimulationScenario) => void;
  onDeleted: () => void;
}) {
  const requestObjectPicker = useObjectPicker();
  const [busy, setBusy] = useState(false);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const patch = (values: Partial<ScenarioForm>) => setForm((current) => ({ ...current, ...values }));

  function updateResource(index: number, unit: string) {
    setForm((current) => ({ ...current, resources: current.resources.map((item, itemIndex) => itemIndex === index ? { ...item, unit } : item) }));
  }

  function updateStock(index: number, values: Partial<StockRow>) {
    setForm((current) => ({ ...current, stocks: current.stocks.map((item, itemIndex) => itemIndex === index ? { ...item, ...values } : item) }));
  }

  function updateRule(index: number, values: Partial<RuleRow>) {
    setForm((current) => ({ ...current, rules: current.rules.map((item, itemIndex) => itemIndex === index ? { ...item, ...values } : item) }));
  }

  function addFactions(results: SearchResult[]) {
    setForm((current) => ({
      ...current,
      factions: [...current.factions, ...results
        .filter((result) => !current.factions.some((item) => item.id === result.object_id))
        .map((result) => ({ id: result.object_id, label: cleanLabel(result) }))],
    }));
  }

  function removeFaction(id: string) {
    setForm((current) => ({
      ...current,
      factions: current.factions.filter((item) => item.id !== id),
      stocks: current.stocks.filter((stock) => stock.factionId !== id),
      rules: current.rules.filter((rule) => rule.factionId !== id && rule.destinationId !== id),
    }));
  }

  function addResource(result: SearchResult) {
    setForm((current) => current.resources.some((item) => item.id === result.object_id) ? current : ({
      ...current,
      resources: [...current.resources, { id: result.object_id, label: cleanLabel(result), unit: "unidad" }],
    }));
  }

  function removeResource(id: string) {
    setForm((current) => ({
      ...current,
      resources: current.resources.filter((item) => item.id !== id),
      stocks: current.stocks.filter((stock) => stock.resourceId !== id),
      rules: current.rules.filter((rule) => rule.resourceId !== id),
    }));
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    try {
      clearError();
      setBusy(true);
      const input = scenarioInput(form, selected);
      const saved = selected
        ? await invoke<SimulationScenario>("update_simulation_scenario", { input: { scenarioId: selected.id, scenario: input } })
        : await invoke<SimulationScenario>("create_simulation_scenario", { input: { scenario: input } });
      onSaved(saved);
      setStatus("Escenario guardado fuera del canon.");
    } catch (value) {
      showError(value);
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!selected) return;
    if (!deleteArmed) {
      setDeleteArmed(true);
      return;
    }
    try {
      clearError();
      setBusy(true);
      await invoke("delete_simulation_scenario", { input: { scenarioId: selected.id } });
      onDeleted();
      setDeleteArmed(false);
      setStatus("Escenario eliminado; el canon no cambió.");
    } catch (value) {
      showError(value);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="simulation-form simulation-builder" onSubmit={(event) => void save(event)}>
      <label>Nombre del escenario
        <input name="scenario-name" autoComplete="off" maxLength={120} required value={form.name} onChange={(event) => patch({ name: event.currentTarget.value })} />
      </label>

      <fieldset>
        <legend>Facciones participantes</legend>
        <p className="muted">Elige entidades del mundo; sus nombres se usan en todas las reglas.</p>
        <div className="simulation-chip-list">
          {form.factions.map((faction) => <span className="simulation-chip" key={faction.id}>{faction.label}<button type="button" className="ghost" aria-label={`Quitar ${faction.label}`} onClick={() => removeFaction(faction.id)}>Quitar</button></span>)}
        </div>
        <button type="button" className="secondary" onClick={(event) => requestObjectPicker({ title: "Facciones participantes", kinds: ["entity"], multiple: true, returnFocus: event.currentTarget, apply: addFactions })}>Añadir facciones</button>
      </fieldset>

      <fieldset>
        <legend>Recursos</legend>
        <div className="simulation-row-list">
          {form.resources.map((resource, index) => (
            <div className="simulation-builder-row" key={resource.id}>
              <strong>{resource.label}</strong>
              <label>Unidad<input name={`resource-${index}-unit`} value={resource.unit} onChange={(event) => updateResource(index, event.currentTarget.value)} /></label>
              <button type="button" className="ghost" onClick={() => removeResource(resource.id)}>Quitar</button>
            </div>
          ))}
        </div>
        <SelectObjectButton title="Recurso de la simulación" onApply={addResource} />
      </fieldset>

      <fieldset>
        <legend>Existencias iniciales</legend>
        <div className="simulation-row-list">
          {form.stocks.map((stock, index) => (
            <div className="simulation-builder-row simulation-stock-row" key={`${index}-${stock.factionId}-${stock.resourceId}`}>
              <label>Facción<select name={`stock-${index}-faction`} value={stock.factionId} onChange={(event) => updateStock(index, { factionId: event.currentTarget.value })}>{form.factions.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
              <label>Recurso<select name={`stock-${index}-resource`} value={stock.resourceId} onChange={(event) => updateStock(index, { resourceId: event.currentTarget.value })}>{form.resources.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
              <label>Cantidad<input name={`stock-${index}-quantity`} type="number" step="1" value={stock.quantity} onChange={(event) => updateStock(index, { quantity: event.currentTarget.value })} /></label>
              <label>Capacidad<input name={`stock-${index}-capacity`} type="number" min="0" step="1" value={stock.capacity} onChange={(event) => updateStock(index, { capacity: event.currentTarget.value })} /></label>
              <button type="button" className="ghost" onClick={() => patch({ stocks: form.stocks.filter((_, itemIndex) => itemIndex !== index) })}>Quitar</button>
            </div>
          ))}
        </div>
        <button type="button" className="secondary" disabled={!form.factions.length || !form.resources.length} onClick={() => patch({ stocks: [...form.stocks, { factionId: form.factions[0]!.id, resourceId: form.resources[0]!.id, quantity: "0", capacity: "0" }] })}>Añadir existencia</button>
      </fieldset>

      <fieldset>
        <legend>Reglas por paso</legend>
        <div className="simulation-row-list">
          {form.rules.map((rule, index) => (
            <div className="simulation-builder-row simulation-rule-row" key={index}>
              <label>Acción<select name={`rule-${index}-action`} value={rule.kind} onChange={(event) => updateRule(index, { kind: event.currentTarget.value as RuleRow["kind"] })}><option value="production">Produce</option><option value="consumption">Consume</option><option value="transfer">Transfiere</option></select></label>
              <label>{rule.kind === "transfer" ? "Origen" : "Facción"}<select name={`rule-${index}-faction`} value={rule.factionId} onChange={(event) => updateRule(index, { factionId: event.currentTarget.value })}>{form.factions.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
              {rule.kind === "transfer" && <label>Destino<select name={`rule-${index}-destination`} value={rule.destinationId} onChange={(event) => updateRule(index, { destinationId: event.currentTarget.value })}>{form.factions.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>}
              <label>Recurso<select name={`rule-${index}-resource`} value={rule.resourceId} onChange={(event) => updateRule(index, { resourceId: event.currentTarget.value })}>{form.resources.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label>
              <label>Cantidad<input name={`rule-${index}-amount`} type="number" min="0" step="1" value={rule.amount} onChange={(event) => updateRule(index, { amount: event.currentTarget.value })} /></label>
              <button type="button" className="ghost" onClick={() => patch({ rules: form.rules.filter((_, itemIndex) => itemIndex !== index) })}>Quitar</button>
            </div>
          ))}
        </div>
        <button type="button" className="secondary" disabled={!form.factions.length || !form.resources.length} onClick={() => patch({ rules: [...form.rules, { kind: "production", factionId: form.factions[0]!.id, destinationId: form.factions[1]?.id ?? form.factions[0]!.id, resourceId: form.resources[0]!.id, amount: "1" }] })}>Añadir regla</button>
      </fieldset>

      <div className="simulation-form-tail">
        <label>Máximo de pasos<input name="scenario-max-steps" type="number" min="1" max="1000" step="1" required value={form.maxSteps} onChange={(event) => patch({ maxSteps: event.currentTarget.value })} /></label>
        <label>Supuestos, uno por línea<textarea id="simulation-assumptions" name="scenario-assumptions" rows={3} value={form.assumptions} onChange={(event) => patch({ assumptions: event.currentTarget.value })} /></label>
      </div>
      <div className="pending-actions simulation-actions">
        <button type="submit" disabled={busy}>{selected ? "Guardar cambios" : "Guardar escenario"}</button>
        {selected && <button type="button" className={deleteArmed ? "danger" : "ghost"} disabled={busy} onClick={() => void remove()}>{deleteArmed ? "Confirmar eliminación" : "Eliminar"}</button>}
        {deleteArmed && <button type="button" className="ghost" onClick={() => setDeleteArmed(false)}>Cancelar</button>}
      </div>
    </form>
  );
}

function RunColumn({ run, scenario, labels, allowPromotion, onReviewed }: {
  run: SimulationRun;
  scenario: SimulationScenario;
  labels: Map<string, string>;
  allowPromotion: boolean;
  onReviewed: () => void;
}) {
  const queryClient = useQueryClient();
  const [drafts, setDrafts] = useState<Record<string, PromotionDraft>>({});
  const [busy, setBusy] = useState(false);
  const label = (id: string, fallback: string) => labels.get(id) ?? fallback;
  const ruleText = (rule: SimulationRule) => {
    const resource = label(rule.resource_id, "el recurso");
    if (rule.kind === "transfer") return `${label(rule.from_faction_id, "La facción de origen")} transfiere ${rule.amount} ${resource} a ${label(rule.to_faction_id, "la facción de destino")}`;
    return `${label(rule.faction_id, "La facción")} ${rule.kind === "production" ? "produce" : "consume"} ${rule.amount} ${resource}`;
  };
  const stockText = (stocks: SimulationStock[]) => stocks.length === 0 ? "Sin cambios" : stocks.map((stock) => `${label(stock.factionId, "Facción")} · ${label(stock.resourceId, "recurso")}: ${stock.quantity} de ${stock.capacity}`).join("; ");

  function draftFor(transition: SimulationTransition): PromotionDraft {
    const key = `${transition.step}:${transition.ruleIndex}`;
    return drafts[key] ?? {
      selected: false,
      kind: "create_event",
      summary: `Paso ${transition.step}: ${ruleText(transition.rule)}; se aplicaron ${transition.applied} de ${transition.requested}.`,
      subjectEntityId: ruleFaction(transition.rule),
      content: `La simulación aplicó ${transition.applied} de ${transition.requested} en el paso ${transition.step}.`,
      tick: "",
    };
  }

  function updateDraft(transition: SimulationTransition, patch: Partial<PromotionDraft>) {
    const key = `${transition.step}:${transition.ruleIndex}`;
    setDrafts((current) => ({ ...current, [key]: { ...draftFor(transition), ...patch } }));
  }

  async function prepareReview() {
    try {
      clearError();
      const blocked = promotionBlockReason(scenario);
      if (blocked) throw new Error(blocked);
      const selections = run.transitions.flatMap((transition): SimulationTransitionSelection[] => {
        const draft = draftFor(transition);
        if (!draft.selected) return [];
        const tick = draft.tick.trim() ? integer(draft.tick, "La unidad temporal") : null;
        if (draft.kind === "create_claim") {
          if (!draft.content.trim()) throw new Error("Escribe el contenido de cada afirmación seleccionada.");
          return [{ kind: "create_claim", step: transition.step, ruleIndex: transition.ruleIndex, subjectEntityId: draft.subjectEntityId, content: draft.content.trim(), tick }];
        }
        if (!draft.summary.trim()) throw new Error("Escribe el resumen de cada acontecimiento seleccionado.");
        return [{ kind: "create_event", step: transition.step, ruleIndex: transition.ruleIndex, summary: draft.summary.trim(), tick }];
      });
      if (selections.length === 0) throw new Error("Selecciona al menos una transición para preparar cambios.");
      setBusy(true);
      await invoke<ManualReviewSnapshot>("prepare_simulation_review", { input: { scenarioId: scenario.id, promotion: { selections } } });
      await queryClient.invalidateQueries({ queryKey: pendingReviewsQueryKey(getAppState().session!) });
      setStatus("Resultados añadidos a Cambios; el canon todavía no cambió.");
      onReviewed();
    } catch (value) {
      showError(value);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="simulation-run-column">
      <header><p className="panel-eyebrow">{allowPromotion ? "Escenario principal" : "Comparación"}</p><h4>{scenario.name}</h4><p>{run.stepsCompleted} pasos · {run.assumptions.length ? run.assumptions.join(" · ") : "Sin supuestos declarados"}</p></header>
      {run.transitions.map((transition) => {
        const draft = draftFor(transition);
        return (
          <article className="simulation-transition" key={`${transition.step}:${transition.ruleIndex}`}>
            <p className="panel-eyebrow">Paso {transition.step}</p>
            <h5>{ruleText(transition.rule)}</h5>
            <dl className="simulation-transition-values"><div><dt>Antes</dt><dd>{stockText(transition.before)}</dd></div><div><dt>Después</dt><dd>{stockText(transition.after)}</dd></div><div><dt>Resultado</dt><dd>{transition.applied} aplicado · {transition.shortage ? `${transition.shortage} sin cubrir` : "sin faltante"}</dd></div></dl>
            {allowPromotion && (
              <div className="simulation-promotion-fields">
                <label className="check-row"><input type="checkbox" checked={draft.selected} onChange={(event) => updateDraft(transition, { selected: event.currentTarget.checked })} /> Preparar este resultado para revisión</label>
                {draft.selected && <>
                  <label>Convertir en<select value={draft.kind} onChange={(event) => updateDraft(transition, { kind: event.currentTarget.value as PromotionDraft["kind"] })}><option value="create_event">Acontecimiento</option><option value="create_claim">Afirmación disputada</option></select></label>
                  {draft.kind === "create_event" ? <label>Resumen<input value={draft.summary} onChange={(event) => updateDraft(transition, { summary: event.currentTarget.value })} /></label> : <><label>Sujeto<select value={draft.subjectEntityId} onChange={(event) => updateDraft(transition, { subjectEntityId: event.currentTarget.value })}>{scenario.factions.map((id, index) => <option key={id} value={id}>{label(id, `Facción ${index + 1}`)}</option>)}</select></label><label>Contenido<input value={draft.content} onChange={(event) => updateDraft(transition, { content: event.currentTarget.value })} /></label></>}
                  <details><summary>Detalles temporales opcionales</summary><label>Unidad temporal interna<input type="number" step="1" value={draft.tick} onChange={(event) => updateDraft(transition, { tick: event.currentTarget.value })} /></label></details>
                </>}
              </div>
            )}
          </article>
        );
      })}
      <p><strong>Existencias finales:</strong> {stockText(run.finalStocks)}</p>
      {allowPromotion && <button type="button" disabled={busy || promotionBlockReason(scenario) !== null} title={promotionBlockReason(scenario) ?? "La selección se enviará a Cambios."} onClick={() => void prepareReview()}>Preparar selección para revisión</button>}
    </section>
  );
}

export function SimulationWorkspace({ active, onOpenReviews }: { active: boolean; onOpenReviews: () => void }) {
  const state = useAppState();
  const [scenarios, setScenarios] = useState<SimulationScenario[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [compareId, setCompareId] = useState("");
  const [form, setForm] = useState<ScenarioForm>(emptyForm);
  const [primaryRun, setPrimaryRun] = useState<SimulationRun | null>(null);
  const [comparisonRun, setComparisonRun] = useState<SimulationRun | null>(null);
  const [labels, setLabels] = useState<Map<string, string>>(new Map());
  const [running, setRunning] = useState(false);
  const selected = scenarios.find((scenario) => scenario.id === selectedId) ?? null;

  async function refresh(preferredId = selectedId) {
    try {
      const listed = await invoke<SimulationScenario[]>("list_simulation_scenarios");
      setScenarios(listed);
      if (preferredId && !listed.some((scenario) => scenario.id === preferredId)) setSelectedId(null);
    } catch (value) {
      showError(value);
    }
  }

  useEffect(() => {
    void refresh();
  }, [state.session?.world_id, state.session?.read_scope.variantId, state.session?.read_scope.revisionId]);

  useEffect(() => {
    const known = new Map(labels);
    for (const faction of form.factions) known.set(faction.id, faction.label);
    for (const resource of form.resources) known.set(resource.id, resource.label);
    setLabels(known);
    setEphemeralWork("simulation", "escenarios o resultados de simulación", scenarios.length > 0 || primaryRun !== null || comparisonRun !== null || JSON.stringify(form) !== JSON.stringify(emptyForm));
  }, [form, scenarios.length, primaryRun, comparisonRun]);

  useEffect(() => {
    setScenarios([]);
    setSelectedId(null);
    setCompareId("");
    setForm(emptyForm);
    setPrimaryRun(null);
    setComparisonRun(null);
    setLabels(new Map());
  }, [state.discardRevision]);

  function selectScenario(id: string) {
    const scenario = scenarios.find((item) => item.id === id) ?? null;
    setSelectedId(scenario?.id ?? null);
    setForm(scenario ? formFromScenario(scenario, labels) : emptyForm);
    setPrimaryRun(null);
    setComparisonRun(null);
    setCompareId("");
  }

  async function run() {
    if (!selected) return;
    try {
      clearError();
      setRunning(true);
      const [primary, comparison] = await Promise.all([
        invoke<SimulationRun>("run_simulation_scenario", { input: { scenarioId: selected.id } }),
        compareId ? invoke<SimulationRun>("run_simulation_scenario", { input: { scenarioId: compareId } }) : Promise.resolve(null),
      ]);
      setPrimaryRun(primary);
      setComparisonRun(comparison);
      setStatus(comparison ? "Dos escenarios comparados fuera del canon." : "Simulación completada fuera del canon.");
    } catch (value) {
      showError(value);
    } finally {
      setRunning(false);
    }
  }

  return (
    <section id="simulation-panel" className="simulation-panel" aria-labelledby="simulation-title" hidden={!active}>
      <div className="assistant-heading">
        <div><p className="panel-eyebrow simulation-eyebrow">Laboratorio temporal · fuera del canon</p><h2 id="simulation-title" tabIndex={-1}>Simulación</h2></div>
        <p className="panel-summary">Los escenarios viven solo durante esta sesión. Ejecutarlos nunca escribe el mundo.</p>
      </div>
      <div className="simulation-selectors">
        <label>Escenario<select value={selectedId ?? ""} onChange={(event) => selectScenario(event.currentTarget.value)}><option value="">Nuevo escenario</option>{scenarios.map((scenario) => <option key={scenario.id} value={scenario.id}>{scenario.name}</option>)}</select></label>
        <label>Comparar al ejecutar<select value={compareId} disabled={!selected || scenarios.length < 2} onChange={(event) => setCompareId(event.currentTarget.value)}><option value="">Sin comparación</option>{scenarios.filter((scenario) => scenario.id !== selectedId).map((scenario) => <option key={scenario.id} value={scenario.id}>{scenario.name}</option>)}</select></label>
        <button type="button" className="secondary" onClick={() => selectScenario("")}>Nuevo escenario</button>
        <button type="button" disabled={!selected || running} onClick={() => void run()}>{running ? "Ejecutando…" : "Ejecutar una vez"}</button>
      </div>
      <SimulationForm
        form={form}
        setForm={setForm}
        selected={selected}
        onSaved={(saved) => { setSelectedId(saved.id); setPrimaryRun(null); setComparisonRun(null); setForm(formFromScenario(saved, labels)); void refresh(saved.id); }}
        onDeleted={() => { selectScenario(""); void refresh(null); }}
      />
      <div className="simulation-results" aria-live="polite">
        {!primaryRun && <p className="empty-state">Guarda y ejecuta un escenario para comparar transiciones y existencias finales.</p>}
        {primaryRun && selected && <div className="simulation-run-grid"><RunColumn run={primaryRun} scenario={selected} labels={labels} allowPromotion onReviewed={onOpenReviews} />{comparisonRun && <RunColumn run={comparisonRun} scenario={scenarios.find((scenario) => scenario.id === comparisonRun.scenarioId)!} labels={labels} allowPromotion={false} onReviewed={onOpenReviews} />}</div>}
      </div>
    </section>
  );
}
