use crate::{
    AppError, ContextBudgetUsage, ContextBundleRequest, ContextEntry, ContextStage,
    context_bundle::{build_context_bundle, citation_for_object},
};
use nirmata_core::{
    claim::ClaimAuthentication,
    document::{DocumentCanonStatus, ObjectRef},
};
use nirmata_store::{ResolvedObject, StructuredSearchKind, StructuredSearchQuery, WorldStore};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchAuthority {
    Canonical,
    Perspective,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchClassification {
    Fact,
    Perspective,
    Inference,
    NoEvidence,
    Unspecified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptySearchClassification {
    NoEvidence,
    Unspecified,
}

impl From<EmptySearchClassification> for SearchClassification {
    fn from(value: EmptySearchClassification) -> Self {
        match value {
            EmptySearchClassification::NoEvidence => Self::NoEvidence,
            EmptySearchClassification::Unspecified => Self::Unspecified,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchResult {
    pub object_ref: ObjectRef,
    pub object_type: &'static str,
    pub object_id: String,
    pub uri: String,
    pub snippet: String,
    pub authority: SearchAuthority,
    pub classification: SearchClassification,
    pub provenance: String,
}

impl SearchResult {
    fn from_object(object: &ResolvedObject, snippet: String, provenance: String) -> Self {
        let object_ref = object.object_ref();
        let (authority, classification) = authority_and_classification(object);
        Self {
            object_ref,
            object_type: object_ref.kind(),
            object_id: object_id(object_ref),
            uri: object_ref.to_string(),
            snippet,
            authority,
            classification,
            provenance,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchAbsence {
    pub classification: SearchClassification,
    pub provenance: String,
}

impl SearchAbsence {
    fn new(classification: EmptySearchClassification, provenance: impl Into<String>) -> Self {
        Self {
            classification: classification.into(),
            provenance: provenance.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchWorldRequest {
    pub query: StructuredSearchQuery,
    pub empty: EmptySearchClassification,
}

impl SearchWorldRequest {
    pub fn new(query: StructuredSearchQuery) -> Self {
        Self {
            query,
            empty: EmptySearchClassification::NoEvidence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SearchWorldResponse {
    pub hits: Vec<SearchResult>,
    pub absence: Option<SearchAbsence>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OpenUriResponse {
    pub result: SearchResult,
    pub object: ResolvedObject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelatedContextRequest {
    pub bundle: ContextBundleRequest,
    pub kinds: Vec<StructuredSearchKind>,
    pub empty: EmptySearchClassification,
}

impl RelatedContextRequest {
    pub fn new(bundle: ContextBundleRequest) -> Self {
        Self {
            bundle,
            kinds: vec![],
            empty: EmptySearchClassification::NoEvidence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RelatedContextEntry {
    pub result: SearchResult,
    pub stage: ContextStage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RelatedContextResponse {
    pub canon: Vec<RelatedContextEntry>,
    pub perspectives: Vec<RelatedContextEntry>,
    pub desires: Vec<RelatedContextEntry>,
    pub obligations: Vec<RelatedContextEntry>,
    pub search_evidence: Vec<RelatedContextEntry>,
    pub usage: ContextBudgetUsage,
    pub absence: Option<SearchAbsence>,
}

impl RelatedContextResponse {
    pub fn all_entries(&self) -> Vec<&RelatedContextEntry> {
        self.canon
            .iter()
            .chain(self.perspectives.iter())
            .chain(self.desires.iter())
            .chain(self.obligations.iter())
            .chain(self.search_evidence.iter())
            .collect()
    }
}

pub(crate) fn search_world(
    store: &WorldStore,
    request: &SearchWorldRequest,
) -> Result<SearchWorldResponse, AppError> {
    let hits = store
        .search_structured(&request.query)?
        .into_iter()
        .map(|hit| {
            let object = store.resolve_object_ref(hit.object)?;
            Ok(SearchResult::from_object(
                &object,
                hit.fragment,
                hit.provenance,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(SearchWorldResponse {
        absence: hits
            .is_empty()
            .then(|| SearchAbsence::new(request.empty, "search_world")),
        hits,
    })
}

pub(crate) fn open_uri(store: &WorldStore, uri: &str) -> Result<OpenUriResponse, AppError> {
    let object = store.resolve_uri(uri)?;
    Ok(OpenUriResponse {
        result: SearchResult::from_object(
            &object,
            citation_for_object(&object),
            format!("open_uri:{uri}"),
        ),
        object,
    })
}

pub(crate) fn get_related_context(
    store: &WorldStore,
    request: &RelatedContextRequest,
) -> Result<RelatedContextResponse, AppError> {
    let bundle = build_context_bundle(store, &request.bundle)?;
    let canon = map_context_entries(bundle.canon, &request.kinds);
    let perspectives = map_context_entries(bundle.perspectives, &request.kinds);
    let desires = map_context_entries(bundle.desires, &request.kinds);
    let obligations = map_context_entries(bundle.obligations, &request.kinds);
    let search_evidence = map_context_entries(bundle.search_evidence, &request.kinds);
    let usage = usage_for_entries(
        request.bundle.budget.max_objects,
        request.bundle.budget.max_chars,
        [
            &canon,
            &perspectives,
            &desires,
            &obligations,
            &search_evidence,
        ],
    );
    let is_empty = canon.is_empty()
        && perspectives.is_empty()
        && desires.is_empty()
        && obligations.is_empty()
        && search_evidence.is_empty();

    Ok(RelatedContextResponse {
        canon,
        perspectives,
        desires,
        obligations,
        search_evidence,
        usage,
        absence: is_empty.then(|| SearchAbsence::new(request.empty, "get_related_context")),
    })
}

fn map_context_entries(
    entries: Vec<ContextEntry>,
    kinds: &[StructuredSearchKind],
) -> Vec<RelatedContextEntry> {
    entries
        .into_iter()
        .filter(|entry| matches_kind_filter(kinds, entry.object_ref()))
        .map(|entry| RelatedContextEntry {
            result: SearchResult::from_object(&entry.object, entry.citation, entry.provenance),
            stage: entry.stage,
        })
        .collect()
}

fn matches_kind_filter(kinds: &[StructuredSearchKind], object: ObjectRef) -> bool {
    kinds.is_empty()
        || kinds.iter().any(|kind| {
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
        })
}

fn usage_for_entries<const N: usize>(
    max_objects: usize,
    max_chars: usize,
    sections: [&Vec<RelatedContextEntry>; N],
) -> ContextBudgetUsage {
    let (used_objects, used_chars) =
        sections
            .into_iter()
            .flatten()
            .fold((0usize, 0usize), |(objects, chars), entry| {
                (
                    objects.saturating_add(1),
                    chars.saturating_add(entry.result.snippet.chars().count()),
                )
            });
    ContextBudgetUsage {
        max_objects,
        max_chars,
        used_objects,
        used_chars,
    }
}

fn authority_and_classification(
    object: &ResolvedObject,
) -> (SearchAuthority, SearchClassification) {
    match object {
        ResolvedObject::World(_) => (SearchAuthority::Canonical, SearchClassification::Fact),
        ResolvedObject::Entity(_)
        | ResolvedObject::Relation(_)
        | ResolvedObject::Event(_)
        | ResolvedObject::Rule(_)
        | ResolvedObject::Goal(_) => (SearchAuthority::Canonical, SearchClassification::Fact),
        ResolvedObject::Claim(claim) => match claim.authentication() {
            ClaimAuthentication::Canonical => {
                (SearchAuthority::Canonical, SearchClassification::Fact)
            }
            ClaimAuthentication::Attributed | ClaimAuthentication::Disputed => (
                SearchAuthority::Perspective,
                SearchClassification::Perspective,
            ),
        },
        ResolvedObject::Document(aggregate) => match aggregate.object().canon_status() {
            DocumentCanonStatus::Canonical => {
                (SearchAuthority::Canonical, SearchClassification::Fact)
            }
            DocumentCanonStatus::NonCanonical => (
                SearchAuthority::Perspective,
                SearchClassification::Perspective,
            ),
        },
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
