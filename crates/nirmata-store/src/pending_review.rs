use crate::{StoreError, WorldStore, map_database_error, map_schema_error};
use nirmata_core::{RevisionId, VariantId, WorldId};
use rusqlite::params;
use std::str::FromStr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingReviewRecord {
    pub review_key: String,
    pub world_id: WorldId,
    pub variant_id: VariantId,
    pub base_revision_id: RevisionId,
    pub origin: String,
    pub payload_json: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorldStore {
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_pending_review(
        &self,
        review_key: &str,
        variant_id: VariantId,
        base_revision_id: RevisionId,
        origin: &str,
        payload_json: &str,
        now_ms: i64,
    ) -> Result<(), StoreError> {
        self.connection
            .execute(
                "INSERT INTO pending_reviews (
                    review_key, world_id, variant_id, base_revision_id, origin,
                    payload_json, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (variant_id, review_key) DO UPDATE SET
                    base_revision_id = excluded.base_revision_id,
                    origin = excluded.origin,
                    payload_json = excluded.payload_json,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    review_key,
                    self.world_id.to_string(),
                    variant_id.to_string(),
                    base_revision_id.to_string(),
                    origin,
                    payload_json,
                    now_ms,
                    now_ms,
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        Ok(())
    }

    pub fn list_pending_reviews(
        &self,
        variant_id: VariantId,
    ) -> Result<Vec<PendingReviewRecord>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT review_key, world_id, variant_id, base_revision_id, origin,
                        payload_json, created_at_ms, updated_at_ms
                 FROM pending_reviews
                 WHERE world_id = ?1 AND variant_id = ?2
                 ORDER BY created_at_ms, review_key",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), variant_id.to_string()],
                |row| {
                    let world_id = parse_id(row.get::<_, String>(1)?, 1)?;
                    let variant_id = parse_id(row.get::<_, String>(2)?, 2)?;
                    let base_revision_id = parse_id(row.get::<_, String>(3)?, 3)?;
                    Ok(PendingReviewRecord {
                        review_key: row.get(0)?,
                        world_id,
                        variant_id,
                        base_revision_id,
                        origin: row.get(4)?,
                        payload_json: row.get(5)?,
                        created_at_ms: row.get(6)?,
                        updated_at_ms: row.get(7)?,
                    })
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn delete_pending_review(
        &self,
        variant_id: VariantId,
        review_key: &str,
    ) -> Result<bool, StoreError> {
        self.connection
            .execute(
                "DELETE FROM pending_reviews WHERE variant_id = ?1 AND review_key = ?2",
                params![variant_id.to_string(), review_key],
            )
            .map(|changed| changed == 1)
            .map_err(|error| map_database_error(&self.path, error))
    }
}

fn parse_id<T>(value: String, index: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
