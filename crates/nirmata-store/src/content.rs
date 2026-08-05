use crate::{StoreError, invalid_data, invalid_value, map_database_error, map_schema_error};
use nirmata_core::{
    ClaimId, DocumentId, EntityId, EventId, GoalId, RelationId, RuleId, WorldId,
    document::{ContentReference, ObjectRef},
};
use rusqlite::{Connection, Row, Transaction, params};
use std::{collections::HashSet, path::Path, str::FromStr};

pub(crate) fn validate(
    source: ObjectRef,
    references: &[ContentReference],
) -> Result<(), StoreError> {
    let mut ordinals = HashSet::with_capacity(references.len());
    for reference in references {
        if reference.source() != source {
            return Err(StoreError::InvalidAggregate(
                "every content reference must originate from its aggregate".to_owned(),
            ));
        }
        if !ordinals.insert(reference.ordinal()) {
            return Err(StoreError::InvalidAggregate(format!(
                "content reference ordinal {} is duplicated",
                reference.ordinal()
            )));
        }
    }
    Ok(())
}

pub(crate) fn insert(
    transaction: &Transaction<'_>,
    path: &Path,
    world_id: WorldId,
    references: &[ContentReference],
) -> Result<(), StoreError> {
    for reference in references {
        if !object_exists(transaction, path, world_id, reference.target())? {
            return Err(StoreError::InvalidAggregate(format!(
                "content reference target {} does not exist in this world",
                reference.target()
            )));
        }
        transaction
            .execute(
                "INSERT INTO content_references (
                    world_id, source_type, source_id, target_type, target_id, ordinal
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    world_id.to_string(),
                    reference.source().kind(),
                    object_id(reference.source()),
                    reference.target().kind(),
                    object_id(reference.target()),
                    i64::from(reference.ordinal()),
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    Ok(())
}

pub(crate) fn replace(
    transaction: &Transaction<'_>,
    path: &Path,
    world_id: WorldId,
    source: ObjectRef,
    references: &[ContentReference],
) -> Result<(), StoreError> {
    transaction
        .execute(
            "DELETE FROM content_references
             WHERE world_id = ?1 AND source_type = ?2 AND source_id = ?3",
            params![world_id.to_string(), source.kind(), object_id(source)],
        )
        .map_err(|error| map_database_error(path, error))?;
    insert(transaction, path, world_id, references)
}

pub(crate) fn remove_object(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
    object: ObjectRef,
) -> Result<(), StoreError> {
    connection
        .execute(
            "DELETE FROM content_references
             WHERE world_id = ?1
               AND (
                    (source_type = ?2 AND source_id = ?3)
                    OR (target_type = ?2 AND target_id = ?3)
               )",
            params![world_id.to_string(), object.kind(), object_id(object)],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(crate) fn load(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
    source: ObjectRef,
) -> Result<Vec<ContentReference>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT source_type, source_id, target_type, target_id, ordinal
             FROM content_references
             WHERE world_id = ?1 AND source_type = ?2 AND source_id = ?3
             ORDER BY ordinal",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map(
            params![world_id.to_string(), source.kind(), object_id(source)],
            reference_from_row,
        )
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

pub(crate) fn load_all(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
) -> Result<Vec<ContentReference>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT source_type, source_id, target_type, target_id, ordinal
             FROM content_references
             WHERE world_id = ?1
             ORDER BY source_type, source_id, ordinal",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map([world_id.to_string()], reference_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

fn object_exists(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
    object: ObjectRef,
) -> Result<bool, StoreError> {
    let sql = match object {
        ObjectRef::World(_) => "SELECT EXISTS(SELECT 1 FROM worlds WHERE id = ?1 AND id = ?2)",
        ObjectRef::Entity(_) => {
            "SELECT EXISTS(SELECT 1 FROM entities WHERE world_id = ?1 AND id = ?2)"
        }
        ObjectRef::Relation(_) => {
            "SELECT EXISTS(SELECT 1 FROM relations WHERE world_id = ?1 AND id = ?2)"
        }
        ObjectRef::Event(_) => {
            "SELECT EXISTS(SELECT 1 FROM events WHERE world_id = ?1 AND id = ?2)"
        }
        ObjectRef::Claim(_) => {
            "SELECT EXISTS(SELECT 1 FROM claims WHERE world_id = ?1 AND id = ?2)"
        }
        ObjectRef::Rule(_) => "SELECT EXISTS(SELECT 1 FROM rules WHERE world_id = ?1 AND id = ?2)",
        ObjectRef::Goal(_) => "SELECT EXISTS(SELECT 1 FROM goals WHERE world_id = ?1 AND id = ?2)",
        ObjectRef::Document(_) => {
            "SELECT EXISTS(SELECT 1 FROM documents WHERE world_id = ?1 AND id = ?2)"
        }
    };
    connection
        .query_row(
            sql,
            params![world_id.to_string(), object_id(object)],
            |row| row.get(0),
        )
        .map_err(|error| map_schema_error(path, error))
}

fn object_id(object: ObjectRef) -> String {
    match object {
        ObjectRef::World(id) => id.to_string(),
        ObjectRef::Entity(id) => id.to_string(),
        ObjectRef::Relation(id) => id.to_string(),
        ObjectRef::Event(id) => id.to_string(),
        ObjectRef::Claim(id) => id.to_string(),
        ObjectRef::Rule(id) => id.to_string(),
        ObjectRef::Goal(id) => id.to_string(),
        ObjectRef::Document(id) => id.to_string(),
    }
}

fn reference_from_row(row: &Row<'_>) -> rusqlite::Result<ContentReference> {
    let source = parse_object_ref(0, &row.get::<_, String>(0)?, &row.get::<_, String>(1)?)?;
    let target = parse_object_ref(2, &row.get::<_, String>(2)?, &row.get::<_, String>(3)?)?;
    let ordinal = u32::try_from(row.get::<_, i64>(4)?).map_err(|error| invalid_data(4, error))?;
    Ok(ContentReference::new(source, target, ordinal))
}

fn parse_object_ref(index: usize, kind: &str, id: &str) -> rusqlite::Result<ObjectRef> {
    macro_rules! parsed {
        ($type:ty, $variant:ident) => {
            <$type>::from_str(id)
                .map(ObjectRef::$variant)
                .map_err(|error| invalid_data(index + 1, error))
        };
    }
    match kind {
        "entity" => parsed!(EntityId, Entity),
        "relation" => parsed!(RelationId, Relation),
        "event" => parsed!(EventId, Event),
        "claim" => parsed!(ClaimId, Claim),
        "rule" => parsed!(RuleId, Rule),
        "goal" => parsed!(GoalId, Goal),
        "document" => parsed!(DocumentId, Document),
        _ => Err(invalid_value(index, kind)),
    }
}
