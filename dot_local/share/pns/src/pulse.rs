//! The light pulse: whether a session earns one, what colour it runs at, and
//! how the light is put back the way a snapshot found it.

/// How long a session must run before a pulse is worth the room's attention.
pub const DEFAULT_LONG_SESSION_SECS: u64 = 300;

/// How long a RESTORE transition takes, in milliseconds: the ONE source both
/// restore arms read. `channels::hue::restore_body` puts it in the CLIP body;
/// `restore_args` below renders it as the `<ms>ms` its flag wants.
///
/// DELIBERATELY SLOWER THAN A PULSE RAMP. The ramps are the alert and want to
/// snap; this is the EXIT from the alert, and the room coming back should be
/// watched rather than cut to. D2 2026-08-12: at 500ms the rise off the
/// near-black final dim was over before it registered.
pub const RESTORE_TRANSITION_MS: u64 = 1200;

/// The colour a pulse runs at: a CIE xy gamut corner plus the peak brightness
/// that colour is pulsed at.
///
/// The coordinates are CIE xy gamut corners rather than RGB because the bridge
/// clamps RGB into its gamut and desaturates hard; xy bypasses that conversion.
/// They stay TEXT so a float formatter can never round one of them into a
/// different corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulseColor {
    pub x: &'static str,
    pub y: &'static str,
    pub peak_brightness: u8,
}

/// Green washes toward white at full brightness (Bezold-Brücke), so it peaks
/// lower and lets the green primary dominate.
pub const SUCCESS_COLOR: PulseColor = PulseColor {
    x: "0.17",
    y: "0.7",
    peak_brightness: 70,
};

/// Red stays saturated at full brightness.
pub const FAILURE_COLOR: PulseColor = PulseColor {
    x: "0.6915",
    y: "0.3083",
    peak_brightness: 100,
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

/// The light-control arguments that put ONE light back the way a snapshot found
/// it, ONE PER ENTRY so a value that ever carries a space survives.
///
/// A light that was off is restored off and told nothing else. Sending it a
/// brightness would turn it on, which is the failure a user actually sees: the
/// pulse ends and a lamp that was dark all evening is now lit. Only the exact
/// text `true` counts as on, so a garbled snapshot restores off.
pub fn restore_args(
    on_state: &str,
    brightness: &str,
    color_mode: &str,
    first_value: &str,
    second_value: &str,
) -> Vec<String> {
    if on_state != "true" {
        return vec![
            "--off".to_string(),
            "--transition-time".to_string(),
            format!("{RESTORE_TRANSITION_MS}ms"),
        ];
    }
    let mut args = vec![
        "--on".to_string(),
        "--brightness".to_string(),
        brightness.to_string(),
    ];
    if color_mode == "ct" {
        args.extend(["-t".to_string(), first_value.to_string()]);
    } else {
        args.extend([
            "-x".to_string(),
            first_value.to_string(),
            "-y".to_string(),
            second_value.to_string(),
        ]);
    }
    args.extend([
        "--transition-time".to_string(),
        format!("{RESTORE_TRANSITION_MS}ms"),
    ]);
    args
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_LONG_SESSION_SECS, FAILURE_COLOR, SUCCESS_COLOR, pulse_color, restore_args,
        session_was_long,
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
        assert_eq!(SUCCESS_COLOR.x, "0.17");
        assert_eq!(SUCCESS_COLOR.y, "0.7");
        assert_eq!(SUCCESS_COLOR.peak_brightness, 70);
    }

    #[test]
    fn a_non_zero_exit_code_pulses_the_red_gamut_corner() {
        assert_eq!(pulse_color("1"), FAILURE_COLOR);
        assert_eq!(FAILURE_COLOR.x, "0.6915");
        assert_eq!(FAILURE_COLOR.y, "0.3083");
        assert_eq!(FAILURE_COLOR.peak_brightness, 100);
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

    // --- restore_args ------------------------------------------------------

    #[test]
    fn a_light_in_colour_temperature_mode_is_restored_by_its_mirek_value() {
        assert_eq!(
            restore_args("true", "80", "ct", "366", ""),
            [
                "--on",
                "--brightness",
                "80",
                "-t",
                "366",
                "--transition-time",
                "1200ms"
            ]
        );
    }

    #[test]
    fn a_light_in_xy_mode_is_restored_by_both_coordinates() {
        assert_eq!(
            restore_args("true", "80", "xy", "0.55", "0.31"),
            [
                "--on",
                "--brightness",
                "80",
                "-x",
                "0.55",
                "-y",
                "0.31",
                "--transition-time",
                "1200ms"
            ]
        );
    }

    #[test]
    fn a_light_that_was_off_is_restored_off_never_to_a_brightness() {
        // Sending a brightness would turn it back on, which is the outcome the
        // operator sees: the pulse ends and a lamp that was dark all evening is
        // lit.
        assert_eq!(
            restore_args("false", "42", "xy", "0.55", "0.31"),
            ["--off", "--transition-time", "1200ms"]
        );
    }

    #[test]
    fn an_on_state_that_is_not_exactly_true_is_restored_off() {
        // The last two are the prefix near-misses. The snapshot arrives as
        // text, so a comparison that only checks the head reads a trailing
        // newline as on and lights a lamp the snapshot found dark.
        for garbled in ["TRUE", "True", "1", "", "yes", "true\n", "truex"] {
            assert_eq!(
                restore_args(garbled, "42", "xy", "0.55", "0.31"),
                ["--off", "--transition-time", "1200ms"],
                "{garbled} must not be read as on"
            );
        }
    }

    #[test]
    fn any_colour_mode_that_is_not_colour_temperature_is_restored_by_coordinates() {
        // "ctx" and "ct\n" are the prefix near-misses: colour temperature is
        // the exact word, not anything that opens with it.
        for not_ct in ["garbled", "ctx", "ct\n"] {
            assert_eq!(
                restore_args("true", "80", not_ct, "0.55", "0.31"),
                [
                    "--on",
                    "--brightness",
                    "80",
                    "-x",
                    "0.55",
                    "-y",
                    "0.31",
                    "--transition-time",
                    "1200ms"
                ],
                "{not_ct} must not be read as colour temperature"
            );
        }
    }
}
