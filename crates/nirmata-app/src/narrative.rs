use crate::{AppError, NirmataApp, app::ActiveWorld};
use nirmata_core::{
    EventId,
    claim::ClaimAuthentication,
    document::{ContentReference, ObjectRef},
    event::{EventLink, EventLinkKind},
    goal::GoalStatus,
    time::{EventTime, EventTimeKind},
};
use nirmata_store::{CanonSnapshot, ReadScope};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MAX_CAUSAL_DEPTH: u8 = 3;
pub const MAX_CAUSAL_RESULTS: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeObjectReference {
    pub object_ref: ObjectRef,
    pub uri: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeTimelineEvent {
    pub event: NarrativeObjectReference,
    pub summary: String,
    pub time: EventTime,
    pub evidence_uris: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeDiscourseEvent {
    pub event: NarrativeObjectReference,
    pub ordinal: u32,
    pub evidence_uris: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeDiscourseSequence {
    pub source: NarrativeObjectReference,
    pub events: Vec<NarrativeDiscourseEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeTimeline {
    pub scope: ReadScope,
    pub story_time: Vec<NarrativeTimelineEvent>,
    pub unknown_story_time: Vec<NarrativeTimelineEvent>,
    pub discourse_order: Vec<NarrativeDiscourseSequence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeCausalLink {
    pub depth: u8,
    pub kind: EventLinkKind,
    pub source: NarrativeObjectReference,
    pub target: NarrativeObjectReference,
    pub evidence_uris: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeCausalThread {
    pub start: NarrativeObjectReference,
    pub links: Vec<NarrativeCausalLink>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeCausalThreads {
    pub scope: ReadScope,
    pub max_depth: u8,
    pub limit: usize,
    pub threads: Vec<NarrativeCausalThread>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeLooseEnd {
    pub code: &'static str,
    pub message: String,
    pub object_refs: Vec<ObjectRef>,
    pub evidence_uris: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrativeLooseEnds {
    pub scope: ReadScope,
    pub findings: Vec<NarrativeLooseEnd>,
}

impl NirmataApp {
    pub fn derive_narrative_timeline(
        &self,
        scope: Option<ReadScope>,
    ) -> Result<NarrativeTimeline, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let (scope, snapshot) = scoped_snapshot(active, scope)?;
        let mut story_time = Vec::new();
        let mut unknown_story_time = Vec::new();

        for aggregate in snapshot.events() {
            let event = aggregate.event();
            let entry = NarrativeTimelineEvent {
                event: object_reference(ObjectRef::Event(event.id())),
                summary: event.summary().to_owned(),
                time: *event.time(),
                evidence_uris: evidence_for_event(&snapshot, event.id()),
            };
            if event.time().start_tick().is_some() {
                story_time.push(entry);
            } else {
                unknown_story_time.push(entry);
            }
        }

        story_time.sort_by(|left, right| timeline_key(left).cmp(&timeline_key(right)));
        unknown_story_time.sort_by(|left, right| left.event.uri.cmp(&right.event.uri));

        let mut references_by_source: BTreeMap<ObjectRef, Vec<&ContentReference>> = BTreeMap::new();
        for reference in snapshot.content_references() {
            if matches!(reference.target(), ObjectRef::Event(_)) {
                references_by_source
                    .entry(reference.source())
                    .or_default()
                    .push(reference);
            }
        }
        let discourse_order = references_by_source
            .into_iter()
            .map(|(source, mut references)| {
                references.sort_by_key(|reference| (reference.ordinal(), reference.target()));
                NarrativeDiscourseSequence {
                    source: object_reference(source),
                    events: references
                        .into_iter()
                        .map(|reference| NarrativeDiscourseEvent {
                            event: object_reference(reference.target()),
                            ordinal: reference.ordinal(),
                            evidence_uris: sorted_uris([reference.source(), reference.target()]),
                        })
                        .collect(),
                }
            })
            .collect();

        Ok(NarrativeTimeline {
            scope,
            story_time,
            unknown_story_time,
            discourse_order,
        })
    }

    pub fn derive_causal_threads(
        &self,
        scope: Option<ReadScope>,
        start_event_ids: Option<Vec<EventId>>,
        max_depth: u8,
        limit: usize,
    ) -> Result<NarrativeCausalThreads, AppError> {
        validate_causal_bounds(max_depth, limit, start_event_ids.as_deref())?;
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let (scope, snapshot) = scoped_snapshot(active, scope)?;
        let event_ids = snapshot
            .events()
            .iter()
            .map(|aggregate| aggregate.event().id())
            .collect::<BTreeSet<_>>();
        let mut links_by_source: BTreeMap<EventId, Vec<EventLink>> = BTreeMap::new();
        for link in snapshot
            .events()
            .iter()
            .flat_map(|aggregate| aggregate.links())
        {
            links_by_source
                .entry(link.source_event_id())
                .or_default()
                .push(link.clone());
        }
        for links in links_by_source.values_mut() {
            links.sort_by_key(|link| (link.target_event_id(), link_kind_order(link.kind())));
        }

        let starts = match start_event_ids {
            Some(mut starts) => {
                starts.sort();
                starts.dedup();
                for start in &starts {
                    if !event_ids.contains(start) {
                        return Err(AppError::ObjectNotFound {
                            object: "event",
                            id: start.to_string(),
                        });
                    }
                }
                starts
            }
            None => default_thread_starts(&links_by_source),
        };

        let mut threads = Vec::new();
        let mut remaining = limit;
        for start in starts {
            if remaining == 0 {
                break;
            }
            let thread = derive_thread(&snapshot, start, max_depth, remaining, &links_by_source);
            remaining -= thread.links.len();
            threads.push(thread);
        }

        Ok(NarrativeCausalThreads {
            scope,
            max_depth,
            limit,
            threads,
        })
    }

    pub fn derive_loose_ends(
        &self,
        scope: Option<ReadScope>,
    ) -> Result<NarrativeLooseEnds, AppError> {
        let active = self.active.as_ref().ok_or(AppError::NoWorldOpen)?;
        let (scope, snapshot) = scoped_snapshot(active, scope)?;
        let mut findings = Vec::new();

        for goal in snapshot
            .goals()
            .iter()
            .filter(|goal| goal.status() == GoalStatus::Active)
        {
            let mut objects = vec![
                ObjectRef::Goal(goal.id()),
                ObjectRef::Entity(goal.holder_entity_id()),
            ];
            if let Some(source) = goal.source().and_then(parse_object_uri) {
                objects.push(source);
            }
            findings.push(loose_end(
                "active_goal_without_resolution",
                format!(
                    "Goal {} is explicitly active in this scope; no resolution is asserted.",
                    goal.id()
                ),
                objects,
            ));
        }

        for aggregate in snapshot
            .events()
            .iter()
            .filter(|aggregate| aggregate.event().time().kind() == EventTimeKind::Ongoing)
        {
            let event = aggregate.event();
            findings.push(NarrativeLooseEnd {
                code: "ongoing_event",
                message: format!(
                    "Event {} is explicitly ongoing and has no asserted end tick.",
                    event.id()
                ),
                object_refs: vec![ObjectRef::Event(event.id())],
                evidence_uris: evidence_for_event(&snapshot, event.id()),
            });
        }

        for claim in snapshot.claims().iter().filter(|claim| {
            claim.authentication() == ClaimAuthentication::Disputed && claim.is_active()
        }) {
            let mut objects = vec![
                ObjectRef::Claim(claim.id()),
                ObjectRef::Entity(claim.subject_entity_id()),
            ];
            objects.extend(claim.holder_entity_id().map(ObjectRef::Entity));
            objects.extend(claim.source_document_id().map(ObjectRef::Document));
            objects.extend(claim.source_claim_id().map(ObjectRef::Claim));
            if let Some(source) = claim.source().and_then(parse_object_uri) {
                objects.push(source);
            }
            findings.push(loose_end(
                "disputed_claim",
                format!(
                    "Claim {} is explicitly disputed and is not superseded in this scope.",
                    claim.id()
                ),
                objects,
            ));
        }

        findings.sort_by(|left, right| {
            (left.code, left.evidence_uris.first()).cmp(&(right.code, right.evidence_uris.first()))
        });
        Ok(NarrativeLooseEnds { scope, findings })
    }
}

fn scoped_snapshot(
    active: &ActiveWorld,
    requested: Option<ReadScope>,
) -> Result<(ReadScope, CanonSnapshot), AppError> {
    let requested = requested.unwrap_or(active.read_scope);
    let revision = active.store.resolve_scope(requested)?;
    let snapshot = active.store.read_canon_snapshot_scoped(requested)?;
    Ok((
        ReadScope::historical(requested.variant_id, revision),
        snapshot,
    ))
}

fn object_reference(object_ref: ObjectRef) -> NarrativeObjectReference {
    NarrativeObjectReference {
        object_ref,
        uri: object_ref.to_string(),
    }
}

fn timeline_key(entry: &NarrativeTimelineEvent) -> (i64, i64, &str) {
    let start = entry
        .time
        .start_tick()
        .expect("story time has a start tick");
    (
        start,
        entry.time.end_tick().unwrap_or(start),
        &entry.event.uri,
    )
}

fn evidence_for_event(snapshot: &CanonSnapshot, event_id: EventId) -> Vec<String> {
    let target = ObjectRef::Event(event_id);
    let mut evidence = BTreeSet::from([target.to_string()]);
    evidence.extend(
        snapshot
            .content_references()
            .iter()
            .filter(|reference| reference.target() == target)
            .map(|reference| reference.source().to_string()),
    );
    evidence.into_iter().collect()
}

fn validate_causal_bounds(
    max_depth: u8,
    limit: usize,
    starts: Option<&[EventId]>,
) -> Result<(), AppError> {
    if max_depth > MAX_CAUSAL_DEPTH {
        return Err(AppError::InvalidNarrativeQuery(format!(
            "causal max_depth must be at most {MAX_CAUSAL_DEPTH}"
        )));
    }
    if limit > MAX_CAUSAL_RESULTS {
        return Err(AppError::InvalidNarrativeQuery(format!(
            "causal limit must be at most {MAX_CAUSAL_RESULTS}"
        )));
    }
    if starts.is_some_and(|starts| starts.len() > MAX_CAUSAL_RESULTS) {
        return Err(AppError::InvalidNarrativeQuery(format!(
            "causal start event count must be at most {MAX_CAUSAL_RESULTS}"
        )));
    }
    Ok(())
}

fn default_thread_starts(links_by_source: &BTreeMap<EventId, Vec<EventLink>>) -> Vec<EventId> {
    let targets = links_by_source
        .values()
        .flatten()
        .map(EventLink::target_event_id)
        .collect::<BTreeSet<_>>();
    let mut starts = links_by_source
        .keys()
        .filter(|source| !targets.contains(source))
        .copied()
        .collect::<Vec<_>>();
    let mut covered = reachable_from(&starts, links_by_source);
    for source in links_by_source.keys() {
        if covered.contains(source) {
            continue;
        }
        starts.push(*source);
        covered.extend(reachable_from(&[*source], links_by_source));
    }
    starts
}

fn reachable_from(
    starts: &[EventId],
    links_by_source: &BTreeMap<EventId, Vec<EventLink>>,
) -> BTreeSet<EventId> {
    let mut reachable = starts.iter().copied().collect::<BTreeSet<_>>();
    let mut pending = starts.iter().copied().collect::<Vec<_>>();
    while let Some(source) = pending.pop() {
        for target in links_by_source
            .get(&source)
            .into_iter()
            .flatten()
            .map(EventLink::target_event_id)
        {
            if reachable.insert(target) {
                pending.push(target);
            }
        }
    }
    reachable
}

fn derive_thread(
    snapshot: &CanonSnapshot,
    start: EventId,
    max_depth: u8,
    limit: usize,
    links_by_source: &BTreeMap<EventId, Vec<EventLink>>,
) -> NarrativeCausalThread {
    let mut pending = VecDeque::from([(start, 0, BTreeSet::from([start]))]);
    let mut emitted = BTreeSet::new();
    let mut emitted_graph: BTreeMap<EventId, BTreeSet<EventId>> = BTreeMap::new();
    let mut links = Vec::new();

    'traversal: while let Some((source, depth, path)) = pending.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for link in links_by_source.get(&source).into_iter().flatten() {
            let target = link.target_event_id();
            if path.contains(&target) || graph_reaches(target, source, &emitted_graph) {
                continue;
            }
            let key = (link.source_event_id(), target, link_kind_order(link.kind()));
            if !emitted.insert(key) {
                continue;
            }
            if links.len() == limit {
                break 'traversal;
            }
            let next_depth = depth + 1;
            links.push(NarrativeCausalLink {
                depth: next_depth,
                kind: link.kind(),
                source: object_reference(ObjectRef::Event(link.source_event_id())),
                target: object_reference(ObjectRef::Event(target)),
                evidence_uris: causal_evidence(snapshot, link),
            });
            emitted_graph.entry(source).or_default().insert(target);
            let mut next_path = path.clone();
            next_path.insert(target);
            pending.push_back((target, next_depth, next_path));
        }
    }

    NarrativeCausalThread {
        start: object_reference(ObjectRef::Event(start)),
        links,
    }
}

fn graph_reaches(
    start: EventId,
    target: EventId,
    graph: &BTreeMap<EventId, BTreeSet<EventId>>,
) -> bool {
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(event) = pending.pop() {
        if event == target {
            return true;
        }
        if !visited.insert(event) {
            continue;
        }
        pending.extend(graph.get(&event).into_iter().flatten().copied());
    }
    false
}

fn causal_evidence(snapshot: &CanonSnapshot, link: &EventLink) -> Vec<String> {
    let source = ObjectRef::Event(link.source_event_id());
    let target = ObjectRef::Event(link.target_event_id());
    let mut evidence = BTreeSet::from([source.to_string(), target.to_string()]);
    evidence.extend(
        snapshot
            .content_references()
            .iter()
            .filter(|reference| reference.target() == source || reference.target() == target)
            .map(|reference| reference.source().to_string()),
    );
    evidence.into_iter().collect()
}

fn loose_end(
    code: &'static str,
    message: String,
    mut object_refs: Vec<ObjectRef>,
) -> NarrativeLooseEnd {
    object_refs.sort();
    object_refs.dedup();
    let evidence_uris = object_refs.iter().map(ToString::to_string).collect();
    NarrativeLooseEnd {
        code,
        message,
        object_refs,
        evidence_uris,
    }
}

fn parse_object_uri(value: &str) -> Option<ObjectRef> {
    value.parse().ok()
}

fn sorted_uris(objects: impl IntoIterator<Item = ObjectRef>) -> Vec<String> {
    objects
        .into_iter()
        .map(|object| object.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

const fn link_kind_order(kind: EventLinkKind) -> u8 {
    match kind {
        EventLinkKind::Enables => 0,
        EventLinkKind::Causes => 1,
        EventLinkKind::Motivates => 2,
        EventLinkKind::Prevents => 3,
        EventLinkKind::Terminates => 4,
        EventLinkKind::Reveals => 5,
    }
}
