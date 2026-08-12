use crate::{
    AnchorContextBundle, AnchorContextEntry, AnchorContextQuery, CanonSnapshot,
    ChangeOperationValue, LogicalVfsDirectory, LogicalVfsNode, LogicalVfsObject, ResolvedObject,
    StoreError, StructuredSearchHit, StructuredSearchKind, StructuredSearchQuery,
    StructuredSearchStage, StructuredSearchTemporal, WorldStore, map_database_error,
    map_schema_error, stored_version, world_store::read_canon_snapshot_from_connection,
};
use nirmata_core::change_set::RetconKind;
use nirmata_core::{ChangeSetId, RevisionId, VariantId, WorldId, document::ObjectRef};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadScope {
    pub variant_id: VariantId,
    pub revision_id: Option<RevisionId>,
}

impl ReadScope {
    pub const fn head(variant_id: VariantId) -> Self {
        Self {
            variant_id,
            revision_id: None,
        }
    }

    pub const fn historical(variant_id: VariantId, revision_id: RevisionId) -> Self {
        Self {
            variant_id,
            revision_id: Some(revision_id),
        }
    }

    pub const fn is_historical(self) -> bool {
        self.revision_id.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub id: VariantId,
    pub world_id: WorldId,
    pub name: String,
    pub head_revision_id: RevisionId,
    pub archived: bool,
    pub created_from_revision_id: RevisionId,
    pub created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantDiffKind {
    Created,
    Deleted,
    Renamed,
    Edited,
    RelationDiverged,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantDiff {
    pub object_ref: ObjectRef,
    pub kind: VariantDiffKind,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub left_scope: ReadScope,
    pub right_scope: ReadScope,
    pub left_source: Option<VariantDiffSource>,
    pub right_source: Option<VariantDiffSource>,
    pub affected_references: Vec<ObjectRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantDiffSource {
    pub revision_id: RevisionId,
    pub change_set_id: ChangeSetId,
    pub operation_id: nirmata_core::ChangeOperationId,
    pub retcon: RetconKind,
    pub audit_source: String,
    pub scope: ReadScope,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantComparison {
    pub left: ReadScope,
    pub right: ReadScope,
    pub differences: Vec<VariantDiff>,
}

pub(crate) fn initialize_variant_history_in_tx(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
    now_ms: i64,
) -> Result<(), StoreError> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM variants", [], |row| row.get(0))
        .map_err(|error| map_schema_error(path, error))?;
    if count == 0 {
        let head_revision: String = connection
            .query_row(
                "SELECT current_revision FROM worlds WHERE id = ?1",
                [world_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|error| map_schema_error(path, error))?;
        let variant_id = VariantId::new();
        connection
            .execute(
                "INSERT INTO variants (
                        id, world_id, name, head_revision_id, archived,
                        created_from_revision_id, created_at_ms
                     ) VALUES (?1, ?2, 'main', ?3, 0, ?3, ?4)",
                params![
                    variant_id.to_string(),
                    world_id.to_string(),
                    head_revision,
                    now_ms,
                ],
            )
            .map_err(|error| map_database_error(path, error))?;
        connection
            .execute(
                "UPDATE worlds SET active_variant_id = ?1 WHERE id = ?2",
                params![variant_id.to_string(), world_id.to_string()],
            )
            .map_err(|error| map_database_error(path, error))?;
        connection
            .execute(
                "UPDATE revisions SET variant_id = ?1 WHERE variant_id IS NULL",
                [variant_id.to_string()],
            )
            .map_err(|error| map_database_error(path, error))?;
        connection
            .execute(
                "UPDATE change_sets SET variant_id = ?1 WHERE variant_id IS NULL",
                [variant_id.to_string()],
            )
            .map_err(|error| map_database_error(path, error))?;
        connection
            .execute(
                "UPDATE import_batches SET variant_id = ?1 WHERE variant_id IS NULL",
                [variant_id.to_string()],
            )
            .map_err(|error| map_database_error(path, error))?;
    }
    backfill_revision_snapshots_in_tx(connection, path, world_id)
}

fn backfill_revision_snapshots_in_tx(
    connection: &Connection,
    path: &Path,
    world_id: WorldId,
) -> Result<(), StoreError> {
    let missing: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM revisions r
             LEFT JOIN revision_snapshots s ON s.revision_id = r.id
             WHERE s.revision_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_schema_error(path, error))?;
    if missing == 0 {
        return Ok(());
    }
    let head: String = connection
        .query_row(
            "SELECT current_revision FROM worlds WHERE id = ?1",
            [world_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|error| map_schema_error(path, error))?;
    let head =
        RevisionId::from_str(&head).map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
    let mut revision = head;
    let mut snapshot =
        read_canon_snapshot_from_connection(connection, path, world_id)?.with_revision(head)?;
    loop {
        insert_snapshot(connection, path, revision, &snapshot)?;
        let row = connection
            .query_row(
                "SELECT parent_revision_id, change_set_id FROM revisions WHERE id = ?1",
                [revision.to_string()],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .map_err(|error| map_schema_error(path, error))?;
        let Some(parent) = row.0 else { break };
        let parent = RevisionId::from_str(&parent)
            .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
        if let Some(change_set_id) = row.1 {
            let id = ChangeSetId::from_str(&change_set_id)
                .map_err(|_| StoreError::InvalidFormat(path.to_owned()))?;
            let record =
                crate::change_set::load_committed_change_set_from_connection(connection, path, id)?
                    .ok_or(StoreError::InvalidFormat(path.to_owned()))?;
            rewind_snapshot(&mut snapshot, record.audits());
        }
        snapshot = snapshot.with_revision(parent)?;
        revision = parent;
    }
    let missing: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM revisions r
             LEFT JOIN revision_snapshots s ON s.revision_id = r.id
             WHERE s.revision_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_schema_error(path, error))?;
    if missing != 0 {
        return Err(StoreError::InvalidFormat(path.to_owned()));
    }
    Ok(())
}

impl WorldStore {
    pub fn active_variant(&self) -> Result<Variant, StoreError> {
        self.connection
            .query_row(
                "SELECT v.id, v.world_id, v.name, v.head_revision_id, v.archived,
                        v.created_from_revision_id, v.created_at_ms
                 FROM variants v JOIN worlds w ON w.active_variant_id = v.id",
                [],
                variant_from_row,
            )
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn active_read_scope(&self) -> Result<ReadScope, StoreError> {
        Ok(ReadScope::head(self.active_variant()?.id))
    }

    pub fn list_variants(&self) -> Result<Vec<Variant>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, world_id, name, head_revision_id, archived,
                        created_from_revision_id, created_at_ms
                 FROM variants ORDER BY archived, lower(name), id",
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        statement
            .query_map([], variant_from_row)
            .map_err(|error| map_schema_error(&self.path, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn get_variant(&self, id: VariantId) -> Result<Option<Variant>, StoreError> {
        self.connection
            .query_row(
                "SELECT id, world_id, name, head_revision_id, archived,
                        created_from_revision_id, created_at_ms
                 FROM variants WHERE id = ?1",
                [id.to_string()],
                variant_from_row,
            )
            .optional()
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn create_variant(
        &mut self,
        name: &str,
        from_revision: RevisionId,
        now_ms: i64,
    ) -> Result<Variant, StoreError> {
        let name = validate_variant_name(name)?;
        if !self.revision_exists(from_revision)? {
            return Err(StoreError::ObjectNotFound {
                object: "revision",
                id: from_revision.to_string(),
            });
        }
        self.snapshot_for_revision(from_revision)?;
        let id = VariantId::new();
        self.connection
            .execute(
                "INSERT INTO variants (
                    id, world_id, name, head_revision_id, archived,
                    created_from_revision_id, created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, 0, ?4, ?5)",
                params![
                    id.to_string(),
                    self.world_id.to_string(),
                    name,
                    from_revision.to_string(),
                    now_ms,
                ],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        self.get_variant(id)?.ok_or(StoreError::ObjectNotFound {
            object: "variant",
            id: id.to_string(),
        })
    }

    pub fn rename_variant(&mut self, id: VariantId, name: &str) -> Result<Variant, StoreError> {
        let name = validate_variant_name(name)?;
        let changed = self
            .connection
            .execute(
                "UPDATE variants SET name = ?1 WHERE id = ?2 AND archived = 0",
                params![name, id.to_string()],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        if changed != 1 {
            return Err(StoreError::InvalidVariant(
                "only an existing, unarchived variant can be renamed".to_owned(),
            ));
        }
        self.get_variant(id)?.ok_or(StoreError::ObjectNotFound {
            object: "variant",
            id: id.to_string(),
        })
    }

    pub fn archive_variant(
        &mut self,
        id: VariantId,
        allow_referenced: bool,
    ) -> Result<(), StoreError> {
        let active = self.active_variant()?;
        if active.id == id {
            return Err(StoreError::InvalidVariant(
                "the active variant cannot be archived".to_owned(),
            ));
        }
        let referenced: bool = self
            .connection
            .query_row(
                "WITH RECURSIVE descendant_origins(child_id, revision_id) AS (
                    SELECT id, created_from_revision_id
                    FROM variants
                    WHERE id <> ?1 AND archived = 0
                    UNION ALL
                    SELECT origins.child_id, revisions.parent_revision_id
                    FROM descendant_origins origins
                    JOIN revisions ON revisions.id = origins.revision_id
                    WHERE revisions.parent_revision_id IS NOT NULL
                 )
                 SELECT EXISTS(
                    SELECT 1
                    FROM descendant_origins origins
                    JOIN revisions ON revisions.id = origins.revision_id
                    WHERE revisions.variant_id = ?1
                    UNION ALL
                    SELECT 1 FROM import_batches WHERE variant_id = ?1 LIMIT 1
                 )",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        if referenced && !allow_referenced {
            return Err(StoreError::InvalidVariant(
                "variant has descendants or import references; explicit confirmation is required"
                    .to_owned(),
            ));
        }
        let changed = self
            .connection
            .execute(
                "UPDATE variants SET archived = 1 WHERE id = ?1 AND archived = 0",
                [id.to_string()],
            )
            .map_err(|error| map_database_error(&self.path, error))?;
        if changed != 1 {
            return Err(StoreError::ObjectNotFound {
                object: "variant",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    pub fn switch_variant(&mut self, id: VariantId) -> Result<Variant, StoreError> {
        let variant = self.get_variant(id)?.ok_or(StoreError::ObjectNotFound {
            object: "variant",
            id: id.to_string(),
        })?;
        if variant.archived {
            return Err(StoreError::InvalidVariant(
                "an archived variant cannot become active".to_owned(),
            ));
        }
        let snapshot = self.snapshot_for_revision(variant.head_revision_id)?;
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| map_database_error(&self.path, error))?;
        materialize_snapshot(&transaction, &self.path, &snapshot, id)?;
        transaction
            .commit()
            .map_err(|error| map_database_error(&self.path, error))?;
        Ok(variant)
    }

    pub fn resolve_scope(&self, scope: ReadScope) -> Result<RevisionId, StoreError> {
        let variant = self
            .get_variant(scope.variant_id)?
            .ok_or(StoreError::ObjectNotFound {
                object: "variant",
                id: scope.variant_id.to_string(),
            })?;
        let revision = scope.revision_id.unwrap_or(variant.head_revision_id);
        if !self.revision_is_in_variant_history(&variant, revision)? {
            return Err(StoreError::InvalidReadScope {
                variant_id: scope.variant_id,
                revision_id: revision,
            });
        }
        Ok(revision)
    }

    pub fn read_canon_snapshot_scoped(
        &self,
        scope: ReadScope,
    ) -> Result<CanonSnapshot, StoreError> {
        if self.is_materialized_scope(scope)? {
            return self.read_canon_snapshot();
        }
        let revision = self.resolve_scope(scope)?;
        self.snapshot_for_revision(revision)
    }

    pub fn resolve_uri_scoped(
        &self,
        scope: ReadScope,
        uri: &str,
    ) -> Result<ResolvedObject, StoreError> {
        if self.is_materialized_scope(scope)? {
            return self.resolve_uri(uri);
        }
        let object =
            ObjectRef::from_str(uri).map_err(|_| StoreError::InvalidObjectUri(uri.to_owned()))?;
        self.resolve_object_ref_scoped(scope, object)
    }

    pub fn resolve_object_ref_scoped(
        &self,
        scope: ReadScope,
        object: ObjectRef,
    ) -> Result<ResolvedObject, StoreError> {
        if self.is_materialized_scope(scope)? {
            return self.resolve_object_ref(object);
        }
        let snapshot = self.read_canon_snapshot_scoped(scope)?;
        resolved_from_snapshot(&snapshot, object).ok_or(StoreError::ObjectNotFound {
            object: object.kind(),
            id: object_id(object),
        })
    }

    pub fn search_structured_scoped(
        &self,
        scope: ReadScope,
        query: &StructuredSearchQuery,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
        if self.is_materialized_scope(scope)? {
            return self.search_structured(query);
        }
        let snapshot = self.read_canon_snapshot_scoped(scope)?;
        let revision = self.resolve_scope(scope)?;
        if query.limit == 0 {
            return Ok(vec![]);
        }
        let text = query
            .text
            .as_deref()
            .map(|value| value.trim().to_lowercase());
        let alias = query
            .alias
            .as_deref()
            .map(|value| value.trim().to_lowercase());
        let mut hits = snapshot_objects(&snapshot)
            .into_iter()
            .filter(|object| {
                query.kinds.is_empty()
                    || query
                        .kinds
                        .iter()
                        .any(|kind| kind_matches(*kind, object.object_ref()))
            })
            .filter(|object| {
                alias
                    .as_ref()
                    .is_none_or(|alias| alias_matches(object, alias))
            })
            .filter(|object| {
                text.as_ref()
                    .is_none_or(|text| searchable_text(object).to_lowercase().contains(text))
            })
            .filter(|object| {
                query.neighbors_of.is_empty() || is_neighbor(object, &query.neighbors_of)
            })
            .filter(|object| query.goal_ids.is_empty() || matches_goal(object, &query.goal_ids))
            .filter(|object| {
                query.perspective_entity_ids.is_empty()
                    || matches_perspective(object, &query.perspective_entity_ids)
            })
            .filter(|object| {
                query
                    .temporal
                    .is_none_or(|temporal| matches_temporal(object, temporal))
            })
            .map(|object| {
                let object_ref = object.object_ref();
                let stage = if alias.is_some() {
                    StructuredSearchStage::Alias
                } else if !query.neighbors_of.is_empty() {
                    StructuredSearchStage::Neighbor
                } else if !query.goal_ids.is_empty() {
                    StructuredSearchStage::Goal
                } else if !query.perspective_entity_ids.is_empty() {
                    StructuredSearchStage::Perspective
                } else if query.temporal.is_some() {
                    StructuredSearchStage::Temporal
                } else if text.is_some() {
                    StructuredSearchStage::Text
                } else {
                    StructuredSearchStage::Type
                };
                StructuredSearchHit {
                    object: object_ref,
                    fragment: searchable_text(&object).chars().take(240).collect(),
                    provenance: format!("scope:{}:{revision}", scope.variant_id),
                    stage,
                }
            })
            .collect::<Vec<_>>();
        hits.sort_by_key(|hit| (hit.object.kind(), object_id(hit.object)));
        hits.truncate(query.limit);
        Ok(hits)
    }

    pub fn load_anchor_context_scoped(
        &self,
        scope: ReadScope,
        query: &AnchorContextQuery,
    ) -> Result<AnchorContextBundle, StoreError> {
        if self.is_materialized_scope(scope)? {
            return self.load_anchor_context(query);
        }
        let snapshot = self.read_canon_snapshot_scoped(scope)?;
        let mut bundle = AnchorContextBundle::default();
        for anchor in &query.anchors {
            let object =
                resolved_from_snapshot(&snapshot, *anchor).ok_or(StoreError::ObjectNotFound {
                    object: anchor.kind(),
                    id: object_id(*anchor),
                })?;
            bundle.anchors.push(AnchorContextEntry {
                object,
                provenance: format!("anchor:{anchor}"),
            });
        }
        let anchor_entities = query
            .anchors
            .iter()
            .filter_map(|object| match object {
                ObjectRef::Entity(id) => Some(*id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for relation in &snapshot.relations {
            if bundle.relations.len() >= query.relation_limit {
                break;
            }
            if anchor_entities.contains(&relation.source_entity_id())
                || anchor_entities.contains(&relation.target_entity_id())
            {
                bundle.relations.push(AnchorContextEntry {
                    object: ResolvedObject::Relation(relation.clone()),
                    provenance: "relation:scoped".to_owned(),
                });
            }
        }
        for event in &snapshot.events {
            if event
                .event()
                .participants()
                .iter()
                .any(|participant| anchor_entities.contains(&participant.entity_id()))
            {
                bundle.events.push(AnchorContextEntry {
                    object: ResolvedObject::Event(event.clone()),
                    provenance: "event_participant:scoped".to_owned(),
                });
            }
        }
        for claim in &snapshot.claims {
            if anchor_entities.contains(&claim.subject_entity_id())
                || claim
                    .holder_entity_id()
                    .is_some_and(|id| anchor_entities.contains(&id))
            {
                bundle.claims.push(AnchorContextEntry {
                    object: ResolvedObject::Claim(claim.clone()),
                    provenance: "claim:scoped".to_owned(),
                });
            }
        }
        for goal in &snapshot.goals {
            if anchor_entities.contains(&goal.holder_entity_id()) {
                bundle.goals.push(AnchorContextEntry {
                    object: ResolvedObject::Goal(goal.clone()),
                    provenance: "goal:scoped".to_owned(),
                });
            }
        }
        bundle.rules.extend(
            snapshot
                .rules
                .iter()
                .cloned()
                .map(|rule| AnchorContextEntry {
                    object: ResolvedObject::Rule(rule),
                    provenance: "rule:scoped".to_owned(),
                }),
        );
        Ok(bundle)
    }

    pub fn read_logical_vfs_scoped(
        &self,
        scope: ReadScope,
    ) -> Result<LogicalVfsDirectory, StoreError> {
        if self.is_materialized_scope(scope)? {
            return self.read_logical_vfs();
        }
        let snapshot = self.read_canon_snapshot_scoped(scope)?;
        let mut groups = BTreeMap::<&str, Vec<LogicalVfsNode>>::new();
        for object in snapshot_objects(&snapshot) {
            let object_ref = object.object_ref();
            if matches!(object_ref, ObjectRef::World(_)) {
                continue;
            }
            groups
                .entry(directory_name(object_ref))
                .or_default()
                .push(LogicalVfsNode::Object(LogicalVfsObject {
                    name: visible_object_name(&object),
                    object: object_ref,
                    uri: object_ref.to_string(),
                }));
        }
        Ok(LogicalVfsDirectory {
            name: "/".to_owned(),
            children: groups
                .into_iter()
                .map(|(name, mut children)| {
                    children.sort_by_key(|node| match node {
                        LogicalVfsNode::Object(object) => object.name.clone(),
                        LogicalVfsNode::Directory(directory) => directory.name.clone(),
                    });
                    LogicalVfsNode::Directory(LogicalVfsDirectory {
                        name: name.to_owned(),
                        children,
                    })
                })
                .collect(),
        })
    }

    pub fn compare_scopes(
        &self,
        left: ReadScope,
        right: ReadScope,
    ) -> Result<VariantComparison, StoreError> {
        let left_revision = self.resolve_scope(left)?;
        let right_revision = self.resolve_scope(right)?;
        let left_snapshot = self.snapshot_for_revision(left_revision)?;
        let right_snapshot = self.snapshot_for_revision(right_revision)?;
        let left_values = snapshot_values(&left_snapshot)?;
        let right_values = snapshot_values(&right_snapshot)?;
        let mut refs = left_values.keys().copied().collect::<Vec<_>>();
        refs.extend(right_values.keys().copied());
        refs.sort();
        refs.dedup();
        let mut differences = Vec::new();
        for object_ref in refs {
            let before = left_values.get(&object_ref);
            let after = right_values.get(&object_ref);
            if before == after {
                continue;
            }
            let kind = match (before, after) {
                (None, Some(_)) => VariantDiffKind::Created,
                (Some(_), None) => VariantDiffKind::Deleted,
                (Some(before), Some(after)) if object_ref.kind() == "relation" => {
                    let _ = (before, after);
                    VariantDiffKind::RelationDiverged
                }
                (Some(before), Some(after)) if visible_name(before) != visible_name(after) => {
                    VariantDiffKind::Renamed
                }
                _ => VariantDiffKind::Edited,
            };
            differences.push(VariantDiff {
                object_ref,
                kind,
                before: before.cloned(),
                after: after.cloned(),
                left_scope: ReadScope::historical(left.variant_id, left_revision),
                right_scope: ReadScope::historical(right.variant_id, right_revision),
                left_source: self.object_provenance(
                    left_revision,
                    ReadScope::historical(left.variant_id, left_revision),
                    object_ref,
                    before.is_some(),
                )?,
                right_source: self.object_provenance(
                    right_revision,
                    ReadScope::historical(right.variant_id, right_revision),
                    object_ref,
                    after.is_some(),
                )?,
                affected_references: affected_references(
                    &left_snapshot,
                    &right_snapshot,
                    object_ref,
                ),
            });
        }
        Ok(VariantComparison {
            left,
            right,
            differences,
        })
    }

    fn object_provenance(
        &self,
        terminal_revision: RevisionId,
        scope: ReadScope,
        object_ref: ObjectRef,
        exists: bool,
    ) -> Result<Option<VariantDiffSource>, StoreError> {
        let mut current = Some(terminal_revision);
        while let Some(revision_id) = current {
            let revision = self
                .get_revision(revision_id)?
                .ok_or_else(|| StoreError::InvalidFormat(self.path.clone()))?;
            current = revision.parent_revision_id();
            let Some(change_set_id) = revision.change_set_id() else {
                continue;
            };
            let record = self
                .get_committed_change_set(change_set_id)?
                .ok_or_else(|| StoreError::InvalidFormat(self.path.clone()))?;
            for audit in record.audits() {
                let matches = if exists {
                    audit
                        .after()
                        .is_some_and(|value| value_ref(value) == object_ref)
                } else {
                    audit
                        .before()
                        .is_some_and(|value| value_ref(value) == object_ref)
                        && audit.after().is_none()
                };
                if !matches {
                    continue;
                }
                let operation = record
                    .change_set()
                    .operations()
                    .iter()
                    .find(|operation| operation.operation_id() == audit.operation_id())
                    .ok_or_else(|| StoreError::InvalidFormat(self.path.clone()))?;
                return Ok(Some(VariantDiffSource {
                    revision_id,
                    change_set_id,
                    operation_id: audit.operation_id(),
                    retcon: operation.retcon(),
                    audit_source: audit.source().to_owned(),
                    scope,
                }));
            }
        }
        Ok(None)
    }

    pub fn common_ancestor(
        &self,
        left: RevisionId,
        right: RevisionId,
    ) -> Result<RevisionId, StoreError> {
        self.connection
            .query_row(
                "WITH RECURSIVE
                 left_history(id, depth) AS (
                    SELECT ?1, 0
                    UNION ALL
                    SELECT r.parent_revision_id, depth + 1
                    FROM revisions r JOIN left_history h ON r.id = h.id
                    WHERE r.parent_revision_id IS NOT NULL
                 ),
                 right_history(id, depth) AS (
                    SELECT ?2, 0
                    UNION ALL
                    SELECT r.parent_revision_id, depth + 1
                    FROM revisions r JOIN right_history h ON r.id = h.id
                    WHERE r.parent_revision_id IS NOT NULL
                 )
                 SELECT l.id FROM left_history l JOIN right_history r ON r.id = l.id
                 ORDER BY l.depth + r.depth LIMIT 1",
                params![left.to_string(), right.to_string()],
                |row| parse_id(row, 0),
            )
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn revision_variant_id(&self, revision: RevisionId) -> Result<VariantId, StoreError> {
        self.connection
            .query_row(
                "SELECT variant_id FROM revisions WHERE id = ?1",
                [revision.to_string()],
                |row| parse_id(row, 0),
            )
            .map_err(|error| map_schema_error(&self.path, error))
    }

    pub fn revision_source_revision_id(
        &self,
        revision: RevisionId,
    ) -> Result<Option<RevisionId>, StoreError> {
        let value = self
            .connection
            .query_row(
                "SELECT source_revision_id FROM revisions WHERE id = ?1",
                [revision.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(|error| map_schema_error(&self.path, error))?;
        value
            .map(|value| {
                RevisionId::from_str(&value)
                    .map_err(|_| StoreError::InvalidFormat(self.path.clone()))
            })
            .transpose()
    }

    fn snapshot_for_revision(&self, revision: RevisionId) -> Result<CanonSnapshot, StoreError> {
        let json = self
            .connection
            .query_row(
                "SELECT snapshot_json FROM revision_snapshots WHERE revision_id = ?1",
                [revision.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| map_schema_error(&self.path, error))?
            .ok_or(StoreError::ObjectNotFound {
                object: "revision snapshot",
                id: revision.to_string(),
            })?;
        serde_json::from_str(&json).map_err(|_| StoreError::InvalidFormat(self.path.clone()))
    }

    fn revision_exists(&self, revision: RevisionId) -> Result<bool, StoreError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM revisions WHERE id = ?1)",
                [revision.to_string()],
                |row| row.get(0),
            )
            .map_err(|error| map_schema_error(&self.path, error))
    }

    fn is_materialized_scope(&self, scope: ReadScope) -> Result<bool, StoreError> {
        Ok(scope.revision_id.is_none() && self.active_variant()?.id == scope.variant_id)
    }

    fn revision_is_in_variant_history(
        &self,
        variant: &Variant,
        revision: RevisionId,
    ) -> Result<bool, StoreError> {
        self.connection
            .query_row(
                "WITH RECURSIVE history(id, parent_revision_id) AS (
                    SELECT id, parent_revision_id FROM revisions WHERE id = ?1
                    UNION ALL
                    SELECT r.id, r.parent_revision_id
                    FROM revisions r JOIN history h ON r.id = h.parent_revision_id
                 ) SELECT EXISTS(SELECT 1 FROM history WHERE id = ?2)",
                params![variant.head_revision_id.to_string(), revision.to_string()],
                |row| row.get(0),
            )
            .map_err(|error| map_schema_error(&self.path, error))
    }
}

fn validate_variant_name(name: &str) -> Result<String, StoreError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(StoreError::InvalidVariant(
            "variant name must contain 1 to 80 characters".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

fn insert_snapshot(
    connection: &Connection,
    path: &Path,
    revision: RevisionId,
    snapshot: &CanonSnapshot,
) -> Result<(), StoreError> {
    let json = serde_json::to_string(snapshot)
        .map_err(|error| StoreError::InvalidAggregate(error.to_string()))?;
    connection
        .execute(
            "INSERT OR IGNORE INTO revision_snapshots (revision_id, snapshot_json) VALUES (?1, ?2)",
            params![revision.to_string(), json],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

pub(crate) fn store_revision_snapshot_in_tx(
    transaction: &Transaction<'_>,
    path: &Path,
    revision: RevisionId,
    snapshot: &CanonSnapshot,
) -> Result<(), StoreError> {
    let json = serde_json::to_string(snapshot)
        .map_err(|error| StoreError::InvalidAggregate(error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO revision_snapshots (revision_id, snapshot_json) VALUES (?1, ?2)",
            params![revision.to_string(), json],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

fn materialize_snapshot(
    transaction: &Transaction<'_>,
    path: &Path,
    snapshot: &CanonSnapshot,
    variant_id: VariantId,
) -> Result<(), StoreError> {
    transaction
        .pragma_update(None, "defer_foreign_keys", true)
        .map_err(|error| map_database_error(path, error))?;
    transaction
        .execute_batch(
            "DELETE FROM content_references;
             DELETE FROM event_links;
             DELETE FROM event_goals;
             DELETE FROM event_participants;
             DELETE FROM claims;
             DELETE FROM documents;
             DELETE FROM events;
             DELETE FROM goals;
             DELETE FROM relations;
             DELETE FROM entity_aliases;
             DELETE FROM rules;
             DELETE FROM entities;
             DELETE FROM canon_fts;",
        )
        .map_err(|error| map_database_error(path, error))?;
    for entity in &snapshot.entities {
        crate::entity::insert_entity_in_tx(transaction, path, entity)?;
    }
    for rule in &snapshot.rules {
        crate::rule::insert_rule_in_tx(transaction, path, rule)?;
    }
    for relation in &snapshot.relations {
        crate::relation::insert_relation_in_tx(transaction, path, relation)?;
    }
    for goal in &snapshot.goals {
        crate::goal::insert_goal_in_tx(transaction, path, goal)?;
    }
    for event in &snapshot.events {
        crate::event::insert_event_in_tx(
            transaction,
            path,
            event,
            stored_version(event.event().version())?,
        )?;
    }
    for document in &snapshot.documents {
        crate::document::insert_document_in_tx(
            transaction,
            path,
            document,
            stored_version(document.object().version())?,
        )?;
    }
    for claim in &snapshot.claims {
        crate::claim::insert_claim_in_tx(transaction, path, claim)?;
    }
    let non_document_references = snapshot
        .content_references
        .iter()
        .filter(|reference| !matches!(reference.source(), ObjectRef::Document(_)))
        .cloned()
        .collect::<Vec<_>>();
    crate::content::insert(
        transaction,
        path,
        snapshot.world.id(),
        &non_document_references,
    )?;
    transaction
        .execute(
            "UPDATE worlds
             SET name = ?1, premise_md = ?2, epoch_label = ?3, current_revision = ?4,
                 updated_at_ms = ?5, active_variant_id = ?6
             WHERE id = ?7",
            params![
                snapshot.world.name(),
                snapshot.world.premise_md(),
                snapshot.world.epoch_label(),
                snapshot.world.current_revision().to_string(),
                snapshot.world.updated_at_ms(),
                variant_id.to_string(),
                snapshot.world.id().to_string(),
            ],
        )
        .map_err(|error| map_database_error(path, error))?;
    Ok(())
}

fn rewind_snapshot(snapshot: &mut CanonSnapshot, audits: &[crate::OperationAudit]) {
    for audit in audits.iter().rev() {
        match (audit.before(), audit.after()) {
            (Some(before), _) => put_value(snapshot, before.clone()),
            (None, Some(after)) => remove_value(snapshot, after),
            (None, None) => {}
        }
    }
}

fn put_value(snapshot: &mut CanonSnapshot, value: ChangeOperationValue) {
    match value {
        ChangeOperationValue::World(value) => snapshot.world = value,
        ChangeOperationValue::Entity(value) => replace(&mut snapshot.entities, value, |v| v.id()),
        ChangeOperationValue::Relation(value) => {
            replace(&mut snapshot.relations, value, |v| v.id())
        }
        ChangeOperationValue::Event(value) => {
            replace(&mut snapshot.events, value, |v| v.event().id())
        }
        ChangeOperationValue::Goal(value) => replace(&mut snapshot.goals, value, |v| v.id()),
        ChangeOperationValue::Rule(value) => replace(&mut snapshot.rules, value, |v| v.id()),
        ChangeOperationValue::Claim(value) => replace(&mut snapshot.claims, value, |v| v.id()),
        ChangeOperationValue::Document(value) => {
            snapshot
                .content_references
                .retain(|reference| reference.source() != ObjectRef::Document(value.object().id()));
            snapshot
                .content_references
                .extend(value.references().iter().cloned());
            replace(&mut snapshot.documents, value, |v| v.object().id());
        }
    }
}

fn remove_value(snapshot: &mut CanonSnapshot, value: &ChangeOperationValue) {
    match value {
        ChangeOperationValue::World(_) => {}
        ChangeOperationValue::Entity(value) => snapshot.entities.retain(|v| v.id() != value.id()),
        ChangeOperationValue::Relation(value) => {
            snapshot.relations.retain(|v| v.id() != value.id())
        }
        ChangeOperationValue::Event(value) => snapshot
            .events
            .retain(|v| v.event().id() != value.event().id()),
        ChangeOperationValue::Goal(value) => snapshot.goals.retain(|v| v.id() != value.id()),
        ChangeOperationValue::Rule(value) => snapshot.rules.retain(|v| v.id() != value.id()),
        ChangeOperationValue::Claim(value) => snapshot.claims.retain(|v| v.id() != value.id()),
        ChangeOperationValue::Document(value) => {
            snapshot
                .documents
                .retain(|v| v.object().id() != value.object().id());
            snapshot
                .content_references
                .retain(|reference| reference.source() != ObjectRef::Document(value.object().id()));
        }
    }
}

fn replace<T, I: Eq>(values: &mut Vec<T>, value: T, id: impl Fn(&T) -> I) {
    let value_id = id(&value);
    if let Some(index) = values.iter().position(|current| id(current) == value_id) {
        values[index] = value;
    } else {
        values.push(value);
    }
}

fn snapshot_values(snapshot: &CanonSnapshot) -> Result<BTreeMap<ObjectRef, Value>, StoreError> {
    let mut values = BTreeMap::new();
    let mut world = serde_json::to_value(&snapshot.world)
        .map_err(|error| StoreError::InvalidAggregate(error.to_string()))?;
    if let Some(world) = world.as_object_mut() {
        world.remove("current_revision");
        world.remove("updated_at_ms");
    }
    values.insert(ObjectRef::World(snapshot.world.id()), world);
    macro_rules! add_values {
        ($items:expr, $ref:path, $id:expr) => {
            for item in $items {
                values.insert(
                    $ref($id(item)),
                    serde_json::to_value(item)
                        .map_err(|error| StoreError::InvalidAggregate(error.to_string()))?,
                );
            }
        };
    }
    add_values!(
        &snapshot.entities,
        ObjectRef::Entity,
        |v: &nirmata_core::entity::Entity| v.id()
    );
    add_values!(
        &snapshot.relations,
        ObjectRef::Relation,
        |v: &nirmata_core::relation::Relation| v.id()
    );
    add_values!(
        &snapshot.events,
        ObjectRef::Event,
        |v: &nirmata_core::event::EventAggregate| v.event().id()
    );
    add_values!(
        &snapshot.claims,
        ObjectRef::Claim,
        |v: &nirmata_core::claim::Claim| v.id()
    );
    add_values!(
        &snapshot.rules,
        ObjectRef::Rule,
        |v: &nirmata_core::rule::Rule| v.id()
    );
    add_values!(
        &snapshot.goals,
        ObjectRef::Goal,
        |v: &nirmata_core::goal::Goal| v.id()
    );
    add_values!(
        &snapshot.documents,
        ObjectRef::Document,
        |v: &nirmata_core::document::DocumentAggregate| v.object().id()
    );
    Ok(values)
}

fn visible_name(value: &Value) -> Option<&str> {
    value
        .get("name")
        .or_else(|| value.get("title"))
        .or_else(|| value.get("summary"))
        .and_then(Value::as_str)
}

fn value_ref(value: &ChangeOperationValue) -> ObjectRef {
    match value {
        ChangeOperationValue::World(value) => ObjectRef::World(value.id()),
        ChangeOperationValue::Entity(value) => ObjectRef::Entity(value.id()),
        ChangeOperationValue::Relation(value) => ObjectRef::Relation(value.id()),
        ChangeOperationValue::Event(value) => ObjectRef::Event(value.event().id()),
        ChangeOperationValue::Goal(value) => ObjectRef::Goal(value.id()),
        ChangeOperationValue::Rule(value) => ObjectRef::Rule(value.id()),
        ChangeOperationValue::Claim(value) => ObjectRef::Claim(value.id()),
        ChangeOperationValue::Document(value) => ObjectRef::Document(value.object().id()),
    }
}

fn affected_references(
    left: &CanonSnapshot,
    right: &CanonSnapshot,
    target: ObjectRef,
) -> Vec<ObjectRef> {
    let mut affected = BTreeMap::<ObjectRef, ()>::new();
    for object in snapshot_objects(left)
        .into_iter()
        .chain(snapshot_objects(right))
    {
        let object_ref = object.object_ref();
        let references = resolved_references(&object);
        if object_ref == target {
            for reference in references {
                affected.insert(reference, ());
            }
        } else if references.contains(&target) {
            affected.insert(object_ref, ());
        }
    }
    affected.remove(&target);
    affected.into_keys().collect()
}

fn resolved_references(object: &ResolvedObject) -> Vec<ObjectRef> {
    match object {
        ResolvedObject::World(_) | ResolvedObject::Entity(_) | ResolvedObject::Rule(_) => vec![],
        ResolvedObject::Relation(value) => vec![
            ObjectRef::Entity(value.source_entity_id()),
            ObjectRef::Entity(value.target_entity_id()),
        ],
        ResolvedObject::Goal(value) => vec![ObjectRef::Entity(value.holder_entity_id())],
        ResolvedObject::Event(value) => value
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
        ResolvedObject::Claim(value) => {
            std::iter::once(ObjectRef::Entity(value.subject_entity_id()))
                .chain(value.holder_entity_id().map(ObjectRef::Entity))
                .chain(value.object().and_then(|object| match object {
                    nirmata_core::claim::ClaimObject::Entity(id) => Some(ObjectRef::Entity(*id)),
                    nirmata_core::claim::ClaimObject::Scalar(_) => None,
                }))
                .chain(value.source_document_id().map(ObjectRef::Document))
                .chain(value.source_claim_id().map(ObjectRef::Claim))
                .collect()
        }
        ResolvedObject::Document(value) => value
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

fn snapshot_objects(snapshot: &CanonSnapshot) -> Vec<ResolvedObject> {
    std::iter::once(ResolvedObject::World(snapshot.world.clone()))
        .chain(
            snapshot
                .entities
                .iter()
                .cloned()
                .map(ResolvedObject::Entity),
        )
        .chain(
            snapshot
                .relations
                .iter()
                .cloned()
                .map(ResolvedObject::Relation),
        )
        .chain(snapshot.events.iter().cloned().map(ResolvedObject::Event))
        .chain(snapshot.claims.iter().cloned().map(ResolvedObject::Claim))
        .chain(snapshot.rules.iter().cloned().map(ResolvedObject::Rule))
        .chain(snapshot.goals.iter().cloned().map(ResolvedObject::Goal))
        .chain(
            snapshot
                .documents
                .iter()
                .cloned()
                .map(ResolvedObject::Document),
        )
        .collect()
}

fn resolved_from_snapshot(snapshot: &CanonSnapshot, object: ObjectRef) -> Option<ResolvedObject> {
    snapshot_objects(snapshot)
        .into_iter()
        .find(|candidate| candidate.object_ref() == object)
}

fn kind_matches(kind: StructuredSearchKind, object: ObjectRef) -> bool {
    matches!(
        (kind, object),
        (StructuredSearchKind::Entity, ObjectRef::Entity(_))
            | (StructuredSearchKind::Relation, ObjectRef::Relation(_))
            | (StructuredSearchKind::Event, ObjectRef::Event(_))
            | (StructuredSearchKind::Claim, ObjectRef::Claim(_))
            | (StructuredSearchKind::Rule, ObjectRef::Rule(_))
            | (StructuredSearchKind::Goal, ObjectRef::Goal(_))
            | (StructuredSearchKind::Document, ObjectRef::Document(_))
    )
}

fn searchable_text(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::World(value) => format!("{} {}", value.name(), value.premise_md()),
        ResolvedObject::Entity(value) => format!(
            "{} {} {} {}",
            value.name(),
            value.aliases().join(" "),
            value.summary(),
            value.body_md()
        ),
        ResolvedObject::Relation(value) => format!(
            "{} {}",
            value.kind(),
            value.source_reference().unwrap_or_default()
        ),
        ResolvedObject::Event(value) => format!(
            "{} {} {}",
            value.event().kind(),
            value.event().summary(),
            value.event().body_md()
        ),
        ResolvedObject::Claim(value) => value.content_md().to_owned(),
        ResolvedObject::Rule(value) => value.statement_md().to_owned(),
        ResolvedObject::Goal(value) => value.desired_state_md().to_owned(),
        ResolvedObject::Document(value) => {
            format!("{} {}", value.object().title(), value.object().body_md())
        }
    }
}

fn alias_matches(object: &ResolvedObject, alias: &str) -> bool {
    matches!(object, ResolvedObject::Entity(entity) if entity.aliases().iter().any(|value| value.to_lowercase() == alias))
}

fn is_neighbor(object: &ResolvedObject, anchors: &[ObjectRef]) -> bool {
    match object {
        ResolvedObject::Relation(relation) => anchors.iter().any(|anchor| matches!(
            anchor,
            ObjectRef::Entity(id) if *id == relation.source_entity_id() || *id == relation.target_entity_id()
        )),
        ResolvedObject::Event(event) => anchors.iter().any(|anchor| matches!(
            anchor,
            ObjectRef::Entity(id) if event.event().participants().iter().any(|participant| participant.entity_id() == *id)
        )),
        _ => false,
    }
}

fn matches_goal(object: &ResolvedObject, goals: &[nirmata_core::GoalId]) -> bool {
    match object {
        ResolvedObject::Goal(goal) => goals.contains(&goal.id()),
        ResolvedObject::Event(event) => event
            .event()
            .affected_goal_ids()
            .iter()
            .any(|id| goals.contains(id)),
        _ => false,
    }
}

fn matches_perspective(object: &ResolvedObject, entities: &[nirmata_core::EntityId]) -> bool {
    match object {
        ResolvedObject::Claim(claim) => claim
            .holder_entity_id()
            .is_some_and(|id| entities.contains(&id)),
        ResolvedObject::Document(document) => document
            .object()
            .perspective_entity_id()
            .is_some_and(|id| entities.contains(&id)),
        _ => false,
    }
}

fn matches_temporal(object: &ResolvedObject, temporal: StructuredSearchTemporal) -> bool {
    let bounds = match object {
        ResolvedObject::Event(event) => (
            event.event().time().start_tick(),
            event.event().time().end_tick(),
        ),
        ResolvedObject::Relation(relation) => {
            (relation.valid_from_tick(), relation.valid_to_tick())
        }
        ResolvedObject::Goal(goal) => goal
            .period()
            .map(|value| (value.start_tick(), value.end_tick()))
            .unwrap_or((None, None)),
        ResolvedObject::Claim(claim) => claim
            .period()
            .map(|value| (value.start_tick(), value.end_tick()))
            .unwrap_or((None, None)),
        _ => return false,
    };
    match temporal {
        StructuredSearchTemporal::Tick(tick) => {
            bounds.0.is_none_or(|start| start <= tick) && bounds.1.is_none_or(|end| tick <= end)
        }
        StructuredSearchTemporal::Period(period) => {
            bounds
                .1
                .is_none_or(|end| period.start_tick().is_none_or(|start| end >= start))
                && period
                    .end_tick()
                    .is_none_or(|end| bounds.0.is_none_or(|start| start <= end))
        }
    }
}

fn visible_object_name(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::World(value) => value.name().to_owned(),
        ResolvedObject::Entity(value) => value.name().to_owned(),
        ResolvedObject::Relation(value) => value.kind().to_owned(),
        ResolvedObject::Event(value) => value.event().summary().to_owned(),
        ResolvedObject::Claim(value) => value.content_md().chars().take(80).collect(),
        ResolvedObject::Rule(value) => value.statement_md().chars().take(80).collect(),
        ResolvedObject::Goal(value) => value.desired_state_md().chars().take(80).collect(),
        ResolvedObject::Document(value) => value.object().title().to_owned(),
    }
}

fn directory_name(object: ObjectRef) -> &'static str {
    match object {
        ObjectRef::Entity(_) => "entities",
        ObjectRef::Relation(_) => "relations",
        ObjectRef::Event(_) => "events",
        ObjectRef::Claim(_) => "claims",
        ObjectRef::Rule(_) => "rules",
        ObjectRef::Goal(_) => "goals",
        ObjectRef::Document(_) => "documents",
        ObjectRef::World(_) => "worlds",
    }
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

fn variant_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Variant> {
    Ok(Variant {
        id: parse_id(row, 0)?,
        world_id: parse_id(row, 1)?,
        name: row.get(2)?,
        head_revision_id: parse_id(row, 3)?,
        archived: row.get(4)?,
        created_from_revision_id: parse_id(row, 5)?,
        created_at_ms: row.get(6)?,
    })
}

fn parse_id<T: FromStr>(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    row.get::<_, String>(index)?.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
