use crate::claim::{Claim, ClaimAuthentication, ClaimObject};
use crate::document::{ContentReference, Document, ObjectRef};
use crate::entity::Entity;
use crate::event::{Event, EventLink};
use crate::goal::Goal;
use crate::relation::Relation;
use crate::rule::{Rule, RuleSeverity, RuleValidatorKind};
use crate::time::PartialTruth;
use crate::{Period, RevisionId, WorldId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Conflict,
    Warning,
    Info,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct IssueObject {
    pub kind: String,
    pub id: String,
}

impl IssueObject {
    pub fn new(kind: impl Into<String>, id: impl ToString) -> Self {
        Self {
            kind: kind.into(),
            id: id.to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub code: String,
    pub severity: ValidationSeverity,
    pub objects: Vec<IssueObject>,
    pub message: String,
}

impl ValidationIssue {
    pub(crate) fn new(
        code: &'static str,
        severity: ValidationSeverity,
        objects: Vec<IssueObject>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_owned(),
            severity,
            objects,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub errors: Vec<ValidationIssue>,
    pub conflicts: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
    pub info: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_issues(issues: impl IntoIterator<Item = ValidationIssue>) -> Self {
        let mut report = Self::new();
        report.extend(issues);
        report
    }

    pub fn push(&mut self, issue: ValidationIssue) {
        match issue.severity {
            ValidationSeverity::Error => self.errors.push(issue),
            ValidationSeverity::Conflict => self.conflicts.push(issue),
            ValidationSeverity::Warning => self.warnings.push(issue),
            ValidationSeverity::Info => self.info.push(issue),
        }
    }

    pub fn extend(&mut self, issues: impl IntoIterator<Item = ValidationIssue>) {
        for issue in issues {
            self.push(issue);
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn is_ok(&self) -> bool {
        !self.has_errors() && !self.has_conflicts()
    }
}

pub fn validate_rules(rules: &[Rule]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut ids = HashSet::new();

    for rule in rules {
        let object = IssueObject::new("rule", rule.id());
        if !ids.insert(rule.id()) {
            issues.push(ValidationIssue::new(
                "rule.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "rule ID is duplicated",
            ));
        }
        if rule.version() == 0 {
            issues.push(invalid_version(object.clone()));
        }
        if rule.severity() == RuleSeverity::Hard && rule.validator_kind().is_none() {
            issues.push(ValidationIssue::new(
                "rule.hard_without_validator",
                ValidationSeverity::Error,
                vec![object.clone()],
                "hard rules require an implemented validator",
            ));
        }
        if matches!(
            rule.validator_kind(),
            Some(RuleValidatorKind::NoResurrection)
        ) && !rule.parameters_json().is_empty()
        {
            issues.push(ValidationIssue::new(
                "rule.invalid_validator_parameters",
                ValidationSeverity::Error,
                vec![object],
                "no_resurrection does not accept parameters",
            ));
        }
    }

    issues
}

pub fn validate_entities(entities: &[Entity]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut ids = HashSet::new();
    let mut slugs = HashMap::new();

    for entity in entities {
        let object = IssueObject::new("entity", entity.id());
        if !ids.insert(entity.id()) {
            issues.push(ValidationIssue::new(
                "entity.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "entity ID is duplicated",
            ));
        }
        if let Some(previous) = slugs.insert((entity.world_id(), entity.slug()), entity.id()) {
            issues.push(ValidationIssue::new(
                "entity.duplicate_slug",
                ValidationSeverity::Error,
                vec![IssueObject::new("entity", previous), object.clone()],
                "entity slug must be unique within a world",
            ));
        }
        if entity.version() == 0 {
            issues.push(invalid_version(object));
        }
    }

    issues
}

pub fn validate_relations(
    relations: &[Relation],
    entities: &[Entity],
    self_relation_kinds: &[&str],
) -> Vec<ValidationIssue> {
    let entity_worlds: HashMap<_, _> = entities
        .iter()
        .map(|entity| (entity.id(), entity.world_id()))
        .collect();
    let mut issues = Vec::new();
    let mut ids = HashSet::new();

    for (index, relation) in relations.iter().enumerate() {
        let object = IssueObject::new("relation", relation.id());
        if !ids.insert(relation.id()) {
            issues.push(ValidationIssue::new(
                "relation.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "relation ID is duplicated",
            ));
        }
        validate_entity_reference(
            relation.world_id(),
            relation.source_entity_id(),
            &entity_worlds,
            object.clone(),
            "relation.source_missing",
            &mut issues,
        );
        validate_entity_reference(
            relation.world_id(),
            relation.target_entity_id(),
            &entity_worlds,
            object.clone(),
            "relation.target_missing",
            &mut issues,
        );
        if relation.source_entity_id() == relation.target_entity_id()
            && !self_relation_kinds.contains(&relation.kind())
        {
            issues.push(ValidationIssue::new(
                "relation.self_not_allowed",
                ValidationSeverity::Error,
                vec![object.clone()],
                "this relation kind does not allow a self-relation",
            ));
        }
        if matches!(
            (relation.valid_from_tick(), relation.valid_to_tick()),
            (Some(start), Some(end)) if start > end
        ) {
            issues.push(ValidationIssue::new(
                "period.inverted",
                ValidationSeverity::Error,
                vec![object.clone()],
                "relation period starts after it ends",
            ));
        }
        if relation.version() == 0 {
            issues.push(invalid_version(object.clone()));
        }

        for duplicate in &relations[..index] {
            if relation.exactly_matches(duplicate) {
                issues.push(ValidationIssue::new(
                    "relation.duplicate",
                    ValidationSeverity::Error,
                    vec![IssueObject::new("relation", duplicate.id()), object.clone()],
                    "exact duplicate relation",
                ));
            }
        }
    }

    issues
}

pub fn validate_goals(goals: &[Goal], entities: &[Entity]) -> Vec<ValidationIssue> {
    let entity_worlds: HashMap<_, _> = entities
        .iter()
        .map(|entity| (entity.id(), entity.world_id()))
        .collect();
    let mut issues = Vec::new();
    let mut ids = HashSet::new();

    for goal in goals {
        let object = IssueObject::new("goal", goal.id());
        if !ids.insert(goal.id()) {
            issues.push(ValidationIssue::new(
                "goal.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "goal ID is duplicated",
            ));
        }
        validate_entity_reference(
            goal.world_id(),
            goal.holder_entity_id(),
            &entity_worlds,
            object.clone(),
            "goal.holder_missing",
            &mut issues,
        );
        if goal.period().is_some_and(|period| !period.is_ordered()) {
            issues.push(inverted_period(object.clone(), "goal"));
        }
        if goal.version() == 0 {
            issues.push(invalid_version(object));
        }
    }

    issues
}

pub fn validate_events(
    events: &[Event],
    entities: &[Entity],
    goals: &[Goal],
) -> Vec<ValidationIssue> {
    let entity_worlds: HashMap<_, _> = entities
        .iter()
        .map(|entity| (entity.id(), entity.world_id()))
        .collect();
    let goal_worlds: HashMap<_, _> = goals
        .iter()
        .map(|goal| (goal.id(), goal.world_id()))
        .collect();
    let mut issues = Vec::new();
    let mut ids = HashSet::new();

    for event in events {
        let object = IssueObject::new("event", event.id());
        if !ids.insert(event.id()) {
            issues.push(ValidationIssue::new(
                "event.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "event ID is duplicated",
            ));
        }
        if let Some(location_id) = event.location_entity_id() {
            validate_entity_reference(
                event.world_id(),
                location_id,
                &entity_worlds,
                object.clone(),
                "event.location_missing",
                &mut issues,
            );
        }

        let mut ordinals = HashSet::new();
        for participant in event.participants() {
            validate_entity_reference(
                event.world_id(),
                participant.entity_id(),
                &entity_worlds,
                object.clone(),
                "event.participant_missing",
                &mut issues,
            );
            if !ordinals.insert(participant.ordinal()) {
                issues.push(ValidationIssue::new(
                    "event.participant_ordinal_duplicate",
                    ValidationSeverity::Error,
                    vec![object.clone()],
                    format!(
                        "participant ordinal {} is duplicated",
                        participant.ordinal()
                    ),
                ));
            }
        }

        let mut affected_goals = HashSet::new();
        for goal_id in event.affected_goal_ids() {
            match goal_worlds.get(goal_id) {
                None => issues.push(ValidationIssue::new(
                    "event.goal_missing",
                    ValidationSeverity::Error,
                    vec![object.clone()],
                    format!("affected goal {goal_id} does not exist"),
                )),
                Some(world_id) if *world_id != event.world_id() => {
                    issues.push(cross_world(object.clone(), "affected goal"))
                }
                Some(_) => {}
            }
            if !affected_goals.insert(*goal_id) {
                issues.push(ValidationIssue::new(
                    "event.goal_duplicate",
                    ValidationSeverity::Error,
                    vec![object.clone()],
                    format!("affected goal {goal_id} is duplicated"),
                ));
            }
        }

        if event.time().validate().is_err() {
            issues.push(ValidationIssue::new(
                "event.time_invalid",
                ValidationSeverity::Error,
                vec![object.clone()],
                "event time fields do not match its kind",
            ));
        }
        if event.version() == 0 {
            issues.push(invalid_version(object));
        }
    }

    issues
}

pub fn validate_event_links(links: &[EventLink], events: &[Event]) -> Vec<ValidationIssue> {
    let event_by_id: HashMap<_, _> = events.iter().map(|event| (event.id(), event)).collect();
    let mut issues = Vec::new();
    let mut seen = HashSet::new();

    for link in links {
        let source = event_by_id.get(&link.source_event_id()).copied();
        let target = event_by_id.get(&link.target_event_id()).copied();
        let objects = vec![
            IssueObject::new("event", link.source_event_id()),
            IssueObject::new("event", link.target_event_id()),
        ];

        if link.source_event_id() == link.target_event_id() {
            issues.push(ValidationIssue::new(
                "causality.self_reference",
                ValidationSeverity::Error,
                objects.clone(),
                "an event cannot causally link to itself",
            ));
        }
        if source.is_none() || target.is_none() {
            issues.push(ValidationIssue::new(
                "causality.event_missing",
                ValidationSeverity::Error,
                objects.clone(),
                "causal link references an event that does not exist",
            ));
            continue;
        }
        let (source, target) = (source.expect("checked"), target.expect("checked"));
        if source.world_id() != target.world_id() {
            issues.push(ValidationIssue::new(
                "reference.cross_world",
                ValidationSeverity::Error,
                objects.clone(),
                "causal events belong to different worlds",
            ));
        }
        if !seen.insert((link.source_event_id(), link.target_event_id(), link.kind())) {
            issues.push(ValidationIssue::new(
                "causality.duplicate",
                ValidationSeverity::Error,
                objects.clone(),
                "causal link is duplicated",
            ));
        }
        if source.time().after(target.time()) == PartialTruth::True {
            issues.push(ValidationIssue::new(
                "causality.cause_after_effect",
                ValidationSeverity::Conflict,
                objects,
                "causal source occurs after its target",
            ));
        }
    }

    issues
}

pub fn validate_claims(
    claims: &[Claim],
    entities: &[Entity],
    documents: &[Document],
    revisions: &HashSet<RevisionId>,
) -> Vec<ValidationIssue> {
    let entity_worlds: HashMap<_, _> = entities
        .iter()
        .map(|entity| (entity.id(), entity.world_id()))
        .collect();
    let document_worlds: HashMap<_, _> = documents
        .iter()
        .map(|document| (document.id(), document.world_id()))
        .collect();
    let claim_by_id: HashMap<_, _> = claims.iter().map(|claim| (claim.id(), claim)).collect();
    let mut issues = Vec::new();
    let mut ids = HashSet::new();

    for claim in claims {
        let object = IssueObject::new("claim", claim.id());
        if !ids.insert(claim.id()) {
            issues.push(ValidationIssue::new(
                "claim.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "claim ID is duplicated",
            ));
        }
        for (entity_id, code) in [
            (Some(claim.subject_entity_id()), "claim.subject_missing"),
            (claim.holder_entity_id(), "claim.holder_missing"),
            (
                match claim.object() {
                    Some(ClaimObject::Entity(entity_id)) => Some(*entity_id),
                    _ => None,
                },
                "claim.object_missing",
            ),
        ] {
            if let Some(entity_id) = entity_id {
                validate_entity_reference(
                    claim.world_id(),
                    entity_id,
                    &entity_worlds,
                    object.clone(),
                    code,
                    &mut issues,
                );
            }
        }

        if let Some(document_id) = claim.source_document_id() {
            match document_worlds.get(&document_id) {
                None => issues.push(missing_reference(
                    object.clone(),
                    "claim.source_document_missing",
                    "source document",
                    document_id,
                )),
                Some(world_id) if *world_id != claim.world_id() => {
                    issues.push(cross_world(object.clone(), "source document"))
                }
                Some(_) => {}
            }
        }
        if let Some(source_claim_id) = claim.source_claim_id() {
            match claim_by_id.get(&source_claim_id) {
                None => issues.push(missing_reference(
                    object.clone(),
                    "claim.source_claim_missing",
                    "source claim",
                    source_claim_id,
                )),
                Some(source_claim) if source_claim.world_id() != claim.world_id() => {
                    issues.push(cross_world(object.clone(), "source claim"))
                }
                Some(_) if source_claim_id == claim.id() => issues.push(ValidationIssue::new(
                    "claim.self_provenance",
                    ValidationSeverity::Error,
                    vec![object.clone()],
                    "a claim cannot derive from itself",
                )),
                Some(_) => {}
            }
        }
        for revision_id in [
            Some(claim.registered_revision_id()),
            claim.superseded_revision_id(),
        ]
        .into_iter()
        .flatten()
        {
            if !revisions.contains(&revision_id) {
                issues.push(missing_reference(
                    object.clone(),
                    "claim.revision_missing",
                    "revision",
                    revision_id,
                ));
            }
        }
        if claim.period().is_some_and(|period| !period.is_ordered()) {
            issues.push(inverted_period(object.clone(), "claim"));
        }
        if claim.version() == 0 {
            issues.push(invalid_version(object));
        }
    }

    for (index, claim) in claims.iter().enumerate() {
        if claim.authentication() != ClaimAuthentication::Canonical || !claim.is_active() {
            continue;
        }
        for other in &claims[..index] {
            if canonical_claims_oppose(claim, other) {
                issues.push(ValidationIssue::new(
                    "claim.canonical_opposition",
                    ValidationSeverity::Conflict,
                    vec![
                        IssueObject::new("claim", other.id()),
                        IssueObject::new("claim", claim.id()),
                    ],
                    "active canonical claims have opposite polarities",
                ));
            }
        }
    }

    issues
}

pub fn canonical_claims_oppose(left: &Claim, right: &Claim) -> bool {
    left.authentication() == ClaimAuthentication::Canonical
        && right.authentication() == ClaimAuthentication::Canonical
        && left.is_active()
        && right.is_active()
        && left.polarity() != right.polarity()
        && left.has_same_normalized_proposition(right)
        && periods_overlap(left.period(), right.period())
}

pub fn validate_documents(documents: &[Document], entities: &[Entity]) -> Vec<ValidationIssue> {
    let entity_worlds: HashMap<_, _> = entities
        .iter()
        .map(|entity| (entity.id(), entity.world_id()))
        .collect();
    let mut issues = Vec::new();
    let mut ids = HashSet::new();

    for document in documents {
        let object = IssueObject::new("document", document.id());
        if !ids.insert(document.id()) {
            issues.push(ValidationIssue::new(
                "document.duplicate_id",
                ValidationSeverity::Error,
                vec![object.clone()],
                "document ID is duplicated",
            ));
        }
        for (entity_id, code) in [
            (document.author_entity_id(), "document.author_missing"),
            (
                document.perspective_entity_id(),
                "document.perspective_missing",
            ),
        ] {
            if let Some(entity_id) = entity_id {
                validate_entity_reference(
                    document.world_id(),
                    entity_id,
                    &entity_worlds,
                    object.clone(),
                    code,
                    &mut issues,
                );
            }
        }
        if document.version() == 0 {
            issues.push(invalid_version(object));
        }
    }

    issues
}

#[allow(clippy::too_many_arguments)]
pub fn validate_content_references(
    references: &[ContentReference],
    rules: &[Rule],
    entities: &[Entity],
    relations: &[Relation],
    goals: &[Goal],
    events: &[Event],
    claims: &[Claim],
    documents: &[Document],
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut ordinals = HashSet::new();

    for reference in references {
        let source_world = object_world(
            reference.source(),
            rules,
            entities,
            relations,
            goals,
            events,
            claims,
            documents,
        );
        let target_world = object_world(
            reference.target(),
            rules,
            entities,
            relations,
            goals,
            events,
            claims,
            documents,
        );
        let objects = vec![
            issue_object(reference.source()),
            issue_object(reference.target()),
        ];

        if source_world.is_none() {
            issues.push(ValidationIssue::new(
                "content_reference.source_missing",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference source does not exist",
            ));
        }
        if target_world.is_none() {
            issues.push(ValidationIssue::new(
                "content_reference.target_missing",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference target does not exist",
            ));
        }
        if matches!((source_world, target_world), (Some(source), Some(target)) if source != target)
        {
            issues.push(ValidationIssue::new(
                "reference.cross_world",
                ValidationSeverity::Error,
                objects.clone(),
                "content reference crosses world boundaries",
            ));
        }
        if !ordinals.insert((reference.source(), reference.ordinal())) {
            issues.push(ValidationIssue::new(
                "content_reference.ordinal_duplicate",
                ValidationSeverity::Error,
                objects,
                format!(
                    "content ordinal {} is duplicated for its source",
                    reference.ordinal()
                ),
            ));
        }
    }

    issues
}

pub fn validate_lifecycle(
    entity: &Entity,
    birth: Option<&Event>,
    death: Option<&Event>,
    participations: &[&Event],
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let entity_object = IssueObject::new("entity", entity.id());

    if let (Some(birth), Some(death)) = (birth, death)
        && birth.time().after(death.time()) == PartialTruth::True
    {
        issues.push(ValidationIssue::new(
            "lifecycle.death_before_birth",
            ValidationSeverity::Error,
            vec![
                entity_object.clone(),
                IssueObject::new("event", birth.id()),
                IssueObject::new("event", death.id()),
            ],
            "death occurs before birth",
        ));
    }

    if let Some(birth) = birth {
        for participation in participations {
            if birth.time().after(participation.time()) == PartialTruth::True {
                issues.push(ValidationIssue::new(
                    "lifecycle.participation_before_birth",
                    ValidationSeverity::Conflict,
                    vec![
                        entity_object.clone(),
                        IssueObject::new("event", birth.id()),
                        IssueObject::new("event", participation.id()),
                    ],
                    "entity participates in an event before its known birth",
                ));
            }
        }
    }

    if let Some(death) = death {
        for participation in participations {
            if participation.time().after(death.time()) == PartialTruth::True {
                issues.push(ValidationIssue::new(
                    "lifecycle.participation_after_death",
                    ValidationSeverity::Conflict,
                    vec![
                        entity_object.clone(),
                        IssueObject::new("event", death.id()),
                        IssueObject::new("event", participation.id()),
                    ],
                    "entity participates in an event after its known death",
                ));
            }
        }
    }

    issues
}

pub fn validate_no_resurrection(
    rule: &Rule,
    entity: &Entity,
    death: &Event,
    participations: &[&Event],
) -> Vec<ValidationIssue> {
    if !(rule.can_produce_hard_error()
        && matches!(
            rule.validator_kind(),
            Some(RuleValidatorKind::NoResurrection)
        ))
    {
        return Vec::new();
    }

    let mut issues = Vec::new();
    for participation in participations {
        if participation.time().after(death.time()) == PartialTruth::True {
            issues.push(ValidationIssue::new(
                "rule.no_resurrection",
                ValidationSeverity::Error,
                vec![
                    IssueObject::new("rule", rule.id()),
                    IssueObject::new("entity", entity.id()),
                    IssueObject::new("event", death.id()),
                    IssueObject::new("event", participation.id()),
                ],
                "no_resurrection forbids participation after a known death",
            ));
        }
    }

    issues
}

pub fn validate_expected_version(
    object: IssueObject,
    actual: u64,
    expected: u64,
) -> Option<ValidationIssue> {
    (actual != expected).then(|| {
        ValidationIssue::new(
            "version.mismatch",
            ValidationSeverity::Error,
            vec![object],
            format!("expected version {expected}, found {actual}"),
        )
    })
}

pub fn relation_active_at(relation: &Relation, tick: i64) -> PartialTruth {
    if relation.valid_from_tick().is_some_and(|start| tick < start)
        || relation.valid_to_tick().is_some_and(|end| tick > end)
    {
        PartialTruth::False
    } else if relation.valid_from_tick().is_none() && relation.valid_to_tick().is_none() {
        PartialTruth::Unspecified
    } else {
        PartialTruth::True
    }
}

fn validate_entity_reference(
    owner_world: WorldId,
    entity_id: crate::EntityId,
    entity_worlds: &HashMap<crate::EntityId, WorldId>,
    owner: IssueObject,
    missing_code: &'static str,
    issues: &mut Vec<ValidationIssue>,
) {
    match entity_worlds.get(&entity_id) {
        None => issues.push(missing_reference(owner, missing_code, "entity", entity_id)),
        Some(world_id) if *world_id != owner_world => {
            issues.push(cross_world(owner, "entity reference"))
        }
        Some(_) => {}
    }
}

fn periods_overlap(left: Option<Period>, right: Option<Period>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.overlaps(right),
        _ => true,
    }
}

#[allow(clippy::too_many_arguments)]
fn object_world(
    object: ObjectRef,
    rules: &[Rule],
    entities: &[Entity],
    relations: &[Relation],
    goals: &[Goal],
    events: &[Event],
    claims: &[Claim],
    documents: &[Document],
) -> Option<WorldId> {
    match object {
        ObjectRef::World(id) => Some(id),
        ObjectRef::Rule(id) => rules
            .iter()
            .find(|rule| rule.id() == id)
            .map(Rule::world_id),
        ObjectRef::Entity(id) => entities
            .iter()
            .find(|entity| entity.id() == id)
            .map(Entity::world_id),
        ObjectRef::Relation(id) => relations
            .iter()
            .find(|relation| relation.id() == id)
            .map(Relation::world_id),
        ObjectRef::Goal(id) => goals
            .iter()
            .find(|goal| goal.id() == id)
            .map(Goal::world_id),
        ObjectRef::Event(id) => events
            .iter()
            .find(|event| event.id() == id)
            .map(Event::world_id),
        ObjectRef::Claim(id) => claims
            .iter()
            .find(|claim| claim.id() == id)
            .map(Claim::world_id),
        ObjectRef::Document(id) => documents
            .iter()
            .find(|document| document.id() == id)
            .map(Document::world_id),
    }
}

fn issue_object(object: ObjectRef) -> IssueObject {
    let id = object.to_string();
    IssueObject::new(object.kind(), id.rsplit('/').next().unwrap_or(&id))
}

fn missing_reference(
    owner: IssueObject,
    code: &'static str,
    kind: &'static str,
    id: impl ToString,
) -> ValidationIssue {
    ValidationIssue::new(
        code,
        ValidationSeverity::Error,
        vec![owner],
        format!("{kind} {} does not exist", id.to_string()),
    )
}

fn cross_world(owner: IssueObject, reference: &'static str) -> ValidationIssue {
    ValidationIssue::new(
        "reference.cross_world",
        ValidationSeverity::Error,
        vec![owner],
        format!("{reference} belongs to another world"),
    )
}

fn inverted_period(object: IssueObject, kind: &'static str) -> ValidationIssue {
    ValidationIssue::new(
        "period.inverted",
        ValidationSeverity::Error,
        vec![object],
        format!("{kind} period starts after it ends"),
    )
}

fn invalid_version(object: IssueObject) -> ValidationIssue {
    ValidationIssue::new(
        "version.invalid",
        ValidationSeverity::Error,
        vec![object],
        "version must be greater than zero",
    )
}

#[cfg(test)]
#[path = "../tests/unit/validation/mod.rs"]
mod tests;
