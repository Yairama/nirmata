use crate::{
    ClaimId, DocumentId, DomainError, EntityId, Period, RevisionId, WorldId, validate_version,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimPolarity {
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimAuthentication {
    Canonical,
    Attributed,
    Disputed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimModality {
    Assertion,
    Belief,
    Hypothesis,
    Counterfactual,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimObject {
    Entity(EntityId),
    Scalar(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    id: ClaimId,
    world_id: WorldId,
    subject_entity_id: EntityId,
    content_md: String,
    predicate_key: Option<String>,
    object: Option<ClaimObject>,
    polarity: ClaimPolarity,
    authentication: ClaimAuthentication,
    holder_entity_id: Option<EntityId>,
    modality: Option<ClaimModality>,
    register: Option<String>,
    epistemic_basis: Option<String>,
    source: Option<String>,
    source_document_id: Option<DocumentId>,
    source_claim_id: Option<ClaimId>,
    holder_confidence: Option<f64>,
    period: Option<Period>,
    registered_revision_id: RevisionId,
    superseded_revision_id: Option<RevisionId>,
    version: u64,
}

impl Claim {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world_id: WorldId,
        subject_entity_id: EntityId,
        content_md: impl Into<String>,
        predicate_key: Option<String>,
        object: Option<ClaimObject>,
        polarity: ClaimPolarity,
        authentication: ClaimAuthentication,
        holder_entity_id: Option<EntityId>,
        modality: Option<ClaimModality>,
        register: Option<String>,
        epistemic_basis: Option<String>,
        source: Option<String>,
        source_document_id: Option<DocumentId>,
        source_claim_id: Option<ClaimId>,
        holder_confidence: Option<f64>,
        period: Option<Period>,
        registered_revision_id: RevisionId,
    ) -> Result<Self, DomainError> {
        Self::restore(
            ClaimId::new(),
            world_id,
            subject_entity_id,
            content_md,
            predicate_key,
            object,
            polarity,
            authentication,
            holder_entity_id,
            modality,
            register,
            epistemic_basis,
            source,
            source_document_id,
            source_claim_id,
            holder_confidence,
            period,
            registered_revision_id,
            None,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: ClaimId,
        world_id: WorldId,
        subject_entity_id: EntityId,
        content_md: impl Into<String>,
        predicate_key: Option<String>,
        object: Option<ClaimObject>,
        polarity: ClaimPolarity,
        authentication: ClaimAuthentication,
        holder_entity_id: Option<EntityId>,
        modality: Option<ClaimModality>,
        register: Option<String>,
        epistemic_basis: Option<String>,
        source: Option<String>,
        source_document_id: Option<DocumentId>,
        source_claim_id: Option<ClaimId>,
        holder_confidence: Option<f64>,
        period: Option<Period>,
        registered_revision_id: RevisionId,
        superseded_revision_id: Option<RevisionId>,
        version: u64,
    ) -> Result<Self, DomainError> {
        validate_version(version)?;
        validate_context(authentication, holder_entity_id, modality)?;
        if predicate_key.is_some() != object.is_some() {
            return Err(DomainError::InvalidClaimContext(
                "predicate_key and object must be provided together",
            ));
        }
        if predicate_key
            .as_deref()
            .is_some_and(|predicate| predicate.trim().is_empty())
        {
            return Err(DomainError::InvalidClaimContext(
                "predicate_key cannot be empty",
            ));
        }
        if holder_confidence
            .is_some_and(|confidence| !confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
        {
            return Err(DomainError::InvalidConfidence);
        }
        if source_claim_id == Some(id) {
            return Err(DomainError::InvalidClaimContext(
                "a claim cannot derive from itself",
            ));
        }

        Ok(Self {
            id,
            world_id,
            subject_entity_id,
            content_md: content_md.into(),
            predicate_key: predicate_key.map(|value| value.trim().to_owned()),
            object,
            polarity,
            authentication,
            holder_entity_id,
            modality,
            register: register.map(|value| value.trim().to_owned()),
            epistemic_basis,
            source,
            source_document_id,
            source_claim_id,
            holder_confidence,
            period,
            registered_revision_id,
            superseded_revision_id,
            version,
        })
    }

    pub fn id(&self) -> ClaimId {
        self.id
    }

    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    pub fn subject_entity_id(&self) -> EntityId {
        self.subject_entity_id
    }

    pub fn content_md(&self) -> &str {
        &self.content_md
    }

    pub fn predicate_key(&self) -> Option<&str> {
        self.predicate_key.as_deref()
    }

    pub fn object(&self) -> Option<&ClaimObject> {
        self.object.as_ref()
    }

    pub fn polarity(&self) -> ClaimPolarity {
        self.polarity
    }

    pub fn authentication(&self) -> ClaimAuthentication {
        self.authentication
    }

    pub fn holder_entity_id(&self) -> Option<EntityId> {
        self.holder_entity_id
    }

    pub fn modality(&self) -> Option<ClaimModality> {
        self.modality
    }

    pub fn register(&self) -> Option<&str> {
        self.register.as_deref()
    }

    pub fn epistemic_basis(&self) -> Option<&str> {
        self.epistemic_basis.as_deref()
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub fn source_document_id(&self) -> Option<DocumentId> {
        self.source_document_id
    }

    pub fn source_claim_id(&self) -> Option<ClaimId> {
        self.source_claim_id
    }

    pub fn holder_confidence(&self) -> Option<f64> {
        self.holder_confidence
    }

    pub fn period(&self) -> Option<Period> {
        self.period
    }

    pub fn registered_revision_id(&self) -> RevisionId {
        self.registered_revision_id
    }

    pub fn superseded_revision_id(&self) -> Option<RevisionId> {
        self.superseded_revision_id
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn is_active(&self) -> bool {
        self.superseded_revision_id.is_none()
    }

    pub(crate) fn has_same_normalized_proposition(&self, other: &Self) -> bool {
        self.world_id == other.world_id
            && self.subject_entity_id == other.subject_entity_id
            && self.predicate_key.is_some()
            && self.predicate_key == other.predicate_key
            && self.object == other.object
    }
}

fn validate_context(
    authentication: ClaimAuthentication,
    holder_entity_id: Option<EntityId>,
    modality: Option<ClaimModality>,
) -> Result<(), DomainError> {
    match authentication {
        ClaimAuthentication::Canonical if holder_entity_id.is_some() || modality.is_some() => Err(
            DomainError::InvalidClaimContext("canonical claims cannot have a holder or modality"),
        ),
        ClaimAuthentication::Attributed if holder_entity_id.is_none() || modality.is_none() => Err(
            DomainError::InvalidClaimContext("attributed claims require a holder and modality"),
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "../tests/unit/claim/mod.rs"]
mod tests;
