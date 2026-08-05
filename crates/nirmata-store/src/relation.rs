use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_domain,
    invalid_value, map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    EntityId, RelationId, WorldId,
    document::ObjectRef,
    relation::{Relation, RelationDirection},
    time::Certainty,
};
use rusqlite::{OptionalExtension, Row, params};
use std::str::FromStr;

impl WorldStore {
    pub fn insert_relation(&mut self, relation: &Relation) -> Result<(), StoreError> {
        ensure_world(self, relation.world_id())?;
        insert_relation_in_tx(&self.connection, &self.path, relation)?;
        Ok(())
    }

    pub fn get_relation(&self, id: RelationId) -> Result<Option<Relation>, StoreError> {
        load_relation(&self.connection, &self.path, id)
    }

    pub fn list_relations(&self) -> Result<Vec<Relation>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, source_entity_id, target_entity_id, kind, direction,
                        valid_from_tick, valid_to_tick, certainty, source_reference,
                        metadata_json, version
                 FROM relations ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([], relation_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn update_relation(&mut self, relation: &Relation) -> Result<Relation, StoreError> {
        ensure_world(self, relation.world_id())?;
        let id = relation.id().to_string();
        update_relation_in_tx(&self.connection, &self.path, relation)?;
        self.get_relation(relation.id())?
            .ok_or(StoreError::ObjectNotFound {
                object: "relation",
                id,
            })
    }
}

pub(crate) fn insert_relation_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    relation: &Relation,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO relations (
                id, world_id, source_entity_id, target_entity_id, kind, direction,
                valid_from_tick, valid_to_tick, certainty, source_reference,
                metadata_json, version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                relation.id().to_string(),
                relation.world_id().to_string(),
                relation.source_entity_id().to_string(),
                relation.target_entity_id().to_string(),
                relation.kind(),
                direction(relation.direction()),
                relation.valid_from_tick(),
                relation.valid_to_tick(),
                certainty(relation.certainty()),
                relation.source_reference(),
                relation.metadata_json().as_str(),
                stored_version(relation.version())?,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(crate) fn update_relation_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    relation: &Relation,
) -> Result<(), StoreError> {
    let expected = expected_version(relation.version())?;
    let id = relation.id().to_string();
    let changed = connection
        .execute(
            "UPDATE relations
             SET source_entity_id = ?1, target_entity_id = ?2, kind = ?3, direction = ?4,
                 valid_from_tick = ?5, valid_to_tick = ?6, certainty = ?7,
                 source_reference = ?8, metadata_json = ?9, version = version + 1
             WHERE id = ?10 AND world_id = ?11 AND version = ?12",
            params![
                relation.source_entity_id().to_string(),
                relation.target_entity_id().to_string(),
                relation.kind(),
                direction(relation.direction()),
                relation.valid_from_tick(),
                relation.valid_to_tick(),
                certainty(relation.certainty()),
                relation.source_reference(),
                relation.metadata_json().as_str(),
                id,
                relation.world_id().to_string(),
                expected,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "relation",
            "SELECT EXISTS(SELECT 1 FROM relations WHERE id = ?1)",
            id,
            relation.version(),
        )?);
    }
    Ok(())
}

pub(crate) fn delete_relation_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    world_id: WorldId,
    id: RelationId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = connection
        .execute(
            "DELETE FROM relations WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "relation",
            "SELECT EXISTS(SELECT 1 FROM relations WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    content::remove_object(connection, path, world_id, ObjectRef::Relation(id))?;
    Ok(())
}

pub(crate) fn load_relation(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    id: RelationId,
) -> Result<Option<Relation>, StoreError> {
    connection
        .query_row(
            "SELECT id, world_id, source_entity_id, target_entity_id, kind, direction,
                    valid_from_tick, valid_to_tick, certainty, source_reference,
                    metadata_json, version
             FROM relations WHERE id = ?1",
            [id.to_string()],
            relation_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))
}

fn relation_from_row(row: &Row<'_>) -> rusqlite::Result<Relation> {
    let id =
        RelationId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?;
    let world_id =
        WorldId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?;
    let source =
        EntityId::from_str(&row.get::<_, String>(2)?).map_err(|error| invalid_data(2, error))?;
    let target =
        EntityId::from_str(&row.get::<_, String>(3)?).map_err(|error| invalid_data(3, error))?;
    let direction = parse_direction(5, &row.get::<_, String>(5)?)?;
    let certainty = parse_certainty(8, &row.get::<_, String>(8)?)?;
    let version = u64::try_from(row.get::<_, i64>(11)?).map_err(|error| invalid_data(11, error))?;
    Relation::restore(
        id,
        world_id,
        source,
        target,
        row.get::<_, String>(4)?,
        direction,
        row.get(6)?,
        row.get(7)?,
        certainty,
        row.get(9)?,
        row.get::<_, String>(10)?,
        version,
    )
    .map_err(|error| invalid_domain(0, error))
}

fn direction(value: RelationDirection) -> &'static str {
    match value {
        RelationDirection::Directed => "directed",
        RelationDirection::Undirected => "undirected",
    }
}

fn parse_direction(index: usize, value: &str) -> rusqlite::Result<RelationDirection> {
    match value {
        "directed" => Ok(RelationDirection::Directed),
        "undirected" => Ok(RelationDirection::Undirected),
        _ => Err(invalid_value(index, value)),
    }
}

fn certainty(value: Certainty) -> &'static str {
    match value {
        Certainty::Certain => "certain",
        Certainty::Approximate => "approximate",
        Certainty::Uncertain => "uncertain",
        Certainty::ApproximateUncertain => "approximate_uncertain",
    }
}

fn parse_certainty(index: usize, value: &str) -> rusqlite::Result<Certainty> {
    match value {
        "certain" => Ok(Certainty::Certain),
        "approximate" => Ok(Certainty::Approximate),
        "uncertain" => Ok(Certainty::Uncertain),
        "approximate_uncertain" => Ok(Certainty::ApproximateUncertain),
        _ => Err(invalid_value(index, value)),
    }
}
