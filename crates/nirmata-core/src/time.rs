use crate::DomainError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTimeKind {
    Unknown,
    Instant,
    Interval,
    Ongoing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimePrecision {
    Exact,
    Day,
    Month,
    Year,
    Era,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    Certain,
    Approximate,
    Uncertain,
    ApproximateUncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartialTruth {
    True,
    False,
    Unspecified,
}

impl From<bool> for PartialTruth {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventTime {
    kind: EventTimeKind,
    start_tick: Option<i64>,
    end_tick: Option<i64>,
    precision: TimePrecision,
    certainty: Certainty,
}

impl EventTime {
    pub fn new(
        kind: EventTimeKind,
        start_tick: Option<i64>,
        end_tick: Option<i64>,
        precision: TimePrecision,
        certainty: Certainty,
    ) -> Result<Self, DomainError> {
        let time = Self {
            kind,
            start_tick,
            end_tick,
            precision,
            certainty,
        };
        if time.validate().is_err() {
            return Err(DomainError::InvalidEventTime);
        }
        Ok(time)
    }

    pub fn unknown(certainty: Certainty) -> Self {
        Self {
            kind: EventTimeKind::Unknown,
            start_tick: None,
            end_tick: None,
            precision: TimePrecision::Unknown,
            certainty,
        }
    }

    pub fn instant(tick: i64, precision: TimePrecision, certainty: Certainty) -> Self {
        Self {
            kind: EventTimeKind::Instant,
            start_tick: Some(tick),
            end_tick: None,
            precision,
            certainty,
        }
    }

    pub fn interval(
        start_tick: i64,
        end_tick: i64,
        precision: TimePrecision,
        certainty: Certainty,
    ) -> Result<Self, DomainError> {
        Self::new(
            EventTimeKind::Interval,
            Some(start_tick),
            Some(end_tick),
            precision,
            certainty,
        )
    }

    pub fn ongoing(start_tick: i64, precision: TimePrecision, certainty: Certainty) -> Self {
        Self {
            kind: EventTimeKind::Ongoing,
            start_tick: Some(start_tick),
            end_tick: None,
            precision,
            certainty,
        }
    }

    pub fn kind(&self) -> EventTimeKind {
        self.kind
    }

    pub fn start_tick(&self) -> Option<i64> {
        self.start_tick
    }

    pub fn end_tick(&self) -> Option<i64> {
        self.end_tick
    }

    pub fn precision(&self) -> TimePrecision {
        self.precision
    }

    pub fn certainty(&self) -> Certainty {
        self.certainty
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        let valid = match self.kind {
            EventTimeKind::Unknown => self.start_tick.is_none() && self.end_tick.is_none(),
            EventTimeKind::Instant => self.start_tick.is_some() && self.end_tick.is_none(),
            EventTimeKind::Interval => {
                matches!(
                    (self.start_tick, self.end_tick),
                    (Some(start), Some(end)) if start <= end
                )
            }
            EventTimeKind::Ongoing => self.start_tick.is_some() && self.end_tick.is_none(),
        };
        if valid {
            Ok(())
        } else {
            Err(DomainError::InvalidEventTime)
        }
    }

    pub fn before(&self, other: &Self) -> PartialTruth {
        match (self.closed_end(), other.start_tick) {
            (Some(end), Some(start)) => (end < start).into(),
            _ => PartialTruth::Unspecified,
        }
    }

    pub fn after(&self, other: &Self) -> PartialTruth {
        other.before(self)
    }

    pub fn overlaps(&self, other: &Self) -> PartialTruth {
        let (Some(self_start), Some(other_start)) = (self.start_tick, other.start_tick) else {
            return PartialTruth::Unspecified;
        };

        if self.closed_end().is_some_and(|end| end < other_start)
            || other.closed_end().is_some_and(|end| end < self_start)
        {
            PartialTruth::False
        } else {
            PartialTruth::True
        }
    }

    pub fn during(&self, other: &Self) -> PartialTruth {
        let (Some(self_start), Some(other_start)) = (self.start_tick, other.start_tick) else {
            return PartialTruth::Unspecified;
        };
        if self_start < other_start {
            return PartialTruth::False;
        }

        match (self.closed_end(), other.closed_end()) {
            (Some(self_end), Some(other_end)) => (self_end <= other_end).into(),
            (Some(_), None) | (None, None) => PartialTruth::True,
            (None, Some(_)) => PartialTruth::False,
        }
    }

    pub fn contains(&self, other: &Self) -> PartialTruth {
        other.during(self)
    }

    pub fn equals(&self, other: &Self) -> PartialTruth {
        match (self.start_tick, other.start_tick) {
            (Some(self_start), Some(other_start)) => {
                (self_start == other_start && self.closed_end() == other.closed_end()).into()
            }
            _ => PartialTruth::Unspecified,
        }
    }

    fn closed_end(&self) -> Option<i64> {
        match self.kind {
            EventTimeKind::Instant => self.start_tick,
            EventTimeKind::Interval => self.end_tick,
            EventTimeKind::Ongoing | EventTimeKind::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
