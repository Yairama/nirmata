use crate::{StoreError, WorldStore, map_database_error, map_schema_error};
use nirmata_core::{RevisionId, VariantId, WorldId};
use rusqlite::{Row, params};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredImportBatch {
    pub id: String,
    pub world_id: WorldId,
    pub target_revision: RevisionId,
    pub variant_id: VariantId,
    pub status: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredImportSource {
    pub id: String,
    pub batch_id: String,
    pub source_path: PathBuf,
    pub file_name: String,
    pub format: String,
    pub content_hash: String,
    pub size_bytes: u64,
    pub content_utf8: String,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredImportChunk {
    pub id: String,
    pub source_id: String,
    pub source_hash: String,
    pub ordinal: u32,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line_start: u32,
    pub line_end: u32,
    pub heading: Option<String>,
    pub content_utf8: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredImportCandidate {
    pub id: String,
    pub batch_id: String,
    pub source_id: String,
    pub source_hash: String,
    pub kind: String,
    pub payload_json: String,
    pub citations_json: String,
    pub technical_confidence: f64,
    pub status: String,
    pub identity_decision: Option<String>,
    pub canonical_uri: Option<String>,
    pub contradiction_key: Option<String>,
}

impl WorldStore {
    pub fn create_import_batch(
        &mut self,
        batch: &StoredImportBatch,
        sources: &[StoredImportSource],
        chunks: &[StoredImportChunk],
    ) -> Result<(), StoreError> {
        if batch.world_id != self.world_id {
            return Err(StoreError::WrongWorld {
                expected: self.world_id,
                found: batch.world_id,
            });
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        transaction
            .execute(
                "INSERT INTO import_batches
                 (id, world_id, target_revision_id, status, created_at_ms, variant_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    batch.id,
                    batch.world_id.to_string(),
                    batch.target_revision.to_string(),
                    batch.status,
                    batch.created_at_ms,
                    batch.variant_id.to_string(),
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        for source in sources {
            if source.batch_id != batch.id {
                return Err(StoreError::InvalidAggregate(
                    "import source belongs to another batch".to_owned(),
                ));
            }
            let size_bytes = i64::try_from(source.size_bytes).map_err(|_| {
                StoreError::InvalidAggregate("import source is too large".to_owned())
            })?;
            transaction
                .execute(
                    "INSERT INTO import_sources
                     (id, batch_id, source_path, file_name, format, content_hash,
                      size_bytes, content_utf8, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        source.id,
                        source.batch_id,
                        source.source_path.to_string_lossy(),
                        source.file_name,
                        source.format,
                        source.content_hash,
                        size_bytes,
                        source.content_utf8,
                        source.status,
                    ],
                )
                .map_err(|error| map_database_error(&self.path, error))?;
        }
        insert_chunks(&transaction, &self.path, chunks)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_import_batch(
        &self,
        batch_id: &str,
    ) -> Result<
        Option<(
            StoredImportBatch,
            Vec<StoredImportSource>,
            Vec<StoredImportChunk>,
        )>,
        StoreError,
    > {
        let mut batch_statement = self
            .connection
            .prepare(
                "SELECT id, world_id, target_revision_id, status, created_at_ms, variant_id
                 FROM import_batches WHERE id = ?1 AND world_id = ?2",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let mut rows = batch_statement
            .query(params![batch_id, self.world_id.to_string()])
            .map_err(|error| map_schema_error(&self.path, error))?;
        let Some(row) = rows
            .next()
            .map_err(|error| map_schema_error(&self.path, error))?
        else {
            return Ok(None);
        };
        let batch = import_batch_from_row(row, &self.path)?;
        drop(rows);
        drop(batch_statement);

        let mut source_statement = self
            .connection
            .prepare(
                "SELECT id, batch_id, source_path, file_name, format, content_hash,
                        size_bytes, content_utf8, status
                 FROM import_sources WHERE batch_id = ?1 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let sources = source_statement
            .query_map([batch_id], import_source_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        let chunks = load_chunks(&self.connection, &self.path, batch_id)?;
        Ok(Some((batch, sources, chunks)))
    }

    pub fn replace_import_source(
        &mut self,
        source: &StoredImportSource,
        chunks: &[StoredImportChunk],
    ) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        transaction
            .execute(
                "DELETE FROM import_candidates WHERE source_id = ?1",
                [&source.id],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        transaction
            .execute(
                "DELETE FROM import_chunks WHERE source_id = ?1",
                [&source.id],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        let size_bytes = i64::try_from(source.size_bytes)
            .map_err(|_| StoreError::InvalidAggregate("import source is too large".to_owned()))?;
        let changed = transaction
            .execute(
                "UPDATE import_sources
                 SET source_path = ?1, file_name = ?2, format = ?3, content_hash = ?4,
                     size_bytes = ?5, content_utf8 = ?6, status = 'replaced'
                 WHERE id = ?7 AND batch_id = ?8",
                params![
                    source.source_path.to_string_lossy(),
                    source.file_name,
                    source.format,
                    source.content_hash,
                    size_bytes,
                    source.content_utf8,
                    source.id,
                    source.batch_id,
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        if changed != 1 {
            return Err(StoreError::InvalidAggregate(
                "import source was not found in the selected batch".to_owned(),
            ));
        }
        insert_chunks(&transaction, &self.path, chunks)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn import_chunk_neighborhood(
        &self,
        batch_id: &str,
        chunk_id: &str,
    ) -> Result<Vec<StoredImportChunk>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "WITH RECURSIVE offsets(delta) AS (
                     VALUES(-1)
                     UNION ALL SELECT delta + 1 FROM offsets WHERE delta < 1
                 ), center AS (
                     SELECT c.source_id, c.source_hash, c.ordinal
                     FROM import_chunks c
                     JOIN import_sources s ON s.id = c.source_id
                     JOIN import_batches b ON b.id = s.batch_id
                     WHERE b.id = ?1 AND b.world_id = ?2 AND c.id = ?3
                 )
                 SELECT c.id, c.source_id, c.source_hash, c.ordinal, c.byte_start,
                        c.byte_end, c.line_start, c.line_end, c.heading, c.content_utf8
                 FROM center
                 JOIN offsets
                 JOIN import_chunks c
                   ON c.source_id = center.source_id
                  AND c.source_hash = center.source_hash
                  AND c.ordinal = center.ordinal + offsets.delta
                 ORDER BY c.ordinal",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![batch_id, self.world_id.to_string(), chunk_id],
                import_chunk_from_row,
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn replace_import_candidates(
        &mut self,
        batch_id: &str,
        candidates: &[StoredImportCandidate],
    ) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        transaction
            .execute(
                "DELETE FROM import_candidates WHERE batch_id = ?1",
                [batch_id],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        for candidate in candidates {
            if candidate.batch_id != batch_id {
                return Err(StoreError::InvalidAggregate(
                    "import candidate belongs to another batch".to_owned(),
                ));
            }
            transaction
                .execute(
                    "INSERT INTO import_candidates
                     (id, batch_id, source_id, source_hash, kind, payload_json,
                      citations_json, technical_confidence, status, identity_decision,
                      canonical_uri, contradiction_key)
                     SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12
                     WHERE EXISTS (
                         SELECT 1 FROM import_sources
                         WHERE id = ?3 AND batch_id = ?2 AND content_hash = ?4
                     )",
                    params![
                        candidate.id,
                        candidate.batch_id,
                        candidate.source_id,
                        candidate.source_hash,
                        candidate.kind,
                        candidate.payload_json,
                        candidate.citations_json,
                        candidate.technical_confidence,
                        candidate.status,
                        candidate.identity_decision,
                        candidate.canonical_uri,
                        candidate.contradiction_key,
                    ],
                )
                .map_err(|error| map_database_error(&self.path, error))?;
            if transaction.changes() != 1 {
                return Err(StoreError::InvalidAggregate(
                    "import candidate cites a stale source hash".to_owned(),
                ));
            }
        }
        transaction
            .execute(
                "UPDATE import_batches SET status = 'reviewing'
                 WHERE id = ?1 AND world_id = ?2",
                params![batch_id, self.world_id.to_string()],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn list_import_candidates(
        &self,
        batch_id: &str,
    ) -> Result<Vec<StoredImportCandidate>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, batch_id, source_id, source_hash, kind, payload_json,
                        citations_json, technical_confidence, status, identity_decision,
                        canonical_uri, contradiction_key
                 FROM import_candidates WHERE batch_id = ?1 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([batch_id], import_candidate_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn decide_import_candidate(
        &mut self,
        batch_id: &str,
        candidate_id: &str,
        status: &str,
        identity_decision: Option<&str>,
        canonical_uri: Option<&str>,
    ) -> Result<bool, StoreError> {
        self.connection
            .execute(
                "UPDATE import_candidates
                 SET status = ?1, identity_decision = ?2, canonical_uri = ?3
                 WHERE id = ?4 AND batch_id = ?5",
                params![
                    status,
                    identity_decision,
                    canonical_uri,
                    candidate_id,
                    batch_id
                ],
            )
            .map(|changed| changed == 1)
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn edit_import_candidate(
        &mut self,
        batch_id: &str,
        candidate_id: &str,
        kind: &str,
        payload_json: &str,
        technical_confidence: f64,
        contradiction_key: Option<&str>,
    ) -> Result<bool, StoreError> {
        self.connection
            .execute(
                "UPDATE import_candidates
                 SET kind = ?1, payload_json = ?2, technical_confidence = ?3,
                     contradiction_key = ?4, status = 'pending',
                     identity_decision = NULL, canonical_uri = NULL
                 WHERE id = ?5 AND batch_id = ?6",
                params![
                    kind,
                    payload_json,
                    technical_confidence,
                    contradiction_key,
                    candidate_id,
                    batch_id,
                ],
            )
            .map(|changed| changed == 1)
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn delete_import_batch(&mut self, batch_id: &str) -> Result<bool, StoreError> {
        self.connection
            .execute(
                "DELETE FROM import_batches WHERE id = ?1 AND world_id = ?2",
                params![batch_id, self.world_id.to_string()],
            )
            .map(|changed| changed == 1)
            .map_err(|error| map_database_error(&self.path, error))
    }
}

fn insert_chunks(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    chunks: &[StoredImportChunk],
) -> Result<(), StoreError> {
    for chunk in chunks {
        connection
            .execute(
                "INSERT INTO import_chunks
                 (id, source_id, source_hash, ordinal, byte_start, byte_end,
                  line_start, line_end, heading, content_utf8)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    chunk.id,
                    chunk.source_id,
                    chunk.source_hash,
                    i64::from(chunk.ordinal),
                    i64::try_from(chunk.byte_start).map_err(|_| StoreError::InvalidAggregate(
                        "chunk offset is too large".to_owned()
                    ))?,
                    i64::try_from(chunk.byte_end).map_err(|_| StoreError::InvalidAggregate(
                        "chunk offset is too large".to_owned()
                    ))?,
                    i64::from(chunk.line_start),
                    i64::from(chunk.line_end),
                    chunk.heading,
                    chunk.content_utf8,
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    Ok(())
}

fn load_chunks(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    batch_id: &str,
) -> Result<Vec<StoredImportChunk>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT c.id, c.source_id, c.source_hash, c.ordinal, c.byte_start,
                    c.byte_end, c.line_start, c.line_end, c.heading, c.content_utf8
             FROM import_chunks c
             JOIN import_sources s ON s.id = c.source_id
             WHERE s.batch_id = ?1
             ORDER BY c.source_id, c.ordinal",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map([batch_id], import_chunk_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

fn import_batch_from_row(
    row: &Row<'_>,
    path: &std::path::Path,
) -> Result<StoredImportBatch, StoreError> {
    let world_id = row
        .get::<_, String>(1)
        .map_err(|error| map_schema_error(path, error))?
        .parse()
        .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    let target_revision = row
        .get::<_, String>(2)
        .map_err(|error| map_schema_error(path, error))?
        .parse()
        .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    let variant_id = row
        .get::<_, String>(5)
        .map_err(|error| map_schema_error(path, error))?
        .parse()
        .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    Ok(StoredImportBatch {
        id: row.get(0).map_err(|error| map_schema_error(path, error))?,
        world_id,
        target_revision,
        variant_id,
        status: row.get(3).map_err(|error| map_schema_error(path, error))?,
        created_at_ms: row.get(4).map_err(|error| map_schema_error(path, error))?,
    })
}

fn import_source_from_row(row: &Row<'_>) -> rusqlite::Result<StoredImportSource> {
    let size_bytes = row.get::<_, i64>(6)?;
    Ok(StoredImportSource {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        source_path: PathBuf::from(row.get::<_, String>(2)?),
        file_name: row.get(3)?,
        format: row.get(4)?,
        content_hash: row.get(5)?,
        size_bytes: u64::try_from(size_bytes).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?,
        content_utf8: row.get(7)?,
        status: row.get(8)?,
    })
}

fn import_chunk_from_row(row: &Row<'_>) -> rusqlite::Result<StoredImportChunk> {
    Ok(StoredImportChunk {
        id: row.get(0)?,
        source_id: row.get(1)?,
        source_hash: row.get(2)?,
        ordinal: u32::try_from(row.get::<_, i64>(3)?).map_err(integer_conversion(3))?,
        byte_start: u64::try_from(row.get::<_, i64>(4)?).map_err(integer_conversion(4))?,
        byte_end: u64::try_from(row.get::<_, i64>(5)?).map_err(integer_conversion(5))?,
        line_start: u32::try_from(row.get::<_, i64>(6)?).map_err(integer_conversion(6))?,
        line_end: u32::try_from(row.get::<_, i64>(7)?).map_err(integer_conversion(7))?,
        heading: row.get(8)?,
        content_utf8: row.get(9)?,
    })
}

fn import_candidate_from_row(row: &Row<'_>) -> rusqlite::Result<StoredImportCandidate> {
    Ok(StoredImportCandidate {
        id: row.get(0)?,
        batch_id: row.get(1)?,
        source_id: row.get(2)?,
        source_hash: row.get(3)?,
        kind: row.get(4)?,
        payload_json: row.get(5)?,
        citations_json: row.get(6)?,
        technical_confidence: row.get(7)?,
        status: row.get(8)?,
        identity_decision: row.get(9)?,
        canonical_uri: row.get(10)?,
        contradiction_key: row.get(11)?,
    })
}

fn integer_conversion(index: usize) -> impl FnOnce(std::num::TryFromIntError) -> rusqlite::Error {
    move |error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    }
}
