use super::*;

#[test]
fn pending_escalation_trips_only_at_the_configured_threshold() {
    for threshold in [1, 2, 3, 9, u32::MAX] {
        let threshold = NonZeroU32::new(threshold).unwrap();
        let at = threshold.get();
        if at > 1 {
            assert_eq!(
                next_pending_streak(at - 2, true, threshold),
                (at - 1, false)
            );
        }
        assert_eq!(next_pending_streak(at - 1, true, threshold), (at, true));
        assert_eq!(
            next_pending_streak(at, true, threshold),
            (at.saturating_add(1), false)
        );
    }
}

#[test]
fn a_non_pending_verdict_resets_pending_history_even_at_saturation() {
    for previous in [0, 2, 3, u32::MAX] {
        assert_eq!(
            next_pending_streak(previous, false, DEFAULT_ESCALATE_AFTER_RUNS),
            (0, false)
        );
    }
}
