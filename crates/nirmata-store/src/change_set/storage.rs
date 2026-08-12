use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StoredChangeSetKind {
    Draft,
    Committed,
}

impl StoredChangeSetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Committed => "committed",
        }
    }
}

pub(super) struct RawChangeSetRow {
    id: ChangeSetId,
    world_id: WorldId,
    base_revision_id: RevisionId,
    result_revision_id: Option<RevisionId>,
    objective: String,
    sources: Vec<ObjectRef>,
    assumptions: Vec<String>,
    deterministic_report: Option<Value>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

pub(super) fn insert_change_set_row(
    transaction: &Transaction<'_>,
    path: &Path,
    kind: StoredChangeSetKind,
    id: ChangeSetId,
    world_id: WorldId,
    base_revision_id: RevisionId,
    result_revision_id: Option<RevisionId>,
    objective: &str,
    sources: &[ObjectRef],
    assumptions: &[String],
    deterministic_report: Option<&Value>,
    created_at_ms: i64,
    updated_at_ms: i64,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO change_sets (
                id, world_id, kind, base_revision_id, result_revision_id, objective,
                source_refs_json, assumptions_json, deterministic_report_json,
                created_at_ms, updated_at_ms, variant_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                (SELECT active_variant_id FROM worlds WHERE id = ?2))",
            params![
                id.to_string(),
                world_id.to_string(),
                kind.as_str(),
                base_revision_id.to_string(),
                result_revision_id.map(|value| value.to_string()),
                objective,
                serialize_json(sources)?,
                serialize_json(assumptions)?,
                serialize_optional_json_value(deterministic_report)?,
                created_at_ms,
                updated_at_ms,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(super) fn insert_undo_link(
    transaction: &Transaction<'_>,
    path: &Path,
    undo_revision_id: RevisionId,
    undone_revision_id: Option<RevisionId>,
) -> Result<(), StoreError> {
    let Some(undone_revision_id) = undone_revision_id else {
        return Ok(());
    };

    transaction
        .execute(
            "INSERT INTO revision_undos (undo_revision_id, undone_revision_id)
             VALUES (?1, ?2)",
            params![undo_revision_id.to_string(), undone_revision_id.to_string(),],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(super) fn insert_change_operations(
    transaction: &Transaction<'_>,
    path: &Path,
    change_set_id: ChangeSetId,
    operations: &[ChangeOperation],
) -> Result<(), StoreError> {
    for (ordinal, operation) in operations.iter().enumerate() {
        let (kind, expected_version, affected_ids) = operation_metadata(operation);
        transaction
            .execute(
                "INSERT INTO change_operations (
                    operation_id, change_set_id, ordinal, kind, retcon, expected_version,
                    affected_refs_json, payload_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    operation.operation_id().to_string(),
                    change_set_id.to_string(),
                    i64::try_from(ordinal).map_err(|error| {
                        StoreError::InvalidChangeSet(format!(
                            "operation ordinal is out of range: {error}"
                        ))
                    })?,
                    kind,
                    retcon_kind(operation.retcon()),
                    i64::try_from(expected_version).map_err(|error| {
                        StoreError::InvalidChangeSet(format!(
                            "expected version is out of range: {error}"
                        ))
                    })?,
                    serialize_json(affected_ids)?,
                    serialize_json(operation)?,
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    Ok(())
}

pub(super) fn insert_decision_points(
    transaction: &Transaction<'_>,
    path: &Path,
    change_set_id: ChangeSetId,
    decisions: &[DecisionPoint],
) -> Result<(), StoreError> {
    for (ordinal, decision) in decisions.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO decision_points (
                    id, change_set_id, ordinal, prompt, operation_ids_json, alternatives_json,
                    replacement_target_ref, reason, resolved_alternative
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    decision.decision_point_id().to_string(),
                    change_set_id.to_string(),
                    i64::try_from(ordinal).map_err(|error| {
                        StoreError::InvalidChangeSet(format!(
                            "decision point ordinal is out of range: {error}"
                        ))
                    })?,
                    decision.prompt(),
                    serialize_json(decision.operation_ids())?,
                    serialize_json(decision.alternatives())?,
                    decision
                        .replacement_target()
                        .map(|target| target.to_string()),
                    decision.reason(),
                    decision.resolved_alternative(),
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    Ok(())
}

pub(super) fn insert_waivers(
    transaction: &Transaction<'_>,
    path: &Path,
    change_set_id: ChangeSetId,
    waivers: &[ChangeSetWaiver],
) -> Result<(), StoreError> {
    for waiver in waivers {
        transaction
            .execute(
                "INSERT INTO change_set_waivers (
                    change_set_id, operation_id, issue_code, rationale, created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    change_set_id.to_string(),
                    waiver.operation_id().to_string(),
                    waiver.issue_code(),
                    waiver.rationale(),
                    waiver.created_at_ms(),
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    Ok(())
}

pub(super) fn insert_audits(
    transaction: &Transaction<'_>,
    path: &Path,
    change_set_id: ChangeSetId,
    audits: &[OperationAudit],
) -> Result<(), StoreError> {
    for audit in audits {
        transaction
            .execute(
                "INSERT INTO change_operation_audits (
                    change_set_id, operation_id, decision, source, before_json, after_json,
                    decided_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    change_set_id.to_string(),
                    audit.operation_id().to_string(),
                    operation_decision(audit.decision()),
                    audit.source(),
                    serialize_optional_json(audit.before())?,
                    serialize_optional_json(audit.after())?,
                    audit.decided_at_ms(),
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    Ok(())
}

pub(super) fn insert_revision(
    transaction: &Transaction<'_>,
    path: &Path,
    revision: &StoredRevision,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO revisions (
                id, world_id, parent_revision_id, created_at_ms, author, summary, change_set_id,
                variant_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                (SELECT active_variant_id FROM worlds WHERE id = ?2))",
            params![
                revision.id().to_string(),
                revision.world_id().to_string(),
                revision.parent_revision_id().map(|value| value.to_string()),
                revision.created_at_ms(),
                revision.author(),
                revision.summary(),
                revision.change_set_id().map(|value| value.to_string()),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(super) fn load_change_set_row(
    connection: &Connection,
    path: &Path,
    id: ChangeSetId,
    expected_kind: StoredChangeSetKind,
) -> Result<Option<RawChangeSetRow>, StoreError> {
    connection
        .query_row(
            "SELECT id, world_id, base_revision_id, result_revision_id, objective,
                    source_refs_json, assumptions_json, deterministic_report_json,
                    created_at_ms, updated_at_ms
             FROM change_sets
             WHERE id = ?1 AND kind = ?2",
            params![id.to_string(), expected_kind.as_str()],
            change_set_row_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn restore_draft_record(
    connection: &Connection,
    path: &Path,
    row: RawChangeSetRow,
) -> Result<ChangeSetDraftRecord, StoreError> {
    let operations = load_change_operations(connection, path, row.id)?;
    let decisions = load_decision_points(connection, path, row.id)?;
    let draft = ChangeSetDraft::restore(
        row.id,
        row.world_id,
        row.base_revision_id,
        row.objective,
        row.sources,
        row.assumptions,
        operations,
        decisions,
    )
    .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    Ok(ChangeSetDraftRecord::new(
        draft,
        row.deterministic_report,
        row.created_at_ms,
        row.updated_at_ms,
    ))
}

pub(super) fn restore_committed_record(
    connection: &Connection,
    path: &Path,
    row: RawChangeSetRow,
) -> Result<CommittedChangeSetRecord, StoreError> {
    let operations = load_change_operations(connection, path, row.id)?;
    let decisions = load_decision_points(connection, path, row.id)?;
    let change_set = ChangeSet::restore(
        row.id,
        row.world_id,
        row.base_revision_id,
        row.objective,
        row.sources,
        row.assumptions,
        operations,
        decisions,
    )
    .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    let revision = load_revision_for_change_set(connection, path, row.id, row.result_revision_id)?;
    let waivers = load_waivers(connection, path, row.id)?;
    let audits = load_audits(connection, path, row.id)?;
    let undone_revision_id = load_undone_revision_id(connection, path, revision.id())?;
    CommittedChangeSetRecord::new(
        change_set,
        row.deterministic_report,
        waivers,
        audits,
        revision,
        undone_revision_id,
    )
    .map_err(|_| StoreError::InvalidFormat(path.to_owned()))
}

pub(super) fn load_change_operations(
    connection: &Connection,
    path: &Path,
    change_set_id: ChangeSetId,
) -> Result<Vec<ChangeOperation>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT operation_id, kind, retcon, expected_version, affected_refs_json, payload_json
             FROM change_operations
             WHERE change_set_id = ?1
             ORDER BY ordinal",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map([change_set_id.to_string()], change_operation_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn load_decision_points(
    connection: &Connection,
    path: &Path,
    change_set_id: ChangeSetId,
) -> Result<Vec<DecisionPoint>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT id, prompt, operation_ids_json, alternatives_json,
                    replacement_target_ref, reason, resolved_alternative
             FROM decision_points
             WHERE change_set_id = ?1
             ORDER BY ordinal",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map([change_set_id.to_string()], decision_point_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn load_waivers(
    connection: &Connection,
    path: &Path,
    change_set_id: ChangeSetId,
) -> Result<Vec<ChangeSetWaiver>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT operation_id, issue_code, rationale, created_at_ms
             FROM change_set_waivers
             WHERE change_set_id = ?1
             ORDER BY created_at_ms, operation_id, issue_code",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map([change_set_id.to_string()], waiver_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn load_audits(
    connection: &Connection,
    path: &Path,
    change_set_id: ChangeSetId,
) -> Result<Vec<OperationAudit>, StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT operation_id, decision, source, before_json, after_json, decided_at_ms
             FROM change_operation_audits
             WHERE change_set_id = ?1
             ORDER BY decided_at_ms, operation_id",
        )
        .map_err(|error| map_schema_error(path, error))?;
    statement
        .query_map([change_set_id.to_string()], audit_from_row)
        .map_err(|error| map_schema_error(path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn load_revision_for_change_set(
    connection: &Connection,
    path: &Path,
    change_set_id: ChangeSetId,
    expected_revision_id: Option<RevisionId>,
) -> Result<StoredRevision, StoreError> {
    let revision = connection
        .query_row(
            "SELECT id, world_id, parent_revision_id, change_set_id, author, summary,
                    created_at_ms
             FROM revisions WHERE change_set_id = ?1",
            [change_set_id.to_string()],
            revision_from_row,
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))?
        .ok_or(StoreError::InvalidFormat(path.to_owned()))?;
    if Some(revision.id()) != expected_revision_id {
        return Err(StoreError::InvalidFormat(path.to_owned()));
    }
    Ok(revision)
}

pub(super) fn load_undone_revision_id(
    connection: &Connection,
    path: &Path,
    undo_revision_id: RevisionId,
) -> Result<Option<RevisionId>, StoreError> {
    connection
        .query_row(
            "SELECT undone_revision_id
             FROM revision_undos
             WHERE undo_revision_id = ?1",
            [undo_revision_id.to_string()],
            |row| {
                RevisionId::from_str(&row.get::<_, String>(0)?)
                    .map_err(|error| invalid_data(0, error))
            },
        )
        .optional()
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn current_head(connection: &Connection, path: &Path) -> Result<RevisionId, StoreError> {
    connection
        .query_row("SELECT current_revision FROM worlds", [], |row| {
            RevisionId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))
        })
        .map_err(|error| map_schema_error(path, error))
}

pub(super) fn update_world_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    before: &World,
    after: &World,
) -> Result<(), StoreError> {
    let before_calendar = crate::world_store::serialize_calendar(before.calendar())?;
    let changed = transaction
        .execute(
            "UPDATE worlds
             SET name = ?1, premise_md = ?2, epoch_label = ?3, calendar_json = ?4,
                 updated_at_ms = ?5
             WHERE id = ?6 AND name = ?7 AND premise_md = ?8 AND epoch_label = ?9
               AND calendar_json IS ?10 AND current_revision = ?11",
            params![
                after.name(),
                after.premise_md(),
                after.epoch_label(),
                crate::world_store::serialize_calendar(after.calendar())?,
                after.updated_at_ms(),
                after.id().to_string(),
                before.name(),
                before.premise_md(),
                before.epoch_label(),
                before_calendar,
                before.current_revision().to_string(),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(StoreError::InvalidChangeSet(
            "world metadata changed after the reviewed snapshot".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn apply_change_operations(
    transaction: &Transaction<'_>,
    path: &Path,
    operations: &[ChangeOperation],
) -> Result<(), StoreError> {
    let mut pending: Vec<_> = operations.iter().collect();
    let mut deferred_error = None;

    while !pending.is_empty() {
        let mut progressed = false;
        let mut remaining = Vec::new();

        for operation in pending {
            match apply_change_operation(transaction, path, operation) {
                Ok(()) => progressed = true,
                Err(error) if should_retry_change_operation(&error) => {
                    deferred_error = Some(error);
                    remaining.push(operation);
                }
                Err(error) => return Err(error),
            }
        }

        if remaining.is_empty() {
            return Ok(());
        }
        if !progressed {
            return Err(deferred_error.unwrap_or_else(|| {
                StoreError::InvalidChangeSet(
                    "change set operations could not be ordered into a valid transaction"
                        .to_owned(),
                )
            }));
        }
        pending = remaining;
    }

    Ok(())
}

pub(super) fn should_retry_change_operation(error: &StoreError) -> bool {
    matches!(error, StoreError::Database(_, _))
}

pub(super) fn apply_change_operation(
    transaction: &Transaction<'_>,
    path: &Path,
    operation: &ChangeOperation,
) -> Result<(), StoreError> {
    match operation {
        ChangeOperation::UpdateWorld { before, after, .. } => {
            update_world_in_tx(transaction, path, before, after)
        }
        ChangeOperation::CreateEntity { after, .. } => {
            crate::entity::insert_entity_in_tx(transaction, path, after)
        }
        ChangeOperation::UpdateEntity { after, .. } => {
            crate::entity::update_entity_in_tx(transaction, path, &entity_before_update(after)?)
        }
        ChangeOperation::DeleteEntity {
            before,
            expected_version,
            ..
        } => crate::entity::delete_entity_in_tx(
            transaction,
            path,
            before.world_id(),
            before.id(),
            *expected_version,
        ),
        ChangeOperation::CreateRelation { after, .. } => {
            crate::relation::insert_relation_in_tx(transaction, path, after)
        }
        ChangeOperation::UpdateRelation { after, .. } => crate::relation::update_relation_in_tx(
            transaction,
            path,
            &relation_before_update(after)?,
        ),
        ChangeOperation::DeleteRelation {
            before,
            expected_version,
            ..
        } => crate::relation::delete_relation_in_tx(
            transaction,
            path,
            before.world_id(),
            before.id(),
            *expected_version,
        ),
        ChangeOperation::CreateEvent { after, .. } => crate::event::insert_event_in_tx(
            transaction,
            path,
            after,
            crate::stored_version(after.event().version())?,
        ),
        ChangeOperation::UpdateEvent { after, .. } => crate::event::update_event_in_tx(
            transaction,
            path,
            &event_aggregate_before_update(after)?,
        ),
        ChangeOperation::DeleteEvent {
            before,
            expected_version,
            ..
        } => crate::event::delete_event_in_tx(
            transaction,
            path,
            before.event().world_id(),
            before.event().id(),
            *expected_version,
        ),
        ChangeOperation::CreateGoal { after, .. } => {
            crate::goal::insert_goal_in_tx(transaction, path, after)
        }
        ChangeOperation::UpdateGoal { after, .. } => {
            crate::goal::update_goal_in_tx(transaction, path, &goal_before_update(after)?)
        }
        ChangeOperation::DeleteGoal {
            before,
            expected_version,
            ..
        } => crate::goal::delete_goal_in_tx(
            transaction,
            path,
            before.world_id(),
            before.id(),
            *expected_version,
        ),
        ChangeOperation::CreateRule { after, .. } => {
            crate::rule::insert_rule_in_tx(transaction, path, after)
        }
        ChangeOperation::UpdateRule { after, .. } => {
            crate::rule::update_rule_in_tx(transaction, path, &rule_before_update(after)?)
        }
        ChangeOperation::DeleteRule {
            before,
            expected_version,
            ..
        } => crate::rule::delete_rule_in_tx(
            transaction,
            path,
            before.world_id(),
            before.id(),
            *expected_version,
        ),
        ChangeOperation::CreateClaim { after, .. } => {
            crate::claim::insert_claim_in_tx(transaction, path, after)
        }
        ChangeOperation::UpdateClaim { after, .. } => {
            crate::claim::update_claim_in_tx(transaction, path, &claim_before_update(after)?)
        }
        ChangeOperation::DeleteClaim {
            before,
            expected_version,
            ..
        } => crate::claim::delete_claim_in_tx(
            transaction,
            path,
            before.world_id(),
            before.id(),
            *expected_version,
        ),
        ChangeOperation::CreateDocument { after, .. } => crate::document::insert_document_in_tx(
            transaction,
            path,
            after,
            crate::stored_version(after.object().version())?,
        ),
        ChangeOperation::UpdateDocument { after, .. } => {
            crate::document::load_document(transaction, path, after.object().id())?.ok_or(
                StoreError::ObjectNotFound {
                    object: "document",
                    id: after.object().id().to_string(),
                },
            )?;
            crate::document::update_document_in_tx(
                transaction,
                path,
                &document_before_update(after)?,
            )
        }
        ChangeOperation::DeleteDocument {
            before,
            expected_version,
            ..
        } => crate::document::delete_document_in_tx(
            transaction,
            path,
            before.object().world_id(),
            before.object().id(),
            *expected_version,
        ),
    }
}

pub(super) fn change_set_row_from_row(row: &Row<'_>) -> rusqlite::Result<RawChangeSetRow> {
    Ok(RawChangeSetRow {
        id: ChangeSetId::from_str(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_data(0, error))?,
        world_id: WorldId::from_str(&row.get::<_, String>(1)?)
            .map_err(|error| invalid_data(1, error))?,
        base_revision_id: RevisionId::from_str(&row.get::<_, String>(2)?)
            .map_err(|error| invalid_data(2, error))?,
        result_revision_id: row
            .get::<_, Option<String>>(3)?
            .map(|value| RevisionId::from_str(&value).map_err(|error| invalid_data(3, error)))
            .transpose()?,
        objective: row.get(4)?,
        sources: parse_json(5, &row.get::<_, String>(5)?)?,
        assumptions: parse_json(6, &row.get::<_, String>(6)?)?,
        deterministic_report: row
            .get::<_, Option<String>>(7)?
            .map(|value| parse_json(7, &value))
            .transpose()?,
        created_at_ms: row.get(8)?,
        updated_at_ms: row.get(9)?,
    })
}

pub(super) fn change_operation_from_row(row: &Row<'_>) -> rusqlite::Result<ChangeOperation> {
    let operation_id = ChangeOperationId::from_str(&row.get::<_, String>(0)?)
        .map_err(|error| invalid_data(0, error))?;
    let kind = row.get::<_, String>(1)?;
    let retcon = parse_retcon_kind(2, &row.get::<_, String>(2)?)?;
    let expected_version =
        u64::try_from(row.get::<_, i64>(3)?).map_err(|error| invalid_data(3, error))?;
    let affected_refs: Vec<ObjectRef> = parse_json(4, &row.get::<_, String>(4)?)?;
    let operation: ChangeOperation = parse_json(5, &row.get::<_, String>(5)?)?;
    let (expected_kind, stored_expected_version, stored_affected_refs) =
        operation_metadata(&operation);
    if operation.operation_id() != operation_id
        || expected_kind != kind
        || operation.retcon() != retcon
        || stored_expected_version != expected_version
        || stored_affected_refs != affected_refs.as_slice()
    {
        return Err(invalid_value(5, "change_operation"));
    }
    Ok(operation)
}

pub(super) fn decision_point_from_row(row: &Row<'_>) -> rusqlite::Result<DecisionPoint> {
    DecisionPoint::restore(
        nirmata_core::DecisionPointId::from_str(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_data(0, error))?,
        parse_json(2, &row.get::<_, String>(2)?)?,
        row.get::<_, String>(1)?,
        parse_json(3, &row.get::<_, String>(3)?)?,
        row.get::<_, Option<String>>(4)?
            .map(|value| ObjectRef::from_str(&value).map_err(|error| invalid_domain(4, error)))
            .transpose()?,
        row.get(5)?,
        row.get(6)?,
    )
    .map_err(|error| invalid_domain(0, error))
}

pub(super) fn waiver_from_row(row: &Row<'_>) -> rusqlite::Result<ChangeSetWaiver> {
    ChangeSetWaiver::new(
        ChangeOperationId::from_str(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_data(0, error))?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get(3)?,
    )
    .map_err(|error| invalid_data(0, error))
}

pub(super) fn audit_from_row(row: &Row<'_>) -> rusqlite::Result<OperationAudit> {
    OperationAudit::restore(
        ChangeOperationId::from_str(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_data(0, error))?,
        parse_operation_decision(1, &row.get::<_, String>(1)?)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?
            .map(|value| parse_json(3, &value))
            .transpose()?,
        row.get::<_, Option<String>>(4)?
            .map(|value| parse_json(4, &value))
            .transpose()?,
        row.get(5)?,
    )
    .map_err(|error| invalid_data(0, error))
}

pub(super) fn revision_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRevision> {
    StoredRevision::restore(
        RevisionId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))?,
        WorldId::from_str(&row.get::<_, String>(1)?).map_err(|error| invalid_data(1, error))?,
        row.get::<_, Option<String>>(2)?
            .map(|value| RevisionId::from_str(&value).map_err(|error| invalid_data(2, error)))
            .transpose()?,
        row.get::<_, Option<String>>(3)?
            .map(|value| ChangeSetId::from_str(&value).map_err(|error| invalid_data(3, error)))
            .transpose()?,
        row.get::<_, String>(4)?,
        row.get::<_, String>(5)?,
        row.get(6)?,
    )
    .map_err(|error| invalid_data(0, error))
}

pub(super) fn validate_operation_annotations(
    operations: &[ChangeOperation],
    waivers: &[ChangeSetWaiver],
    audits: &[OperationAudit],
) -> Result<(), StoreError> {
    let operation_ids: HashSet<_> = operations
        .iter()
        .map(ChangeOperation::operation_id)
        .collect();
    let mut audited = HashSet::with_capacity(audits.len());
    for audit in audits {
        if !operation_ids.contains(&audit.operation_id()) {
            return Err(StoreError::InvalidChangeSet(
                "an audit references an operation outside the change set".to_owned(),
            ));
        }
        if !audited.insert(audit.operation_id()) {
            return Err(StoreError::InvalidChangeSet(
                "each operation can be audited only once".to_owned(),
            ));
        }
    }
    if audited.len() != operation_ids.len() {
        return Err(StoreError::InvalidChangeSet(
            "every committed operation must have an audit record".to_owned(),
        ));
    }
    for waiver in waivers {
        if !operation_ids.contains(&waiver.operation_id()) {
            return Err(StoreError::InvalidChangeSet(
                "a waiver references an operation outside the change set".to_owned(),
            ));
        }
    }
    Ok(())
}

pub(super) fn operation_metadata(operation: &ChangeOperation) -> (&'static str, u64, &[ObjectRef]) {
    match operation {
        ChangeOperation::UpdateWorld {
            affected_ids,
            expected_version,
            ..
        } => ("update_world", *expected_version, affected_ids),
        ChangeOperation::CreateEntity {
            affected_ids,
            expected_version,
            ..
        } => ("create_entity", *expected_version, affected_ids),
        ChangeOperation::UpdateEntity {
            affected_ids,
            expected_version,
            ..
        } => ("update_entity", *expected_version, affected_ids),
        ChangeOperation::DeleteEntity {
            affected_ids,
            expected_version,
            ..
        } => ("delete_entity", *expected_version, affected_ids),
        ChangeOperation::CreateRelation {
            affected_ids,
            expected_version,
            ..
        } => ("create_relation", *expected_version, affected_ids),
        ChangeOperation::UpdateRelation {
            affected_ids,
            expected_version,
            ..
        } => ("update_relation", *expected_version, affected_ids),
        ChangeOperation::DeleteRelation {
            affected_ids,
            expected_version,
            ..
        } => ("delete_relation", *expected_version, affected_ids),
        ChangeOperation::CreateEvent {
            affected_ids,
            expected_version,
            ..
        } => ("create_event", *expected_version, affected_ids),
        ChangeOperation::UpdateEvent {
            affected_ids,
            expected_version,
            ..
        } => ("update_event", *expected_version, affected_ids),
        ChangeOperation::DeleteEvent {
            affected_ids,
            expected_version,
            ..
        } => ("delete_event", *expected_version, affected_ids),
        ChangeOperation::CreateGoal {
            affected_ids,
            expected_version,
            ..
        } => ("create_goal", *expected_version, affected_ids),
        ChangeOperation::UpdateGoal {
            affected_ids,
            expected_version,
            ..
        } => ("update_goal", *expected_version, affected_ids),
        ChangeOperation::DeleteGoal {
            affected_ids,
            expected_version,
            ..
        } => ("delete_goal", *expected_version, affected_ids),
        ChangeOperation::CreateRule {
            affected_ids,
            expected_version,
            ..
        } => ("create_rule", *expected_version, affected_ids),
        ChangeOperation::UpdateRule {
            affected_ids,
            expected_version,
            ..
        } => ("update_rule", *expected_version, affected_ids),
        ChangeOperation::DeleteRule {
            affected_ids,
            expected_version,
            ..
        } => ("delete_rule", *expected_version, affected_ids),
        ChangeOperation::CreateClaim {
            affected_ids,
            expected_version,
            ..
        } => ("create_claim", *expected_version, affected_ids),
        ChangeOperation::UpdateClaim {
            affected_ids,
            expected_version,
            ..
        } => ("update_claim", *expected_version, affected_ids),
        ChangeOperation::DeleteClaim {
            affected_ids,
            expected_version,
            ..
        } => ("delete_claim", *expected_version, affected_ids),
        ChangeOperation::CreateDocument {
            affected_ids,
            expected_version,
            ..
        } => ("create_document", *expected_version, affected_ids),
        ChangeOperation::UpdateDocument {
            affected_ids,
            expected_version,
            ..
        } => ("update_document", *expected_version, affected_ids),
        ChangeOperation::DeleteDocument {
            affected_ids,
            expected_version,
            ..
        } => ("delete_document", *expected_version, affected_ids),
    }
}

pub(super) fn snapshots(
    operation: &ChangeOperation,
) -> (Option<ChangeOperationValue>, Option<ChangeOperationValue>) {
    match operation {
        ChangeOperation::CreateEntity { after, .. } => {
            (None, Some(ChangeOperationValue::Entity(after.clone())))
        }
        ChangeOperation::UpdateEntity { before, after, .. } => (
            Some(ChangeOperationValue::Entity(before.clone())),
            Some(ChangeOperationValue::Entity(after.clone())),
        ),
        ChangeOperation::DeleteEntity { before, .. } => {
            (Some(ChangeOperationValue::Entity(before.clone())), None)
        }
        ChangeOperation::CreateRelation { after, .. } => {
            (None, Some(ChangeOperationValue::Relation(after.clone())))
        }
        ChangeOperation::UpdateRelation { before, after, .. } => (
            Some(ChangeOperationValue::Relation(before.clone())),
            Some(ChangeOperationValue::Relation(after.clone())),
        ),
        ChangeOperation::DeleteRelation { before, .. } => {
            (Some(ChangeOperationValue::Relation(before.clone())), None)
        }
        ChangeOperation::CreateEvent { after, .. } => {
            (None, Some(ChangeOperationValue::Event(after.clone())))
        }
        ChangeOperation::UpdateEvent { before, after, .. } => (
            Some(ChangeOperationValue::Event(before.clone())),
            Some(ChangeOperationValue::Event(after.clone())),
        ),
        ChangeOperation::DeleteEvent { before, .. } => {
            (Some(ChangeOperationValue::Event(before.clone())), None)
        }
        ChangeOperation::CreateGoal { after, .. } => {
            (None, Some(ChangeOperationValue::Goal(after.clone())))
        }
        ChangeOperation::UpdateGoal { before, after, .. } => (
            Some(ChangeOperationValue::Goal(before.clone())),
            Some(ChangeOperationValue::Goal(after.clone())),
        ),
        ChangeOperation::DeleteGoal { before, .. } => {
            (Some(ChangeOperationValue::Goal(before.clone())), None)
        }
        ChangeOperation::CreateRule { after, .. } => {
            (None, Some(ChangeOperationValue::Rule(after.clone())))
        }
        ChangeOperation::UpdateRule { before, after, .. } => (
            Some(ChangeOperationValue::Rule(before.clone())),
            Some(ChangeOperationValue::Rule(after.clone())),
        ),
        ChangeOperation::DeleteRule { before, .. } => {
            (Some(ChangeOperationValue::Rule(before.clone())), None)
        }
        ChangeOperation::CreateClaim { after, .. } => {
            (None, Some(ChangeOperationValue::Claim(after.clone())))
        }
        ChangeOperation::UpdateClaim { before, after, .. } => (
            Some(ChangeOperationValue::Claim(before.clone())),
            Some(ChangeOperationValue::Claim(after.clone())),
        ),
        ChangeOperation::DeleteClaim { before, .. } => {
            (Some(ChangeOperationValue::Claim(before.clone())), None)
        }
        ChangeOperation::CreateDocument { after, .. } => {
            (None, Some(ChangeOperationValue::Document(after.clone())))
        }
        ChangeOperation::UpdateDocument { before, after, .. } => (
            Some(ChangeOperationValue::Document(before.clone())),
            Some(ChangeOperationValue::Document(after.clone())),
        ),
        ChangeOperation::DeleteDocument { before, .. } => {
            (Some(ChangeOperationValue::Document(before.clone())), None)
        }
        ChangeOperation::UpdateWorld { before, after, .. } => (
            Some(ChangeOperationValue::World(before.clone())),
            Some(ChangeOperationValue::World(after.clone())),
        ),
    }
}

pub(super) fn retcon_kind(value: RetconKind) -> &'static str {
    match value {
        RetconKind::Additive => "additive",
        RetconKind::Reinterpretive => "reinterpretive",
        RetconKind::Replacement => "replacement",
    }
}

pub(super) fn parse_retcon_kind(index: usize, value: &str) -> rusqlite::Result<RetconKind> {
    match value {
        "additive" => Ok(RetconKind::Additive),
        "reinterpretive" => Ok(RetconKind::Reinterpretive),
        "replacement" => Ok(RetconKind::Replacement),
        _ => Err(invalid_value(index, value)),
    }
}

pub(super) fn operation_decision(value: OperationDecision) -> &'static str {
    match value {
        OperationDecision::Accept => "accept",
        OperationDecision::Edit => "edit",
        OperationDecision::Reject => "reject",
    }
}

pub(super) fn parse_operation_decision(
    index: usize,
    value: &str,
) -> rusqlite::Result<OperationDecision> {
    match value {
        "accept" => Ok(OperationDecision::Accept),
        "edit" => Ok(OperationDecision::Edit),
        "reject" => Ok(OperationDecision::Reject),
        _ => Err(invalid_value(index, value)),
    }
}

pub(super) fn required_text(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, StoreError> {
    let value = value.into().trim().to_owned();
    if value.is_empty() {
        return Err(StoreError::InvalidChangeSet(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(value)
}

pub(super) fn serialize_json<T: Serialize + ?Sized>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(|error| {
        StoreError::InvalidChangeSet(format!("failed to serialize typed payload: {error}"))
    })
}

pub(super) fn serialize_optional_json(
    value: Option<&ChangeOperationValue>,
) -> Result<Option<String>, StoreError> {
    value.map(serialize_json).transpose()
}

pub(super) fn serialize_optional_json_value(
    value: Option<&Value>,
) -> Result<Option<String>, StoreError> {
    value.map(serialize_json).transpose()
}

pub(super) fn parse_json<T: DeserializeOwned>(index: usize, value: &str) -> rusqlite::Result<T> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}
