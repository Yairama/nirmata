struct ValidationState {
    world_id: WorldId,
    entities: HashMap<crate::EntityId, Entity>,
    relations: HashMap<crate::RelationId, Relation>,
    goals: HashMap<crate::GoalId, Goal>,
    events: HashMap<crate::EventId, Event>,
    event_links: Vec<EventLink>,
    rules: HashMap<crate::RuleId, Rule>,
    claims: HashMap<crate::ClaimId, Claim>,
    documents: HashMap<crate::DocumentId, Document>,
    content_references: Vec<ContentReference>,
    revisions: HashSet<RevisionId>,
}

impl ValidationState {
    fn from_snapshot(
        snapshot: &ChangeSetValidationSnapshot<'_>,
        world_id: WorldId,
        base_revision: RevisionId,
    ) -> Self {
        let mut revisions = snapshot.revisions.iter().copied().collect::<HashSet<_>>();
        revisions.insert(base_revision);

        Self {
            world_id,
            entities: snapshot
                .entities
                .iter()
                .cloned()
                .map(|entity| (entity.id(), entity))
                .collect(),
            relations: snapshot
                .relations
                .iter()
                .cloned()
                .map(|relation| (relation.id(), relation))
                .collect(),
            goals: snapshot
                .goals
                .iter()
                .cloned()
                .map(|goal| (goal.id(), goal))
                .collect(),
            events: snapshot
                .events
                .iter()
                .cloned()
                .map(|event| (event.id(), event))
                .collect(),
            event_links: snapshot.event_links.to_vec(),
            rules: snapshot
                .rules
                .iter()
                .cloned()
                .map(|rule| (rule.id(), rule))
                .collect(),
            claims: snapshot
                .claims
                .iter()
                .cloned()
                .map(|claim| (claim.id(), claim))
                .collect(),
            documents: snapshot
                .documents
                .iter()
                .cloned()
                .map(|document| (document.id(), document))
                .collect(),
            content_references: snapshot.content_references.to_vec(),
            revisions,
        }
    }

    fn contains(&self, object: ObjectRef) -> bool {
        match object {
            ObjectRef::World(id) => id == self.world_id,
            ObjectRef::Entity(id) => self.entities.contains_key(&id),
            ObjectRef::Relation(id) => self.relations.contains_key(&id),
            ObjectRef::Event(id) => self.events.contains_key(&id),
            ObjectRef::Claim(id) => self.claims.contains_key(&id),
            ObjectRef::Rule(id) => self.rules.contains_key(&id),
            ObjectRef::Goal(id) => self.goals.contains_key(&id),
            ObjectRef::Document(id) => self.documents.contains_key(&id),
        }
    }

    fn version(&self, object: ObjectRef) -> Option<u64> {
        match object {
            ObjectRef::World(_) => None,
            ObjectRef::Entity(id) => self.entities.get(&id).map(Entity::version),
            ObjectRef::Relation(id) => self.relations.get(&id).map(Relation::version),
            ObjectRef::Event(id) => self.events.get(&id).map(Event::version),
            ObjectRef::Claim(id) => self.claims.get(&id).map(Claim::version),
            ObjectRef::Rule(id) => self.rules.get(&id).map(Rule::version),
            ObjectRef::Goal(id) => self.goals.get(&id).map(Goal::version),
            ObjectRef::Document(id) => self.documents.get(&id).map(Document::version),
        }
    }

    fn object_world(&self, object: ObjectRef) -> Option<WorldId> {
        match object {
            ObjectRef::World(id) => (id == self.world_id).then_some(self.world_id),
            ObjectRef::Entity(id) => self.entities.get(&id).map(Entity::world_id),
            ObjectRef::Relation(id) => self.relations.get(&id).map(Relation::world_id),
            ObjectRef::Event(id) => self.events.get(&id).map(Event::world_id),
            ObjectRef::Claim(id) => self.claims.get(&id).map(Claim::world_id),
            ObjectRef::Rule(id) => self.rules.get(&id).map(Rule::world_id),
            ObjectRef::Goal(id) => self.goals.get(&id).map(Goal::world_id),
            ObjectRef::Document(id) => self.documents.get(&id).map(Document::world_id),
        }
    }

    fn has_entity_slug(
        &self,
        world_id: WorldId,
        slug: &str,
        ignore_id: Option<crate::EntityId>,
    ) -> bool {
        self.entities.values().any(|entity| {
            entity.world_id() == world_id && entity.slug() == slug && Some(entity.id()) != ignore_id
        })
    }

    fn entity_values(&self) -> Vec<Entity> {
        self.entities.values().cloned().collect()
    }

    fn goal_values(&self) -> Vec<Goal> {
        self.goals.values().cloned().collect()
    }

    fn event_values(&self) -> Vec<Event> {
        self.events.values().cloned().collect()
    }

    fn document_values(&self) -> Vec<Document> {
        self.documents.values().cloned().collect()
    }

    fn claim_values(&self) -> Vec<Claim> {
        self.claims.values().cloned().collect()
    }

    fn dependents(&self, target: ObjectRef) -> Vec<IssueObject> {
        let mut dependents = HashSet::new();

        match target {
            ObjectRef::World(_) => {}
            ObjectRef::Entity(entity_id) => {
                for relation in self.relations.values() {
                    if relation.source_entity_id() == entity_id
                        || relation.target_entity_id() == entity_id
                    {
                        dependents.insert(IssueObject::new("relation", relation.id()));
                    }
                }
                for goal in self.goals.values() {
                    if goal.holder_entity_id() == entity_id {
                        dependents.insert(IssueObject::new("goal", goal.id()));
                    }
                }
                for event in self.events.values() {
                    if event.location_entity_id() == Some(entity_id)
                        || event
                            .participants()
                            .iter()
                            .any(|participant| participant.entity_id() == entity_id)
                    {
                        dependents.insert(IssueObject::new("event", event.id()));
                    }
                }
                for claim in self.claims.values() {
                    let object_entity = match claim.object() {
                        Some(crate::claim::ClaimObject::Entity(object_id)) => Some(*object_id),
                        _ => None,
                    };
                    if claim.subject_entity_id() == entity_id
                        || claim.holder_entity_id() == Some(entity_id)
                        || object_entity == Some(entity_id)
                    {
                        dependents.insert(IssueObject::new("claim", claim.id()));
                    }
                }
                for document in self.documents.values() {
                    if document.author_entity_id() == Some(entity_id)
                        || document.perspective_entity_id() == Some(entity_id)
                    {
                        dependents.insert(IssueObject::new("document", document.id()));
                    }
                }
            }
            ObjectRef::Goal(goal_id) => {
                for event in self.events.values() {
                    if event.affected_goal_ids().contains(&goal_id) {
                        dependents.insert(IssueObject::new("event", event.id()));
                    }
                }
            }
            ObjectRef::Document(document_id) => {
                for claim in self.claims.values() {
                    if claim.source_document_id() == Some(document_id) {
                        dependents.insert(IssueObject::new("claim", claim.id()));
                    }
                }
            }
            ObjectRef::Claim(claim_id) => {
                for claim in self.claims.values() {
                    if claim.source_claim_id() == Some(claim_id) {
                        dependents.insert(IssueObject::new("claim", claim.id()));
                    }
                }
            }
            ObjectRef::Relation(_) | ObjectRef::Rule(_) => {}
            ObjectRef::Event(event_id) => {
                for link in &self.event_links {
                    if link.target_event_id() == event_id {
                        dependents.insert(IssueObject::new("event", link.source_event_id()));
                    }
                }
            }
        }

        for reference in &self.content_references {
            if reference.target() == target {
                dependents.insert(issue_object(reference.source()));
            }
        }

        let mut dependents: Vec<_> = dependents.into_iter().collect();
        dependents.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.id.cmp(&right.id))
        });
        dependents
    }

    fn apply(&mut self, operation: &ChangeOperation) {
        if is_delete_operation(operation) {
            let deleted_ref = operation.primary_ref();
            self.content_references.retain(|reference| {
                reference.source() != deleted_ref && reference.target() != deleted_ref
            });
        }

        match operation {
            ChangeOperation::UpdateWorld { .. } => {}
            ChangeOperation::CreateEntity { after, .. }
            | ChangeOperation::UpdateEntity { after, .. } => {
                self.entities.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteEntity { before, .. } => {
                self.entities.remove(&before.id());
            }
            ChangeOperation::CreateRelation { after, .. }
            | ChangeOperation::UpdateRelation { after, .. } => {
                self.relations.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteRelation { before, .. } => {
                self.relations.remove(&before.id());
            }
            ChangeOperation::CreateEvent { after, .. }
            | ChangeOperation::UpdateEvent { after, .. } => {
                self.events
                    .insert(after.event().id(), after.event().clone());
                self.event_links
                    .retain(|link| link.source_event_id() != after.event().id());
                self.event_links.extend(after.links().iter().cloned());
            }
            ChangeOperation::DeleteEvent { before, .. } => {
                self.events.remove(&before.event().id());
                self.event_links.retain(|link| {
                    link.source_event_id() != before.event().id()
                        && link.target_event_id() != before.event().id()
                });
            }
            ChangeOperation::CreateGoal { after, .. }
            | ChangeOperation::UpdateGoal { after, .. } => {
                self.goals.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteGoal { before, .. } => {
                self.goals.remove(&before.id());
            }
            ChangeOperation::CreateRule { after, .. }
            | ChangeOperation::UpdateRule { after, .. } => {
                self.rules.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteRule { before, .. } => {
                self.rules.remove(&before.id());
            }
            ChangeOperation::CreateClaim { after, .. }
            | ChangeOperation::UpdateClaim { after, .. } => {
                self.claims.insert(after.id(), after.clone());
            }
            ChangeOperation::DeleteClaim { before, .. } => {
                self.claims.remove(&before.id());
            }
            ChangeOperation::CreateDocument { after, .. }
            | ChangeOperation::UpdateDocument { after, .. } => {
                let source = ObjectRef::Document(after.object().id());
                self.documents
                    .insert(after.object().id(), after.object().clone());
                self.content_references
                    .retain(|reference| reference.source() != source);
                self.content_references
                    .extend(after.references().iter().cloned());
            }
            ChangeOperation::DeleteDocument { before, .. } => {
                self.documents.remove(&before.object().id());
            }
        }
    }
}

#[derive(Default)]
struct ResultingStateValidationScope {
    entity_ids: HashSet<crate::EntityId>,
    event_ids: HashSet<crate::EventId>,
    scan_all_claims: bool,
    scan_all_lifecycles: bool,
}

impl ResultingStateValidationScope {
    fn observe(&mut self, operation: &ChangeOperation) {
        match operation {
            ChangeOperation::UpdateWorld { .. } => {}
            ChangeOperation::CreateEntity { after, .. }
            | ChangeOperation::UpdateEntity { after, .. } => {
                self.entity_ids.insert(after.id());
            }
            ChangeOperation::DeleteEntity { before, .. } => {
                self.entity_ids.insert(before.id());
            }
            ChangeOperation::CreateEvent { after, .. } => self.track_event(after.event()),
            ChangeOperation::UpdateEvent { before, after, .. } => {
                self.track_event(before.event());
                self.track_event(after.event());
            }
            ChangeOperation::DeleteEvent { before, .. } => self.track_event(before.event()),
            ChangeOperation::CreateClaim { .. }
            | ChangeOperation::UpdateClaim { .. }
            | ChangeOperation::DeleteClaim { .. } => {
                self.scan_all_claims = true;
            }
            ChangeOperation::CreateRule { after, .. }
            | ChangeOperation::UpdateRule { after, .. } => {
                if after.can_produce_hard_error()
                    && matches!(
                        after.validator_kind(),
                        Some(RuleValidatorKind::NoResurrection)
                    )
                {
                    self.scan_all_lifecycles = true;
                }
            }
            _ => {}
        }
    }

    fn track_event(&mut self, event: &Event) {
        self.event_ids.insert(event.id());
        self.entity_ids.extend(
            event
                .participants()
                .iter()
                .map(|participant| participant.entity_id()),
        );
    }
}

fn validate_resulting_state(
    state: &ValidationState,
    scope: &ResultingStateValidationScope,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if scope.scan_all_claims {
        issues.extend(validate_claims(
            &state.claim_values(),
            &state.entity_values(),
            &state.document_values(),
            &state.revisions,
        ));
    }

    if !scope.event_ids.is_empty() {
        let events = state.event_values();
        let relevant_links: Vec<_> = state
            .event_links
            .iter()
            .filter(|link| {
                scope.event_ids.contains(&link.source_event_id())
                    || scope.event_ids.contains(&link.target_event_id())
            })
            .cloned()
            .collect();
        issues.extend(validate_event_links(&relevant_links, &events));
    }

    let no_resurrection_rules: Vec<_> = state
        .rules
        .values()
        .filter(|rule| {
            rule.can_produce_hard_error()
                && matches!(
                    rule.validator_kind(),
                    Some(RuleValidatorKind::NoResurrection)
                )
        })
        .collect();
    if no_resurrection_rules.is_empty() && scope.entity_ids.is_empty() && !scope.scan_all_lifecycles
    {
        return issues;
    }

    let entity_ids: Vec<_> = if scope.scan_all_lifecycles {
        state.entities.keys().copied().collect()
    } else {
        scope.entity_ids.iter().copied().collect()
    };
    for entity_id in entity_ids {
        let Some(entity) = state.entities.get(&entity_id) else {
            continue;
        };
        if entity.kind() != EntityKind::Person {
            continue;
        }

        let lifecycle = collect_entity_lifecycle(entity, state.events.values());
        issues.extend(validate_lifecycle(
            entity,
            lifecycle.birth,
            lifecycle.death,
            &lifecycle.participations,
        ));

        if let Some(death) = lifecycle.death {
            for rule in &no_resurrection_rules {
                issues.extend(validate_no_resurrection(
                    rule,
                    entity,
                    death,
                    &lifecycle.participations,
                ));
            }
        }
    }

    issues
}

struct EntityLifecycle<'a> {
    birth: Option<&'a Event>,
    death: Option<&'a Event>,
    participations: Vec<&'a Event>,
}

fn collect_entity_lifecycle<'a>(
    entity: &Entity,
    events: impl Iterator<Item = &'a Event>,
) -> EntityLifecycle<'a> {
    let mut births = Vec::new();
    let mut deaths = Vec::new();
    let mut participations = Vec::new();

    for event in events {
        if !event
            .participants()
            .iter()
            .any(|participant| participant.entity_id() == entity.id())
        {
            continue;
        }

        participations.push(event);
        if event.kind().eq_ignore_ascii_case("birth")
            && lifecycle_subject_matches(event, entity.id(), &["subject", "born", "child"])
        {
            births.push(event);
        }
        if event.kind().eq_ignore_ascii_case("death")
            && lifecycle_subject_matches(event, entity.id(), &["subject", "deceased", "victim"])
        {
            deaths.push(event);
        }
    }

    EntityLifecycle {
        birth: unique_event(births),
        death: unique_event(deaths),
        participations,
    }
}

fn lifecycle_subject_matches(event: &Event, entity_id: crate::EntityId, roles: &[&str]) -> bool {
    if let Some(participant) = event.participants().iter().find(|participant| {
        roles
            .iter()
            .any(|role| participant.role().eq_ignore_ascii_case(role))
    }) {
        return participant.entity_id() == entity_id;
    }

    event.participants().len() == 1 && event.participants()[0].entity_id() == entity_id
}

fn unique_event(events: Vec<&Event>) -> Option<&Event> {
    match events.as_slice() {
        [event] => Some(*event),
        _ => None,
    }
}

fn validate_operations_and_decisions(
    world_id: WorldId,
    operations: &[ChangeOperation],
    decisions: &[DecisionPoint],
) -> Result<(), DomainError> {
    let mut operation_ids = HashSet::with_capacity(operations.len());
    let mut replacement_operations = HashSet::new();

    for operation in operations {
        if !operation_ids.insert(operation.operation_id()) {
            return Err(DomainError::DuplicateChangeOperationId(
                operation.operation_id(),
            ));
        }
        operation.validate(world_id)?;
        if operation.retcon() == RetconKind::Replacement {
            replacement_operations.insert(operation.operation_id());
        }
    }

    let mut decision_point_ids = HashSet::with_capacity(decisions.len());
    let mut replacement_decisions = HashSet::new();

    for decision in decisions {
        if !decision_point_ids.insert(decision.decision_point_id()) {
            return Err(DomainError::DuplicateDecisionPointId(
                decision.decision_point_id(),
            ));
        }
        decision.validate(&operation_ids)?;
        for operation_id in decision.operation_ids() {
            if replacement_operations.contains(operation_id) {
                replacement_decisions.insert(*operation_id);
            }
        }
    }

    for operation_id in replacement_operations {
        if !replacement_decisions.contains(&operation_id) {
            return Err(DomainError::InvalidChangeSetContext(
                "replacement operations require a decision point",
            ));
        }
    }

    Ok(())
}

fn validate_world_update(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    before: &World,
    after: &World,
) -> Result<(), DomainError> {
    if before.id() != world_id || after.id() != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if before.id() != after.id() {
        return Err(DomainError::InvalidChangeSetContext(
            "update operations must preserve the aggregate id",
        ));
    }
    if expected_version != 0 {
        return Err(DomainError::InvalidChangeSetContext(
            "world updates do not use numeric versions",
        ));
    }
    if before.current_revision() != after.current_revision() {
        return Err(DomainError::InvalidChangeSetContext(
            "world updates must preserve the base revision during draft construction",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if !affected_ids.contains(&ObjectRef::World(after.id())) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_create(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
) -> Result<(), DomainError> {
    if object_world_id != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if expected_version != 0 {
        return Err(DomainError::InvalidChangeSetContext(
            "create operations must expect version 0",
        ));
    }
    if object_version != 1 {
        return Err(DomainError::InvalidChangeSetContext(
            "created aggregates must start at version 1",
        ));
    }
    if !affected_ids.contains(&primary_ref) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_update(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    before_ref: ObjectRef,
    before_world_id: WorldId,
    before_version: u64,
    after_ref: ObjectRef,
    after_world_id: WorldId,
    after_version: u64,
) -> Result<(), DomainError> {
    if before_world_id != world_id || after_world_id != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if before_ref != after_ref {
        return Err(DomainError::InvalidChangeSetContext(
            "update operations must preserve the aggregate id",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if expected_version != before_version {
        return Err(DomainError::InvalidChangeSetContext(
            "expected version must match the current version",
        ));
    }
    let next_version = before_version
        .checked_add(1)
        .ok_or(DomainError::VersionOverflow)?;
    if after_version != next_version {
        return Err(DomainError::InvalidChangeSetContext(
            "updated aggregates must increment version by one",
        ));
    }
    if !affected_ids.contains(&after_ref) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_delete(
    world_id: WorldId,
    affected_ids: &[ObjectRef],
    expected_version: u64,
    primary_ref: ObjectRef,
    object_world_id: WorldId,
    object_version: u64,
) -> Result<(), DomainError> {
    if object_world_id != world_id {
        return Err(DomainError::InvalidChangeSetContext(
            "operation world does not match change set world",
        ));
    }
    if affected_ids.is_empty() {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must affect at least one object",
        ));
    }
    if expected_version != object_version {
        return Err(DomainError::InvalidChangeSetContext(
            "expected version must match the current version",
        ));
    }
    if !affected_ids.contains(&primary_ref) {
        return Err(DomainError::InvalidChangeSetContext(
            "an operation must include its primary object id",
        ));
    }
    validate_affected_ids(affected_ids)
}

fn validate_affected_ids(affected_ids: &[ObjectRef]) -> Result<(), DomainError> {
    let mut seen = HashSet::with_capacity(affected_ids.len());
    for affected_id in affected_ids {
        if !seen.insert(*affected_id) {
            return Err(DomainError::InvalidChangeSetContext(
                "an operation cannot repeat an affected id",
            ));
        }
    }
    Ok(())
}

fn normalize_strings(field: &'static str, values: Vec<String>) -> Result<Vec<String>, DomainError> {
    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::with_capacity(values.len());

    for value in values {
        let value = required(field, value)?;
        if !seen.insert(value.clone()) {
            return Err(DomainError::InvalidChangeSetContext(
                "duplicate string values are not allowed",
            ));
        }
        normalized.push(value);
    }

    Ok(normalized)
}

impl DecisionPoint {
    fn validate(&self, operation_ids: &HashSet<ChangeOperationId>) -> Result<(), DomainError> {
        for operation_id in &self.operation_ids {
            if !operation_ids.contains(operation_id) {
                return Err(DomainError::InvalidChangeSetContext(
                    "a decision point references an unknown operation",
                ));
            }
        }
        Ok(())
    }
}
