//! The poll's state file: what its lines say, and nothing about what they
//! MEAN.
//!
//! SYNTAX ONLY, the presence state file's own split, and for its reason: the
//! FORMAT changes when the poll's transport does, and the POLICY (how long an
//! identity is remembered, which cursor wins) changes when the routing does.
//!
//! ONE FILE FOR ALL THREE PARTS, published by rename, which is what makes a
//! failed poll lose nothing: the cursor, the interval and the seen-set are
//! written together or not at all, so there is no half-advanced state for the
//! next tick to read.

use crate::{publish_state_line, readable_state_file};
use pns_domain::github::poll::{PollState, SEEN_MAX, Seen};
use std::path::Path;

/// THE STATE FILE, owner-only, under the state directory.
///
/// ```text
/// <interval_secs> <last_modified...>
/// <first_seen_epoch> <identity>
/// ...
/// ```
///
/// The first line is the cursor pair, the interval first because it is the
/// bounded field and `Last-Modified` is free text to the end of the line. Each
/// line after it is one remembered identity. A file that is not this is read
/// as NO STATE AT ALL, which costs one duplicate per remembered event and
/// never loses one.
pub const GITHUB_POLL_STATE: &str = "github-poll";

/// The most of it any reader pulls into memory. `SEEN_MAX` identities at a
/// generous 200 bytes each, plus the cursor line: generous, and still a bound,
/// so a file some other hand grew is refused rather than allocated.
pub const GITHUB_POLL_READ_MAX: u64 = (SEEN_MAX as u64 + 1) * 256;

/// The state this directory holds, or a default one when there is none to
/// read.
///
/// A MISSING, UNREADABLE OR CORRUPT FILE IS ONE ANSWER, and it is the empty
/// state rather than a refusal: a poll that stopped over a state file it
/// could not parse is a source that goes silent, and the cost of starting
/// over is one duplicate per event still inside the window.
pub fn read_poll_state(state: &Path) -> PollState {
    readable_state_file(&state.join(GITHUB_POLL_STATE), GITHUB_POLL_READ_MAX)
        .ok()
        .as_deref()
        .map(parse_poll_state)
        .unwrap_or_default()
}

/// Publish one state, atomically. The error is returned rather than swallowed
/// so the caller states its own direction.
pub fn write_poll_state(state: &Path, poll: &PollState) -> std::io::Result<()> {
    publish_state_line(&state.join(GITHUB_POLL_STATE), &render_poll_state(poll))
}

/// The file's text as a state.
pub fn parse_poll_state(text: &str) -> PollState {
    let mut lines = text.lines();
    let (interval_secs, last_modified) =
        lines.next().map(cursor_line).unwrap_or((0, String::new()));
    PollState {
        last_modified,
        interval_secs,
        seen: lines.filter_map(seen_line).collect(),
    }
}

/// The cursor line: a count, then the rest of the line verbatim.
///
/// VERBATIM TO THE END OF THE LINE, because `Last-Modified` is an HTTP date
/// carrying spaces and commas and is sent back byte for byte. A line whose
/// first field is not a count is no cursor at all rather than a cursor with a
/// zero interval, because the interval and the date come from one answer.
fn cursor_line(line: &str) -> (u64, String) {
    let (interval, rest) = line.split_once(' ').unwrap_or((line, ""));
    match pns_domain::count::parse_count(interval) {
        Some(interval) if printable(rest) => (interval, rest.to_string()),
        _ => (0, String::new()),
    }
}

/// One seen line, or nothing when it is not one.
fn seen_line(line: &str) -> Option<Seen> {
    let (first_seen, identity) = line.split_once(' ')?;
    let first_seen = pns_domain::count::parse_count(first_seen)?;
    (!identity.is_empty() && printable(identity)).then(|| Seen {
        identity: identity.to_string(),
        first_seen,
    })
}

/// Whether this field can cross the file and come back unchanged.
///
/// A CONTROL CHARACTER WOULD FORGE A SECOND LINE, which is the one way a
/// value read off the network could change what the next read believes. Both
/// fields that carry remote text go through it, on the way in and on the way
/// out.
fn printable(text: &str) -> bool {
    !text.chars().any(char::is_control)
}

/// One state as the text the reader parses back.
///
/// THERE IS NO UNCHECKED SPELLING OF THIS: every field goes through the same
/// predicate the parse applies, so an identity or a cursor the reader would
/// refuse is DROPPED here rather than published as a line that silently
/// truncates the file.
fn render_poll_state(poll: &PollState) -> String {
    let cursor = if printable(&poll.last_modified) {
        poll.last_modified.as_str()
    } else {
        ""
    };
    let mut lines = vec![format!("{} {cursor}", poll.interval_secs)];
    lines.extend(
        poll.seen
            .iter()
            .filter(|seen| !seen.identity.is_empty() && printable(&seen.identity))
            .map(|seen| format!("{} {}", seen.first_seen, seen.identity)),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests;
