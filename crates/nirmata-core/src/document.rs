use crate::{
    ClaimId, DocumentId, DomainError, EntityId, EventId, GoalId, RelationId, RuleId, WorldId,
    validate_version,
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectRef {
    World(WorldId),
    Entity(EntityId),
    Relation(RelationId),
    Event(EventId),
    Claim(ClaimId),
    Rule(RuleId),
    Goal(GoalId),
    Document(DocumentId),
}

impl ObjectRef {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::World(_) => "world",
            Self::Entity(_) => "entity",
            Self::Relation(_) => "relation",
            Self::Event(_) => "event",
            Self::Claim(_) => "claim",
            Self::Rule(_) => "rule",
            Self::Goal(_) => "goal",
            Self::Document(_) => "document",
        }
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = match self {
            Self::World(id) => id.to_string(),
            Self::Entity(id) => id.to_string(),
            Self::Relation(id) => id.to_string(),
            Self::Event(id) => id.to_string(),
            Self::Claim(id) => id.to_string(),
            Self::Rule(id) => id.to_string(),
            Self::Goal(id) => id.to_string(),
            Self::Document(id) => id.to_string(),
        };
        write!(formatter, "nirmata://{}/{id}", self.kind())
    }
}

impl FromStr for ObjectRef {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let path = value
            .strip_prefix("nirmata://")
            .ok_or(DomainError::InvalidContentUri)?;
        let (kind, id) = path.split_once('/').ok_or(DomainError::InvalidContentUri)?;
        if id.is_empty() || id.contains('/') {
            return Err(DomainError::InvalidContentUri);
        }

        match kind {
            "world" => WorldId::from_str(id).map(Self::World),
            "entity" => EntityId::from_str(id).map(Self::Entity),
            "relation" => RelationId::from_str(id).map(Self::Relation),
            "event" => EventId::from_str(id).map(Self::Event),
            "claim" => ClaimId::from_str(id).map(Self::Claim),
            "rule" => RuleId::from_str(id).map(Self::Rule),
            "goal" => GoalId::from_str(id).map(Self::Goal),
            "document" => DocumentId::from_str(id).map(Self::Document),
            _ => return Err(DomainError::InvalidContentUri),
        }
        .map_err(|_| DomainError::InvalidContentUri)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContentReference {
    source: ObjectRef,
    target: ObjectRef,
    ordinal: u32,
}

impl ContentReference {
    pub fn new(source: ObjectRef, target: ObjectRef, ordinal: u32) -> Self {
        Self {
            source,
            target,
            ordinal,
        }
    }

    pub fn source(&self) -> ObjectRef {
        self.source
    }

    pub fn target(&self) -> ObjectRef {
        self.target
    }

    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

pub fn ordered_content_references(
    source: ObjectRef,
    references: &[ContentReference],
) -> Vec<&ContentReference> {
    let mut ordered: Vec<_> = references
        .iter()
        .filter(|reference| reference.source == source)
        .collect();
    ordered.sort_by_key(|reference| reference.ordinal);
    ordered
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DocumentAggregate {
    object: Document,
    references: Vec<ContentReference>,
}

impl DocumentAggregate {
    pub fn new(object: Document, references: Vec<ContentReference>) -> Self {
        Self { object, references }
    }

    pub fn object(&self) -> &Document {
        &self.object
    }

    pub fn references(&self) -> &[ContentReference] {
        &self.references
    }

    pub fn into_parts(self) -> (Document, Vec<ContentReference>) {
        (self.object, self.references)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCanonStatus {
    Canonical,
    NonCanonical,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Document {
    id: DocumentId,
    world_id: WorldId,
    title: String,
    kind: String,
    author_entity_id: Option<EntityId>,
    perspective_entity_id: Option<EntityId>,
    canon_status: DocumentCanonStatus,
    body_md: String,
    version: u64,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl Document {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        title: impl Into<String>,
        kind: impl Into<String>,
        author_entity_id: Option<EntityId>,
        perspective_entity_id: Option<EntityId>,
        canon_status: DocumentCanonStatus,
        body_md: impl Into<String>,
        now_ms: i64,
    ) -> Result<Self, DomainError> {
        Self::restore(
            DocumentId::new(),
            world_id,
            title,
            kind,
            author_entity_id,
            perspective_entity_id,
            canon_status,
            body_md,
            1,
            now_ms,
            now_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: DocumentId,
        world_id: WorldId,
        title: impl Into<String>,
        kind: impl Into<String>,
        author_entity_id: Option<EntityId>,
        perspective_entity_id: Option<EntityId>,
        canon_status: DocumentCanonStatus,
        body_md: impl Into<String>,
        version: u64,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;
        Ok(Self {
            id,
            world_id,
            title: title.into(),
            kind: kind.into(),
            author_entity_id,
            perspective_entity_id,
            canon_status,
            body_md: body_md.into(),
            version,
            created_at_ms,
            updated_at_ms,
        })
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn author_entity_id(&self) -> Option<EntityId> {
        self.author_entity_id
    }

    pub fn perspective_entity_id(&self) -> Option<EntityId> {
        self.perspective_entity_id
    }

    pub fn canon_status(&self) -> DocumentCanonStatus {
        self.canon_status
    }

    pub fn body_md(&self) -> &str {
        &self.body_md
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

#[cfg(test)]
#[path = "../tests/unit/document/mod.rs"]
mod tests;
