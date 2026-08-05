use crate::{DomainError, EntityId, JsonObject, WorldId, required, validate_version};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Person,
    Place,
    Faction,
    Culture,
    Resource,
    Concept,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    id: EntityId,
    world_id: WorldId,
    kind: EntityKind,
    name: String,
    slug: String,
    summary: String,
    body_md: String,
    attributes_json: JsonObject,
    aliases: Vec<String>,
    version: u64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl Entity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        kind: EntityKind,
        name: impl Into<String>,
        slug: impl Into<String>,
        summary: impl Into<String>,
        body_md: impl Into<String>,
        attributes_json: impl Into<String>,
        aliases: Vec<String>,
        now_ms: i64,
    ) -> Result<Self, DomainError> {
        Self::restore(
            EntityId::new(),
            world_id,
            kind,
            name,
            slug,
            summary,
            body_md,
            attributes_json,
            aliases,
            1,
            now_ms,
            now_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: EntityId,
        world_id: WorldId,
        kind: EntityKind,
        name: impl Into<String>,
        slug: impl Into<String>,
        summary: impl Into<String>,
        body_md: impl Into<String>,
        attributes_json: impl Into<String>,
        aliases: Vec<String>,
        version: u64,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;

        Ok(Self {
            id,
            world_id,
            kind,
            name: required("name", name)?,
            slug: required("slug", slug)?,
            summary: summary.into(),
            body_md: body_md.into(),
            attributes_json: JsonObject::new("attributes_json", attributes_json)?,
            aliases: normalize_aliases(aliases)?,
            version,
            created_at_ms,
            updated_at_ms,
        })
    }

    pub fn rename(
        &mut self,
        name: impl Into<String>,
        slug: impl Into<String>,
        now_ms: i64,
    ) -> Result<(), DomainError> {
        let name = required("name", name)?;
        let slug = required("slug", slug)?;
        let version = self
            .version
            .checked_add(1)
            .ok_or(DomainError::VersionOverflow)?;

        self.name = name;
        self.slug = slug;
        self.version = version;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn kind(&self) -> EntityKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn body_md(&self) -> &str {
        &self.body_md
    }

    pub fn attributes_json(&self) -> &JsonObject {
        &self.attributes_json
    }

    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

fn normalize_aliases(aliases: Vec<String>) -> Result<Vec<String>, DomainError> {
    let mut normalized = Vec::with_capacity(aliases.len());
    let mut seen = HashSet::with_capacity(aliases.len());

    for alias in aliases {
        let alias = required("alias", alias)?;
        let key = alias.to_lowercase();
        if !seen.insert(key) {
            return Err(DomainError::DuplicateAlias(alias));
        }
        normalized.push(alias);
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_aliases_and_preserves_identity_when_renamed() {
        let mut entity = Entity::new(
            WorldId::new(),
            EntityKind::Person,
            "Mara",
            "mara",
            "",
            "",
            "{}",
            vec!["  The Cartographer  ".to_owned()],
            1,
        )
        .expect("valid entity");
        let id = entity.id();

        assert_eq!(entity.aliases(), ["The Cartographer"]);
        entity.rename("Mara Vale", "mara-vale", 2).expect("rename");
        assert_eq!(entity.id(), id);
        assert_eq!(entity.name(), "Mara Vale");
        assert_eq!(entity.version(), 2);
    }

    #[test]
    fn rejects_empty_duplicate_aliases_and_invalid_json() {
        let duplicate = Entity::new(
            WorldId::new(),
            EntityKind::Person,
            "Mara",
            "mara",
            "",
            "",
            "{}",
            vec!["Witness".to_owned(), " witness ".to_owned()],
            1,
        );
        assert!(matches!(duplicate, Err(DomainError::DuplicateAlias(_))));

        let empty = Entity::new(
            WorldId::new(),
            EntityKind::Person,
            "",
            "mara",
            "",
            "",
            "{}",
            vec![],
            1,
        );
        assert_eq!(empty, Err(DomainError::EmptyField { field: "name" }));

        let invalid_json = Entity::new(
            WorldId::new(),
            EntityKind::Person,
            "Mara",
            "mara",
            "",
            "",
            "[]",
            vec![],
            1,
        );
        assert_eq!(
            invalid_json,
            Err(DomainError::InvalidJsonObject {
                field: "attributes_json"
            })
        );
    }
}
