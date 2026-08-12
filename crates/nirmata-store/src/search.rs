use crate::{
    DocumentAggregate, EventAggregate, StoreError, WorldStore, invalid_data, invalid_value,
    map_database_error, map_schema_error,
};
use nirmata_core::{
    ClaimId, DocumentId, EntityId, EventId, GoalId, Period, RelationId, RuleId, World, WorldId,
    claim::Claim,
    document::{Document, ObjectRef},
    entity::{Entity, EntityKind},
    event::Event,
    goal::Goal,
    relation::Relation,
    rule::Rule,
};
use rusqlite::{Connection, Row, params};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    str::FromStr,
    sync::OnceLock,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StructuredSearchKind {
    Entity,
    Relation,
    Event,
    Claim,
    Rule,
    Goal,
    Document,
}

impl StructuredSearchKind {
    fn matches(self, object: ObjectRef) -> bool {
        matches!(
            (self, object),
            (Self::Entity, ObjectRef::Entity(_))
                | (Self::Relation, ObjectRef::Relation(_))
                | (Self::Event, ObjectRef::Event(_))
                | (Self::Claim, ObjectRef::Claim(_))
                | (Self::Rule, ObjectRef::Rule(_))
                | (Self::Goal, ObjectRef::Goal(_))
                | (Self::Document, ObjectRef::Document(_))
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredSearchTemporal {
    Tick(i64),
    Period(Period),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredSearchStage {
    Type,
    Alias,
    Neighbor,
    Goal,
    Perspective,
    Temporal,
    Text,
    Semantic,
}

pub const SEMANTIC_MODEL_ID: &str = "wordnet-en-offline";
pub const SEMANTIC_MODEL_VERSION: u32 = 1;
const MAX_SEMANTIC_RESULTS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredSearchQuery {
    pub kinds: Vec<StructuredSearchKind>,
    pub alias: Option<String>,
    pub neighbors_of: Vec<ObjectRef>,
    pub goal_ids: Vec<GoalId>,
    pub perspective_entity_ids: Vec<EntityId>,
    pub temporal: Option<StructuredSearchTemporal>,
    pub text: Option<String>,
    pub limit: usize,
}

impl Default for StructuredSearchQuery {
    fn default() -> Self {
        Self {
            kinds: vec![],
            alias: None,
            neighbors_of: vec![],
            goal_ids: vec![],
            perspective_entity_ids: vec![],
            temporal: None,
            text: None,
            limit: 25,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredSearchHit {
    pub object: ObjectRef,
    pub fragment: String,
    pub provenance: String,
    pub stage: StructuredSearchStage,
}

impl StructuredSearchHit {
    pub fn score(&self) -> u32 {
        match self.stage {
            StructuredSearchStage::Alias => 70_000,
            StructuredSearchStage::Neighbor => 65_000,
            StructuredSearchStage::Goal => 60_000,
            StructuredSearchStage::Perspective => 55_000,
            StructuredSearchStage::Temporal => 50_000,
            StructuredSearchStage::Type => 45_000,
            StructuredSearchStage::Text => 30_000,
            StructuredSearchStage::Semantic => 10_000 + semantic_score(&self.provenance),
        }
    }

    pub fn score_explanation(&self) -> String {
        match self.stage {
            StructuredSearchStage::Semantic => format!(
                "semantic model {SEMANTIC_MODEL_ID} v{SEMANTIC_MODEL_VERSION}; {} basis points of query concepts matched",
                semantic_score(&self.provenance)
            ),
            stage => format!(
                "fixed deterministic priority for {} retrieval",
                stage_name(stage)
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedObject {
    World(World),
    Entity(Entity),
    Relation(Relation),
    Event(EventAggregate),
    Claim(Claim),
    Rule(Rule),
    Goal(Goal),
    Document(DocumentAggregate),
}

impl ResolvedObject {
    pub fn object_ref(&self) -> ObjectRef {
        match self {
            Self::World(world) => ObjectRef::World(world.id()),
            Self::Entity(entity) => ObjectRef::Entity(entity.id()),
            Self::Relation(relation) => ObjectRef::Relation(relation.id()),
            Self::Event(aggregate) => ObjectRef::Event(aggregate.event().id()),
            Self::Claim(claim) => ObjectRef::Claim(claim.id()),
            Self::Rule(rule) => ObjectRef::Rule(rule.id()),
            Self::Goal(goal) => ObjectRef::Goal(goal.id()),
            Self::Document(aggregate) => ObjectRef::Document(aggregate.object().id()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnchorContextQuery {
    pub anchors: Vec<ObjectRef>,
    pub relation_limit: usize,
}

impl Default for AnchorContextQuery {
    fn default() -> Self {
        Self {
            anchors: vec![],
            relation_limit: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnchorContextEntry {
    pub object: ResolvedObject,
    pub provenance: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnchorContextBundle {
    pub anchors: Vec<AnchorContextEntry>,
    pub relations: Vec<AnchorContextEntry>,
    pub events: Vec<AnchorContextEntry>,
    pub participants: Vec<AnchorContextEntry>,
    pub claims: Vec<AnchorContextEntry>,
    pub goals: Vec<AnchorContextEntry>,
    pub rules: Vec<AnchorContextEntry>,
}

impl AnchorContextBundle {
    pub fn ordered_entries(&self) -> Vec<&AnchorContextEntry> {
        self.anchors
            .iter()
            .chain(self.relations.iter())
            .chain(self.events.iter())
            .chain(self.participants.iter())
            .chain(self.claims.iter())
            .chain(self.goals.iter())
            .chain(self.rules.iter())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalVfsDirectory {
    pub name: String,
    pub children: Vec<LogicalVfsNode>,
}

impl LogicalVfsDirectory {
    pub fn child_directory(&self, name: &str) -> Option<&Self> {
        self.children.iter().find_map(|child| match child {
            LogicalVfsNode::Directory(directory) if directory.name == name => Some(directory),
            _ => None,
        })
    }

    pub fn child_object(&self, name: &str) -> Option<&LogicalVfsObject> {
        self.children.iter().find_map(|child| match child {
            LogicalVfsNode::Object(object) if object.name == name => Some(object),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalVfsObject {
    pub name: String,
    pub object: ObjectRef,
    pub uri: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogicalVfsNode {
    Directory(LogicalVfsDirectory),
    Object(LogicalVfsObject),
}

pub(crate) const TEXT_SEARCH_SCHEMA: &str = "
    CREATE VIRTUAL TABLE canon_fts USING fts5(
        world_id UNINDEXED,
        object_type UNINDEXED,
        object_id UNINDEXED,
        name_title,
        summary,
        markdown
    );
";

impl WorldStore {
    pub fn search_canon_text(&self, query: &str) -> Result<Vec<ObjectRef>, StoreError> {
        let match_query = build_match_query(query);
        if match_query.is_empty() {
            return Ok(vec![]);
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT object_type, object_id
                 FROM canon_fts
                 WHERE world_id = ?1 AND canon_fts MATCH ?2
                 ORDER BY object_type, object_id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), match_query],
                object_ref_from_row,
            )
            .map_err(|error| map_database_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn rebuild_canon_text_index(&mut self) -> Result<(), StoreError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        rebuild_canon_text_index(&transaction, &self.path, self.world_id)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))
    }

    pub fn search_structured(
        &self,
        query: &StructuredSearchQuery,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        self.search_structured_inner(query, true)
    }

    pub fn search_structured_fts(
        &self,
        query: &StructuredSearchQuery,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        self.search_structured_inner(query, false)
    }

    fn search_structured_inner(
        &self,
        query: &StructuredSearchQuery,
        include_semantic: bool,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        if query.limit == 0 {
            return Ok(vec![]);
        }

        let mut stages = Vec::new();

        if !query.kinds.is_empty() {
            stages.push(self.search_by_type(&query.kinds)?);
        }
        if let Some(alias) = normalize_filter(query.alias.as_deref()) {
            stages.push(self.search_by_alias(&alias)?);
        }
        if !query.neighbors_of.is_empty() {
            stages.push(self.search_neighbors(&query.neighbors_of)?);
        }
        if !query.goal_ids.is_empty() {
            stages.push(self.search_by_goals(&query.goal_ids)?);
        }
        if !query.perspective_entity_ids.is_empty() {
            stages.push(self.search_by_perspectives(&query.perspective_entity_ids)?);
        }
        if let Some(temporal) = query.temporal {
            stages.push(self.search_by_temporal(temporal)?);
        }
        if let Some(text) = normalize_filter(query.text.as_deref()) {
            let mut lexical_hits = BTreeMap::new();
            for hit in self.search_by_text(&text)? {
                lexical_hits.insert(hit.object, hit);
            }
            if include_semantic {
                // Semantic retrieval is derived evidence. Its failure cannot replace or
                // invalidate deterministic SQL/FTS results.
                if let Ok(hits) =
                    self.search_semantic(&text, &query.kinds, query.limit.min(MAX_SEMANTIC_RESULTS))
                {
                    for hit in hits {
                        lexical_hits.entry(hit.object).or_insert(hit);
                    }
                }
            }
            if lexical_hits.is_empty() {
                return Ok(vec![]);
            }
            stages.push(lexical_hits.into_values().collect());
        }

        if stages.is_empty() {
            return Ok(vec![]);
        }

        let required = stages.len();
        let mut counts = BTreeMap::<ObjectRef, usize>::new();
        let mut selected = BTreeMap::<ObjectRef, StructuredSearchHit>::new();
        for stage_hits in stages {
            let mut unique_hits = BTreeMap::<ObjectRef, StructuredSearchHit>::new();
            for hit in stage_hits {
                if !matches_kind_filter(&query.kinds, hit.object) {
                    continue;
                }
                unique_hits.entry(hit.object).or_insert(hit);
            }
            if unique_hits.is_empty() {
                return Ok(vec![]);
            }

            for (object, hit) in unique_hits {
                *counts.entry(object).or_insert(0) += 1;
                match selected.get(&object) {
                    Some(existing)
                        if stage_priority(existing.stage) <= stage_priority(hit.stage) => {}
                    _ => {
                        selected.insert(object, hit);
                    }
                }
            }
        }

        let mut results = counts
            .into_iter()
            .filter_map(|(object, matched)| {
                (matched == required)
                    .then(|| selected.remove(&object))
                    .flatten()
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            (
                stage_priority(left.stage),
                std::cmp::Reverse(left.score()),
                left.object.kind(),
                object_id(left.object),
            )
                .cmp(&(
                    stage_priority(right.stage),
                    std::cmp::Reverse(right.score()),
                    right.object.kind(),
                    object_id(right.object),
                ))
        });
        results.truncate(query.limit);
        Ok(results)
    }

    pub fn search_semantic(
        &self,
        text: &str,
        kinds: &[StructuredSearchKind],
        limit: usize,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        #[cfg(test)]
        if self.fail_semantic_search {
            return Err(StoreError::Database(
                self.path.clone(),
                "simulated semantic derived failure".to_owned(),
            ));
        }
        if limit == 0 {
            return Ok(vec![]);
        }
        let query_tokens = semantic_tokens(text);
        if query_tokens.is_empty() {
            return Ok(vec![]);
        }

        let mut query_features = BTreeMap::new();
        for token in &query_tokens {
            let synonyms = wordnet()
                .get(token)
                .into_iter()
                .flatten()
                .flat_map(|synonym| semantic_tokens(&synonym))
                .collect::<BTreeSet<_>>();
            query_features.insert(token.clone(), synonyms);
        }

        let mut scored = self
            .semantic_canon_texts()?
            .into_iter()
            .filter(|(object, _)| matches_kind_filter(kinds, *object))
            .filter_map(|(object, text)| {
                semantic_chunks(&text)
                    .into_iter()
                    .filter_map(|chunk| {
                        let chunk_tokens = semantic_tokens(&chunk);
                        let matched = query_features
                            .iter()
                            .filter(|(token, synonyms)| {
                                chunk_tokens.contains(*token)
                                    || chunk_tokens.iter().any(|word| synonyms.contains(word))
                            })
                            .count();
                        let score_bps = matched * 10_000 / query_tokens.len();
                        // Broad WordNet senses need support from at least half the query concepts.
                        (matched > 0 && score_bps >= 5_000).then_some((score_bps, chunk))
                    })
                    .max_by(|(left_score, left), (right_score, right)| {
                        left_score.cmp(right_score).then_with(|| right.cmp(left))
                    })
                    .map(|(score_bps, chunk)| {
                        (
                            score_bps,
                            StructuredSearchHit {
                                object,
                                fragment: preview(&[chunk]),
                                provenance: format!(
                                    "semantic:{SEMANTIC_MODEL_ID}:v{SEMANTIC_MODEL_VERSION}:matched_bps={score_bps}"
                                ),
                                stage: StructuredSearchStage::Semantic,
                            },
                        )
                    })
            })
            .collect::<Vec<_>>();
        scored.sort_by(|(left_score, left), (right_score, right)| {
            right_score.cmp(left_score).then_with(|| {
                (left.object.kind(), object_id(left.object))
                    .cmp(&(right.object.kind(), object_id(right.object)))
            })
        });
        scored.truncate(limit);
        Ok(scored.into_iter().map(|(_, hit)| hit).collect())
    }

    fn semantic_canon_texts(&self) -> Result<Vec<(ObjectRef, String)>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT object_type, object_id, content
                 FROM (
                     SELECT 'rule' AS object_type, id AS object_id,
                            statement_md || ' ' || ifnull(source, '') AS content
                     FROM rules WHERE world_id = ?1
                     UNION ALL
                     SELECT 'entity', id, name || ' ' || summary || ' ' || body_md
                     FROM entities WHERE world_id = ?1
                     UNION ALL
                     SELECT 'relation', id, kind || ' ' || ifnull(source_reference, '')
                     FROM relations WHERE world_id = ?1
                     UNION ALL
                     SELECT 'event', id, kind || ' ' || summary || ' ' || body_md
                     FROM events WHERE world_id = ?1
                     UNION ALL
                     SELECT 'claim', id, content_md || ' ' || ifnull(source, '')
                     FROM claims WHERE world_id = ?1
                     UNION ALL
                     SELECT 'goal', id, desired_state_md || ' ' || ifnull(source, '')
                     FROM goals WHERE world_id = ?1
                     UNION ALL
                     SELECT 'document', id, title || ' ' || body_md
                     FROM documents WHERE world_id = ?1
                 )
                 ORDER BY object_type, object_id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([self.world_id.to_string()], |row| {
                Ok((object_ref_from_row(row)?, row.get::<_, String>(2)?))
            })
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn resolve_uri(&self, uri: &str) -> Result<ResolvedObject, StoreError> {
        let object =
            ObjectRef::from_str(uri).map_err(|_| StoreError::InvalidObjectUri(uri.to_owned()))?;
        self.resolve_object_ref(object)
    }

    pub fn resolve_object_ref(&self, object: ObjectRef) -> Result<ResolvedObject, StoreError> {
        match object {
            ObjectRef::World(id) => {
                let world = self.load_world()?;
                if world.id() == id {
                    Ok(ResolvedObject::World(world))
                } else {
                    Err(StoreError::ObjectNotFound {
                        object: object.kind(),
                        id: object_id(object),
                    })
                }
            }
            ObjectRef::Entity(id) => {
                self.get_entity(id)?
                    .map(ResolvedObject::Entity)
                    .ok_or(StoreError::ObjectNotFound {
                        object: object.kind(),
                        id: object_id(object),
                    })
            }
            ObjectRef::Relation(id) => self.get_relation(id)?.map(ResolvedObject::Relation).ok_or(
                StoreError::ObjectNotFound {
                    object: object.kind(),
                    id: object_id(object),
                },
            ),
            ObjectRef::Event(id) => {
                self.get_event(id)?
                    .map(ResolvedObject::Event)
                    .ok_or(StoreError::ObjectNotFound {
                        object: object.kind(),
                        id: object_id(object),
                    })
            }
            ObjectRef::Claim(id) => {
                self.get_claim(id)?
                    .map(ResolvedObject::Claim)
                    .ok_or(StoreError::ObjectNotFound {
                        object: object.kind(),
                        id: object_id(object),
                    })
            }
            ObjectRef::Rule(id) => {
                self.get_rule(id)?
                    .map(ResolvedObject::Rule)
                    .ok_or(StoreError::ObjectNotFound {
                        object: object.kind(),
                        id: object_id(object),
                    })
            }
            ObjectRef::Goal(id) => {
                self.get_goal(id)?
                    .map(ResolvedObject::Goal)
                    .ok_or(StoreError::ObjectNotFound {
                        object: object.kind(),
                        id: object_id(object),
                    })
            }
            ObjectRef::Document(id) => self.get_document(id)?.map(ResolvedObject::Document).ok_or(
                StoreError::ObjectNotFound {
                    object: object.kind(),
                    id: object_id(object),
                },
            ),
        }
    }

    pub fn load_anchor_context(
        &self,
        query: &AnchorContextQuery,
    ) -> Result<AnchorContextBundle, StoreError> {
        if query.anchors.is_empty() {
            return Ok(AnchorContextBundle::default());
        }

        let mut bundle = AnchorContextBundle::default();
        let mut anchor_entities = Vec::new();
        let mut anchor_goals = Vec::new();
        let mut anchor_events = Vec::new();

        for anchor in dedup_refs_preserving_order(&query.anchors) {
            let resolved = self.resolve_object_ref(anchor)?;
            match &resolved {
                ResolvedObject::Entity(entity) => anchor_entities.push(entity.id()),
                ResolvedObject::Goal(goal) => anchor_goals.push(goal.id()),
                ResolvedObject::Event(event) => anchor_events.push(event.event().id()),
                _ => {}
            }
            bundle.anchors.push(AnchorContextEntry {
                object: resolved,
                provenance: format!("anchor:{anchor}"),
            });
        }

        let anchor_ref_set = bundle
            .anchors
            .iter()
            .map(|entry| entry.object.object_ref())
            .collect::<BTreeSet<_>>();

        let mut seen_relations = BTreeSet::new();
        if query.relation_limit > 0 {
            for entity_id in &anchor_entities {
                for relation_ref in self.direct_relation_refs(*entity_id, query.relation_limit)? {
                    if anchor_ref_set.contains(&relation_ref)
                        || !seen_relations.insert(relation_ref)
                    {
                        continue;
                    }
                    bundle.relations.push(AnchorContextEntry {
                        object: self.resolve_object_ref(relation_ref)?,
                        provenance: format!("relation:{}", ObjectRef::Entity(*entity_id)),
                    });
                }
            }
        }

        let mut seen_events = BTreeSet::new();
        for event_id in &anchor_events {
            seen_events.insert(ObjectRef::Event(*event_id));
        }
        for entity_id in &anchor_entities {
            for event_ref in self.associated_event_refs_for_entity(*entity_id)? {
                if anchor_ref_set.contains(&event_ref) || !seen_events.insert(event_ref) {
                    continue;
                }
                bundle.events.push(AnchorContextEntry {
                    object: self.resolve_object_ref(event_ref)?,
                    provenance: format!("event:{}", ObjectRef::Entity(*entity_id)),
                });
            }
        }
        for goal_id in &anchor_goals {
            for event_ref in self.associated_event_refs_for_goal(*goal_id)? {
                if anchor_ref_set.contains(&event_ref) || !seen_events.insert(event_ref) {
                    continue;
                }
                bundle.events.push(AnchorContextEntry {
                    object: self.resolve_object_ref(event_ref)?,
                    provenance: format!("event:{}", ObjectRef::Goal(*goal_id)),
                });
            }
        }

        let event_ids = bundle
            .anchors
            .iter()
            .chain(bundle.events.iter())
            .filter_map(|entry| match entry.object.object_ref() {
                ObjectRef::Event(id) => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut seen_participants = BTreeSet::new();
        for event_id in event_ids {
            for participant_ref in self.participant_entity_refs_for_event(event_id)? {
                if anchor_ref_set.contains(&participant_ref)
                    || !seen_participants.insert(participant_ref)
                {
                    continue;
                }
                bundle.participants.push(AnchorContextEntry {
                    object: self.resolve_object_ref(participant_ref)?,
                    provenance: format!("participant:{}", ObjectRef::Event(event_id)),
                });
            }
        }

        let context_entity_ids = bundle
            .anchors
            .iter()
            .chain(bundle.participants.iter())
            .filter_map(|entry| match entry.object.object_ref() {
                ObjectRef::Entity(id) => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();
        let anchor_document_ids = bundle
            .anchors
            .iter()
            .filter_map(|entry| match entry.object.object_ref() {
                ObjectRef::Document(id) => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();
        let anchor_claim_ids = bundle
            .anchors
            .iter()
            .filter_map(|entry| match entry.object.object_ref() {
                ObjectRef::Claim(id) => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();

        let mut seen_claims = BTreeSet::new();
        for entity_id in &context_entity_ids {
            for claim_ref in self.claim_refs_for_entity(*entity_id)? {
                if anchor_ref_set.contains(&claim_ref) || !seen_claims.insert(claim_ref) {
                    continue;
                }
                bundle.claims.push(AnchorContextEntry {
                    object: self.resolve_object_ref(claim_ref)?,
                    provenance: format!("claim:{}", ObjectRef::Entity(*entity_id)),
                });
            }
        }
        for document_id in &anchor_document_ids {
            for claim_ref in self.claim_refs_for_document(*document_id)? {
                if anchor_ref_set.contains(&claim_ref) || !seen_claims.insert(claim_ref) {
                    continue;
                }
                bundle.claims.push(AnchorContextEntry {
                    object: self.resolve_object_ref(claim_ref)?,
                    provenance: format!("claim:{}", ObjectRef::Document(*document_id)),
                });
            }
        }
        for claim_id in &anchor_claim_ids {
            for claim_ref in self.claim_refs_for_source_claim(*claim_id)? {
                if anchor_ref_set.contains(&claim_ref) || !seen_claims.insert(claim_ref) {
                    continue;
                }
                bundle.claims.push(AnchorContextEntry {
                    object: self.resolve_object_ref(claim_ref)?,
                    provenance: format!("claim:{}", ObjectRef::Claim(*claim_id)),
                });
            }
        }

        let mut seen_goals = BTreeSet::new();
        for goal_id in &anchor_goals {
            seen_goals.insert(ObjectRef::Goal(*goal_id));
        }
        for entity_id in &context_entity_ids {
            for goal_ref in self.goal_refs_for_holder(*entity_id)? {
                if anchor_ref_set.contains(&goal_ref) || !seen_goals.insert(goal_ref) {
                    continue;
                }
                bundle.goals.push(AnchorContextEntry {
                    object: self.resolve_object_ref(goal_ref)?,
                    provenance: format!("goal:{}", ObjectRef::Entity(*entity_id)),
                });
            }
        }
        for event_entry in bundle.anchors.iter().chain(bundle.events.iter()) {
            if let ObjectRef::Event(event_id) = event_entry.object.object_ref() {
                for goal_ref in self.goal_refs_for_event(event_id)? {
                    if anchor_ref_set.contains(&goal_ref) || !seen_goals.insert(goal_ref) {
                        continue;
                    }
                    bundle.goals.push(AnchorContextEntry {
                        object: self.resolve_object_ref(goal_ref)?,
                        provenance: format!("goal:{}", ObjectRef::Event(event_id)),
                    });
                }
            }
        }

        let context_objects = bundle
            .ordered_entries()
            .into_iter()
            .map(|entry| &entry.object)
            .collect::<Vec<_>>();
        let mut seen_rules = BTreeSet::new();
        for (rule, scope) in self.applicable_rules_for_context(&context_objects)? {
            let rule_ref = ObjectRef::Rule(rule.id());
            if anchor_ref_set.contains(&rule_ref) || !seen_rules.insert(rule_ref) {
                continue;
            }
            bundle.rules.push(AnchorContextEntry {
                object: ResolvedObject::Rule(rule),
                provenance: format!("rule_scope:{scope}"),
            });
        }

        Ok(bundle)
    }

    pub fn read_logical_vfs(&self) -> Result<LogicalVfsDirectory, StoreError> {
        let mut root = LogicalVfsDirectory {
            name: "/".to_owned(),
            children: vec![],
        };

        let entity_groups = self.logical_entities()?;
        if !entity_groups.is_empty() {
            root.children
                .push(LogicalVfsNode::Directory(LogicalVfsDirectory {
                    name: "entities".to_owned(),
                    children: entity_groups,
                }));
        }

        push_directory_if_any(&mut root, "relations", self.logical_relations()?);
        push_directory_if_any(&mut root, "events", self.logical_events()?);
        push_directory_if_any(&mut root, "claims", self.logical_claims()?);
        push_directory_if_any(&mut root, "rules", self.logical_rules()?);
        push_directory_if_any(&mut root, "goals", self.logical_goals()?);

        let document_groups = self.logical_documents()?;
        if !document_groups.is_empty() {
            root.children
                .push(LogicalVfsNode::Directory(LogicalVfsDirectory {
                    name: "documents".to_owned(),
                    children: document_groups,
                }));
        }

        root.children.sort_by(logical_node_name);
        Ok(root)
    }
}

pub(crate) fn create_text_search_schema(
    connection: &Connection,
    path: &Path,
) -> Result<(), StoreError> {
    connection
        .execute_batch(TEXT_SEARCH_SCHEMA)
        .map_err(|error| map_database_error(path, error))
}

pub(crate) fn rebuild_canon_text_index(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
) -> Result<(), StoreError> {
    connection
        .execute(
            "DELETE FROM canon_fts WHERE world_id = ?1",
            [world_id.to_string()],
        )
        .map_err(|error| map_database_error(path, error))?;
    connection
        .execute(
            "INSERT INTO canon_fts (
                world_id, object_type, object_id, name_title, summary, markdown
             )
             SELECT world_id, 'rule', id, '', '', statement_md
             FROM rules
             WHERE world_id = ?1
             UNION ALL
             SELECT world_id, 'entity', id, name, summary, body_md
             FROM entities
             WHERE world_id = ?1
             UNION ALL
             SELECT world_id, 'event', id, '', summary, body_md
             FROM events
             WHERE world_id = ?1
             UNION ALL
             SELECT world_id, 'claim', id, '', '', content_md
             FROM claims
             WHERE world_id = ?1
             UNION ALL
             SELECT world_id, 'document', id, title, '', body_md
             FROM documents
             WHERE world_id = ?1",
            [world_id.to_string()],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(crate) fn index_rule(
    connection: &Connection,
    path: &Path,
    rule: &Rule,
) -> Result<(), StoreError> {
    replace_text_index_row(
        connection,
        path,
        rule.world_id(),
        ObjectRef::Rule(rule.id()),
        "",
        "",
        rule.statement_md(),
    )
}

pub(crate) fn index_entity(
    connection: &Connection,
    path: &Path,
    entity: &Entity,
) -> Result<(), StoreError> {
    replace_text_index_row(
        connection,
        path,
        entity.world_id(),
        ObjectRef::Entity(entity.id()),
        entity.name(),
        entity.summary(),
        entity.body_md(),
    )
}

pub(crate) fn index_event(
    connection: &Connection,
    path: &Path,
    event: &Event,
) -> Result<(), StoreError> {
    replace_text_index_row(
        connection,
        path,
        event.world_id(),
        ObjectRef::Event(event.id()),
        "",
        event.summary(),
        event.body_md(),
    )
}

pub(crate) fn index_claim(
    connection: &Connection,
    path: &Path,
    claim: &Claim,
) -> Result<(), StoreError> {
    replace_text_index_row(
        connection,
        path,
        claim.world_id(),
        ObjectRef::Claim(claim.id()),
        "",
        "",
        claim.content_md(),
    )
}

pub(crate) fn index_document(
    connection: &Connection,
    path: &Path,
    document: &Document,
) -> Result<(), StoreError> {
    replace_text_index_row(
        connection,
        path,
        document.world_id(),
        ObjectRef::Document(document.id()),
        document.title(),
        "",
        document.body_md(),
    )
}

pub(crate) fn remove_text_index_row(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
    object: ObjectRef,
) -> Result<(), StoreError> {
    connection
        .execute(
            "DELETE FROM canon_fts
             WHERE world_id = ?1 AND object_type = ?2 AND object_id = ?3",
            params![world_id.to_string(), object.kind(), object_id(object)],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

fn replace_text_index_row(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
    object: ObjectRef,
    name_title: &str,
    summary: &str,
    markdown: &str,
) -> Result<(), StoreError> {
    remove_text_index_row(connection, path, world_id, object)?;
    connection
        .execute(
            "INSERT INTO canon_fts (
                world_id, object_type, object_id, name_title, summary, markdown
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                world_id.to_string(),
                object.kind(),
                object_id(object),
                name_title,
                summary,
                markdown,
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

fn build_match_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn object_id(object: ObjectRef) -> String {
    match object {
        ObjectRef::World(id) => id.to_string(),
        ObjectRef::Entity(id) => id.to_string(),
        ObjectRef::Relation(id) => id.to_string(),
        ObjectRef::Event(id) => id.to_string(),
        ObjectRef::Claim(id) => id.to_string(),
        ObjectRef::Rule(id) => id.to_string(),
        ObjectRef::Goal(id) => id.to_string(),
        ObjectRef::Document(id) => id.to_string(),
    }
}

fn object_ref_from_row(row: &Row<'_>) -> rusqlite::Result<ObjectRef> {
    parse_object_ref(0, &row.get::<_, String>(0)?, &row.get::<_, String>(1)?)
}

fn parse_object_ref(index: usize, kind: &str, id: &str) -> rusqlite::Result<ObjectRef> {
    match kind {
        "world" => WorldId::from_str(id)
            .map(ObjectRef::World)
            .map_err(|error| invalid_data(index + 1, error)),
        "entity" => EntityId::from_str(id)
            .map(ObjectRef::Entity)
            .map_err(|error| invalid_data(index + 1, error)),
        "relation" => RelationId::from_str(id)
            .map(ObjectRef::Relation)
            .map_err(|error| invalid_data(index + 1, error)),
        "event" => EventId::from_str(id)
            .map(ObjectRef::Event)
            .map_err(|error| invalid_data(index + 1, error)),
        "claim" => ClaimId::from_str(id)
            .map(ObjectRef::Claim)
            .map_err(|error| invalid_data(index + 1, error)),
        "rule" => RuleId::from_str(id)
            .map(ObjectRef::Rule)
            .map_err(|error| invalid_data(index + 1, error)),
        "goal" => GoalId::from_str(id)
            .map(ObjectRef::Goal)
            .map_err(|error| invalid_data(index + 1, error)),
        "document" => DocumentId::from_str(id)
            .map(ObjectRef::Document)
            .map_err(|error| invalid_data(index + 1, error)),
        _ => Err(invalid_value(index, kind)),
    }
}

mod logical_vfs;
mod structured;

fn normalize_filter(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn dedup_refs_preserving_order(values: &[ObjectRef]) -> Vec<ObjectRef> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(*value) {
            deduped.push(*value);
        }
    }
    deduped
}

fn matches_kind_filter(kinds: &[StructuredSearchKind], object: ObjectRef) -> bool {
    kinds.is_empty() || kinds.iter().copied().any(|kind| kind.matches(object))
}

fn preview<T: AsRef<str>>(parts: &[T]) -> String {
    let text = parts
        .iter()
        .map(|part| part.as_ref())
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ");
    let text = if text.len() > 160 {
        format!("{}…", &text[..160].trim_end())
    } else {
        text
    };
    text
}

fn temporal_bounds(filter: StructuredSearchTemporal) -> (Option<i64>, Option<i64>, String) {
    match filter {
        StructuredSearchTemporal::Tick(tick) => (Some(tick), Some(tick), format!("tick:{tick}")),
        StructuredSearchTemporal::Period(period) => (
            period.start_tick(),
            period.end_tick(),
            format!(
                "period:{}..{}",
                period
                    .start_tick()
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "*".to_owned()),
                period
                    .end_tick()
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "*".to_owned())
            ),
        ),
    }
}

fn stage_priority(stage: StructuredSearchStage) -> u8 {
    match stage {
        StructuredSearchStage::Alias => 0,
        StructuredSearchStage::Neighbor => 1,
        StructuredSearchStage::Goal => 2,
        StructuredSearchStage::Perspective => 3,
        StructuredSearchStage::Temporal => 4,
        StructuredSearchStage::Text => 5,
        StructuredSearchStage::Semantic => 6,
        StructuredSearchStage::Type => 7,
    }
}

fn stage_name(stage: StructuredSearchStage) -> &'static str {
    match stage {
        StructuredSearchStage::Type => "structured SQL",
        StructuredSearchStage::Alias => "alias",
        StructuredSearchStage::Neighbor => "relation",
        StructuredSearchStage::Goal => "goal",
        StructuredSearchStage::Perspective => "perspective",
        StructuredSearchStage::Temporal => "time",
        StructuredSearchStage::Text => "FTS5",
        StructuredSearchStage::Semantic => "semantic",
    }
}

fn semantic_score(provenance: &str) -> u32 {
    provenance
        .rsplit_once("matched_bps=")
        .and_then(|(_, score)| score.parse().ok())
        .unwrap_or(0)
}

fn semantic_tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(|word| {
            let word = word.to_ascii_lowercase();
            if word.len() < 3 || is_semantic_stop_word(&word) {
                None
            } else {
                Some(english_lemma(word))
            }
        })
        .collect()
}

fn semantic_chunks(value: &str) -> Vec<String> {
    const MAX_CHARS: usize = 800;
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if !current.is_empty()
            && current.chars().count() + separator + word.chars().count() > MAX_CHARS
        {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn wordnet() -> &'static HashMap<String, Vec<String>> {
    static WORDNET: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    WORDNET.get_or_init(thesaurus::dict)
}

fn english_lemma(mut word: String) -> String {
    if word.len() > 4 && word.ends_with("ies") {
        word.truncate(word.len() - 3);
        word.push('y');
    } else if word.len() > 4
        && ["sses", "xes", "zes", "ches", "shes"]
            .iter()
            .any(|suffix| word.ends_with(suffix))
    {
        word.truncate(word.len() - 2);
    } else if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
        word.pop();
    }
    word
}

fn is_semantic_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "against"
            | "and"
            | "for"
            | "from"
            | "into"
            | "not"
            | "over"
            | "that"
            | "this"
            | "with"
    )
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

fn display_name(value: &str, object: ObjectRef) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        object_id(object)
    } else {
        trimmed.to_owned()
    }
}

fn scope_tokens_for_object(object: &ResolvedObject) -> BTreeSet<String> {
    match object {
        ResolvedObject::World(_) => BTreeSet::from(["world".to_owned()]),
        ResolvedObject::Entity(entity) => BTreeSet::from([
            "entity".to_owned(),
            normalize_scope(entity_group_name(entity.kind())),
            normalize_scope(match entity.kind() {
                EntityKind::Person => "person",
                EntityKind::Place => "place",
                EntityKind::Faction => "faction",
                EntityKind::Culture => "culture",
                EntityKind::Resource => "resource",
                EntityKind::Concept => "concept",
            }),
        ]),
        ResolvedObject::Relation(relation) => {
            BTreeSet::from(["relation".to_owned(), normalize_scope(relation.kind())])
        }
        ResolvedObject::Event(event) => {
            BTreeSet::from(["event".to_owned(), normalize_scope(event.event().kind())])
        }
        ResolvedObject::Claim(claim) => {
            let mut scopes = BTreeSet::from(["claim".to_owned()]);
            if let Some(predicate) = claim.predicate_key() {
                scopes.insert(normalize_scope(predicate));
            }
            scopes
        }
        ResolvedObject::Rule(_) => BTreeSet::from(["rule".to_owned()]),
        ResolvedObject::Goal(_) => BTreeSet::from(["goal".to_owned()]),
        ResolvedObject::Document(document) => BTreeSet::from([
            "document".to_owned(),
            normalize_scope(document.object().kind()),
        ]),
    }
}

fn normalize_scope(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn logical_object_node(name: &str, object: ObjectRef) -> LogicalVfsNode {
    LogicalVfsNode::Object(LogicalVfsObject {
        name: name.to_owned(),
        object,
        uri: object.to_string(),
    })
}

fn directory_groups(groups: BTreeMap<String, Vec<LogicalVfsNode>>) -> Vec<LogicalVfsNode> {
    groups
        .into_iter()
        .map(|(name, mut children)| {
            children.sort_by(logical_node_name);
            LogicalVfsNode::Directory(LogicalVfsDirectory { name, children })
        })
        .collect()
}

fn push_directory_if_any(
    root: &mut LogicalVfsDirectory,
    name: &str,
    children: Vec<LogicalVfsNode>,
) {
    if !children.is_empty() {
        root.children
            .push(LogicalVfsNode::Directory(LogicalVfsDirectory {
                name: name.to_owned(),
                children,
            }));
    }
}

fn logical_node_name(left: &LogicalVfsNode, right: &LogicalVfsNode) -> std::cmp::Ordering {
    logical_node_label(left).cmp(logical_node_label(right))
}

fn logical_node_label(node: &LogicalVfsNode) -> &str {
    match node {
        LogicalVfsNode::Directory(directory) => &directory.name,
        LogicalVfsNode::Object(object) => &object.name,
    }
}
