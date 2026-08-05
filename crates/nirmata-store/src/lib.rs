mod change_set;
mod claim;
mod content;
mod document;
mod entity;
mod event;
mod goal;
mod relation;
mod rule;
mod search;

pub use change_set::{
    AffectedChangeSetGraph, ChangeOperationValue, ChangeSetDraftRecord, ChangeSetWaiver,
    CommittedChangeSetRecord, OperationAudit, OperationDecision, StoredRevision,
};
pub use event::EventAggregate;
pub use search::{
    AnchorContextBundle, AnchorContextEntry, AnchorContextQuery, LogicalVfsDirectory,
    LogicalVfsNode, LogicalVfsObject, ResolvedObject, StructuredSearchHit, StructuredSearchKind,
    StructuredSearchQuery, StructuredSearchStage, StructuredSearchTemporal,
};

pub use nirmata_core::document::DocumentAggregate;

use nirmata_core::{DomainError, RevisionId, World, WorldId, document::ContentReference};
use rusqlite::{Connection, ErrorCode, OpenFlags, params, types::Type};
use serde::Serialize;
use std::{
    error::Error,
    ffi::OsStr,
    fmt, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

const SCHEMA_VERSION: i64 = 6;

const INITIAL_SCHEMA: &str = "
    CREATE TABLE schema_migrations (
        version INTEGER PRIMARY KEY,
        applied_at_ms INTEGER NOT NULL
    );
    CREATE TABLE worlds (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        premise_md TEXT NOT NULL,
        epoch_label TEXT NOT NULL,
        current_revision TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        FOREIGN KEY (current_revision) REFERENCES revisions(id)
            DEFERRABLE INITIALLY DEFERRED
    );
    CREATE TABLE revisions (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        parent_revision_id TEXT,
        created_at_ms INTEGER NOT NULL,
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
            DEFERRABLE INITIALLY DEFERRED,
        FOREIGN KEY (parent_revision_id) REFERENCES revisions(id)
    );
";

const CANON_SCHEMA: &str = "
    CREATE TABLE rules (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('constitutive', 'generative', 'institutional', 'authorial')
        ),
        statement_md TEXT NOT NULL,
        scope TEXT NOT NULL,
        severity TEXT NOT NULL CHECK (severity IN ('advisory', 'hard')),
        source TEXT,
        validator_kind TEXT CHECK (
            validator_kind IS NULL OR validator_kind = 'no_resurrection'
        ),
        parameters_json TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        CHECK (severity <> 'hard' OR validator_kind IS NOT NULL),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
    );

    CREATE TABLE entities (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('person', 'place', 'faction', 'culture', 'resource', 'concept')
        ),
        name TEXT NOT NULL CHECK (length(trim(name)) > 0),
        slug TEXT NOT NULL CHECK (length(trim(slug)) > 0),
        summary TEXT NOT NULL,
        body_md TEXT NOT NULL,
        attributes_json TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        UNIQUE (world_id, slug),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
    );

    CREATE TABLE entity_aliases (
        world_id TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        alias TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(alias)) > 0),
        UNIQUE (entity_id, alias),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, entity_id) REFERENCES entities(world_id, id)
            ON DELETE CASCADE
    );

    CREATE TABLE relations (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        source_entity_id TEXT NOT NULL,
        target_entity_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (length(trim(kind)) > 0),
        direction TEXT NOT NULL CHECK (direction IN ('directed', 'undirected')),
        valid_from_tick INTEGER,
        valid_to_tick INTEGER,
        certainty TEXT NOT NULL CHECK (
            certainty IN ('certain', 'approximate', 'uncertain', 'approximate_uncertain')
        ),
        source_reference TEXT,
        metadata_json TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        UNIQUE (world_id, id),
        CHECK (
            valid_from_tick IS NULL
            OR valid_to_tick IS NULL
            OR valid_from_tick <= valid_to_tick
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, source_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, target_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE goals (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        holder_entity_id TEXT NOT NULL,
        desired_state_md TEXT NOT NULL CHECK (length(trim(desired_state_md)) > 0),
        priority INTEGER NOT NULL,
        status TEXT NOT NULL CHECK (
            status IN ('active', 'achieved', 'abandoned', 'frustrated')
        ),
        valid_from_tick INTEGER,
        valid_to_tick INTEGER,
        visibility TEXT NOT NULL CHECK (visibility IN ('public', 'secret')),
        source TEXT,
        version INTEGER NOT NULL CHECK (version > 0),
        UNIQUE (world_id, id),
        CHECK (
            valid_from_tick IS NULL
            OR valid_to_tick IS NULL
            OR valid_from_tick <= valid_to_tick
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, holder_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE events (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (length(trim(kind)) > 0),
        summary TEXT NOT NULL,
        body_md TEXT NOT NULL,
        time_kind TEXT NOT NULL CHECK (
            time_kind IN ('unknown', 'instant', 'interval', 'ongoing')
        ),
        start_tick INTEGER,
        end_tick INTEGER,
        time_precision TEXT NOT NULL CHECK (
            time_precision IN ('exact', 'day', 'month', 'year', 'era', 'unknown')
        ),
        certainty TEXT NOT NULL CHECK (
            certainty IN ('certain', 'approximate', 'uncertain', 'approximate_uncertain')
        ),
        location_entity_id TEXT,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        CHECK (
            (time_kind = 'unknown' AND start_tick IS NULL AND end_tick IS NULL)
            OR (time_kind = 'instant' AND start_tick IS NOT NULL AND end_tick IS NULL)
            OR (
                time_kind = 'interval'
                AND start_tick IS NOT NULL
                AND end_tick IS NOT NULL
                AND start_tick <= end_tick
            )
            OR (time_kind = 'ongoing' AND start_tick IS NOT NULL AND end_tick IS NULL)
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, location_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE event_participants (
        world_id TEXT NOT NULL,
        event_id TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        role TEXT NOT NULL CHECK (length(trim(role)) > 0),
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        UNIQUE (event_id, ordinal),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE,
        FOREIGN KEY (world_id, entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE event_links (
        world_id TEXT NOT NULL,
        source_event_id TEXT NOT NULL,
        target_event_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (
            kind IN ('enables', 'causes', 'motivates', 'prevents', 'terminates', 'reveals')
        ),
        UNIQUE (source_event_id, target_event_id, kind),
        CHECK (source_event_id <> target_event_id),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, source_event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE,
        FOREIGN KEY (world_id, target_event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE
    );

    CREATE TABLE event_goals (
        world_id TEXT NOT NULL,
        event_id TEXT NOT NULL,
        goal_id TEXT NOT NULL,
        UNIQUE (event_id, goal_id),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, event_id) REFERENCES events(world_id, id)
            ON DELETE CASCADE,
        FOREIGN KEY (world_id, goal_id) REFERENCES goals(world_id, id)
    );

    CREATE TABLE documents (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        title TEXT NOT NULL,
        kind TEXT NOT NULL,
        author_entity_id TEXT,
        perspective_entity_id TEXT,
        canon_status TEXT NOT NULL CHECK (
            canon_status IN ('canonical', 'non_canonical')
        ),
        body_md TEXT NOT NULL,
        version INTEGER NOT NULL CHECK (version > 0),
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        UNIQUE (world_id, id),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, author_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, perspective_entity_id) REFERENCES entities(world_id, id)
    );

    CREATE TABLE claims (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        subject_entity_id TEXT NOT NULL,
        content_md TEXT NOT NULL,
        predicate_key TEXT,
        object_kind TEXT CHECK (object_kind IS NULL OR object_kind IN ('entity', 'scalar')),
        object_entity_id TEXT,
        object_scalar TEXT,
        polarity TEXT NOT NULL CHECK (polarity IN ('positive', 'negative')),
        authentication TEXT NOT NULL CHECK (
            authentication IN ('canonical', 'attributed', 'disputed')
        ),
        holder_entity_id TEXT,
        modality TEXT CHECK (
            modality IS NULL
            OR modality IN ('assertion', 'belief', 'hypothesis', 'counterfactual')
        ),
        register TEXT,
        epistemic_basis TEXT,
        source TEXT,
        source_document_id TEXT,
        source_claim_id TEXT,
        holder_confidence REAL CHECK (
            holder_confidence IS NULL
            OR (holder_confidence >= 0.0 AND holder_confidence <= 1.0)
        ),
        valid_from_tick INTEGER,
        valid_to_tick INTEGER,
        registered_revision_id TEXT NOT NULL,
        superseded_revision_id TEXT,
        version INTEGER NOT NULL CHECK (version > 0),
        UNIQUE (world_id, id),
        CHECK (
            (predicate_key IS NULL AND object_kind IS NULL)
            OR (predicate_key IS NOT NULL AND length(trim(predicate_key)) > 0
                AND object_kind IS NOT NULL)
        ),
        CHECK (
            (object_kind IS NULL AND object_entity_id IS NULL AND object_scalar IS NULL)
            OR (object_kind = 'entity' AND object_entity_id IS NOT NULL
                AND object_scalar IS NULL)
            OR (object_kind = 'scalar' AND object_entity_id IS NULL
                AND object_scalar IS NOT NULL)
        ),
        CHECK (
            authentication <> 'canonical'
            OR (holder_entity_id IS NULL AND modality IS NULL)
        ),
        CHECK (
            authentication <> 'attributed'
            OR (holder_entity_id IS NOT NULL AND modality IS NOT NULL)
        ),
        CHECK (source_claim_id IS NULL OR source_claim_id <> id),
        CHECK (
            valid_from_tick IS NULL
            OR valid_to_tick IS NULL
            OR valid_from_tick <= valid_to_tick
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (world_id, subject_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, object_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, holder_entity_id) REFERENCES entities(world_id, id),
        FOREIGN KEY (world_id, source_document_id) REFERENCES documents(world_id, id),
        FOREIGN KEY (world_id, source_claim_id) REFERENCES claims(world_id, id),
        FOREIGN KEY (registered_revision_id) REFERENCES revisions(id),
        FOREIGN KEY (superseded_revision_id) REFERENCES revisions(id)
    );

    CREATE TABLE content_references (
        world_id TEXT NOT NULL,
        source_type TEXT NOT NULL CHECK (
            source_type IN ('entity', 'relation', 'event', 'claim', 'rule', 'goal', 'document')
        ),
        source_id TEXT NOT NULL,
        target_type TEXT NOT NULL CHECK (
            target_type IN ('entity', 'relation', 'event', 'claim', 'rule', 'goal', 'document')
        ),
        target_id TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        UNIQUE (world_id, source_type, source_id, ordinal),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE
    );

    CREATE UNIQUE INDEX relations_exact_unique ON relations (
        world_id,
        kind,
        direction,
        CASE
            WHEN direction = 'undirected' AND source_entity_id > target_entity_id
                THEN target_entity_id
            ELSE source_entity_id
        END,
        CASE
            WHEN direction = 'undirected' AND source_entity_id > target_entity_id
                THEN source_entity_id
            ELSE target_entity_id
        END,
        valid_from_tick IS NULL,
        ifnull(valid_from_tick, 0),
        valid_to_tick IS NULL,
        ifnull(valid_to_tick, 0),
        certainty,
        source_reference IS NULL,
        ifnull(source_reference, ''),
        metadata_json
    );

    CREATE INDEX rules_world_id ON rules (world_id);
    CREATE INDEX entities_world_id ON entities (world_id);
    CREATE INDEX entity_aliases_world_id ON entity_aliases (world_id, entity_id);
    CREATE INDEX relations_world_id ON relations (world_id);
    CREATE INDEX relations_source_id ON relations (world_id, source_entity_id);
    CREATE INDEX relations_target_id ON relations (world_id, target_entity_id);
    CREATE INDEX relations_time ON relations (world_id, valid_from_tick, valid_to_tick);
    CREATE INDEX goals_world_id ON goals (world_id);
    CREATE INDEX goals_holder_id ON goals (world_id, holder_entity_id);
    CREATE INDEX goals_time ON goals (world_id, valid_from_tick, valid_to_tick);
    CREATE INDEX events_world_id ON events (world_id);
    CREATE INDEX events_location_id ON events (world_id, location_entity_id);
    CREATE INDEX events_time ON events (world_id, start_tick, end_tick);
    CREATE INDEX event_participants_world_id ON event_participants (world_id, event_id);
    CREATE INDEX event_participants_entity_id ON event_participants (world_id, entity_id);
    CREATE INDEX event_links_world_id ON event_links (world_id, source_event_id);
    CREATE INDEX event_links_target_id ON event_links (world_id, target_event_id);
    CREATE INDEX event_goals_world_id ON event_goals (world_id, event_id);
    CREATE INDEX event_goals_goal_id ON event_goals (world_id, goal_id);
    CREATE INDEX documents_world_id ON documents (world_id);
    CREATE INDEX documents_author_id ON documents (world_id, author_entity_id);
    CREATE INDEX documents_perspective_id ON documents (world_id, perspective_entity_id);
    CREATE INDEX claims_world_id ON claims (world_id);
    CREATE INDEX claims_subject_id ON claims (world_id, subject_entity_id);
    CREATE INDEX claims_object_id ON claims (world_id, object_entity_id);
    CREATE INDEX claims_holder_id ON claims (world_id, holder_entity_id);
    CREATE INDEX claims_source_document_id ON claims (world_id, source_document_id);
    CREATE INDEX claims_source_claim_id ON claims (world_id, source_claim_id);
    CREATE INDEX claims_registered_revision_id ON claims (registered_revision_id);
    CREATE INDEX claims_superseded_revision_id ON claims (superseded_revision_id);
    CREATE INDEX claims_time ON claims (world_id, valid_from_tick, valid_to_tick);
    CREATE INDEX content_references_source_id
        ON content_references (world_id, source_type, source_id);
    CREATE INDEX content_references_target_id
        ON content_references (world_id, target_type, target_id);
";

const CHANGE_SET_SCHEMA: &str = "
    CREATE TABLE change_sets (
        id TEXT PRIMARY KEY,
        world_id TEXT NOT NULL,
        kind TEXT NOT NULL CHECK (kind IN ('draft', 'committed')),
        base_revision_id TEXT NOT NULL,
        result_revision_id TEXT,
        objective TEXT NOT NULL CHECK (length(trim(objective)) > 0),
        source_refs_json TEXT NOT NULL,
        assumptions_json TEXT NOT NULL,
        deterministic_report_json TEXT,
        created_at_ms INTEGER NOT NULL,
        updated_at_ms INTEGER NOT NULL,
        CHECK (
            (kind = 'draft' AND result_revision_id IS NULL)
            OR (kind = 'committed' AND result_revision_id IS NOT NULL)
        ),
        FOREIGN KEY (world_id) REFERENCES worlds(id) ON DELETE CASCADE,
        FOREIGN KEY (base_revision_id) REFERENCES revisions(id)
            DEFERRABLE INITIALLY DEFERRED,
        FOREIGN KEY (result_revision_id) REFERENCES revisions(id)
            DEFERRABLE INITIALLY DEFERRED,
        UNIQUE (result_revision_id)
    );

    CREATE TABLE change_operations (
        operation_id TEXT NOT NULL,
        change_set_id TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        kind TEXT NOT NULL,
        retcon TEXT NOT NULL CHECK (
            retcon IN ('additive', 'reinterpretive', 'replacement')
        ),
        expected_version INTEGER NOT NULL CHECK (expected_version >= 0),
        affected_refs_json TEXT NOT NULL,
        payload_json TEXT NOT NULL,
        PRIMARY KEY (change_set_id, operation_id),
        UNIQUE (change_set_id, ordinal),
        FOREIGN KEY (change_set_id) REFERENCES change_sets(id) ON DELETE CASCADE
    );

    CREATE TABLE decision_points (
        id TEXT PRIMARY KEY,
        change_set_id TEXT NOT NULL,
        ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4294967295),
        prompt TEXT NOT NULL CHECK (length(trim(prompt)) > 0),
        operation_ids_json TEXT NOT NULL,
        alternatives_json TEXT NOT NULL,
        replacement_target_ref TEXT,
        reason TEXT,
        resolved_alternative TEXT,
        UNIQUE (change_set_id, ordinal),
        FOREIGN KEY (change_set_id) REFERENCES change_sets(id) ON DELETE CASCADE
    );

    CREATE TABLE change_set_waivers (
        change_set_id TEXT NOT NULL,
        operation_id TEXT NOT NULL,
        issue_code TEXT NOT NULL CHECK (length(trim(issue_code)) > 0),
        rationale TEXT NOT NULL CHECK (length(trim(rationale)) > 0),
        created_at_ms INTEGER NOT NULL,
        PRIMARY KEY (change_set_id, operation_id, issue_code),
        FOREIGN KEY (change_set_id, operation_id)
            REFERENCES change_operations(change_set_id, operation_id)
            ON DELETE CASCADE
    );

    CREATE TABLE change_operation_audits (
        change_set_id TEXT NOT NULL,
        operation_id TEXT NOT NULL,
        decision TEXT NOT NULL CHECK (decision IN ('accept', 'edit', 'reject')),
        source TEXT NOT NULL CHECK (length(trim(source)) > 0),
        before_json TEXT,
        after_json TEXT,
        decided_at_ms INTEGER NOT NULL,
        PRIMARY KEY (change_set_id, operation_id),
        FOREIGN KEY (change_set_id, operation_id)
            REFERENCES change_operations(change_set_id, operation_id)
            ON DELETE CASCADE
    );

    CREATE INDEX change_sets_world_id ON change_sets (world_id, created_at_ms);
    CREATE INDEX change_sets_base_revision_id ON change_sets (base_revision_id);
    CREATE INDEX change_operations_change_set_id
        ON change_operations (change_set_id, ordinal);
    CREATE INDEX change_operations_operation_id ON change_operations (operation_id);
    CREATE INDEX decision_points_change_set_id
        ON decision_points (change_set_id, ordinal);
    CREATE INDEX change_set_waivers_change_set_id
        ON change_set_waivers (change_set_id, operation_id);
    CREATE INDEX change_operation_audits_change_set_id
        ON change_operation_audits (change_set_id, operation_id);
";

const REVISION_COMPLETION_SCHEMA: &str = "
    ALTER TABLE revisions ADD COLUMN author TEXT NOT NULL DEFAULT 'system'
        CHECK (length(trim(author)) > 0);
    ALTER TABLE revisions ADD COLUMN summary TEXT NOT NULL DEFAULT 'World created'
        CHECK (length(trim(summary)) > 0);
    ALTER TABLE revisions ADD COLUMN change_set_id TEXT;
    CREATE UNIQUE INDEX revisions_single_root ON revisions (world_id)
        WHERE parent_revision_id IS NULL;
    CREATE UNIQUE INDEX revisions_linear_parent ON revisions (parent_revision_id)
        WHERE parent_revision_id IS NOT NULL;
    CREATE UNIQUE INDEX revisions_change_set_id ON revisions (change_set_id)
        WHERE change_set_id IS NOT NULL;
    CREATE INDEX revisions_world_id ON revisions (world_id, created_at_ms);
";

const UNDO_SCHEMA: &str = "
    CREATE TABLE revision_undos (
        undo_revision_id TEXT PRIMARY KEY,
        undone_revision_id TEXT NOT NULL UNIQUE,
        FOREIGN KEY (undo_revision_id) REFERENCES revisions(id) ON DELETE CASCADE,
        FOREIGN KEY (undone_revision_id) REFERENCES revisions(id)
    );

    CREATE INDEX revision_undos_undone_revision_id
        ON revision_undos (undone_revision_id);
";

pub struct WorldStore {
    connection: Connection,
    path: PathBuf,
    world_id: WorldId,
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
            Ok(Self {
                connection,
                path: path.to_owned(),
                world_id: world.id(),
            })
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

        Ok(Self {
            connection,
            path: path.to_owned(),
            world_id,
        })
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
}

fn load_world_id(path: &Path, connection: &Connection) -> Result<WorldId, StoreError> {
    let id = connection
        .query_row("SELECT id FROM worlds", [], |row| row.get::<_, String>(0))
        .map_err(|error| map_schema_error(path, error))?;
    id.parse()
        .map_err(|_| StoreError::InvalidFormat(path.to_owned()))
}

fn ensure_world(store: &WorldStore, world_id: WorldId) -> Result<(), StoreError> {
    if store.world_id != world_id {
        return Err(StoreError::WrongWorld {
            expected: store.world_id,
            found: world_id,
        });
    }
    Ok(())
}

fn stored_version(version: u64) -> Result<i64, StoreError> {
    i64::try_from(version).map_err(|_| StoreError::VersionOutOfRange(version))
}

fn expected_version(version: u64) -> Result<i64, StoreError> {
    if version >= i64::MAX as u64 {
        return Err(StoreError::VersionOutOfRange(version));
    }
    stored_version(version)
}

fn invalid_data(index: usize, error: impl Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
}

fn invalid_domain(index: usize, error: DomainError) -> rusqlite::Error {
    invalid_data(index, error)
}

fn invalid_value(index: usize, value: &str) -> rusqlite::Error {
    invalid_data(
        index,
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid stored value {value}"),
        ),
    )
}

fn update_conflict(
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

fn map_schema_error(path: &Path, error: rusqlite::Error) -> StoreError {
    match map_database_error(path, error) {
        StoreError::Locked(path) => StoreError::Locked(path),
        StoreError::Corrupt(path, details) => StoreError::Corrupt(path, details),
        StoreError::InvalidFormat(path) => StoreError::InvalidFormat(path),
        _ => StoreError::InvalidFormat(path.to_owned()),
    }
}

fn map_database_error(path: &Path, error: rusqlite::Error) -> StoreError {
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
mod tests {
    use super::*;
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
}
