
use super::*;
use crate::time::{Certainty, TimePrecision};

#[test]
fn rejects_duplicate_participant_ordinals_and_self_causality() {
    let entity = EntityId::new();
    let participants = vec![
        EventParticipant::new(entity, "actor", 0).expect("participant"),
        EventParticipant::new(entity, "witness", 0).expect("participant"),
    ];
    let event = Event::new(
        WorldId::new(),
        "arrival",
        "",
        "",
        EventTime::instant(1, TimePrecision::Exact, Certainty::Certain),
        None,
        participants,
        vec![],
        1,
    );
    assert_eq!(event, Err(DomainError::DuplicateOrdinal(0)));

    let event_id = EventId::new();
    assert_eq!(
        EventLink::new(event_id, event_id, EventLinkKind::Causes),
        Err(DomainError::DuplicateReference)
    );
}
