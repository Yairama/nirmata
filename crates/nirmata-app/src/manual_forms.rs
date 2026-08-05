use crate::{
    AppError, DraftOperationInput, ManualReviewInput, WorldSession,
    manual_review::{ManualReviewSession, ManualReviewSnapshot},
};
use nirmata_core::{
    ChangeOperationId, ClaimId, DocumentId, DomainError, EntityId, GoalId, Period, World,
    change_set::ChangeOperation,
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentAggregate, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventAggregate, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
    time::{Certainty, EventTime, EventTimeKind, TimePrecision},
};
use nirmata_store::{ResolvedObject, WorldStore};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDraftResponse {
    pub draft: Option<ManualDraftPreview>,
    pub review: Option<ManualReviewSnapshot>,
    pub field_issues: Vec<ManualFieldIssue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualFieldIssue {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDraftPreview {
    pub draft_key: String,
    pub target_uri: String,
    pub object_type: &'static str,
    pub mode: &'static str,
    pub title: String,
    pub objective: String,
    pub source_uris: Vec<String>,
    pub assumptions: Vec<String>,
    pub logical_path: String,
    pub validation_report: nirmata_core::validation::ValidationReport,
    pub ready_to_confirm: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDraftRequest {
    pub object_type: String,
    pub existing_uri: Option<String>,
    pub objective: Option<String>,
    #[serde(default)]
    pub source_uris: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

struct Builder<'a> {
    store: &'a WorldStore,
    session: &'a WorldSession,
    world: &'a World,
    request: ManualDraftRequest,
    now_ms: i64,
    issues: Vec<ManualFieldIssue>,
}

pub(crate) struct BuiltOperation {
    target_uri: String,
    object_type: &'static str,
    mode: &'static str,
    title: String,
    logical_path: String,
    pub operation: DraftOperationInput,
}

pub(crate) struct PreviewManualDraftOutcome {
    pub response: ManualDraftResponse,
    pub review: Option<ManualReviewSession>,
}

pub(crate) struct PreparedManualOperation {
    pub objective: String,
    pub sources: Vec<ObjectRef>,
    pub assumptions: Vec<String>,
    pub built: BuiltOperation,
}

pub(crate) struct PreparedManualOperationOutcome {
    pub prepared: Option<PreparedManualOperation>,
    pub field_issues: Vec<ManualFieldIssue>,
}

#[derive(Clone, Copy)]
struct EventLinkSpec {
    target_event_id: nirmata_core::EventId,
    kind: EventLinkKind,
}

#[derive(Clone)]
struct ContentReferenceSpec {
    target: ObjectRef,
    ordinal: u32,
}

impl<'a> Builder<'a> {
    fn new(
        store: &'a WorldStore,
        session: &'a WorldSession,
        world: &'a World,
        request: ManualDraftRequest,
        now_ms: i64,
    ) -> Self {
        Self {
            store,
            session,
            world,
            request,
            now_ms,
            issues: vec![],
        }
    }

    fn prepare(&mut self) -> Result<PreparedManualOperationOutcome, AppError> {
        let built = match self.request.object_type.trim() {
            "world" => self.build_world()?,
            "entity" => self.build_entity()?,
            "relation" => self.build_relation()?,
            "event" => self.build_event()?,
            "goal" => self.build_goal()?,
            "rule" => self.build_rule()?,
            "claim" => self.build_claim()?,
            "document" => self.build_document()?,
            other => {
                self.issue(
                    "objectType",
                    format!("tipo de formulario no soportado: {other}"),
                );
                None
            }
        };

        let sources = self.parse_source_uris();
        let assumptions = self.parse_assumptions();

        if !self.issues.is_empty() || built.is_none() {
            return Ok(PreparedManualOperationOutcome {
                prepared: None,
                field_issues: std::mem::take(&mut self.issues),
            });
        }

        let built = built.expect("checked built operation");
        let objective = self.objective_for(&built);
        Ok(PreparedManualOperationOutcome {
            prepared: Some(PreparedManualOperation {
                objective,
                sources,
                assumptions,
                built,
            }),
            field_issues: vec![],
        })
    }

    fn build(mut self) -> Result<PreviewManualDraftOutcome, AppError> {
        let current_revision = self.session.current_revision;
        let request_source_uris = self.request.source_uris.clone();
        let prepared = self.prepare()?;
        let Some(prepared) = prepared.prepared else {
            return Ok(PreviewManualDraftOutcome {
                response: ManualDraftResponse {
                    draft: None,
                    review: None,
                    field_issues: prepared.field_issues,
                },
                review: None,
            });
        };
        let review = ManualReviewSession::create(
            self.session.world_id,
            self.session.current_revision,
            ManualReviewInput {
                objective: prepared.objective.clone(),
                sources: prepared.sources.clone(),
                assumptions: prepared.assumptions.clone(),
                operations: vec![prepared.built.operation],
            },
            self.store,
        )?;

        let review_key = prepared.built.target_uri.clone();
        Ok(PreviewManualDraftOutcome {
            response: ManualDraftResponse {
                field_issues: vec![],
                draft: Some(ManualDraftPreview {
                    draft_key: review_key.clone(),
                    target_uri: prepared.built.target_uri,
                    object_type: prepared.built.object_type,
                    mode: prepared.built.mode,
                    title: prepared.built.title,
                    objective: prepared.objective,
                    source_uris: request_source_uris,
                    assumptions: prepared.assumptions,
                    logical_path: prepared.built.logical_path,
                    validation_report: review.validation_report().clone(),
                    ready_to_confirm: review.ready_to_confirm(),
                }),
                review: Some(review.snapshot(
                    &review_key,
                    crate::manual_review::ManualReviewFreshnessSnapshot {
                        status: crate::manual_review::ManualReviewFreshnessStatus::Current,
                        current_revision: current_revision.to_string(),
                        can_revalidate: false,
                        message: "La revisión está alineada con la cabeza actual.".to_owned(),
                    },
                )),
            },
            review: Some(review),
        })
    }

    fn build_world(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        if let Some(uri) = self
            .request
            .existing_uri
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            let expected = ObjectRef::World(self.world.id()).to_string();
            if uri.trim() != expected {
                self.issue("existingUri", "el URI actual no apunta al mundo activo");
                return Ok(None);
            }
        }

        let name = self.required("name");
        let premise_md = self.value("premise_md");
        let epoch_label = self.value("epoch_label");
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = World::restore(
            self.world.id(),
            name.expect("world name"),
            premise_md,
            epoch_label,
            self.world.current_revision(),
            self.world.created_at_ms(),
            self.now_ms,
        )
        .map(|after| BuiltOperation {
            target_uri: ObjectRef::World(after.id()).to_string(),
            object_type: "world",
            mode: "update",
            title: after.name().to_owned(),
            logical_path: "/world".to_owned(),
            operation: DraftOperationInput::UpdateWorld {
                retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                before: self.world.clone(),
                after,
            },
        });
        Ok(self.map_built("name", built))
    }

    fn build_entity(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("entity")? {
            Some(ResolvedObject::Entity(entity)) => Some(entity),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a una entidad");
                None
            }
            None => None,
        };
        let kind = self.parse_entity_kind("kind");
        let name = self.required("name");
        let slug = self.required("slug");
        let summary = self.value("summary");
        let body_md = self.value("body_md");
        let attributes_json = self.value_or("attributes_json", "{}");
        let aliases = self.lines("aliases");
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => Entity::restore(
                before.id(),
                before.world_id(),
                kind.expect("entity kind"),
                name.expect("entity name"),
                slug.expect("entity slug"),
                summary,
                body_md,
                attributes_json,
                aliases,
                before.version() + 1,
                before.created_at_ms(),
                self.now_ms,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Entity(after.id()).to_string(),
                object_type: "entity",
                mode: "update",
                title: after.name().to_owned(),
                logical_path: format!(
                    "/entities/{}/{}",
                    entity_group_name(after.kind()),
                    display_name(after.name(), after.id().to_string())
                ),
                operation: DraftOperationInput::UpdateEntity {
                    retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                    before,
                    after,
                },
            }),
            None => Entity::new(
                self.world.id(),
                kind.expect("entity kind"),
                name.expect("entity name"),
                slug.expect("entity slug"),
                summary,
                body_md,
                attributes_json,
                aliases,
                self.now_ms,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Entity(after.id()).to_string(),
                object_type: "entity",
                mode: "create",
                title: after.name().to_owned(),
                logical_path: format!(
                    "/entities/{}/{}",
                    entity_group_name(after.kind()),
                    display_name(after.name(), after.id().to_string())
                ),
                operation: DraftOperationInput::CreateEntity {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after,
                },
            }),
        };
        Ok(self.map_built("aliases", built))
    }

    fn build_relation(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("relation")? {
            Some(ResolvedObject::Relation(relation)) => Some(relation),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a una relación");
                None
            }
            None => None,
        };
        let source_entity_id = self.parse_entity_id_required("source_entity");
        let target_entity_id = self.parse_entity_id_required("target_entity");
        let kind = self.required("kind");
        let direction = self.parse_relation_direction("direction");
        let certainty = self.parse_certainty("certainty");
        let (valid_from_tick, valid_to_tick) =
            self.optional_tick_range("valid_from_tick", "valid_to_tick");
        let source_reference = self.optional("source_reference");
        let metadata_json = self.value_or("metadata_json", "{}");
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => Relation::restore(
                before.id(),
                before.world_id(),
                source_entity_id.expect("relation source"),
                target_entity_id.expect("relation target"),
                kind.expect("relation kind"),
                direction.expect("relation direction"),
                valid_from_tick,
                valid_to_tick,
                certainty.expect("relation certainty"),
                source_reference,
                metadata_json,
                before.version() + 1,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Relation(after.id()).to_string(),
                object_type: "relation",
                mode: "update",
                title: after.kind().to_owned(),
                logical_path: format!(
                    "/relations/{}",
                    display_name(after.kind(), after.id().to_string())
                ),
                operation: DraftOperationInput::UpdateRelation {
                    retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                    before,
                    after,
                },
            }),
            None => Relation::new(
                self.world.id(),
                source_entity_id.expect("relation source"),
                target_entity_id.expect("relation target"),
                kind.expect("relation kind"),
                direction.expect("relation direction"),
                valid_from_tick,
                valid_to_tick,
                certainty.expect("relation certainty"),
                source_reference,
                metadata_json,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Relation(after.id()).to_string(),
                object_type: "relation",
                mode: "create",
                title: after.kind().to_owned(),
                logical_path: format!(
                    "/relations/{}",
                    display_name(after.kind(), after.id().to_string())
                ),
                operation: DraftOperationInput::CreateRelation {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after,
                },
            }),
        };
        Ok(self.map_built("valid_to_tick", built))
    }

    fn build_event(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("event")? {
            Some(ResolvedObject::Event(aggregate)) => Some(aggregate),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a un evento");
                None
            }
            None => None,
        };
        let kind = self.required("kind");
        let summary = self.required("summary");
        let body_md = self.value("body_md");
        let time = self.parse_event_time();
        let location_entity_id = self.parse_entity_id_optional("location_entity");
        let participants = self.parse_participants("participants");
        let affected_goal_ids = self.parse_goal_ids("affected_goal_ids");
        let causal_specs = self.parse_event_links("causal_links");
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => {
                let event = before.event();
                Event::restore(
                    event.id(),
                    event.world_id(),
                    kind.expect("event kind"),
                    summary.expect("event summary"),
                    body_md,
                    time.expect("event time"),
                    location_entity_id,
                    participants,
                    affected_goal_ids,
                    event.version() + 1,
                    event.created_at_ms(),
                    self.now_ms,
                )
                .and_then(|after_event| {
                    build_event_links(after_event.id(), &causal_specs)
                        .map(|links| (after_event, links))
                })
                .map(|(after_event, links)| BuiltOperation {
                    target_uri: ObjectRef::Event(after_event.id()).to_string(),
                    object_type: "event",
                    mode: "update",
                    title: after_event.summary().to_owned(),
                    logical_path: format!(
                        "/events/{}",
                        display_name(after_event.summary(), after_event.id().to_string())
                    ),
                    operation: DraftOperationInput::UpdateEvent {
                        retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                        before,
                        after: EventAggregate::new(after_event, links),
                    },
                })
            }
            None => Event::new(
                self.world.id(),
                kind.expect("event kind"),
                summary.expect("event summary"),
                body_md,
                time.expect("event time"),
                location_entity_id,
                participants,
                affected_goal_ids,
                self.now_ms,
            )
            .and_then(|after_event| {
                build_event_links(after_event.id(), &causal_specs).map(|links| (after_event, links))
            })
            .map(|(after_event, links)| BuiltOperation {
                target_uri: ObjectRef::Event(after_event.id()).to_string(),
                object_type: "event",
                mode: "create",
                title: after_event.summary().to_owned(),
                logical_path: format!(
                    "/events/{}",
                    display_name(after_event.summary(), after_event.id().to_string())
                ),
                operation: DraftOperationInput::CreateEvent {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after: EventAggregate::new(after_event, links),
                },
            }),
        };
        Ok(self.map_built("participants", built))
    }

    fn build_goal(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("goal")? {
            Some(ResolvedObject::Goal(goal)) => Some(goal),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a una meta");
                None
            }
            None => None,
        };
        let holder_entity_id = self.parse_entity_id_required("holder_entity");
        let desired_state_md = self.required("desired_state_md");
        let priority = self.parse_i32("priority", true);
        let status = self.parse_goal_status("status");
        let visibility = self.parse_goal_visibility("visibility");
        let source = self.optional("source");
        let period = self.parse_period("period_start_tick", "period_end_tick");
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => Goal::restore(
                before.id(),
                before.world_id(),
                holder_entity_id.expect("goal holder"),
                desired_state_md.expect("goal desired state"),
                priority.expect("goal priority"),
                status.expect("goal status"),
                period,
                visibility.expect("goal visibility"),
                source,
                before.version() + 1,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Goal(after.id()).to_string(),
                object_type: "goal",
                mode: "update",
                title: preview(after.desired_state_md(), "Meta"),
                logical_path: format!(
                    "/goals/{}",
                    display_name(
                        &preview(after.desired_state_md(), "goal"),
                        after.id().to_string()
                    )
                ),
                operation: DraftOperationInput::UpdateGoal {
                    retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                    before,
                    after,
                },
            }),
            None => Goal::new(
                self.world.id(),
                holder_entity_id.expect("goal holder"),
                desired_state_md.expect("goal desired state"),
                priority.expect("goal priority"),
                status.expect("goal status"),
                period,
                visibility.expect("goal visibility"),
                source,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Goal(after.id()).to_string(),
                object_type: "goal",
                mode: "create",
                title: preview(after.desired_state_md(), "Meta"),
                logical_path: format!(
                    "/goals/{}",
                    display_name(
                        &preview(after.desired_state_md(), "goal"),
                        after.id().to_string()
                    )
                ),
                operation: DraftOperationInput::CreateGoal {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after,
                },
            }),
        };
        Ok(self.map_built("period_end_tick", built))
    }

    fn build_rule(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("rule")? {
            Some(ResolvedObject::Rule(rule)) => Some(rule),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a una regla");
                None
            }
            None => None,
        };
        let kind = self.parse_rule_kind("kind");
        let statement_md = self.required("statement_md");
        let scope = self.required("scope");
        let severity = self.parse_rule_severity("severity");
        let source = self.optional("source");
        let validator_kind = self.parse_rule_validator_kind("validator_kind");
        let parameters_json = self.value_or("parameters_json", "{}");

        if matches!(severity, Some(RuleSeverity::Hard)) && validator_kind.is_none() {
            self.issue(
                "validator_kind",
                "las reglas hard requieren un validador implementado",
            );
        }
        if matches!(validator_kind, Some(RuleValidatorKind::NoResurrection))
            && parameters_json.trim() != "{}"
        {
            self.issue(
                "parameters_json",
                "no_resurrection no acepta parámetros adicionales",
            );
        }
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => Rule::restore(
                before.id(),
                before.world_id(),
                kind.expect("rule kind"),
                statement_md.expect("rule statement"),
                scope.expect("rule scope"),
                severity.expect("rule severity"),
                source,
                validator_kind,
                parameters_json,
                before.version() + 1,
                before.created_at_ms(),
                self.now_ms,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Rule(after.id()).to_string(),
                object_type: "rule",
                mode: "update",
                title: preview(after.statement_md(), "Regla"),
                logical_path: format!(
                    "/rules/{}",
                    display_name(
                        &preview(after.statement_md(), "rule"),
                        after.id().to_string()
                    )
                ),
                operation: DraftOperationInput::UpdateRule {
                    retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                    before,
                    after,
                },
            }),
            None => Rule::new(
                self.world.id(),
                kind.expect("rule kind"),
                statement_md.expect("rule statement"),
                scope.expect("rule scope"),
                severity.expect("rule severity"),
                source,
                validator_kind,
                parameters_json,
                self.now_ms,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Rule(after.id()).to_string(),
                object_type: "rule",
                mode: "create",
                title: preview(after.statement_md(), "Regla"),
                logical_path: format!(
                    "/rules/{}",
                    display_name(
                        &preview(after.statement_md(), "rule"),
                        after.id().to_string()
                    )
                ),
                operation: DraftOperationInput::CreateRule {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after,
                },
            }),
        };
        Ok(self.map_built("parameters_json", built))
    }

    fn build_claim(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("claim")? {
            Some(ResolvedObject::Claim(claim)) => Some(claim),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a un claim");
                None
            }
            None => None,
        };
        let subject_entity_id = self.parse_entity_id_required("subject_entity");
        let content_md = self.value("content_md");
        let predicate_key = self.optional("predicate_key");
        let object = self.parse_claim_object("object_kind", "object_value");
        let polarity = self.parse_claim_polarity("polarity");
        let authentication = self.parse_claim_authentication("authentication");
        let holder_entity_id = self.parse_entity_id_optional("holder_entity");
        let modality = self.parse_claim_modality("modality");
        let register = self.optional("register");
        let epistemic_basis = self.optional("epistemic_basis");
        let source = self.optional("source");
        let source_document_id = self.parse_document_id_optional("source_document");
        let source_claim_id = self.parse_claim_id_optional("source_claim");
        let holder_confidence = self.parse_optional_f64("holder_confidence");
        let period = self.parse_period("period_start_tick", "period_end_tick");

        if predicate_key.is_some() != object.is_some() {
            self.issue(
                "predicate_key",
                "predicate_key y objeto deben completarse juntos",
            );
            self.issue(
                "object_value",
                "predicate_key y objeto deben completarse juntos",
            );
        }
        match authentication {
            Some(ClaimAuthentication::Canonical)
                if holder_entity_id.is_some() || modality.is_some() =>
            {
                self.issue(
                    "holder_entity",
                    "los claims canónicos no pueden tener holder ni modalidad",
                );
                self.issue(
                    "modality",
                    "los claims canónicos no pueden tener holder ni modalidad",
                );
            }
            Some(ClaimAuthentication::Attributed)
                if holder_entity_id.is_none() || modality.is_none() =>
            {
                self.issue(
                    "holder_entity",
                    "los claims attributed requieren holder y modalidad",
                );
                self.issue(
                    "modality",
                    "los claims attributed requieren holder y modalidad",
                );
            }
            _ => {}
        }

        if let Some(confidence) = holder_confidence {
            if !(0.0..=1.0).contains(&confidence) || !confidence.is_finite() {
                self.issue("holder_confidence", "la confianza debe estar entre 0 y 1");
            }
        }
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => Claim::restore(
                before.id(),
                before.world_id(),
                subject_entity_id.expect("claim subject"),
                content_md,
                predicate_key,
                object,
                polarity.expect("claim polarity"),
                authentication.expect("claim authentication"),
                holder_entity_id,
                modality,
                register,
                epistemic_basis,
                source,
                source_document_id,
                source_claim_id,
                holder_confidence,
                period,
                before.registered_revision_id(),
                before.superseded_revision_id(),
                before.version() + 1,
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Claim(after.id()).to_string(),
                object_type: "claim",
                mode: "update",
                title: preview(after.content_md(), "Claim"),
                logical_path: format!(
                    "/claims/{}",
                    display_name(
                        &preview(after.content_md(), "claim"),
                        after.id().to_string()
                    )
                ),
                operation: DraftOperationInput::UpdateClaim {
                    retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                    before,
                    after,
                },
            }),
            None => Claim::new(
                self.world.id(),
                subject_entity_id.expect("claim subject"),
                content_md,
                predicate_key,
                object,
                polarity.expect("claim polarity"),
                authentication.expect("claim authentication"),
                holder_entity_id,
                modality,
                register,
                epistemic_basis,
                source,
                source_document_id,
                source_claim_id,
                holder_confidence,
                period,
                self.world.current_revision(),
            )
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Claim(after.id()).to_string(),
                object_type: "claim",
                mode: "create",
                title: preview(after.content_md(), "Claim"),
                logical_path: format!(
                    "/claims/{}",
                    display_name(
                        &preview(after.content_md(), "claim"),
                        after.id().to_string()
                    )
                ),
                operation: DraftOperationInput::CreateClaim {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after,
                },
            }),
        };
        Ok(self.map_built("holder_confidence", built))
    }

    fn build_document(&mut self) -> Result<Option<BuiltOperation>, AppError> {
        let before = match self.resolve_existing("document")? {
            Some(ResolvedObject::Document(aggregate)) => Some(aggregate),
            Some(_) => {
                self.issue("existingUri", "el URI actual no apunta a un documento");
                None
            }
            None => None,
        };
        let title = self.required("title");
        let kind = self.required("kind");
        let author_entity_id = self.parse_entity_id_optional("author_entity");
        let perspective_entity_id = self.parse_entity_id_optional("perspective_entity");
        let canon_status = self.parse_document_canon_status("canon_status");
        let body_md = self.value("body_md");
        let content_references =
            self.parse_content_references("content_references", before.as_ref());
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = match before {
            Some(before) => Document::restore(
                before.object().id(),
                before.object().world_id(),
                title.expect("document title"),
                kind.expect("document kind"),
                author_entity_id,
                perspective_entity_id,
                canon_status.expect("document canon"),
                body_md,
                before.object().version() + 1,
                before.object().created_at_ms(),
                self.now_ms,
            )
            .map(|after| {
                let source = ObjectRef::Document(after.id());
                DocumentAggregate::new(after, build_content_references(source, &content_references))
            })
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Document(after.object().id()).to_string(),
                object_type: "document",
                mode: "update",
                title: after.object().title().to_owned(),
                logical_path: format!(
                    "/documents/{}/{}",
                    sanitize_segment(after.object().kind()),
                    display_name(after.object().title(), after.object().id().to_string())
                ),
                operation: DraftOperationInput::UpdateDocument {
                    retcon: nirmata_core::change_set::RetconKind::Reinterpretive,
                    before,
                    after,
                },
            }),
            None => Document::new(
                self.world.id(),
                title.expect("document title"),
                kind.expect("document kind"),
                author_entity_id,
                perspective_entity_id,
                canon_status.expect("document canon"),
                body_md,
                self.now_ms,
            )
            .map(|after| {
                let source = ObjectRef::Document(after.id());
                DocumentAggregate::new(after, build_content_references(source, &content_references))
            })
            .map(|after| BuiltOperation {
                target_uri: ObjectRef::Document(after.object().id()).to_string(),
                object_type: "document",
                mode: "create",
                title: after.object().title().to_owned(),
                logical_path: format!(
                    "/documents/{}/{}",
                    sanitize_segment(after.object().kind()),
                    display_name(after.object().title(), after.object().id().to_string())
                ),
                operation: DraftOperationInput::CreateDocument {
                    retcon: nirmata_core::change_set::RetconKind::Additive,
                    after,
                },
            }),
        };
        Ok(self.map_built("content_references", built))
    }

    fn resolve_existing(&mut self, object_type: &str) -> Result<Option<ResolvedObject>, AppError> {
        let Some(uri) = self
            .request
            .existing_uri
            .clone()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(None);
        };
        match self.store.resolve_uri(uri.trim()) {
            Ok(object) if object.object_ref().kind() == object_type => Ok(Some(object)),
            Ok(_) => {
                self.issue(
                    "existingUri",
                    format!("el URI actual no apunta a {object_type}"),
                );
                Ok(None)
            }
            Err(error) => {
                self.issue("existingUri", error.to_string());
                Ok(None)
            }
        }
    }

    fn parse_source_uris(&mut self) -> Vec<ObjectRef> {
        let source_uris = self.request.source_uris.clone();
        let mut sources = Vec::with_capacity(source_uris.len());
        for value in source_uris {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                continue;
            }
            match self.store.resolve_uri(trimmed) {
                Ok(object) => sources.push(object.object_ref()),
                Err(_) => self.issue(
                    "sourceUris",
                    format!("URI fuente inválida o inexistente: {trimmed}"),
                ),
            }
        }
        sources
    }

    fn parse_assumptions(&self) -> Vec<String> {
        self.request
            .assumptions
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn objective_for(&self, built: &BuiltOperation) -> String {
        let objective = self
            .request
            .objective
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        objective
            .map(str::to_owned)
            .unwrap_or_else(|| match built.mode {
                "create" => format!("Create {} {}", built.object_type, built.title),
                _ => format!("Update {} {}", built.object_type, built.title),
            })
    }

    fn parse_entity_kind(&mut self, field: &str) -> Option<EntityKind> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "person" => Some(EntityKind::Person),
                "place" => Some(EntityKind::Place),
                "faction" => Some(EntityKind::Faction),
                "culture" => Some(EntityKind::Culture),
                "resource" => Some(EntityKind::Resource),
                "concept" => Some(EntityKind::Concept),
                _ => None,
            },
        )
    }

    fn parse_relation_direction(&mut self, field: &str) -> Option<RelationDirection> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "directed" => Some(RelationDirection::Directed),
                "undirected" => Some(RelationDirection::Undirected),
                _ => None,
            },
        )
    }

    fn parse_certainty(&mut self, field: &str) -> Option<Certainty> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "certain" => Some(Certainty::Certain),
                "approximate" => Some(Certainty::Approximate),
                "uncertain" => Some(Certainty::Uncertain),
                "approximate_uncertain" => Some(Certainty::ApproximateUncertain),
                _ => None,
            },
        )
    }

    fn parse_goal_status(&mut self, field: &str) -> Option<GoalStatus> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "active" => Some(GoalStatus::Active),
                "achieved" => Some(GoalStatus::Achieved),
                "abandoned" => Some(GoalStatus::Abandoned),
                "frustrated" => Some(GoalStatus::Frustrated),
                _ => None,
            },
        )
    }

    fn parse_goal_visibility(&mut self, field: &str) -> Option<GoalVisibility> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "public" => Some(GoalVisibility::Public),
                "secret" => Some(GoalVisibility::Secret),
                _ => None,
            },
        )
    }

    fn parse_rule_kind(&mut self, field: &str) -> Option<RuleKind> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "constitutive" => Some(RuleKind::Constitutive),
                "generative" => Some(RuleKind::Generative),
                "institutional" => Some(RuleKind::Institutional),
                "authorial" => Some(RuleKind::Authorial),
                _ => None,
            },
        )
    }

    fn parse_rule_severity(&mut self, field: &str) -> Option<RuleSeverity> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "advisory" => Some(RuleSeverity::Advisory),
                "hard" => Some(RuleSeverity::Hard),
                _ => None,
            },
        )
    }

    fn parse_rule_validator_kind(&mut self, field: &str) -> Option<RuleValidatorKind> {
        match self.optional(field).as_deref() {
            None => None,
            Some("no_resurrection") => Some(RuleValidatorKind::NoResurrection),
            Some(_) => {
                self.issue(field, "validador desconocido");
                None
            }
        }
    }

    fn parse_claim_polarity(&mut self, field: &str) -> Option<ClaimPolarity> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "positive" => Some(ClaimPolarity::Positive),
                "negative" => Some(ClaimPolarity::Negative),
                _ => None,
            },
        )
    }

    fn parse_claim_authentication(&mut self, field: &str) -> Option<ClaimAuthentication> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "canonical" => Some(ClaimAuthentication::Canonical),
                "attributed" => Some(ClaimAuthentication::Attributed),
                "disputed" => Some(ClaimAuthentication::Disputed),
                _ => None,
            },
        )
    }

    fn parse_claim_modality(&mut self, field: &str) -> Option<ClaimModality> {
        match self.optional(field).as_deref() {
            None => None,
            Some("assertion") => Some(ClaimModality::Assertion),
            Some("belief") => Some(ClaimModality::Belief),
            Some("hypothesis") => Some(ClaimModality::Hypothesis),
            Some("counterfactual") => Some(ClaimModality::Counterfactual),
            Some(_) => {
                self.issue(field, "modalidad desconocida");
                None
            }
        }
    }

    fn parse_document_canon_status(&mut self, field: &str) -> Option<DocumentCanonStatus> {
        parse_enum(
            field,
            self.value(field).as_str(),
            &mut self.issues,
            |value| match value {
                "canonical" => Some(DocumentCanonStatus::Canonical),
                "non_canonical" => Some(DocumentCanonStatus::NonCanonical),
                _ => None,
            },
        )
    }

    fn parse_event_time(&mut self) -> Option<EventTime> {
        let kind = parse_enum(
            "time_kind",
            self.value("time_kind").as_str(),
            &mut self.issues,
            |value| match value {
                "unknown" => Some(EventTimeKind::Unknown),
                "instant" => Some(EventTimeKind::Instant),
                "interval" => Some(EventTimeKind::Interval),
                "ongoing" => Some(EventTimeKind::Ongoing),
                _ => None,
            },
        );
        let precision = parse_enum(
            "time_precision",
            self.value("time_precision").as_str(),
            &mut self.issues,
            |value| match value {
                "exact" => Some(TimePrecision::Exact),
                "day" => Some(TimePrecision::Day),
                "month" => Some(TimePrecision::Month),
                "year" => Some(TimePrecision::Year),
                "era" => Some(TimePrecision::Era),
                "unknown" => Some(TimePrecision::Unknown),
                _ => None,
            },
        );
        let certainty = self.parse_certainty("time_certainty");
        let start_tick = self.parse_optional_i64("start_tick");
        let end_tick = self.parse_optional_i64("end_tick");
        if !self.issues.is_empty() {
            return None;
        }

        match EventTime::new(
            kind.expect("event time kind"),
            start_tick,
            end_tick,
            precision.expect("event time precision"),
            certainty.expect("event time certainty"),
        ) {
            Ok(time) => Some(time),
            Err(DomainError::InvalidEventTime) => {
                self.issue(
                    "time_kind",
                    "los campos temporales no coinciden con el tipo de tiempo",
                );
                self.issue(
                    "end_tick",
                    "los campos temporales no coinciden con el tipo de tiempo",
                );
                None
            }
            Err(other) => {
                self.issue("time_kind", other.to_string());
                None
            }
        }
    }

    fn parse_claim_object(&mut self, kind_field: &str, value_field: &str) -> Option<ClaimObject> {
        match self.optional(kind_field).as_deref() {
            None | Some("none") => None,
            Some("entity") => self
                .parse_entity_id_required(value_field)
                .map(ClaimObject::Entity),
            Some("scalar") => self.required(value_field).map(ClaimObject::Scalar),
            Some(_) => {
                self.issue(kind_field, "tipo de objeto inválido");
                None
            }
        }
    }

    fn parse_participants(&mut self, field: &str) -> Vec<EventParticipant> {
        let mut participants = Vec::new();
        for (index, line) in self.lines(field).into_iter().enumerate() {
            let parts: Vec<_> = line.split('|').map(str::trim).collect();
            if !(2..=3).contains(&parts.len()) {
                self.issue(
                    field,
                    format!(
                        "participante inválido en línea {}: usa entidad|rol|ordinal",
                        index + 1
                    ),
                );
                continue;
            }
            let Some(entity_id) = parse_entity_id(parts[0]) else {
                self.issue(
                    field,
                    format!(
                        "participante inválido en línea {}: entidad desconocida",
                        index + 1
                    ),
                );
                continue;
            };
            let ordinal = if parts.len() == 3 {
                match parts[2].parse::<u32>() {
                    Ok(value) => value,
                    Err(_) => {
                        self.issue(
                            field,
                            format!(
                                "participante inválido en línea {}: ordinal inválido",
                                index + 1
                            ),
                        );
                        continue;
                    }
                }
            } else {
                index as u32
            };
            match EventParticipant::new(entity_id, parts[1], ordinal) {
                Ok(participant) => participants.push(participant),
                Err(error) => self.issue(field, error.to_string()),
            }
        }
        participants
    }

    fn parse_event_links(&mut self, field: &str) -> Vec<EventLinkSpec> {
        let mut links = Vec::new();
        for (index, line) in self.lines(field).into_iter().enumerate() {
            let parts: Vec<_> = line.split('|').map(str::trim).collect();
            if parts.len() != 2 {
                self.issue(
                    field,
                    format!(
                        "causalidad inválida en línea {}: usa evento|kind",
                        index + 1
                    ),
                );
                continue;
            }
            let Some(target_event_id) = parse_event_id(parts[0]) else {
                self.issue(
                    field,
                    format!(
                        "causalidad inválida en línea {}: evento desconocido",
                        index + 1
                    ),
                );
                continue;
            };
            let Some(kind) = parse_event_link_kind(parts[1]) else {
                self.issue(
                    field,
                    format!(
                        "causalidad inválida en línea {}: tipo de enlace desconocido",
                        index + 1
                    ),
                );
                continue;
            };
            links.push(EventLinkSpec {
                target_event_id,
                kind,
            });
        }
        links
    }

    fn parse_goal_ids(&mut self, field: &str) -> Vec<GoalId> {
        let mut goal_ids = Vec::new();
        for line in self.lines(field) {
            match parse_goal_id(&line) {
                Some(goal_id) => goal_ids.push(goal_id),
                None => self.issue(field, format!("meta inválida: {line}")),
            }
        }
        goal_ids
    }

    fn parse_period(&mut self, start_field: &str, end_field: &str) -> Option<Period> {
        let start_tick = self.parse_optional_i64(start_field);
        let end_tick = self.parse_optional_i64(end_field);
        if !self.issues.is_empty() {
            return None;
        }
        if start_tick.is_none() && end_tick.is_none() {
            return None;
        }
        match Period::new(start_tick, end_tick) {
            Ok(period) => Some(period),
            Err(DomainError::InvalidPeriod) => {
                self.issue(end_field, "el periodo no puede terminar antes de empezar");
                None
            }
            Err(other) => {
                self.issue(end_field, other.to_string());
                None
            }
        }
    }

    fn optional_tick_range(
        &mut self,
        start_field: &str,
        end_field: &str,
    ) -> (Option<i64>, Option<i64>) {
        let start_tick = self.parse_optional_i64(start_field);
        let end_tick = self.parse_optional_i64(end_field);
        if let (Some(start), Some(end)) = (start_tick, end_tick) {
            if start > end {
                self.issue(end_field, "el periodo no puede terminar antes de empezar");
            }
        }
        (start_tick, end_tick)
    }

    fn parse_i32(&mut self, field: &str, required: bool) -> Option<i32> {
        let value = self.optional(field);
        match value.as_deref() {
            None if required => {
                self.issue(field, "este campo es obligatorio");
                None
            }
            None => None,
            Some(value) => match value.parse::<i32>() {
                Ok(number) => Some(number),
                Err(_) => {
                    self.issue(field, "debe ser un número entero");
                    None
                }
            },
        }
    }

    fn parse_optional_i64(&mut self, field: &str) -> Option<i64> {
        match self.optional(field).as_deref() {
            None => None,
            Some(value) => match value.parse::<i64>() {
                Ok(number) => Some(number),
                Err(_) => {
                    self.issue(field, "debe ser un número entero");
                    None
                }
            },
        }
    }

    fn parse_optional_f64(&mut self, field: &str) -> Option<f64> {
        match self.optional(field).as_deref() {
            None => None,
            Some(value) => match value.parse::<f64>() {
                Ok(number) => Some(number),
                Err(_) => {
                    self.issue(field, "debe ser un número decimal");
                    None
                }
            },
        }
    }

    fn parse_entity_id_required(&mut self, field: &str) -> Option<EntityId> {
        match self.required(field) {
            Some(value) => match parse_entity_id(&value) {
                Some(entity_id) => Some(entity_id),
                None => {
                    self.issue(field, "usa un UUID o URI nirmata://entity/... válido");
                    None
                }
            },
            None => None,
        }
    }

    fn parse_entity_id_optional(&mut self, field: &str) -> Option<EntityId> {
        match self.optional(field) {
            None => None,
            Some(value) => match parse_entity_id(&value) {
                Some(entity_id) => Some(entity_id),
                None => {
                    self.issue(field, "usa un UUID o URI nirmata://entity/... válido");
                    None
                }
            },
        }
    }

    fn parse_document_id_optional(&mut self, field: &str) -> Option<DocumentId> {
        match self.optional(field) {
            None => None,
            Some(value) => match parse_document_id(&value) {
                Some(document_id) => Some(document_id),
                None => {
                    self.issue(field, "usa un UUID o URI nirmata://document/... válido");
                    None
                }
            },
        }
    }

    fn parse_claim_id_optional(&mut self, field: &str) -> Option<ClaimId> {
        match self.optional(field) {
            None => None,
            Some(value) => match parse_claim_id(&value) {
                Some(claim_id) => Some(claim_id),
                None => {
                    self.issue(field, "usa un UUID o URI nirmata://claim/... válido");
                    None
                }
            },
        }
    }

    fn parse_content_references(
        &mut self,
        field: &str,
        before: Option<&DocumentAggregate>,
    ) -> Vec<ContentReferenceSpec> {
        let raw = self.value(field);
        if raw.trim().is_empty() {
            return before
                .map(|aggregate| {
                    aggregate
                        .references()
                        .iter()
                        .map(|reference| ContentReferenceSpec {
                            target: reference.target(),
                            ordinal: reference.ordinal(),
                        })
                        .collect()
                })
                .unwrap_or_default();
        }

        let mut references = Vec::new();
        for (index, line) in raw
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .enumerate()
        {
            let parts: Vec<_> = line.split('|').map(str::trim).collect();
            let (target_raw, ordinal) = match parts.as_slice() {
                [target] => (*target, index as u32),
                [left, right] => match (left.parse::<u32>(), right.parse::<u32>()) {
                    (Ok(ordinal), Err(_)) => (*right, ordinal),
                    (Err(_), Ok(ordinal)) => (*left, ordinal),
                    _ => {
                        self.issue(
                            field,
                            format!(
                                "referencia inválida en línea {}: usa uri|ordinal u ordinal|uri",
                                index + 1
                            ),
                        );
                        continue;
                    }
                },
                _ => {
                    self.issue(
                        field,
                        format!(
                            "referencia inválida en línea {}: usa uri|ordinal u ordinal|uri",
                            index + 1
                        ),
                    );
                    continue;
                }
            };

            let Some(target) = parse_object_ref(target_raw) else {
                self.issue(
                    field,
                    format!(
                        "referencia inválida en línea {}: usa una URI nirmata://...",
                        index + 1
                    ),
                );
                continue;
            };
            if self.store.resolve_object_ref(target).is_err() {
                self.issue(
                    field,
                    format!(
                        "referencia inválida en línea {}: la URI no existe en el mundo activo",
                        index + 1
                    ),
                );
                continue;
            }
            references.push(ContentReferenceSpec { target, ordinal });
        }

        references
    }

    fn required(&mut self, field: &str) -> Option<String> {
        match self.optional(field) {
            Some(value) => Some(value),
            None => {
                self.issue(field, "este campo es obligatorio");
                None
            }
        }
    }

    fn optional(&self, field: &str) -> Option<String> {
        self.request
            .values
            .get(field)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }

    fn value(&self, field: &str) -> String {
        self.request.values.get(field).cloned().unwrap_or_default()
    }

    fn value_or(&self, field: &str, default: &str) -> String {
        self.request
            .values
            .get(field)
            .cloned()
            .unwrap_or_else(|| default.to_owned())
    }

    fn lines(&self, field: &str) -> Vec<String> {
        self.value(field)
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    }

    fn map_built(
        &mut self,
        field: &str,
        built: Result<BuiltOperation, DomainError>,
    ) -> Option<BuiltOperation> {
        match built {
            Ok(operation) => Some(operation),
            Err(error) => {
                self.issue(field, map_domain_error(&error));
                None
            }
        }
    }

    fn issue(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ManualFieldIssue {
            field: field.into(),
            message: message.into(),
        });
    }
}

pub(crate) fn preview_manual_draft(
    store: &WorldStore,
    session: &WorldSession,
    world: &World,
    request: ManualDraftRequest,
    now_ms: i64,
) -> Result<PreviewManualDraftOutcome, AppError> {
    Builder::new(store, session, world, request, now_ms).build()
}

pub(crate) fn prepare_manual_operation(
    store: &WorldStore,
    session: &WorldSession,
    world: &World,
    request: ManualDraftRequest,
    now_ms: i64,
) -> Result<PreparedManualOperationOutcome, AppError> {
    let mut builder = Builder::new(store, session, world, request, now_ms);
    builder.prepare()
}

pub(crate) fn manual_request_for_review_operation(
    review: &ManualReviewSession,
    operation_id: ChangeOperationId,
) -> Result<ManualDraftRequest, AppError> {
    let operation = review
        .operations()
        .iter()
        .find(|operation| operation.operation_id() == operation_id)
        .ok_or(AppError::UnknownReviewOperation(operation_id))?;
    Ok(operation_request(review, operation.current()))
}

fn operation_request(
    review: &ManualReviewSession,
    operation: &ChangeOperation,
) -> ManualDraftRequest {
    let (object_type, existing_uri, values) = match operation {
        ChangeOperation::UpdateWorld { after, .. } => (
            "world".to_owned(),
            Some(ObjectRef::World(after.id()).to_string()),
            world_values(after),
        ),
        ChangeOperation::CreateEntity { after, .. } => {
            ("entity".to_owned(), None, entity_values(after))
        }
        ChangeOperation::UpdateEntity { after, .. } => (
            "entity".to_owned(),
            Some(ObjectRef::Entity(after.id()).to_string()),
            entity_values(after),
        ),
        ChangeOperation::DeleteEntity { before, .. } => (
            "entity".to_owned(),
            Some(ObjectRef::Entity(before.id()).to_string()),
            entity_values(before),
        ),
        ChangeOperation::CreateRelation { after, .. } => {
            ("relation".to_owned(), None, relation_values(after))
        }
        ChangeOperation::UpdateRelation { after, .. } => (
            "relation".to_owned(),
            Some(ObjectRef::Relation(after.id()).to_string()),
            relation_values(after),
        ),
        ChangeOperation::DeleteRelation { before, .. } => (
            "relation".to_owned(),
            Some(ObjectRef::Relation(before.id()).to_string()),
            relation_values(before),
        ),
        ChangeOperation::CreateEvent { after, .. } => {
            ("event".to_owned(), None, event_values(after))
        }
        ChangeOperation::UpdateEvent { after, .. } => (
            "event".to_owned(),
            Some(ObjectRef::Event(after.event().id()).to_string()),
            event_values(after),
        ),
        ChangeOperation::DeleteEvent { before, .. } => (
            "event".to_owned(),
            Some(ObjectRef::Event(before.event().id()).to_string()),
            event_values(before),
        ),
        ChangeOperation::CreateGoal { after, .. } => ("goal".to_owned(), None, goal_values(after)),
        ChangeOperation::UpdateGoal { after, .. } => (
            "goal".to_owned(),
            Some(ObjectRef::Goal(after.id()).to_string()),
            goal_values(after),
        ),
        ChangeOperation::DeleteGoal { before, .. } => (
            "goal".to_owned(),
            Some(ObjectRef::Goal(before.id()).to_string()),
            goal_values(before),
        ),
        ChangeOperation::CreateRule { after, .. } => ("rule".to_owned(), None, rule_values(after)),
        ChangeOperation::UpdateRule { after, .. } => (
            "rule".to_owned(),
            Some(ObjectRef::Rule(after.id()).to_string()),
            rule_values(after),
        ),
        ChangeOperation::DeleteRule { before, .. } => (
            "rule".to_owned(),
            Some(ObjectRef::Rule(before.id()).to_string()),
            rule_values(before),
        ),
        ChangeOperation::CreateClaim { after, .. } => {
            ("claim".to_owned(), None, claim_values(after))
        }
        ChangeOperation::UpdateClaim { after, .. } => (
            "claim".to_owned(),
            Some(ObjectRef::Claim(after.id()).to_string()),
            claim_values(after),
        ),
        ChangeOperation::DeleteClaim { before, .. } => (
            "claim".to_owned(),
            Some(ObjectRef::Claim(before.id()).to_string()),
            claim_values(before),
        ),
        ChangeOperation::CreateDocument { after, .. } => {
            ("document".to_owned(), None, document_values(after))
        }
        ChangeOperation::UpdateDocument { after, .. } => (
            "document".to_owned(),
            Some(ObjectRef::Document(after.object().id()).to_string()),
            document_values(after),
        ),
        ChangeOperation::DeleteDocument { before, .. } => (
            "document".to_owned(),
            Some(ObjectRef::Document(before.object().id()).to_string()),
            document_values(before),
        ),
    };

    ManualDraftRequest {
        object_type,
        existing_uri,
        objective: Some(review.draft().objective().to_owned()),
        source_uris: review
            .draft()
            .sources()
            .iter()
            .map(ToString::to_string)
            .collect(),
        assumptions: review.draft().assumptions().to_vec(),
        values,
    }
}

fn world_values(world: &World) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("name".to_owned(), world.name().to_owned()),
        ("premise_md".to_owned(), world.premise_md().to_owned()),
        ("epoch_label".to_owned(), world.epoch_label().to_owned()),
    ])
}

fn entity_values(entity: &Entity) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "kind".to_owned(),
            entity_kind_value(entity.kind()).to_owned(),
        ),
        ("name".to_owned(), entity.name().to_owned()),
        ("slug".to_owned(), entity.slug().to_owned()),
        ("aliases".to_owned(), entity.aliases().join("\n")),
        ("summary".to_owned(), entity.summary().to_owned()),
        ("body_md".to_owned(), entity.body_md().to_owned()),
        (
            "attributes_json".to_owned(),
            entity.attributes_json().as_str().to_owned(),
        ),
    ])
}

fn relation_values(relation: &Relation) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "source_entity".to_owned(),
            relation.source_entity_id().to_string(),
        ),
        (
            "target_entity".to_owned(),
            relation.target_entity_id().to_string(),
        ),
        ("kind".to_owned(), relation.kind().to_owned()),
        (
            "direction".to_owned(),
            relation_direction_value(relation.direction()).to_owned(),
        ),
        (
            "certainty".to_owned(),
            certainty_value(relation.certainty()).to_owned(),
        ),
        (
            "valid_from_tick".to_owned(),
            relation
                .valid_from_tick()
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
        (
            "valid_to_tick".to_owned(),
            relation
                .valid_to_tick()
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
        (
            "source_reference".to_owned(),
            relation.source_reference().unwrap_or_default().to_owned(),
        ),
        (
            "metadata_json".to_owned(),
            relation.metadata_json().as_str().to_owned(),
        ),
    ])
}

fn event_values(aggregate: &EventAggregate) -> BTreeMap<String, String> {
    let event = aggregate.event();
    BTreeMap::from([
        ("kind".to_owned(), event.kind().to_owned()),
        ("summary".to_owned(), event.summary().to_owned()),
        ("body_md".to_owned(), event.body_md().to_owned()),
        (
            "time_kind".to_owned(),
            event_time_kind_value(event.time().kind()).to_owned(),
        ),
        (
            "time_precision".to_owned(),
            time_precision_value(event.time().precision()).to_owned(),
        ),
        (
            "time_certainty".to_owned(),
            certainty_value(event.time().certainty()).to_owned(),
        ),
        (
            "start_tick".to_owned(),
            event
                .time()
                .start_tick()
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
        (
            "end_tick".to_owned(),
            event
                .time()
                .end_tick()
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
        (
            "location_entity".to_owned(),
            event
                .location_entity_id()
                .map(|entity_id| entity_id.to_string())
                .unwrap_or_default(),
        ),
        (
            "participants".to_owned(),
            event
                .participants()
                .iter()
                .map(|participant| {
                    format!(
                        "{}|{}|{}",
                        participant.entity_id(),
                        participant.role(),
                        participant.ordinal()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        (
            "affected_goal_ids".to_owned(),
            aggregate
                .event()
                .affected_goal_ids()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        (
            "causal_links".to_owned(),
            aggregate
                .links()
                .iter()
                .map(|link| {
                    format!(
                        "{}|{}",
                        ObjectRef::Event(link.target_event_id()),
                        event_link_kind_value(link.kind())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    ])
}

fn goal_values(goal: &Goal) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "holder_entity".to_owned(),
            goal.holder_entity_id().to_string(),
        ),
        (
            "desired_state_md".to_owned(),
            goal.desired_state_md().to_owned(),
        ),
        ("priority".to_owned(), goal.priority().to_string()),
        (
            "status".to_owned(),
            goal_status_value(goal.status()).to_owned(),
        ),
        (
            "visibility".to_owned(),
            goal_visibility_value(goal.visibility()).to_owned(),
        ),
        (
            "source".to_owned(),
            goal.source().unwrap_or_default().to_owned(),
        ),
        (
            "period_start_tick".to_owned(),
            goal.period()
                .and_then(|period| period.start_tick())
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
        (
            "period_end_tick".to_owned(),
            goal.period()
                .and_then(|period| period.end_tick())
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
    ])
}

fn rule_values(rule: &Rule) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("kind".to_owned(), rule_kind_value(rule.kind()).to_owned()),
        ("statement_md".to_owned(), rule.statement_md().to_owned()),
        ("scope".to_owned(), rule.scope().to_owned()),
        (
            "severity".to_owned(),
            rule_severity_value(rule.severity()).to_owned(),
        ),
        (
            "validator_kind".to_owned(),
            rule.validator_kind()
                .map(rule_validator_kind_value)
                .unwrap_or_default()
                .to_owned(),
        ),
        (
            "source".to_owned(),
            rule.source().unwrap_or_default().to_owned(),
        ),
        (
            "parameters_json".to_owned(),
            rule.parameters_json().as_str().to_owned(),
        ),
    ])
}

fn claim_values(claim: &Claim) -> BTreeMap<String, String> {
    let (object_kind, object_value) = match claim.object() {
        None => ("none".to_owned(), String::new()),
        Some(ClaimObject::Entity(entity_id)) => ("entity".to_owned(), entity_id.to_string()),
        Some(ClaimObject::Scalar(value)) => ("scalar".to_owned(), value.clone()),
    };
    BTreeMap::from([
        (
            "subject_entity".to_owned(),
            claim.subject_entity_id().to_string(),
        ),
        ("content_md".to_owned(), claim.content_md().to_owned()),
        (
            "predicate_key".to_owned(),
            claim.predicate_key().unwrap_or_default().to_owned(),
        ),
        ("object_kind".to_owned(), object_kind),
        ("object_value".to_owned(), object_value),
        (
            "polarity".to_owned(),
            claim_polarity_value(claim.polarity()).to_owned(),
        ),
        (
            "authentication".to_owned(),
            claim_authentication_value(claim.authentication()).to_owned(),
        ),
        (
            "holder_entity".to_owned(),
            claim
                .holder_entity_id()
                .map(|entity_id| entity_id.to_string())
                .unwrap_or_default(),
        ),
        (
            "modality".to_owned(),
            claim
                .modality()
                .map(claim_modality_value)
                .unwrap_or_default()
                .to_owned(),
        ),
        (
            "register".to_owned(),
            claim.register().unwrap_or_default().to_owned(),
        ),
        (
            "epistemic_basis".to_owned(),
            claim.epistemic_basis().unwrap_or_default().to_owned(),
        ),
        (
            "source".to_owned(),
            claim.source().unwrap_or_default().to_owned(),
        ),
        (
            "source_document".to_owned(),
            claim
                .source_document_id()
                .map(|document_id| document_id.to_string())
                .unwrap_or_default(),
        ),
        (
            "source_claim".to_owned(),
            claim
                .source_claim_id()
                .map(|claim_id| claim_id.to_string())
                .unwrap_or_default(),
        ),
        (
            "holder_confidence".to_owned(),
            claim
                .holder_confidence()
                .map(|confidence| confidence.to_string())
                .unwrap_or_default(),
        ),
        (
            "period_start_tick".to_owned(),
            claim
                .period()
                .and_then(|period| period.start_tick())
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
        (
            "period_end_tick".to_owned(),
            claim
                .period()
                .and_then(|period| period.end_tick())
                .map(|tick| tick.to_string())
                .unwrap_or_default(),
        ),
    ])
}

fn document_values(document: &DocumentAggregate) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("title".to_owned(), document.object().title().to_owned()),
        ("kind".to_owned(), document.object().kind().to_owned()),
        (
            "author_entity".to_owned(),
            document
                .object()
                .author_entity_id()
                .map(|entity_id| entity_id.to_string())
                .unwrap_or_default(),
        ),
        (
            "perspective_entity".to_owned(),
            document
                .object()
                .perspective_entity_id()
                .map(|entity_id| entity_id.to_string())
                .unwrap_or_default(),
        ),
        (
            "canon_status".to_owned(),
            document_canon_status_value(document.object().canon_status()).to_owned(),
        ),
        ("body_md".to_owned(), document.object().body_md().to_owned()),
        (
            "content_references".to_owned(),
            document
                .references()
                .iter()
                .map(|reference| format!("{}|{}", reference.target(), reference.ordinal()))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    ])
}

fn entity_kind_value(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Person => "person",
        EntityKind::Place => "place",
        EntityKind::Faction => "faction",
        EntityKind::Culture => "culture",
        EntityKind::Resource => "resource",
        EntityKind::Concept => "concept",
    }
}

fn relation_direction_value(direction: RelationDirection) -> &'static str {
    match direction {
        RelationDirection::Directed => "directed",
        RelationDirection::Undirected => "undirected",
    }
}

fn certainty_value(certainty: Certainty) -> &'static str {
    match certainty {
        Certainty::Certain => "certain",
        Certainty::Approximate => "approximate",
        Certainty::Uncertain => "uncertain",
        Certainty::ApproximateUncertain => "approximate_uncertain",
    }
}

fn event_time_kind_value(kind: EventTimeKind) -> &'static str {
    match kind {
        EventTimeKind::Unknown => "unknown",
        EventTimeKind::Instant => "instant",
        EventTimeKind::Interval => "interval",
        EventTimeKind::Ongoing => "ongoing",
    }
}

fn time_precision_value(precision: TimePrecision) -> &'static str {
    match precision {
        TimePrecision::Exact => "exact",
        TimePrecision::Day => "day",
        TimePrecision::Month => "month",
        TimePrecision::Year => "year",
        TimePrecision::Era => "era",
        TimePrecision::Unknown => "unknown",
    }
}

fn event_link_kind_value(kind: EventLinkKind) -> &'static str {
    match kind {
        EventLinkKind::Enables => "enables",
        EventLinkKind::Causes => "causes",
        EventLinkKind::Motivates => "motivates",
        EventLinkKind::Prevents => "prevents",
        EventLinkKind::Terminates => "terminates",
        EventLinkKind::Reveals => "reveals",
    }
}

fn goal_status_value(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Active => "active",
        GoalStatus::Achieved => "achieved",
        GoalStatus::Abandoned => "abandoned",
        GoalStatus::Frustrated => "frustrated",
    }
}

fn goal_visibility_value(visibility: GoalVisibility) -> &'static str {
    match visibility {
        GoalVisibility::Public => "public",
        GoalVisibility::Secret => "secret",
    }
}

fn rule_kind_value(kind: RuleKind) -> &'static str {
    match kind {
        RuleKind::Constitutive => "constitutive",
        RuleKind::Generative => "generative",
        RuleKind::Institutional => "institutional",
        RuleKind::Authorial => "authorial",
    }
}

fn rule_severity_value(severity: RuleSeverity) -> &'static str {
    match severity {
        RuleSeverity::Advisory => "advisory",
        RuleSeverity::Hard => "hard",
    }
}

fn rule_validator_kind_value(kind: RuleValidatorKind) -> &'static str {
    match kind {
        RuleValidatorKind::NoResurrection => "no_resurrection",
    }
}

fn claim_polarity_value(polarity: ClaimPolarity) -> &'static str {
    match polarity {
        ClaimPolarity::Positive => "positive",
        ClaimPolarity::Negative => "negative",
    }
}

fn claim_authentication_value(authentication: ClaimAuthentication) -> &'static str {
    match authentication {
        ClaimAuthentication::Canonical => "canonical",
        ClaimAuthentication::Attributed => "attributed",
        ClaimAuthentication::Disputed => "disputed",
    }
}

fn claim_modality_value(modality: ClaimModality) -> &'static str {
    match modality {
        ClaimModality::Assertion => "assertion",
        ClaimModality::Belief => "belief",
        ClaimModality::Hypothesis => "hypothesis",
        ClaimModality::Counterfactual => "counterfactual",
    }
}

fn document_canon_status_value(status: DocumentCanonStatus) -> &'static str {
    match status {
        DocumentCanonStatus::Canonical => "canonical",
        DocumentCanonStatus::NonCanonical => "non_canonical",
    }
}

fn parse_enum<T>(
    field: &str,
    raw: &str,
    issues: &mut Vec<ManualFieldIssue>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Option<T> {
    match parse(raw.trim()) {
        Some(value) => Some(value),
        None => {
            issues.push(ManualFieldIssue {
                field: field.to_owned(),
                message: "valor inválido".to_owned(),
            });
            None
        }
    }
}

fn parse_entity_id(value: &str) -> Option<EntityId> {
    parse_object_id(value, EntityId::from_str, |reference| match reference {
        ObjectRef::Entity(id) => Some(id),
        _ => None,
    })
}

fn parse_goal_id(value: &str) -> Option<GoalId> {
    parse_object_id(value, GoalId::from_str, |reference| match reference {
        ObjectRef::Goal(id) => Some(id),
        _ => None,
    })
}

fn parse_event_id(value: &str) -> Option<nirmata_core::EventId> {
    parse_object_id(
        value,
        nirmata_core::EventId::from_str,
        |reference| match reference {
            ObjectRef::Event(id) => Some(id),
            _ => None,
        },
    )
}

fn parse_document_id(value: &str) -> Option<DocumentId> {
    parse_object_id(value, DocumentId::from_str, |reference| match reference {
        ObjectRef::Document(id) => Some(id),
        _ => None,
    })
}

fn parse_claim_id(value: &str) -> Option<ClaimId> {
    parse_object_id(value, ClaimId::from_str, |reference| match reference {
        ObjectRef::Claim(id) => Some(id),
        _ => None,
    })
}

fn parse_object_ref(value: &str) -> Option<ObjectRef> {
    ObjectRef::from_str(value.trim()).ok()
}

fn build_content_references(
    source: ObjectRef,
    references: &[ContentReferenceSpec],
) -> Vec<ContentReference> {
    references
        .iter()
        .map(|reference| ContentReference::new(source, reference.target, reference.ordinal))
        .collect()
}

fn parse_object_id<T, E>(
    value: &str,
    direct: impl Fn(&str) -> Result<T, E>,
    from_ref: impl Fn(ObjectRef) -> Option<T>,
) -> Option<T> {
    let trimmed = value.trim();
    if trimmed.starts_with("nirmata://") {
        ObjectRef::from_str(trimmed).ok().and_then(from_ref)
    } else {
        direct(trimmed).ok()
    }
}

fn entity_group_name(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Person => "people",
        EntityKind::Place => "places",
        EntityKind::Faction => "factions",
        EntityKind::Culture => "cultures",
        EntityKind::Resource => "resources",
        EntityKind::Concept => "concepts",
    }
}

fn display_name(value: &str, fallback: String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed.to_owned()
    }
}

fn preview(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else if trimmed.chars().count() > 80 {
        format!(
            "{}…",
            trimmed.chars().take(80).collect::<String>().trim_end()
        )
    } else {
        trimmed.to_owned()
    }
}

fn sanitize_segment(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace(' ', "-");
    if normalized.is_empty() {
        "unknown".to_owned()
    } else {
        normalized
    }
}

fn build_event_links(
    source_event_id: nirmata_core::EventId,
    specs: &[EventLinkSpec],
) -> Result<Vec<EventLink>, DomainError> {
    specs
        .iter()
        .map(|spec| EventLink::new(source_event_id, spec.target_event_id, spec.kind))
        .collect()
}

fn parse_event_link_kind(value: &str) -> Option<EventLinkKind> {
    match value.trim() {
        "enables" => Some(EventLinkKind::Enables),
        "causes" => Some(EventLinkKind::Causes),
        "motivates" => Some(EventLinkKind::Motivates),
        "prevents" => Some(EventLinkKind::Prevents),
        "terminates" => Some(EventLinkKind::Terminates),
        "reveals" => Some(EventLinkKind::Reveals),
        _ => None,
    }
}

fn map_domain_error(error: &DomainError) -> String {
    match error {
        DomainError::EmptyField { .. }
        | DomainError::InvalidJsonObject { .. }
        | DomainError::InvalidPeriod
        | DomainError::InvalidEventTime
        | DomainError::HardRuleWithoutValidator
        | DomainError::InvalidRuleValidatorParameters { .. }
        | DomainError::DuplicateAlias(_)
        | DomainError::DuplicateOrdinal(_)
        | DomainError::DuplicateReference
        | DomainError::InvalidClaimContext(_)
        | DomainError::InvalidConfidence
        | DomainError::TextTooLong { .. } => error.to_string(),
        _ => format!("no se pudo construir el draft manual: {error}"),
    }
}
