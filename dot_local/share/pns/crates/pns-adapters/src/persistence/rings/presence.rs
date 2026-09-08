//! The lamp-narrowing journal: what one decision SAYS, and nothing about what
//! it means.
//!
//! ITS OWN MODULE beside `presence_policy`, which is the split `presence_file`
//! draws beside `presence`: the FORMAT changes when a reader or a writer of
//! the record changes, and the POLICY changes when the routing does.
//!
//! JSON AND NOT THE RING'S `key=value`, for `missed_notifications::entry`'s
//! exact reason: the free text. A room is the bridge's own name and may hold a
//! newline, a quote, or the reader's own field marker, and one entry has to
//! stay one line or an append forges a second. The ring solves that by
//! refusing free text; this cannot, so the escaping is taken from the library
//! that is already a dependency. BUILT WITH `json!` AND NEVER WITH `format!`,
//! which is the Rust spelling of this repo's "build JSON with `jq -n --arg`"
//! rule.
//!
//! IT DEPENDS ON THE POLICY AND NEVER THE OTHER WAY ROUND, following
//! `decision_log` beside `engine`: a record is written FROM a decision, so the
//! decision's own vocabulary is what it renders, and the policy stays a total
//! function of its arguments with nothing to say about where it is written
//! down.

use pns_domain::PresenceStatus;
use pns_domain::home::HomePresence;
use pns_domain::{Full, Narrowing, Snapshot};

use pns_domain::PresenceDecision as Entry;

/// One decision as one line of the ring.
pub fn entry(entry: &Entry) -> String {
    serde_json::json!({
        "at": entry.at,
        "presence": entry.presence,
        "desk_idle_secs": entry.desk_idle_secs,
        "home": entry.home,
        "room": entry.room,
        "reason": entry.reason,
    })
    .to_string()
}

/// The last decision the ring holds, or `None` when it holds none this reader
/// recognises.
///
/// PARSED BY KEY, never by position, which is `missed_notifications::entries`'
/// own rule: the writer's key order is `serde_json`'s business and invisible
/// here. A line this cannot read is skipped rather than failing the read, so
/// one corrupt entry never hides the four good ones behind it.
///
/// AND THE SCHEMA IS WHAT "CANNOT READ" MEANS, not the syntax alone. Every
/// entry this module writes carries `presence`, `home` and `reason` as
/// strings, so a line missing one of them was written by something else.
/// Filled in from `Default` instead, a bare `{}` parsed successfully and
/// became an entry saying nothing at all, which stopped the reverse walk on
/// the spot and hid the real decision on the line above it: an empty answer
/// where the doctor had a record to report. `room` stays genuinely optional
/// because a routing left whole names none, and `at` because a decision taken
/// with no clock has none.
pub fn last(contents: &str) -> Option<Entry> {
    contents.lines().rev().find_map(|line| {
        let parsed: serde_json::Value = serde_json::from_str(line).ok()?;
        let text = |key: &str| parsed.get(key)?.as_str().map(str::to_string);
        // OPTIONAL IS NOT UNTYPED. A room that is present and is not a string
        // is as much a line this reader does not recognise as a missing one:
        // read as `None` it becomes a decision that narrowed nothing, which
        // stops the reverse walk and hides the real one behind it.
        let room = match parsed.get("room") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(named)) => Some(named.clone()),
            Some(_) => return None,
        };
        Some(Entry {
            at: parsed.get("at").and_then(serde_json::Value::as_u64),
            presence: text("presence")?,
            desk_idle_secs: parsed
                .get("desk_idle_secs")
                .and_then(serde_json::Value::as_u64),
            home: text("home")?,
            room,
            reason: text("reason")?,
        })
    })
}

/// Why a routing was left whole, in one phrase, for the record and the doctor.
pub fn reason_said(full: &Full) -> String {
    match full {
        Full::NotHome => "the phone is not on the home network".to_string(),
        Full::NoDeskRoom => "at the desk, and no desk_room says which room that is".to_string(),
        Full::Ambiguous { desk, motion } => {
            format!("the desk in {desk:?} and newer motion in {motion:?} disagree")
        }
        Full::Nowhere => "motion in no watched room".to_string(),
        Full::Unknown(reason) => format!("unknown: {}", pns_domain::unreadable_said(reason)),
        Full::NoLampIn(room) => format!("no lamp in {room:?}"),
    }
}

/// What the reading itself says, in one phrase.
pub fn reading_said(status: &PresenceStatus) -> String {
    match status {
        PresenceStatus::Room { room, age_secs } => format!("room {room:?} ({age_secs}s ago)"),
        PresenceStatus::Nowhere { poll_age_secs } => format!("nowhere (poll {poll_age_secs}s ago)"),
        PresenceStatus::Unknown(reason) => {
            format!("unknown ({})", pns_domain::unreadable_said(reason))
        }
    }
}

/// One decision as the record the ring keeps.
pub fn recorded(snapshot: &Snapshot, narrowing: &Narrowing) -> Entry {
    let (room, reason) = match narrowing {
        Narrowing::To(room) => (Some(room.clone()), String::new()),
        Narrowing::Full(full) => (None, reason_said(full)),
    };
    Entry {
        at: snapshot.now,
        presence: reading_said(&snapshot.status),
        desk_idle_secs: snapshot.desk_idle_secs,
        home: format!("{:?}", HomeSaid(&snapshot.home)),
        room,
        reason,
    }
}

/// The router's verdict as one word, without the evidence the `Home` variant
/// carries: the record is read by a human looking for which way the gate went,
/// and the matched key and its value are the router diagnostic's business.
struct HomeSaid<'reading>(&'reading HomePresence);

impl std::fmt::Debug for HomeSaid<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self.0 {
            HomePresence::Home { .. } => "home",
            HomePresence::NotHome => "not-home",
            HomePresence::Unknown => "unknown",
        })
    }
}

#[cfg(test)]
mod tests;
