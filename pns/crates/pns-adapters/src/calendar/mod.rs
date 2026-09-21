//! The calendar edge: the source the config names, the busy intervals it
//! answers with, and the little state file this feature keeps beside it.
//!
//! WHICH CALENDAR IS THE CONFIG'S BUSINESS. `type = "command"` runs a command
//! the operator's own config states and reads one documented document off its
//! standard output, the way the recap runs the summarizer it is given;
//! `type = "google"` reads busy intervals out of Google Calendar with
//! credentials the same table carries.
//!
//! NOTHING OFF THE CALENDAR REACHES A MESSAGE. Both sources carry three
//! numbers per event and no text at all, and every refusal below names the
//! shape rather than quoting what arrived, so a meeting's subject, its
//! attendees and its identifier cannot reach a log line, and neither can a
//! credential.

use crate::config::CalendarSource;
use pns_domain::mute::calendar::{CalendarState, Event};
use std::path::Path;
use std::time::Duration;

mod command;
mod google;

/// One poll of whichever calendar the config names, as busy intervals in
/// epoch seconds, or one sentence saying why there are none.
///
/// `state` IS WHERE A SOURCE KEEPS ITS OWN CACHE (the Google access token);
/// the command source keeps nothing and reads it not at all.
pub fn read_calendar(
    source: &CalendarSource,
    state: &Path,
    now: u64,
    deadline: Duration,
) -> Result<Vec<Event>, String> {
    match source {
        CalendarSource::Command(argv) => command::read_command(argv, deadline),
        CalendarSource::Google(settings) => {
            google::GoogleCalendarSource::new(settings.clone(), deadline).read(state, now)
        }
    }
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
