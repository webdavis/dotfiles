//! `[schedule]`: which day and time `uu schedule render` writes into the job.
//!
//! TWO TRUTHS, and this is the standalone one. A machine whose scheduler is
//! managed elsewhere (this repo tracks a launchd plist of its own) takes its
//! timing from there, and this block feeds only the rendered plist.

use super::ConfigError;
use super::schema::{admits, non_empty, table_of};

/// When `uu schedule render` says the job should run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// launchd's own `Weekday` numbering: Sunday is 0.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            weekday: DEFAULT_WEEKDAY,
            hour: DEFAULT_HOUR,
            minute: DEFAULT_MINUTE,
        }
    }
}

/// Sunday noon, the shipped schedule.
const DEFAULT_WEEKDAY: u8 = 0;
const DEFAULT_HOUR: u8 = 12;
const DEFAULT_MINUTE: u8 = 0;

/// The day names a config writes, in launchd's own numbering.
pub const WEEKDAY_NAMES: [(&str, u8); 7] = [
    ("sunday", 0),
    ("monday", 1),
    ("tuesday", 2),
    ("wednesday", 3),
    ("thursday", 4),
    ("friday", 5),
    ("saturday", 6),
];

pub(super) fn parse_schedule(value: toml::Value) -> Result<Schedule, ConfigError> {
    let table = table_of("schedule", value)?;
    let mut schedule = Schedule::default();
    for (key, setting) in table {
        admits("schedule", "schedule", &key)?;
        match key.as_str() {
            "day" => schedule.weekday = weekday(&non_empty("schedule", &key, &setting)?)?,
            "time" => {
                let (hour, minute) = time_of_day(&non_empty("schedule", &key, &setting)?)?;
                schedule.hour = hour;
                schedule.minute = minute;
            }
            // `admits` above is the ONE gate; nothing reaches here.
            _ => {}
        }
    }
    Ok(schedule)
}

/// `day`, refused BY NAME outside the seven, because a day nothing matches is
/// a schedule the operator wrote and no plist can carry.
fn weekday(name: &str) -> Result<u8, ConfigError> {
    WEEKDAY_NAMES
        .iter()
        .find(|(spelling, _)| *spelling == name)
        .map(|(_, number)| *number)
        .ok_or_else(|| {
            let known: Vec<&str> = WEEKDAY_NAMES.iter().map(|(word, _)| *word).collect();
            ConfigError::Invalid(format!(
                "`schedule` key `day` is `{name}`, which is no day; it serves {}",
                known.join(", ")
            ))
        })
}

/// `time`, as `HH:MM` on a 24-hour clock.
///
/// THE SHAPE IS JUDGED RATHER THAN COERCED. `12` and `12:00:00` and `24:00`
/// each parse as something under a lenient reading, and each would render a
/// plist that runs at an hour nobody asked for.
fn time_of_day(stated: &str) -> Result<(u8, u8), ConfigError> {
    let refusal = || {
        ConfigError::Invalid(format!(
            "`schedule` key `time` is `{stated}`, which is not a 24-hour `HH:MM` time"
        ))
    };
    let (hour, minute) = stated.split_once(':').ok_or_else(refusal)?;
    if hour.len() != 2 || minute.len() != 2 {
        return Err(refusal());
    }
    // DIGITS ONLY. `u8::parse` on its own admits a leading `+`, so `+1:23`
    // would coerce to 01:23, an hour nobody wrote.
    if !hour
        .bytes()
        .chain(minute.bytes())
        .all(|byte| byte.is_ascii_digit())
    {
        return Err(refusal());
    }
    let hour: u8 = hour.parse().map_err(|_| refusal())?;
    let minute: u8 = minute.parse().map_err(|_| refusal())?;
    if hour > 23 || minute > 59 {
        return Err(refusal());
    }
    Ok((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::{parsed, refusal};

    #[test]
    fn the_shipped_schedule_is_sunday_at_noon() {
        assert_eq!(
            parsed("").schedule,
            Schedule {
                weekday: 0,
                hour: 12,
                minute: 0
            }
        );
    }

    #[test]
    fn each_day_name_maps_to_launchds_own_numbering_with_sunday_at_zero() {
        // THE PAIRS ARE WRITTEN OUT rather than read back off `WEEKDAY_NAMES`.
        // Walking the table to check the table is a tautology: it stays green
        // while every day renders a plist that fires on the wrong one.
        for (name, number) in [
            ("sunday", 0),
            ("monday", 1),
            ("tuesday", 2),
            ("wednesday", 3),
            ("thursday", 4),
            ("friday", 5),
            ("saturday", 6),
        ] {
            let config = parsed(&format!("[schedule]\nday = \"{name}\"\n"));
            assert_eq!(config.schedule.weekday, number, "case: {name}");
        }
        assert_eq!(WEEKDAY_NAMES.len(), 7, "a week has seven days");
    }

    #[test]
    fn a_day_that_is_no_day_is_refused_and_the_seven_are_listed() {
        // The near-misses cover each lenient matching a rewrite could reach
        // for: a typo, a capitalized day, a prefix, and a trailing stutter.
        // Every one of them names a day the operator can see, so accepting any
        // of them silently is a schedule they never wrote.
        for stated in ["sundae", "Sunday", "sun", "sundayy"] {
            let detail = refusal(&format!("[schedule]\nday = \"{stated}\"\n"));
            assert!(detail.contains(&format!("`{stated}`")), "{detail}");
            assert!(detail.contains("sunday, monday"), "{detail}");
            assert!(detail.contains("saturday"), "{detail}");
        }
    }

    #[test]
    fn a_time_is_read_as_hours_and_minutes_on_a_twenty_four_hour_clock() {
        let config = parsed("[schedule]\ntime = \"23:45\"\n");
        assert_eq!(config.schedule.hour, 23);
        assert_eq!(config.schedule.minute, 45);
        let midnight = parsed("[schedule]\ntime = \"00:00\"\n");
        assert_eq!((midnight.schedule.hour, midnight.schedule.minute), (0, 0));
    }

    #[test]
    fn a_time_that_is_not_hh_colon_mm_is_refused_rather_than_coerced() {
        // Each of these parses as SOMETHING under a lenient reading, and each
        // would render a plist that fires at an hour nobody asked for.
        for stated in [
            "12", "12:00:00", "24:00", "12:60", "9:00", "noon", "-1:00", "+1:23", "12:+5",
        ] {
            let detail = refusal(&format!("[schedule]\ntime = \"{stated}\"\n"));
            assert!(
                detail.contains("not a 24-hour `HH:MM` time"),
                "{stated}: {detail}"
            );
        }
    }
}
