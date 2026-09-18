//! Quiet that follows a calendar: what a window of busy time means for the
//! mute, and what the operator's own hand means for the calendar.
//!
//! THE CALENDAR IS A SECOND HAND ON THE SAME SWITCH, so the whole policy here
//! is about telling the two hands apart. It owns the mute it set and nothing
//! else: what it armed it may extend, shorten or clear, and a mute it did not
//! arm it never touches.

use super::is_muted;

/// One event the calendar command reported, in epoch seconds.
///
/// THE BUSY FLAG IS THE PRODUCER'S READING and the FILTER IS THIS CRATE'S.
/// A command that only ever reported the events it considered busy would put
/// the policy in the implementer, where a second implementer would have to
/// guess it; reported flat, the window is data and the rule is one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub start: u64,
    pub end: u64,
    pub busy: bool,
}

/// What the calendar last did, and what the operator last took back off it.
///
/// BOTH ARE END SECONDS. `armed_until` is the expiry this feature wrote, which
/// is what "the mute is still mine" is judged by, and `declined_until` is the
/// end of an event the operator overruled, which is what keeps this from
/// re-arming two minutes later. An event is identified by the second it ends
/// on, so nothing about its subject, its attendees or its identifier is ever
/// held on disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CalendarState {
    pub armed_until: Option<u64>,
    pub declined_until: Option<u64>,
}

/// What one poll asks of the mute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// Mute until this second.
    Arm(u64),
    /// Clear the mute this feature set.
    Clear,
    /// Touch nothing.
    Leave,
}

/// One poll's whole decision: the events, the clock, the standing mute and
/// what this feature did last, in; the move and the state to remember, out.
///
/// A MANUAL `pns quiet` ALWAYS WINS, in both directions, and both directions
/// are here:
///
/// - A mute this feature did not arm is never extended, shortened or cleared,
///   so quiet switched on by hand outlasts the meeting it was set during.
/// - A mute this feature DID arm that no longer matches the standing expiry is
///   an operator who cleared it or replaced it with their own, so the rest of
///   that event is declined rather than re-armed on the next poll.
///
/// AN OVERRIDE EXPIRES WITH THE EVENT IT WAS AGAINST. It is kept against the
/// event's end second, so the next event arms normally; there is no separate
/// timer to hold a stale override open.
pub fn decide(
    events: &[Event],
    now: u64,
    quiet_expiry: Option<u64>,
    state: CalendarState,
) -> (Move, CalendarState) {
    // THE LONGEST WINDOW COVERING NOW, because overlapping meetings are one
    // stretch of busy time as far as the mute is concerned.
    let busy_until = events
        .iter()
        .filter(|event| event.busy && event.start <= now && now < event.end)
        .map(|event| event.end)
        .max();
    let owned = state
        .armed_until
        .is_some_and(|until| quiet_expiry == Some(until));
    let overruled = state.armed_until.is_some() && !owned;
    let Some(end) = busy_until else {
        // NOTHING IS ON. The mute this feature armed goes with the meeting,
        // which is what ends quiet early when an event is shortened or
        // deleted; an expiry that simply ran out clears the same way.
        let mv = if owned { Move::Clear } else { Move::Leave };
        return (mv, CalendarState::default());
    };
    if overruled || state.declined_until == Some(end) {
        return (
            Move::Leave,
            CalendarState {
                armed_until: None,
                declined_until: Some(end),
            },
        );
    }
    if !owned && is_muted(quiet_expiry, Some(now)) {
        // A MUTE SOMEBODY ELSE SET, left alone and NOT declined: when it runs
        // out mid-meeting the next poll arms the rest of the event.
        return (Move::Leave, CalendarState::default());
    }
    if quiet_expiry == Some(end) {
        return (
            Move::Leave,
            CalendarState {
                armed_until: Some(end),
                declined_until: None,
            },
        );
    }
    (
        Move::Arm(end),
        CalendarState {
            armed_until: Some(end),
            declined_until: None,
        },
    )
}

#[cfg(test)]
mod tests;
