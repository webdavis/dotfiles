//! `type = "command"`: the command the config names, and the document it
//! answers with.
//!
//! NO CALENDAR IS NAMED HERE. Which calendar the command reads, and how, is
//! the command's business.

use pns_domain::mute::calendar::Event;
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
pub(super) fn read_command(argv: &[String], deadline: Duration) -> Result<Vec<Event>, String> {
    let (program, arguments) = argv.split_first().ok_or(NO_COMMAND)?;
    let mut command = std::process::Command::new(program);
    command.args(arguments);
    let answer = crate::run_bounded(command, None, deadline, CALENDAR_READ_MAX).ok_or(NO_ANSWER)?;
    parse_calendar(&answer)
}

/// What the command may write before its answer is no answer at all. Generous
/// for a day of events at a hundred bytes each, and still a bound.
const CALENDAR_READ_MAX: u64 = 256 * 1024;

pub(super) const NO_COMMAND: &str = "the calendar command names no program to run";

/// ONE SENTENCE FOR EVERY WAY A RUN PRODUCES NOTHING, because `run_bounded`
/// answers the same way for each and the operator's next step is the same:
/// run the command by hand and look at what it does.
pub(super) const NO_ANSWER: &str =
    "the calendar command answered nothing (it failed, ran past its deadline, or said too much)";

pub(super) const NOT_THE_DOCUMENT: &str = "the calendar command answered something other than {\"events\": [{\"start\": <epoch>, \
     \"end\": <epoch>, \"busy\": <true|false>}]}";

/// The document as events, or the one refusal above.
///
/// STRICT, AND NEVER PARTIAL. An event missing a field or carrying a
/// fractional second is the whole answer refused rather than the event
/// dropped: a dropped event is a meeting that silently does not mute, which
/// is the failure this feature exists to prevent, and it looks exactly like a
/// clear calendar.
pub(super) fn parse_calendar(answer: &str) -> Result<Vec<Event>, String> {
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
