//! The per-lane staleness bound: how many consecutive runs a lane may go
//! without succeeding before its silence is worth an alert of its own.
//!
//! ITS OWN MODULE BECAUSE IT IS NOT PART OF THE RECORD. The record says what
//! ONE run amounted to; this counts ACROSS runs, per lane, and feeds the alert
//! path rather than the entry. A lane that keeps DEFERRING never appears in
//! the record as a failure at all, which is exactly why this exists.

/// How many consecutive runs a lane may go without succeeding before its
/// silence deserves an alert of its own, whatever verdict each of those runs
/// carried. A lane that keeps DEFERRING is silent by design, which is this
/// whole capability's point, so nothing else on the machine would otherwise
/// ever say it stopped running. Three (ORCHESTRATOR ruling, 2026-09-02): long
/// enough that one contended week is not itself an escalation, short enough
/// that a lane does not go quiet for a month before anyone hears about it.
pub const STALE_AFTER_RUNS: u32 = 3;

/// The next value of a lane's non-success streak, and whether THIS run is the
/// one that first crosses `STALE_AFTER_RUNS` since the streak's last reset.
///
/// TRIPS EXACTLY ONCE PER STREAK, on the run where the count reaches the
/// threshold, never on every run after: an already-tripped streak keeps
/// counting (so the record still shows how long it has gone) but does not
/// alert again until a success resets it and a fresh streak climbs back up.
pub fn next_streak(previous: u32, succeeded: bool) -> (u32, bool) {
    if succeeded {
        return (0, false);
    }
    let next = previous.saturating_add(1);
    (next, next == STALE_AFTER_RUNS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_success_resets_the_streak_to_zero_and_never_trips() {
        assert_eq!(next_streak(5, true), (0, false));
        assert_eq!(next_streak(0, true), (0, false));
    }

    #[test]
    fn a_non_success_climbs_the_streak_by_one_without_tripping_below_the_threshold() {
        assert_eq!(next_streak(0, false), (1, false));
        assert_eq!(next_streak(1, false), (2, false));
    }

    #[test]
    fn the_streak_trips_exactly_on_the_run_that_first_reaches_the_threshold() {
        assert_eq!(
            next_streak(STALE_AFTER_RUNS - 1, false),
            (STALE_AFTER_RUNS, true)
        );
    }

    #[test]
    fn a_streak_past_the_threshold_keeps_counting_but_never_trips_again() {
        // "alerting once when it trips, rather than every run after"
        // (ORCHESTRATOR ruling): a mutant that trips on every run at or past
        // the threshold would still pass the test above; this pins the run
        // right after the trip, and one well past it, as both silent.
        assert_eq!(
            next_streak(STALE_AFTER_RUNS, false),
            (STALE_AFTER_RUNS + 1, false)
        );
        assert_eq!(
            next_streak(STALE_AFTER_RUNS + 10, false),
            (STALE_AFTER_RUNS + 11, false)
        );
    }
}
