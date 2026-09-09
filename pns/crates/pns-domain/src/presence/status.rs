//! Where the operator is: the raw readings turned into the units the
//! arbitration compares.
//!
//! THE SPLIT INSIDE THIS MODULE MATTERS AS MUCH AS THE ONE BETWEEN THE MODULES.
//! Every function here is a function of its arguments; the readings themselves
//! sit behind the probe traits, so a test hands this module fixture bytes
//! instead of a live machine.
//!
//! THE FILE A ROOM READING ARRIVES IN IS `presence_file`'s, and the line
//! between them is syntax against meaning: that module says what a line SAYS
//! and this one says what it MEANS, because the writer and the routing are
//! what change them and those are two different changes.
//!
//! The arbitration those units feed lives in `surface`, and it lives there
//! ONCE: the engine deciding which channels fire and a harness gate deciding
//! whether a phone round trip fires at all both call it, because a second copy
//! is how the two would drift into disagreeing about where the operator is.

/// The unit the idle counter is read in.
const NANOSECONDS_PER_SEC: u64 = 1_000_000_000;

/// Seconds since the last human input, read from a nanosecond counter, or
/// `None` when that cannot be read.
///
/// None is the unknown verdict, and the phone rule reads unknown as away: a
/// garbled probe line must never coerce to 0, which reads as "actively typing"
/// and silently drops the push.
pub fn idle_secs_from_ns(idle_nanoseconds: &str) -> Option<u64> {
    crate::count::parse_count(idle_nanoseconds).map(|nanoseconds| nanoseconds / NANOSECONDS_PER_SEC)
}

/// Where the operator is, as far as the bridge can say.
///
/// AN ENUM RATHER THAN AN `Option`, because every way of not knowing is a
/// different thing to go and fix, and the doctor line and the eventual
/// routing both need the reason an `Option` cannot carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceStatus {
    /// The room the operator is in, and how old its edge is. Motion reported
    /// NOW is age zero.
    Room { room: String, age_secs: u64 },
    /// A fresh poll that found motion in no watched room. NOT unknown: the
    /// bridge answered, and it answered "not there".
    Nowhere { poll_age_secs: u64 },
    /// No usable reading, and why.
    Unknown(Unreadable),
}

/// Why there is no reading. Every variant is a fail-CLOSED outcome: presence
/// narrows what the lights do and never widens it, so not knowing costs the
/// narrowing and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreadable {
    /// Nothing at the path, or nothing in it this parse accepts.
    NoReading,
    /// There was a line and no clock to age it against.
    NoClock,
    /// The last poll is older than the bound, so the writer stopped. A DEAD
    /// BRIDGE IS UNKNOWN, never present and never absent.
    Stale { poll_age_secs: u64 },
    /// An epoch newer than the clock. TWO-SIDED ON PURPOSE: reading a future
    /// epoch as the freshest input there is would let a wrong clock or a
    /// hand-written file pin the operator in a room for good.
    Future,
    /// The reported room is not one this config watches, or is excluded,
    /// which is not a claim about where the operator is.
    NotWatched,
}

/// One line of the state file, as SYNTAX ALONE. Nothing here is aged or
/// matched against the config: `classify` is where a reading becomes a
/// verdict, so the two are read and tested apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPresence {
    pub poll_epoch: u64,
    /// `None` for the poll-only line: the poll ran and no configured room had
    /// an edge.
    pub edge: Option<Edge>,
}

/// The motion edge a poll found, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub epoch: u64,
    /// Whether motion is reported NOW, rather than having been at `epoch`.
    pub motion: bool,
    /// The bridge's own room name, verbatim.
    pub room: String,
}

/// What a reading MEANS: the whole policy, as a function of its arguments.
///
/// THE ORDER OF THE REFUSALS IS THE POINT. The future check runs before the
/// staleness arithmetic, because `now - poll_epoch` on a future epoch would
/// underflow into an enormous age (or, saturating, into a fresh-looking
/// zero); staleness runs before the room, because a room name off a poll
/// nobody refreshed is not evidence about now.
pub fn classify(
    raw: Option<RawPresence>,
    now: Option<u64>,
    stale_after_secs: u64,
    rooms: &[String],
    exclude: &[String],
) -> PresenceStatus {
    let Some(raw) = raw else {
        return PresenceStatus::Unknown(Unreadable::NoReading);
    };
    let Some(now) = now else {
        return PresenceStatus::Unknown(Unreadable::NoClock);
    };
    if raw.poll_epoch > now || raw.edge.as_ref().is_some_and(|edge| edge.epoch > now) {
        return PresenceStatus::Unknown(Unreadable::Future);
    }
    let poll_age_secs = now - raw.poll_epoch;
    if poll_age_secs >= stale_after_secs {
        return PresenceStatus::Unknown(Unreadable::Stale { poll_age_secs });
    }
    let Some(edge) = raw.edge else {
        return PresenceStatus::Nowhere { poll_age_secs };
    };
    if !rooms.contains(&edge.room) || exclude.contains(&edge.room) {
        return PresenceStatus::Unknown(Unreadable::NotWatched);
    }
    PresenceStatus::Room {
        // MOTION NOW IS AGE ZERO. The edge is when motion STARTED, so a room
        // still reporting motion after a long turn would otherwise age out of
        // freshness while the operator is standing in it.
        age_secs: if edge.motion { 0 } else { now - edge.epoch },
        room: edge.room,
    }
}

/// Why there is no reading, in one phrase and with no prefix on it.
///
/// ONE WORDING, TWO READERS, following `channels::hue::missing_sentence`: the
/// doctor prefixes it with the plugin name and the lamp journal writes it
/// inside its own line, and an operator who reads the same unknown reported
/// two different ways has to work out whether they are the same problem.
pub fn unreadable_said(reason: &Unreadable) -> String {
    match reason {
        Unreadable::NoReading => "no reading".to_string(),
        Unreadable::NoClock => "the clock could not be read".to_string(),
        Unreadable::Stale { poll_age_secs } => format!("stale, poll {poll_age_secs}s old"),
        Unreadable::Future => "future epoch".to_string(),
        Unreadable::NotWatched => "the reported room is not one this config watches".to_string(),
    }
}

#[cfg(test)]
mod tests;
