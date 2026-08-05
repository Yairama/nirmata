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
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
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
}

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
            let hits = self.search_by_text(&text)?;
            if !hits.is_empty() {
                stages.push(hits);
            } else {
                return Ok(vec![]);
            }
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
                left.object.kind(),
                object_id(left.object),
            )
                .cmp(&(
                    stage_priority(right.stage),
                    right.object.kind(),
                    object_id(right.object),
                ))
        });
        results.truncate(query.limit);
        Ok(results)
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

impl WorldStore {
    fn search_by_type(
        &self,
        kinds: &[StructuredSearchKind],
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let mut hits = Vec::new();
        let mut seen = BTreeSet::new();
        for kind in kinds {
            let kind_hits = match kind {
                StructuredSearchKind::Entity => self.collect_type_hits(
                    "SELECT id, name, summary FROM entities WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Entity(
                                EntityId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                            ]),
                            provenance: "type:entity".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
                StructuredSearchKind::Relation => self.collect_type_hits(
                    "SELECT id, kind FROM relations WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Relation(
                                RelationId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: "type:relation".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
                StructuredSearchKind::Event => self.collect_type_hits(
                    "SELECT id, summary FROM events WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Event(
                                EventId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: "type:event".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
                StructuredSearchKind::Claim => self.collect_type_hits(
                    "SELECT id, content_md FROM claims WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Claim(
                                ClaimId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: "type:claim".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
                StructuredSearchKind::Rule => self.collect_type_hits(
                    "SELECT id, statement_md FROM rules WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Rule(
                                RuleId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: "type:rule".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
                StructuredSearchKind::Goal => self.collect_type_hits(
                    "SELECT id, desired_state_md FROM goals WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Goal(
                                GoalId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: "type:goal".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
                StructuredSearchKind::Document => self.collect_type_hits(
                    "SELECT id, title, body_md FROM documents WHERE world_id = ?1 ORDER BY id",
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Document(
                                DocumentId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                            ]),
                            provenance: "type:document".to_owned(),
                            stage: StructuredSearchStage::Type,
                        })
                    },
                )?,
            };
            for hit in kind_hits {
                if seen.insert(hit.object) {
                    hits.push(hit);
                }
            }
        }
        Ok(hits)
    }

    fn collect_type_hits<T>(
        &self,
        sql: &str,
        mut map_row: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, StoreError> {
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([self.world_id.to_string()], |row| map_row(row))
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn search_by_alias(&self, alias: &str) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT e.id, e.name, e.summary, a.alias
                 FROM entity_aliases a
                 JOIN entities e
                   ON e.world_id = a.world_id
                  AND e.id = a.entity_id
                 WHERE a.world_id = ?1 AND a.alias = ?2
                 ORDER BY e.id, a.alias COLLATE NOCASE",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(params![self.world_id.to_string(), alias], |row| {
                Ok(StructuredSearchHit {
                    object: ObjectRef::Entity(
                        EntityId::from_str(&row.get::<_, String>(0)?)
                            .map_err(|error| invalid_data(0, error))?,
                    ),
                    fragment: preview(&[row.get::<_, String>(1)?, row.get::<_, String>(2)?]),
                    provenance: format!("alias:{}", row.get::<_, String>(3)?),
                    stage: StructuredSearchStage::Alias,
                })
            })
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn search_neighbors(
        &self,
        anchors: &[ObjectRef],
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let mut hits = Vec::new();
        for anchor in anchors {
            match *anchor {
                ObjectRef::World(_) => {}
                ObjectRef::Entity(id) => {
                    let mut statement = self
                        .connection
                        .prepare(
                            "SELECT r.id, r.kind, e.id, e.name, e.summary
                             FROM relations r
                             JOIN entities e
                               ON e.world_id = r.world_id
                              AND e.id = CASE
                                  WHEN r.source_entity_id = ?2 THEN r.target_entity_id
                                  ELSE r.source_entity_id
                              END
                             WHERE r.world_id = ?1
                               AND (r.source_entity_id = ?2 OR r.target_entity_id = ?2)
                             ORDER BY r.id, e.id",
                        )
                        .map_err(|error| map_schema_error(&self.path, error))?;
                    let anchor_label = anchor.to_string();
                    let rows = statement
                        .query_map(params![self.world_id.to_string(), id.to_string()], |row| {
                            Ok((
                                RelationId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                                row.get::<_, String>(1)?,
                                EntityId::from_str(&row.get::<_, String>(2)?)
                                    .map_err(|error| invalid_data(2, error))?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                            ))
                        })
                        .map_err(|error| map_schema_error(&self.path, error))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| map_schema_error(&self.path, error))?;
                    for (relation_id, relation_kind, entity_id, entity_name, entity_summary) in rows
                    {
                        hits.push(StructuredSearchHit {
                            object: ObjectRef::Relation(relation_id),
                            fragment: preview(&[relation_kind.clone()]),
                            provenance: format!("neighbor:{anchor_label}"),
                            stage: StructuredSearchStage::Neighbor,
                        });
                        hits.push(StructuredSearchHit {
                            object: ObjectRef::Entity(entity_id),
                            fragment: preview(&[entity_name, entity_summary]),
                            provenance: format!("neighbor:{anchor_label}:relation:{relation_kind}"),
                            stage: StructuredSearchStage::Neighbor,
                        });
                    }
                }
                ObjectRef::Event(id) => {
                    let mut statement = self
                        .connection
                        .prepare(
                            "SELECT e.id, e.summary, l.kind
                             FROM event_links l
                             JOIN events e
                               ON e.world_id = l.world_id
                              AND e.id = CASE
                                  WHEN l.source_event_id = ?2 THEN l.target_event_id
                                  ELSE l.source_event_id
                              END
                             WHERE l.world_id = ?1
                               AND (l.source_event_id = ?2 OR l.target_event_id = ?2)
                             ORDER BY e.id, l.kind",
                        )
                        .map_err(|error| map_schema_error(&self.path, error))?;
                    let anchor_label = anchor.to_string();
                    let rows = statement
                        .query_map(params![self.world_id.to_string(), id.to_string()], |row| {
                            Ok(StructuredSearchHit {
                                object: ObjectRef::Event(
                                    EventId::from_str(&row.get::<_, String>(0)?)
                                        .map_err(|error| invalid_data(0, error))?,
                                ),
                                fragment: preview(&[row.get::<_, String>(1)?]),
                                provenance: format!(
                                    "neighbor:{anchor_label}:event_link:{}",
                                    row.get::<_, String>(2)?
                                ),
                                stage: StructuredSearchStage::Neighbor,
                            })
                        })
                        .map_err(|error| map_schema_error(&self.path, error))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| map_schema_error(&self.path, error))?;
                    hits.extend(rows);
                }
                ObjectRef::Goal(id) => {
                    let mut statement = self
                        .connection
                        .prepare(
                            "SELECT e.id, e.summary
                             FROM event_goals eg
                             JOIN events e
                               ON e.world_id = eg.world_id
                              AND e.id = eg.event_id
                             WHERE eg.world_id = ?1 AND eg.goal_id = ?2
                             ORDER BY e.id",
                        )
                        .map_err(|error| map_schema_error(&self.path, error))?;
                    let anchor_label = anchor.to_string();
                    let rows = statement
                        .query_map(params![self.world_id.to_string(), id.to_string()], |row| {
                            Ok(StructuredSearchHit {
                                object: ObjectRef::Event(
                                    EventId::from_str(&row.get::<_, String>(0)?)
                                        .map_err(|error| invalid_data(0, error))?,
                                ),
                                fragment: preview(&[row.get::<_, String>(1)?]),
                                provenance: format!("neighbor:{anchor_label}:event_goal"),
                                stage: StructuredSearchStage::Neighbor,
                            })
                        })
                        .map_err(|error| map_schema_error(&self.path, error))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| map_schema_error(&self.path, error))?;
                    hits.extend(rows);
                }
                ObjectRef::Relation(_)
                | ObjectRef::Claim(_)
                | ObjectRef::Rule(_)
                | ObjectRef::Document(_) => {}
            }
        }
        Ok(hits)
    }

    fn search_by_goals(&self, goal_ids: &[GoalId]) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let mut hits = Vec::new();
        for goal_id in goal_ids {
            let goal_label = goal_id.to_string();

            let mut goal_statement = self
                .connection
                .prepare(
                    "SELECT id, desired_state_md
                     FROM goals
                     WHERE world_id = ?1 AND id = ?2",
                )
                .map_err(|error| map_schema_error(&self.path, error))?;
            let goal_rows = goal_statement
                .query_map(
                    params![self.world_id.to_string(), goal_label.clone()],
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Goal(
                                GoalId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: format!("goal:{goal_label}"),
                            stage: StructuredSearchStage::Goal,
                        })
                    },
                )
                .map_err(|error| map_schema_error(&self.path, error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_schema_error(&self.path, error))?;
            hits.extend(goal_rows);

            let mut event_statement = self
                .connection
                .prepare(
                    "SELECT e.id, e.summary
                     FROM event_goals eg
                     JOIN events e
                       ON e.world_id = eg.world_id
                      AND e.id = eg.event_id
                     WHERE eg.world_id = ?1 AND eg.goal_id = ?2
                     ORDER BY e.id",
                )
                .map_err(|error| map_schema_error(&self.path, error))?;
            let event_rows = event_statement
                .query_map(
                    params![self.world_id.to_string(), goal_label.clone()],
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Event(
                                EventId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: format!("goal:{goal_label}:event"),
                            stage: StructuredSearchStage::Goal,
                        })
                    },
                )
                .map_err(|error| map_schema_error(&self.path, error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_schema_error(&self.path, error))?;
            hits.extend(event_rows);
        }
        Ok(hits)
    }

    fn search_by_perspectives(
        &self,
        perspective_entity_ids: &[EntityId],
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let mut hits = Vec::new();
        for entity_id in perspective_entity_ids {
            let entity_label = entity_id.to_string();

            let mut claim_statement = self
                .connection
                .prepare(
                    "SELECT id, content_md
                     FROM claims
                     WHERE world_id = ?1 AND holder_entity_id = ?2
                     ORDER BY id",
                )
                .map_err(|error| map_schema_error(&self.path, error))?;
            let claim_rows = claim_statement
                .query_map(
                    params![self.world_id.to_string(), entity_label.clone()],
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Claim(
                                ClaimId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[row.get::<_, String>(1)?]),
                            provenance: format!("perspective:{entity_label}:claim"),
                            stage: StructuredSearchStage::Perspective,
                        })
                    },
                )
                .map_err(|error| map_schema_error(&self.path, error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_schema_error(&self.path, error))?;
            hits.extend(claim_rows);

            let mut document_statement = self
                .connection
                .prepare(
                    "SELECT id, title, body_md
                     FROM documents
                     WHERE world_id = ?1 AND perspective_entity_id = ?2
                     ORDER BY id",
                )
                .map_err(|error| map_schema_error(&self.path, error))?;
            let document_rows = document_statement
                .query_map(
                    params![self.world_id.to_string(), entity_label.clone()],
                    |row| {
                        Ok(StructuredSearchHit {
                            object: ObjectRef::Document(
                                DocumentId::from_str(&row.get::<_, String>(0)?)
                                    .map_err(|error| invalid_data(0, error))?,
                            ),
                            fragment: preview(&[
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                            ]),
                            provenance: format!("perspective:{entity_label}:document"),
                            stage: StructuredSearchStage::Perspective,
                        })
                    },
                )
                .map_err(|error| map_schema_error(&self.path, error))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_schema_error(&self.path, error))?;
            hits.extend(document_rows);
        }
        Ok(hits)
    }

    fn search_by_temporal(
        &self,
        temporal: StructuredSearchTemporal,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let (start, end, provenance) = temporal_bounds(temporal);
        let mut hits = Vec::new();

        let mut relation_statement = self
            .connection
            .prepare(
                "SELECT id, kind
                 FROM relations
                 WHERE world_id = ?1
                   AND (valid_from_tick IS NOT NULL OR valid_to_tick IS NOT NULL)
                   AND (?2 IS NULL OR valid_to_tick IS NULL OR valid_to_tick >= ?2)
                   AND (?3 IS NULL OR valid_from_tick IS NULL OR valid_from_tick <= ?3)
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let relation_rows = relation_statement
            .query_map(
                params![self.world_id.to_string(), start, end],
                |row| -> rusqlite::Result<StructuredSearchHit> {
                    Ok(StructuredSearchHit {
                        object: ObjectRef::Relation(
                            RelationId::from_str(&row.get::<_, String>(0)?)
                                .map_err(|error| invalid_data(0, error))?,
                        ),
                        fragment: preview(&[row.get::<_, String>(1)?]),
                        provenance: provenance.clone(),
                        stage: StructuredSearchStage::Temporal,
                    })
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        hits.extend(relation_rows);

        let mut goal_statement = self
            .connection
            .prepare(
                "SELECT id, desired_state_md
                 FROM goals
                 WHERE world_id = ?1
                   AND (valid_from_tick IS NOT NULL OR valid_to_tick IS NOT NULL)
                   AND (?2 IS NULL OR valid_to_tick IS NULL OR valid_to_tick >= ?2)
                   AND (?3 IS NULL OR valid_from_tick IS NULL OR valid_from_tick <= ?3)
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let goal_rows = goal_statement
            .query_map(
                params![self.world_id.to_string(), start, end],
                |row| -> rusqlite::Result<StructuredSearchHit> {
                    Ok(StructuredSearchHit {
                        object: ObjectRef::Goal(
                            GoalId::from_str(&row.get::<_, String>(0)?)
                                .map_err(|error| invalid_data(0, error))?,
                        ),
                        fragment: preview(&[row.get::<_, String>(1)?]),
                        provenance: provenance.clone(),
                        stage: StructuredSearchStage::Temporal,
                    })
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        hits.extend(goal_rows);

        let mut event_statement = self
            .connection
            .prepare(
                "SELECT id, summary
                 FROM events
                 WHERE world_id = ?1
                   AND time_kind <> 'unknown'
                   AND (?2 IS NULL OR end_tick IS NULL OR end_tick >= ?2)
                   AND (?3 IS NULL OR start_tick IS NULL OR start_tick <= ?3)
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let event_rows = event_statement
            .query_map(
                params![self.world_id.to_string(), start, end],
                |row| -> rusqlite::Result<StructuredSearchHit> {
                    Ok(StructuredSearchHit {
                        object: ObjectRef::Event(
                            EventId::from_str(&row.get::<_, String>(0)?)
                                .map_err(|error| invalid_data(0, error))?,
                        ),
                        fragment: preview(&[row.get::<_, String>(1)?]),
                        provenance: provenance.clone(),
                        stage: StructuredSearchStage::Temporal,
                    })
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        hits.extend(event_rows);

        let mut claim_statement = self
            .connection
            .prepare(
                "SELECT id, content_md
                 FROM claims
                 WHERE world_id = ?1
                   AND (valid_from_tick IS NOT NULL OR valid_to_tick IS NOT NULL)
                   AND (?2 IS NULL OR valid_to_tick IS NULL OR valid_to_tick >= ?2)
                   AND (?3 IS NULL OR valid_from_tick IS NULL OR valid_from_tick <= ?3)
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        let claim_rows = claim_statement
            .query_map(
                params![self.world_id.to_string(), start, end],
                |row| -> rusqlite::Result<StructuredSearchHit> {
                    Ok(StructuredSearchHit {
                        object: ObjectRef::Claim(
                            ClaimId::from_str(&row.get::<_, String>(0)?)
                                .map_err(|error| invalid_data(0, error))?,
                        ),
                        fragment: preview(&[row.get::<_, String>(1)?]),
                        provenance: provenance.clone(),
                        stage: StructuredSearchStage::Temporal,
                    })
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))?;
        hits.extend(claim_rows);

        Ok(hits)
    }

    fn search_by_text(&self, text: &str) -> Result<Vec<StructuredSearchHit>, StoreError> {
        let match_query = build_match_query(text);
        if match_query.is_empty() {
            return Ok(vec![]);
        }

        let mut statement = self
            .connection
            .prepare(
                "SELECT object_type, object_id,
                        snippet(canon_fts, -1, '[', ']', '…', 12)
                 FROM canon_fts
                 WHERE world_id = ?1 AND canon_fts MATCH ?2
                 ORDER BY object_type, object_id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(params![self.world_id.to_string(), match_query], |row| {
                Ok(StructuredSearchHit {
                    object: object_ref_from_row(row)?,
                    fragment: preview(&[row.get::<_, String>(2)?]),
                    provenance: "fts5".to_owned(),
                    stage: StructuredSearchStage::Text,
                })
            })
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }
}

impl WorldStore {
    fn direct_relation_refs(
        &self,
        entity_id: EntityId,
        limit: usize,
    ) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id
                 FROM relations
                 WHERE world_id = ?1
                   AND (source_entity_id = ?2 OR target_entity_id = ?2)
                 ORDER BY id
                 LIMIT ?3",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![
                    self.world_id.to_string(),
                    entity_id.to_string(),
                    i64::try_from(limit).map_err(|error| StoreError::Database(
                        self.path.clone(),
                        error.to_string()
                    ))?,
                ],
                |row| {
                    RelationId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Relation)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn associated_event_refs_for_entity(
        &self,
        entity_id: EntityId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT e.id
                 FROM events e
                 LEFT JOIN event_participants ep
                   ON ep.world_id = e.world_id
                  AND ep.event_id = e.id
                 WHERE e.world_id = ?1
                   AND (ep.entity_id = ?2 OR e.location_entity_id = ?2)
                 ORDER BY e.id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), entity_id.to_string()],
                |row| {
                    EventId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Event)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn associated_event_refs_for_goal(
        &self,
        goal_id: GoalId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT e.id
                 FROM event_goals eg
                 JOIN events e
                   ON e.world_id = eg.world_id
                  AND e.id = eg.event_id
                 WHERE eg.world_id = ?1 AND eg.goal_id = ?2
                 ORDER BY e.id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), goal_id.to_string()],
                |row| {
                    EventId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Event)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn participant_entity_refs_for_event(
        &self,
        event_id: EventId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT entity_id
                 FROM event_participants
                 WHERE world_id = ?1 AND event_id = ?2
                 ORDER BY ordinal",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), event_id.to_string()],
                |row| {
                    EntityId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Entity)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn claim_refs_for_entity(&self, entity_id: EntityId) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT id
                 FROM claims
                 WHERE world_id = ?1
                   AND (subject_entity_id = ?2 OR holder_entity_id = ?2 OR object_entity_id = ?2)
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), entity_id.to_string()],
                |row| {
                    ClaimId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Claim)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn claim_refs_for_document(
        &self,
        document_id: DocumentId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id
                 FROM claims
                 WHERE world_id = ?1 AND source_document_id = ?2
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), document_id.to_string()],
                |row| {
                    ClaimId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Claim)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn claim_refs_for_source_claim(&self, claim_id: ClaimId) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id
                 FROM claims
                 WHERE world_id = ?1 AND source_claim_id = ?2
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), claim_id.to_string()],
                |row| {
                    ClaimId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Claim)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn goal_refs_for_holder(&self, entity_id: EntityId) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id
                 FROM goals
                 WHERE world_id = ?1 AND holder_entity_id = ?2
                 ORDER BY id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), entity_id.to_string()],
                |row| {
                    GoalId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Goal)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn goal_refs_for_event(&self, event_id: EventId) -> Result<Vec<ObjectRef>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT goal_id
                 FROM event_goals
                 WHERE world_id = ?1 AND event_id = ?2
                 ORDER BY goal_id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map(
                params![self.world_id.to_string(), event_id.to_string()],
                |row| {
                    GoalId::from_str(&row.get::<_, String>(0)?)
                        .map(ObjectRef::Goal)
                        .map_err(|error| invalid_data(0, error))
                },
            )
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn applicable_rules_for_context(
        &self,
        context_objects: &[&ResolvedObject],
    ) -> Result<Vec<(Rule, String)>, StoreError> {
        let mut scope_tokens = BTreeSet::from(["world".to_owned()]);
        for object in context_objects {
            scope_tokens.extend(scope_tokens_for_object(object));
        }

        let mut matches = Vec::new();
        for rule in self.list_rules()? {
            let scope = normalize_scope(rule.scope());
            if let Some(matched_scope) = scope_tokens.iter().find(|token| **token == scope).cloned()
            {
                matches.push((rule, matched_scope));
            }
        }
        Ok(matches)
    }

    fn logical_entities(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        let mut groups = BTreeMap::<String, Vec<LogicalVfsNode>>::new();
        for entity in self.list_entities()? {
            groups
                .entry(entity_group_name(entity.kind()).to_owned())
                .or_default()
                .push(logical_object_node(
                    &display_name(entity.name(), ObjectRef::Entity(entity.id())),
                    ObjectRef::Entity(entity.id()),
                ));
        }
        Ok(directory_groups(groups))
    }

    fn logical_relations(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        Ok(self
            .list_relations()?
            .into_iter()
            .map(|relation| {
                logical_object_node(
                    &display_name(relation.kind(), ObjectRef::Relation(relation.id())),
                    ObjectRef::Relation(relation.id()),
                )
            })
            .collect())
    }

    fn logical_events(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        Ok(self
            .list_events()?
            .into_iter()
            .map(|event| {
                logical_object_node(
                    &display_name(
                        event.event().summary(),
                        ObjectRef::Event(event.event().id()),
                    ),
                    ObjectRef::Event(event.event().id()),
                )
            })
            .collect())
    }

    fn logical_claims(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        Ok(self
            .list_claims()?
            .into_iter()
            .map(|claim| {
                logical_object_node(
                    &display_name(
                        &preview(&[claim.content_md()]),
                        ObjectRef::Claim(claim.id()),
                    ),
                    ObjectRef::Claim(claim.id()),
                )
            })
            .collect())
    }

    fn logical_rules(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        Ok(self
            .list_rules()?
            .into_iter()
            .map(|rule| {
                logical_object_node(
                    &display_name(&preview(&[rule.statement_md()]), ObjectRef::Rule(rule.id())),
                    ObjectRef::Rule(rule.id()),
                )
            })
            .collect())
    }

    fn logical_goals(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        Ok(self
            .list_goals()?
            .into_iter()
            .map(|goal| {
                logical_object_node(
                    &display_name(
                        &preview(&[goal.desired_state_md()]),
                        ObjectRef::Goal(goal.id()),
                    ),
                    ObjectRef::Goal(goal.id()),
                )
            })
            .collect())
    }

    fn logical_documents(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
        let mut groups = BTreeMap::<String, Vec<LogicalVfsNode>>::new();
        for document in self.list_documents()? {
            groups
                .entry(document.object().kind().to_owned())
                .or_default()
                .push(logical_object_node(
                    &display_name(
                        document.object().title(),
                        ObjectRef::Document(document.object().id()),
                    ),
                    ObjectRef::Document(document.object().id()),
                ));
        }
        Ok(directory_groups(groups))
    }
}

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
        StructuredSearchStage::Type => 6,
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
