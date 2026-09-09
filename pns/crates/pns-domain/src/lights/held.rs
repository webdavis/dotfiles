//! Which state the house is in, and which lamp is entitled to show it.

use super::unread::Unread;

/// Whether any wait is still live.
pub fn any_blocked(marker_epochs: &[u64], now: u64, max_age_secs: u64) -> bool {
    marker_epochs
        .iter()
        .any(|at| marker_is_live(*at, now, max_age_secs))
}
/// Whether one epoch is still inside its bound.
///
/// ONE PREDICATE FOR EVERY AGED MARKER IN THIS MODULE, because each of them has
/// two readers that must agree: the aggregate that lights a lamp, and the sweep
/// that DELETES what has aged out. Two spellings of "expired" would be a marker
/// the aggregate ignored and the sweep kept, accumulating forever, or one the
/// sweep removed while the aggregate was still lighting a lamp for it.
///
/// BOTH EDGES CLOSED: exactly at the bound is still live. A MARKER FROM THE
/// FUTURE IS LIVE TOO, because a clock that stepped backwards is not a wait that
/// ended, and the saturating subtraction reads it as zero seconds old rather
/// than as an enormous age that would delete it.
pub fn marker_is_live(at: u64, now: u64, max_age_secs: u64) -> bool {
    now.saturating_sub(at) <= max_age_secs
}
/// One HELD state, and the four of them in the order they outrank each other.
///
/// THE DECLARATION ORDER IS THE RANK, and `active_held` pushes in that fixed
/// order rather than sorting: nothing here compares one `Held` to another at
/// runtime. Blocked is on top, which is the operator's own ruling: a question
/// waiting on them outranks work in progress, and work in progress outranks
/// news about work that has already finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    Blocked,
    Looping,
    UnreadFailure,
    UnreadSuccess,
}
impl Held {
    /// The ROUTABLE word this state is carried by. The two unread flavours
    /// answer the same word, which is what makes a lamp carry both or neither.
    pub fn behaviour(self) -> crate::lamps::config::Behaviour {
        match self {
            Held::Blocked => crate::lamps::config::Behaviour::Blocked,
            Held::Looping => crate::lamps::config::Behaviour::Looping,
            Held::UnreadFailure | Held::UnreadSuccess => crate::lamps::config::Behaviour::Unread,
        }
    }

    /// The word a held record carries to say WHICH breath a phase belongs to.
    ///
    /// THE STATE AND NOT ITS ROUTABLE WORD, which is why this is not
    /// `behaviour`: the two unread flavours share one routable word and do NOT
    /// share a colour, so a red failure inheriting a green success's phase is
    /// exactly the delay the phase identity exists to stop. Four states, four
    /// words.
    pub fn word(self) -> &'static str {
        match self {
            Held::Blocked => "blocked",
            Held::Looping => "loop",
            Held::UnreadFailure => "failure",
            Held::UnreadSuccess => "success",
        }
    }

    /// The state a recorded word names, or None for a word this build does not
    /// know.
    pub fn from_word(word: &str) -> Option<Held> {
        [
            Held::Blocked,
            Held::Looping,
            Held::UnreadFailure,
            Held::UnreadSuccess,
        ]
        .into_iter()
        .find(|held| held.word() == word)
    }
}
/// What the house is holding this tick, one field per state.
///
/// A NAMED STRUCT rather than three positional values, two of which are bools:
/// a transposition would be a lamp showing the wrong state and nothing would
/// catch it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct House {
    pub blocked: bool,
    pub looping: bool,
    pub unread: Option<Unread>,
}
/// Every state the house is holding, most urgent first.
///
/// A LIST RATHER THAN ONE STATE, which is the whole difference from the shipped
/// design: the house holds all of them at once and each LAMP resolves which one
/// it shows, so a blocked lamp and a loop lamp can be lit at the same moment
/// because they are routed for different words.
///
/// THE PUSHES ARE IN RANK ORDER and there is no sort behind them. One was here
/// and could never change the answer, which is exactly the code a reader trusts
/// and a mutation walks straight through. What pins the order instead is the
/// test that asserts the whole vector, so pushing out of order is red.
pub fn active_held(house: &House) -> Vec<Held> {
    let mut held = Vec::new();
    if house.blocked {
        held.push(Held::Blocked);
    }
    if house.looping {
        held.push(Held::Looping);
    }
    match house.unread {
        Some(Unread::Failure) => held.push(Held::UnreadFailure),
        Some(Unread::Success) => held.push(Held::UnreadSuccess),
        None => {}
    }
    held
}
/// What ONE lamp shows: the most urgent active state it is routed for, or
/// nothing.
///
/// THE LAMP'S OWN ROUTING IS THE FILTER, so a state nothing routes to that lamp
/// leaves it dark rather than falling through to a lamp that was not asked. That
/// is what lets one house state reach three lamps saying different things.
pub fn shown(active: &[Held], shows: &[crate::lamps::config::Behaviour]) -> Option<Held> {
    active
        .iter()
        .copied()
        .find(|held| shows.contains(&held.behaviour()))
}
/// Whether a PULSE fires on one lamp.
///
/// A HELD STATE PREEMPTS A PULSE ON THE LAMP THAT IS HOLDING IT, which is the
/// operator's "dedicated, but it helps out when free" ruling generalised: a lamp
/// dedicated to the held states joins the pulse lamps whenever none of them is
/// active, and stops joining the moment one is. The pulse still fires on every
/// OTHER lamp routed for it, so nothing is lost, and the held state is not
/// interrupted by a four-second blink it would have to be re-armed after.
pub fn pulse_fires(
    shows: &[crate::lamps::config::Behaviour],
    behaviour: crate::lamps::config::Behaviour,
    lamp_is_held: bool,
) -> bool {
    shows.contains(&behaviour) && !lamp_is_held
}

#[cfg(test)]
mod tests;
