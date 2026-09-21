use super::*;

/// Which compiled-in reader answers `[quiet.calendar]`, carrying the settings
/// only that reader takes.
///
/// ONE ENUM RATHER THAN A TYPE WORD BESIDE FOUR OPTIONAL KEYS: a command and a
/// set of Google credentials cannot both be the calendar, so the shape that
/// cannot hold both is what the parser resolves to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarSource {
    /// ARGV, NEVER A SHELL STRING, the way `[recap] summarizer` is: the words
    /// are handed to the process directly.
    Command(Vec<String>),
    Google(GoogleCalendar),
}

impl Default for CalendarSource {
    fn default() -> Self {
        CalendarSource::Command(Vec::new())
    }
}

/// `type = "google"`: which calendars are read and the credentials that reach
/// them.
///
/// THE THREE CREDENTIALS ARE SECRETS. They travel from the config into a
/// request body and nowhere else: never argv, never a child's environment,
/// never an error string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoogleCalendar {
    pub calendars: Vec<String>,
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
}

/// The one calendar every Google account has, which is what a table naming
/// none reads.
const DEFAULT_GOOGLE_CALENDARS: [&str; 1] = ["primary"];

/// The type words `type` admits, in the order a refusal lists them.
const CALENDAR_TYPES: [&str; 2] = ["command", "google"];

/// The keys only a `type = "google"` table reads.
const GOOGLE_CREDENTIALS: [&str; 3] = ["client_id", "client_secret", "refresh_token"];

/// `[quiet.calendar]`: the calendar that switches the mute on and off.
///
/// OFF UNTIL BOTH KEYS SAY OTHERWISE. The switch and the source are separate
/// because one of them is a decision and the other is a pathname: a machine
/// that has the command written and the switch off is a machine ready to try
/// it, and `enabled = true` with no source is a feature that cannot run,
/// which `armed` reads as off rather than as an error on every tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuietCalendar {
    pub enabled: bool,
    pub source: CalendarSource,
    pub poll_secs: u64,
    pub deadline_secs: u64,
}

impl Default for QuietCalendar {
    fn default() -> Self {
        QuietCalendar {
            enabled: false,
            source: CalendarSource::default(),
            poll_secs: DEFAULT_CALENDAR_POLL_SECS,
            deadline_secs: DEFAULT_CALENDAR_DEADLINE_SECS,
        }
    }
}

impl QuietCalendar {
    /// How often the poll runs, or `None` while the feature cannot run at all.
    pub fn armed(&self) -> Option<u64> {
        let readable = match &self.source {
            CalendarSource::Command(argv) => !argv.is_empty(),
            CalendarSource::Google(google) => !google.calendars.is_empty(),
        };
        (self.enabled && readable).then_some(self.poll_secs)
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

/// Every `[quiet.calendar]` key as it was written, before `type` decides which
/// of them are read at all.
#[derive(Default)]
struct CalendarKeys {
    kind: Option<String>,
    command: Option<Vec<String>>,
    calendars: Option<Vec<String>>,
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
}

const TABLE: &str = "quiet.calendar";

fn parse_quiet_calendar(value: toml::Value) -> Result<QuietCalendar, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`quiet.calendar` is not a table".to_string(),
        ));
    };
    let mut calendar = QuietCalendar::default();
    let mut written = CalendarKeys::default();
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
            "type" => written.kind = Some(text(TABLE, "type", &setting)?),
            "command" => {
                let command = strings(TABLE, "command", "a list of command words", &setting)?;
                // AN EMPTY FIRST WORD NAMES NO PROGRAM, `[recap] summarizer`'s
                // own refusal: it parses, fails to spawn, and reads to the
                // operator as a calendar that is not answering.
                if command.first().is_some_and(String::is_empty) {
                    return Err(ConfigError::Invalid(format!(
                        "`{TABLE}` key `command` starts with an empty word, so it names no program"
                    )));
                }
                written.command = Some(command);
            }
            "calendars" => {
                written.calendars = Some(strings(
                    TABLE,
                    "calendars",
                    "a list of calendar ids",
                    &setting,
                )?);
            }
            "client_id" => written.client_id = Some(text(TABLE, "client_id", &setting)?),
            "client_secret" => {
                written.client_secret = Some(text(TABLE, "client_secret", &setting)?)
            }
            "refresh_token" => {
                written.refresh_token = Some(text(TABLE, "refresh_token", &setting)?)
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
    calendar.source = resolve_source(written)?;
    Ok(calendar)
}

/// Which reader the table names, with the keys that reader does not take
/// refused by name.
///
/// `calendars` IS NOT REFUSED UNDER `type = "command"`: it ships live at its
/// default, so a command calendar carries it on every machine and refusing it
/// would refuse the file this repository generates.
fn resolve_source(written: CalendarKeys) -> Result<CalendarSource, ConfigError> {
    let kind = written
        .kind
        .clone()
        .unwrap_or_else(|| CALENDAR_TYPES[0].to_string());
    match kind.as_str() {
        "command" => {
            for (key, stated) in credentials(&written) {
                if stated.is_some() {
                    return Err(ConfigError::Invalid(format!(
                        "`{TABLE}` states `type = \"command\"` and carries `{key}`, which only \
                         `type = \"google\"` reads"
                    )));
                }
            }
            Ok(CalendarSource::Command(written.command.unwrap_or_default()))
        }
        "google" => {
            if written.command.is_some() {
                return Err(ConfigError::Invalid(format!(
                    "`{TABLE}` states `type = \"google\"` and carries `command`, which only \
                     `type = \"command\"` reads"
                )));
            }
            let mut google = GoogleCalendar {
                calendars: written
                    .calendars
                    .clone()
                    .unwrap_or_else(|| DEFAULT_GOOGLE_CALENDARS.map(str::to_string).to_vec()),
                ..GoogleCalendar::default()
            };
            for (key, stated) in credentials(&written) {
                let Some(value) = stated else {
                    return Err(ConfigError::Invalid(format!(
                        "`{TABLE}` states `type = \"google\"` and states no `{key}`, which it \
                         cannot read a calendar without"
                    )));
                };
                match key {
                    "client_id" => google.client_id = value.clone(),
                    "client_secret" => google.client_secret = value.clone(),
                    _ => google.refresh_token = value.clone(),
                }
            }
            Ok(CalendarSource::Google(google))
        }
        other => Err(ConfigError::Invalid(format!(
            "`{TABLE}` key `type` names `{other}`; the table serves {}",
            CALENDAR_TYPES.join(", ")
        ))),
    }
}

/// The three credential keys as they were written, paired with their names so
/// one walk both refuses them under `command` and requires them under
/// `google`.
fn credentials(written: &CalendarKeys) -> [(&'static str, &Option<String>); 3] {
    [
        (GOOGLE_CREDENTIALS[0], &written.client_id),
        (GOOGLE_CREDENTIALS[1], &written.client_secret),
        (GOOGLE_CREDENTIALS[2], &written.refresh_token),
    ]
}

#[cfg(test)]
mod tests;
