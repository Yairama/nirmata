use crate::{
    StoreError, WorldStore, content, ensure_world, invalid_data, invalid_domain, invalid_value,
    map_database_error, map_schema_error,
};
use nirmata_core::{
    ChangeOperationId, ChangeSetId, RevisionId, World, WorldId,
    change_set::{
        ChangeOperation, ChangeSet, ChangeSetDraft, ChangeSetValidationSnapshot, DecisionPoint,
        RetconKind,
    },
    claim::Claim,
    document::{ContentReference, Document, DocumentAggregate, ObjectRef},
    entity::Entity,
    event::{Event, EventAggregate, EventLink},
    goal::Goal,
    relation::Relation,
    rule::Rule,
    validation::ValidationReport,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params, types::Type};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{collections::HashSet, path::Path, str::FromStr};

#[derive(Clone, Debug, PartialEq)]
pub struct ChangeSetDraftRecord {
    draft: ChangeSetDraft,
    deterministic_report: Option<Value>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl ChangeSetDraftRecord {
    pub fn new(
        draft: ChangeSetDraft,
        deterministic_report: Option<Value>,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Self {
        Self {
            draft,
            deterministic_report,
            created_at_ms,
            updated_at_ms,
        }
    }

    pub fn draft(&self) -> &ChangeSetDraft {
        &self.draft
    }

    pub fn deterministic_report(&self) -> Option<&Value> {
        self.deterministic_report.as_ref()
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

fn entity_before_update(entity: &Entity) -> Result<Entity, StoreError> {
    Entity::restore(
        entity.id(),
        entity.world_id(),
        entity.kind(),
        entity.name(),
        entity.slug(),
        entity.summary().to_owned(),
        entity.body_md().to_owned(),
        entity.attributes_json().as_str().to_owned(),
        entity.aliases().to_vec(),
        previous_version(entity.version())?,
        entity.created_at_ms(),
        entity.updated_at_ms(),
    )
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn relation_before_update(relation: &Relation) -> Result<Relation, StoreError> {
    Relation::restore(
        relation.id(),
        relation.world_id(),
        relation.source_entity_id(),
        relation.target_entity_id(),
        relation.kind(),
        relation.direction(),
        relation.valid_from_tick(),
        relation.valid_to_tick(),
        relation.certainty(),
        relation.source_reference().map(str::to_owned),
        relation.metadata_json().as_str().to_owned(),
        previous_version(relation.version())?,
    )
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn event_before_update(event: &Event) -> Result<Event, StoreError> {
    Event::restore(
        event.id(),
        event.world_id(),
        event.kind(),
        event.summary(),
        event.body_md(),
        event.time().clone(),
        event.location_entity_id(),
        event.participants().to_vec(),
        event.affected_goal_ids().to_vec(),
        previous_version(event.version())?,
        event.created_at_ms(),
        event.updated_at_ms(),
    )
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn event_aggregate_before_update(aggregate: &EventAggregate) -> Result<EventAggregate, StoreError> {
    Ok(EventAggregate::new(
        event_before_update(aggregate.event())?,
        aggregate.links().to_vec(),
    ))
}

fn goal_before_update(goal: &Goal) -> Result<Goal, StoreError> {
    Goal::restore(
        goal.id(),
        goal.world_id(),
        goal.holder_entity_id(),
        goal.desired_state_md(),
        goal.priority(),
        goal.status(),
        goal.period(),
        goal.visibility(),
        goal.source().map(str::to_owned),
        previous_version(goal.version())?,
    )
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn rule_before_update(rule: &Rule) -> Result<Rule, StoreError> {
    Rule::restore(
        rule.id(),
        rule.world_id(),
        rule.kind(),
        rule.statement_md(),
        rule.scope(),
        rule.severity(),
        rule.source().map(str::to_owned),
        rule.validator_kind(),
        rule.parameters_json().as_str().to_owned(),
        previous_version(rule.version())?,
        rule.created_at_ms(),
        rule.updated_at_ms(),
    )
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn claim_before_update(claim: &Claim) -> Result<Claim, StoreError> {
    Claim::restore(
        claim.id(),
        claim.world_id(),
        claim.subject_entity_id(),
        claim.content_md(),
        claim.predicate_key().map(str::to_owned),
        claim.object().cloned(),
        claim.polarity(),
        claim.authentication(),
        claim.holder_entity_id(),
        claim.modality(),
        claim.register().map(str::to_owned),
        claim.epistemic_basis().map(str::to_owned),
        claim.source().map(str::to_owned),
        claim.source_document_id(),
        claim.source_claim_id(),
        claim.holder_confidence(),
        claim.period(),
        claim.registered_revision_id(),
        claim.superseded_revision_id(),
        previous_version(claim.version())?,
    )
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn document_before_update(document: &DocumentAggregate) -> Result<DocumentAggregate, StoreError> {
    let (document, references) = document.clone().into_parts();
    Document::restore(
        document.id(),
        document.world_id(),
        document.title(),
        document.kind(),
        document.author_entity_id(),
        document.perspective_entity_id(),
        document.canon_status(),
        document.body_md(),
        previous_version(document.version())?,
        document.created_at_ms(),
        document.updated_at_ms(),
    )
    .map(|previous| DocumentAggregate::new(previous, references))
    .map_err(|error| StoreError::InvalidChangeSet(error.to_string()))
}

fn previous_version(version: u64) -> Result<u64, StoreError> {
    version.checked_sub(1).ok_or_else(|| {
        StoreError::InvalidChangeSet("updated aggregates must have a prior version".to_owned())
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredRevision {
    id: RevisionId,
    world_id: WorldId,
    parent_revision_id: Option<RevisionId>,
    change_set_id: Option<ChangeSetId>,
    author: String,
    summary: String,
    created_at_ms: i64,
}

impl StoredRevision {
    pub fn new(
        world_id: WorldId,
        parent_revision_id: Option<RevisionId>,
        change_set_id: Option<ChangeSetId>,
        author: impl Into<String>,
        summary: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, StoreError> {
        Self::restore(
            RevisionId::new(),
            world_id,
            parent_revision_id,
            change_set_id,
            author,
            summary,
            created_at_ms,
        )
    }

    pub fn restore(
        id: RevisionId,
        world_id: WorldId,
        parent_revision_id: Option<RevisionId>,
        change_set_id: Option<ChangeSetId>,
        author: impl Into<String>,
        summary: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, StoreError> {
        Ok(Self {
            id,
            world_id,
            parent_revision_id,
            change_set_id,
            author: required_text("author", author)?,
            summary: required_text("summary", summary)?,
            created_at_ms,
        })
    }

    pub fn id(&self) -> RevisionId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn parent_revision_id(&self) -> Option<RevisionId> {
        self.parent_revision_id
    }

    pub fn change_set_id(&self) -> Option<ChangeSetId> {
        self.change_set_id
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeSetWaiver {
    operation_id: ChangeOperationId,
    issue_code: String,
    rationale: String,
    created_at_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AffectedChangeSetGraph {
    entities: Vec<Entity>,
    relations: Vec<Relation>,
    goals: Vec<Goal>,
    events: Vec<Event>,
    event_links: Vec<EventLink>,
    rules: Vec<Rule>,
    claims: Vec<Claim>,
    documents: Vec<Document>,
    content_references: Vec<ContentReference>,
    revisions: Vec<RevisionId>,
}

impl AffectedChangeSetGraph {
    pub fn validation_snapshot(&self) -> ChangeSetValidationSnapshot<'_> {
        ChangeSetValidationSnapshot {
            entities: &self.entities,
            relations: &self.relations,
            goals: &self.goals,
            events: &self.events,
            event_links: &self.event_links,
            rules: &self.rules,
            claims: &self.claims,
            documents: &self.documents,
            content_references: &self.content_references,
            revisions: &self.revisions,
        }
    }

    pub fn content_references(&self) -> &[ContentReference] {
        &self.content_references
    }
}

impl ChangeSetWaiver {
    pub fn new(
        operation_id: ChangeOperationId,
        issue_code: impl Into<String>,
        rationale: impl Into<String>,
        created_at_ms: i64,
    ) -> Result<Self, StoreError> {
        Ok(Self {
            operation_id,
            issue_code: required_text("issue_code", issue_code)?,
            rationale: required_text("rationale", rationale)?,
            created_at_ms,
        })
    }

    pub fn operation_id(&self) -> ChangeOperationId {
        self.operation_id
    }

    pub fn issue_code(&self) -> &str {
        &self.issue_code
    }

    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationDecision {
    Accept,
    Edit,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOperationValue {
    World(World),
    Entity(Entity),
    Relation(Relation),
    Event(EventAggregate),
    Goal(Goal),
    Rule(Rule),
    Claim(Claim),
    Document(DocumentAggregate),
}

#[derive(Clone, Debug, PartialEq)]
pub struct OperationAudit {
    operation_id: ChangeOperationId,
    decision: OperationDecision,
    source: String,
    before: Option<ChangeOperationValue>,
    after: Option<ChangeOperationValue>,
    decided_at_ms: i64,
}

impl OperationAudit {
    pub fn from_operation(
        operation: &ChangeOperation,
        decision: OperationDecision,
        source: impl Into<String>,
        decided_at_ms: i64,
    ) -> Result<Self, StoreError> {
        let (before, after) = snapshots(operation);
        Self::restore(
            operation.operation_id(),
            decision,
            source,
            before,
            after,
            decided_at_ms,
        )
    }

    pub fn restore(
        operation_id: ChangeOperationId,
        decision: OperationDecision,
        source: impl Into<String>,
        before: Option<ChangeOperationValue>,
        after: Option<ChangeOperationValue>,
        decided_at_ms: i64,
    ) -> Result<Self, StoreError> {
        if before.is_none() && after.is_none() {
            return Err(StoreError::InvalidChangeSet(
                "an operation audit must store a before or after value".to_owned(),
            ));
        }
        Ok(Self {
            operation_id,
            decision,
            source: required_text("source", source)?,
            before,
            after,
            decided_at_ms,
        })
    }

    pub fn operation_id(&self) -> ChangeOperationId {
        self.operation_id
    }

    pub fn decision(&self) -> OperationDecision {
        self.decision
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn before(&self) -> Option<&ChangeOperationValue> {
        self.before.as_ref()
    }

    pub fn after(&self) -> Option<&ChangeOperationValue> {
        self.after.as_ref()
    }

    pub fn decided_at_ms(&self) -> i64 {
        self.decided_at_ms
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommittedChangeSetRecord {
    change_set: ChangeSet,
    deterministic_report: Option<Value>,
    waivers: Vec<ChangeSetWaiver>,
    audits: Vec<OperationAudit>,
    revision: StoredRevision,
    undone_revision_id: Option<RevisionId>,
}

impl CommittedChangeSetRecord {
    pub fn new(
        change_set: ChangeSet,
        deterministic_report: Option<Value>,
        waivers: Vec<ChangeSetWaiver>,
        audits: Vec<OperationAudit>,
        revision: StoredRevision,
        undone_revision_id: Option<RevisionId>,
    ) -> Result<Self, StoreError> {
        validate_operation_annotations(change_set.operations(), &waivers, &audits)?;
        if revision.world_id() != change_set.world_id() {
            return Err(StoreError::InvalidChangeSet(
                "revision world does not match change set world".to_owned(),
            ));
        }
        if revision.parent_revision_id() != Some(change_set.base_revision()) {
            return Err(StoreError::InvalidChangeSet(
                "revision parent must match the change set base revision".to_owned(),
            ));
        }
        if revision.change_set_id() != Some(change_set.id()) {
            return Err(StoreError::InvalidChangeSet(
                "revision change set id must match the committed change set".to_owned(),
            ));
        }
        if undone_revision_id == Some(revision.id()) {
            return Err(StoreError::InvalidChangeSet(
                "an undo revision cannot target itself".to_owned(),
            ));
        }

        Ok(Self {
            change_set,
            deterministic_report,
            waivers,
            audits,
            revision,
            undone_revision_id,
        })
    }

    pub fn change_set(&self) -> &ChangeSet {
        &self.change_set
    }

    pub fn deterministic_report(&self) -> Option<&Value> {
        self.deterministic_report.as_ref()
    }

    pub fn waivers(&self) -> &[ChangeSetWaiver] {
        &self.waivers
    }

    pub fn audits(&self) -> &[OperationAudit] {
        &self.audits
    }

    pub fn revision(&self) -> &StoredRevision {
        &self.revision
    }

    pub fn undone_revision_id(&self) -> Option<RevisionId> {
        self.undone_revision_id
    }
}

impl WorldStore {
    pub fn save_change_set_draft(
        &mut self,
        record: &ChangeSetDraftRecord,
    ) -> Result<(), StoreError> {
        ensure_world(self, record.draft().world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        insert_change_set_row(
            &transaction,
            &self.path,
            StoredChangeSetKind::Draft,
            record.draft().id(),
            record.draft().world_id(),
            record.draft().base_revision(),
            None,
            record.draft().objective(),
            record.draft().sources(),
            record.draft().assumptions(),
            record.deterministic_report(),
            record.created_at_ms(),
            record.updated_at_ms(),
        )?;
        insert_change_operations(
            &transaction,
            &self.path,
            record.draft().id(),
            record.draft().operations(),
        )?;
        insert_decision_points(
            &transaction,
            &self.path,
            record.draft().id(),
            record.draft().decisions(),
        )?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn get_change_set_draft(
        &self,
        id: ChangeSetId,
    ) -> Result<Option<ChangeSetDraftRecord>, StoreError> {
        let row =
            load_change_set_row(&self.connection, &self.path, id, StoredChangeSetKind::Draft)?;
        row.map(|row| restore_draft_record(&self.connection, &self.path, row))
            .transpose()
    }

    pub fn load_affected_graph_for_draft(
        &self,
        draft: &ChangeSetDraft,
    ) -> Result<AffectedChangeSetGraph, StoreError> {
        ensure_world(self, draft.world_id())?;
        self.load_affected_graph(draft.sources(), draft.operations(), draft.decisions())
    }

    pub fn load_affected_graph_for_change_set(
        &self,
        change_set: &ChangeSet,
    ) -> Result<AffectedChangeSetGraph, StoreError> {
        ensure_world(self, change_set.world_id())?;
        self.load_affected_graph(
            change_set.sources(),
            change_set.operations(),
            change_set.decisions(),
        )
    }

    pub fn validate_change_set_draft(
        &self,
        draft: &ChangeSetDraft,
    ) -> Result<ValidationReport, StoreError> {
        let graph = self.load_affected_graph_for_draft(draft)?;
        Ok(draft.validation_report(&graph.validation_snapshot()))
    }

    pub fn validate_change_set(
        &self,
        change_set: &ChangeSet,
    ) -> Result<ValidationReport, StoreError> {
        let graph = self.load_affected_graph_for_change_set(change_set)?;
        Ok(change_set.validation_report(&graph.validation_snapshot()))
    }

    pub fn commit_change_set(
        &mut self,
        record: &CommittedChangeSetRecord,
    ) -> Result<StoredRevision, StoreError> {
        self.commit_change_set_from_source(record, None)
    }

    pub fn commit_change_set_from_source(
        &mut self,
        record: &CommittedChangeSetRecord,
        source_revision: Option<RevisionId>,
    ) -> Result<StoredRevision, StoreError> {
        ensure_world(self, record.change_set().world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        let expected_current = current_head(&transaction, &self.path)?;
        if expected_current != record.change_set().base_revision() {
            return Err(StoreError::StaleRevision {
                expected_current,
                found_base: record.change_set().base_revision(),
            });
        }

        apply_change_operations(&transaction, &self.path, record.change_set().operations())?;

        #[cfg(test)]
        if std::mem::take(&mut self.fail_next_derived_index_update) {
            return Err(StoreError::Database(
                self.path.clone(),
                "simulated derived index update failure".to_owned(),
            ));
        }

        insert_change_set_row(
            &transaction,
            &self.path,
            StoredChangeSetKind::Committed,
            record.change_set().id(),
            record.change_set().world_id(),
            record.change_set().base_revision(),
            Some(record.revision().id()),
            record.change_set().objective(),
            record.change_set().sources(),
            record.change_set().assumptions(),
            record.deterministic_report(),
            record.revision().created_at_ms(),
            record.revision().created_at_ms(),
        )?;
        insert_change_operations(
            &transaction,
            &self.path,
            record.change_set().id(),
            record.change_set().operations(),
        )?;
        insert_decision_points(
            &transaction,
            &self.path,
            record.change_set().id(),
            record.change_set().decisions(),
        )?;
        insert_waivers(
            &transaction,
            &self.path,
            record.change_set().id(),
            record.waivers(),
        )?;
        insert_audits(
            &transaction,
            &self.path,
            record.change_set().id(),
            record.audits(),
        )?;
        insert_revision(&transaction, &self.path, record.revision())?;
        if let Some(source_revision) = source_revision {
            if source_revision == record.revision().id() {
                return Err(StoreError::InvalidChangeSet(
                    "merge source revision cannot be the result revision".to_owned(),
                ));
            }
            let changed = transaction
                .execute(
                    "UPDATE revisions SET source_revision_id = ?1 WHERE id = ?2",
                    params![
                        source_revision.to_string(),
                        record.revision().id().to_string()
                    ],
                )
                .map_err(|error| map_database_error(&self.path, error))?;
            if changed != 1 {
                return Err(StoreError::InvalidChangeSet(
                    "merge source revision could not be recorded".to_owned(),
                ));
            }
        }
        insert_undo_link(
            &transaction,
            &self.path,
            record.revision().id(),
            record.undone_revision_id(),
        )?;
        let changed = transaction
            .execute(
                "UPDATE variants
                 SET head_revision_id = ?1
                 WHERE id = (SELECT active_variant_id FROM worlds WHERE id = ?2)
                   AND head_revision_id = ?3 AND archived = 0",
                params![
                    record.revision().id().to_string(),
                    record.change_set().world_id().to_string(),
                    record.change_set().base_revision().to_string(),
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        if changed != 1 {
            return Err(StoreError::StaleRevision {
                expected_current: current_head(&transaction, &self.path)?,
                found_base: record.change_set().base_revision(),
            });
        }
        transaction
            .execute(
                "UPDATE worlds
                 SET current_revision = ?1, updated_at_ms = ?2
                 WHERE id = ?3",
                params![
                    record.revision().id().to_string(),
                    record.revision().created_at_ms(),
                    record.change_set().world_id().to_string(),
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        let snapshot = crate::world_store::read_canon_snapshot_from_connection(
            &transaction,
            &self.path,
            self.world_id,
        )?;
        crate::variant::store_revision_snapshot_in_tx(
            &transaction,
            &self.path,
            record.revision().id(),
            &snapshot,
        )?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_revision(record.revision().id())?
            .ok_or(StoreError::ObjectNotFound {
                object: "revision",
                id: record.revision().id().to_string(),
            })
    }

    pub fn get_committed_change_set(
        &self,
        id: ChangeSetId,
    ) -> Result<Option<CommittedChangeSetRecord>, StoreError> {
        let row = load_change_set_row(
            &self.connection,
            &self.path,
            id,
            StoredChangeSetKind::Committed,
        )?;
        row.map(|row| restore_committed_record(&self.connection, &self.path, row))
            .transpose()
    }

    pub fn get_revision(&self, id: RevisionId) -> Result<Option<StoredRevision>, StoreError> {
        self.connection
            .query_row(
                "SELECT id, world_id, parent_revision_id, change_set_id, author, summary,
                        created_at_ms
                 FROM revisions WHERE id = ?1",
                [id.to_string()],
                revision_from_row,
            )
            .optional()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn list_revisions(&self) -> Result<Vec<StoredRevision>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, parent_revision_id, change_set_id, author, summary,
                        created_at_ms
                 FROM revisions
                 ORDER BY created_at_ms, id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([], revision_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn load_affected_graph(
        &self,
        sources: &[ObjectRef],
        operations: &[ChangeOperation],
        decisions: &[DecisionPoint],
    ) -> Result<AffectedChangeSetGraph, StoreError> {
        let entities = self.list_entities()?;
        let relations = self.list_relations()?;
        let goals = self.list_goals()?;
        let event_aggregates = self.list_events()?;
        let rules = self.list_rules()?;
        let claims = self.list_claims()?;
        let document_aggregates = self.list_documents()?;
        let content_references = content::load_all(&self.connection, &self.path, self.world_id)?;
        let revisions = self
            .list_revisions()?
            .into_iter()
            .map(|revision| revision.id())
            .collect::<Vec<_>>();

        let mut affected = HashSet::new();
        affected.extend(sources.iter().copied());
        for operation in operations {
            affected.insert(operation.primary_ref());
            affected.extend(operation.affected_ids().iter().copied());
        }
        for decision in decisions {
            if let Some(target) = decision.replacement_target() {
                affected.insert(target);
            }
        }

        loop {
            let previous_len = affected.len();

            for reference in &content_references {
                if affected.contains(&reference.source()) || affected.contains(&reference.target())
                {
                    affected.insert(reference.source());
                    affected.insert(reference.target());
                }
            }

            for relation in &relations {
                let relation_ref = ObjectRef::Relation(relation.id());
                let source_ref = ObjectRef::Entity(relation.source_entity_id());
                let target_ref = ObjectRef::Entity(relation.target_entity_id());
                if affected.contains(&relation_ref)
                    || affected.contains(&source_ref)
                    || affected.contains(&target_ref)
                {
                    affected.insert(relation_ref);
                    affected.insert(source_ref);
                    affected.insert(target_ref);
                }
            }

            for goal in &goals {
                let goal_ref = ObjectRef::Goal(goal.id());
                let holder_ref = ObjectRef::Entity(goal.holder_entity_id());
                if affected.contains(&goal_ref) || affected.contains(&holder_ref) {
                    affected.insert(goal_ref);
                    affected.insert(holder_ref);
                }
            }

            for aggregate in &event_aggregates {
                let event = aggregate.event();
                let event_ref = ObjectRef::Event(event.id());
                let participant_touch = event.participants().iter().any(|participant| {
                    affected.contains(&ObjectRef::Entity(participant.entity_id()))
                });
                let goal_touch = event
                    .affected_goal_ids()
                    .iter()
                    .any(|goal_id| affected.contains(&ObjectRef::Goal(*goal_id)));
                let location_touch = event
                    .location_entity_id()
                    .is_some_and(|entity_id| affected.contains(&ObjectRef::Entity(entity_id)));
                let link_touch = aggregate.links().iter().any(|link| {
                    affected.contains(&ObjectRef::Event(link.source_event_id()))
                        || affected.contains(&ObjectRef::Event(link.target_event_id()))
                });
                if affected.contains(&event_ref)
                    || participant_touch
                    || goal_touch
                    || location_touch
                    || link_touch
                {
                    affected.insert(event_ref);
                    if let Some(location_id) = event.location_entity_id() {
                        affected.insert(ObjectRef::Entity(location_id));
                    }
                    affected.extend(
                        event
                            .participants()
                            .iter()
                            .map(|participant| ObjectRef::Entity(participant.entity_id())),
                    );
                    affected.extend(
                        event
                            .affected_goal_ids()
                            .iter()
                            .copied()
                            .map(ObjectRef::Goal),
                    );
                    for link in aggregate.links() {
                        affected.insert(ObjectRef::Event(link.source_event_id()));
                        affected.insert(ObjectRef::Event(link.target_event_id()));
                    }
                }
            }

            for claim in &claims {
                let claim_ref = ObjectRef::Claim(claim.id());
                let object_entity_ref = match claim.object() {
                    Some(nirmata_core::claim::ClaimObject::Entity(entity_id)) => {
                        Some(ObjectRef::Entity(*entity_id))
                    }
                    _ => None,
                };
                let touches = affected.contains(&claim_ref)
                    || affected.contains(&ObjectRef::Entity(claim.subject_entity_id()))
                    || claim
                        .holder_entity_id()
                        .is_some_and(|holder_id| affected.contains(&ObjectRef::Entity(holder_id)))
                    || object_entity_ref.is_some_and(|reference| affected.contains(&reference))
                    || claim.source_document_id().is_some_and(|document_id| {
                        affected.contains(&ObjectRef::Document(document_id))
                    })
                    || claim
                        .source_claim_id()
                        .is_some_and(|claim_id| affected.contains(&ObjectRef::Claim(claim_id)));
                if touches {
                    affected.insert(claim_ref);
                    affected.insert(ObjectRef::Entity(claim.subject_entity_id()));
                    if let Some(holder_id) = claim.holder_entity_id() {
                        affected.insert(ObjectRef::Entity(holder_id));
                    }
                    if let Some(reference) = object_entity_ref {
                        affected.insert(reference);
                    }
                    if let Some(document_id) = claim.source_document_id() {
                        affected.insert(ObjectRef::Document(document_id));
                    }
                    if let Some(source_claim_id) = claim.source_claim_id() {
                        affected.insert(ObjectRef::Claim(source_claim_id));
                    }
                }
            }

            for aggregate in &document_aggregates {
                let document = aggregate.object();
                let document_ref = ObjectRef::Document(document.id());
                let author_touch = document
                    .author_entity_id()
                    .is_some_and(|entity_id| affected.contains(&ObjectRef::Entity(entity_id)));
                let perspective_touch = document
                    .perspective_entity_id()
                    .is_some_and(|entity_id| affected.contains(&ObjectRef::Entity(entity_id)));
                let content_touch = aggregate.references().iter().any(|reference| {
                    affected.contains(&reference.source()) || affected.contains(&reference.target())
                });
                if affected.contains(&document_ref)
                    || author_touch
                    || perspective_touch
                    || content_touch
                {
                    affected.insert(document_ref);
                    if let Some(author_id) = document.author_entity_id() {
                        affected.insert(ObjectRef::Entity(author_id));
                    }
                    if let Some(perspective_id) = document.perspective_entity_id() {
                        affected.insert(ObjectRef::Entity(perspective_id));
                    }
                    for reference in aggregate.references() {
                        affected.insert(reference.source());
                        affected.insert(reference.target());
                    }
                }
            }

            if affected.len() == previous_len {
                break;
            }
        }

        let events: Vec<_> = event_aggregates
            .iter()
            .filter(|aggregate| affected.contains(&ObjectRef::Event(aggregate.event().id())))
            .map(|aggregate| aggregate.event().clone())
            .collect();
        let event_links: Vec<_> = event_aggregates
            .iter()
            .filter(|aggregate| affected.contains(&ObjectRef::Event(aggregate.event().id())))
            .flat_map(|aggregate| {
                aggregate
                    .links()
                    .iter()
                    .filter(|link| {
                        affected.contains(&ObjectRef::Event(link.source_event_id()))
                            || affected.contains(&ObjectRef::Event(link.target_event_id()))
                    })
                    .cloned()
            })
            .collect();

        Ok(AffectedChangeSetGraph {
            entities: entities
                .into_iter()
                .filter(|entity| affected.contains(&ObjectRef::Entity(entity.id())))
                .collect(),
            relations: relations
                .into_iter()
                .filter(|relation| affected.contains(&ObjectRef::Relation(relation.id())))
                .collect(),
            goals: goals
                .into_iter()
                .filter(|goal| affected.contains(&ObjectRef::Goal(goal.id())))
                .collect(),
            events,
            event_links,
            rules,
            claims: claims
                .into_iter()
                .filter(|claim| affected.contains(&ObjectRef::Claim(claim.id())))
                .collect(),
            documents: document_aggregates
                .into_iter()
                .filter(|aggregate| {
                    affected.contains(&ObjectRef::Document(aggregate.object().id()))
                })
                .map(|aggregate| aggregate.object().clone())
                .collect(),
            content_references: content_references
                .into_iter()
                .filter(|reference| {
                    affected.contains(&reference.source()) || affected.contains(&reference.target())
                })
                .collect(),
            revisions,
        })
    }
}

mod storage;

use storage::*;
