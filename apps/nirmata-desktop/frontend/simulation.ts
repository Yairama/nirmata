import { buildCreateEditor } from "./editor-create.js";
import { clearError, setStatus, showError } from "./helpers.js";
import { renderPending } from "./render-pending.js";
import {
  invoke,
  simulationAssumptions,
  simulationCompareSelect,
  simulationDelete,
  simulationFactions,
  simulationForm,
  simulationMaxSteps,
  simulationNew,
  simulationResources,
  simulationResults,
  simulationRules,
  simulationRun,
  simulationScenarioSelect,
  simulationStatus,
  simulationStocks,
  state,
} from "./state.js";
import type {
  ManualDraftPreview,
  ManualReviewSnapshot,
  SearchObjectKind,
  SimulationRule,
  SimulationRun as SimulationRunResult,
  SimulationScenario,
  SimulationScenarioInput,
  SimulationStock,
  SimulationTransition,
  SimulationTransitionSelection,
} from "./types.js";

const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;
let scenarios: SimulationScenario[] = [];
let selectedScenarioId: string | null = null;
let primaryRun: SimulationRunResult | null = null;
let comparisonRun: SimulationRunResult | null = null;

function lines(value: string): string[] {
  return value.split(/\r?\n/u).map((line) => line.trim()).filter(Boolean);
}

function uuid(value: string, label: string): string {
  if (!uuidPattern.test(value)) {
    throw new Error(`${label} debe ser un UUID.`);
  }
  return value;
}

function integer(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${label} debe ser un entero seguro.`);
  }
  return parsed;
}

function fields(line: string, count: number, format: string): string[] {
  const values = line.split("|").map((value) => value.trim());
  if (values.length !== count || values.some((value) => value.length === 0)) {
    throw new Error(`Formato inválido: ${line}. Usa ${format}.`);
  }
  return values;
}

function parseRules(): SimulationRule[] {
  return lines(simulationRules.value).map((line, index) => {
    const values = line.split("|").map((value) => value.trim());
    const label = `rules línea ${index + 1}`;
    if (values[0] === "production" || values[0] === "consumption") {
      if (values.length !== 4 || values.some((value) => value.length === 0)) {
        throw new Error(`${label}: usa ${values[0]}|faction|resource|amount.`);
      }
      return {
        kind: values[0],
        faction_id: uuid(values[1]!, `${label} faction`),
        resource_id: uuid(values[2]!, `${label} resource`),
        amount: integer(values[3]!, `${label} amount`),
      };
    }
    if (values[0] === "transfer") {
      if (values.length !== 5 || values.some((value) => value.length === 0)) {
        throw new Error(`${label}: usa transfer|from|to|resource|amount.`);
      }
      return {
        kind: "transfer",
        from_faction_id: uuid(values[1]!, `${label} from`),
        to_faction_id: uuid(values[2]!, `${label} to`),
        resource_id: uuid(values[3]!, `${label} resource`),
        amount: integer(values[4]!, `${label} amount`),
      };
    }
    throw new Error(`${label}: tipo desconocido; usa production, consumption o transfer.`);
  });
}

function scenarioFromForm(): SimulationScenarioInput {
  const selected = scenarios.find((scenario) => scenario.id === selectedScenarioId);
  const session = state.session;
  if (!selected && !session) {
    throw new Error("Abre un mundo antes de crear un escenario.");
  }
  const maxSteps = integer(simulationMaxSteps.value, "max steps");
  if (maxSteps < 1 || maxSteps > 1000) {
    throw new Error("Max steps debe estar entre 1 y 1000.");
  }
  return {
    worldId: selected?.worldId ?? session!.world_id,
    variantId: selected?.variantId ?? session!.read_scope.variantId,
    baseRevision: selected?.baseRevision ?? session!.world.current_revision,
    factions: lines(simulationFactions.value).map((value, index) =>
      uuid(value, `factions línea ${index + 1}`),
    ),
    resources: lines(simulationResources.value).map((line, index) => {
      const [resourceId, unit] = fields(line, 2, "id|unit");
      return { resourceId: uuid(resourceId!, `resources línea ${index + 1}`), unit: unit! };
    }),
    stocks: lines(simulationStocks.value).map((line, index) => {
      const [factionId, resourceId, quantity, capacity] = fields(
        line,
        4,
        "faction|resource|quantity|capacity",
      );
      return {
        factionId: uuid(factionId!, `stocks línea ${index + 1} faction`),
        resourceId: uuid(resourceId!, `stocks línea ${index + 1} resource`),
        quantity: integer(quantity!, `stocks línea ${index + 1} quantity`),
        capacity: integer(capacity!, `stocks línea ${index + 1} capacity`),
      };
    }),
    rules: parseRules(),
    maxSteps,
    assumptions: lines(simulationAssumptions.value),
  };
}

function ruleText(rule: SimulationRule): string {
  switch (rule.kind) {
    case "production":
      return `production ${rule.faction_id} / ${rule.resource_id} / ${rule.amount}`;
    case "consumption":
      return `consumption ${rule.faction_id} / ${rule.resource_id} / ${rule.amount}`;
    case "transfer":
      return `transfer ${rule.from_faction_id} → ${rule.to_faction_id} / ${rule.resource_id} / ${rule.amount}`;
  }
}

function writeForm(scenario: SimulationScenario | null): void {
  simulationFactions.value = scenario?.factions.join("\n") ?? "";
  simulationResources.value = scenario?.resources
    .map((resource) => `${resource.resourceId}|${resource.unit}`)
    .join("\n") ?? "";
  simulationStocks.value = scenario?.stocks
    .map((stock) => `${stock.factionId}|${stock.resourceId}|${stock.quantity}|${stock.capacity}`)
    .join("\n") ?? "";
  simulationRules.value = scenario?.rules.map((rule) => {
    switch (rule.kind) {
      case "production":
      case "consumption":
        return `${rule.kind}|${rule.faction_id}|${rule.resource_id}|${rule.amount}`;
      case "transfer":
        return `transfer|${rule.from_faction_id}|${rule.to_faction_id}|${rule.resource_id}|${rule.amount}`;
    }
  }).join("\n") ?? "";
  simulationAssumptions.value = scenario?.assumptions.join("\n") ?? "";
  simulationMaxSteps.value = String(scenario?.maxSteps ?? 1);
}

function scenarioLabel(scenario: SimulationScenario, index: number): string {
  return `Escenario ${index + 1} · ${scenario.id.slice(0, 8)} · ${scenario.maxSteps} pasos`;
}

function renderSelectors(): void {
  const selected = scenarios.find((scenario) => scenario.id === selectedScenarioId) ?? null;
  const newOption = document.createElement("option");
  newOption.value = "";
  newOption.textContent = "Nuevo escenario sin guardar";
  simulationScenarioSelect.replaceChildren(newOption);
  scenarios.forEach((scenario, index) => {
    const option = document.createElement("option");
    option.value = scenario.id;
    option.textContent = scenarioLabel(scenario, index);
    option.selected = scenario.id === selectedScenarioId;
    simulationScenarioSelect.append(option);
  });

  const noComparison = document.createElement("option");
  noComparison.value = "";
  noComparison.textContent = "Sin comparación";
  const previousComparison = simulationCompareSelect.value;
  simulationCompareSelect.replaceChildren(noComparison);
  scenarios.filter((scenario) => scenario.id !== selectedScenarioId).forEach((scenario, index) => {
    const option = document.createElement("option");
    option.value = scenario.id;
    option.textContent = scenarioLabel(scenario, index);
    option.selected = scenario.id === previousComparison;
    simulationCompareSelect.append(option);
  });
  const hasSession = state.session !== null;
  simulationForm.querySelectorAll<HTMLInputElement | HTMLTextAreaElement | HTMLButtonElement>(
    "input, textarea, button",
  ).forEach((control) => { control.disabled = !hasSession; });
  simulationDelete.disabled = !selected;
  simulationRun.disabled = !selected;
  simulationCompareSelect.disabled = !selected || scenarios.length < 2;
  simulationScenarioSelect.disabled = !hasSession;
  simulationNew.disabled = !hasSession;
  simulationStatus.textContent = selected
    ? `Base ${selected.baseRevision.slice(0, 8)} · variante ${selected.variantId.slice(0, 8)}.`
    : scenarios.length > 0
      ? `${scenarios.length} escenario(s) de sesión. Completa el formulario para crear otro.`
      : "Sin escenarios en esta sesión.";
}

async function refreshScenarios(): Promise<void> {
  if (!state.session) {
    scenarios = [];
    selectedScenarioId = null;
    renderSelectors();
    return;
  }
  try {
    scenarios = await invoke<SimulationScenario[]>("list_simulation_scenarios");
    if (selectedScenarioId && !scenarios.some((scenario) => scenario.id === selectedScenarioId)) {
      selectedScenarioId = null;
      writeForm(null);
    }
    renderSelectors();
  } catch (value) {
    showError(value);
  }
}

function stockText(stocks: SimulationStock[]): string {
  if (stocks.length === 0) return "ninguno";
  return stocks.map((stock) =>
    `${stock.factionId}/${stock.resourceId}: ${stock.quantity}/${stock.capacity}`,
  ).join("; ");
}

function defaultSubject(rule: SimulationRule): string {
  return rule.kind === "transfer" ? rule.from_faction_id : rule.faction_id;
}

function appendPromotionFields(card: HTMLElement, transition: SimulationTransition): void {
  const selectionLabel = document.createElement("label");
  const checkbox = document.createElement("input");
  checkbox.type = "checkbox";
  checkbox.className = "simulation-transition-selection";
  const selectionText = document.createElement("span");
  selectionText.textContent = "Seleccionar para promoción revisable";
  selectionLabel.append(checkbox, selectionText);

  const fieldsContainer = document.createElement("div");
  fieldsContainer.className = "simulation-promotion-fields";
  const mappingLabel = document.createElement("label");
  mappingLabel.textContent = "Mapping";
  const mapping = document.createElement("select");
  mapping.className = "simulation-mapping";
  for (const [value, label] of [["event", "Event"], ["claim", "Claim"]]) {
    const option = document.createElement("option");
    option.value = value!;
    option.textContent = label!;
    mapping.append(option);
  }
  mappingLabel.append(mapping);

  const summaryLabel = document.createElement("label");
  summaryLabel.textContent = "Event summary";
  const summary = document.createElement("input");
  summary.className = "simulation-event-summary";
  summary.value = `Paso ${transition.step}: ${ruleText(transition.rule)} aplicó ${transition.applied} de ${transition.requested}`;
  summaryLabel.append(summary);

  const subjectLabel = document.createElement("label");
  subjectLabel.textContent = "Claim subject entity UUID";
  subjectLabel.hidden = true;
  const subject = document.createElement("input");
  subject.className = "simulation-claim-subject";
  subject.value = defaultSubject(transition.rule);
  subjectLabel.append(subject);

  const contentLabel = document.createElement("label");
  contentLabel.textContent = "Claim content";
  contentLabel.hidden = true;
  const content = document.createElement("input");
  content.className = "simulation-claim-content";
  content.value = `La simulación aplicó ${transition.applied} de ${transition.requested} en el paso ${transition.step}.`;
  contentLabel.append(content);

  const tickLabel = document.createElement("label");
  tickLabel.textContent = "Tick (opcional)";
  const tick = document.createElement("input");
  tick.type = "number";
  tick.step = "1";
  tick.className = "simulation-promotion-tick";
  tickLabel.append(tick);

  mapping.addEventListener("change", () => {
    const claim = mapping.value === "claim";
    summaryLabel.hidden = claim;
    subjectLabel.hidden = !claim;
    contentLabel.hidden = !claim;
  });
  fieldsContainer.append(mappingLabel, summaryLabel, subjectLabel, contentLabel, tickLabel);
  card.append(selectionLabel, fieldsContainer);
}

function promotionBlockReason(scenario: SimulationScenario): string | null {
  const session = state.session;
  if (!session) return "No hay un mundo abierto.";
  if (session.read_only) return "Vuelve a la cabeza activa para promover.";
  if (scenario.worldId !== session.world_id) return "El escenario pertenece a otro mundo.";
  if (
    scenario.variantId !== session.active_variant.id
    || scenario.variantId !== session.read_scope.variantId
  ) return "La variante del escenario no coincide con la variante activa.";
  if (scenario.baseRevision !== session.world.current_revision) {
    return "La revisión base del escenario no coincide con la cabeza vigente.";
  }
  return null;
}

function parseOptionalTick(input: HTMLInputElement): number | null {
  return input.value.trim() ? integer(input.value, "tick") : null;
}

function selectedPromotions(container: HTMLElement): SimulationTransitionSelection[] {
  return Array.from(container.querySelectorAll<HTMLElement>(".simulation-transition"))
    .filter((card) => card.querySelector<HTMLInputElement>(".simulation-transition-selection")?.checked)
    .map((card) => {
      const step = Number(card.dataset.step);
      const ruleIndex = Number(card.dataset.ruleIndex);
      const mapping = card.querySelector<HTMLSelectElement>(".simulation-mapping")!;
      const tick = parseOptionalTick(card.querySelector<HTMLInputElement>(".simulation-promotion-tick")!);
      if (mapping.value === "event") {
        const summary = card.querySelector<HTMLInputElement>(".simulation-event-summary")!.value.trim();
        if (!summary) throw new Error("Cada Event seleccionado requiere summary.");
        return { kind: "create_event", step, ruleIndex, summary, tick };
      }
      const subjectEntityId = uuid(
        card.querySelector<HTMLInputElement>(".simulation-claim-subject")!.value.trim(),
        "Claim subject entity",
      );
      const content = card.querySelector<HTMLInputElement>(".simulation-claim-content")!.value.trim();
      if (!content) throw new Error("Cada Claim seleccionado requiere content.");
      return { kind: "create_claim", step, ruleIndex, subjectEntityId, content, tick };
    });
}

function addReviewToPending(review: ManualReviewSnapshot): void {
  const first = review.operations[0];
  const objectType = (first?.after?.objectType ?? first?.before?.objectType ?? "event") as SearchObjectKind;
  const preview: ManualDraftPreview = {
    draftKey: review.reviewKey,
    targetUri: first?.targetUri ?? review.reviewKey,
    objectType,
    mode: first?.before ? "update" : "create",
    title: "Promoción de simulación",
    objective: review.objective,
    sourceUris: review.sources,
    assumptions: review.assumptions,
    logicalPath: "Transiciones de simulación",
    validationReport: review.validationReport,
    readyToConfirm: review.readyToConfirm,
  };
  state.pendingDrafts.set(review.reviewKey, {
    preview,
    review,
    editor: buildCreateEditor(objectType),
  });
  renderPending();
}

async function promoteScenario(scenario: SimulationScenario, container: HTMLElement): Promise<void> {
  try {
    clearError();
    const blocked = promotionBlockReason(scenario);
    if (blocked) throw new Error(blocked);
    const selections = selectedPromotions(container);
    if (selections.length === 0) throw new Error("Selecciona al menos una transición para promover.");
    const review = await invoke<ManualReviewSnapshot>("prepare_simulation_review", {
      input: { scenarioId: scenario.id, promotion: { selections } },
    });
    addReviewToPending(review);
    simulationStatus.textContent = "ManualReviewSnapshot añadido a Cambios pendientes; el canon no cambió.";
    setStatus("Promoción preparada para revisión manual estándar.");
  } catch (value) {
    showError(value);
  }
}

function renderRunColumn(
  run: SimulationRunResult,
  scenario: SimulationScenario,
  title: string,
  allowPromotion: boolean,
): HTMLElement {
  const column = document.createElement("section");
  column.className = "simulation-run-column";
  const heading = document.createElement("h4");
  heading.textContent = `${title} · ${run.stepsCompleted} pasos`;
  const assumptions = document.createElement("p");
  assumptions.textContent = `Supuestos: ${run.assumptions.join(" · ") || "ninguno"}`;
  column.append(heading, assumptions);

  const ordered = [...run.transitions].sort((left, right) =>
    left.step - right.step || left.ruleIndex - right.ruleIndex,
  );
  let currentStep = -1;
  let stepSection: HTMLElement | null = null;
  for (const transition of ordered) {
    if (transition.step !== currentStep) {
      currentStep = transition.step;
      stepSection = document.createElement("section");
      stepSection.className = "simulation-step";
      const stepHeading = document.createElement("h5");
      stepHeading.textContent = `Paso ${transition.step}`;
      stepSection.append(stepHeading);
      column.append(stepSection);
    }
    const card = document.createElement("article");
    card.className = "simulation-transition";
    card.dataset.step = String(transition.step);
    card.dataset.ruleIndex = String(transition.ruleIndex);
    const rule = document.createElement("p");
    rule.textContent = `Regla ${transition.ruleIndex}: ${ruleText(transition.rule)}`;
    const before = document.createElement("p");
    before.textContent = `Before: ${stockText(transition.before)}`;
    const after = document.createElement("p");
    after.textContent = `After: ${stockText(transition.after)}`;
    const amounts = document.createElement("p");
    amounts.textContent = `Requested: ${transition.requested} · Applied: ${transition.applied} · Shortage: ${transition.shortage}`;
    card.append(rule, before, after, amounts);
    if (allowPromotion) appendPromotionFields(card, transition);
    stepSection!.append(card);
  }

  const finalStocks = document.createElement("p");
  finalStocks.textContent = `Final stocks: ${stockText(run.finalStocks)}`;
  column.append(finalStocks);
  if (allowPromotion) {
    const promote = document.createElement("button");
    promote.type = "button";
    promote.textContent = "Preparar selección para revisión";
    const blocked = promotionBlockReason(scenario);
    promote.disabled = blocked !== null;
    promote.title = blocked ?? "Crea un ManualReviewSnapshot sin confirmar.";
    promote.addEventListener("click", () => void promoteScenario(scenario, column));
    column.append(promote);
  }
  return column;
}

function renderRuns(): void {
  simulationResults.replaceChildren();
  if (!primaryRun) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "La ejecución es determinista, no continua y nunca modifica el canon.";
    simulationResults.append(empty);
    return;
  }
  const scenario = scenarios.find((item) => item.id === primaryRun!.scenarioId);
  if (!scenario) return;
  const grid = document.createElement("div");
  grid.className = "simulation-run-grid";
  grid.append(renderRunColumn(primaryRun, scenario, "Escenario principal", true));
  if (comparisonRun) {
    const compared = scenarios.find((item) => item.id === comparisonRun!.scenarioId);
    if (compared) grid.append(renderRunColumn(comparisonRun, compared, "Comparación", false));
  }
  simulationResults.append(grid);
}

simulationScenarioSelect.addEventListener("change", () => {
  selectedScenarioId = simulationScenarioSelect.value || null;
  primaryRun = null;
  comparisonRun = null;
  writeForm(scenarios.find((scenario) => scenario.id === selectedScenarioId) ?? null);
  renderSelectors();
  renderRuns();
});

simulationNew.addEventListener("click", () => {
  selectedScenarioId = null;
  primaryRun = null;
  comparisonRun = null;
  writeForm(null);
  renderSelectors();
  renderRuns();
});

simulationForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    clearError();
    const scenario = scenarioFromForm();
    const saved = selectedScenarioId
      ? await invoke<SimulationScenario>("update_simulation_scenario", {
          input: { scenarioId: selectedScenarioId, scenario },
        })
      : await invoke<SimulationScenario>("create_simulation_scenario", { input: { scenario } });
    selectedScenarioId = saved.id;
    primaryRun = null;
    comparisonRun = null;
    await refreshScenarios();
    writeForm(saved);
    renderRuns();
    simulationStatus.textContent = "Escenario guardado fuera del canon.";
    setStatus("Escenario de simulación guardado en la sesión.");
  } catch (value) {
    showError(value);
  }
});

simulationDelete.addEventListener("click", async () => {
  if (!selectedScenarioId || !window.confirm("¿Eliminar este escenario efímero?")) return;
  try {
    clearError();
    await invoke<SimulationScenario>("delete_simulation_scenario", {
      input: { scenarioId: selectedScenarioId },
    });
    selectedScenarioId = null;
    primaryRun = null;
    comparisonRun = null;
    writeForm(null);
    await refreshScenarios();
    renderRuns();
    setStatus("Escenario eliminado; el canon no cambió.");
  } catch (value) {
    showError(value);
  }
});

simulationRun.addEventListener("click", async () => {
  if (!selectedScenarioId) return;
  try {
    clearError();
    simulationStatus.textContent = "Ejecutando una vez…";
    const comparisonId = simulationCompareSelect.value || null;
    const [run, compared] = await Promise.all([
      invoke<SimulationRunResult>("run_simulation_scenario", {
        input: { scenarioId: selectedScenarioId },
      }),
      comparisonId
        ? invoke<SimulationRunResult>("run_simulation_scenario", {
            input: { scenarioId: comparisonId },
          })
        : Promise.resolve(null),
    ]);
    primaryRun = run;
    comparisonRun = compared;
    renderRuns();
    simulationStatus.textContent = comparisonRun
      ? "Dos corridas deterministas comparadas lado a lado."
      : "Corrida determinista completada; el canon no cambió.";
    setStatus("Simulación completada fuera del canon.");
  } catch (value) {
    showError(value);
  }
});

window.addEventListener("nirmata:scope-changed", () => {
  renderRuns();
  void refreshScenarios();
});

writeForm(null);
renderSelectors();
renderRuns();
