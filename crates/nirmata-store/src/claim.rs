use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_domain,
    invalid_value, map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    ClaimId, DocumentId, EntityId, Period, RevisionId, WorldId,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::ObjectRef,
};
use rusqlite::{OptionalExtension, Row, params};
use std::str::FromStr;

impl WorldStore {
    pub fn insert_claim(&mut self, claim: &Claim) -> Result<(), StoreError> {
        ensure_world(self, claim.world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        insert_claim_in_tx(&transaction, &self.path, claim)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_claim(&self, id: ClaimId) -> Result<Option<Claim>, StoreError> {
        load_claim(&self.connection, &self.path, id)
    }

    pub fn list_claims(&self) -> Result<Vec<Claim>, StoreError> {
        let mut statement = self
            .connection
            .prepare(CLAIM_SELECT)
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([], claim_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn update_claim(&mut self, claim: &Claim) -> Result<Claim, StoreError> {
        ensure_world(self, claim.world_id())?;
        let id = claim.id().to_string();
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        update_claim_in_tx(&transaction, &self.path, claim)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_claim(claim.id())?
            .ok_or(StoreError::ObjectNotFound {
                object: "claim",
                id,
            })
    }
}

pub(crate) fn insert_claim_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    claim: &Claim,
) -> Result<(), StoreError> {
    let object = object_columns(claim.object());
    let period = claim.period();
    connection
        .execute(
            "INSERT INTO claims (
                id, world_id, subject_entity_id, content_md, predicate_key, object_kind,
                object_entity_id, object_scalar, polarity, authentication, holder_entity_id,
                modality, register, epistemic_basis, source, source_document_id,
                source_claim_id, holder_confidence, valid_from_tick, valid_to_tick,
                registered_revision_id, superseded_revision_id, version
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23
             )",
            params![
                claim.id().to_string(),
                claim.world_id().to_string(),
                claim.subject_entity_id().to_string(),
                claim.content_md(),
                claim.predicate_key(),
                object.0,
                object.1,
                object.2,
                polarity(claim.polarity()),
                authentication(claim.authentication()),
                claim.holder_entity_id().map(|id| id.to_string()),
                claim.modality().map(modality),
                claim.register(),
                claim.epistemic_basis(),
                claim.source(),
                claim.source_document_id().map(|id| id.to_string()),
                claim.source_claim_id().map(|id| id.to_string()),
                claim.holder_confidence(),
                period.and_then(|value| value.start_tick()),
                period.and_then(|value| value.end_tick()),
                claim.registered_revision_id().to_string(),
                claim.superseded_revision_id().map(|id| id.to_string()),
                stored_version(claim.version())?,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    crate::search::index_claim(connection, path, claim)?;
    Ok(())
}

pub(crate) fn update_claim_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    claim: &Claim,
) -> Result<(), StoreError> {
    let expected = expected_version(claim.version())?;
    let object = object_columns(claim.object());
    let period = claim.period();
    let id = claim.id().to_string();
    let changed = connection
        .execute(
            "UPDATE claims
             SET subject_entity_id = ?1, content_md = ?2, predicate_key = ?3,
                 object_kind = ?4, object_entity_id = ?5, object_scalar = ?6,
                 polarity = ?7, authentication = ?8, holder_entity_id = ?9,
                 modality = ?10, register = ?11, epistemic_basis = ?12, source = ?13,
                 source_document_id = ?14, source_claim_id = ?15, holder_confidence = ?16,
                 valid_from_tick = ?17, valid_to_tick = ?18, registered_revision_id = ?19,
                 superseded_revision_id = ?20, version = version + 1
             WHERE id = ?21 AND world_id = ?22 AND version = ?23",
            params![
                claim.subject_entity_id().to_string(),
                claim.content_md(),
                claim.predicate_key(),
                object.0,
                object.1,
                object.2,
                polarity(claim.polarity()),
                authentication(claim.authentication()),
                claim.holder_entity_id().map(|value| value.to_string()),
                claim.modality().map(modality),
                claim.register(),
                claim.epistemic_basis(),
                claim.source(),
                claim.source_document_id().map(|value| value.to_string()),
                claim.source_claim_id().map(|value| value.to_string()),
                claim.holder_confidence(),
                period.and_then(|value| value.start_tick()),
                period.and_then(|value| value.end_tick()),
                claim.registered_revision_id().to_string(),
                claim
                    .superseded_revision_id()
                    .map(|value| value.to_string()),
                id,
                claim.world_id().to_string(),
                expected,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "claim",
            "SELECT EXISTS(SELECT 1 FROM claims WHERE id = ?1)",
            id,
            claim.version(),
        )?);
    }
    crate::search::index_claim(connection, path, claim)?;
    Ok(())
}

pub(crate) fn delete_claim_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    world_id: WorldId,
    id: ClaimId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = connection
        .execute(
            "DELETE FROM claims WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "claim",
            "SELECT EXISTS(SELECT 1 FROM claims WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    crate::search::remove_text_index_row(connection, path, world_id, ObjectRef::Claim(id))?;
    content::remove_object(connection, path, world_id, ObjectRef::Claim(id))?;
    Ok(())
}

const CLAIM_SELECT: &str = "
    SELECT id, world_id, subject_entity_id, content_md, predicate_key, object_kind,
           object_entity_id, object_scalar, polarity, authentication, holder_entity_id,
           modality, register, epistemic_basis, source, source_document_id, source_claim_id,
           holder_confidence, valid_from_tick, valid_to_tick, registered_revision_id,
           superseded_revision_id, version
    FROM claims ORDER BY id
";

pub(crate) fn load_claim(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    id: ClaimId,
) -> Result<Option<Claim>, StoreError> {
    connection
        .query_row(
            &CLAIM_SELECT.replace(" ORDER BY id", " WHERE id = ?1"),
            [id.to_string()],
            claim_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))
}

fn claim_from_row(row: &Row<'_>) -> rusqlite::Result<Claim> {
    let object = match row.get::<_, Option<String>>(5)?.as_deref() {
        None => None,
        Some("entity") => Some(ClaimObject::Entity(
            EntityId::from_str(
                &row.get::<_, Option<String>>(6)?
                    .ok_or_else(|| invalid_value(6, "NULL"))?,
            )
            .map_err(|error| invalid_data(6, error))?,
        )),
        Some("scalar") => Some(ClaimObject::Scalar(
            row.get::<_, Option<String>>(7)?
                .ok_or_else(|| invalid_value(7, "NULL"))?,
        )),
        Some(value) => return Err(invalid_value(5, value)),
    };
    let start: Option<i64> = row.get(18)?;
    let end: Option<i64> = row.get(19)?;
    let period = if start.is_some() || end.is_some() {
        Some(Period::new(start, end).map_err(|error| invalid_domain(18, error))?)
    } else {
        None
    };
    Claim::restore(
        ClaimId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        WorldId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?,
        EntityId::from_str(&row.get::<_, String>(2)?).map_err(|error| invalid_data(2, error))?,
        row.get::<_, String>(3)?,
        row.get(4)?,
        object,
        parse_polarity(8, &row.get::<_, String>(8)?)?,
        parse_authentication(9, &row.get::<_, String>(9)?)?,
        parse_optional_id::<EntityId>(row, 10)?,
        row.get::<_, Option<String>>(11)?
            .map(|value| parse_modality(11, &value))
            .transpose()?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        parse_optional_id::<DocumentId>(row, 15)?,
        parse_optional_id::<ClaimId>(row, 16)?,
        row.get(17)?,
        period,
        RevisionId::from_str(&row.get::<_, String>(20)?)
            .map_err(|error| invalid_data(20, error))?,
        parse_optional_id::<RevisionId>(row, 21)?,
        u64::try_from(row.get::<_, i64>(22)?).map_err(|error| invalid_data(22, error))?,
    )
    .map_err(|error| invalid_domain(0, error))
}

fn parse_optional_id<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<T>>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    row.get::<_, Option<String>>(index)?
        .map(|value| T::from_str(&value).map_err(|error| invalid_data(index, error)))
        .transpose()
}

fn object_columns(
    object: Option<&ClaimObject>,
) -> (Option<&'static str>, Option<String>, Option<&str>) {
    match object {
        None => (None, None, None),
        Some(ClaimObject::Entity(id)) => (Some("entity"), Some(id.to_string()), None),
        Some(ClaimObject::Scalar(value)) => (Some("scalar"), None, Some(value)),
    }
}

fn polarity(value: ClaimPolarity) -> &'static str {
    match value {
        ClaimPolarity::Positive => "positive",
        ClaimPolarity::Negative => "negative",
    }
}

fn parse_polarity(index: usize, value: &str) -> rusqlite::Result<ClaimPolarity> {
    match value {
        "positive" => Ok(ClaimPolarity::Positive),
        "negative" => Ok(ClaimPolarity::Negative),
        _ => Err(invalid_value(index, value)),
    }
}

fn authentication(value: ClaimAuthentication) -> &'static str {
    match value {
        ClaimAuthentication::Canonical => "canonical",
        ClaimAuthentication::Attributed => "attributed",
        ClaimAuthentication::Disputed => "disputed",
    }
}

fn parse_authentication(index: usize, value: &str) -> rusqlite::Result<ClaimAuthentication> {
    match value {
        "canonical" => Ok(ClaimAuthentication::Canonical),
        "attributed" => Ok(ClaimAuthentication::Attributed),
        "disputed" => Ok(ClaimAuthentication::Disputed),
        _ => Err(invalid_value(index, value)),
    }
}

fn modality(value: ClaimModality) -> &'static str {
    match value {
        ClaimModality::Assertion => "assertion",
        ClaimModality::Belief => "belief",
        ClaimModality::Hypothesis => "hypothesis",
        ClaimModality::Counterfactual => "counterfactual",
    }
}

fn parse_modality(index: usize, value: &str) -> rusqlite::Result<ClaimModality> {
    match value {
        "assertion" => Ok(ClaimModality::Assertion),
        "belief" => Ok(ClaimModality::Belief),
        "hypothesis" => Ok(ClaimModality::Hypothesis),
        "counterfactual" => Ok(ClaimModality::Counterfactual),
        _ => Err(invalid_value(index, value)),
    }
}
