mod tests {
    use super::super::*;

    #[test]
    fn every_reread_interval_that_is_not_a_duration_falls_back_to_the_default() {
        // The float shapes here panicked `Duration::from_secs_f64` outright or,
        // finite and non-negative, passed the guard written for the others and
        // panicked in the constructor anyway (exit 101 on a hook whose whole
        // contract is exiting 0). `0.05` and `2` are what the variable used to
        // be typed as, and a bare number is now ambiguous rather than seconds.
        for raw in [
            "NaN",
            "inf",
            "-inf",
            "-1",
            "not-a-number",
            "",
            "1e30",
            "1e300",
            "0.05",
            "2",
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
        // never hours. The interval is REFUSED above its own rather than
        // clamped to it, because a duration silently moved is a wait the
        // operator believes they set.
        assert_eq!(
            reread_interval_from(Some("1000000s")),
            DEFAULT_REREAD_INTERVAL
        );
        assert_eq!(
            reread_attempts_from(Some("4294967295")),
            MAX_REREAD_ATTEMPTS
        );
        assert_eq!(reread_attempts_from(Some("11")), MAX_REREAD_ATTEMPTS);
    }

    #[test]
    fn a_reread_knob_inside_its_ceiling_is_taken_as_written() {
        assert_eq!(
            reread_interval_from(Some("250ms")),
            Duration::from_millis(250)
        );
        assert_eq!(reread_interval_from(Some("5s")), MAX_REREAD_INTERVAL);
        assert_eq!(reread_interval_from(Some("0s")), Duration::ZERO);
        assert_eq!(reread_attempts_from(Some("2")), 2);
        assert_eq!(reread_attempts_from(Some("0")), 0);
        assert_eq!(reread_attempts_from(None), DEFAULT_REREAD_ATTEMPTS);
    }
}
