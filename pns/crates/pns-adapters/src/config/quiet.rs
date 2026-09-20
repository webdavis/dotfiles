use super::*;

/// `[quiet.calendar]`: the calendar that switches the mute on and off.
///
/// OFF UNTIL BOTH KEYS SAY OTHERWISE. The switch and the command are separate
/// because one of them is a decision and the other is a pathname: a machine
/// that has the command written and the switch off is a machine ready to try
/// it, and `enabled = true` with no command is a feature that cannot run,
/// which `armed` reads as off rather than as an error on every tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuietCalendar {
    pub enabled: bool,
    /// ARGV, NEVER A SHELL STRING, the way `[recap] summarizer` is: the words
    /// are handed to the process directly.
    pub command: Vec<String>,
    pub poll_secs: u64,
    pub deadline_secs: u64,
}

impl Default for QuietCalendar {
    fn default() -> Self {
        QuietCalendar {
            enabled: false,
            command: Vec::new(),
            poll_secs: DEFAULT_CALENDAR_POLL_SECS,
            deadline_secs: DEFAULT_CALENDAR_DEADLINE_SECS,
        }
    }
}

impl QuietCalendar {
    /// How often the poll runs, or `None` while the feature cannot run at all.
    pub fn armed(&self) -> Option<u64> {
        (self.enabled && !self.command.is_empty()).then_some(self.poll_secs)
    }

    pub fn deadline(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.deadline_secs)
    }
}

/// TWO MINUTES. A meeting is muted within one poll of starting, which is
/// before the first "are you there" reaches the machine, and a calendar this
/// asks thirty times an hour is a calendar nobody notices being asked.
pub(super) const DEFAULT_CALENDAR_POLL_SECS: u64 = 120;
const MIN_CALENDAR_POLL_SECS: u64 = 30;
/// HALF AN HOUR AT THE TOP. Past it a meeting can be over before the poll
/// that would have muted it, which is the feature not working rather than
/// working slowly.
const MAX_CALENDAR_POLL_SECS: u64 = 1_800;

/// TWENTY SECONDS. A calendar read over the network answers in one or two,
/// and a command that is still going at twenty is one this poll gives up on:
/// the mute it would have set is worth less than a child held open.
pub(super) const DEFAULT_CALENDAR_DEADLINE_SECS: u64 = 20;
const MIN_CALENDAR_DEADLINE_SECS: u64 = 1;
/// THIRTY SECONDS, the daemon's own child-kill bound (`CHILD_TICKS` at the
/// production tick): a deadline past it is dead config, never reached before
/// the daemon kills the job itself.
const MAX_CALENDAR_DEADLINE_SECS: u64 = 30;

/// `poll_interval`'s range, as the duration parser takes it.
fn poll_interval_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_CALENDAR_POLL_SECS)..=Duration::from_secs(MAX_CALENDAR_POLL_SECS)
}

/// `deadline`'s range, on the same terms.
fn deadline_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_CALENDAR_DEADLINE_SECS)
        ..=Duration::from_secs(MAX_CALENDAR_DEADLINE_SECS)
}

/// `[quiet]`, whose only member is the calendar table.
pub(super) fn parse_quiet(value: toml::Value) -> Result<QuietCalendar, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`quiet` is not a table".to_string()));
    };
    let mut calendar = QuietCalendar::default();
    for (key, setting) in table {
        admits_flat("quiet", &key)?;
        match key.as_str() {
            "calendar" => calendar = parse_quiet_calendar(setting)?,
            _ => return Err(unknown_key("quiet", "quiet", &key)),
        }
    }
    Ok(calendar)
}

fn parse_quiet_calendar(value: toml::Value) -> Result<QuietCalendar, ConfigError> {
    const TABLE: &str = "quiet.calendar";
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`quiet.calendar` is not a table".to_string(),
        ));
    };
    let mut calendar = QuietCalendar::default();
    for (key, setting) in table {
        admits_flat(TABLE, &key)?;
        match key.as_str() {
            "enabled" => {
                calendar.enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`{TABLE}` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "command" => {
                calendar.command = strings(TABLE, "command", "a list of command words", &setting)?;
                // AN EMPTY FIRST WORD NAMES NO PROGRAM, `[recap] summarizer`'s
                // own refusal: it parses, fails to spawn, and reads to the
                // operator as a calendar that is not answering.
                if calendar.command.first().is_some_and(String::is_empty) {
                    return Err(ConfigError::Invalid(format!(
                        "`{TABLE}` key `command` starts with an empty word, so it names no program"
                    )));
                }
            }
            "poll_interval" => {
                calendar.poll_secs =
                    nonzero_duration_key(TABLE, "poll_interval", &setting, poll_interval_range())?;
            }
            "deadline" => {
                calendar.deadline_secs =
                    nonzero_duration_key(TABLE, "deadline", &setting, deadline_range())?;
            }
            _ => return Err(unknown_key(TABLE, TABLE, &key)),
        }
    }
    Ok(calendar)
}

#[cfg(test)]
mod tests;
