//! The pulse policy's own tests: which sessions earn a light, and what a
//! lamp says about an exit code and about an event state.

use super::{
    DEFAULT_LONG_SESSION_SECS, LAMP_BLOCKED, exit_behaviour, session_was_long, state_behaviour,
};
use crate::lamps::config::Behaviour;

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
fn every_waiting_state_says_blocked_and_a_failure_says_failed() {
    // THE ONE MAPPING, and the reason the lights do not reuse
    // `missed_notifications::NEEDS_YOU`: that list holds `failed`, which
    // must read RED here. A lamp that held a dead turn blocked would tell
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
fn the_condensers_own_waiting_word_lights_the_blocked_lamp() {
    // `asking` IS A REAL STATE ON EVERY CONDENSED TURN, not a corner. The
    // condenser classifies each one as done, asking or blocked
    // (`hooks::condenser_prompt`), and `asking` is its word for a turn that
    // wants the operator to answer or choose. Read as done, it flashed
    // GREEN, recorded a finished turn as unread SUCCESS news, and ENDED the
    // wait marker instead of starting one.
    assert_eq!(state_behaviour("asking", true), Behaviour::Blocked);
}

#[test]
fn without_a_lamp_map_a_waiting_agent_reports_done_exactly_as_it_did_before() {
    // THE COMPATIBILITY EDGE, and it is a real event rather than a corner:
    // a LONG-RUNNING turn that ends `blocked` has earned a pulse since the
    // bash, and on a machine with no `[lights]` table it flashed green,
    // because the event path asked one question ("is this failed?") and
    // handed everything else the success branch.
    //
    // THE BLOCKED LAMP IS A FEATURE OF THE MAP, not of the state word.
    // Without the map there is no third colour to show, no lamp that means
    // "waiting" rather than "finished", and turning that flash into the
    // blocked colour would be a new behaviour arriving on a machine that
    // asked for nothing.
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
    assert_eq!(exit_behaviour("0"), Some(Behaviour::Done));
}

#[test]
fn a_non_zero_exit_code_is_failed() {
    assert_eq!(exit_behaviour("1"), Some(Behaviour::Failed));
}

#[test]
fn an_exit_code_that_is_not_a_number_is_refused_rather_than_guessed_at() {
    // H-C: garbage used to take the failure branch, which flashed the
    // room red on a code nobody proved. `pulse_mode` reads this `None` as
    // a refusal with usage instead.
    assert_eq!(exit_behaviour("oops"), None);
}

#[test]
fn an_arabic_indic_digit_is_refused_because_the_boundary_is_ascii_only() {
    // Kills the `is_ascii_digit` -> `is_numeric` mutant: `char::is_numeric`
    // is true for "١" (Arabic-Indic one, U+0661), which would turn a
    // non-ASCII digit into a real failure pulse instead of a refusal.
    assert_eq!(exit_behaviour("١"), None);
}

#[test]
fn a_padded_zero_is_still_a_success() {
    assert_eq!(exit_behaviour("00"), Some(Behaviour::Done));
}

#[test]
fn a_signed_zero_is_refused_rather_than_read_as_a_failure() {
    // H-C: `-0` is not all ASCII digits, so it is no longer a code this
    // function will read at all, refused rather than guessed as failed.
    assert_eq!(exit_behaviour("-0"), None);
}

#[test]
fn a_zero_with_whitespace_around_it_is_refused_the_same_way() {
    // H-C: padding is not a digit either, so both read as `None` and
    // `pulse_mode` refuses them instead of pulsing red on unproven input.
    assert_eq!(exit_behaviour(" 0"), None);
    assert_eq!(exit_behaviour("0\n"), None);
}

#[test]
fn an_absent_exit_code_arrives_as_empty_and_takes_the_success_branch() {
    // The shell version reads a missing argument as zero, so absent and
    // empty are the same input and there is no third answer to give.
    assert_eq!(exit_behaviour(""), Some(Behaviour::Done));
}
