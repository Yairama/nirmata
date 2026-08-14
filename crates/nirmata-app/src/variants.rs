use crate::{
    AppError, ManualReviewSession, ManualReviewSnapshot, NirmataApp,
    app::{StoredManualReview, ensure_active_write_scope},
};
use nirmata_core::{
    ChangeOperationId, DecisionPointId, RevisionId, World,
    change_set::{ChangeOperation, ChangeSetDraft, DecisionPoint, RetconKind},
    claim::Claim,
    document::{DocumentAggregate, ObjectRef},
    entity::Entity,
    event::EventAggregate,
    goal::Goal,
    relation::Relation,
    rule::Rule,
    validation::canonical_claims_oppose,
};
use nirmata_store::{CanonSnapshot, ReadScope};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const KEEP_DESTINATION: &str = "keep_destination";
const TAKE_SOURCE: &str = "take_source";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeReviewResult {
    pub source_scope: ReadScope,
    pub destination_scope: ReadScope,
    pub common_ancestor_revision: RevisionId,
    pub automatic_operation_ids: Vec<String>,
    pub decision_operation_ids: Vec<String>,
    pub review: ManualReviewSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
enum MergeValue {
    World(MergeWorld),
    Entity(Entity),
    Relation(Relation),
    Event(EventAggregate),
    Claim(Claim),
    Rule(Rule),
    Goal(Goal),
    Document(DocumentAggregate),
}

#[derive(Clone, Debug)]
struct MergeWorld(World);

impl PartialEq for MergeWorld {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
            && self.0.name() == other.0.name()
            && self.0.premise_md() == other.0.premise_md()
            && self.0.epoch_label() == other.0.epoch_label()
            && self.0.calendar() == other.0.calendar()
            && self.0.created_at_ms() == other.0.created_at_ms()
    }
}

impl NirmataApp {
    pub fn prepare_variant_merge(
        &mut self,
        source_scope: ReadScope,
    ) -> Result<MergeReviewResult, AppError> {
        let active = self.active.as_mut().ok_or(AppError::NoWorldOpen)?;
        ensure_active_write_scope(active)?;
        if source_scope.variant_id == active.session.active_variant.id {
            return Err(nirmata_store::StoreError::InvalidVariant(
                "merge source must be a different variant".to_owned(),
            )
            .into());
        }
        let source_revision = active.store.resolve_scope(source_scope)?;
        let destination_scope = ReadScope::head(active.session.active_variant.id);
        let destination_revision = active.store.resolve_scope(destination_scope)?;
        let common_ancestor = active
            .store
            .common_ancestor(source_revision, destination_revision)?;
        let base = active
            .store
            .read_canon_snapshot_scoped(ReadScope::historical(
                source_scope.variant_id,
                common_ancestor,
            ))?;
        let source = active
            .store
            .read_canon_snapshot_scoped(ReadScope::historical(
                source_scope.variant_id,
                source_revision,
            ))?;
        let destination = active.store.read_canon_snapshot_scoped(destination_scope)?;
        let base = merge_values(&base);
        let source = merge_values(&source);
        let destination = merge_values(&destination);
        let mut object_refs = base.keys().copied().collect::<Vec<_>>();
        object_refs.extend(source.keys().copied());
        object_refs.sort();
        object_refs.dedup();

        let source_changed = object_refs
            .into_iter()
            .filter(|object_ref| base.get(object_ref) != source.get(object_ref))
            .collect::<Vec<_>>();
        let created_refs = source_changed
            .iter()
            .filter(|object_ref| base.get(object_ref).is_none() && source.get(object_ref).is_some())
            .copied()
            .collect::<BTreeSet<_>>();
        let mut operations = Vec::new();
        let mut decisions = Vec::new();
        let mut automatic_operation_ids = Vec::new();
        let mut decision_operation_ids = Vec::new();

        for object_ref in source_changed {
            let base_value = base.get(&object_ref);
            let source_value = source.get(&object_ref);
            let destination_value = destination.get(&object_ref);
            if source_value == destination_value {
                continue;
            }
            let overlap = destination_value != base_value;
            let operation = merge_operation(object_ref, source_value, destination_value, overlap)?;
            let operation_id = operation.operation_id();
            let missing_dependency = source_value.is_some_and(|value| {
                dependencies(value).into_iter().any(|dependency| {
                    !destination.contains_key(&dependency) && !created_refs.contains(&dependency)
                })
            });
            let opposing_claim = match source_value {
                Some(MergeValue::Claim(source_claim)) => {
                    destination.iter().any(|(candidate_ref, candidate)| {
                        matches!(
                            candidate,
                            MergeValue::Claim(destination_claim)
                                if canonical_claims_oppose(source_claim, destination_claim)
                                    && base.get(candidate_ref) == source.get(candidate_ref)
                        )
                    })
                }
                _ => false,
            };
            let replacement = operation.retcon() == RetconKind::Replacement;
            if overlap || missing_dependency || opposing_claim || replacement {
                let reason = if overlap {
                    "destination changed the same stable object since the common ancestor"
                } else if opposing_claim {
                    "source introduces a canonical claim opposed by the destination"
                } else {
                    if missing_dependency {
                        "source operation depends on an object not present in the destination merge"
                    } else {
                        "source removes canon and requires an explicit replacement decision"
                    }
                };
                decisions.push(merge_decision(
                    operation_id,
                    object_ref,
                    source_revision,
                    reason,
                    replacement,
                )?);
                decision_operation_ids.push(operation_id.to_string());
            } else {
                automatic_operation_ids.push(operation_id.to_string());
            }
            operations.push(operation);
        }
        if operations.is_empty() {
            return Err(nirmata_store::StoreError::InvalidChangeSet(
                "source has no changes to merge into the destination".to_owned(),
            )
            .into());
        }
        decisions = expand_dependent_decisions(decisions, &operations)?;
        let sources = destination_sources(&operations, &destination);
        let objective = format!("Merge variant revision {source_revision}");
        let assumptions = vec![
            format!("merge source revision: {source_revision}"),
            format!("common ancestor revision: {common_ancestor}"),
        ];
        let provisional = ChangeSetDraft::new(
            active.session.world_id,
            destination_revision,
            objective.clone(),
            sources.clone(),
            assumptions.clone(),
            operations.clone(),
            decisions.clone(),
        )?;
        let report = active.store.validate_change_set_draft(&provisional)?;
        let mut manual_operation_ids = decisions
            .iter()
            .flat_map(|decision| decision.operation_ids().iter().copied())
            .collect::<BTreeSet<_>>();
        for operation in &operations {
            if manual_operation_ids.contains(&operation.operation_id()) {
                continue;
            }
            let Some(reason) = temporal_merge_conflict_reason(&report, operation.primary_ref())
            else {
                continue;
            };
            decisions.push(merge_decision(
                operation.operation_id(),
                operation.primary_ref(),
                source_revision,
                &reason,
                false,
            )?);
            manual_operation_ids.insert(operation.operation_id());
        }
        automatic_operation_ids = operations
            .iter()
            .filter(|operation| !manual_operation_ids.contains(&operation.operation_id()))
            .map(|operation| operation.operation_id().to_string())
            .collect();
        decision_operation_ids = operations
            .iter()
            .filter(|operation| manual_operation_ids.contains(&operation.operation_id()))
            .map(|operation| operation.operation_id().to_string())
            .collect();
        let draft = ChangeSetDraft::new(
            active.session.world_id,
            destination_revision,
            objective,
            sources,
            assumptions,
            operations,
            decisions,
        )?;
        let review = ManualReviewSession::from_draft(
            active.session.active_variant.id,
            draft,
            &active.store,
        )?;
        let review_key = ObjectRef::World(active.session.world_id).to_string();
        if self.manual_reviews.contains_key(&review_key) {
            return Err(AppError::ReviewSessionConflict(review_key));
        }
        let mut stored = StoredManualReview::new(review);
        stored.merge_source_revision = Some(source_revision);
        stored.origin = crate::PendingReviewOrigin::VersionsMerge;
        let review = stored.snapshot(&review_key);
        self.insert_pending_review(review_key, stored)?;
        Ok(MergeReviewResult {
            source_scope: ReadScope::historical(source_scope.variant_id, source_revision),
            destination_scope,
            common_ancestor_revision: common_ancestor,
            automatic_operation_ids,
            decision_operation_ids,
            review,
        })
    }
}

pub(crate) fn apply_variant_merge_review_action(
    review: &ManualReviewSession,
    action: crate::ManualReviewAction,
    decided_at_ms: i64,
    store: &nirmata_store::WorldStore,
) -> Result<ManualReviewSession, AppError> {
    let (decision_point_id, alternative) = match &action {
        crate::ManualReviewAction::ResolveDecision {
            decision_point_id,
            alternative,
        } => (*decision_point_id, alternative.as_str()),
        _ => return review.apply_action(action, decided_at_ms, store),
    };
    let decision = review
        .original_draft()
        .decisions()
        .iter()
        .find(|decision| decision.decision_point_id() == decision_point_id)
        .ok_or(AppError::UnknownReviewDecision(decision_point_id))?;
    if decision.alternatives() != [KEEP_DESTINATION, TAKE_SOURCE] {
        return review.apply_action(action, decided_at_ms, store);
    }
    let operation_ids = decision.operation_ids().to_vec();
    let selected = alternative == TAKE_SOURCE;
    let review = review.apply_action(action, decided_at_ms, store)?;
    review.set_operation_selection(&operation_ids, selected, store)
}

fn merge_decision(
    operation_id: ChangeOperationId,
    object_ref: ObjectRef,
    source_revision: RevisionId,
    reason: &str,
    replacement: bool,
) -> Result<DecisionPoint, AppError> {
    let prompt = format!(
        "Merge {} from revision {}: {reason}. Choose explicitly.",
        object_ref, source_revision
    );
    let alternatives = vec![KEEP_DESTINATION.to_owned(), TAKE_SOURCE.to_owned()];
    if replacement {
        return Ok(DecisionPoint::restore(
            DecisionPointId::new(),
            vec![operation_id],
            prompt,
            alternatives,
            Some(object_ref),
            Some(reason.to_owned()),
            None,
        )?);
    }
    Ok(DecisionPoint::new(
        vec![operation_id],
        prompt,
        alternatives,
    )?)
}

fn expand_dependent_decisions(
    decisions: Vec<DecisionPoint>,
    operations: &[ChangeOperation],
) -> Result<Vec<DecisionPoint>, AppError> {
    decisions
        .into_iter()
        .map(|decision| {
            let mut operation_ids = decision
                .operation_ids()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut decided_refs = operations
                .iter()
                .filter(|operation| operation_ids.contains(&operation.operation_id()))
                .map(ChangeOperation::primary_ref)
                .collect::<BTreeSet<_>>();
            loop {
                let mut changed = false;
                for operation in operations {
                    if operation_ids.contains(&operation.operation_id()) {
                        continue;
                    }
                    if operation
                        .affected_ids()
                        .iter()
                        .any(|object_ref| decided_refs.contains(object_ref))
                    {
                        operation_ids.insert(operation.operation_id());
                        decided_refs.insert(operation.primary_ref());
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            Ok(DecisionPoint::restore(
                decision.decision_point_id(),
                operation_ids.into_iter().collect(),
                decision.prompt().to_owned(),
                decision.alternatives().to_vec(),
                decision.replacement_target(),
                decision.reason().map(str::to_owned),
                decision.resolved_alternative().map(str::to_owned),
            )?)
        })
        .collect()
}

fn temporal_merge_conflict_reason(
    report: &nirmata_core::validation::ValidationReport,
    primary_ref: ObjectRef,
) -> Option<String> {
    let ObjectRef::Event(primary_id) = primary_ref else {
        return None;
    };
    let primary_id = primary_id.to_string();
    report.conflicts.iter().find_map(|issue| {
        if !matches!(
            issue.code.as_str(),
            "causality.cause_after_effect"
                | "lifecycle.participation_before_birth"
                | "lifecycle.participation_after_death"
        ) {
            return None;
        }
        let events = issue
            .objects
            .iter()
            .filter(|object| object.kind == "event")
            .map(|object| object.id.as_str())
            .collect::<Vec<_>>();
        if !events.contains(&primary_id.as_str()) {
            return None;
        }
        let related = events
            .into_iter()
            .find(|event_id| *event_id != primary_id)?;
        Some(format!(
            "deterministic temporal conflict {} with nirmata://event/{related}",
            issue.code
        ))
    })
}

fn merge_values(snapshot: &CanonSnapshot) -> BTreeMap<ObjectRef, MergeValue> {
    let mut values = BTreeMap::new();
    values.insert(
        ObjectRef::World(snapshot.world().id()),
        MergeValue::World(MergeWorld(snapshot.world().clone())),
    );
    values.extend(
        snapshot
            .entities()
            .iter()
            .cloned()
            .map(|value| (ObjectRef::Entity(value.id()), MergeValue::Entity(value))),
    );
    values.extend(
        snapshot
            .relations()
            .iter()
            .cloned()
            .map(|value| (ObjectRef::Relation(value.id()), MergeValue::Relation(value))),
    );
    values.extend(snapshot.events().iter().cloned().map(|value| {
        (
            ObjectRef::Event(value.event().id()),
            MergeValue::Event(value),
        )
    }));
    values.extend(
        snapshot
            .claims()
            .iter()
            .cloned()
            .map(|value| (ObjectRef::Claim(value.id()), MergeValue::Claim(value))),
    );
    values.extend(
        snapshot
            .rules()
            .iter()
            .cloned()
            .map(|value| (ObjectRef::Rule(value.id()), MergeValue::Rule(value))),
    );
    values.extend(
        snapshot
            .goals()
            .iter()
            .cloned()
            .map(|value| (ObjectRef::Goal(value.id()), MergeValue::Goal(value))),
    );
    values.extend(snapshot.documents().iter().cloned().map(|value| {
        (
            ObjectRef::Document(value.object().id()),
            MergeValue::Document(value),
        )
    }));
    values
}

fn merge_operation(
    object_ref: ObjectRef,
    source: Option<&MergeValue>,
    destination: Option<&MergeValue>,
    overlap: bool,
) -> Result<ChangeOperation, AppError> {
    let operation_id = ChangeOperationId::new();
    let mut affected_ids = vec![object_ref];
    if let Some(value) = source.or(destination) {
        affected_ids.extend(dependencies(value));
    }
    affected_ids.sort();
    affected_ids.dedup();
    macro_rules! operation {
        ($create:ident, $update:ident, $delete:ident, $variant:ident, $version:expr) => {
            match (source, destination) {
                (Some(MergeValue::$variant(after)), None) => ChangeOperation::$create {
                    operation_id,
                    affected_ids,
                    expected_version: 0,
                    retcon: if overlap {
                        RetconKind::Replacement
                    } else {
                        RetconKind::Additive
                    },
                    after: normalize(after, 1, $version)?,
                },
                (Some(MergeValue::$variant(after)), Some(MergeValue::$variant(before))) => {
                    ChangeOperation::$update {
                        operation_id,
                        affected_ids,
                        expected_version: $version(before),
                        retcon: if overlap {
                            RetconKind::Replacement
                        } else {
                            RetconKind::Additive
                        },
                        before: before.clone(),
                        after: normalize(after, $version(before) + 1, $version)?,
                    }
                }
                (None, Some(MergeValue::$variant(before))) => ChangeOperation::$delete {
                    operation_id,
                    affected_ids,
                    expected_version: $version(before),
                    retcon: RetconKind::Replacement,
                    before: before.clone(),
                },
                _ => {
                    return Err(nirmata_store::StoreError::InvalidChangeSet(format!(
                        "merge payload type does not match {object_ref}"
                    ))
                    .into())
                }
            }
        };
    }
    Ok(match object_ref {
        ObjectRef::World(_) => match (source, destination) {
            (Some(MergeValue::World(after)), Some(MergeValue::World(before))) => {
                ChangeOperation::UpdateWorld {
                    operation_id,
                    affected_ids,
                    expected_version: 0,
                    retcon: if overlap {
                        RetconKind::Replacement
                    } else {
                        RetconKind::Additive
                    },
                    before: before.0.clone(),
                    after: World::restore(
                        before.0.id(),
                        after.0.name(),
                        after.0.premise_md(),
                        after.0.epoch_label(),
                        after.0.calendar().cloned(),
                        before.0.current_revision(),
                        before.0.created_at_ms(),
                        after.0.updated_at_ms(),
                    )?,
                }
            }
            _ => {
                return Err(nirmata_store::StoreError::InvalidChangeSet(
                    "world metadata can only be updated by a limited merge".to_owned(),
                )
                .into());
            }
        },
        ObjectRef::Entity(_) => operation!(
            CreateEntity,
            UpdateEntity,
            DeleteEntity,
            Entity,
            |v: &Entity| v.version()
        ),
        ObjectRef::Relation(_) => operation!(
            CreateRelation,
            UpdateRelation,
            DeleteRelation,
            Relation,
            |v: &Relation| v.version()
        ),
        ObjectRef::Event(_) => operation!(
            CreateEvent,
            UpdateEvent,
            DeleteEvent,
            Event,
            |v: &EventAggregate| v.event().version()
        ),
        ObjectRef::Claim(_) => {
            operation!(CreateClaim, UpdateClaim, DeleteClaim, Claim, |v: &Claim| v
                .version())
        }
        ObjectRef::Rule(_) => operation!(CreateRule, UpdateRule, DeleteRule, Rule, |v: &Rule| v
            .version()),
        ObjectRef::Goal(_) => operation!(CreateGoal, UpdateGoal, DeleteGoal, Goal, |v: &Goal| v
            .version()),
        ObjectRef::Document(_) => operation!(
            CreateDocument,
            UpdateDocument,
            DeleteDocument,
            Document,
            |v: &DocumentAggregate| v.object().version()
        ),
    })
}

fn normalize<T>(value: &T, version: u64, _version: impl Fn(&T) -> u64) -> Result<T, AppError>
where
    T: Serialize + DeserializeOwned,
{
    let mut value = serde_json::to_value(value)
        .map_err(|error| nirmata_store::StoreError::InvalidChangeSet(error.to_string()))?;
    if let Some(field) = value.get_mut("version") {
        *field = Value::from(version);
    } else if let Some(field) = value.pointer_mut("/event/version") {
        *field = Value::from(version);
    } else if let Some(field) = value.pointer_mut("/object/version") {
        *field = Value::from(version);
    }
    serde_json::from_value(value)
        .map_err(|error| nirmata_store::StoreError::InvalidChangeSet(error.to_string()).into())
}

fn dependencies(value: &MergeValue) -> Vec<ObjectRef> {
    match value {
        MergeValue::World(_) | MergeValue::Entity(_) | MergeValue::Rule(_) => vec![],
        MergeValue::Relation(value) => vec![
            ObjectRef::Entity(value.source_entity_id()),
            ObjectRef::Entity(value.target_entity_id()),
        ],
        MergeValue::Goal(value) => vec![ObjectRef::Entity(value.holder_entity_id())],
        MergeValue::Event(value) => value
            .event()
            .participants()
            .iter()
            .map(|participant| ObjectRef::Entity(participant.entity_id()))
            .chain(value.event().location_entity_id().map(ObjectRef::Entity))
            .chain(
                value
                    .event()
                    .affected_goal_ids()
                    .iter()
                    .copied()
                    .map(ObjectRef::Goal),
            )
            .chain(value.links().iter().flat_map(|link| {
                [
                    ObjectRef::Event(link.source_event_id()),
                    ObjectRef::Event(link.target_event_id()),
                ]
            }))
            .collect(),
        MergeValue::Claim(value) => std::iter::once(ObjectRef::Entity(value.subject_entity_id()))
            .chain(value.holder_entity_id().map(ObjectRef::Entity))
            .chain(value.object().and_then(|object| match object {
                nirmata_core::claim::ClaimObject::Entity(id) => Some(ObjectRef::Entity(*id)),
                nirmata_core::claim::ClaimObject::Scalar(_) => None,
            }))
            .chain(value.source_document_id().map(ObjectRef::Document))
            .chain(value.source_claim_id().map(ObjectRef::Claim))
            .collect(),
        MergeValue::Document(value) => value
            .object()
            .author_entity_id()
            .map(ObjectRef::Entity)
            .into_iter()
            .chain(
                value
                    .object()
                    .perspective_entity_id()
                    .map(ObjectRef::Entity),
            )
            .chain(
                value
                    .references()
                    .iter()
                    .map(|reference| reference.target()),
            )
            .collect(),
    }
}

fn destination_sources(
    operations: &[ChangeOperation],
    destination: &BTreeMap<ObjectRef, MergeValue>,
) -> Vec<ObjectRef> {
    let mut refs = operations
        .iter()
        .flat_map(|operation| {
            operation
                .affected_ids()
                .iter()
                .copied()
                .filter(|object_ref| *object_ref != operation.primary_ref())
        })
        .filter(|object_ref| destination.contains_key(object_ref))
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}
