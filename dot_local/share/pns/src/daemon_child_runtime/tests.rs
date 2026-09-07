mod tests {
    use super::super::*;

    #[test]
    fn a_child_outlives_the_longest_interval_plus_the_write_and_the_reap_that_follow_it() {
        // THE SEAMLESS BREATH ISSUES ITS LAST FADE INSIDE THE BUDGET AND LETS
        // IT FINISH AFTER, so a tick's child is alive for its whole interval,
        // then for however long that last write takes, and it is only noticed
        // as gone on the reap tick after that. Bounded at the interval alone,
        // the supported thirty-second refresh equalled a thirty-second child,
        // and a legal last write was killed before the tick could record where
        // its breath had landed.
        assert_eq!(
            child_bound(Duration::from_secs(1), LIGHTS_JOB),
            Duration::from_secs(37),
            "at the production clock: thirty seconds of interval, the six-second \
             write deadline that ceiling implies, and one reap tick"
        );
        assert_eq!(
            child_bound(Duration::from_secs(60), LIGHTS_JOB),
            Duration::from_secs(1800),
            "and a slow clock keeps the tick-scaled bound, which is the larger of \
             the two there"
        );
        // AND NO OTHER JOB IS WIDENED BY IT. An event delivery's channels each
        // carry their own deadline, so one still alive at `CHILD_TICKS` is
        // wedged; giving it thirty-seven seconds would only delay the kill.
        assert_eq!(
            child_bound(Duration::from_millis(10), "nag:a-session"),
            Duration::from_millis(300),
            "every job but the lights tick keeps the tick-scaled bound exactly"
        );
    }
}
