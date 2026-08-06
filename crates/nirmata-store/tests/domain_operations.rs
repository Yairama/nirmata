use nirmata_core::{
    Period, RevisionId, World,
    change_set::{ChangeOperation, ChangeSet, ChangeSetDraft, DecisionPoint, RetconKind},
    claim::{Claim, ClaimAuthentication, ClaimModality, ClaimObject, ClaimPolarity},
    document::{ContentReference, Document, DocumentCanonStatus, ObjectRef},
    entity::{Entity, EntityKind},
    event::{Event, EventLink, EventLinkKind, EventParticipant},
    goal::{Goal, GoalStatus, GoalVisibility},
    relation::{Relation, RelationDirection},
    rule::{Rule, RuleKind, RuleSeverity, RuleValidatorKind},
    time::{Certainty, EventTime, TimePrecision},
};
use nirmata_store::{
    AnchorContextQuery, ChangeSetDraftRecord, CommittedChangeSetRecord, DocumentAggregate,
    EventAggregate, OperationAudit, OperationDecision, ResolvedObject, StoreError, StoredRevision,
    StructuredSearchHit, StructuredSearchKind, StructuredSearchQuery, StructuredSearchStage,
    StructuredSearchTemporal, WorldStore,
};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn project_path(label: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/nirmata-tests");
    fs::create_dir_all(&directory).expect("create test directory");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    directory.join(format!("{label}-{}-{nonce}.nirmata", std::process::id()))
}

fn create_entity_change_operation(world: &World, now_ms: i64) -> (Entity, ChangeOperation) {
    let entity = Entity::new(
        world.id(),
        EntityKind::Person,
        "Mara",
        "mara",
        "Cartographer",
        "",
        "{}",
        vec![],
        now_ms,
    )
    .expect("entity");
    let operation = ChangeOperation::CreateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: 0,
        retcon: RetconKind::Additive,
        after: entity.clone(),
    };
    (entity, operation)
}

fn renamed_entity(entity: &Entity, name: &str, slug: &str, now_ms: i64) -> Entity {
    Entity::restore(
        entity.id(),
        entity.world_id(),
        entity.kind(),
        name,
        slug,
        entity.summary().to_owned(),
        entity.body_md().to_owned(),
        entity.attributes_json().as_str().to_owned(),
        entity.aliases().to_vec(),
        entity.version() + 1,
        entity.created_at_ms(),
        now_ms,
    )
    .expect("renamed entity")
}

fn update_entity_operation(before: &Entity, after: &Entity) -> ChangeOperation {
    ChangeOperation::UpdateEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(before.id())],
        expected_version: before.version(),
        retcon: RetconKind::Additive,
        before: before.clone(),
        after: after.clone(),
    }
}

fn create_claim_operation(claim: &Claim, retcon: RetconKind) -> ChangeOperation {
    let mut affected_ids = vec![
        ObjectRef::Claim(claim.id()),
        ObjectRef::Entity(claim.subject_entity_id()),
    ];
    if let Some(holder_id) = claim.holder_entity_id() {
        affected_ids.push(ObjectRef::Entity(holder_id));
    }
    if let Some(ClaimObject::Entity(entity_id)) = claim.object() {
        affected_ids.push(ObjectRef::Entity(*entity_id));
    }
    if let Some(document_id) = claim.source_document_id() {
        affected_ids.push(ObjectRef::Document(document_id));
    }
    if let Some(source_claim_id) = claim.source_claim_id() {
        affected_ids.push(ObjectRef::Claim(source_claim_id));
    }
    ChangeOperation::CreateClaim {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids,
        expected_version: 0,
        retcon,
        after: claim.clone(),
    }
}

fn delete_entity_operation(entity: &Entity, retcon: RetconKind) -> ChangeOperation {
    ChangeOperation::DeleteEntity {
        operation_id: nirmata_core::ChangeOperationId::new(),
        affected_ids: vec![ObjectRef::Entity(entity.id())],
        expected_version: entity.version(),
        retcon,
        before: entity.clone(),
    }
}

fn canonical_claim(
    world: &World,
    subject_entity_id: nirmata_core::EntityId,
    predicate_key: &str,
    object: ClaimObject,
    polarity: ClaimPolarity,
    period: Period,
) -> Claim {
    Claim::new(
        world.id(),
        subject_entity_id,
        "canonical claim",
        Some(predicate_key.to_owned()),
        Some(object),
        polarity,
        ClaimAuthentication::Canonical,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(period),
        world.current_revision(),
    )
    .expect("canonical claim")
}

fn attributed_claim(
    world: &World,
    subject_entity_id: nirmata_core::EntityId,
    holder_entity_id: nirmata_core::EntityId,
    register: &str,
    polarity: ClaimPolarity,
) -> Claim {
    Claim::new(
        world.id(),
        subject_entity_id,
        "attributed claim",
        Some("gate.open".to_owned()),
        Some(ClaimObject::Scalar("true".to_owned())),
        polarity,
        ClaimAuthentication::Attributed,
        Some(holder_entity_id),
        Some(ClaimModality::Belief),
        Some(register.to_owned()),
        None,
        None,
        None,
        None,
        Some(0.6),
        Some(Period::new(Some(10), Some(10)).expect("period")),
        world.current_revision(),
    )
    .expect("attributed claim")
}

include!("domain_operations/canon.rs");
include!("domain_operations/change_sets.rs");
include!("domain_operations/search.rs");
include!("domain_operations/context.rs");
