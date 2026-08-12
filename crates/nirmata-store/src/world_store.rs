use crate::schema::{
    CANON_SCHEMA, CHANGE_SET_SCHEMA, INITIAL_SCHEMA, LORE_IMPORT_SCHEMA,
    REVISION_COMPLETION_SCHEMA, SCHEMA_VERSION, UNDO_SCHEMA, VARIANT_SCHEMA,
};
use crate::search;
use nirmata_core::{
    DomainError, RevisionId, VariantId, World, WorldId,
    claim::Claim,
    document::{ContentReference, DocumentAggregate},
    entity::Entity,
    event::EventAggregate,
    goal::Goal,
    relation::Relation,
    rule::Rule,
};
use rusqlite::{Connection, ErrorCode, OpenFlags, params, types::Type};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    ffi::OsStr,
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

pub struct WorldStore {
    pub(crate) connection: Connection,
    pub(crate) path: PathBuf,
    pub(crate) world_id: WorldId,
    #[cfg(test)]
    pub(crate) fail_next_derived_index_update: bool,
    #[cfg(test)]
    pub(crate) fail_semantic_search: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanonSnapshot {
    pub(crate) world: World,
    pub(crate) schema_version: i64,
    pub(crate) entities: Vec<Entity>,
    pub(crate) relations: Vec<Relation>,
    pub(crate) goals: Vec<Goal>,
    pub(crate) events: Vec<EventAggregate>,
    pub(crate) claims: Vec<Claim>,
    pub(crate) rules: Vec<Rule>,
    pub(crate) documents: Vec<DocumentAggregate>,
    pub(crate) content_references: Vec<ContentReference>,
}

impl CanonSnapshot {
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn schema_version(&self) -> i64 {
        self.schema_version
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn relations(&self) -> &[Relation] {
        &self.relations
    }

    pub fn goals(&self) -> &[Goal] {
        &self.goals
    }

    pub fn events(&self) -> &[EventAggregate] {
        &self.events
    }

    pub fn claims(&self) -> &[Claim] {
        &self.claims
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn documents(&self) -> &[DocumentAggregate] {
        &self.documents
    }

    pub fn content_references(&self) -> &[ContentReference] {
        &self.content_references
    }

    pub(crate) fn with_revision(&self, revision_id: RevisionId) -> Result<Self, StoreError> {
        let mut snapshot = self.clone();
        snapshot.world = World::restore(
            self.world.id(),
            self.world.name(),
            self.world.premise_md(),
            self.world.epoch_label(),
            revision_id,
            self.world.created_at_ms(),
            self.world.updated_at_ms(),
        )
        .map_err(|error| StoreError::InvalidAggregate(error.to_string()))?;
        Ok(snapshot)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonAggregate<T> {
    object: T,
    references: Vec<ContentReference>,
}

impl<T> CanonAggregate<T> {
    pub fn new(object: T, references: Vec<ContentReference>) -> Self {
        Self { object, references }
    }

    pub fn object(&self) -> &T {
        &self.object
    }

    pub fn references(&self) -> &[ContentReference] {
        &self.references
    }

    pub fn into_parts(self) -> (T, Vec<ContentReference>) {
        (self.object, self.references)
    }
}

impl WorldStore {
    pub fn create(path: &Path, world: &World) -> Result<Self, StoreError> {
        validate_extension(path)?;

        let reserved = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| map_create_error(path, error))?;
        drop(reserved);

        let result = (|| {
            let mut connection =
                Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                    .map_err(|error| map_database_error(path, error))?;
            enable_foreign_keys(path, &connection)?;
            initialize(path, &mut connection, world)?;
            let mut store = Self {
                connection,
                path: path.to_owned(),
                world_id: world.id(),
                #[cfg(test)]
                fail_next_derived_index_update: false,
                #[cfg(test)]
                fail_semantic_search: false,
            };
            store.initialize_variants(world.created_at_ms())?;
            Ok(store)
        })();

        if result.is_err() {
            let _ = fs::remove_file(path);
        }

        result
    }

    pub fn open(path: &Path) -> Result<Self, StoreError> {
        validate_extension(path)?;
        if !path
            .try_exists()
            .map_err(|error| StoreError::Path(path.to_owned(), error))?
        {
            return Err(StoreError::NotFound(path.to_owned()));
        }

        let mut connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|error| map_database_error(path, error))?;
        enable_foreign_keys(path, &connection)?;
        migrate(path, &mut connection)?;
        verify_schema(path, &connection)?;
        let world_id = load_world_id(path, &connection)?;

        let mut store = Self {
            connection,
            path: path.to_owned(),
            world_id,
            #[cfg(test)]
            fail_next_derived_index_update: false,
            #[cfg(test)]
            fail_semantic_search: false,
        };
        store.initialize_variants(current_time_ms())?;
        Ok(store)
    }

    pub fn load_world(&self) -> Result<World, StoreError> {
        let world_count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM worlds", [], |row| row.get(0))
            .map_err(|error| map_schema_error(&self.path, error))?;
        if world_count != 1 {
            return Err(StoreError::InvalidFormat(self.path.clone()));
        }

        let values = self
            .connection
            .query_row(
                "SELECT id, name, premise_md, epoch_label, current_revision,
                        created_at_ms, updated_at_ms
                 FROM worlds",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?;

        let world_id = WorldId::from_str(&values.0)
            .map_err(|_| StoreError::InvalidFormat(self.path.clone()))?;
        let revision_id = RevisionId::from_str(&values.4)
            .map_err(|_| StoreError::InvalidFormat(self.path.clone()))?;

        World::restore(
            world_id,
            values.1,
            values.2,
            values.3,
            revision_id,
            values.5,
            values.6,
        )
        .map_err(|_| StoreError::InvalidFormat(self.path.clone()))
    }

    pub fn read_canon_snapshot(&self) -> Result<CanonSnapshot, StoreError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        let snapshot = CanonSnapshot {
            world: self.load_world()?,
            schema_version: SCHEMA_VERSION,
            entities: self.list_entities()?,
            relations: self.list_relations()?,
            goals: self.list_goals()?,
            events: self.list_events()?,
            claims: self.list_claims()?,
            rules: self.list_rules()?,
            documents: self.list_documents()?,
            content_references: crate::content::load_all(
                &self.connection,
                &self.path,
                self.world_id,
            )?,
        };
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        Ok(snapshot)
    }
}

pub(crate) fn read_canon_snapshot_from_connection(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
) -> Result<CanonSnapshot, StoreError> {
    let values = connection
        .query_row(
            "SELECT id, name, premise_md, epoch_label, current_revision,
                    created_at_ms, updated_at_ms FROM worlds",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .map_err(|error| map_schema_error(path, error))?;
    let world = World::restore(
        values
            .0
            .parse()
            .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?,
        values.1,
        values.2,
        values.3,
        values
            .4
            .parse()
            .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?,
        values.5,
        values.6,
    )
    .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    let ids = |table: &str| -> Result<Vec<String>, StoreError> {
        let sql = format!("SELECT id FROM {table} ORDER BY id");
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| map_schema_error(path, error))?;
        statement
            .query_map([], |row| row.get(0))
            .map_err(|error| map_schema_error(path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(path, error))
    };
    macro_rules! load_values {
        ($table:literal, $id:ty, $loader:path) => {{
            ids($table)?
                .into_iter()
                .map(|id| {
                    let id: $id = id
                        .parse()
                        .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
                    $loader(connection, path, id)?.ok_or(StoreError::InvalidFormat(path.to_owned()))
                })
                .collect::<Result<Vec<_>, StoreError>>()?
        }};
    }
    Ok(CanonSnapshot {
        world,
        schema_version: SCHEMA_VERSION,
        entities: load_values!(
            "entities",
            nirmata_core::EntityId,
            crate::entity::load_entity
        ),
        relations: load_values!(
            "relations",
            nirmata_core::RelationId,
            crate::relation::load_relation
        ),
        goals: load_values!("goals", nirmata_core::GoalId, crate::goal::load_goal),
        events: load_values!("events", nirmata_core::EventId, crate::event::load_event),
        claims: load_values!("claims", nirmata_core::ClaimId, crate::claim::load_claim),
        rules: load_values!("rules", nirmata_core::RuleId, crate::rule::load_rule),
        documents: load_values!(
            "documents",
            nirmata_core::DocumentId,
            crate::document::load_document
        ),
        content_references: crate::content::load_all(connection, path, world_id)?,
    })
}

fn current_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| i64::try_from(value.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn load_world_id(path: &Path, connection: &Connection) -> Result<WorldId, StoreError> {
    let id = connection
        .query_row("SELECT id FROM worlds", [], |row| row.get::<_, String>(0))
        .map_err(|error| map_schema_error(path, error))?;
    id.parse()
        .map_err(|_| StoreError::InvalidFormat(path.to_owned()))
}

pub(crate) fn ensure_world(store: &WorldStore, world_id: WorldId) -> Result<(), StoreError> {
    if store.world_id != world_id {
        return Err(StoreError::WrongWorld {
            expected: store.world_id,
            found: world_id,
        });
    }
    Ok(())
}

pub(crate) fn stored_version(version: u64) -> Result<i64, StoreError> {
    i64::try_from(version).map_err(|_| StoreError::VersionOutOfRange(version))
}

pub(crate) fn expected_version(version: u64) -> Result<i64, StoreError> {
    if version >= i64::MAX as u64 {
        return Err(StoreError::VersionOutOfRange(version));
    }
    stored_version(version)
}

pub(crate) fn invalid_data(
    index: usize,
    error: impl Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
}

pub(crate) fn invalid_domain(index: usize, error: DomainError) -> rusqlite::Error {
    invalid_data(index, error)
}

pub(crate) fn invalid_value(index: usize, value: &str) -> rusqlite::Error {
    invalid_data(
        index,
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid stored value {value}"),
        ),
    )
}

pub(crate) fn update_conflict(
    connection: &Connection,
    path: &Path,
    object: &'static str,
    exists_sql: &str,
    id: String,
    expected: u64,
) -> Result<StoreError, StoreError> {
    let exists = connection
        .query_row(exists_sql, [&id], |row| row.get::<_, bool>(0))
        .map_err(|error| map_schema_error(path, error))?;
    Ok(if exists {
        StoreError::StaleVersion {
            object,
            id,
            expected,
        }
    } else {
        StoreError::ObjectNotFound { object, id }
    })
}

fn validate_extension(path: &Path) -> Result<(), StoreError> {
    let valid = path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("nirmata"));
    if !valid {
        return Err(StoreError::InvalidExtension(path.to_owned()));
    }
    Ok(())
}

fn enable_foreign_keys(path: &Path, connection: &Connection) -> Result<(), StoreError> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|error| map_database_error(path, error))?;
    let enabled: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(|error| map_database_error(path, error))?;
    if enabled != 1 {
        return Err(StoreError::Database(
            path.to_owned(),
            "SQLite foreign keys could not be enabled".to_owned(),
        ));
    }
    Ok(())
}

fn initialize(path: &Path, connection: &mut Connection, world: &World) -> Result<(), StoreError> {
    let transaction = connection
        .transaction()
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(INITIAL_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (?1, ?2)",
            params![1, world.created_at_ms()],
        )
        .map_err(|error| map_database_error(path, error))?;
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
        .map_err(|error| map_database_error(path, error))?;
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
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(CANON_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (?1, ?2)",
            params![2, world.created_at_ms()],
        )
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(CHANGE_SET_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(REVISION_COMPLETION_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    install_text_search(&transaction, path, world.id())?;
    transaction
        .execute_batch(UNDO_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(LORE_IMPORT_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(VARIANT_SCHEMA)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at_ms) VALUES (?1, ?2)",
            params![SCHEMA_VERSION, world.created_at_ms()],
        )
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .commit()
        .map_err(|error| map_database_error(path, error))
}

fn migrate(path: &Path, connection: &mut Connection) -> Result<(), StoreError> {
    verify_database(path, connection)?;
    let version = schema_version(path, connection)?;
    if version > SCHEMA_VERSION {
        return Err(StoreError::IncompatibleSchema {
            path: path.to_owned(),
            found: version,
            supported: SCHEMA_VERSION,
        });
    }
    verify_schema_version(path, connection, version)?;

    match version {
        SCHEMA_VERSION => Ok(()),
        1 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(CANON_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (2, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(CHANGE_SET_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(REVISION_COMPLETION_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            let world_id = load_world_id(path, &transaction)?;
            install_text_search(&transaction, path, world_id)?;
            transaction
                .execute_batch(UNDO_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(LORE_IMPORT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        2 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(CHANGE_SET_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(REVISION_COMPLETION_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            let world_id = load_world_id(path, &transaction)?;
            install_text_search(&transaction, path, world_id)?;
            transaction
                .execute_batch(UNDO_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(LORE_IMPORT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        3 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "ALTER TABLE decision_points
                     ADD COLUMN replacement_target_ref TEXT",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "ALTER TABLE decision_points
                     ADD COLUMN reason TEXT",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "ALTER TABLE decision_points
                     ADD COLUMN resolved_alternative TEXT",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            let world_id = load_world_id(path, &transaction)?;
            install_text_search(&transaction, path, world_id)?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (4, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", 4)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (5, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", 5)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(UNDO_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(LORE_IMPORT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        4 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            let world_id = load_world_id(path, &transaction)?;
            install_text_search(&transaction, path, world_id)?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (5, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", 5)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(UNDO_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(LORE_IMPORT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        5 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(UNDO_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(LORE_IMPORT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        6 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(LORE_IMPORT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        7 => {
            let transaction = connection
                .transaction()
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute_batch(VARIANT_SCHEMA)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at_ms)
                     VALUES (?1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| map_database_error(path, error))?;
            transaction
                .commit()
                .map_err(|error| map_database_error(path, error))
        }
        _ => Err(StoreError::InvalidFormat(path.to_owned())),
    }
}

fn verify_schema(path: &Path, connection: &Connection) -> Result<(), StoreError> {
    verify_database(path, connection)?;
    verify_schema_version(path, connection, SCHEMA_VERSION)
}

fn verify_database(path: &Path, connection: &Connection) -> Result<(), StoreError> {
    let quick_check: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|error| map_database_error(path, error))?;
    if quick_check != "ok" {
        return Err(StoreError::Corrupt(path.to_owned(), quick_check));
    }
    Ok(())
}

fn schema_version(path: &Path, connection: &Connection) -> Result<i64, StoreError> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| map_database_error(path, error))
}

fn verify_schema_version(
    path: &Path,
    connection: &Connection,
    expected_version: i64,
) -> Result<(), StoreError> {
    let version = schema_version(path, connection)?;
    if version != expected_version {
        return Err(StoreError::InvalidFormat(path.to_owned()));
    }

    let recorded_version: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| map_schema_error(path, error))?;
    if recorded_version != Some(version) {
        return Err(StoreError::InvalidFormat(path.to_owned()));
    }

    let required_table_count: i64 = connection
        .query_row(
            if version == 1 {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN ('schema_migrations', 'worlds', 'revisions')"
            } else if version == 2 {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'schema_migrations', 'worlds', 'revisions', 'rules', 'entities',
                       'entity_aliases', 'relations', 'events', 'event_participants',
                       'event_links', 'event_goals', 'goals', 'claims', 'documents',
                       'content_references'
                   )"
            } else if version == 5 {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'schema_migrations', 'worlds', 'revisions', 'rules', 'entities',
                       'entity_aliases', 'relations', 'events', 'event_participants',
                       'event_links', 'event_goals', 'goals', 'claims', 'documents',
                       'content_references', 'change_sets', 'change_operations',
                       'decision_points', 'change_set_waivers', 'change_operation_audits',
                       'canon_fts'
                   )"
            } else if version == 6 {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'schema_migrations', 'worlds', 'revisions', 'rules', 'entities',
                       'entity_aliases', 'relations', 'events', 'event_participants',
                       'event_links', 'event_goals', 'goals', 'claims', 'documents',
                       'content_references', 'change_sets', 'change_operations',
                       'decision_points', 'change_set_waivers', 'change_operation_audits',
                       'canon_fts', 'revision_undos'
                   )"
            } else if version == 7 {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'schema_migrations', 'worlds', 'revisions', 'rules', 'entities',
                       'entity_aliases', 'relations', 'events', 'event_participants',
                       'event_links', 'event_goals', 'goals', 'claims', 'documents',
                       'content_references', 'change_sets', 'change_operations',
                       'decision_points', 'change_set_waivers', 'change_operation_audits',
                       'canon_fts', 'revision_undos', 'import_batches', 'import_sources',
                       'import_chunks', 'import_candidates'
                   )"
            } else if version == 8 {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'schema_migrations', 'worlds', 'revisions', 'rules', 'entities',
                       'entity_aliases', 'relations', 'events', 'event_participants',
                       'event_links', 'event_goals', 'goals', 'claims', 'documents',
                       'content_references', 'change_sets', 'change_operations',
                       'decision_points', 'change_set_waivers', 'change_operation_audits',
                       'canon_fts', 'revision_undos', 'import_batches', 'import_sources',
                       'import_chunks', 'import_candidates', 'variants', 'revision_snapshots'
                   )"
            } else {
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table'
                   AND name IN (
                       'schema_migrations', 'worlds', 'revisions', 'rules', 'entities',
                       'entity_aliases', 'relations', 'events', 'event_participants',
                       'event_links', 'event_goals', 'goals', 'claims', 'documents',
                       'content_references', 'change_sets', 'change_operations',
                       'decision_points', 'change_set_waivers', 'change_operation_audits'
                   )"
            },
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_schema_error(path, error))?;
    let expected_table_count = match version {
        1 => 3,
        2 => 15,
        3 | 4 => 20,
        5 => 21,
        6 => 22,
        7 => 26,
        8 => 28,
        _ => return Err(StoreError::InvalidFormat(path.to_owned())),
    };
    if required_table_count != expected_table_count {
        return Err(StoreError::InvalidFormat(path.to_owned()));
    }

    let world_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM worlds", [], |row| row.get(0))
        .map_err(|error| map_schema_error(path, error))?;
    let revision_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM revisions", [], |row| row.get(0))
        .map_err(|error| map_schema_error(path, error))?;
    if world_count != 1 || revision_count < 1 {
        return Err(StoreError::InvalidFormat(path.to_owned()));
    }

    Ok(())
}

fn map_create_error(path: &Path, error: io::Error) -> StoreError {
    if error.kind() == io::ErrorKind::AlreadyExists {
        StoreError::AlreadyExists(path.to_owned())
    } else {
        StoreError::Path(path.to_owned(), error)
    }
}

fn install_text_search(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
) -> Result<(), StoreError> {
    search::create_text_search_schema(connection, path)?;
    search::rebuild_canon_text_index(connection, path, world_id)
}

pub(crate) fn map_schema_error(path: &Path, error: rusqlite::Error) -> StoreError {
    match map_database_error(path, error) {
        StoreError::Locked(path) => StoreError::Locked(path),
        StoreError::Corrupt(path, details) => StoreError::Corrupt(path, details),
        StoreError::InvalidFormat(path) => StoreError::InvalidFormat(path),
        _ => StoreError::InvalidFormat(path.to_owned()),
    }
}

pub(crate) fn map_database_error(path: &Path, error: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(code, details) = &error {
        return match code.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => {
                StoreError::Locked(path.to_owned())
            }
            ErrorCode::DatabaseCorrupt => StoreError::Corrupt(
                path.to_owned(),
                details.clone().unwrap_or_else(|| error.to_string()),
            ),
            ErrorCode::NotADatabase => StoreError::InvalidFormat(path.to_owned()),
            _ => StoreError::Database(path.to_owned(), error.to_string()),
        };
    }
    StoreError::Database(path.to_owned(), error.to_string())
}

#[derive(Debug)]
pub enum StoreError {
    InvalidExtension(PathBuf),
    AlreadyExists(PathBuf),
    NotFound(PathBuf),
    InvalidFormat(PathBuf),
    IncompatibleSchema {
        path: PathBuf,
        found: i64,
        supported: i64,
    },
    Locked(PathBuf),
    Corrupt(PathBuf, String),
    WrongWorld {
        expected: WorldId,
        found: WorldId,
    },
    InvalidObjectUri(String),
    ObjectNotFound {
        object: &'static str,
        id: String,
    },
    StaleVersion {
        object: &'static str,
        id: String,
        expected: u64,
    },
    InvalidAggregate(String),
    InvalidChangeSet(String),
    StaleRevision {
        expected_current: RevisionId,
        found_base: RevisionId,
    },
    InvalidVariant(String),
    InvalidReadScope {
        variant_id: VariantId,
        revision_id: RevisionId,
    },
    VersionOutOfRange(u64),
    Path(PathBuf, io::Error),
    Database(PathBuf, String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExtension(path) => {
                write!(
                    formatter,
                    "{} must use the .nirmata extension",
                    path.display()
                )
            }
            Self::AlreadyExists(path) => write!(formatter, "{} already exists", path.display()),
            Self::NotFound(path) => write!(formatter, "{} does not exist", path.display()),
            Self::InvalidFormat(path) => {
                write!(
                    formatter,
                    "{} is not a valid Nirmata project",
                    path.display()
                )
            }
            Self::IncompatibleSchema {
                path,
                found,
                supported,
            } => write!(
                formatter,
                "{} uses schema version {found}; this build supports up to {supported}",
                path.display()
            ),
            Self::Locked(path) => {
                write!(formatter, "{} is locked by another process", path.display())
            }
            Self::Corrupt(path, details) => {
                write!(formatter, "{} is corrupt: {details}", path.display())
            }
            Self::WrongWorld { expected, found } => {
                write!(
                    formatter,
                    "object belongs to world {found}, expected {expected}"
                )
            }
            Self::InvalidObjectUri(uri) => write!(formatter, "invalid nirmata URI {uri}"),
            Self::ObjectNotFound { object, id } => write!(formatter, "{object} {id} was not found"),
            Self::StaleVersion {
                object,
                id,
                expected,
            } => write!(
                formatter,
                "{object} {id} no longer has expected version {expected}"
            ),
            Self::InvalidAggregate(details) => write!(formatter, "invalid aggregate: {details}"),
            Self::InvalidChangeSet(details) => write!(formatter, "invalid change set: {details}"),
            Self::StaleRevision {
                expected_current,
                found_base,
            } => write!(
                formatter,
                "base revision {found_base} is stale; current head is {expected_current}"
            ),
            Self::InvalidVariant(details) => write!(formatter, "invalid variant: {details}"),
            Self::InvalidReadScope {
                variant_id,
                revision_id,
            } => write!(
                formatter,
                "revision {revision_id} is not in variant {variant_id} history"
            ),
            Self::VersionOutOfRange(version) => {
                write!(formatter, "version {version} cannot be stored in SQLite")
            }
            Self::Path(path, error) => write!(formatter, "{}: {error}", path.display()),
            Self::Database(path, error) => write!(formatter, "{}: {error}", path.display()),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Path(_, error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/world_store/mod.rs"]
mod tests;
