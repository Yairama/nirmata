use nirmata_app::{AppError, ExportSnapshotInput, NirmataApp};
use nirmata_core::{
    Period, World,
    claim::{Claim, ClaimAuthentication, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventAggregate, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{CanonSnapshot, WorldStore};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    project: PathBuf,
    parent: PathBuf,
    actor: Entity,
}

fn fixture_path(label: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&root).expect("create test root");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let parent = root.join(format!("{label}-{}-{nonce}", std::process::id()));
    fs::create_dir(&parent).expect("create snapshot parent");
    (parent.join("world.nirmata"), parent)
}

fn create_fixture() -> Fixture {
    let (project, parent) = fixture_path("snapshot-export");
    let world = World::new(
        "Memory Realm",
        "A realm preserves dangerous memories.",
        "First Bell",
        1,
    )
    .expect("world");
    let mut store = WorldStore::create(&project, &world).expect("create store");

    let rule = Rule::new(
        world.id(),
        RuleKind::Institutional,
        "Witnessed oaths bind.",
        "realm",
        RuleSeverity::Advisory,
        Some("charter".to_owned()),
        None,
        "{}",
        1,
    )
    .expect("rule");
    store.insert_rule(&rule).expect("insert rule");

    let actor = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Keeper of the gate",
        "Mara remembers the first bell.",
        r#"{"rank":"keeper"}"#,
        vec!["The Witness".to_owned()],
        1,
    )
    .expect("actor");
    let place = Entity::new(
        world.id(),
        EntityKind::Place,
        "North Gate",
        "north-gate",
        "The oldest gate",
        "The gate faces the salt road.",
        "{}",
        vec![],
        1,
    )
    .expect("place");
    store.insert_entity(&actor).expect("insert actor");
    store.insert_entity(&place).expect("insert place");

    let relation = Relation::new(
        world.id(),
        actor.id(),
        place.id(),
        "guards",
        RelationDirection::Directed,
        Some(1),
        None,
        Certainty::Certain,
        Some("charter".to_owned()),
        "{}",
    )
    .expect("relation");
    store.insert_relation(&relation).expect("insert relation");

    let goal = Goal::new(
        world.id(),
        actor.id(),
        "Keep the gate open to witnesses.",
        8,
        GoalStatus::Active,
        Some(Period::new(Some(1), None).expect("goal period")),
        GoalVisibility::Public,
        Some("oath".to_owned()),
    )
    .expect("goal");
    store.insert_goal(&goal).expect("insert goal");

    let consequence = Event::new(
        world.id(),
        "aftermath",
        "The gate remains open.",
        "Travelers enter safely.",
        EventTime::instant(3, TimePrecision::Exact, Certainty::Certain),
        Some(place.id()),
        vec![],
        vec![],
        1,
    )
    .expect("consequence");
    store
        .insert_event(&EventAggregate::new(consequence.clone(), vec![]))
        .expect("insert consequence");
    let event = Event::new(
        world.id(),
        "defense",
        "Mara defends the gate.",
        "Mara invokes the charter.",
        EventTime::instant(2, TimePrecision::Exact, Certainty::Certain),
        Some(place.id()),
        vec![EventParticipant::new(actor.id(), "defender", 0).expect("participant")],
        vec![goal.id()],
        1,
    )
    .expect("event");
    let link =
        EventLink::new(event.id(), consequence.id(), EventLinkKind::Causes).expect("event link");
    store
        .insert_event(&EventAggregate::new(event.clone(), vec![link]))
        .expect("insert event");

    let source_document = Document::new(
        world.id(),
        "Gate Charter",
        "charter",
        Some(actor.id()),
        Some(actor.id()),
        DocumentCanonStatus::Canonical,
        "The charter records Mara's oath.",
        1,
    )
    .expect("source document");
    store
        .insert_document(&DocumentAggregate::new(source_document.clone(), vec![]))
        .expect("insert source document");

    let claim = Claim::new(
        world.id(),
        actor.id(),
        "Mara kept the gate open.",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        ClaimPolarity::Positive,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        Some("direct observation".to_owned()),
        Some("charter".to_owned()),
        Some(source_document.id()),
        None,
        None,
        Some(Period::new(Some(2), Some(3)).expect("claim period")),
        world.current_revision(),
    )
    .expect("claim");
    store.insert_claim(&claim).expect("insert claim");

    let chronicle = Document::new(
        world.id(),
        "Gate Chronicle",
        "chronicle",
        Some(actor.id()),
        Some(actor.id()),
        DocumentCanonStatus::Canonical,
        "Mara defended the gate, and witnesses entered.",
        1,
    )
    .expect("chronicle");
    let targets = [
        ObjectRef::Entity(actor.id()),
        ObjectRef::Relation(relation.id()),
        ObjectRef::Event(event.id()),
        ObjectRef::Claim(claim.id()),
        ObjectRef::Rule(rule.id()),
        ObjectRef::Goal(goal.id()),
        ObjectRef::Document(source_document.id()),
    ];
    let references = targets
        .into_iter()
        .enumerate()
        .map(|(ordinal, target)| {
            ContentReference::new(
                ObjectRef::Document(chronicle.id()),
                target,
                u32::try_from(ordinal).expect("small ordinal"),
            )
        })
        .collect();
    store
        .insert_document(&DocumentAggregate::new(chronicle, references))
        .expect("insert chronicle with references");
    drop(store);

    Fixture {
        project,
        parent,
        actor,
    }
}

fn canon_value(snapshot: &CanonSnapshot) -> Value {
    json!({
        "world": snapshot.world(),
        "schemaVersion": snapshot.schema_version(),
        "entities": snapshot.entities(),
        "relations": snapshot.relations(),
        "goals": snapshot.goals(),
        "events": snapshot.events(),
        "claims": snapshot.claims(),
        "rules": snapshot.rules(),
        "documents": snapshot.documents(),
        "references": snapshot.content_references(),
    })
}

fn read_manifest(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path.join("manifest.json")).expect("read manifest"))
        .expect("parse manifest")
}

fn export(app: &NirmataApp, parent: &Path, name: &str) -> nirmata_app::ExportSnapshotResult {
    app.export_vfs_snapshot(ExportSnapshotInput {
        parent_directory: parent.to_path_buf(),
        snapshot_name: name.to_owned(),
    })
    .expect("export snapshot")
}

#[test]
fn exports_complete_equivalent_snapshots_with_stable_identity_and_no_canon_write() {
    let fixture = create_fixture();
    let before = canon_value(
        &WorldStore::open(&fixture.project)
            .expect("open before export")
            .read_canon_snapshot()
            .expect("read before export"),
    );
    let mut app = NirmataApp::default();
    let session = app
        .open_world(fixture.project.clone())
        .expect("open fixture in app");

    let first = export(&app, &fixture.parent, "snapshot-a");
    let second = export(&app, &fixture.parent, "snapshot-b");
    assert_eq!(first.world_id, session.world_id.to_string());
    assert_eq!(first.base_revision, session.current_revision.to_string());
    assert_eq!(first.logical_hash, second.logical_hash);

    let first_manifest_bytes = fs::read(first.path.join("manifest.json")).expect("first manifest");
    let second_manifest_bytes =
        fs::read(second.path.join("manifest.json")).expect("second manifest");
    assert_eq!(first_manifest_bytes, second_manifest_bytes);
    let manifest: Value = serde_json::from_slice(&first_manifest_bytes).expect("manifest JSON");
    assert_eq!(manifest["format"], "nirmata-vfs-snapshot");
    assert_eq!(manifest["format_version"], 1);
    assert_eq!(manifest["hash_algorithm"], "sha256");
    assert_eq!(manifest["world_id"], session.world_id.to_string());
    assert_eq!(manifest["variant"], "main");
    assert_eq!(
        manifest["variant_id"],
        session.active_variant.id.to_string()
    );
    assert_eq!(
        manifest["base_revision"],
        session.current_revision.to_string()
    );
    assert_eq!(manifest["canon_schema_version"], 8);
    assert_eq!(manifest["logical_hash"], first.logical_hash);

    let objects = manifest["objects"].as_array().expect("manifest objects");
    assert_eq!(objects.len(), first.object_count);
    let object_types = objects
        .iter()
        .map(|object| object["object_type"].as_str().expect("object type"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        object_types,
        BTreeSet::from([
            "world", "entity", "relation", "event", "claim", "rule", "goal", "document",
        ])
    );

    let references = objects
        .iter()
        .flat_map(|object| object["references"].as_array().expect("references"))
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 7);
    let referenced_types = references
        .iter()
        .map(|reference| {
            reference["target_uri"]
                .as_str()
                .expect("target URI")
                .split('/')
                .nth(2)
                .expect("URI type")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        referenced_types,
        BTreeSet::from([
            "entity", "relation", "event", "claim", "rule", "goal", "document"
        ])
    );

    for object in objects {
        let relative = object["path"].as_str().expect("relative object path");
        assert!(!relative.contains(".."));
        let first_bytes = fs::read(first.path.join(relative)).expect("first object file");
        let second_bytes = fs::read(second.path.join(relative)).expect("second object file");
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            object["content_hash"],
            format!("sha256:{:x}", Sha256::digest(&first_bytes))
        );
        assert!(
            String::from_utf8(first_bytes)
                .expect("Markdown is UTF-8")
                .contains("## Content")
        );
    }

    let occupied = fixture.parent.join("occupied");
    fs::create_dir(&occupied).expect("create occupied destination");
    let error = app
        .export_vfs_snapshot(ExportSnapshotInput {
            parent_directory: fixture.parent.clone(),
            snapshot_name: "occupied".to_owned(),
        })
        .expect_err("occupied destination must be rejected");
    assert!(matches!(error, AppError::SnapshotDestinationOccupied(_)));

    app.close_world().expect("close after exports");
    let after = canon_value(
        &WorldStore::open(&fixture.project)
            .expect("reopen after export")
            .read_canon_snapshot()
            .expect("read after export"),
    );
    assert_eq!(after, before, "export and reopen must not change canon");

    let mut store = WorldStore::open(&fixture.project).expect("open for rename");
    let renamed = Entity::restore(
        fixture.actor.id(),
        fixture.actor.world_id(),
        fixture.actor.kind(),
        "Mara Vale",
        "mara-vale",
        fixture.actor.summary(),
        fixture.actor.body_md(),
        fixture.actor.attributes_json().as_str(),
        fixture.actor.aliases().to_vec(),
        fixture.actor.version(),
        fixture.actor.created_at_ms(),
        2,
    )
    .expect("renamed entity");
    store.update_entity(&renamed).expect("persist rename");
    let canon_after_rename = canon_value(&store.read_canon_snapshot().expect("renamed canon"));
    drop(store);

    app.open_world(fixture.project.clone())
        .expect("reopen renamed fixture");
    let renamed_export = export(&app, &fixture.parent, "snapshot-renamed");
    let renamed_manifest = read_manifest(&renamed_export.path);
    let original_entity = objects
        .iter()
        .find(|object| object["id"] == fixture.actor.id().to_string())
        .expect("original entity entry");
    let renamed_entity = renamed_manifest["objects"]
        .as_array()
        .expect("renamed objects")
        .iter()
        .find(|object| object["id"] == fixture.actor.id().to_string())
        .expect("renamed entity entry");
    assert_eq!(renamed_entity["uri"], original_entity["uri"]);
    assert_eq!(renamed_entity["path"], original_entity["path"]);
    assert_eq!(renamed_entity["metadata"]["name"], "Mara Vale");
    assert_ne!(renamed_export.logical_hash, first.logical_hash);

    app.close_world().expect("close renamed world");
    let reopened_after_rename = canon_value(
        &WorldStore::open(&fixture.project)
            .expect("reopen renamed project")
            .read_canon_snapshot()
            .expect("read reopened renamed canon"),
    );
    assert_eq!(reopened_after_rename, canon_after_rename);

    fs::remove_dir_all(&fixture.parent).expect("remove fixture");
}
