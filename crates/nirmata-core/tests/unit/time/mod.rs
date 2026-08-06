
use super::*;

fn interval(start: i64, end: i64) -> EventTime {
    EventTime::interval(start, end, TimePrecision::Exact, Certainty::Certain)
        .expect("valid interval")
}

#[test]
fn compares_only_when_position_is_known() {
    let early = interval(1, 3);
    let middle = interval(3, 5);
    let late = interval(6, 8);
    let inner = interval(2, 3);
    let unknown = EventTime::unknown(Certainty::Uncertain);

    assert_eq!(early.before(&middle), PartialTruth::False);
    assert_eq!(middle.after(&early), PartialTruth::False);
    assert_eq!(early.before(&late), PartialTruth::True);
    assert_eq!(late.after(&early), PartialTruth::True);
    assert_eq!(early.overlaps(&middle), PartialTruth::True);
    assert_eq!(early.overlaps(&late), PartialTruth::False);
    assert_eq!(inner.during(&early), PartialTruth::True);
    assert_eq!(early.contains(&inner), PartialTruth::True);
    assert_eq!(early.equals(&interval(1, 3)), PartialTruth::True);
    assert_eq!(early.equals(&middle), PartialTruth::False);
    assert_eq!(unknown.before(&early), PartialTruth::Unspecified);
}

#[test]
fn validates_kinds_and_open_ended_time() {
    assert_eq!(
        EventTime::interval(5, 4, TimePrecision::Exact, Certainty::Certain),
        Err(DomainError::InvalidEventTime)
    );

    let ongoing = EventTime::ongoing(5, TimePrecision::Year, Certainty::Approximate);
    assert_eq!(ongoing.end_tick(), None);
    assert_eq!(ongoing.overlaps(&interval(10, 20)), PartialTruth::True);
    assert_eq!(
        EventTime::new(
            EventTimeKind::Unknown,
            Some(1),
            None,
            TimePrecision::Unknown,
            Certainty::Uncertain
        ),
        Err(DomainError::InvalidEventTime)
    );
}
