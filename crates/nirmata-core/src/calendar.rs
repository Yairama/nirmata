use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, JsonSchema)]
pub struct CalendarMonth {
    name: String,
    days: u32,
}

impl<'de> Deserialize<'de> for CalendarMonth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CalendarMonthData {
            name: String,
            days: u32,
        }

        let data = CalendarMonthData::deserialize(deserializer)?;
        Self::new(data.name, data.days).map_err(de::Error::custom)
    }
}

impl CalendarMonth {
    pub fn new(name: impl Into<String>, days: u32) -> Result<Self, CalendarError> {
        let name = required_name("month", name)?;
        if days == 0 {
            return Err(CalendarError::InvalidMonthLength);
        }
        Ok(Self { name, days })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn days(&self) -> u32 {
        self.days
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, JsonSchema)]
pub struct WorldCalendar {
    name: String,
    epoch_tick: i64,
    ticks_per_day: i64,
    weekday_names: Vec<String>,
    months: Vec<CalendarMonth>,
}

impl<'de> Deserialize<'de> for WorldCalendar {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WorldCalendarData {
            name: String,
            epoch_tick: i64,
            ticks_per_day: i64,
            weekday_names: Vec<String>,
            months: Vec<CalendarMonth>,
        }

        let data = WorldCalendarData::deserialize(deserializer)?;
        Self::new(
            data.name,
            data.epoch_tick,
            data.ticks_per_day,
            data.weekday_names,
            data.months,
        )
        .map_err(de::Error::custom)
    }
}

impl WorldCalendar {
    pub fn new(
        name: impl Into<String>,
        epoch_tick: i64,
        ticks_per_day: i64,
        weekday_names: Vec<String>,
        months: Vec<CalendarMonth>,
    ) -> Result<Self, CalendarError> {
        let name = required_name("calendar", name)?;
        if ticks_per_day <= 0 {
            return Err(CalendarError::InvalidTicksPerDay);
        }
        if weekday_names.is_empty() || weekday_names.len() > 64 {
            return Err(CalendarError::InvalidWeek);
        }
        let weekday_names = weekday_names
            .into_iter()
            .map(|name| required_name("weekday", name))
            .collect::<Result<Vec<_>, _>>()?;
        if months.is_empty() || months.len() > 64 {
            return Err(CalendarError::InvalidYear);
        }
        let days_per_year = months.iter().try_fold(0_i64, |total, month| {
            total
                .checked_add(i64::from(month.days))
                .ok_or(CalendarError::OutOfRange)
        })?;
        if days_per_year <= 0 {
            return Err(CalendarError::InvalidYear);
        }
        Ok(Self {
            name,
            epoch_tick,
            ticks_per_day,
            weekday_names,
            months,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn epoch_tick(&self) -> i64 {
        self.epoch_tick
    }

    pub fn ticks_per_day(&self) -> i64 {
        self.ticks_per_day
    }

    pub fn weekday_names(&self) -> &[String] {
        &self.weekday_names
    }

    pub fn months(&self) -> &[CalendarMonth] {
        &self.months
    }

    pub fn tick_to_date(&self, tick: i64) -> Result<CalendarDate, CalendarError> {
        let relative_tick = i128::from(tick) - i128::from(self.epoch_tick);
        let ticks_per_day = i128::from(self.ticks_per_day);
        let day_offset = relative_tick.div_euclid(ticks_per_day);
        let tick_in_day = relative_tick.rem_euclid(ticks_per_day);
        let days_per_year = i128::from(self.days_per_year());
        let year = day_offset.div_euclid(days_per_year);
        let mut day_of_year = day_offset.rem_euclid(days_per_year);
        let mut month_index = 0_usize;
        for (index, month) in self.months.iter().enumerate() {
            let month_days = i128::from(month.days);
            if day_of_year < month_days {
                month_index = index;
                break;
            }
            day_of_year -= month_days;
        }
        Ok(CalendarDate {
            year: i64::try_from(year).map_err(|_| CalendarError::OutOfRange)?,
            month: u32::try_from(month_index + 1).map_err(|_| CalendarError::OutOfRange)?,
            day: u32::try_from(day_of_year + 1).map_err(|_| CalendarError::OutOfRange)?,
            tick_in_day: i64::try_from(tick_in_day).map_err(|_| CalendarError::OutOfRange)?,
            weekday_index: u32::try_from(day_offset.rem_euclid(self.weekday_names.len() as i128))
                .map_err(|_| CalendarError::OutOfRange)?,
        })
    }

    pub fn date_to_tick(&self, date: CalendarDate) -> Result<i64, CalendarError> {
        let month_index = date
            .month
            .checked_sub(1)
            .ok_or(CalendarError::InvalidDate)? as usize;
        let month = self
            .months
            .get(month_index)
            .ok_or(CalendarError::InvalidDate)?;
        if date.day == 0 || date.day > month.days {
            return Err(CalendarError::InvalidDate);
        }
        if date.tick_in_day < 0 || date.tick_in_day >= self.ticks_per_day {
            return Err(CalendarError::InvalidDate);
        }
        let prior_month_days = self.months[..month_index]
            .iter()
            .map(|month| i128::from(month.days))
            .sum::<i128>();
        let day_offset = i128::from(date.year) * i128::from(self.days_per_year())
            + prior_month_days
            + i128::from(date.day - 1);
        let tick = i128::from(self.epoch_tick)
            + day_offset * i128::from(self.ticks_per_day)
            + i128::from(date.tick_in_day);
        i64::try_from(tick).map_err(|_| CalendarError::OutOfRange)
    }

    fn days_per_year(&self) -> i64 {
        self.months.iter().map(|month| i64::from(month.days)).sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CalendarDate {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub tick_in_day: i64,
    pub weekday_index: u32,
}

impl CalendarDate {
    pub const fn new(year: i64, month: u32, day: u32, tick_in_day: i64) -> Self {
        Self {
            year,
            month,
            day,
            tick_in_day,
            weekday_index: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalendarError {
    EmptyName(&'static str),
    InvalidTicksPerDay,
    InvalidMonthLength,
    InvalidWeek,
    InvalidYear,
    InvalidDate,
    OutOfRange,
}

impl fmt::Display for CalendarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName(field) => write!(formatter, "{field} name cannot be empty"),
            Self::InvalidTicksPerDay => write!(formatter, "ticks per day must be positive"),
            Self::InvalidMonthLength => write!(formatter, "month length must be positive"),
            Self::InvalidWeek => write!(formatter, "week must contain 1 to 64 named days"),
            Self::InvalidYear => write!(formatter, "year must contain 1 to 64 valid months"),
            Self::InvalidDate => write!(formatter, "date is not valid for this calendar"),
            Self::OutOfRange => write!(formatter, "calendar conversion is outside the tick range"),
        }
    }
}

impl Error for CalendarError {}

fn required_name(field: &'static str, value: impl Into<String>) -> Result<String, CalendarError> {
    let value = value.into();
    let value = value.trim();
    if value.is_empty() {
        return Err(CalendarError::EmptyName(field));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calendar() -> WorldCalendar {
        WorldCalendar::new(
            "Imperial",
            100,
            10,
            vec!["First".to_owned(), "Second".to_owned(), "Third".to_owned()],
            vec![
                CalendarMonth::new("Ash", 2).expect("month"),
                CalendarMonth::new("Rain", 3).expect("month"),
            ],
        )
        .expect("calendar")
    }

    #[test]
    fn converts_epoch_day_month_year_and_negative_ticks() {
        let calendar = calendar();
        assert_eq!(
            calendar.tick_to_date(100).expect("epoch"),
            CalendarDate::new(0, 1, 1, 0)
        );
        assert_eq!(
            calendar.tick_to_date(119).expect("day"),
            CalendarDate {
                year: 0,
                month: 1,
                day: 2,
                tick_in_day: 9,
                weekday_index: 1,
            }
        );
        assert_eq!(
            calendar.tick_to_date(120).expect("month"),
            CalendarDate {
                year: 0,
                month: 2,
                day: 1,
                tick_in_day: 0,
                weekday_index: 2,
            }
        );
        assert_eq!(
            calendar.tick_to_date(150).expect("year"),
            CalendarDate {
                year: 1,
                month: 1,
                day: 1,
                tick_in_day: 0,
                weekday_index: 2,
            }
        );
        assert_eq!(
            calendar.tick_to_date(99).expect("negative"),
            CalendarDate {
                year: -1,
                month: 2,
                day: 3,
                tick_in_day: 9,
                weekday_index: 2,
            }
        );
    }

    #[test]
    fn exact_dates_round_trip_across_the_epoch() {
        let calendar = calendar();
        for tick in -250..=450 {
            let date = calendar.tick_to_date(tick).expect("date");
            assert_eq!(calendar.date_to_tick(date).expect("tick"), tick);
        }
    }

    #[test]
    fn renaming_display_labels_does_not_change_tick_round_trips() {
        let original = calendar();
        let renamed = WorldCalendar::new(
            "Calendario revisado",
            100,
            10,
            vec!["Alba".to_owned(), "Cenit".to_owned(), "Ocaso".to_owned()],
            vec![
                CalendarMonth::new("Brasa", 2).expect("month"),
                CalendarMonth::new("Lluvia", 3).expect("month"),
            ],
        )
        .expect("renamed calendar");

        for tick in -250..=450 {
            let original_date = original.tick_to_date(tick).expect("original date");
            let renamed_date = renamed.tick_to_date(tick).expect("renamed date");
            assert_eq!(original_date, renamed_date);
            assert_eq!(renamed.date_to_tick(renamed_date).expect("tick"), tick);
        }
        assert_ne!(original.months()[1].name(), renamed.months()[1].name());
        assert_ne!(original.weekday_names()[0], renamed.weekday_names()[0]);
    }

    #[test]
    fn rejects_invalid_configuration_dates_and_overflow() {
        assert!(WorldCalendar::new("", 0, 1, vec!["Day".to_owned()], vec![]).is_err());
        assert!(
            WorldCalendar::new(
                "Calendar",
                0,
                0,
                vec!["Day".to_owned()],
                vec![CalendarMonth::new("Month", 1).expect("month"),]
            )
            .is_err()
        );
        assert!(CalendarMonth::new("Month", 0).is_err());
        let calendar = calendar();
        assert!(
            calendar
                .date_to_tick(CalendarDate::new(0, 2, 4, 0))
                .is_err()
        );
        assert!(
            calendar
                .date_to_tick(CalendarDate::new(0, 1, 1, 10))
                .is_err()
        );
        assert!(
            calendar
                .date_to_tick(CalendarDate::new(i64::MAX, 1, 1, 0))
                .is_err()
        );
    }

    #[test]
    fn deserializes_calendars_through_validated_constructors() {
        let calendar: WorldCalendar = serde_json::from_str(
            r#"{
                "name": " Imperial ",
                "epoch_tick": 100,
                "ticks_per_day": 10,
                "weekday_names": [" First ", "Second"],
                "months": [{"name": " Ash ", "days": 2}]
            }"#,
        )
        .expect("valid calendar");

        assert_eq!(calendar.name(), "Imperial");
        assert_eq!(calendar.weekday_names(), ["First", "Second"]);
        assert_eq!(calendar.months()[0].name(), "Ash");
    }

    #[test]
    fn calendar_month_deserialization_rejects_invalid_and_unknown_fields() {
        for json in [
            r#"{"name":"   ","days":1}"#,
            r#"{"name":"Ash","days":0}"#,
            r#"{"name":"Ash","days":1,"unknown":true}"#,
        ] {
            assert!(
                serde_json::from_str::<CalendarMonth>(json).is_err(),
                "accepted invalid month: {json}"
            );
        }
    }

    #[test]
    fn world_calendar_deserialization_rejects_invalid_and_unknown_fields() {
        for json in [
            r#"{"name":" ","epoch_tick":0,"ticks_per_day":1,"weekday_names":["Day"],"months":[{"name":"Month","days":1}]}"#,
            r#"{"name":"Calendar","epoch_tick":0,"ticks_per_day":0,"weekday_names":["Day"],"months":[{"name":"Month","days":1}]}"#,
            r#"{"name":"Calendar","epoch_tick":0,"ticks_per_day":-1,"weekday_names":["Day"],"months":[{"name":"Month","days":1}]}"#,
            r#"{"name":"Calendar","epoch_tick":0,"ticks_per_day":1,"weekday_names":[],"months":[{"name":"Month","days":1}]}"#,
            r#"{"name":"Calendar","epoch_tick":0,"ticks_per_day":1,"weekday_names":[" "],"months":[{"name":"Month","days":1}]}"#,
            r#"{"name":"Calendar","epoch_tick":0,"ticks_per_day":1,"weekday_names":["Day"],"months":[]}"#,
            r#"{"name":"Calendar","epoch_tick":0,"ticks_per_day":1,"weekday_names":["Day"],"months":[{"name":"Month","days":1}],"unknown":true}"#,
        ] {
            assert!(
                serde_json::from_str::<WorldCalendar>(json).is_err(),
                "accepted invalid calendar: {json}"
            );
        }
    }
}
