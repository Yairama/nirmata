use crate::{
    StoreError, WorldStore, content, ensure_world, expected_version, invalid_data, invalid_domain,
    invalid_value, map_database_error, map_schema_error, stored_version, update_conflict,
};
use nirmata_core::{
    RuleId, WorldId,
    document::ObjectRef,
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
};
use rusqlite::{OptionalExtension, Row, params};
use std::str::FromStr;

impl WorldStore {
    pub fn insert_rule(&mut self, rule: &Rule) -> Result<(), StoreError> {
        ensure_world(self, rule.world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        insert_rule_in_tx(&transaction, &self.path, rule)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_rule(&self, id: RuleId) -> Result<Option<Rule>, StoreError> {
        load_rule(&self.connection, &self.path, id)
    }

    pub fn list_rules(&self) -> Result<Vec<Rule>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, kind, statement_md, scope, severity, source,
                        validator_kind, parameters_json, version, created_at_ms, updated_at_ms
                 FROM rules ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let rows = statement
            .query_map([], rule_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn update_rule(&mut self, rule: &Rule) -> Result<Rule, StoreError> {
        ensure_world(self, rule.world_id())?;
        let id = rule.id().to_string();
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        update_rule_in_tx(&transaction, &self.path, rule)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_rule(rule.id())?
            .ok_or(StoreError::ObjectNotFound { object: "rule", id })
    }
}

pub(crate) fn insert_rule_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    rule: &Rule,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO rules (
                id, world_id, kind, statement_md, scope, severity, source,
                validator_kind, parameters_json, version, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                rule.id().to_string(),
                rule.world_id().to_string(),
                rule_kind(rule.kind()),
                rule.statement_md(),
                rule.scope(),
                rule_severity(rule.severity()),
                rule.source(),
                rule.validator_kind().map(rule_validator_kind),
                rule.parameters_json().as_str(),
                stored_version(rule.version())?,
                rule.created_at_ms(),
                rule.updated_at_ms(),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    crate::search::index_rule(connection, path, rule)?;
    Ok(())
}

pub(crate) fn update_rule_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    rule: &Rule,
) -> Result<(), StoreError> {
    let expected = expected_version(rule.version())?;
    let id = rule.id().to_string();
    let changed = connection
        .execute(
            "UPDATE rules
             SET kind = ?1, statement_md = ?2, scope = ?3, severity = ?4, source = ?5,
                 validator_kind = ?6, parameters_json = ?7, version = version + 1,
                 updated_at_ms = ?8
             WHERE id = ?9 AND world_id = ?10 AND version = ?11",
            params![
                rule_kind(rule.kind()),
                rule.statement_md(),
                rule.scope(),
                rule_severity(rule.severity()),
                rule.source(),
                rule.validator_kind().map(rule_validator_kind),
                rule.parameters_json().as_str(),
                rule.updated_at_ms(),
                id,
                rule.world_id().to_string(),
                expected,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "rule",
            "SELECT EXISTS(SELECT 1 FROM rules WHERE id = ?1)",
            id,
            rule.version(),
        )?);
    }
    crate::search::index_rule(connection, path, rule)?;
    Ok(())
}

pub(crate) fn delete_rule_in_tx(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    world_id: WorldId,
    id: RuleId,
    expected_version_value: u64,
) -> Result<(), StoreError> {
    let expected = expected_version(expected_version_value)?;
    let id_value = id.to_string();
    let changed = connection
        .execute(
            "DELETE FROM rules WHERE id = ?1 AND world_id = ?2 AND version = ?3",
            params![id_value, world_id.to_string(), expected],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(update_conflict(
            connection,
            path,
            "rule",
            "SELECT EXISTS(SELECT 1 FROM rules WHERE id = ?1)",
            id.to_string(),
            expected_version_value,
        )?);
    }
    crate::search::remove_text_index_row(connection, path, world_id, ObjectRef::Rule(id))?;
    content::remove_object(connection, path, world_id, ObjectRef::Rule(id))?;
    Ok(())
}

pub(crate) fn load_rule(
    connection: &rusqlite::Connection,
    path: &std::path::Path,
    id: RuleId,
) -> Result<Option<Rule>, StoreError> {
    connection
        .query_row(
            "SELECT id, world_id, kind, statement_md, scope, severity, source,
                    validator_kind, parameters_json, version, created_at_ms, updated_at_ms
             FROM rules WHERE id = ?1",
            [id.to_string()],
            rule_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))
}

fn rule_from_row(row: &Row<'_>) -> rusqlite::Result<Rule> {
    let id = RuleId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?;
    let world_id =
        WorldId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?;
    let kind = parse_rule_kind(2, &row.get::<_, String>(2)?)?;
    let severity = parse_rule_severity(5, &row.get::<_, String>(5)?)?;
    let validator = row
        .get::<_, Option<String>>(7)?
        .map(|value| parse_rule_validator(7, &value))
        .transpose()?;
    let version = u64::try_from(row.get::<_, i64>(9)?).map_err(|error| invalid_data(9, error))?;

    Rule::restore(
        id,
        world_id,
        kind,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        severity,
        row.get(6)?,
        validator,
        row.get::<_, String>(8)?,
        version,
        row.get(10)?,
        row.get(11)?,
    )
    .map_err(|error| invalid_domain(0, error))
}

fn rule_kind(value: RuleKind) -> &'static str {
    match value {
        RuleKind::Constitutive => "constitutive",
        RuleKind::Generative => "generative",
        RuleKind::Institutional => "institutional",
        RuleKind::Authorial => "authorial",
    }
}

fn parse_rule_kind(index: usize, value: &str) -> rusqlite::Result<RuleKind> {
    match value {
        "constitutive" => Ok(RuleKind::Constitutive),
        "generative" => Ok(RuleKind::Generative),
        "institutional" => Ok(RuleKind::Institutional),
        "authorial" => Ok(RuleKind::Authorial),
        _ => Err(invalid_value(index, value)),
    }
}

fn rule_severity(value: RuleSeverity) -> &'static str {
    match value {
        RuleSeverity::Advisory => "advisory",
        RuleSeverity::Hard => "hard",
    }
}

fn parse_rule_severity(index: usize, value: &str) -> rusqlite::Result<RuleSeverity> {
    match value {
        "advisory" => Ok(RuleSeverity::Advisory),
        "hard" => Ok(RuleSeverity::Hard),
        _ => Err(invalid_value(index, value)),
    }
}

fn rule_validator_kind(value: RuleValidatorKind) -> &'static str {
    match value {
        RuleValidatorKind::NoResurrection => "no_resurrection",
    }
}

fn parse_rule_validator(index: usize, value: &str) -> rusqlite::Result<RuleValidatorKind> {
    match value {
        "no_resurrection" => Ok(RuleValidatorKind::NoResurrection),
        _ => Err(invalid_value(index, value)),
    }
}
