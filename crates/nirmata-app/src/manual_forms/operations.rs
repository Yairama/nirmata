pub(crate) fn preview_manual_draft(
    store: &WorldStore,
    session: &WorldSession,
    world: &World,
    request: ManualDraftRequest,
    now_ms: i64,
) -> Result<PreviewManualDraftOutcome, AppError> {
    Builder::new(store, session, world, request, now_ms).build()
}

pub(crate) fn prepare_manual_operation(
    store: &WorldStore,
    session: &WorldSession,
    world: &World,
    request: ManualDraftRequest,
    now_ms: i64,
) -> Result<PreparedManualOperationOutcome, AppError> {
    let mut builder = Builder::new(store, session, world, request, now_ms);
    builder.prepare()
}

pub(crate) fn manual_request_for_review_operation(
    review: &ManualReviewSession,
    operation_id: ChangeOperationId,
) -> Result<ManualDraftRequest, AppError> {
    let operation = review
        .operations()
        .iter()
        .find(|operation| operation.operation_id() == operation_id)
        .ok_or(AppError::UnknownReviewOperation(operation_id))?;
    Ok(operation_request(review, operation.current()))
}

fn operation_request(
    review: &ManualReviewSession,
    operation: &ChangeOperation,
) -> ManualDraftRequest {
    let (object_type, existing_uri, values) = match operation {
        ChangeOperation::UpdateWorld { after, .. } => (
            "world".to_owned(),
            Some(ObjectRef::World(after.id()).to_string()),
            world_values(after),
        ),
        ChangeOperation::CreateEntity { after, .. } => {
            ("entity".to_owned(), None, entity_values(after))
        }
        ChangeOperation::UpdateEntity { after, .. } => (
            "entity".to_owned(),
            Some(ObjectRef::Entity(after.id()).to_string()),
            entity_values(after),
        ),
        ChangeOperation::DeleteEntity { before, .. } => (
            "entity".to_owned(),
            Some(ObjectRef::Entity(before.id()).to_string()),
            entity_values(before),
        ),
        ChangeOperation::CreateRelation { after, .. } => {
            ("relation".to_owned(), None, relation_values(after))
        }
        ChangeOperation::UpdateRelation { after, .. } => (
            "relation".to_owned(),
            Some(ObjectRef::Relation(after.id()).to_string()),
            relation_values(after),
        ),
        ChangeOperation::DeleteRelation { before, .. } => (
            "relation".to_owned(),
            Some(ObjectRef::Relation(before.id()).to_string()),
            relation_values(before),
        ),
        ChangeOperation::CreateEvent { after, .. } => {
            ("event".to_owned(), None, event_values(after))
        }
        ChangeOperation::UpdateEvent { after, .. } => (
            "event".to_owned(),
            Some(ObjectRef::Event(after.event().id()).to_string()),
            event_values(after),
        ),
        ChangeOperation::DeleteEvent { before, .. } => (
            "event".to_owned(),
            Some(ObjectRef::Event(before.event().id()).to_string()),
            event_values(before),
        ),
        ChangeOperation::CreateGoal { after, .. } => ("goal".to_owned(), None, goal_values(after)),
        ChangeOperation::UpdateGoal { after, .. } => (
            "goal".to_owned(),
            Some(ObjectRef::Goal(after.id()).to_string()),
            goal_values(after),
        ),
        ChangeOperation::DeleteGoal { before, .. } => (
            "goal".to_owned(),
            Some(ObjectRef::Goal(before.id()).to_string()),
            goal_values(before),
        ),
        ChangeOperation::CreateRule { after, .. } => ("rule".to_owned(), None, rule_values(after)),
        ChangeOperation::UpdateRule { after, .. } => (
            "rule".to_owned(),
            Some(ObjectRef::Rule(after.id()).to_string()),
            rule_values(after),
        ),
        ChangeOperation::DeleteRule { before, .. } => (
            "rule".to_owned(),
            Some(ObjectRef::Rule(before.id()).to_string()),
            rule_values(before),
        ),
        ChangeOperation::CreateClaim { after, .. } => {
            ("claim".to_owned(), None, claim_values(after))
        }
        ChangeOperation::UpdateClaim { after, .. } => (
            "claim".to_owned(),
            Some(ObjectRef::Claim(after.id()).to_string()),
            claim_values(after),
        ),
        ChangeOperation::DeleteClaim { before, .. } => (
            "claim".to_owned(),
            Some(ObjectRef::Claim(before.id()).to_string()),
            claim_values(before),
        ),
        ChangeOperation::CreateDocument { after, .. } => {
            ("document".to_owned(), None, document_values(after))
        }
        ChangeOperation::UpdateDocument { after, .. } => (
            "document".to_owned(),
            Some(ObjectRef::Document(after.object().id()).to_string()),
            document_values(after),
        ),
        ChangeOperation::DeleteDocument { before, .. } => (
            "document".to_owned(),
            Some(ObjectRef::Document(before.object().id()).to_string()),
            document_values(before),
        ),
    };

    ManualDraftRequest {
        object_type,
        existing_uri,
        objective: Some(review.draft().objective().to_owned()),
        source_uris: review
            .draft()
            .sources()
            .iter()
            .map(ToString::to_string)
            .collect(),
        assumptions: review.draft().assumptions().to_vec(),
        values,
    }
}
