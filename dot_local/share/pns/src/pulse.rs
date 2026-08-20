//! The light pulse: whether a session earns one and what colour it runs at.
//! Putting the light back is the BRIDGE's job now, not ours: the hue channel
//! asks for a timed signal and the bridge restores the room when it ends.

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

/// The colour a pulse runs at for a given exit code.
///
/// ANYTHING that is not all zeroes is a failure, garbage included. An EMPTY
/// code is the absent one: the shell version defaulted a missing argument to
/// zero, so absent and empty both mean success and there is no third answer to
/// give. Unproven success would be a failure, so a caller that cannot prove one
/// passes something that is not all zeroes.
pub fn pulse_color(exit_code: &str) -> PulseColor {
    // An empty code has no character that is not a zero, which is the absent
    // case taking the success branch.
    if exit_code.chars().all(|character| character == '0') {
        SUCCESS_COLOR
    } else {
        FAILURE_COLOR
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_LONG_SESSION_SECS, FAILURE_COLOR, SUCCESS_COLOR, pulse_color, session_was_long,
    };

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

    // --- pulse_color -------------------------------------------------------

    #[test]
    fn a_zero_exit_code_pulses_the_green_gamut_corner() {
        assert_eq!(pulse_color("0"), SUCCESS_COLOR);
        assert_eq!(SUCCESS_COLOR.x, 0.2151);
        assert_eq!(SUCCESS_COLOR.y, 0.7106);
    }

    #[test]
    fn a_non_zero_exit_code_pulses_the_red_gamut_corner() {
        assert_eq!(pulse_color("1"), FAILURE_COLOR);
        assert_eq!(FAILURE_COLOR.x, 0.675);
        assert_eq!(FAILURE_COLOR.y, 0.322);
    }

    #[test]
    fn an_exit_code_that_is_not_a_number_pulses_red_rather_than_aborting_the_pulse() {
        assert_eq!(pulse_color("oops"), FAILURE_COLOR);
    }

    #[test]
    fn a_padded_zero_is_still_a_success() {
        assert_eq!(pulse_color("00"), SUCCESS_COLOR);
    }

    #[test]
    fn a_signed_zero_is_not_all_zeroes_so_it_pulses_red() {
        assert_eq!(pulse_color("-0"), FAILURE_COLOR);
    }

    #[test]
    fn a_zero_with_whitespace_around_it_is_not_all_zeroes_either() {
        // Tolerating padding is the obvious kindness and it inverts the fail
        // direction: unproven success has to pulse red, so only a code that
        // reads as plainly zero earns green.
        assert_eq!(pulse_color(" 0"), FAILURE_COLOR);
        assert_eq!(pulse_color("0\n"), FAILURE_COLOR);
    }

    #[test]
    fn an_absent_exit_code_arrives_as_empty_and_takes_the_success_branch() {
        // The shell version reads a missing argument as zero, so absent and
        // empty are the same input and there is no third answer to give.
        assert_eq!(pulse_color(""), SUCCESS_COLOR);
    }
}
