impl<'a> Builder<'a> {
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
        let start_date_tick = self.parse_calendar_date("start_calendar_date");
        let end_date_tick = self.parse_calendar_date("end_calendar_date");
        if start_tick.is_some() && start_date_tick.is_some() && start_tick != start_date_tick {
            self.issue("start_calendar_date", "la fecha no coincide con start_tick");
        }
        if end_tick.is_some() && end_date_tick.is_some() && end_tick != end_date_tick {
            self.issue("end_calendar_date", "la fecha no coincide con end_tick");
        }
        let start_tick = start_date_tick.or(start_tick);
        let end_tick = end_date_tick.or(end_tick);
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

    fn parse_calendar_date(&mut self, field: &str) -> Option<i64> {
        let value = self.optional(field)?;
        let Some(calendar) = self.world.calendar() else {
            self.issue(field, "el mundo no tiene calendario configurado");
            return None;
        };
        let parts = value
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if parts.len() != 4 {
            self.issue(field, "usa año|mes|día|sub-tick");
            return None;
        }
        let parsed = (
            parts[0].parse::<i64>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
            parts[3].parse::<i64>(),
        );
        let (Ok(year), Ok(month), Ok(day), Ok(tick_in_day)) = parsed else {
            self.issue(field, "año, mes, día y sub-tick deben ser enteros");
            return None;
        };
        match calendar.date_to_tick(CalendarDate::new(year, month, day, tick_in_day)) {
            Ok(tick) => Some(tick),
            Err(error) => {
                self.issue(field, error.to_string());
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
