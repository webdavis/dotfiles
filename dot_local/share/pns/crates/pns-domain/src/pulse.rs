//! The light pulse: whether a session earns one and what colour it runs at.
//! Putting the light back is the BRIDGE's job now, not ours: the hue channel
//! asks for a timed signal and the bridge puts the lamp back when it ends,
//! byte for byte, measured on a real lamp on 2026-09-01.

/// How long a session must run before a pulse is worth the room's attention.
pub const DEFAULT_LONG_SESSION_SECS: u64 = 300;

/// The colour a signal runs at, as CIE xy. Not RGB, because the bridge clamps
/// RGB into its gamut and desaturates hard; xy bypasses that conversion.
///
/// BOTH PAIRS ARE OPERATOR-APPROVED AS SEEN, in the manual bridge trials of
/// 2026-08-12 (trials 4 and 5), which is the only test a colour can pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseColor {
    pub x: f64,
    pub y: f64,
}

/// Deep green, operator-locked on a real lamp on 2026-08-31 under the
/// observe-adjust-lock protocol. It replaced a paler green that read as
/// yellow-green across a room.
pub const SUCCESS_COLOR: PulseColor = PulseColor { x: 0.17, y: 0.70 };

/// Red, and it carries TWO jobs: the failure pulse, and the unread lamp's
/// failure flavour. ONE CONSTANT because they are one statement, "something
/// died", said once as a blink and once as a breath; two constants would let
/// the two drift into looking like different events.
pub const FAILURE_COLOR: PulseColor = PulseColor { x: 0.675, y: 0.322 };

/// Magenta, set on a Studio lamp by the operator on 2026-09-02 and read back
/// off the bridge rather than computed. It is the colour a question waiting on
/// the operator breathes in.
///
/// IT TOOK OVER FROM THE DEEP BLUE THAT NOW BELONGS TO `LOOP_COLOR`. Blocked
/// and loop sat 0.067 apart in the xy space the bridge takes, against 0.192 for
/// the next-closest pair in the whole set, and in daylight the two lamps read
/// as one. This pair is 0.207 apart, so the closest two colours in the
/// vocabulary are no longer these: they are failure and the unread success, at
/// that same 0.192 the swap does not touch.
pub const BLOCKED_COLOR: PulseColor = PulseColor {
    x: 0.3395,
    y: 0.1379,
};

/// Daylight, for the unread lamp's SUCCESS flavour: a run that finished while
/// the operator was away. Operator-locked on 2026-08-31.
///
/// A COLOUR NOBODY READS AS AN ALARM, which is the point of it: the red
/// flavour beside it is the one that needs answering, and a success that has
/// merely gone unseen must not compete with it.
pub const UNREAD_SUCCESS_COLOR: PulseColor = PulseColor { x: 0.50, y: 0.40 };

/// The deepest blue the studio lamps report, operator-locked on 2026-08-31
/// under the observe-adjust-lock protocol and moved here from `BLOCKED_COLOR`
/// on 2026-09-02: the loop lamp, breathing while long-running work is in
/// flight.
///
/// THE BLUE MOVED AND THE VIOLET LEFT, rather than the two trading places, so
/// no colour nobody has looked at enters the set: this blue had already been
/// locked on a real lamp, and the violet it displaces leaves the vocabulary
/// entirely.
pub const LOOP_COLOR: PulseColor = PulseColor {
    x: 0.1532,
    y: 0.0475,
};

/// True when a session ran long enough to be worth a light pulse.
///
/// An unreadable elapsed time or threshold is NOT long: unlike a dropped phone
/// push, a missed pulse costs nothing, so this one fails CLOSED rather than
/// flashing the room on garbage.
pub fn session_was_long(elapsed_secs: Option<u64>, threshold_secs: Option<u64>) -> bool {
    let (Some(elapsed_secs), Some(threshold_secs)) = (elapsed_secs, threshold_secs) else {
        return false;
    };
    elapsed_secs >= threshold_secs
}

/// What a lamp says about a given exit code, or `None` when the code is not
/// one `pulse_mode` can trust.
///
/// ANYTHING ALL ZEROES IS A SUCCESS, and any other run of ASCII digits is a
/// failure. An EMPTY code is the absent one: the shell version defaulted a
/// missing argument to zero, so absent and empty both mean success and there
/// is no third answer to give. GARBAGE IS NO LONGER A GUESS: a code that is
/// neither empty nor all ASCII digits (`-0`, padding, a stray word) answers
/// `None`, and `pulse_mode` refuses those with usage rather than painting the
/// room red on unproven input.
///
/// AN EXIT CODE HAS NO THIRD ANSWER, once it is a code at all. `pns pulse`
/// and the long-command notifier know a number and nothing else, so they
/// reach two of the five behaviours; the event path knows a STATE and reaches
/// three, through `state_behaviour`.
pub fn exit_behaviour(exit_code: &str) -> Option<crate::lamps::config::Behaviour> {
    if exit_code.is_empty() {
        return Some(crate::lamps::config::Behaviour::Done);
    }
    if !exit_code
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return None;
    }
    // Every character is an ASCII digit at this point, so "all zeroes" is a
    // safe textual test for the numeric value being zero.
    if exit_code.chars().all(|character| character == '0') {
        Some(crate::lamps::config::Behaviour::Done)
    } else {
        Some(crate::lamps::config::Behaviour::Failed)
    }
}

/// The states that hold a lamp BLOCKED: an agent waiting on the operator.
///
/// IT TRADES ONE WORD WITH `missed_notifications::NEEDS_YOU` IN EACH
/// DIRECTION. That constant is right to carry `failed`, because a turn that
/// died needs the operator every bit as much as one that asked; the lamps must
/// tell them apart, since `failed` says it died and `blocked` says it is
/// waiting, so reusing the shared list would hold every failure blocked.
///
/// AND `asking` IS ON THIS LIST ALONE. The shared list is the harness's own
/// state words, while a lamp also has to answer for what the CONDENSER writes:
/// every condensed turn is classified done, asking or blocked
/// (`hooks::condenser_prompt`), and `asking` is its word for a turn waiting on
/// an answer. Left off, it read as `done` and flashed green over a question.
pub const LAMP_BLOCKED: [&str; 5] = ["blocked", "asked", "plan-ready", "denied", "asking"];

/// What a lamp says about an event's state, given whether this machine has a
/// lamp map at all.
///
/// THE ONE MAPPING, stated here and read nowhere else. Everything that earns a
/// pulse and is not a failure or a wait is green, which is the shipped rule:
/// the event path used to ask whether the state was `failed` and hand the
/// success branch an exit code of zero for everything else.
///
/// `lamps_are_mapped` IS THE `[lights]` TABLE'S PRESENCE, and `blocked` exists
/// only behind it. Without the map there is one room-shaped pulse and two
/// colours, which is what has shipped since the bash; a long-running turn that
/// ends `blocked` has earned a pulse all along and flashed GREEN for it.
/// Turning that flash into the blocked colour would be a new behaviour arriving
/// on a machine that wrote no map and asked for nothing.
///
/// IT IS ONE ANSWER RATHER THAN A COLOUR AND A SEPARATE OPT-IN GATE. The
/// composition root asks this once and reads the opt-in off the answer, so
/// "which colour" and "may it fire at all" cannot come out disagreeing.
pub fn state_behaviour(state: &str, lamps_are_mapped: bool) -> crate::lamps::config::Behaviour {
    if state == "failed" {
        return crate::lamps::config::Behaviour::Failed;
    }
    if lamps_are_mapped && LAMP_BLOCKED.contains(&state) {
        return crate::lamps::config::Behaviour::Blocked;
    }
    crate::lamps::config::Behaviour::Done
}

#[cfg(test)]
mod tests;
