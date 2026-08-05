use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_value,
    map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    DocumentId, EntityId, WorldId,
    document::{Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::{path::Path, str::FromStr};

impl WorldStore {
    pub fn insert_document(&mut self, aggregate: &DocumentAggregate) -> Result<(), StoreError> {
        let document = aggregate.object();
        ensure_world(self, document.world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        insert_document_in_tx(
            &transaction,
            &self.path,
            aggregate,
            stored_version(document.version())?,
        )?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_document(&self, id: DocumentId) -> Result<Option<DocumentAggregate>, StoreError> {
        load_document(&self.connection, &self.path, id)
    }

    pub fn list_documents(&self) -> Result<Vec<DocumentAggregate>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, title, kind, author_entity_id, perspective_entity_id,
                        canon_status, body_md, version, created_at_ms, updated_at_ms
                 FROM documents ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let rows = statement
            .query_map([], document_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        rows.into_iter()
            .map(|document| aggregate_document(&self.connection, &self.path, document))
            .collect()
    }

    pub fn update_document(
        &mut self,
        aggregate: &DocumentAggregate,
    ) -> Result<DocumentAggregate, StoreError> {
        let document = aggregate.object();
        ensure_world(self, document.world_id())?;
        let id = document.id().to_string();
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        update_document_in_tx(&transaction, &self.path, aggregate)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_document(document.id())?
            .ok_or(StoreError::ObjectNotFound {
                object: "document",
                id,
            })
    }
}

pub(crate) fn insert_document_in_tx(
    transaction: &rusqlite::Transaction<'_>,
    path: &Path,
    aggregate: &DocumentAggregate,
    version: i64,
) -> Result<(), StoreError> {
    let document = aggregate.object();
    let source = ObjectRef::Document(document.id());
    content::validate(source, aggregate.references())?;
    transaction
        .execute(
            "INSERT INTO documents (
                id, world_id, title, kind, author_entity_id, perspective_entity_id,
                canon_status, body_md, version, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                document.id().to_string(),
                document.world_id().to_string(),
                document.title(),
                document.kind(),
                document.author_entity_id().map(|id| id.to_string()),
                document.perspective_entity_id().map(|id| id.to_string()),
                canon_status(document.canon_status()),
                document.body_md(),
                version,
                document.created_at_ms(),
                document.updated_at_ms(),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    content::insert(
        transaction,
        path,
        document.world_id(),
        aggregate.references(),
    )?;
    crate::search::index_document(transaction, path, document)?;
    Ok(())
}

pub(crate) fn update_document_in_tx(
    transaction: &rusqlite::Transaction<'_>,
    path: &Path,
    aggregate: &DocumentAggregate,
) -> Result<(), StoreError> {
    let document = aggregate.object();
    let source = ObjectRef::Document(document.id());
    content::validate(source, aggregate.references())?;
    let expected = expected_version(document.version())?;
    let id = document.id().to_string();
    let changed = transaction
        .execute(
            "UPDATE documents
             SET title = ?1, kind = ?2, author_entity_id = ?3, perspective_entity_id = ?4,
                 canon_status = ?5, body_md = ?6, version = version + 1, updated_at_ms = ?7
             WHERE id = ?8 AND world_id = ?9 AND version = ?10",
            params![
                document.title(),
                document.kind(),
                document.author_entity_id().map(|value| value.to_string()),
                document
                    .perspective_entity_id()
                    .map(|value| value.to_string()),
                canon_status(document.canon_status()),
                document.body_md(),
                document.updated_at_ms(),
                id,
                document.world_id().to_string(),
                expected,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            transaction,
            path,
            "document",
            "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
            id,
            document.version(),
        )?);
    }
    content::replace(
        transaction,
        path,
        document.world_id(),
        source,
        aggregate.references(),
    )?;
    crate::search::index_document(transaction, path, document)?;
    Ok(())
}

pub(crate) fn delete_document_in_tx(
    transaction: &rusqlite::Transaction<'_>,
    path: &Path,
    world_id: WorldId,
    id: DocumentId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = transaction
        .execute(
            "DELETE FROM documents WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            transaction,
            path,
            "document",
            "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    crate::search::remove_text_index_row(transaction, path, world_id, ObjectRef::Document(id))?;
    content::remove_object(transaction, path, world_id, ObjectRef::Document(id))?;
    Ok(())
}

pub(crate) fn load_document(
    connection: &Connection,
    path: &Path,
    id: DocumentId,
) -> Result<Option<DocumentAggregate>, StoreError> {
    let document = connection
        .query_row(
            "SELECT id, world_id, title, kind, author_entity_id, perspective_entity_id,
                    canon_status, body_md, version, created_at_ms, updated_at_ms
             FROM documents WHERE id = ?1",
            [id.to_string()],
            document_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))?;
    document
        .map(|document| aggregate_document(connection, path, document))
        .transpose()
}

fn aggregate_document(
    connection: &Connection,
    path: &Path,
    document: Document,
) -> Result<DocumentAggregate, StoreError> {
    let references = content::load(
        connection,
        path,
        document.world_id(),
        ObjectRef::Document(document.id()),
    )?;
    Ok(DocumentAggregate::new(document, references))
}

fn document_from_row(row: &Row<'_>) -> rusqlite::Result<Document> {
    Document::restore(
        DocumentId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        WorldId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        parse_optional_entity(row, 4)?,
        parse_optional_entity(row, 5)?,
        parse_canon_status(6, &row.get::<_, String>(6)?)?,
        row.get::<_, String>(7)?,
        u64::try_from(row.get::<_, i64>(8)?).map_err(|error| invalid_data(8, error))?,
        row.get(9)?,
        row.get(10)?,
    )
    .map_err(|error| crate::invalid_domain(0, error))
}

fn parse_optional_entity(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<EntityId>> {
    row.get::<_, Option<String>>(index)?
        .map(|value| EntityId::from_str(&value).map_err(|error| invalid_data(index, error)))
        .transpose()
}

fn canon_status(value: DocumentCanonStatus) -> &'static str {
    match value {
        DocumentCanonStatus::Canonical => "canonical",
        DocumentCanonStatus::NonCanonical => "non_canonical",
    }
}

fn parse_canon_status(index: usize, value: &str) -> rusqlite::Result<DocumentCanonStatus> {
    match value {
        "canonical" => Ok(DocumentCanonStatus::Canonical),
        "non_canonical" => Ok(DocumentCanonStatus::NonCanonical),
        _ => Err(invalid_value(index, value)),
    }
}
