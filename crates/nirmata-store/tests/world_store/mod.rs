use super::*;
use crate::{DocumentAggregate, StructuredSearchKind, StructuredSearchQuery};
use nirmata_core::document::{Document, DocumentCanonStatus, ObjectRef as SearchObjectRef};
use std::time::{SystemTime, UNIX_EPOCH};

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

fn create_initial_project(path: &Path, world: &World) {
    let mut connection = Connection::open(path).expect("create initial database");
    enable_foreign_keys(path, &connection).expect("enable foreign keys");
    let transaction = connection.transaction().expect("begin initial schema");
    transaction
        .execute_batch(INITIAL_SCHEMA)
        .expect("create initial schema");
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (1, ?1)",
            [world.created_at_ms()],
        )
        .expect("record initial schema");
    transaction
        .execute(
            "INSERT INTO worlds (
                    id, name, premise_md, epoch_label, current_revision,
                    created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                world.id().to_string(),
                world.name(),
                world.premise_md(),
                world.epoch_label(),
                world.current_revision().to_string(),
                world.created_at_ms(),
                world.updated_at_ms(),
            ],
        )
        .expect("insert initial world");
    transaction
        .execute(
            "INSERT INTO revisions (id, world_id, parent_revision_id, created_at_ms)
                 VALUES (?1, ?2, NULL, ?3)",
            params![
                world.current_revision().to_string(),
                world.id().to_string(),
                world.created_at_ms(),
            ],
        )
        .expect("insert initial revision");
    transaction
        .pragma_update(None, "user_version", 1)
        .expect("set initial version");
    transaction.commit().expect("commit initial schema");
}

fn downgrade_to_schema_seven(path: &Path) {
    let connection = Connection::open(path).expect("open project as old build");
    connection
        .execute_batch(
            "DROP TRIGGER variants_head_world_insert;
             DROP TRIGGER variants_head_world_update;
             DROP TRIGGER worlds_active_variant_update;
             DROP TRIGGER revisions_variant_insert;
             DROP TRIGGER revisions_variant_update;
             DROP TRIGGER revisions_source_insert;
             DROP TRIGGER revisions_source_update;
             DROP TRIGGER change_sets_variant_insert;
             DROP TRIGGER change_sets_variant_update;
             DROP TRIGGER import_batches_variant_insert;
             DROP TRIGGER import_batches_variant_update;
             DROP INDEX revisions_variant_parent;
             DROP INDEX revisions_variant_id;
             DROP INDEX change_sets_variant_id;
             DROP INDEX import_batches_variant_id;
             DROP TABLE revision_snapshots;
             DROP TABLE variants;
             ALTER TABLE worlds DROP COLUMN active_variant_id;
             ALTER TABLE revisions DROP COLUMN source_revision_id;
             ALTER TABLE revisions DROP COLUMN variant_id;
             ALTER TABLE change_sets DROP COLUMN variant_id;
             ALTER TABLE import_batches DROP COLUMN variant_id;
             CREATE UNIQUE INDEX revisions_linear_parent ON revisions (parent_revision_id)
                 WHERE parent_revision_id IS NOT NULL;
             UPDATE schema_migrations SET version = 7 WHERE version = 9;
             PRAGMA user_version = 7;",
        )
        .expect("downgrade fixture to schema seven");
}

fn insert_entity(connection: &Connection, world_id: &str, id: &str, slug: &str) {
    connection
        .execute(
            "INSERT INTO entities (
                    id, world_id, kind, name, slug, summary, body_md, attributes_json,
                    version, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, 'person', ?1, ?3, '', '', '{}', 1, 1, 1)",
            params![id, world_id, slug],
        )
        .expect("insert entity");
}

#[test]
fn creates_schema_and_reopens_same_world() {
    let path = project_path("store-round-trip");
    let world =
        World::new("Arcadia", "A remembered world.", "First Dawn", 42).expect("valid world");

    let store = WorldStore::create(&path, &world).expect("create project");
    assert_eq!(store.load_world().expect("load created world"), world);
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))
            .expect("foreign keys pragma"),
        1
    );
    let parent: Option<String> = store
        .connection
        .query_row(
            "SELECT parent_revision_id FROM revisions WHERE id = ?1",
            [world.current_revision().to_string()],
            |row| row.get(0),
        )
        .expect("initial revision");
    assert!(parent.is_none());
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .expect("schema version"),
        SCHEMA_VERSION
    );
    let canon_tables: i64 = store
        .connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'rules', 'entities', 'entity_aliases', 'relations', 'events',
                       'event_participants', 'event_links', 'event_goals', 'goals',
                       'claims', 'documents', 'content_references', 'canon_fts'
                   )",
            [],
            |row| row.get(0),
        )
        .expect("canon tables");
    assert_eq!(canon_tables, 13);
    let change_set_tables: i64 = store
        .connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'change_sets', 'change_operations', 'decision_points',
                       'change_set_waivers', 'change_operation_audits', 'revision_undos'
                   )",
            [],
            |row| row.get(0),
        )
        .expect("change set tables");
    assert_eq!(change_set_tables, 6);
    drop(store);

    let reopened = WorldStore::open(&path).expect("reopen project");
    assert_eq!(reopened.load_world().expect("load reopened world"), world);
    drop(reopened);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn semantic_failure_falls_back_to_fts_without_changing_canon() {
    let path = project_path("semantic-failure-fallback");
    let world = World::new("Arcadia", "", "Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
    let document = Document::new(
        world.id(),
        "Senate Record",
        "minutes",
        None,
        None,
        DocumentCanonStatus::Canonical,
        "The senate refuses the border pact.",
        1,
    )
    .expect("document");
    store
        .insert_document(&DocumentAggregate::new(document.clone(), vec![]))
        .expect("insert document");
    store.fail_semantic_search = true;

    let hits = store
        .search_structured(&StructuredSearchQuery {
            kinds: vec![StructuredSearchKind::Document],
            text: Some("senate refuses pact".to_owned()),
            limit: 5,
            ..Default::default()
        })
        .expect("FTS fallback");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].object, SearchObjectRef::Document(document.id()));
    assert_eq!(hits[0].provenance, "fts5");
    assert_eq!(
        store
            .get_document(document.id())
            .expect("read canon")
            .expect("document remains")
            .object(),
        &document
    );

    drop(store);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn migrates_initial_schema_to_complete_canon() {
    let path = project_path("initial-migration");
    let world = World::new("Arcadia", "# Premise", "First Dawn", 42).expect("valid world");
    create_initial_project(&path, &world);

    let store = WorldStore::open(&path).expect("migrate initial schema");
    assert_eq!(store.load_world().expect("load migrated world"), world);
    assert_eq!(
        store
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("latest migration"),
        SCHEMA_VERSION
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                     WHERE type = 'table'
                       AND name IN (
                           'content_references', 'change_sets', 'change_operation_audits',
                           'canon_fts', 'revision_undos'
                       )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("complete schema"),
        5
    );
    drop(store);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn migrates_version_seven_to_main_without_changing_ids_or_history() {
    let path = project_path("variant-migration");
    let world = World::new("Arcadia", "", "Dawn", 42).expect("world");
    let store = WorldStore::create(&path, &world).expect("create current project");
    let entity_id = nirmata_core::EntityId::new();
    insert_entity(
        &store.connection,
        &world.id().to_string(),
        &entity_id.to_string(),
        "mara",
    );
    drop(store);

    downgrade_to_schema_seven(&path);

    let migrated = WorldStore::open(&path).expect("migrate variants");
    let variant = migrated.active_variant().expect("main variant");
    assert_eq!(variant.name, "main");
    assert_eq!(variant.head_revision_id, world.current_revision());
    assert_eq!(migrated.list_variants().expect("variants").len(), 1);
    assert_eq!(migrated.load_world().expect("world").id(), world.id());
    assert_eq!(
        migrated
            .read_canon_snapshot_scoped(crate::ReadScope::historical(
                variant.id,
                world.current_revision(),
            ))
            .expect("historical root")
            .entities()[0]
            .id(),
        entity_id
    );
    assert_eq!(migrated.list_revisions().expect("history").len(), 1);
    drop(migrated);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn failed_variant_backfill_rolls_back_schema_main_and_snapshots() {
    let path = project_path("variant-backfill-rollback");
    let world = World::new("Arcadia", "", "Dawn", 42).expect("world");
    let store = WorldStore::create(&path, &world).expect("create current project");
    drop(store);

    let missing_parent = RevisionId::new();
    let connection = Connection::open(&path).expect("open fixture");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys for corrupt fixture");
    connection
        .execute(
            "UPDATE revisions SET parent_revision_id = ?1 WHERE id = ?2",
            params![
                missing_parent.to_string(),
                world.current_revision().to_string()
            ],
        )
        .expect("break revision chain");
    drop(connection);
    downgrade_to_schema_seven(&path);

    assert!(WorldStore::open(&path).is_err());
    let connection = Connection::open(&path).expect("inspect rolled back migration");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version");
    assert_eq!(version, 7);
    let variant_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name IN ('variants', 'revision_snapshots')",
            [],
            |row| row.get(0),
        )
        .expect("variant tables");
    assert_eq!(variant_tables, 0);
    assert_eq!(
        connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("recorded version"),
        7
    );
    drop(connection);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn variant_integrity_triggers_reject_null_and_unknown_revision_links() {
    let path = project_path("variant-integrity");
    let world = World::new("Arcadia", "", "Dawn", 1).expect("world");
    let store = WorldStore::create(&path, &world).expect("create store");
    let active = store.active_variant().expect("active variant");
    drop(store);

    let connection = Connection::open(&path).expect("open database");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable foreign keys");
    assert!(
        connection
            .execute(
                "UPDATE worlds SET active_variant_id = NULL WHERE id = ?1",
                [world.id().to_string()],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE revisions SET variant_id = NULL WHERE id = ?1",
                [world.current_revision().to_string()],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE revisions SET source_revision_id = ?1 WHERE id = ?2",
                params![
                    RevisionId::new().to_string(),
                    world.current_revision().to_string()
                ],
            )
            .is_err()
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT active_variant_id FROM worlds WHERE id = ?1",
                [world.id().to_string()],
                |row| row.get::<_, String>(0),
            )
            .expect("active variant remains"),
        active.id.to_string()
    );
    drop(connection);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn constraints_reject_broken_fk_duplicate_slug_and_impossible_interval() {
    let path = project_path("canon-constraints");
    let world = World::new("Arcadia", "", "", 42).expect("valid world");
    let store = WorldStore::create(&path, &world).expect("create project");
    let world_id = world.id().to_string();
    insert_entity(&store.connection, &world_id, "entity-1", "mara");

    assert!(
        store
            .connection
            .execute(
                "INSERT INTO goals (
                        id, world_id, holder_entity_id, desired_state_md, priority,
                        status, visibility, version
                     ) VALUES ('goal-1', ?1, 'missing-entity', 'Escape', 1,
                               'active', 'secret', 1)",
                [&world_id],
            )
            .is_err()
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT COUNT(*) FROM goals", [], |row| row.get::<_, i64>(0))
            .expect("goal count"),
        0
    );

    assert!(
        store
            .connection
            .execute(
                "INSERT INTO entities (
                        id, world_id, kind, name, slug, summary, body_md, attributes_json,
                        version, created_at_ms, updated_at_ms
                     ) VALUES
                        ('entity-2', ?1, 'person', 'Talia', 'talia', '', '', '{}', 1, 1, 1),
                        ('entity-3', ?1, 'person', 'Other Mara', 'mara', '', '', '{}', 1, 1, 1)",
                [&world_id],
            )
            .is_err()
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT COUNT(*) FROM entities", [], |row| row
                .get::<_, i64>(0))
            .expect("entity count"),
        1
    );

    assert!(
        store
            .connection
            .execute(
                "INSERT INTO events (
                        id, world_id, kind, summary, body_md, time_kind, start_tick,
                        end_tick, time_precision, certainty, version, created_at_ms,
                        updated_at_ms
                     ) VALUES ('event-1', ?1, 'journey', '', '', 'interval', 10, 9,
                               'exact', 'certain', 1, 1, 1)",
                [&world_id],
            )
            .is_err()
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT COUNT(*) FROM events", [], |row| row
                .get::<_, i64>(0))
            .expect("event count"),
        0
    );

    drop(store);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn failed_migration_rolls_back_schema_and_version() {
    let path = project_path("atomic-migration");
    let world = World::new("Arcadia", "", "", 42).expect("valid world");
    create_initial_project(&path, &world);
    let connection = Connection::open(&path).expect("open initial database");
    connection
        .execute("CREATE TABLE entities (id TEXT PRIMARY KEY)", [])
        .expect("create migration conflict");
    drop(connection);

    assert!(matches!(
        WorldStore::open(&path),
        Err(StoreError::Database(_, _))
    ));

    let connection = Connection::open(&path).expect("inspect rolled back database");
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .expect("schema version"),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("latest migration"),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                     WHERE type = 'table' AND name = 'rules'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("rolled back rules table"),
        0
    );
    drop(connection);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn change_set_foreign_keys_require_existing_revisions() {
    let path = project_path("change-set-fk");
    let world = World::new("Arcadia", "", "", 42).expect("valid world");
    let store = WorldStore::create(&path, &world).expect("create project");

    assert!(
        store
            .connection
            .execute(
                "INSERT INTO change_sets (
                        id, world_id, kind, base_revision_id, result_revision_id, objective,
                        source_refs_json, assumptions_json, deterministic_report_json,
                        created_at_ms, updated_at_ms
                     ) VALUES (?1, ?2, 'draft', 'missing-revision', NULL, 'Add Mara',
                               '[]', '[]', NULL, 1, 1)",
                params![
                    nirmata_core::ChangeSetId::new().to_string(),
                    world.id().to_string()
                ],
            )
            .is_err()
    );

    drop(store);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn audited_operation_without_change_set_is_rejected() {
    let path = project_path("audit-fk");
    let world = World::new("Arcadia", "", "", 42).expect("valid world");
    let store = WorldStore::create(&path, &world).expect("create project");

    assert!(
        store
            .connection
            .execute(
                "INSERT INTO change_operation_audits (
                        change_set_id, operation_id, decision, source, before_json, after_json,
                        decided_at_ms
                     ) VALUES (?1, ?2, 'accept', 'manual_review', NULL, '{}', 1)",
                params![
                    nirmata_core::ChangeSetId::new().to_string(),
                    nirmata_core::ChangeOperationId::new().to_string(),
                ],
            )
            .is_err()
    );

    drop(store);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn rejects_newer_schema() {
    let path = project_path("newer-schema");
    let world = World::new("Arcadia", "", "", 42).expect("valid world");
    drop(WorldStore::create(&path, &world).expect("create project"));
    let connection = Connection::open(&path).expect("open raw database");
    let original_head: String = connection
        .query_row("SELECT current_revision FROM worlds", [], |row| row.get(0))
        .expect("head before future schema");
    let original_revisions: i64 = connection
        .query_row("SELECT COUNT(*) FROM revisions", [], |row| row.get(0))
        .expect("revisions before future schema");
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("set newer schema");
    drop(connection);

    assert!(matches!(
        WorldStore::open(&path),
        Err(StoreError::IncompatibleSchema {
            found,
            supported,
            ..
        }) if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
    ));

    let connection = Connection::open(&path).expect("inspect rejected future schema");
    assert_eq!(
        connection
            .query_row("SELECT current_revision FROM worlds", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("head after rejection"),
        original_head
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM revisions", [], |row| row
                .get::<_, i64>(0))
            .expect("revisions after rejection"),
        original_revisions
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM change_operation_audits", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("audits after rejection"),
        0
    );
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .expect("restore supported fixture version");
    drop(connection);
    let reopened = WorldStore::open(&path).expect("reopen after restoring supported version");
    assert_eq!(
        reopened
            .load_world()
            .expect("reopened world")
            .current_revision(),
        world.current_revision()
    );
    drop(reopened);
    fs::remove_file(path).expect("remove test project");
}

#[test]
fn invalid_project_is_not_overwritten() {
    let path = project_path("invalid-project");
    fs::write(&path, b"not sqlite").expect("write invalid project");

    assert!(matches!(
        WorldStore::open(&path),
        Err(StoreError::InvalidFormat(_))
    ));
    assert_eq!(fs::read(&path).expect("read original file"), b"not sqlite");
    fs::remove_file(path).expect("remove test project");
}

include!("durability.rs");
