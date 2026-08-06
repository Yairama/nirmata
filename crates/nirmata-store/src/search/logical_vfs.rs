use super::*;

impl WorldStore {
    pub(super) fn direct_relation_refs(
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

    pub(super) fn associated_event_refs_for_entity(
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

    pub(super) fn associated_event_refs_for_goal(
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

    pub(super) fn participant_entity_refs_for_event(
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

    pub(super) fn claim_refs_for_entity(
        &self,
        entity_id: EntityId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
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

    pub(super) fn claim_refs_for_document(
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

    pub(super) fn claim_refs_for_source_claim(
        &self,
        claim_id: ClaimId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
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

    pub(super) fn goal_refs_for_holder(
        &self,
        entity_id: EntityId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
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

    pub(super) fn goal_refs_for_event(
        &self,
        event_id: EventId,
    ) -> Result<Vec<ObjectRef>, StoreError> {
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

    pub(super) fn applicable_rules_for_context(
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

    pub(super) fn logical_entities(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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

    pub(super) fn logical_relations(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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

    pub(super) fn logical_events(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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

    pub(super) fn logical_claims(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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

    pub(super) fn logical_rules(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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

    pub(super) fn logical_goals(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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

    pub(super) fn logical_documents(&self) -> Result<Vec<LogicalVfsNode>, StoreError> {
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
