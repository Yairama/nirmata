use crate::{CommittedChangeSetRecord, OperationAudit, OperationDecision, StoredRevision};
use nirmata_core::{
    ChangeSetId, EntityId,
    change_set::{ChangeOperation, ChangeSet, RetconKind},
    document::ObjectRef,
    entity::{Entity, EntityKind},
};

fn durability_entity(world: &World, name: &str, slug: &str, now_ms: i64) -> Entity {
    Entity::new(
        world.id(),
        EntityKind::Person,
        name,
        slug,
        "",
        format!("{name} keeps the Moonvault records."),
        "{}",
        vec![],
        now_ms,
    )
    .expect("durability entity")
}

fn durability_record(
    world: &World,
    objective: &str,
    entities: &[Entity],
    now_ms: i64,
) -> CommittedChangeSetRecord {
    let operations = entities
        .iter()
        .map(|entity| ChangeOperation::CreateEntity {
            operation_id: nirmata_core::ChangeOperationId::new(),
            affected_ids: vec![ObjectRef::Entity(entity.id())],
            expected_version: 0,
            retcon: RetconKind::Additive,
            after: entity.clone(),
        })
        .collect::<Vec<_>>();
    let change_set = ChangeSet::new(
        world.id(),
        world.current_revision(),
        objective,
        vec![],
        vec![],
        operations.clone(),
        vec![],
    )
    .expect("durability change set");
    let revision = StoredRevision::new(
        world.id(),
        Some(world.current_revision()),
        Some(change_set.id()),
        "durability_test",
        objective,
        now_ms,
    )
    .expect("durability revision");
    let audits = operations
        .iter()
        .map(|operation| {
            OperationAudit::from_operation(
                operation,
                OperationDecision::Accept,
                "durability_test",
                now_ms,
            )
            .expect("durability audit")
        })
        .collect();
    CommittedChangeSetRecord::new(change_set, None, vec![], audits, revision, None)
        .expect("durability record")
}

fn assert_failed_commit_reopens_cleanly(
    path: &Path,
    expected_head: nirmata_core::RevisionId,
    failed_change_set_id: ChangeSetId,
    absent_entities: &[EntityId],
) -> WorldStore {
    let reopened = WorldStore::open(path).expect("project reopens after failed commit");
    assert_eq!(
        reopened
            .load_world()
            .expect("world after failed commit")
            .current_revision(),
        expected_head
    );
    assert_eq!(
        reopened.list_revisions().expect("revision history after failure").len(),
        1
    );
    assert_eq!(
        reopened
            .connection
            .query_row("SELECT COUNT(*) FROM change_operation_audits", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("audit count after failure"),
        0
    );
    assert_eq!(
        reopened
            .get_committed_change_set(failed_change_set_id)
            .expect("failed change set lookup"),
        None
    );
    for entity_id in absent_entities {
        assert_eq!(
            reopened.get_entity(*entity_id).expect("failed entity lookup"),
            None
        );
    }
    reopened
}

#[test]
fn constraint_failure_rolls_back_canon_head_and_audit_then_allows_a_corrected_retry() {
    let path = project_path("constraint-durability");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
    let first = durability_entity(&world, "Mara", "shared-slug", 2);
    let duplicate = durability_entity(&world, "Sera", "shared-slug", 3);
    let failed = durability_record(
        &world,
        "Create two conflicting people",
        &[first.clone(), duplicate.clone()],
        4,
    );
    let failed_id = failed.change_set().id();

    let error = store
        .commit_change_set(&failed)
        .expect_err("duplicate slug must fail the transaction");
    assert!(matches!(error, StoreError::Database(_, _)));
    drop(store);

    let mut reopened = assert_failed_commit_reopens_cleanly(
        &path,
        world.current_revision(),
        failed_id,
        &[first.id(), duplicate.id()],
    );
    let corrected = durability_entity(&world, "Mara", "mara", 5);
    let retry = durability_record(&world, "Create corrected person", &[corrected.clone()], 6);
    let retry_revision = retry.revision().id();
    reopened
        .commit_change_set(&retry)
        .expect("corrected commit can be retried");
    drop(reopened);

    let verified = WorldStore::open(&path).expect("reopen corrected project");
    assert_eq!(
        verified.load_world().expect("corrected world").current_revision(),
        retry_revision
    );
    assert_eq!(
        verified.get_entity(corrected.id()).expect("corrected entity"),
        Some(corrected)
    );
    assert_eq!(verified.list_revisions().expect("corrected history").len(), 2);
    drop(verified);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn real_sqlite_lock_rolls_back_and_commit_succeeds_after_the_lock_is_released() {
    let path = project_path("lock-durability");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
    let entity = durability_entity(&world, "Mara", "mara", 2);
    let record = durability_record(&world, "Create Mara after lock", &[entity.clone()], 3);
    let failed_id = record.change_set().id();
    let revision_id = record.revision().id();
    let lock = rusqlite::Connection::open(&path).expect("open lock connection");
    lock.execute_batch("BEGIN EXCLUSIVE")
        .expect("take real SQLite exclusive lock");

    let error = store
        .commit_change_set(&record)
        .expect_err("exclusive lock must reject commit");
    assert!(matches!(error, StoreError::Locked(locked_path) if locked_path == path));
    lock.execute_batch("ROLLBACK").expect("release SQLite lock");
    drop(lock);
    drop(store);

    let mut reopened = assert_failed_commit_reopens_cleanly(
        &path,
        world.current_revision(),
        failed_id,
        &[entity.id()],
    );
    reopened
        .commit_change_set(&record)
        .expect("same reviewed commit retries after lock release");
    drop(reopened);

    let verified = WorldStore::open(&path).expect("reopen retried project");
    assert_eq!(
        verified.load_world().expect("retried world").current_revision(),
        revision_id
    );
    assert_eq!(verified.get_entity(entity.id()).expect("retried entity"), Some(entity));
    drop(verified);
    fs::remove_file(path).expect("remove project");
}

#[test]
fn simulated_derived_index_failure_rolls_back_canon_head_and_audit_then_retries() {
    let path = project_path("derived-index-durability");
    let world = World::new("Arcadia", "", "First Dawn", 1).expect("world");
    let mut store = WorldStore::create(&path, &world).expect("store");
    let entity = durability_entity(&world, "Mara", "mara", 2);
    let record = durability_record(&world, "Create indexed Mara", &[entity.clone()], 3);
    let failed_id = record.change_set().id();
    let revision_id = record.revision().id();
    store.fail_next_derived_index_update = true;

    let error = store
        .commit_change_set(&record)
        .expect_err("simulated derived index update must fail");
    assert!(matches!(error, StoreError::Database(_, _)));
    drop(store);

    let mut reopened = assert_failed_commit_reopens_cleanly(
        &path,
        world.current_revision(),
        failed_id,
        &[entity.id()],
    );
    assert!(
        reopened
            .search_canon_text("Moonvault")
            .expect("search after rollback")
            .is_empty()
    );
    reopened
        .commit_change_set(&record)
        .expect("commit retries after transient index failure");
    drop(reopened);

    let verified = WorldStore::open(&path).expect("reopen retried indexed project");
    assert_eq!(
        verified.load_world().expect("indexed world").current_revision(),
        revision_id
    );
    assert_eq!(
        verified.search_canon_text("Moonvault").expect("indexed search"),
        vec![ObjectRef::Entity(entity.id())]
    );
    drop(verified);
    fs::remove_file(path).expect("remove project");
}
