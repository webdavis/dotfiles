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

use crate::home::HomePresence;
use crate::presence::PresenceStatus;
use crate::presence_policy::{Full, Narrowing, Snapshot};

/// One narrowing decision, as the fields the line carries.
///
/// THE STRUCT IS THE SCHEMA, and it is the READ side too: `entry` writes these
/// fields and `last` reads them back, so the pair cannot drift.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// The clock the decision's readings were taken against, absent when
    /// there was no clock.
    pub at: Option<u64>,
    /// What the room sensor said, in one phrase.
    pub presence: String,
    /// Seconds since the desk keyboard was touched, absent when unreadable.
    pub desk_idle_secs: Option<u64>,
    /// What the router said about the phone, as its variant name.
    pub home: String,
    /// The room the lamps were narrowed to. `None` is the whole routing left
    /// standing, and `reason` says why.
    pub room: Option<String>,
    /// Why the routing was left whole, empty when a room was named.
    pub reason: String,
}

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
pub fn last(contents: &str) -> Option<Entry> {
    contents.lines().rev().find_map(|line| {
        let parsed: serde_json::Value = serde_json::from_str(line).ok()?;
        let text = |key: &str| parsed.get(key)?.as_str().map(str::to_string);
        Some(Entry {
            at: parsed.get("at").and_then(serde_json::Value::as_u64),
            presence: text("presence").unwrap_or_default(),
            desk_idle_secs: parsed
                .get("desk_idle_secs")
                .and_then(serde_json::Value::as_u64),
            home: text("home").unwrap_or_default(),
            room: text("room"),
            reason: text("reason").unwrap_or_default(),
        })
    })
}

/// Why a routing was left whole, in one phrase, for the record and the doctor.
pub fn reason_said(full: &Full) -> String {
    match full {
        Full::NotHome => "the phone is not on the home network".to_string(),
        Full::NoDeskRoom => "at the desk, and no desk_room says which room that is".to_string(),
        Full::Nowhere => "motion in no watched room".to_string(),
        Full::Unknown(reason) => format!("unknown: {}", crate::presence::unreadable_said(reason)),
        Full::NoLampIn(room) => format!("no lamp in {room:?}"),
    }
}

/// What the reading itself says, in one phrase.
pub fn reading_said(status: &PresenceStatus) -> String {
    match status {
        PresenceStatus::Room { room, age_secs } => format!("room {room:?} ({age_secs}s ago)"),
        PresenceStatus::Nowhere { poll_age_secs } => format!("nowhere (poll {poll_age_secs}s ago)"),
        PresenceStatus::Unknown(reason) => {
            format!("unknown ({})", crate::presence::unreadable_said(reason))
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
mod tests {
    use super::{Entry, entry, last, reason_said, recorded};
    use crate::home::{DeviceKey, HomePresence};
    use crate::presence::{PresenceStatus, Unreadable};
    use crate::presence_policy::{Full, Narrowing, Snapshot};

    /// The snapshot the record is taken from: the desk warm, the phone home,
    /// and a fresh reading in the master bedroom.
    fn snapshot() -> Snapshot {
        Snapshot {
            status: PresenceStatus::Room {
                room: "3F - MBedroom".to_string(),
                age_secs: 0,
            },
            desk_idle_secs: Some(4),
            screen_locked: Some(false),
            home: HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            },
            desk_room: Some("3F - Studio".to_string()),
            desk_stale_after_secs: 120,
            now: Some(1_700_000_000),
        }
    }

    #[test]
    fn the_record_carries_the_reading_the_desk_clock_and_the_router_verdict() {
        // All four, because the answer is only explicable from all of them:
        // the same room can be chosen off motion or off the desk, and the same
        // motion can be overruled or obeyed.
        let record = recorded(&snapshot(), &Narrowing::To("3F - MBedroom".to_string()));
        assert_eq!(record.at, Some(1_700_000_000));
        assert_eq!(record.presence, r#"room "3F - MBedroom" (0s ago)"#);
        assert_eq!(record.desk_idle_secs, Some(4));
        assert_eq!(record.home, "home");
        assert_eq!(record.room, Some("3F - MBedroom".to_string()));
        assert_eq!(record.reason, "");
    }

    #[test]
    fn the_router_verdict_is_recorded_as_one_word_and_never_its_evidence() {
        // The matched key and the value it matched are the router
        // diagnostic's business; this record is read for which way the gate
        // went, and the value is the phone's own name.
        for (home, said) in [
            (
                HomePresence::Home {
                    matched_by: DeviceKey::Hostname,
                    value: "mister".to_string(),
                },
                "home",
            ),
            (HomePresence::NotHome, "not-home"),
            (HomePresence::Unknown, "unknown"),
        ] {
            let record = recorded(
                &Snapshot { home, ..snapshot() },
                &Narrowing::Full(Full::Nowhere),
            );
            assert_eq!(record.home, said);
            assert!(!record.home.contains("mister"), "{}", record.home);
        }
    }

    #[test]
    fn every_way_a_routing_can_be_left_whole_names_its_own_reason() {
        // EACH IS A DIFFERENT THING TO GO AND FIX, so collapsing any two into
        // one wording sends half the readers to the wrong edit.
        let said = [
            (Full::NotHome, "the phone is not on the home network"),
            (
                Full::NoDeskRoom,
                "at the desk, and no desk_room says which room that is",
            ),
            (Full::Nowhere, "motion in no watched room"),
            (
                Full::NoLampIn("3F - Hallway".to_string()),
                r#"no lamp in "3F - Hallway""#,
            ),
            (Full::Unknown(Unreadable::NoReading), "unknown: no reading"),
            (
                Full::Unknown(Unreadable::NoClock),
                "unknown: the clock could not be read",
            ),
            (
                Full::Unknown(Unreadable::Stale { poll_age_secs: 90 }),
                "unknown: stale, poll 90s old",
            ),
            (Full::Unknown(Unreadable::Future), "unknown: future epoch"),
            (
                Full::Unknown(Unreadable::NotWatched),
                "unknown: the reported room is not one this config watches",
            ),
        ];
        for (full, named) in said {
            assert_eq!(reason_said(&full), named, "{full:?}");
        }
    }

    fn narrowed_to(room: &str) -> Entry {
        Entry {
            at: Some(1_700_000_000),
            presence: "room (0s ago)".to_string(),
            desk_idle_secs: Some(12),
            home: "Home".to_string(),
            room: Some(room.to_string()),
            reason: String::new(),
        }
    }

    #[test]
    fn a_room_name_carrying_the_readers_own_field_marker_still_reads_back_whole() {
        // The name is the bridge's own text. Parsed out of a `key=value` line
        // by splitting on a marker, this name would hand the doctor the tail
        // of its own room name and call it the decision.
        let hostile = narrowed_to("Kitchen narrowed=Studio");
        assert_eq!(last(&entry(&hostile)), Some(hostile));
    }

    #[test]
    fn a_room_name_carrying_a_newline_stays_one_entry() {
        // One entry is one line, or an append forges a second the doctor then
        // reads as the decision.
        let written = entry(&narrowed_to("Kitchen\nforged"));
        assert!(!written.contains('\n'), "{written}");
        assert_eq!(
            last(&written).and_then(|read| read.room),
            Some("Kitchen\nforged".to_string())
        );
    }

    #[test]
    fn a_routing_left_whole_carries_its_reason_and_names_no_room() {
        // The two are told apart structurally rather than by the shape of a
        // phrase, so a room literally named "nothing (away)" cannot read as a
        // fallback.
        let stood = Entry {
            room: None,
            reason: "away".to_string(),
            ..narrowed_to("unused")
        };
        let read = last(&entry(&stood)).expect("the entry reads back");
        assert_eq!((read.room, read.reason), (None, "away".to_string()));
    }

    #[test]
    fn the_newest_entry_is_the_one_read_back() {
        // The ring appends, so the decision the doctor reports is the last
        // line, never the first.
        let ring = format!(
            "{}\n{}\n",
            entry(&narrowed_to("2F - Kitchen")),
            entry(&narrowed_to("3F - Studio"))
        );
        assert_eq!(
            last(&ring).and_then(|read| read.room),
            Some("3F - Studio".to_string())
        );
    }

    #[test]
    fn a_line_this_cannot_read_is_skipped_rather_than_hiding_the_ones_behind_it() {
        let ring = format!("{}\nnot json at all\n", entry(&narrowed_to("2F - Kitchen")));
        assert_eq!(
            last(&ring).and_then(|read| read.room),
            Some("2F - Kitchen".to_string())
        );
    }
}
