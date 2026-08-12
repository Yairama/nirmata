use crate::{AppError, NirmataApp};
use nirmata_core::{EntityId, RevisionId, VariantId, WorldId, entity::EntityKind};
use nirmata_store::{CanonSnapshot, ReadScope};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};
use uuid::Uuid;

pub const MAX_SIMULATION_STEPS: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SimulationScenarioId(Uuid);

impl SimulationScenarioId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SimulationScenarioId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SimulationScenarioId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SimulationScenarioId {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationResource {
    pub resource_id: EntityId,
    pub unit: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationStock {
    pub faction_id: EntityId,
    pub resource_id: EntityId,
    pub quantity: i64,
    pub capacity: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SimulationRule {
    Production {
        faction_id: EntityId,
        resource_id: EntityId,
        amount: i64,
    },
    Consumption {
        faction_id: EntityId,
        resource_id: EntityId,
        amount: i64,
    },
    Transfer {
        from_faction_id: EntityId,
        to_faction_id: EntityId,
        resource_id: EntityId,
        amount: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationScenarioInput {
    pub world_id: WorldId,
    pub variant_id: VariantId,
    pub base_revision: RevisionId,
    pub factions: Vec<EntityId>,
    pub resources: Vec<SimulationResource>,
    pub stocks: Vec<SimulationStock>,
    pub rules: Vec<SimulationRule>,
    pub max_steps: u32,
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationScenario {
    pub id: SimulationScenarioId,
    pub world_id: WorldId,
    pub variant_id: VariantId,
    pub base_revision: RevisionId,
    pub factions: Vec<EntityId>,
    pub resources: Vec<SimulationResource>,
    pub stocks: Vec<SimulationStock>,
    pub rules: Vec<SimulationRule>,
    pub max_steps: u32,
    pub assumptions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationTransition {
    pub step: u32,
    pub rule_index: usize,
    pub rule: SimulationRule,
    pub before: Vec<SimulationStock>,
    pub after: Vec<SimulationStock>,
    pub requested: i64,
    pub applied: i64,
    pub shortage: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationRun {
    pub scenario_id: SimulationScenarioId,
    pub world_id: WorldId,
    pub variant_id: VariantId,
    pub base_revision: RevisionId,
    pub steps_completed: u32,
    pub assumptions: Vec<String>,
    pub transitions: Vec<SimulationTransition>,
    pub final_stocks: Vec<SimulationStock>,
}

type StockKey = (EntityId, EntityId);

impl NirmataApp {
    pub fn create_simulation_scenario(
        &mut self,
        input: SimulationScenarioInput,
    ) -> Result<SimulationScenario, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        validate_scenario(active, &input)?;
        let scenario = SimulationScenario::from_input(SimulationScenarioId::new(), input);
        self.simulation_scenarios
            .insert(scenario.id, scenario.clone());
        Ok(scenario)
    }

    pub fn update_simulation_scenario(
        &mut self,
        id: SimulationScenarioId,
        input: SimulationScenarioInput,
    ) -> Result<SimulationScenario, AppError> {
        if !self.simulation_scenarios.contains_key(&id) {
            return Err(AppError::SimulationScenarioNotFound(id));
        }
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        validate_scenario(active, &input)?;
        let scenario = SimulationScenario::from_input(id, input);
        self.simulation_scenarios.insert(id, scenario.clone());
        Ok(scenario)
    }

    pub fn delete_simulation_scenario(
        &mut self,
        id: SimulationScenarioId,
    ) -> Result<SimulationScenario, AppError> {
        self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        self.simulation_scenarios
            .remove(&id)
            .ok_or(AppError::SimulationScenarioNotFound(id))
    }

    pub fn list_simulation_scenarios(&self) -> Result<Vec<SimulationScenario>, AppError> {
        self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        Ok(self.simulation_scenarios.values().cloned().collect())
    }

    pub fn run_simulation_scenario(
        &self,
        id: SimulationScenarioId,
    ) -> Result<SimulationRun, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let scenario = self
            .simulation_scenarios
            .get(&id)
            .ok_or(AppError::SimulationScenarioNotFound(id))?;
        validate_scenario(active, &scenario.as_input())?;
        run_scenario(scenario)
    }
}

impl SimulationScenario {
    fn from_input(id: SimulationScenarioId, input: SimulationScenarioInput) -> Self {
        Self {
            id,
            world_id: input.world_id,
            variant_id: input.variant_id,
            base_revision: input.base_revision,
            factions: input.factions,
            resources: input.resources,
            stocks: input.stocks,
            rules: input.rules,
            max_steps: input.max_steps,
            assumptions: input.assumptions,
        }
    }

    fn as_input(&self) -> SimulationScenarioInput {
        SimulationScenarioInput {
            world_id: self.world_id,
            variant_id: self.variant_id,
            base_revision: self.base_revision,
            factions: self.factions.clone(),
            resources: self.resources.clone(),
            stocks: self.stocks.clone(),
            rules: self.rules.clone(),
            max_steps: self.max_steps,
            assumptions: self.assumptions.clone(),
        }
    }
}

fn validate_scenario(
    active: &crate::app::ActiveWorld,
    scenario: &SimulationScenarioInput,
) -> Result<(), AppError> {
    if scenario.world_id != active.session.world_id {
        return invalid("world_id does not match the open world");
    }
    if !(1..=MAX_SIMULATION_STEPS).contains(&scenario.max_steps) {
        return invalid("max_steps must be between 1 and 1000");
    }

    let scope = ReadScope::historical(scenario.variant_id, scenario.base_revision);
    active.store.resolve_scope(scope).map_err(|error| {
        AppError::InvalidSimulationScenario(format!("invalid base variant or revision: {error}"))
    })?;
    let snapshot = active.store.read_canon_snapshot_scoped(scope)?;
    if snapshot.world().id() != scenario.world_id
        || snapshot.world().current_revision() != scenario.base_revision
    {
        return invalid("base variant and revision do not belong to the scenario world");
    }
    validate_references(&snapshot, scenario)?;
    validate_state(scenario)
}

fn validate_references(
    snapshot: &CanonSnapshot,
    scenario: &SimulationScenarioInput,
) -> Result<(), AppError> {
    let entities = snapshot
        .entities()
        .iter()
        .map(|entity| (entity.id(), entity.kind()))
        .collect::<BTreeMap<_, _>>();
    let mut factions = BTreeSet::new();
    for faction_id in &scenario.factions {
        if !factions.insert(*faction_id) {
            return invalid(format!("duplicate faction {faction_id}"));
        }
        match entities.get(faction_id) {
            Some(EntityKind::Faction) => {}
            Some(_) => return invalid(format!("entity {faction_id} is not a faction")),
            None => return invalid(format!("faction {faction_id} does not exist at the base")),
        }
    }

    let mut resources = BTreeSet::new();
    for resource in &scenario.resources {
        if !resources.insert(resource.resource_id) {
            return invalid(format!("duplicate resource {}", resource.resource_id));
        }
        if resource.unit.trim().is_empty() {
            return invalid(format!(
                "resource {} must declare a non-empty unit",
                resource.resource_id
            ));
        }
        match entities.get(&resource.resource_id) {
            Some(EntityKind::Resource) => {}
            Some(_) => {
                return invalid(format!("entity {} is not a resource", resource.resource_id));
            }
            None => {
                return invalid(format!(
                    "resource {} does not exist at the base",
                    resource.resource_id
                ));
            }
        }
    }

    for stock in &scenario.stocks {
        require_faction_resource(&factions, &resources, stock.faction_id, stock.resource_id)?;
    }
    for rule in &scenario.rules {
        match rule {
            SimulationRule::Production {
                faction_id,
                resource_id,
                ..
            }
            | SimulationRule::Consumption {
                faction_id,
                resource_id,
                ..
            } => require_faction_resource(&factions, &resources, *faction_id, *resource_id)?,
            SimulationRule::Transfer {
                from_faction_id,
                to_faction_id,
                resource_id,
                ..
            } => {
                require_faction_resource(&factions, &resources, *from_faction_id, *resource_id)?;
                require_faction_resource(&factions, &resources, *to_faction_id, *resource_id)?;
                if from_faction_id == to_faction_id {
                    return invalid("a transfer must use two different factions");
                }
            }
        }
    }
    Ok(())
}

fn validate_state(scenario: &SimulationScenarioInput) -> Result<(), AppError> {
    let mut stocks = BTreeSet::new();
    for stock in &scenario.stocks {
        if stock.quantity < 0 || stock.capacity < 0 {
            return invalid("stock quantity and capacity must be non-negative");
        }
        if stock.quantity > stock.capacity {
            return invalid(format!(
                "stock for faction {} and resource {} exceeds capacity",
                stock.faction_id, stock.resource_id
            ));
        }
        if !stocks.insert((stock.faction_id, stock.resource_id)) {
            return invalid(format!(
                "duplicate stock for faction {} and resource {}",
                stock.faction_id, stock.resource_id
            ));
        }
    }

    for rule in &scenario.rules {
        let (amount, required_stocks): (i64, &[StockKey]) = match rule {
            SimulationRule::Production {
                faction_id,
                resource_id,
                amount,
            }
            | SimulationRule::Consumption {
                faction_id,
                resource_id,
                amount,
            } => (*amount, &[(*faction_id, *resource_id)]),
            SimulationRule::Transfer {
                from_faction_id,
                to_faction_id,
                resource_id,
                amount,
            } => (
                *amount,
                &[
                    (*from_faction_id, *resource_id),
                    (*to_faction_id, *resource_id),
                ],
            ),
        };
        if amount < 0 {
            return invalid("rule amounts must be non-negative");
        }
        for key in required_stocks {
            if !stocks.contains(key) {
                return invalid(format!(
                    "rule references missing stock for faction {} and resource {}",
                    key.0, key.1
                ));
            }
        }
    }
    Ok(())
}

fn require_faction_resource(
    factions: &BTreeSet<EntityId>,
    resources: &BTreeSet<EntityId>,
    faction_id: EntityId,
    resource_id: EntityId,
) -> Result<(), AppError> {
    if !factions.contains(&faction_id) {
        return invalid(format!("undeclared faction {faction_id}"));
    }
    if !resources.contains(&resource_id) {
        return invalid(format!("undeclared resource {resource_id}"));
    }
    Ok(())
}

fn run_scenario(scenario: &SimulationScenario) -> Result<SimulationRun, AppError> {
    let mut stocks = scenario
        .stocks
        .iter()
        .cloned()
        .map(|stock| ((stock.faction_id, stock.resource_id), stock))
        .collect::<BTreeMap<_, _>>();
    let mut transitions = Vec::with_capacity(
        scenario
            .rules
            .len()
            .saturating_mul(scenario.max_steps as usize),
    );

    for step in 1..=scenario.max_steps {
        for (rule_index, rule) in scenario.rules.iter().enumerate() {
            transitions.push(apply_rule(step, rule_index, rule, &mut stocks)?);
        }
    }

    Ok(SimulationRun {
        scenario_id: scenario.id,
        world_id: scenario.world_id,
        variant_id: scenario.variant_id,
        base_revision: scenario.base_revision,
        steps_completed: scenario.max_steps,
        assumptions: scenario.assumptions.clone(),
        transitions,
        final_stocks: stocks.into_values().collect(),
    })
}

fn apply_rule(
    step: u32,
    rule_index: usize,
    rule: &SimulationRule,
    stocks: &mut BTreeMap<StockKey, SimulationStock>,
) -> Result<SimulationTransition, AppError> {
    let (before, after, requested, applied) = match rule {
        SimulationRule::Production {
            faction_id,
            resource_id,
            amount,
        } => {
            let stock = stock_mut(stocks, (*faction_id, *resource_id))?;
            let before = vec![stock.clone()];
            let applied = (*amount).min(stock.capacity - stock.quantity);
            stock.quantity += applied;
            (before, vec![stock.clone()], *amount, applied)
        }
        SimulationRule::Consumption {
            faction_id,
            resource_id,
            amount,
        } => {
            let stock = stock_mut(stocks, (*faction_id, *resource_id))?;
            let before = vec![stock.clone()];
            let applied = (*amount).min(stock.quantity);
            stock.quantity -= applied;
            (before, vec![stock.clone()], *amount, applied)
        }
        SimulationRule::Transfer {
            from_faction_id,
            to_faction_id,
            resource_id,
            amount,
        } => {
            let from_key = (*from_faction_id, *resource_id);
            let to_key = (*to_faction_id, *resource_id);
            let from_before = stocks
                .get(&from_key)
                .cloned()
                .ok_or_else(|| missing_stock(from_key))?;
            let to_before = stocks
                .get(&to_key)
                .cloned()
                .ok_or_else(|| missing_stock(to_key))?;
            let applied = (*amount)
                .min(from_before.quantity)
                .min(to_before.capacity - to_before.quantity);
            stocks
                .get_mut(&from_key)
                .expect("validated source stock")
                .quantity -= applied;
            stocks
                .get_mut(&to_key)
                .expect("validated destination stock")
                .quantity += applied;
            let after = vec![
                stocks.get(&from_key).expect("source stock").clone(),
                stocks.get(&to_key).expect("destination stock").clone(),
            ];
            (vec![from_before, to_before], after, *amount, applied)
        }
    };
    Ok(SimulationTransition {
        step,
        rule_index,
        rule: rule.clone(),
        before,
        after,
        requested,
        applied,
        shortage: requested - applied,
    })
}

fn stock_mut(
    stocks: &mut BTreeMap<StockKey, SimulationStock>,
    key: StockKey,
) -> Result<&mut SimulationStock, AppError> {
    stocks.get_mut(&key).ok_or_else(|| missing_stock(key))
}

fn missing_stock(key: StockKey) -> AppError {
    AppError::InvalidSimulationScenario(format!(
        "missing stock for faction {} and resource {}",
        key.0, key.1
    ))
}

fn invalid<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::InvalidSimulationScenario(message.into()))
}
