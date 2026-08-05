use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_value,
    map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    EntityId, WorldId,
    document::ObjectRef,
    entity::{Entity, EntityKind},
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use std::{path::Path, str::FromStr};

impl WorldStore {
    pub fn insert_entity(&mut self, entity: &Entity) -> Result<(), StoreError> {
        ensure_world(self, entity.world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        insert_entity_in_tx(&transaction, &self.path, entity)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_entity(&self, id: EntityId) -> Result<Option<Entity>, StoreError> {
        load_entity(&self.connection, &self.path, id)
    }

    pub fn list_entities(&self) -> Result<Vec<Entity>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, kind, name, slug, summary, body_md, attributes_json,
                        version, created_at_ms, updated_at_ms
                 FROM entities ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let rows = statement
            .query_map([], raw_entity_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        rows.into_iter()
            .map(|row| restore_entity(&self.connection, &self.path, row))
            .collect()
    }

    pub fn update_entity(&mut self, entity: &Entity) -> Result<Entity, StoreError> {
        ensure_world(self, entity.world_id())?;
        let id = entity.id().to_string();
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        update_entity_in_tx(&transaction, &self.path, entity)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_entity(entity.id())?
            .ok_or(StoreError::ObjectNotFound {
                object: "entity",
                id,
            })
    }
}

pub(crate) fn insert_entity_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    entity: &Entity,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO entities (
                id, world_id, kind, name, slug, summary, body_md, attributes_json,
                version, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                entity.id().to_string(),
                entity.world_id().to_string(),
                entity_kind(entity.kind()),
                entity.name(),
                entity.slug(),
                entity.summary(),
                entity.body_md(),
                entity.attributes_json().as_str(),
                stored_version(entity.version())?,
                entity.created_at_ms(),
                entity.updated_at_ms(),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    insert_aliases(transaction, entity).map_err(|error| map_database_error(path, error))?;
    crate::search::index_entity(transaction, path, entity)?;
    Ok(())
}

pub(crate) fn update_entity_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    entity: &Entity,
) -> Result<(), StoreError> {
    let expected = expected_version(entity.version())?;
    let id = entity.id().to_string();
    let changed = transaction
        .execute(
            "UPDATE entities
             SET kind = ?1, name = ?2, slug = ?3, summary = ?4, body_md = ?5,
                 attributes_json = ?6, version = version + 1, updated_at_ms = ?7
             WHERE id = ?8 AND world_id = ?9 AND version = ?10",
            params![
                entity_kind(entity.kind()),
                entity.name(),
                entity.slug(),
                entity.summary(),
                entity.body_md(),
                entity.attributes_json().as_str(),
                entity.updated_at_ms(),
                id,
                entity.world_id().to_string(),
                expected,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            transaction,
            path,
            "entity",
            "SELECT EXISTS(SELECT 1 FROM entities WHERE id = ?1)",
            id,
            entity.version(),
        )?);
    }
    transaction
        .execute(
            "DELETE FROM entity_aliases WHERE world_id = ?1 AND entity_id = ?2",
            params![entity.world_id().to_string(), entity.id().to_string()],
        )
        .map_err(|error| map_database_error(path, error))?;
    insert_aliases(transaction, entity).map_err(|error| map_database_error(path, error))?;
    crate::search::index_entity(transaction, path, entity)?;
    Ok(())
}

pub(crate) fn delete_entity_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    world_id: WorldId,
    id: EntityId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = transaction
        .execute(
            "DELETE FROM entities WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            transaction,
            path,
            "entity",
            "SELECT EXISTS(SELECT 1 FROM entities WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    crate::search::remove_text_index_row(transaction, path, world_id, ObjectRef::Entity(id))?;
    content::remove_object(transaction, path, world_id, ObjectRef::Entity(id))?;
    Ok(())
}

fn insert_aliases(transaction: &Transaction<'_>, entity: &Entity) -> rusqlite::Result<()> {
    for alias in entity.aliases() {
        transaction.execute(
            "INSERT INTO entity_aliases (world_id, entity_id, alias) VALUES (?1, ?2, ?3)",
            params![
                entity.world_id().to_string(),
                entity.id().to_string(),
                alias
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn load_entity(
    connection: &Connection,
    path: &Path,
    id: EntityId,
) -> Result<Option<Entity>, StoreError> {
    let row = connection
        .query_row(
            "SELECT id, world_id, kind, name, slug, summary, body_md, attributes_json,
                    version, created_at_ms, updated_at_ms
             FROM entities WHERE id = ?1",
            [id.to_string()],
            raw_entity_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))?;
    row.map(|row| restore_entity(connection, path, row))
        .transpose()
}

fn restore_entity(
    connection: &Connection,
    path: &Path,
    row: RawEntity,
) -> Result<Entity, StoreError> {
    let mut statement = connection
        .prepare("SELECT alias FROM entity_aliases WHERE entity_id = ?1 ORDER BY rowid")
        .map_err(|error| map_schema_error(path, error))?;
    let aliases = statement
        .query_map([row.id.to_string()], |row| row.get::<_, String>(0))
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))?;
    Entity::restore(
        row.id,
        row.world_id,
        row.kind,
        row.name,
        row.slug,
        row.summary,
        row.body_md,
        row.attributes_json,
        aliases,
        row.version,
        row.created_at_ms,
        row.updated_at_ms,
    )
    .map_err(|_| StoreError::InvalidFormat(path.to_owned()))
}

struct RawEntity {
    id: EntityId,
    world_id: WorldId,
    kind: EntityKind,
    name: String,
    slug: String,
    summary: String,
    body_md: String,
    attributes_json: String,
    version: u64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

fn raw_entity_from_row(row: &Row<'_>) -> rusqlite::Result<RawEntity> {
    Ok(RawEntity {
        id: EntityId::from_str(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_data(0, error))?,
        world_id: WorldId::from_str(&row.get::<_, String>(1)?)
            .map_err(|error| invalid_data(1, error))?,
        kind: parse_entity_kind(2, &row.get::<_, String>(2)?)?,
        name: row.get(3)?,
        slug: row.get(4)?,
        summary: row.get(5)?,
        body_md: row.get(6)?,
        attributes_json: row.get(7)?,
        version: u64::try_from(row.get::<_, i64>(8)?).map_err(|error| invalid_data(8, error))?,
        created_at_ms: row.get(9)?,
        updated_at_ms: row.get(10)?,
    })
}

fn entity_kind(value: EntityKind) -> &'static str {
    match value {
        EntityKind::Person => "person",
        EntityKind::Place => "place",
        EntityKind::Faction => "faction",
        EntityKind::Culture => "culture",
        EntityKind::Resource => "resource",
        EntityKind::Concept => "concept",
    }
}

fn parse_entity_kind(index: usize, value: &str) -> rusqlite::Result<EntityKind> {
    match value {
        "person" => Ok(EntityKind::Person),
        "place" => Ok(EntityKind::Place),
        "faction" => Ok(EntityKind::Faction),
        "culture" => Ok(EntityKind::Culture),
        "resource" => Ok(EntityKind::Resource),
        "concept" => Ok(EntityKind::Concept),
        _ => Err(invalid_value(index, value)),
    }
}
