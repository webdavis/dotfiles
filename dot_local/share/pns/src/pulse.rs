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

/// The deepest blue the studio lamps report, operator-locked on 2026-08-31.
/// It is the colour a question waiting on the operator breathes in.
pub const BLOCKED_COLOR: PulseColor = PulseColor {
    x: 0.1532,
    y: 0.0475,
};

/// Daylight, for the unread lamp's SUCCESS flavour: a run that finished while
/// the operator was away. Operator-locked on 2026-08-31.
///
/// A COLOUR NOBODY READS AS AN ALARM, which is the point of it: the red
/// flavour beside it is the one that needs answering, and a success that has
/// merely gone unseen must not compete with it.
pub const UNREAD_SUCCESS_COLOR: PulseColor = PulseColor { x: 0.50, y: 0.40 };

/// Deep violet, picked on the lamp by the operator on 2026-09-01: the loop
/// lamp, breathing while long-running work is in flight.
pub const LOOP_COLOR: PulseColor = PulseColor {
    x: 0.213,
    y: 0.0766,
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

/// What a lamp says about a given exit code.
///
/// ANYTHING that is not all zeroes is a failure, garbage included. An EMPTY
/// code is the absent one: the shell version defaulted a missing argument to
/// zero, so absent and empty both mean success and there is no third answer to
/// give. Unproven success would be a failure, so a caller that cannot prove one
/// passes something that is not all zeroes.
///
/// AN EXIT CODE HAS NO THIRD ANSWER. `pns pulse` and the long-command notifier
/// know a number and nothing else, so they reach two of the five behaviours;
/// the event path knows a STATE and reaches three, through `state_behaviour`.
pub fn exit_behaviour(exit_code: &str) -> crate::config::Behaviour {
    // An empty code has no character that is not a zero, which is the absent
    // case taking the success branch.
    if exit_code.chars().all(|character| character == '0') {
        crate::config::Behaviour::Done
    } else {
        crate::config::Behaviour::Failed
    }
}

/// The states that put a lamp on BLUE: an agent waiting on the operator.
///
/// IT TRADES ONE WORD WITH `missed_notifications::NEEDS_YOU` IN EACH
/// DIRECTION. That constant is right to carry `failed`, because a turn that
/// died needs the operator every bit as much as one that asked; the lamps must
/// tell them apart, since red says it died and blue says it is waiting, so
/// reusing the shared list would paint every failure blue.
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
/// `lamps_are_mapped` IS THE `[lights]` TABLE'S PRESENCE, and blue exists only
/// behind it. Without the map there is one room-shaped pulse and two colours,
/// which is what has shipped since the bash; a long-running turn that ends
/// `blocked` has earned a pulse all along and flashed GREEN for it. Turning
/// that flash blue would be a new behaviour arriving on a machine that wrote no
/// map and asked for nothing.
///
/// IT IS ONE ANSWER RATHER THAN A COLOUR AND A SEPARATE OPT-IN GATE. The
/// composition root asks this once and reads the opt-in off the answer, so
/// "which colour" and "may it fire at all" cannot come out disagreeing.
pub fn state_behaviour(state: &str, lamps_are_mapped: bool) -> crate::config::Behaviour {
    if state == "failed" {
        return crate::config::Behaviour::Failed;
    }
    if lamps_are_mapped && LAMP_BLOCKED.contains(&state) {
        return crate::config::Behaviour::Blocked;
    }
    crate::config::Behaviour::Done
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_LONG_SESSION_SECS, LAMP_BLOCKED, exit_behaviour, session_was_long, state_behaviour,
    };
    use crate::config::Behaviour;

    // --- session_was_long --------------------------------------------------

    #[test]
    fn a_session_past_the_threshold_was_long() {
        assert!(session_was_long(Some(400), Some(300)));
    }

    #[test]
    fn a_session_exactly_at_the_threshold_was_long() {
        assert!(session_was_long(Some(300), Some(300)));
    }

    #[test]
    fn a_session_under_the_threshold_was_not_long() {
        assert!(!session_was_long(Some(299), Some(300)));
    }

    #[test]
    fn an_unreadable_elapsed_time_fails_closed_because_a_missed_pulse_costs_nothing() {
        assert!(!session_was_long(None, Some(300)));
    }

    #[test]
    fn an_unreadable_threshold_fails_closed_too() {
        assert!(!session_was_long(Some(100_000), None));
    }

    #[test]
    fn the_default_threshold_is_five_minutes() {
        assert_eq!(DEFAULT_LONG_SESSION_SECS, 300);
        assert!(session_was_long(Some(300), Some(DEFAULT_LONG_SESSION_SECS)));
        assert!(!session_was_long(
            Some(299),
            Some(DEFAULT_LONG_SESSION_SECS)
        ));
    }

    // --- state_behaviour ---------------------------------------------------

    #[test]
    fn every_needs_you_state_says_needs_you_and_a_failure_says_failed() {
        // THE ONE MAPPING, and the reason the lights do not reuse
        // `missed_notifications::NEEDS_YOU`: that list holds `failed`, which
        // must read RED here. A lamp that painted a dead turn blue would tell
        // the operator to come and answer a question nobody asked.
        for state in ["blocked", "asked", "plan-ready", "denied"] {
            assert_eq!(
                state_behaviour(state, true),
                Behaviour::Blocked,
                "state {state:?} waits on the operator"
            );
        }
        assert_eq!(state_behaviour("failed", true), Behaviour::Failed);
        assert_eq!(state_behaviour("done", true), Behaviour::Done);
    }

    #[test]
    fn a_state_the_lamps_have_no_word_for_reports_done() {
        // EVERY OTHER STATE THAT EARNS A PULSE IS GREEN, which is the shipped
        // rule: today the event path asks whether the state is `failed` and
        // takes the success branch for everything else.
        assert_eq!(state_behaviour("shipped", true), Behaviour::Done);
        assert_eq!(state_behaviour("", true), Behaviour::Done);
    }

    #[test]
    fn the_condensers_own_waiting_word_lights_the_blue_lamp() {
        // `asking` IS A REAL STATE ON EVERY CONDENSED TURN, not a corner. The
        // condenser classifies each one as done, asking or blocked
        // (`hooks::condenser_prompt`), and `asking` is its word for a turn that
        // wants the operator to answer or choose. Read as done, it flashed
        // GREEN, recorded a finished turn as unread SUCCESS news, and ENDED the
        // wait marker instead of starting one.
        assert_eq!(state_behaviour("asking", true), Behaviour::Blocked);
    }

    #[test]
    fn the_lamps_list_drops_the_failure_and_adds_the_condensers_waiting_word() {
        // THE DIVERGENCE, PINNED, and it runs both ways. `failed` is on the
        // shared list and must read RED here, or a dead turn would be painted
        // blue. `asking` reaches only the lamps, because the shared list is the
        // harness's own state words and this one also has to answer for what
        // the condenser writes. So a sixth harness state cannot quietly leave
        // the lamps behind, and nobody can "tidy" the lamps into reusing it.
        let mut shared_traded: Vec<&str> = crate::missed_notifications::NEEDS_YOU
            .iter()
            .copied()
            .filter(|state| *state != "failed")
            .chain(["asking"])
            .collect();
        shared_traded.sort_unstable();
        let mut lamps = LAMP_BLOCKED.to_vec();
        lamps.sort_unstable();
        assert_eq!(lamps, shared_traded);
    }

    #[test]
    fn without_a_lamp_map_a_waiting_agent_reports_done_exactly_as_it_did_before() {
        // THE COMPATIBILITY EDGE, and it is a real event rather than a corner:
        // a LONG-RUNNING turn that ends `blocked` has earned a pulse since the
        // bash, and on a machine with no `[lights]` table it flashed green,
        // because the event path asked one question ("is this failed?") and
        // handed everything else the success branch.
        //
        // BLUE IS A FEATURE OF THE MAP, not of the state word. Without the map
        // there is no third colour to show, no lamp that means "waiting" rather
        // than "finished", and turning that flash blue would be a new behaviour
        // arriving on a machine that asked for nothing.
        for state in LAMP_BLOCKED {
            assert_eq!(
                state_behaviour(state, false),
                Behaviour::Done,
                "state {state:?} with no map"
            );
            assert_eq!(state_behaviour(state, true), Behaviour::Blocked);
        }
        // The failure keeps its colour either way: red predates the map.
        assert_eq!(state_behaviour("failed", false), Behaviour::Failed);
        assert_eq!(state_behaviour("failed", true), Behaviour::Failed);
    }

    // --- exit_behaviour ----------------------------------------------------

    #[test]
    fn a_zero_exit_code_is_done() {
        assert_eq!(exit_behaviour("0"), Behaviour::Done);
    }

    #[test]
    fn a_non_zero_exit_code_is_failed() {
        assert_eq!(exit_behaviour("1"), Behaviour::Failed);
    }

    #[test]
    fn an_exit_code_that_is_not_a_number_is_failed_rather_than_aborting_the_pulse() {
        assert_eq!(exit_behaviour("oops"), Behaviour::Failed);
    }

    #[test]
    fn a_padded_zero_is_still_a_success() {
        assert_eq!(exit_behaviour("00"), Behaviour::Done);
    }

    #[test]
    fn a_signed_zero_is_not_all_zeroes_so_it_is_failed() {
        assert_eq!(exit_behaviour("-0"), Behaviour::Failed);
    }

    #[test]
    fn a_zero_with_whitespace_around_it_is_not_all_zeroes_either() {
        // Tolerating padding is the obvious kindness and it inverts the fail
        // direction: unproven success has to pulse red, so only a code that
        // reads as plainly zero earns green.
        assert_eq!(exit_behaviour(" 0"), Behaviour::Failed);
        assert_eq!(exit_behaviour("0\n"), Behaviour::Failed);
    }

    #[test]
    fn an_absent_exit_code_arrives_as_empty_and_takes_the_success_branch() {
        // The shell version reads a missing argument as zero, so absent and
        // empty are the same input and there is no third answer to give.
        assert_eq!(exit_behaviour(""), Behaviour::Done);
    }
}
