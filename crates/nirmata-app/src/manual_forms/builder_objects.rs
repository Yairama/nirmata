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
            self.store.active_variant()?.id,
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
        let calendar = self.build_calendar();
        if !self.issues.is_empty() {
            return Ok(None);
        }

        let built = World::restore(
            self.world.id(),
            name.expect("world name"),
            premise_md,
            epoch_label,
            calendar,
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

    fn build_calendar(&mut self) -> Option<WorldCalendar> {
        match self.value_or("calendar_mode", "none").trim() {
            "none" => None,
            "fixed" => {
                let name = self.required("calendar_name");
                let epoch_tick = self.parse_optional_i64("calendar_epoch_tick");
                if epoch_tick.is_none() {
                    self.issue("calendar_epoch_tick", "este campo es obligatorio");
                }
                let ticks_per_day = self.parse_optional_i64("calendar_ticks_per_day");
                if ticks_per_day.is_none() {
                    self.issue("calendar_ticks_per_day", "este campo es obligatorio");
                }
                let weekdays = self.lines("calendar_weekdays");
                if weekdays.is_empty() {
                    self.issue("calendar_weekdays", "define al menos un día semanal");
                }
                let mut months = Vec::new();
                for (index, line) in self.lines("calendar_months").iter().enumerate() {
                    let Some((month_name, days)) = line.split_once('|') else {
                        self.issue(
                            "calendar_months",
                            format!("línea {}: usa nombre|días", index + 1),
                        );
                        continue;
                    };
                    let Ok(days) = days.trim().parse::<u32>() else {
                        self.issue(
                            "calendar_months",
                            format!("línea {}: días debe ser entero", index + 1),
                        );
                        continue;
                    };
                    match CalendarMonth::new(month_name, days) {
                        Ok(month) => months.push(month),
                        Err(error) => self.issue("calendar_months", error.to_string()),
                    }
                }
                if months.is_empty() {
                    self.issue("calendar_months", "define al menos un mes nombre|días");
                }
                if !self.issues.is_empty() {
                    return None;
                }
                match WorldCalendar::new(
                    name.expect("calendar name"),
                    epoch_tick.expect("calendar epoch"),
                    ticks_per_day.expect("calendar ticks per day"),
                    weekdays,
                    months,
                ) {
                    Ok(calendar) => Some(calendar),
                    Err(error) => {
                        self.issue("calendar_mode", error.to_string());
                        None
                    }
                }
            }
            _ => {
                self.issue("calendar_mode", "usa none o fixed");
                None
            }
        }
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

}
