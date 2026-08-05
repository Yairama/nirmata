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
        ensure_world(self, record.change_set().world_id())?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        let changed = transaction
            .execute(
                "UPDATE worlds
                 SET current_revision = ?1, updated_at_ms = ?2
                 WHERE id = ?3 AND current_revision = ?4",
                params![
                    record.revision().id().to_string(),
                    record.revision().created_at_ms(),
                    record.change_set().world_id().to_string(),
                    record.change_set().base_revision().to_string(),
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        if changed == 0 {
            let expected_current = current_head(&transaction, &self.path)?;
            return Err(StoreError::StaleRevision {
                expected_current,
                found_base: record.change_set().base_revision(),
            });
        }

        apply_change_operations(&transaction, &self.path, record.change_set().operations())?;

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
        insert_undo_link(
            &transaction,
            &self.path,
            record.revision().id(),
            record.undone_revision_id(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoredChangeSetKind {
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

struct RawChangeSetRow {
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

fn insert_change_set_row(
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
                created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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

fn insert_undo_link(
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

fn insert_change_operations(
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

fn insert_decision_points(
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

fn insert_waivers(
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

fn insert_audits(
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

fn insert_revision(
    transaction: &Transaction<'_>,
    path: &Path,
    revision: &StoredRevision,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO revisions (
                id, world_id, parent_revision_id, created_at_ms, author, summary, change_set_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
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

fn load_change_set_row(
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

fn restore_draft_record(
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

fn restore_committed_record(
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

fn load_change_operations(
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

fn load_decision_points(
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

fn load_waivers(
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

fn load_audits(
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

fn load_revision_for_change_set(
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

fn load_undone_revision_id(
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

fn current_head(connection: &Connection, path: &Path) -> Result<RevisionId, StoreError> {
    connection
        .query_row("SELECT current_revision FROM worlds", [], |row| {
            RevisionId::from_str(&row.get::<_, String>(0)?).map_err(|error| invalid_data(0, error))
        })
        .map_err(|error| map_schema_error(path, error))
}

fn update_world_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    world: &World,
) -> Result<(), StoreError> {
    let changed = transaction
        .execute(
            "UPDATE worlds
             SET name = ?1, premise_md = ?2, epoch_label = ?3, updated_at_ms = ?4
             WHERE id = ?5",
            params![
                world.name(),
                world.premise_md(),
                world.epoch_label(),
                world.updated_at_ms(),
                world.id().to_string(),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    if changed == 0 {
        return Err(StoreError::InvalidChangeSet(
            "world metadata update could not find the target world".to_owned(),
        ));
    }
    Ok(())
}

fn apply_change_operations(
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

fn should_retry_change_operation(error: &StoreError) -> bool {
    matches!(error, StoreError::Database(_, _))
}

fn apply_change_operation(
    transaction: &Transaction<'_>,
    path: &Path,
    operation: &ChangeOperation,
) -> Result<(), StoreError> {
    match operation {
        ChangeOperation::UpdateWorld { after, .. } => update_world_in_tx(transaction, path, after),
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

fn change_set_row_from_row(row: &Row<'_>) -> rusqlite::Result<RawChangeSetRow> {
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

fn change_operation_from_row(row: &Row<'_>) -> rusqlite::Result<ChangeOperation> {
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

fn decision_point_from_row(row: &Row<'_>) -> rusqlite::Result<DecisionPoint> {
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

fn waiver_from_row(row: &Row<'_>) -> rusqlite::Result<ChangeSetWaiver> {
    ChangeSetWaiver::new(
        ChangeOperationId::from_str(&row.get::<_, String>(0)?)
            .map_err(|error| invalid_data(0, error))?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get(3)?,
    )
    .map_err(|error| invalid_data(0, error))
}

fn audit_from_row(row: &Row<'_>) -> rusqlite::Result<OperationAudit> {
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

fn revision_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRevision> {
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

fn validate_operation_annotations(
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

fn operation_metadata(operation: &ChangeOperation) -> (&'static str, u64, &[ObjectRef]) {
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

fn snapshots(
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

fn retcon_kind(value: RetconKind) -> &'static str {
    match value {
        RetconKind::Additive => "additive",
        RetconKind::Reinterpretive => "reinterpretive",
        RetconKind::Replacement => "replacement",
    }
}

fn parse_retcon_kind(index: usize, value: &str) -> rusqlite::Result<RetconKind> {
    match value {
        "additive" => Ok(RetconKind::Additive),
        "reinterpretive" => Ok(RetconKind::Reinterpretive),
        "replacement" => Ok(RetconKind::Replacement),
        _ => Err(invalid_value(index, value)),
    }
}

fn operation_decision(value: OperationDecision) -> &'static str {
    match value {
        OperationDecision::Accept => "accept",
        OperationDecision::Edit => "edit",
        OperationDecision::Reject => "reject",
    }
}

fn parse_operation_decision(index: usize, value: &str) -> rusqlite::Result<OperationDecision> {
    match value {
        "accept" => Ok(OperationDecision::Accept),
        "edit" => Ok(OperationDecision::Edit),
        "reject" => Ok(OperationDecision::Reject),
        _ => Err(invalid_value(index, value)),
    }
}

fn required_text(field: &'static str, value: impl Into<String>) -> Result<String, StoreError> {
    let value = value.into().trim().to_owned();
    if value.is_empty() {
        return Err(StoreError::InvalidChangeSet(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(value)
}

fn serialize_json<T: Serialize + ?Sized>(value: &T) -> Result<String, StoreError> {
    serde_json::to_string(value).map_err(|error| {
        StoreError::InvalidChangeSet(format!("failed to serialize typed payload: {error}"))
    })
}

fn serialize_optional_json(
    value: Option<&ChangeOperationValue>,
) -> Result<Option<String>, StoreError> {
    value.map(serialize_json).transpose()
}

fn serialize_optional_json_value(value: Option<&Value>) -> Result<Option<String>, StoreError> {
    value.map(serialize_json).transpose()
}

fn parse_json<T: DeserializeOwned>(index: usize, value: &str) -> rusqlite::Result<T> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}
