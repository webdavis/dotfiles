//! The calendar edge: the command the config names, the document it answers
//! with, and the little state file this feature keeps beside it.
//!
//! NO CALENDAR IS NAMED HERE. pns runs a command the operator's own config
//! states and reads one documented document off its standard output, the way
//! the recap runs the summarizer it is given: which calendar that command
//! reads, and how, is the command's business.
//!
//! NOTHING OFF THE COMMAND REACHES A MESSAGE. The document carries three
//! numbers per event and no text at all, and every refusal below names the
//! shape rather than quoting what arrived, so a meeting's subject, its
//! attendees and its identifier cannot reach a log line.

use pns_domain::quiet::calendar::{CalendarState, Event};
use std::path::Path;
use std::time::Duration;

/// The document one poll reads, on standard output:
///
/// ```json
/// {"events": [{"start": 1758132000, "end": 1758134400, "busy": true}]}
/// ```
///
/// `start` and `end` are epoch seconds and `busy` is whether the event holds
/// the operator's time. Every field is required and every event the window
/// covers is reported, busy or not: the filter is pns's, so a second
/// implementer states what the calendar says rather than what pns should do
/// about it. Unknown fields are ignored, which is what lets the document grow.
pub fn read_calendar(argv: &[String], deadline: Duration) -> Result<Vec<Event>, String> {
    let (program, arguments) = argv.split_first().ok_or(NO_COMMAND)?;
    let mut command = std::process::Command::new(program);
    command.args(arguments);
    let answer = crate::run_bounded(command, None, deadline, CALENDAR_READ_MAX).ok_or(NO_ANSWER)?;
    parse_calendar(&answer)
}

/// What the command may write before its answer is no answer at all. Generous
/// for a day of events at a hundred bytes each, and still a bound.
pub const CALENDAR_READ_MAX: u64 = 256 * 1024;

const NO_COMMAND: &str = "the calendar command names no program to run";

/// ONE SENTENCE FOR EVERY WAY A RUN PRODUCES NOTHING, because `run_bounded`
/// answers the same way for each and the operator's next step is the same:
/// run the command by hand and look at what it does.
const NO_ANSWER: &str =
    "the calendar command answered nothing (it failed, ran past its deadline, or said too much)";

const NOT_THE_DOCUMENT: &str = "the calendar command answered something other than {\"events\": [{\"start\": <epoch>, \
     \"end\": <epoch>, \"busy\": <true|false>}]}";

/// The document as events, or the one refusal above.
///
/// STRICT, AND NEVER PARTIAL. An event missing a field or carrying a
/// fractional second is the whole answer refused rather than the event
/// dropped: a dropped event is a meeting that silently does not mute, which
/// is the failure this feature exists to prevent, and it looks exactly like a
/// clear calendar.
pub fn parse_calendar(answer: &str) -> Result<Vec<Event>, String> {
    let document: serde_json::Value =
        serde_json::from_str(answer).map_err(|_| NOT_THE_DOCUMENT.to_string())?;
    let events = document
        .get("events")
        .and_then(serde_json::Value::as_array)
        .ok_or(NOT_THE_DOCUMENT)?;
    events
        .iter()
        .map(|event| {
            let second = |name: &str| event.get(name).and_then(serde_json::Value::as_u64);
            match (
                second("start"),
                second("end"),
                event.get("busy").and_then(serde_json::Value::as_bool),
            ) {
                (Some(start), Some(end), Some(busy)) => Ok(Event { start, end, busy }),
                _ => Err(NOT_THE_DOCUMENT.to_string()),
            }
        })
        .collect()
}

/// This feature's own two numbers, under the state directory: the expiry it
/// armed and the end of an event the operator overruled, `0` for neither.
///
/// A FILE RATHER THAN A ROW, beside the poll cursor next to it and for its
/// reason: it is one line of syntax that a poll rewrites, and a file that
/// cannot be read is simply no state, which costs one re-arm and never a mute
/// nobody asked for.
pub const CALENDAR_STATE: &str = "quiet-calendar";

/// The most of it any reader pulls in: two counts and a space.
const CALENDAR_STATE_READ_MAX: u64 = 64;

pub fn read_calendar_state(state: &Path) -> CalendarState {
    crate::readable_state_file(&state.join(CALENDAR_STATE), CALENDAR_STATE_READ_MAX)
        .ok()
        .as_deref()
        .map(parse_calendar_state)
        .unwrap_or_default()
}

pub fn write_calendar_state(state: &Path, held: &CalendarState) -> std::io::Result<()> {
    crate::publish_state_line(&state.join(CALENDAR_STATE), &render_calendar_state(held))
}

/// `<armed_until> <declined_until>`, each a count with `0` for none. A line
/// that is not that is no state at all.
fn parse_calendar_state(text: &str) -> CalendarState {
    let line = text.lines().next().unwrap_or_default();
    let Some((armed, declined)) = line.split_once(' ') else {
        return CalendarState::default();
    };
    match (
        pns_domain::count::parse_count(armed),
        pns_domain::count::parse_count(declined),
    ) {
        (Some(armed), Some(declined)) => CalendarState {
            armed_until: (armed > 0).then_some(armed),
            declined_until: (declined > 0).then_some(declined),
        },
        _ => CalendarState::default(),
    }
}

fn render_calendar_state(held: &CalendarState) -> String {
    format!(
        "{} {}",
        held.armed_until.unwrap_or_default(),
        held.declined_until.unwrap_or_default()
    )
}

#[cfg(test)]
mod tests;
