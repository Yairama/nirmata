use crate::AppError;
use nirmata_core::{
    EntityId, GoalId, Period,
    claim::{Claim, ClaimAuthentication, ClaimObject},
    document::{DocumentCanonStatus, ObjectRef},
    event::Event,
    time::{EventTime, EventTimeKind},
};
use nirmata_store::{
    ReadScope, ResolvedObject, StructuredSearchQuery, StructuredSearchStage,
    StructuredSearchTemporal, WorldStore,
};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextIntent {
    EntityQuery,
    ImpactAnalysis,
    ContradictionCheck,
    DocumentDraft,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStage {
    Selection,
    Relation,
    Temporal,
    Goal,
    Perspective,
    Search,
    Semantic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextBudget {
    pub max_objects: usize,
    pub max_chars: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_objects: 24,
            max_chars: 4_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContextBudgetUsage {
    pub max_objects: usize,
    pub max_chars: usize,
    pub used_objects: usize,
    pub used_chars: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextBundleRequest {
    pub intent: ContextIntent,
    pub anchors: Vec<ObjectRef>,
    pub query_text: Option<String>,
    pub temporal: Option<StructuredSearchTemporal>,
    pub temporal_radius: Option<i64>,
    pub perspective_entity_ids: Vec<EntityId>,
    pub include_perspectives: bool,
    pub relation_limit: usize,
    pub budget: ContextBudget,
}

impl ContextBundleRequest {
    pub fn new(intent: ContextIntent) -> Self {
        Self {
            intent,
            anchors: vec![],
            query_text: None,
            temporal: None,
            temporal_radius: None,
            perspective_entity_ids: vec![],
            include_perspectives: false,
            relation_limit: 8,
            budget: ContextBudget::default(),
        }
    }

    fn wants_perspectives(&self) -> bool {
        self.include_perspectives || matches!(self.intent, ContextIntent::DocumentDraft)
    }

    fn default_temporal_radius(&self) -> Option<i64> {
        match self.intent {
            ContextIntent::ImpactAnalysis => Some(3),
            ContextIntent::ContradictionCheck | ContextIntent::DocumentDraft => Some(0),
            ContextIntent::EntityQuery => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntry {
    pub object: ResolvedObject,
    pub uri: String,
    pub citation: String,
    pub provenance: String,
    pub stage: ContextStage,
    pub score: u32,
    pub rank: usize,
    pub score_explanation: String,
}

impl ContextEntry {
    pub fn object_ref(&self) -> ObjectRef {
        self.object.object_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ContextSection {
    Canon,
    Perspectives,
    Desires,
    Obligations,
    SearchEvidence,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextBundle {
    pub canon: Vec<ContextEntry>,
    pub perspectives: Vec<ContextEntry>,
    pub desires: Vec<ContextEntry>,
    pub obligations: Vec<ContextEntry>,
    pub search_evidence: Vec<ContextEntry>,
    pub usage: ContextBudgetUsage,
}

impl ContextBundle {
    fn with_budget(budget: ContextBudget) -> Self {
        Self {
            canon: vec![],
            perspectives: vec![],
            desires: vec![],
            obligations: vec![],
            search_evidence: vec![],
            usage: ContextBudgetUsage {
                max_objects: budget.max_objects,
                max_chars: budget.max_chars,
                used_objects: 0,
                used_chars: 0,
            },
        }
    }

    pub fn all_entries(&self) -> Vec<&ContextEntry> {
        self.canon
            .iter()
            .chain(self.perspectives.iter())
            .chain(self.desires.iter())
            .chain(self.obligations.iter())
            .chain(self.search_evidence.iter())
            .collect()
    }

    pub fn contains(&self, object: ObjectRef) -> bool {
        self.all_entries()
            .into_iter()
            .any(|entry| entry.object_ref() == object)
    }
}

pub(crate) fn build_context_bundle_scoped(
    store: &WorldStore,
    scope: ReadScope,
    request: &ContextBundleRequest,
) -> Result<ContextBundle, AppError> {
    let mut collector = ContextCollector::new(request.budget, &request.perspective_entity_ids);
    if request.budget.max_objects == 0 || request.budget.max_chars == 0 {
        return Ok(collector.finish());
    }

    let anchor_refs = dedup_refs(&request.anchors);
    let mut resolved_anchors = Vec::with_capacity(anchor_refs.len());
    let mut context_seed_refs = anchor_refs.clone();

    for anchor in anchor_refs {
        let resolved = store.resolve_object_ref_scoped(scope, anchor)?;
        collector.add(
            resolved.clone(),
            ContextStage::Selection,
            format!("anchor:{anchor}"),
            None,
            true,
            false,
        );
        for related in related_refs(&resolved) {
            context_seed_refs.push(related);
            if collector.is_exhausted() {
                continue;
            }
            let related_resolved = store.resolve_object_ref_scoped(scope, related)?;
            collector.add(
                related_resolved,
                ContextStage::Relation,
                format!("selection_related:{anchor}"),
                None,
                request.wants_perspectives(),
                false,
            );
        }
        resolved_anchors.push(resolved);
    }

    if !collector.is_exhausted() && !context_seed_refs.is_empty() {
        let bundle = store.load_anchor_context_scoped(
            scope,
            &nirmata_store::AnchorContextQuery {
                anchors: dedup_refs(&context_seed_refs),
                relation_limit: request.relation_limit,
            },
        )?;

        for entry in bundle.relations {
            collector.add(
                entry.object,
                ContextStage::Relation,
                entry.provenance,
                None,
                request.wants_perspectives(),
                false,
            );
        }
        for entry in bundle.events {
            collector.add(
                entry.object,
                ContextStage::Relation,
                entry.provenance,
                None,
                request.wants_perspectives(),
                false,
            );
        }
        for entry in bundle.participants {
            collector.add(
                entry.object,
                ContextStage::Relation,
                entry.provenance,
                None,
                request.wants_perspectives(),
                false,
            );
        }
        for entry in bundle.claims {
            collector.add(
                entry.object,
                ContextStage::Relation,
                entry.provenance,
                None,
                request.wants_perspectives(),
                false,
            );
        }
        for entry in bundle.goals {
            collector.add(
                entry.object,
                ContextStage::Goal,
                entry.provenance,
                None,
                request.wants_perspectives(),
                false,
            );
        }
        for entry in bundle.rules {
            collector.add(
                entry.object,
                ContextStage::Relation,
                entry.provenance,
                None,
                request.wants_perspectives(),
                false,
            );
        }
    }

    if !collector.is_exhausted() {
        if let Some(temporal) = effective_temporal(request, &resolved_anchors, &collector.bundle) {
            for hit in store.search_structured_scoped(
                scope,
                &StructuredSearchQuery {
                    temporal: Some(temporal),
                    limit: stage_fetch_limit(&collector),
                    ..Default::default()
                },
            )? {
                let resolved = store.resolve_object_ref_scoped(scope, hit.object)?;
                if !is_related_to_context(&resolved, collector.context_refs()) {
                    continue;
                }
                collector.add(
                    resolved,
                    ContextStage::Temporal,
                    hit.provenance,
                    Some(hit.fragment),
                    request.wants_perspectives(),
                    false,
                );
            }
        }
    }

    if !collector.is_exhausted() {
        let goal_ids = goal_ids_from_bundle(&collector.bundle);
        if !goal_ids.is_empty() {
            for hit in store.search_structured_scoped(
                scope,
                &StructuredSearchQuery {
                    goal_ids,
                    limit: stage_fetch_limit(&collector),
                    ..Default::default()
                },
            )? {
                let resolved = store.resolve_object_ref_scoped(scope, hit.object)?;
                collector.add(
                    resolved,
                    ContextStage::Goal,
                    hit.provenance,
                    Some(hit.fragment),
                    request.wants_perspectives(),
                    false,
                );
            }
        }
    }

    if !collector.is_exhausted() && request.wants_perspectives() {
        let perspective_entity_ids = perspective_ids_from_bundle(request, &collector.bundle);
        if !perspective_entity_ids.is_empty() {
            for hit in store.search_structured_scoped(
                scope,
                &StructuredSearchQuery {
                    perspective_entity_ids,
                    limit: stage_fetch_limit(&collector),
                    ..Default::default()
                },
            )? {
                let resolved = store.resolve_object_ref_scoped(scope, hit.object)?;
                collector.add(
                    resolved,
                    ContextStage::Perspective,
                    hit.provenance,
                    Some(hit.fragment),
                    true,
                    false,
                );
            }
        }
    }

    if !collector.is_exhausted() {
        if let Some(text) = effective_query_text(request, &resolved_anchors, &collector.bundle) {
            for hit in store.search_structured_scoped(
                scope,
                &StructuredSearchQuery {
                    text: Some(text),
                    limit: stage_fetch_limit(&collector),
                    ..Default::default()
                },
            )? {
                let resolved = store.resolve_object_ref_scoped(scope, hit.object)?;
                collector.add(
                    resolved,
                    if hit.stage == StructuredSearchStage::Semantic {
                        ContextStage::Semantic
                    } else {
                        ContextStage::Search
                    },
                    hit.provenance,
                    Some(hit.fragment),
                    request.wants_perspectives(),
                    true,
                );
            }
        }
    }

    Ok(collector.finish())
}

struct ContextCollector {
    bundle: ContextBundle,
    seen: BTreeSet<ObjectRef>,
    context_refs: BTreeSet<ObjectRef>,
    perspective_filter: BTreeSet<EntityId>,
}

impl ContextCollector {
    fn new(budget: ContextBudget, perspective_entity_ids: &[EntityId]) -> Self {
        Self {
            bundle: ContextBundle::with_budget(budget),
            seen: BTreeSet::new(),
            context_refs: BTreeSet::new(),
            perspective_filter: perspective_entity_ids.iter().copied().collect(),
        }
    }

    fn is_exhausted(&self) -> bool {
        self.remaining_objects() == 0 || self.remaining_chars() == 0
    }

    fn remaining_objects(&self) -> usize {
        self.bundle
            .usage
            .max_objects
            .saturating_sub(self.bundle.usage.used_objects)
    }

    fn remaining_chars(&self) -> usize {
        self.bundle
            .usage
            .max_chars
            .saturating_sub(self.bundle.usage.used_chars)
    }

    fn context_refs(&self) -> &BTreeSet<ObjectRef> {
        &self.context_refs
    }

    fn add(
        &mut self,
        object: ResolvedObject,
        stage: ContextStage,
        provenance: String,
        citation: Option<String>,
        allow_perspectives: bool,
        force_search_evidence: bool,
    ) -> bool {
        if self.is_exhausted() {
            return false;
        }

        let object_ref = object.object_ref();
        if stage != ContextStage::Selection
            && !matches_perspective_filter(&object, &self.perspective_filter)
        {
            return false;
        }
        if !self.seen.insert(object_ref) {
            return false;
        }

        let Some(section) = section_for_object(&object, allow_perspectives, force_search_evidence)
        else {
            self.seen.remove(&object_ref);
            return false;
        };

        let citation = clip_chars(
            &normalize_text(citation.unwrap_or_else(|| citation_for_object(&object))),
            self.remaining_chars(),
        );
        if citation.is_empty() {
            self.seen.remove(&object_ref);
            return false;
        }

        self.context_refs.insert(object_ref);
        self.context_refs.extend(related_refs(&object));
        self.bundle.usage.used_objects += 1;
        self.bundle.usage.used_chars += citation.chars().count();

        let rank = self.bundle.usage.used_objects;
        let score = context_score(stage, &provenance);

        let entry = ContextEntry {
            uri: object_ref.to_string(),
            object,
            citation,
            provenance,
            stage,
            score,
            rank,
            score_explanation: context_score_explanation(stage, score),
        };
        match section {
            ContextSection::Canon => self.bundle.canon.push(entry),
            ContextSection::Perspectives => self.bundle.perspectives.push(entry),
            ContextSection::Desires => self.bundle.desires.push(entry),
            ContextSection::Obligations => self.bundle.obligations.push(entry),
            ContextSection::SearchEvidence => self.bundle.search_evidence.push(entry),
        }
        true
    }

    fn finish(self) -> ContextBundle {
        self.bundle
    }
}

fn context_score(stage: ContextStage, provenance: &str) -> u32 {
    match stage {
        ContextStage::Selection => 100_000,
        ContextStage::Relation => 90_000,
        ContextStage::Temporal => 80_000,
        ContextStage::Goal => 70_000,
        ContextStage::Perspective => 60_000,
        ContextStage::Search => 30_000,
        ContextStage::Semantic => {
            10_000
                + provenance
                    .rsplit_once("matched_bps=")
                    .and_then(|(_, value)| value.parse::<u32>().ok())
                    .unwrap_or(0)
        }
    }
}

fn context_score_explanation(stage: ContextStage, score: u32) -> String {
    match stage {
        ContextStage::Semantic => format!(
            "semantic WordNet concept match; {} basis points",
            score.saturating_sub(10_000)
        ),
        _ => format!("fixed deterministic priority for {stage:?} context"),
    }
}

fn stage_fetch_limit(collector: &ContextCollector) -> usize {
    collector.remaining_objects().saturating_mul(4).max(8)
}

fn section_for_object(
    object: &ResolvedObject,
    allow_perspectives: bool,
    force_search_evidence: bool,
) -> Option<ContextSection> {
    if force_search_evidence {
        if !allow_perspectives && is_perspective_only(object) {
            return None;
        }
        return Some(ContextSection::SearchEvidence);
    }

    match object {
        ResolvedObject::World(_) => Some(ContextSection::Canon),
        ResolvedObject::Entity(_) | ResolvedObject::Relation(_) | ResolvedObject::Event(_) => {
            Some(ContextSection::Canon)
        }
        ResolvedObject::Goal(_) => Some(ContextSection::Desires),
        ResolvedObject::Rule(_) => Some(ContextSection::Obligations),
        ResolvedObject::Claim(claim) => match claim.authentication() {
            ClaimAuthentication::Canonical => Some(ContextSection::Canon),
            ClaimAuthentication::Attributed | ClaimAuthentication::Disputed => {
                allow_perspectives.then_some(ContextSection::Perspectives)
            }
        },
        ResolvedObject::Document(aggregate) => match aggregate.object().canon_status() {
            DocumentCanonStatus::Canonical => Some(ContextSection::Canon),
            DocumentCanonStatus::NonCanonical => {
                allow_perspectives.then_some(ContextSection::Perspectives)
            }
        },
    }
}

fn is_perspective_only(object: &ResolvedObject) -> bool {
    match object {
        ResolvedObject::World(_) => false,
        ResolvedObject::Claim(claim) => claim.authentication() != ClaimAuthentication::Canonical,
        ResolvedObject::Document(aggregate) => {
            aggregate.object().canon_status() == DocumentCanonStatus::NonCanonical
        }
        ResolvedObject::Entity(_)
        | ResolvedObject::Relation(_)
        | ResolvedObject::Event(_)
        | ResolvedObject::Rule(_)
        | ResolvedObject::Goal(_) => false,
    }
}

fn matches_perspective_filter(object: &ResolvedObject, filter: &BTreeSet<EntityId>) -> bool {
    if filter.is_empty() || !is_perspective_only(object) {
        return true;
    }

    perspective_owner_ids(object)
        .into_iter()
        .any(|entity_id| filter.contains(&entity_id))
}

fn perspective_owner_ids(object: &ResolvedObject) -> Vec<EntityId> {
    match object {
        ResolvedObject::World(_) => vec![],
        ResolvedObject::Claim(claim) => claim.holder_entity_id().into_iter().collect(),
        ResolvedObject::Document(aggregate) => {
            let mut owners = Vec::new();
            if let Some(perspective_id) = aggregate.object().perspective_entity_id() {
                owners.push(perspective_id);
            }
            if let Some(author_id) = aggregate.object().author_entity_id() {
                if !owners.contains(&author_id) {
                    owners.push(author_id);
                }
            }
            owners
        }
        ResolvedObject::Entity(_)
        | ResolvedObject::Relation(_)
        | ResolvedObject::Event(_)
        | ResolvedObject::Rule(_)
        | ResolvedObject::Goal(_) => vec![],
    }
}

fn dedup_refs(values: &[ObjectRef]) -> Vec<ObjectRef> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(*value) {
            deduped.push(*value);
        }
    }
    deduped
}

fn effective_temporal(
    request: &ContextBundleRequest,
    resolved_anchors: &[ResolvedObject],
    bundle: &ContextBundle,
) -> Option<StructuredSearchTemporal> {
    if let Some(temporal) = request.temporal {
        return Some(temporal);
    }

    let radius = request
        .temporal_radius
        .or_else(|| request.default_temporal_radius())?;
    resolved_anchors
        .iter()
        .chain(bundle.canon.iter().map(|entry| &entry.object))
        .chain(bundle.desires.iter().map(|entry| &entry.object))
        .chain(bundle.perspectives.iter().map(|entry| &entry.object))
        .find_map(|object| {
            temporal_bounds_for_object(object).map(|(start, end)| {
                StructuredSearchTemporal::Period(
                    Period::new(
                        start.map(|value| value.saturating_sub(radius)),
                        end.map(|value| value.saturating_add(radius)),
                    )
                    .expect("derived temporal window stays ordered"),
                )
            })
        })
}

fn effective_query_text(
    request: &ContextBundleRequest,
    resolved_anchors: &[ResolvedObject],
    bundle: &ContextBundle,
) -> Option<String> {
    normalize_query_text(request.query_text.as_deref()).or_else(|| {
        resolved_anchors
            .iter()
            .chain(bundle.canon.iter().map(|entry| &entry.object))
            .find_map(search_seed_text)
    })
}

fn goal_ids_from_bundle(bundle: &ContextBundle) -> Vec<GoalId> {
    let mut ids = BTreeSet::new();
    for entry in bundle.all_entries() {
        match &entry.object {
            ResolvedObject::Goal(goal) => {
                ids.insert(goal.id());
            }
            ResolvedObject::Event(aggregate) => {
                ids.extend(aggregate.event().affected_goal_ids().iter().copied());
            }
            _ => {}
        }
    }
    ids.into_iter().collect()
}

fn perspective_ids_from_bundle(
    request: &ContextBundleRequest,
    bundle: &ContextBundle,
) -> Vec<EntityId> {
    let mut ids = BTreeSet::new();
    ids.extend(request.perspective_entity_ids.iter().copied());
    for entry in bundle.all_entries() {
        match &entry.object {
            ResolvedObject::Claim(claim) => {
                if let Some(holder_id) = claim.holder_entity_id() {
                    ids.insert(holder_id);
                }
            }
            ResolvedObject::Document(aggregate) => {
                if let Some(author_id) = aggregate.object().author_entity_id() {
                    ids.insert(author_id);
                }
                if let Some(perspective_id) = aggregate.object().perspective_entity_id() {
                    ids.insert(perspective_id);
                }
            }
            _ => {}
        }
    }
    ids.into_iter().collect()
}

fn normalize_query_text(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn citation_for_object(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::World(world) => {
            preview(&[world.name(), world.premise_md(), world.epoch_label()])
        }
        ResolvedObject::Entity(entity) => {
            preview(&[entity.name(), entity.summary(), entity.body_md()])
        }
        ResolvedObject::Relation(relation) => preview(&[
            relation.kind(),
            relation.source_reference().unwrap_or_default(),
        ]),
        ResolvedObject::Event(aggregate) => {
            preview(&[aggregate.event().summary(), aggregate.event().body_md()])
        }
        ResolvedObject::Claim(claim) => preview(&[
            claim.content_md(),
            claim.epistemic_basis().unwrap_or_default(),
            claim.source().unwrap_or_default(),
        ]),
        ResolvedObject::Rule(rule) => {
            preview(&[rule.statement_md(), rule.source().unwrap_or_default()])
        }
        ResolvedObject::Goal(goal) => {
            preview(&[goal.desired_state_md(), goal.source().unwrap_or_default()])
        }
        ResolvedObject::Document(aggregate) => {
            preview(&[aggregate.object().title(), aggregate.object().body_md()])
        }
    }
}

fn search_seed_text(object: &ResolvedObject) -> Option<String> {
    let raw = match object {
        ResolvedObject::World(world) => world.name(),
        ResolvedObject::Entity(entity) => entity.name(),
        ResolvedObject::Relation(relation) => relation.kind(),
        ResolvedObject::Event(aggregate) => aggregate.event().summary(),
        ResolvedObject::Claim(claim) => claim.predicate_key().unwrap_or_else(|| claim.content_md()),
        ResolvedObject::Rule(rule) => rule.statement_md(),
        ResolvedObject::Goal(goal) => goal.desired_state_md(),
        ResolvedObject::Document(aggregate) => aggregate.object().title(),
    };
    let text = raw.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

fn preview(parts: &[&str]) -> String {
    let text = parts
        .iter()
        .copied()
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ");
    let text = if text.chars().count() > 160 {
        format!("{}…", clip_chars(&text, 160))
    } else {
        text
    };
    if text.is_empty() {
        "sin cita".to_owned()
    } else {
        text
    }
}

fn normalize_text(value: String) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        "sin cita".to_owned()
    } else {
        normalized
    }
}

fn clip_chars(value: &str, limit: usize) -> String {
    if limit == 0 {
        return String::new();
    }
    let total = value.chars().count();
    if total <= limit {
        return value.to_owned();
    }
    let mut end = value.len();
    for (count, (index, _)) in value.char_indices().enumerate() {
        if count == limit {
            end = index;
            break;
        }
    }
    value[..end].trim_end().to_owned()
}

fn related_refs(object: &ResolvedObject) -> Vec<ObjectRef> {
    match object {
        ResolvedObject::World(_) | ResolvedObject::Entity(_) | ResolvedObject::Rule(_) => vec![],
        ResolvedObject::Relation(relation) => vec![
            ObjectRef::Entity(relation.source_entity_id()),
            ObjectRef::Entity(relation.target_entity_id()),
        ],
        ResolvedObject::Event(aggregate) => {
            let mut refs = event_related_refs(aggregate.event());
            refs.extend(aggregate.links().iter().flat_map(|link| {
                [
                    ObjectRef::Event(link.source_event_id()),
                    ObjectRef::Event(link.target_event_id()),
                ]
            }));
            refs
        }
        ResolvedObject::Claim(claim) => claim_related_refs(claim),
        ResolvedObject::Goal(goal) => vec![ObjectRef::Entity(goal.holder_entity_id())],
        ResolvedObject::Document(aggregate) => {
            let mut refs = Vec::new();
            if let Some(author_id) = aggregate.object().author_entity_id() {
                refs.push(ObjectRef::Entity(author_id));
            }
            if let Some(perspective_id) = aggregate.object().perspective_entity_id() {
                refs.push(ObjectRef::Entity(perspective_id));
            }
            refs.extend(
                aggregate
                    .references()
                    .iter()
                    .map(|reference| reference.target()),
            );
            refs
        }
    }
}

fn event_related_refs(event: &Event) -> Vec<ObjectRef> {
    let mut refs = event
        .participants()
        .iter()
        .map(|participant| ObjectRef::Entity(participant.entity_id()))
        .collect::<Vec<_>>();
    refs.extend(
        event
            .affected_goal_ids()
            .iter()
            .copied()
            .map(ObjectRef::Goal),
    );
    if let Some(location_id) = event.location_entity_id() {
        refs.push(ObjectRef::Entity(location_id));
    }
    refs
}

fn claim_related_refs(claim: &Claim) -> Vec<ObjectRef> {
    let mut refs = vec![ObjectRef::Entity(claim.subject_entity_id())];
    if let Some(holder_id) = claim.holder_entity_id() {
        refs.push(ObjectRef::Entity(holder_id));
    }
    if let Some(ClaimObject::Entity(entity_id)) = claim.object() {
        refs.push(ObjectRef::Entity(*entity_id));
    }
    if let Some(document_id) = claim.source_document_id() {
        refs.push(ObjectRef::Document(document_id));
    }
    if let Some(claim_id) = claim.source_claim_id() {
        refs.push(ObjectRef::Claim(claim_id));
    }
    refs
}

fn is_related_to_context(object: &ResolvedObject, context_refs: &BTreeSet<ObjectRef>) -> bool {
    let object_ref = object.object_ref();
    context_refs.contains(&object_ref)
        || related_refs(object)
            .into_iter()
            .any(|reference| context_refs.contains(&reference))
}

fn temporal_bounds_for_object(object: &ResolvedObject) -> Option<(Option<i64>, Option<i64>)> {
    match object {
        ResolvedObject::World(_) => None,
        ResolvedObject::Relation(relation) => {
            Some((relation.valid_from_tick(), relation.valid_to_tick()))
        }
        ResolvedObject::Event(aggregate) => temporal_bounds_for_event(aggregate.event().time()),
        ResolvedObject::Claim(claim) => claim
            .period()
            .map(|period| (period.start_tick(), period.end_tick())),
        ResolvedObject::Goal(goal) => goal
            .period()
            .map(|period| (period.start_tick(), period.end_tick())),
        ResolvedObject::Entity(_) | ResolvedObject::Rule(_) | ResolvedObject::Document(_) => None,
    }
}

fn temporal_bounds_for_event(time: &EventTime) -> Option<(Option<i64>, Option<i64>)> {
    match time.kind() {
        EventTimeKind::Unknown => None,
        EventTimeKind::Instant => Some((time.start_tick(), time.start_tick())),
        EventTimeKind::Interval | EventTimeKind::Ongoing => {
            Some((time.start_tick(), time.end_tick()))
        }
    }
}
