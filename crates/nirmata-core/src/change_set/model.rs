pub struct ChangeSetValidationSnapshot<'a> {
    pub entities: &'a [Entity],
    pub relations: &'a [Relation],
    pub goals: &'a [Goal],
    pub events: &'a [Event],
    pub event_links: &'a [EventLink],
    pub rules: &'a [Rule],
    pub claims: &'a [Claim],
    pub documents: &'a [Document],
    pub content_references: &'a [ContentReference],
    pub revisions: &'a [RevisionId],
}

impl<'a> ChangeSetValidationSnapshot<'a> {
    pub const fn empty() -> Self {
        Self {
            entities: &[],
            relations: &[],
            goals: &[],
            events: &[],
            event_links: &[],
            rules: &[],
            claims: &[],
            documents: &[],
            content_references: &[],
            revisions: &[],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetconKind {
    Additive,
    Reinterpretive,
    Replacement,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionPoint {
    decision_point_id: DecisionPointId,
    operation_ids: Vec<ChangeOperationId>,
    prompt: String,
    alternatives: Vec<String>,
    replacement_target: Option<ObjectRef>,
    reason: Option<String>,
    resolved_alternative: Option<String>,
}

impl DecisionPoint {
    pub fn new(
        operation_ids: Vec<ChangeOperationId>,
        prompt: impl Into<String>,
        alternatives: Vec<String>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            DecisionPointId::new(),
            operation_ids,
            prompt,
            alternatives,
            None,
            None,
            None,
        )
    }

    pub fn new_replacement(
        operation_ids: Vec<ChangeOperationId>,
        prompt: impl Into<String>,
        alternatives: Vec<String>,
        replacement_target: ObjectRef,
        reason: impl Into<String>,
        resolved_alternative: impl Into<String>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            DecisionPointId::new(),
            operation_ids,
            prompt,
            alternatives,
            Some(replacement_target),
            Some(reason.into()),
            Some(resolved_alternative.into()),
        )
    }

    pub fn restore(
        decision_point_id: DecisionPointId,
        operation_ids: Vec<ChangeOperationId>,
        prompt: impl Into<String>,
        alternatives: Vec<String>,
        replacement_target: Option<ObjectRef>,
        reason: Option<String>,
        resolved_alternative: Option<String>,
    ) -> Result<Self, DomainError> {
        let prompt = required("prompt", prompt)?;
        if operation_ids.is_empty() {
            return Err(DomainError::InvalidChangeSetContext(
                "a decision point must reference at least one operation",
            ));
        }
        if alternatives.len() < 2 {
            return Err(DomainError::InvalidChangeSetContext(
                "a decision point must expose at least two alternatives",
            ));
        }

        let mut seen_operations = HashSet::with_capacity(operation_ids.len());
        for operation_id in &operation_ids {
            if !seen_operations.insert(*operation_id) {
                return Err(DomainError::InvalidChangeSetContext(
                    "a decision point cannot repeat an operation id",
                ));
            }
        }

        let mut seen_alternatives = HashSet::with_capacity(alternatives.len());
        let mut normalized_alternatives = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let alternative = required("alternative", alternative)?;
            if !seen_alternatives.insert(alternative.clone()) {
                return Err(DomainError::InvalidChangeSetContext(
                    "a decision point cannot repeat an alternative",
                ));
            }
            normalized_alternatives.push(alternative);
        }

        let reason = reason.map(|value| required("reason", value)).transpose()?;
        let resolved_alternative = resolved_alternative
            .map(|value| required("resolved_alternative", value))
            .transpose()?;
        if resolved_alternative.as_deref().is_some_and(|alternative| {
            !normalized_alternatives
                .iter()
                .any(|value| value == alternative)
        }) {
            return Err(DomainError::InvalidChangeSetContext(
                "a resolved alternative must match one of the decision alternatives",
            ));
        }

        Ok(Self {
            decision_point_id,
            operation_ids,
            prompt,
            alternatives: normalized_alternatives,
            replacement_target,
            reason,
            resolved_alternative,
        })
    }

    pub fn decision_point_id(&self) -> DecisionPointId {
        self.decision_point_id
    }

    pub fn operation_ids(&self) -> &[ChangeOperationId] {
        &self.operation_ids
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn alternatives(&self) -> &[String] {
        &self.alternatives
    }

    pub fn replacement_target(&self) -> Option<ObjectRef> {
        self.replacement_target
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    pub fn resolved_alternative(&self) -> Option<&str> {
        self.resolved_alternative.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ChangeOperation {
    UpdateWorld {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: World,
        after: World,
    },
    CreateEntity {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Entity,
    },
    UpdateEntity {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Entity,
        after: Entity,
    },
    DeleteEntity {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Entity,
    },
    CreateRelation {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Relation,
    },
    UpdateRelation {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Relation,
        after: Relation,
    },
    DeleteRelation {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Relation,
    },
    CreateEvent {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: EventAggregate,
    },
    UpdateEvent {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: EventAggregate,
        after: EventAggregate,
    },
    DeleteEvent {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: EventAggregate,
    },
    CreateGoal {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Goal,
    },
    UpdateGoal {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Goal,
        after: Goal,
    },
    DeleteGoal {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Goal,
    },
    CreateRule {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Rule,
    },
    UpdateRule {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Rule,
        after: Rule,
    },
    DeleteRule {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Rule,
    },
    CreateClaim {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: Claim,
    },
    UpdateClaim {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Claim,
        after: Claim,
    },
    DeleteClaim {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: Claim,
    },
    CreateDocument {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        after: DocumentAggregate,
    },
    UpdateDocument {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: DocumentAggregate,
        after: DocumentAggregate,
    },
    DeleteDocument {
        operation_id: ChangeOperationId,
        affected_ids: Vec<ObjectRef>,
        expected_version: u64,
        retcon: RetconKind,
        before: DocumentAggregate,
    },
}

impl ChangeOperation {
    pub fn operation_id(&self) -> ChangeOperationId {
        match self {
            Self::UpdateWorld { operation_id, .. }
            | Self::CreateEntity { operation_id, .. }
            | Self::UpdateEntity { operation_id, .. }
            | Self::DeleteEntity { operation_id, .. }
            | Self::CreateRelation { operation_id, .. }
            | Self::UpdateRelation { operation_id, .. }
            | Self::DeleteRelation { operation_id, .. }
            | Self::CreateEvent { operation_id, .. }
            | Self::UpdateEvent { operation_id, .. }
            | Self::DeleteEvent { operation_id, .. }
            | Self::CreateGoal { operation_id, .. }
            | Self::UpdateGoal { operation_id, .. }
            | Self::DeleteGoal { operation_id, .. }
            | Self::CreateRule { operation_id, .. }
            | Self::UpdateRule { operation_id, .. }
            | Self::DeleteRule { operation_id, .. }
            | Self::CreateClaim { operation_id, .. }
            | Self::UpdateClaim { operation_id, .. }
            | Self::DeleteClaim { operation_id, .. }
            | Self::CreateDocument { operation_id, .. }
            | Self::UpdateDocument { operation_id, .. }
            | Self::DeleteDocument { operation_id, .. } => *operation_id,
        }
    }

    pub fn retcon(&self) -> RetconKind {
        match self {
            Self::UpdateWorld { retcon, .. }
            | Self::CreateEntity { retcon, .. }
            | Self::UpdateEntity { retcon, .. }
            | Self::DeleteEntity { retcon, .. }
            | Self::CreateRelation { retcon, .. }
            | Self::UpdateRelation { retcon, .. }
            | Self::DeleteRelation { retcon, .. }
            | Self::CreateEvent { retcon, .. }
            | Self::UpdateEvent { retcon, .. }
            | Self::DeleteEvent { retcon, .. }
            | Self::CreateGoal { retcon, .. }
            | Self::UpdateGoal { retcon, .. }
            | Self::DeleteGoal { retcon, .. }
            | Self::CreateRule { retcon, .. }
            | Self::UpdateRule { retcon, .. }
            | Self::DeleteRule { retcon, .. }
            | Self::CreateClaim { retcon, .. }
            | Self::UpdateClaim { retcon, .. }
            | Self::DeleteClaim { retcon, .. }
            | Self::CreateDocument { retcon, .. }
            | Self::UpdateDocument { retcon, .. }
            | Self::DeleteDocument { retcon, .. } => *retcon,
        }
    }

    pub fn expected_version(&self) -> u64 {
        match self {
            Self::UpdateWorld {
                expected_version, ..
            }
            | Self::CreateEntity {
                expected_version, ..
            }
            | Self::UpdateEntity {
                expected_version, ..
            }
            | Self::DeleteEntity {
                expected_version, ..
            }
            | Self::CreateRelation {
                expected_version, ..
            }
            | Self::UpdateRelation {
                expected_version, ..
            }
            | Self::DeleteRelation {
                expected_version, ..
            }
            | Self::CreateEvent {
                expected_version, ..
            }
            | Self::UpdateEvent {
                expected_version, ..
            }
            | Self::DeleteEvent {
                expected_version, ..
            }
            | Self::CreateGoal {
                expected_version, ..
            }
            | Self::UpdateGoal {
                expected_version, ..
            }
            | Self::DeleteGoal {
                expected_version, ..
            }
            | Self::CreateRule {
                expected_version, ..
            }
            | Self::UpdateRule {
                expected_version, ..
            }
            | Self::DeleteRule {
                expected_version, ..
            }
            | Self::CreateClaim {
                expected_version, ..
            }
            | Self::UpdateClaim {
                expected_version, ..
            }
            | Self::DeleteClaim {
                expected_version, ..
            }
            | Self::CreateDocument {
                expected_version, ..
            }
            | Self::UpdateDocument {
                expected_version, ..
            }
            | Self::DeleteDocument {
                expected_version, ..
            } => *expected_version,
        }
    }

    pub fn affected_ids(&self) -> &[ObjectRef] {
        match self {
            Self::UpdateWorld { affected_ids, .. }
            | Self::CreateEntity { affected_ids, .. }
            | Self::UpdateEntity { affected_ids, .. }
            | Self::DeleteEntity { affected_ids, .. }
            | Self::CreateRelation { affected_ids, .. }
            | Self::UpdateRelation { affected_ids, .. }
            | Self::DeleteRelation { affected_ids, .. }
            | Self::CreateEvent { affected_ids, .. }
            | Self::UpdateEvent { affected_ids, .. }
            | Self::DeleteEvent { affected_ids, .. }
            | Self::CreateGoal { affected_ids, .. }
            | Self::UpdateGoal { affected_ids, .. }
            | Self::DeleteGoal { affected_ids, .. }
            | Self::CreateRule { affected_ids, .. }
            | Self::UpdateRule { affected_ids, .. }
            | Self::DeleteRule { affected_ids, .. }
            | Self::CreateClaim { affected_ids, .. }
            | Self::UpdateClaim { affected_ids, .. }
            | Self::DeleteClaim { affected_ids, .. }
            | Self::CreateDocument { affected_ids, .. }
            | Self::UpdateDocument { affected_ids, .. }
            | Self::DeleteDocument { affected_ids, .. } => affected_ids,
        }
    }

    pub fn primary_ref(&self) -> ObjectRef {
        match self {
            Self::UpdateWorld { before, .. } => ObjectRef::World(before.id()),
            Self::CreateEntity { after, .. } => ObjectRef::Entity(after.id()),
            Self::UpdateEntity { before, .. } | Self::DeleteEntity { before, .. } => {
                ObjectRef::Entity(before.id())
            }
            Self::CreateRelation { after, .. } => ObjectRef::Relation(after.id()),
            Self::UpdateRelation { before, .. } | Self::DeleteRelation { before, .. } => {
                ObjectRef::Relation(before.id())
            }
            Self::CreateEvent { after, .. } => ObjectRef::Event(after.event().id()),
            Self::UpdateEvent { before, .. } | Self::DeleteEvent { before, .. } => {
                ObjectRef::Event(before.event().id())
            }
            Self::CreateGoal { after, .. } => ObjectRef::Goal(after.id()),
            Self::UpdateGoal { before, .. } | Self::DeleteGoal { before, .. } => {
                ObjectRef::Goal(before.id())
            }
            Self::CreateRule { after, .. } => ObjectRef::Rule(after.id()),
            Self::UpdateRule { before, .. } | Self::DeleteRule { before, .. } => {
                ObjectRef::Rule(before.id())
            }
            Self::CreateClaim { after, .. } => ObjectRef::Claim(after.id()),
            Self::UpdateClaim { before, .. } | Self::DeleteClaim { before, .. } => {
                ObjectRef::Claim(before.id())
            }
            Self::CreateDocument { after, .. } => ObjectRef::Document(after.object().id()),
            Self::UpdateDocument { before, .. } | Self::DeleteDocument { before, .. } => {
                ObjectRef::Document(before.object().id())
            }
        }
    }

    fn created_ref(&self) -> Option<ObjectRef> {
        match self {
            Self::CreateEntity { after, .. } => Some(ObjectRef::Entity(after.id())),
            Self::CreateRelation { after, .. } => Some(ObjectRef::Relation(after.id())),
            Self::CreateEvent { after, .. } => Some(ObjectRef::Event(after.event().id())),
            Self::CreateGoal { after, .. } => Some(ObjectRef::Goal(after.id())),
            Self::CreateRule { after, .. } => Some(ObjectRef::Rule(after.id())),
            Self::CreateClaim { after, .. } => Some(ObjectRef::Claim(after.id())),
            Self::CreateDocument { after, .. } => Some(ObjectRef::Document(after.object().id())),
            _ => None,
        }
    }

    fn validate(&self, world_id: WorldId) -> Result<(), DomainError> {
        match self {
            Self::UpdateWorld {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_world_update(world_id, affected_ids, *expected_version, before, after),
            Self::CreateEntity {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Entity(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateEntity {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Entity(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Entity(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteEntity {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Entity(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateRelation {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Relation(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateRelation {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Relation(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Relation(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteRelation {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Relation(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateEvent {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Event(after.event().id()),
                after.event().world_id(),
                after.event().version(),
            ),
            Self::UpdateEvent {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Event(before.event().id()),
                before.event().world_id(),
                before.event().version(),
                ObjectRef::Event(after.event().id()),
                after.event().world_id(),
                after.event().version(),
            ),
            Self::DeleteEvent {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Event(before.event().id()),
                before.event().world_id(),
                before.event().version(),
            ),
            Self::CreateGoal {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Goal(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateGoal {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Goal(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Goal(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteGoal {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Goal(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateRule {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Rule(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateRule {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Rule(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Rule(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteRule {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Rule(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateClaim {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Claim(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::UpdateClaim {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Claim(before.id()),
                before.world_id(),
                before.version(),
                ObjectRef::Claim(after.id()),
                after.world_id(),
                after.version(),
            ),
            Self::DeleteClaim {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Claim(before.id()),
                before.world_id(),
                before.version(),
            ),
            Self::CreateDocument {
                affected_ids,
                expected_version,
                after,
                ..
            } => validate_create(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Document(after.object().id()),
                after.object().world_id(),
                after.object().version(),
            ),
            Self::UpdateDocument {
                affected_ids,
                expected_version,
                before,
                after,
                ..
            } => validate_update(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Document(before.object().id()),
                before.object().world_id(),
                before.object().version(),
                ObjectRef::Document(after.object().id()),
                after.object().world_id(),
                after.object().version(),
            ),
            Self::DeleteDocument {
                affected_ids,
                expected_version,
                before,
                ..
            } => validate_delete(
                world_id,
                affected_ids,
                *expected_version,
                ObjectRef::Document(before.object().id()),
                before.object().world_id(),
                before.object().version(),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSetDraft {
    id: ChangeSetId,
    world_id: WorldId,
    base_revision: RevisionId,
    objective: String,
    sources: Vec<ObjectRef>,
    assumptions: Vec<String>,
    operations: Vec<ChangeOperation>,
    decisions: Vec<DecisionPoint>,
}

impl ChangeSetDraft {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            ChangeSetId::new(),
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: ChangeSetId,
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        let objective = required("objective", objective)?;
        let assumptions = normalize_strings("assumption", assumptions)?;
        validate_operations_and_decisions(world_id, &operations, &decisions)?;

        Ok(Self {
            id,
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        })
    }

    pub fn id(&self) -> ChangeSetId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn base_revision(&self) -> RevisionId {
        self.base_revision
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn sources(&self) -> &[ObjectRef] {
        &self.sources
    }

    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn operations(&self) -> &[ChangeOperation] {
        &self.operations
    }

    pub fn decisions(&self) -> &[DecisionPoint] {
        &self.decisions
    }

    pub fn validation_report(
        &self,
        snapshot: &ChangeSetValidationSnapshot<'_>,
    ) -> ValidationReport {
        validate_change_set_parts(
            self.world_id,
            self.base_revision,
            &self.objective,
            &self.sources,
            &self.assumptions,
            &self.operations,
            &self.decisions,
            snapshot,
            false,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    id: ChangeSetId,
    world_id: WorldId,
    base_revision: RevisionId,
    objective: String,
    sources: Vec<ObjectRef>,
    assumptions: Vec<String>,
    operations: Vec<ChangeOperation>,
    decisions: Vec<DecisionPoint>,
}

impl ChangeSet {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        Self::restore(
            ChangeSetId::new(),
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: ChangeSetId,
        world_id: WorldId,
        base_revision: RevisionId,
        objective: impl Into<String>,
        sources: Vec<ObjectRef>,
        assumptions: Vec<String>,
        operations: Vec<ChangeOperation>,
        decisions: Vec<DecisionPoint>,
    ) -> Result<Self, DomainError> {
        let objective = required("objective", objective)?;
        let assumptions = normalize_strings("assumption", assumptions)?;
        validate_operations_and_decisions(world_id, &operations, &decisions)?;

        Ok(Self {
            id,
            world_id,
            base_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        })
    }

    pub fn id(&self) -> ChangeSetId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn base_revision(&self) -> RevisionId {
        self.base_revision
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn sources(&self) -> &[ObjectRef] {
        &self.sources
    }

    pub fn assumptions(&self) -> &[String] {
        &self.assumptions
    }

    pub fn operations(&self) -> &[ChangeOperation] {
        &self.operations
    }

    pub fn decisions(&self) -> &[DecisionPoint] {
        &self.decisions
    }

    pub fn validation_report(
        &self,
        snapshot: &ChangeSetValidationSnapshot<'_>,
    ) -> ValidationReport {
        validate_change_set_parts(
            self.world_id,
            self.base_revision,
            &self.objective,
            &self.sources,
            &self.assumptions,
            &self.operations,
            &self.decisions,
            snapshot,
            true,
        )
    }
}
