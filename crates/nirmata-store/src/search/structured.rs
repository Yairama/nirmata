use super::*;

impl WorldStore {
    pub(super) fn search_by_type(
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

    pub(super) fn collect_type_hits<T>(
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

    pub(super) fn search_by_alias(
        &self,
        alias: &str,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
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

    pub(super) fn search_neighbors(
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

    pub(super) fn search_by_goals(
        &self,
        goal_ids: &[GoalId],
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
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

    pub(super) fn search_by_perspectives(
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

    pub(super) fn search_by_temporal(
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

    pub(super) fn search_by_text(
        &self,
        text: &str,
    ) -> Result<Vec<StructuredSearchHit>, StoreError> {
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
