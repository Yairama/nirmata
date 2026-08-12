use nirmata_app::{
    DraftOperationInput, ManualReviewInput, NirmataApp, ReadScope, SimulationResource,
    SimulationRule, SimulationScenarioInput, SimulationStock,
};
use nirmata_core::{
    World,
    calendar::{CalendarMonth, WorldCalendar},
    change_set::RetconKind,
    document::ObjectRef,
    entity::{Entity, EntityKind},
    event::{Event, EventAggregate},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::WorldStore;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct CapabilityOwner {
    capability: &'static str,
    target: &'static str,
    test: &'static str,
}

const FINAL_CAPABILITIES: &[&str] = &[
    "foundation",
    "retrieval",
    "snapshots",
    "deep_review",
    "lore",
    "variants",
    "history",
    "merge",
    "calendar",
    "simulation",
    "narrative",
    "internal_document",
    "continuity",
    "provider",
];

const CAPABILITY_OWNERS: &[CapabilityOwner] = &[
    CapabilityOwner {
        capability: "foundation",
        target: "tests/vertical_slice.rs",
        test: "mine_collapse_proposal_is_reviewed_committed_atomically_and_undone_after_reopen",
    },
    CapabilityOwner {
        capability: "retrieval",
        target: "tests/retrieval_benchmark.rs",
        test: "hybrid_active_path_meets_the_nir_053_gate",
    },
    CapabilityOwner {
        capability: "snapshots",
        target: "tests/snapshot_import.rs",
        test: "nir_058_hybrid_retrieval_and_snapshot_round_trip_preserve_authority_and_human_selection",
    },
    CapabilityOwner {
        capability: "deep_review",
        target: "tests/unit/deep_review.rs",
        test: "disagreement_requires_a_sourced_decision_point_before_standard_review",
    },
    CapabilityOwner {
        capability: "lore",
        target: "tests/unit/lore_import.rs",
        test: "nir_070_offline_multipage_import_commits_only_reviewed_provenance_and_undoes_after_reopen",
    },
    CapabilityOwner {
        capability: "variants",
        target: "tests/phase10_variants.rs",
        test: "variants_isolate_heads_history_reopen_stale_and_undo",
    },
    CapabilityOwner {
        capability: "history",
        target: "tests/phase10_variants.rs",
        test: "revision_history_follows_only_the_observed_variant_lineage",
    },
    CapabilityOwner {
        capability: "merge",
        target: "tests/phase10_variants.rs",
        test: "compare_and_limited_merge_use_ids_and_leave_source_untouched",
    },
    CapabilityOwner {
        capability: "calendar",
        target: "tests/phase10_variants.rs",
        test: "calendar_is_scoped_by_revision_variant_snapshot_and_undo_without_changing_ticks",
    },
    CapabilityOwner {
        capability: "simulation",
        target: "tests/simulation.rs",
        test: "scenario_lifecycle_uses_its_variant_revision_and_never_changes_canon",
    },
    CapabilityOwner {
        capability: "narrative",
        target: "tests/narrative.rs",
        test: "narrative_derivations_are_scoped_deterministic_bounded_and_read_only",
    },
    CapabilityOwner {
        capability: "internal_document",
        target: "tests/unit/ai.rs",
        test: "internal_document_is_perspective_scoped_referenced_and_stored_only_for_review",
    },
    CapabilityOwner {
        capability: "continuity",
        target: "tests/unit/ai.rs",
        test: "narrative_continuity_is_read_only_then_preserves_alternatives_and_sources_in_standard_review",
    },
    CapabilityOwner {
        capability: "provider",
        target: "../nirmata-ai/tests/capabilities/mod.rs",
        test: "provider_boundary_stays_concrete_without_marketplace_abstraction",
    },
];

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

#[test]
fn every_final_capability_has_one_stable_executable_owner() {
    assert_eq!(
        CAPABILITY_OWNERS
            .iter()
            .map(|owner| owner.capability)
            .collect::<Vec<_>>(),
        FINAL_CAPABILITIES
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for owner in CAPABILITY_OWNERS {
        let source = fs::read_to_string(root.join(owner.target)).unwrap_or_else(|error| {
            panic!(
                "owner target {} for {} is not readable: {error}",
                owner.target, owner.capability
            )
        });
        let signature = format!("fn {}(", owner.test);
        let index = source.find(&signature).unwrap_or_else(|| {
            panic!("owner test {} is missing from {}", owner.test, owner.target)
        });
        let annotation = source[..index]
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            annotation.contains("#[test]") || annotation.contains("#[tokio::test]"),
            "owner {} in {} is not an executable test",
            owner.test,
            owner.target
        );
    }
}

#[test]
fn calendar_variant_simulation_and_narrative_derivation_remain_read_only() {
    let path = project_path("general-acceptance");
    let world = World::new("Acceptance World", "", "Founding", 1).expect("world");
    let calendar = WorldCalendar::new(
        "Harbor Calendar",
        0,
        10,
        vec!["First".to_owned(), "Second".to_owned()],
        vec![CalendarMonth::new("Tide", 4).expect("month")],
    )
    .expect("calendar");
    let faction = Entity::new(
        world.id(),
        EntityKind::Faction,
        "Harbor Guild",
        "harbor-guild",
        "",
        "",
        "{}",
        vec![],
        2,
    )
    .expect("faction");
    let resource = Entity::new(
        world.id(),
        EntityKind::Resource,
        "Grain",
        "grain",
        "",
        "",
        "{}",
        vec![],
        3,
    )
    .expect("resource");
    let event = Event::new(
        world.id(),
        "arrival",
        "The grain fleet reaches the harbor.",
        "",
        EventTime::instant(10, TimePrecision::Exact, Certainty::Certain),
        None,
        vec![],
        vec![],
        4,
    )
    .expect("event");
    let event_id = event.id();

    WorldStore::create(&path, &world).expect("create store");

    let mut app = NirmataApp::default();
    app.open_world(path.clone()).expect("open world");
    let before_world = app
        .get_current_world()
        .expect("read initial session")
        .expect("open initial session")
        .world;
    let after_world = World::restore(
        before_world.id(),
        before_world.name(),
        before_world.premise_md(),
        before_world.epoch_label(),
        Some(calendar),
        before_world.current_revision(),
        before_world.created_at_ms(),
        before_world.updated_at_ms() + 1,
    )
    .expect("calendar world update");
    let fixture_review = app
        .start_manual_review(ManualReviewInput {
            objective: "Create the general acceptance fixture".to_owned(),
            sources: vec![],
            assumptions: vec![],
            operations: vec![
                DraftOperationInput::UpdateWorld {
                    retcon: RetconKind::Reinterpretive,
                    before: before_world,
                    after: after_world,
                },
                DraftOperationInput::CreateEntity {
                    retcon: RetconKind::Additive,
                    after: faction.clone(),
                },
                DraftOperationInput::CreateEntity {
                    retcon: RetconKind::Additive,
                    after: resource.clone(),
                },
                DraftOperationInput::CreateEvent {
                    retcon: RetconKind::Additive,
                    after: EventAggregate::new(event, vec![]),
                },
            ],
        })
        .expect("create fixture review");
    app.confirm_manual_review(&fixture_review)
        .expect("commit fixture review");
    let base_revision = app
        .get_current_world()
        .expect("read fixture session")
        .expect("open fixture session")
        .current_revision;
    let branch = app
        .create_variant("acceptance branch", base_revision)
        .expect("create variant");
    let scenario = app
        .create_simulation_scenario(SimulationScenarioInput {
            world_id: world.id(),
            variant_id: branch.id,
            base_revision,
            factions: vec![faction.id()],
            resources: vec![SimulationResource {
                resource_id: resource.id(),
                unit: "sacks".to_owned(),
            }],
            stocks: vec![SimulationStock {
                faction_id: faction.id(),
                resource_id: resource.id(),
                quantity: 2,
                capacity: 5,
            }],
            rules: vec![SimulationRule::Production {
                faction_id: faction.id(),
                resource_id: resource.id(),
                amount: 2,
            }],
            max_steps: 1,
            assumptions: vec!["The production rule runs once.".to_owned()],
        })
        .expect("create scenario");
    let run = app
        .run_simulation_scenario(scenario.id)
        .expect("run simulation");
    assert_eq!(run.final_stocks[0].quantity, 4);

    let timeline = app
        .derive_narrative_timeline(Some(ReadScope::head(branch.id)))
        .expect("derive branch timeline");
    assert_eq!(
        timeline.scope,
        ReadScope::historical(branch.id, base_revision)
    );
    assert_eq!(timeline.story_time.len(), 1);
    assert_eq!(
        timeline.story_time[0].event.object_ref,
        ObjectRef::Event(event_id)
    );
    assert_eq!(timeline.story_time[0].time.start_tick(), Some(10));

    let session = app
        .get_current_world()
        .expect("read session")
        .expect("open session");
    assert_eq!(session.current_revision, base_revision);
    let date = session
        .world
        .calendar()
        .expect("calendar remains present")
        .tick_to_date(10)
        .expect("calendar projection");
    assert_eq!((date.year, date.month, date.day), (0, 1, 2));

    app.close_world().expect("close world");
    fs::remove_file(path).expect("remove project");
}
