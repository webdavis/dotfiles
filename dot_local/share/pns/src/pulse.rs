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

pub const SUCCESS_COLOR: PulseColor = PulseColor {
    x: 0.2151,
    y: 0.7106,
};

pub const FAILURE_COLOR: PulseColor = PulseColor { x: 0.675, y: 0.322 };

/// The two colours a needs-you signal alternates between: TWO DEEP BLUES.
///
/// THE SECOND ONE WAS A GREEN-BLUE UNTIL THE 2026-09-01 DRILL. Alternating a
/// blue with a colour that far from it read as a colour CHANGE rather than as
/// one lamp saying one thing, and the operator asked for a deeper blue, so the
/// alternate moved next door to the primary. The pair still alternates, which
/// is what keeps a wait from being mistaken for a `done` at a glance; what it
/// no longer does is look like two different messages.
///
/// NEITHER IS APPROVED AS SEEN YET, and that is the honest state rather than
/// an oversight. Green and red above passed the only test a colour can pass,
/// which is the operator looking at the lamp; these two have been chosen for
/// the gamut the studio lamps report (type C) and not yet looked at. The
/// operator's post-apply eye is the last step, and a change afterwards is
/// THESE TWO CONSTANTS and nothing else.
pub const NEEDS_YOU_COLOR: PulseColor = PulseColor {
    x: 0.1532,
    y: 0.0475,
};

pub const NEEDS_YOU_ALT_COLOR: PulseColor = PulseColor { x: 0.15, y: 0.06 };

/// The loop lamp's GLOW colour: magenta.
///
/// THE GLOW'S ALONE, and breathing has no colour of its own by design. The
/// bridge's native breathe swells around whatever the lamp is already showing,
/// so a colour here would be one this crate never gets to state. That is the
/// operator's decision of 2026-08-30 read backwards: the two states are one
/// lamp saying one thing, and only the steady one has to pick a hue.
///
/// IN GAMUT AND NOT YET APPROVED AS SEEN, exactly like the two blues above: it
/// sits inside the type C triangle the studio lamps report, between their red
/// and blue primaries and pulled off that edge toward white, and the
/// operator's post-apply eye is what settles it. A change is this one
/// constant.
pub const LOOP_COLOR: PulseColor = PulseColor { x: 0.40, y: 0.19 };

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
/// FOUR WORDS, AND `missed_notifications::NEEDS_YOU` HAS FIVE. That constant
/// is right to carry `failed`, because a turn that died needs the operator
/// every bit as much as one that asked. The lamps must tell them apart: red
/// says it died, blue says it is waiting, and reusing the shared list would
/// paint every failure blue. This is the honest divergence rather than a
/// silent one, and a test pins the two lists as differing by exactly that word.
pub const LAMP_NEEDS_YOU: [&str; 4] = ["blocked", "asked", "plan-ready", "denied"];

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
    if lamps_are_mapped && LAMP_NEEDS_YOU.contains(&state) {
        return crate::config::Behaviour::NeedsYou;
    }
    crate::config::Behaviour::Done
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_LONG_SESSION_SECS, FAILURE_COLOR, LAMP_NEEDS_YOU, SUCCESS_COLOR, exit_behaviour,
        session_was_long, state_behaviour,
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
                Behaviour::NeedsYou,
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
        assert_eq!(state_behaviour("asking", true), Behaviour::Done);
        assert_eq!(state_behaviour("", true), Behaviour::Done);
    }

    #[test]
    fn the_lamps_needs_you_list_is_the_shared_one_minus_the_failure() {
        // THE DIVERGENCE, PINNED. The two lists are deliberately different and
        // the difference is exactly one word, so a sixth state joining the
        // shared list cannot quietly leave the lamps behind, and nobody can
        // "tidy" the lamps into reusing it.
        let shared_minus_failed: Vec<&str> = crate::missed_notifications::NEEDS_YOU
            .iter()
            .copied()
            .filter(|state| *state != "failed")
            .collect();
        let mut lamps = LAMP_NEEDS_YOU.to_vec();
        lamps.sort_unstable();
        assert_eq!(lamps, shared_minus_failed);
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
        for state in LAMP_NEEDS_YOU {
            assert_eq!(
                state_behaviour(state, false),
                Behaviour::Done,
                "state {state:?} with no map"
            );
            assert_eq!(state_behaviour(state, true), Behaviour::NeedsYou);
        }
        // The failure keeps its colour either way: red predates the map.
        assert_eq!(state_behaviour("failed", false), Behaviour::Failed);
        assert_eq!(state_behaviour("failed", true), Behaviour::Failed);
    }

    // --- exit_behaviour ----------------------------------------------------

    #[test]
    fn a_zero_exit_code_is_done_and_green_is_the_colour_it_carries() {
        assert_eq!(exit_behaviour("0"), Behaviour::Done);
        assert_eq!(SUCCESS_COLOR.x, 0.2151);
        assert_eq!(SUCCESS_COLOR.y, 0.7106);
    }

    #[test]
    fn a_non_zero_exit_code_is_failed_and_red_is_the_colour_it_carries() {
        assert_eq!(exit_behaviour("1"), Behaviour::Failed);
        assert_eq!(FAILURE_COLOR.x, 0.675);
        assert_eq!(FAILURE_COLOR.y, 0.322);
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
