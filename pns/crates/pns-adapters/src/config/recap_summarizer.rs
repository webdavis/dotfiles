use super::*;
use pns_domain::recap::summarizer::{Kind, Settings, WORDS};

const TABLE: &str = "recap.summarizer";

/// `[recap.summarizer]`: which model writes the summary, and what it is
/// allowed to spend doing it.
///
/// THE HARNESS IS NAMED BY WORD, not by its flags. `type = "claude"` states
/// which tool, and pns owns the invocation, so a flag that moves is a pns
/// release rather than every operator's config edit. `custom` is the escape
/// hatch and carries the argument vector itself.
///
/// THE TWO WAYS OF NAMING AN INVOCATION CANNOT BOTH BE USED. A known type with
/// a `command` beside it is two answers to one question, and pns would have to
/// pick one silently, so it is refused by name instead.
///
/// THE SHIPPED DEFAULT IS NO SUMMARIZER. `type = "custom"` with an empty
/// `command` is what a fresh install carries, and it means the recap writes no
/// summary at all, the way the absent key meant it before this table existed.
pub(super) fn parse_recap_summarizer(value: toml::Value) -> Result<Settings, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(format!("`{TABLE}` is not a table")));
    };
    let mut settings = Settings::default();
    for (key, setting) in table {
        admits_flat(TABLE, &key)?;
        match key.as_str() {
            "type" => settings.kind = kind(&setting)?,
            "command" => {
                settings.command = strings(TABLE, &key, "a list of command words", &setting)?
            }
            "model" => settings.model = text(TABLE, &key, &setting)?,
            "deadline" => {
                settings.deadline =
                    duration_value(TABLE, "deadline", &setting, summarizer_deadline_range())?;
            }
            "transcripts" => settings.transcripts = switch(&key, &setting)?,
            "transcript_bytes_per_session" => {
                settings.transcript_bytes_per_session = bytes(&key, &setting)?;
            }
            "transcript_bytes_total" => settings.transcript_bytes_total = bytes(&key, &setting)?,
            "prompt" => settings.prompt = text(TABLE, &key, &setting)?,
            "prompt_file" => settings.prompt_file = text(TABLE, &key, &setting)?,
            _ => return Err(unknown_key(TABLE, TABLE, &key)),
        }
    }
    agrees(&settings)?;
    Ok(settings)
}

/// The three ways one table can contradict itself, each refused by name.
fn agrees(settings: &Settings) -> Result<(), ConfigError> {
    let stated = settings.kind.word();
    if settings.kind != Kind::Custom && !settings.command.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` names `type = \"{stated}\"` and a `command`; \
             a `command` belongs to `type = \"custom\"` alone"
        )));
    }
    // AN EMPTY FIRST WORD NAMES NO COMMAND, which `Command::new(\"\")` then
    // fails to spawn: the operator would read a summarizer that is not
    // answering rather than the table they have to fix.
    if settings.command.first().is_some_and(String::is_empty) {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` key `command` starts with an empty word, so it names no command to run"
        )));
    }
    // OLLAMA NAMES ITS OWN MODEL AND HAS NO DEFAULT, so a table that leaves
    // `model` out composes `ollama run` with nothing to run.
    if settings.kind == Kind::Ollama && settings.model.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` names `type = \"ollama\"` with no `model`, so it names no model to run"
        )));
    }
    if !settings.prompt.is_empty() && !settings.prompt_file.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` sets both `prompt` and `prompt_file`; one instruction replaces the other, \
             so only one may be stated"
        )));
    }
    Ok(())
}

/// `type`, one of the five words and nothing else.
fn kind(setting: &toml::Value) -> Result<Kind, ConfigError> {
    let stated = text(TABLE, "type", setting)?;
    Kind::of(&stated).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{TABLE}` key `type` is `{stated}`, which is no summarizer; it takes {}",
            WORDS
                .iter()
                .map(|kind| kind.word())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })
}

/// One of the table's own switches, refused BY NAME and BY TYPE.
fn switch(key: &str, setting: &toml::Value) -> Result<bool, ConfigError> {
    setting.as_bool().ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{TABLE}` key `{key}` has type `{}`, not boolean",
            setting.type_str()
        ))
    })
}

/// One of the two transcript ceilings. ZERO IS ACCEPTED and means no transcript
/// of that scope is appended, which is a bound rather than a trap.
fn bytes(key: &str, setting: &toml::Value) -> Result<usize, ConfigError> {
    usize::try_from(bounded(TABLE, key, setting, 0, MAX_TRANSCRIPT_BYTES)?)
        .map_err(|_| ConfigError::Invalid(format!("`{TABLE}` key `{key}` is out of range")))
}

/// The most either transcript ceiling may be: four mebibytes, which is past
/// any context a summarizer takes and still a bound on what pns reads into
/// memory.
const MAX_TRANSCRIPT_BYTES: u64 = 4 * 1024 * 1024;

/// The most any summarizer may be given. ONE HOUR, which is fifteen times the
/// default, so no honest backend on any machine meets it.
///
/// THE FLOOR IS ONE MILLISECOND, so a test can prove expiry without waiting on
/// a real backend.
///
/// THE TOP END IS REFUSED BY NAME, and two things break past it, neither
/// visible where it happens. NOTHING SUPERVISES THE DETACHED RECAP CHILD, so at
/// a day it is one child plus one wedged backend held for a day. AND A DURATION
/// PAST THE CEILING PANICS at `Instant::now() + deadline` (MEASURED: "overflow
/// when adding duration to instant") inside a process whose stderr is
/// /dev/null and whose exit code nobody reads, so the recap simply vanishes
/// after the card has said it is coming.
fn summarizer_deadline_range() -> RangeInclusive<Duration> {
    Duration::from_millis(1)..=Duration::from_secs(MAX_SUMMARIZER_DEADLINE_SECS)
}

pub(super) const MAX_SUMMARIZER_DEADLINE_SECS: u64 = 3600;
