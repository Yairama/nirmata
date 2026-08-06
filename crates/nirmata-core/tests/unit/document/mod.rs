
use super::*;
use crate::time::{Certainty, EventTime, TimePrecision};

#[test]
fn parses_stable_uris_and_orders_mentions() {
    let document = DocumentId::new();
    let event = EventId::new();
    let uri = ObjectRef::Event(event).to_string();
    assert_eq!(ObjectRef::from_str(&uri), Ok(ObjectRef::Event(event)));
    assert!(ObjectRef::from_str("file://event/anything").is_err());

    let source = ObjectRef::Document(document);
    let references = vec![
        ContentReference::new(source, ObjectRef::Event(EventId::new()), 2),
        ContentReference::new(source, ObjectRef::Event(event), 0),
    ];
    let ordered = ordered_content_references(source, &references);
    assert_eq!(ordered[0].target(), ObjectRef::Event(event));
}

#[test]
fn discourse_order_does_not_change_event_time() {
    let time = EventTime::instant(10, TimePrecision::Year, Certainty::Certain);
    let source = ObjectRef::Document(DocumentId::new());
    let event = ObjectRef::Event(EventId::new());
    let first = ContentReference::new(source, event, 5);
    let flashback = ContentReference::new(source, event, 1);

    assert_ne!(first.ordinal(), flashback.ordinal());
    assert_eq!(time.start_tick(), Some(10));
}
