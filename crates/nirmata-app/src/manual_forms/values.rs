fn world_values(world: &World) -> BTreeMap<String, String> {
    let mut values = BTreeMap::from([
        ("name".to_owned(), world.name().to_owned()),
        ("premise_md".to_owned(), world.premise_md().to_owned()),
        ("epoch_label".to_owned(), world.epoch_label().to_owned()),
    ]);
    if let Some(calendar) = world.calendar() {
        values.insert("calendar_mode".to_owned(), "fixed".to_owned());
        values.insert("calendar_name".to_owned(), calendar.name().to_owned());
        values.insert(
            "calendar_epoch_tick".to_owned(),
            calendar.epoch_tick().to_string(),
        );
        values.insert(
            "calendar_ticks_per_day".to_owned(),
            calendar.ticks_per_day().to_string(),
        );
        values.insert(
            "calendar_weekdays".to_owned(),
            calendar.weekday_names().join("\n"),
        );
        values.insert(
            "calendar_months".to_owned(),
            calendar
                .months()
                .iter()
                .map(|month| format!("{}|{}", month.name(), month.days()))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    } else {
        values.insert("calendar_mode".to_owned(), "none".to_owned());
        for field in [
            "calendar_name",
            "calendar_epoch_tick",
            "calendar_ticks_per_day",
            "calendar_weekdays",
            "calendar_months",
        ] {
            values.insert(field.to_owned(), String::new());
        }
    }
    values
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
