use nirmata_app::{
    AppError, CreateWorldInput, DraftOperationInput, ManualReviewActionRequest, ManualReviewInput,
    NirmataApp, PendingReviewOrigin, SimulationPromotionInput, SimulationResource, SimulationRule,
    SimulationScenarioInput, SimulationStock, SimulationTransitionSelection,
};
use nirmata_core::{
    EntityId, RevisionId, WorldId,
    change_set::RetconKind,
    claim::ClaimAuthentication,
    entity::{Entity, EntityKind},
};
use nirmata_store::WorldStore;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    app: NirmataApp,
    path: PathBuf,
    world_id: WorldId,
    faction_a: EntityId,
    faction_b: EntityId,
    resource: EntityId,
    person: EntityId,
}

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

fn entity(world_id: WorldId, kind: EntityKind, name: &str, slug: &str, now: i64) -> Entity {
    Entity::new(world_id, kind, name, slug, "", "", "{}", vec![], now).expect("entity")
}

fn fixture(label: &str) -> Fixture {
    let path = project_path(label);
    let mut app = NirmataApp::default();
    let created = app
        .create_world(CreateWorldInput {
            path: path.clone(),
            name: "Resource world".to_owned(),
            premise_md: String::new(),
            epoch_label: "Epoch".to_owned(),
        })
        .expect("create world");
    let faction_a = entity(
        created.world_id,
        EntityKind::Faction,
        "North Guild",
        "north-guild",
        2,
    );
    let faction_b = entity(
        created.world_id,
        EntityKind::Faction,
        "South Guild",
        "south-guild",
        3,
    );
    let resource = entity(created.world_id, EntityKind::Resource, "Grain", "grain", 4);
    let person = entity(created.world_id, EntityKind::Person, "Mara", "mara", 5);
    let operations = [&faction_a, &faction_b, &resource, &person]
        .into_iter()
        .cloned()
        .map(|after| DraftOperationInput::CreateEntity {
            retcon: RetconKind::Additive,
            after,
        })
        .collect();
    let review = app
        .start_manual_review(ManualReviewInput {
            objective: "Create simulation references".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations,
        })
        .expect("start reference review");
    app.confirm_manual_review(&review)
        .expect("commit simulation references");
    Fixture {
        app,
        path,
        world_id: created.world_id,
        faction_a: faction_a.id(),
        faction_b: faction_b.id(),
        resource: resource.id(),
        person: person.id(),
    }
}

fn scenario_input(fixture: &mut Fixture) -> SimulationScenarioInput {
    let session = fixture
        .app
        .get_current_world()
        .expect("session")
        .expect("open world");
    SimulationScenarioInput {
        name: "Cosecha del norte".to_owned(),
        world_id: fixture.world_id,
        variant_id: session.active_variant.id,
        base_revision: session.current_revision,
        factions: vec![fixture.faction_a, fixture.faction_b],
        resources: vec![SimulationResource {
            resource_id: fixture.resource,
            unit: "sacks".to_owned(),
        }],
        stocks: vec![
            SimulationStock {
                faction_id: fixture.faction_a,
                resource_id: fixture.resource,
                quantity: 2,
                capacity: 10,
            },
            SimulationStock {
                faction_id: fixture.faction_b,
                resource_id: fixture.resource,
                quantity: 0,
                capacity: 4,
            },
        ],
        rules: vec![
            SimulationRule::Production {
                faction_id: fixture.faction_a,
                resource_id: fixture.resource,
                amount: 7,
            },
            SimulationRule::Consumption {
                faction_id: fixture.faction_a,
                resource_id: fixture.resource,
                amount: 3,
            },
            SimulationRule::Transfer {
                from_faction_id: fixture.faction_a,
                to_faction_id: fixture.faction_b,
                resource_id: fixture.resource,
                amount: 5,
            },
            SimulationRule::Consumption {
                faction_id: fixture.faction_b,
                resource_id: fixture.resource,
                amount: 6,
            },
            SimulationRule::Production {
                faction_id: fixture.faction_b,
                resource_id: fixture.resource,
                amount: 8,
            },
        ],
        max_steps: 1,
        assumptions: vec!["Each rule runs once per step in listed order.".to_owned()],
    }
}

fn assert_invalid(result: Result<nirmata_app::SimulationScenario, AppError>) {
    assert!(matches!(
        result,
        Err(AppError::InvalidSimulationScenario(_))
    ));
}

#[test]
fn validates_strict_scenarios_against_the_base_snapshot() {
    let mut fixture = fixture("simulation-validation");
    let valid = scenario_input(&mut fixture);

    let mut input = valid.clone();
    input.max_steps = 0;
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.max_steps = 1_001;
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.factions.push(fixture.faction_a);
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.factions[0] = fixture.person;
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.resources[0].unit = "  ".to_owned();
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.resources.push(input.resources[0].clone());
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.stocks[0].quantity = -1;
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.stocks[0].quantity = 11;
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.stocks.push(input.stocks[0].clone());
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.rules[0] = SimulationRule::Production {
        faction_id: fixture.faction_a,
        resource_id: fixture.resource,
        amount: -1,
    };
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.stocks.pop();
    assert_invalid(fixture.app.create_simulation_scenario(input));
    let mut input = valid.clone();
    input.base_revision = RevisionId::new();
    assert_invalid(fixture.app.create_simulation_scenario(input));

    let mut json = serde_json::to_value(&valid).expect("serialize scenario input");
    assert_eq!(
        serde_json::from_value::<SimulationScenarioInput>(json.clone())
            .expect("strict scenario round trip"),
        valid
    );
    json.as_object_mut()
        .expect("scenario object")
        .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<SimulationScenarioInput>(json).is_err());

    fixture.app.close_world().expect("close world");
    fs::remove_file(fixture.path).expect("remove project");
}

#[test]
fn executes_production_consumption_transfer_capacity_and_shortage_deterministically() {
    let mut fixture = fixture("simulation-run");
    let input = scenario_input(&mut fixture);
    let scenario = fixture
        .app
        .create_simulation_scenario(input)
        .expect("create scenario");

    let first = fixture
        .app
        .run_simulation_scenario(scenario.id)
        .expect("first run");
    let second = fixture
        .app
        .run_simulation_scenario(scenario.id)
        .expect("second run");
    assert_eq!(
        serde_json::to_vec(&first).expect("serialize first run"),
        serde_json::to_vec(&second).expect("serialize second run"),
        "the same scenario must produce byte-for-byte identical JSON"
    );
    assert_eq!(first.steps_completed, 1);
    assert_eq!(first.transitions.len(), 5);
    assert_eq!(first.transitions[0].before[0].quantity, 2);
    assert_eq!(first.transitions[0].after[0].quantity, 9);
    assert_eq!(first.transitions[0].requested, 7);
    assert_eq!(first.transitions[0].applied, 7);
    assert_eq!(first.transitions[0].shortage, 0);
    assert_eq!(first.transitions[1].after[0].quantity, 6);
    assert_eq!(first.transitions[2].before[0].quantity, 6);
    assert_eq!(first.transitions[2].before[1].quantity, 0);
    assert_eq!(first.transitions[2].after[0].quantity, 2);
    assert_eq!(first.transitions[2].after[1].quantity, 4);
    assert_eq!(first.transitions[2].requested, 5);
    assert_eq!(first.transitions[2].applied, 4);
    assert_eq!(first.transitions[2].shortage, 1);
    assert_eq!(first.transitions[3].applied, 4);
    assert_eq!(first.transitions[3].shortage, 2);
    assert_eq!(first.transitions[4].applied, 4);
    assert_eq!(first.transitions[4].shortage, 4);
    assert_eq!(first.final_stocks.len(), 2);
    assert_eq!(
        first
            .final_stocks
            .iter()
            .find(|stock| stock.faction_id == fixture.faction_a)
            .expect("faction A final stock")
            .quantity,
        2
    );
    assert_eq!(
        first
            .final_stocks
            .iter()
            .find(|stock| stock.faction_id == fixture.faction_b)
            .expect("faction B final stock")
            .quantity,
        4
    );

    fixture.app.close_world().expect("close world");
    fs::remove_file(fixture.path).expect("remove project");
}

#[test]
fn scenario_lifecycle_uses_its_variant_revision_and_never_changes_canon() {
    let mut fixture = fixture("simulation-lifecycle");
    let before = fixture
        .app
        .get_current_world()
        .expect("session")
        .expect("world");
    let branch = fixture
        .app
        .create_variant("simulation branch", before.current_revision)
        .expect("create branch");
    let mut input = scenario_input(&mut fixture);
    input.variant_id = branch.id;
    input.base_revision = before.current_revision;

    let mut unnamed = input.clone();
    unnamed.name = "   ".to_owned();
    assert_invalid(fixture.app.create_simulation_scenario(unnamed));

    let scenario = fixture
        .app
        .create_simulation_scenario(input.clone())
        .expect("create branch-based scenario");
    assert_eq!(scenario.variant_id, branch.id);
    assert_eq!(scenario.name, "Cosecha del norte");
    assert_eq!(scenario.base_revision, before.current_revision);
    assert_eq!(
        fixture.app.list_simulation_scenarios().unwrap(),
        [scenario.clone()]
    );
    assert_eq!(
        fixture
            .app
            .get_current_world()
            .unwrap()
            .expect("world")
            .current_revision,
        before.current_revision
    );

    let mut invalid_update = input.clone();
    invalid_update.rules[0] = SimulationRule::Production {
        faction_id: fixture.faction_a,
        resource_id: fixture.resource,
        amount: -1,
    };
    assert_invalid(
        fixture
            .app
            .update_simulation_scenario(scenario.id, invalid_update),
    );
    assert_eq!(
        fixture.app.list_simulation_scenarios().unwrap(),
        [scenario.clone()],
        "an invalid update must not replace the last complete scenario"
    );

    input.name = "Cosecha alternativa".to_owned();
    input.max_steps = 2;
    let updated = fixture
        .app
        .update_simulation_scenario(scenario.id, input)
        .expect("update scenario");
    assert_eq!(updated.id, scenario.id);
    assert_eq!(updated.name, "Cosecha alternativa");
    let run = fixture
        .app
        .run_simulation_scenario(scenario.id)
        .expect("run updated scenario");
    assert_eq!(run.variant_id, branch.id);
    assert_eq!(run.base_revision, before.current_revision);
    assert_eq!(run.steps_completed, 2);
    assert_eq!(
        fixture
            .app
            .get_current_world()
            .unwrap()
            .expect("world")
            .current_revision,
        before.current_revision
    );

    assert_eq!(
        fixture
            .app
            .delete_simulation_scenario(scenario.id)
            .expect("delete scenario")
            .id,
        scenario.id
    );
    assert!(fixture.app.list_simulation_scenarios().unwrap().is_empty());
    assert!(matches!(
        fixture.app.run_simulation_scenario(scenario.id),
        Err(AppError::SimulationScenarioNotFound(id)) if id == scenario.id
    ));
    assert_eq!(
        fixture
            .app
            .get_current_world()
            .unwrap()
            .expect("world")
            .current_revision,
        before.current_revision
    );

    fixture.app.close_world().expect("close world");
    fs::remove_file(fixture.path).expect("remove project");
}

#[test]
fn selected_transitions_prepare_exact_operations_without_changing_canon() {
    let mut fixture = fixture("simulation-promotion-selection");
    let before = fixture
        .app
        .get_current_world()
        .expect("session")
        .expect("world")
        .current_revision;
    let confirm_input = scenario_input(&mut fixture);
    let scenario = fixture
        .app
        .create_simulation_scenario(confirm_input)
        .expect("create scenario");
    fixture
        .app
        .run_simulation_scenario(scenario.id)
        .expect("execute scenario before promotion");

    let missing = fixture.app.prepare_simulation_review(
        scenario.id,
        SimulationPromotionInput {
            selections: vec![SimulationTransitionSelection::CreateEvent {
                step: 2,
                rule_index: 0,
                summary: "This transition does not exist".to_owned(),
                tick: None,
            }],
        },
    );
    assert!(matches!(
        missing,
        Err(AppError::InvalidSimulationPromotion(message))
            if message.contains("step 2 rule 0")
    ));

    let input = SimulationPromotionInput {
        selections: vec![
            SimulationTransitionSelection::CreateEvent {
                step: 1,
                rule_index: 0,
                summary: "The North Guild harvests a surplus".to_owned(),
                tick: Some(40),
            },
            SimulationTransitionSelection::CreateClaim {
                step: 1,
                rule_index: 1,
                subject_entity_id: fixture.faction_a,
                content: "The North Guild consumes part of its grain reserve.".to_owned(),
                tick: None,
            },
        ],
    };
    let round_trip = serde_json::to_value(&input).expect("serialize strict promotion input");
    assert_eq!(
        serde_json::from_value::<SimulationPromotionInput>(round_trip.clone())
            .expect("strict promotion round trip"),
        input
    );
    let mut unknown_input = round_trip.clone();
    unknown_input
        .as_object_mut()
        .expect("promotion object")
        .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<SimulationPromotionInput>(unknown_input).is_err());
    let mut unknown_selection = round_trip;
    unknown_selection["selections"][0]
        .as_object_mut()
        .expect("selection object")
        .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<SimulationPromotionInput>(unknown_selection).is_err());

    let review = fixture
        .app
        .prepare_simulation_review(scenario.id, input)
        .expect("prepare simulation review");
    assert_eq!(review.operations.len(), 2);
    assert_eq!(
        review
            .operations
            .iter()
            .map(|operation| operation
                .after
                .as_ref()
                .expect("create operation")
                .object_type
                .as_str())
            .collect::<Vec<_>>(),
        ["event", "claim"]
    );
    assert!(
        review
            .assumptions
            .contains(&format!("simulation://{}/step/1/rule/0", scenario.id))
    );
    assert!(
        review
            .assumptions
            .contains(&format!("simulation://{}/step/1/rule/1", scenario.id))
    );
    assert_eq!(
        fixture
            .app
            .get_current_world()
            .unwrap()
            .expect("world")
            .current_revision,
        before,
        "execute and prepare must not change canon"
    );
    assert_eq!(fixture.app.list_simulation_scenarios().unwrap(), [scenario]);

    fixture.app.close_world().expect("close world");
    fixture
        .app
        .open_world(fixture.path.clone())
        .expect("reopen promoted review");
    let pending = fixture.app.list_pending_reviews().expect("pending review");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].origin, PendingReviewOrigin::Simulation);
    assert_eq!(pending[0].review.operations.len(), 2);
    fixture.app.close_world().expect("close reopened world");
    fs::remove_file(fixture.path).expect("remove project");
}

#[test]
fn stale_scenario_blocks_promotion_without_mutating_scenario_or_head() {
    let mut fixture = fixture("simulation-promotion-stale");
    let discard_input = scenario_input(&mut fixture);
    let scenario = fixture
        .app
        .create_simulation_scenario(discard_input)
        .expect("create scenario");
    let branch = fixture
        .app
        .create_variant("other simulation branch", scenario.base_revision)
        .expect("create other variant");
    fixture
        .app
        .switch_variant(branch.id)
        .expect("switch away from scenario variant");
    let variant_error = fixture
        .app
        .prepare_simulation_review(
            scenario.id,
            SimulationPromotionInput {
                selections: vec![SimulationTransitionSelection::CreateEvent {
                    step: 1,
                    rule_index: 0,
                    summary: "Wrong variant result".to_owned(),
                    tick: None,
                }],
            },
        )
        .expect_err("active variant mismatch must block promotion");
    assert!(matches!(
        variant_error,
        AppError::SimulationScenarioStale {
            scenario_variant,
            active_variant,
            ..
        } if scenario_variant == scenario.variant_id && active_variant == branch.id
    ));
    fixture
        .app
        .switch_variant(scenario.variant_id)
        .expect("return to scenario variant");

    let added = entity(
        fixture.world_id,
        EntityKind::Place,
        "New Granary",
        "new-granary",
        20,
    );
    let review = fixture
        .app
        .start_manual_review(ManualReviewInput {
            objective: "Advance canonical head".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![DraftOperationInput::CreateEntity {
                retcon: RetconKind::Additive,
                after: added,
            }],
        })
        .expect("prepare head advance");
    let advanced = fixture
        .app
        .confirm_manual_review(&review)
        .expect("advance canonical head");

    let error = fixture
        .app
        .prepare_simulation_review(
            scenario.id,
            SimulationPromotionInput {
                selections: vec![SimulationTransitionSelection::CreateEvent {
                    step: 1,
                    rule_index: 0,
                    summary: "Stale result".to_owned(),
                    tick: None,
                }],
            },
        )
        .expect_err("stale scenario must block promotion");
    assert!(error.to_string().contains("rebase"));
    assert!(matches!(
        error,
        AppError::SimulationScenarioStale {
            scenario_id,
            base_revision,
            current_revision,
            ..
        } if scenario_id == scenario.id
            && base_revision == scenario.base_revision
            && current_revision == advanced.current_revision
    ));
    assert_eq!(
        fixture
            .app
            .get_current_world()
            .unwrap()
            .expect("world")
            .current_revision,
        advanced.current_revision
    );
    assert_eq!(fixture.app.list_simulation_scenarios().unwrap(), [scenario]);

    fixture.app.close_world().expect("close world");
    fs::remove_file(fixture.path).expect("remove project");
}

#[test]
fn standard_confirmation_applies_and_discard_keeps_the_scenario() {
    let mut fixture = fixture("simulation-promotion-confirm");
    let confirm_input = scenario_input(&mut fixture);
    let scenario = fixture
        .app
        .create_simulation_scenario(confirm_input)
        .expect("create scenario");
    let review = fixture
        .app
        .prepare_simulation_review(
            scenario.id,
            SimulationPromotionInput {
                selections: vec![
                    SimulationTransitionSelection::CreateEvent {
                        step: 1,
                        rule_index: 0,
                        summary: "The harvest fills the northern stores".to_owned(),
                        tick: Some(42),
                    },
                    SimulationTransitionSelection::CreateClaim {
                        step: 1,
                        rule_index: 1,
                        subject_entity_id: fixture.faction_a,
                        content: "The northern stores retain grain after consumption.".to_owned(),
                        tick: Some(42),
                    },
                ],
            },
        )
        .expect("prepare review for confirmation");
    assert!(review.ready_to_confirm);
    let committed = fixture
        .app
        .confirm_stored_manual_review(&review.review_key)
        .expect("confirm through standard stored review workflow");
    assert_ne!(committed.current_revision, scenario.base_revision);
    assert_eq!(
        fixture.app.list_simulation_scenarios().unwrap(),
        [scenario.clone()],
        "confirmation must not consume the external scenario"
    );
    let store = WorldStore::open(&fixture.path).expect("open committed project");
    let canon = store.read_canon_snapshot().expect("read committed canon");
    assert_eq!(canon.events().len(), 1);
    assert_eq!(canon.claims().len(), 1);
    assert_eq!(
        canon.claims()[0].authentication(),
        ClaimAuthentication::Disputed,
        "simulation findings are not canonical by default"
    );
    assert_eq!(
        canon.events()[0].event().summary(),
        "The harvest fills the northern stores"
    );
    assert_eq!(canon.events()[0].event().time().start_tick(), Some(42));
    assert_eq!(
        canon.claims()[0]
            .period()
            .expect("claim period")
            .start_tick(),
        Some(42)
    );
    drop(store);

    fixture.app.close_world().expect("close confirmed world");
    fs::remove_file(fixture.path).expect("remove confirmed project");

    let mut fixture = crate::fixture("simulation-promotion-discard");
    let before = fixture
        .app
        .get_current_world()
        .unwrap()
        .expect("world")
        .current_revision;
    let discard_input = scenario_input(&mut fixture);
    let scenario = fixture
        .app
        .create_simulation_scenario(discard_input)
        .expect("create discard scenario");
    let review = fixture
        .app
        .prepare_simulation_review(
            scenario.id,
            SimulationPromotionInput {
                selections: vec![SimulationTransitionSelection::CreateEvent {
                    step: 1,
                    rule_index: 2,
                    summary: "A transfer candidate".to_owned(),
                    tick: None,
                }],
            },
        )
        .expect("prepare discard review");
    let rejected = fixture
        .app
        .apply_stored_manual_review_action(
            &review.review_key,
            ManualReviewActionRequest::Reject {
                operation_id: review.operations[0].operation_id.clone(),
            },
        )
        .expect("reject promoted operation");
    assert!(!rejected.operations[0].selected);
    fixture
        .app
        .discard_stored_manual_review(&review.review_key)
        .expect("discard promoted review");
    assert_eq!(fixture.app.list_simulation_scenarios().unwrap(), [scenario]);
    assert_eq!(
        fixture
            .app
            .get_current_world()
            .unwrap()
            .expect("world")
            .current_revision,
        before,
        "rejecting and discarding must not change canon"
    );

    fixture.app.close_world().expect("close discarded world");
    fs::remove_file(fixture.path).expect("remove discarded project");
}
