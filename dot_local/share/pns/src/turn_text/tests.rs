mod tests {
    use super::super::*;

    #[test]
    fn every_reread_interval_that_is_not_a_duration_falls_back_to_the_default() {
        // The first four panicked `Duration::from_secs_f64` outright. The last
        // two are FINITE and non-negative, so they passed the guard written
        // for the others and panicked in the constructor anyway (exit 101 on
        // a hook whose whole contract is exiting 0).
        for raw in [
            "NaN",
            "inf",
            "-inf",
            "-1",
            "not-a-number",
            "",
            "1e30",
            "1e300",
        ] {
            assert_eq!(
                reread_interval_from(Some(raw)),
                DEFAULT_REREAD_INTERVAL,
                "interval {raw:?}"
            );
        }
        assert_eq!(reread_interval_from(None), DEFAULT_REREAD_INTERVAL);
    }

    #[test]
    fn an_oversized_reread_knob_is_clamped_rather_than_believed() {
        // Both knobs multiply into how long a Stop hook can hold a turn's
        // report open, so each has a ceiling: a stray zero must cost seconds,
        // never hours.
        assert_eq!(reread_interval_from(Some("1000000")), MAX_REREAD_INTERVAL);
        assert_eq!(
            reread_attempts_from(Some("4294967295")),
            MAX_REREAD_ATTEMPTS
        );
        assert_eq!(reread_attempts_from(Some("11")), MAX_REREAD_ATTEMPTS);
    }

    #[test]
    fn a_reread_knob_inside_its_ceiling_is_taken_as_written() {
        assert_eq!(
            reread_interval_from(Some("0.25")),
            Duration::from_millis(250)
        );
        assert_eq!(reread_interval_from(Some("0")), Duration::ZERO);
        assert_eq!(reread_attempts_from(Some("2")), 2);
        assert_eq!(reread_attempts_from(Some("0")), 0);
        assert_eq!(reread_attempts_from(None), DEFAULT_REREAD_ATTEMPTS);
    }
}
