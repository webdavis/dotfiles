//! The per-lane deadline: how long one lane may run before uu stops it.
//!
//! WHY EVERY LANE HAS ONE BY DEFAULT. `uu run` takes a non-blocking whole-run
//! `flock` and holds it across lane execution, so a lane that never ends is a
//! lock that is never released and every later run does nothing at all. A
//! default is what bounds the lanes nobody thought about, and those are the
//! ones that produce this failure.

use std::time::Duration;

/// SIX HOURS, chosen against both ends of the range it has to sit between.
///
/// The floor is the slowest HONEST lane. A full `brew upgrade` downloads and
/// installs multi-gigabyte casks and can build from source, and the herdr lane
/// reinstalls every plugin from its source's tip, which compiles. NOTHING IN
/// THIS REPO TIMES EITHER, so an hour or two is an ESTIMATE and not a
/// measurement; six hours is picked to sit well clear of it rather than close
/// to it, because a deadline near the honest run time turns a slow week into
/// an outage uu inflicted on itself.
///
/// The ceiling is the SCHEDULE. These jobs run weekly, so anything well under
/// 168 hours hands the lock back long before the next slot. A lane that
/// genuinely needs longer states its own `deadline_secs`. What bounds the
/// whole locked run, which is a different question, is `RUN_DEADLINE`.
pub const DEFAULT_LANE_DEADLINE: Duration = Duration::from_secs(6 * 60 * 60);

/// TWENTY-FOUR HOURS for the WHOLE RUN, because a per-lane bound does not
/// bound the run. One `flock` covers every lane, lanes run in sequence, and
/// nothing caps how many a config declares, so five defaulted lanes would hold
/// that lock for thirty hours and twenty-nine for a week. The next weekly slot
/// would then find the lock still held and do nothing, which is the exact
/// failure the per-lane deadline exists to prevent.
///
/// A day is a seventh of the period and longer than every lane's honest work
/// put together, so it fires on a run that has gone wrong and on nothing else.
/// Not configurable: a machine that needs a longer weekly run needs fewer
/// lanes per lock, which is the per-lane invocation capability, not a bigger
/// number here.
pub const RUN_DEADLINE: Duration = Duration::from_secs(24 * 60 * 60);

/// What one lane may actually have: its own deadline, or all that is left of
/// the run's, whichever is less.
pub fn lane_budget(declared: Duration, run_elapsed: Duration) -> Duration {
    declared.min(RUN_DEADLINE.saturating_sub(run_elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lane_gets_its_own_deadline_while_the_run_has_room_for_it() {
        assert_eq!(
            lane_budget(Duration::from_secs(90), Duration::ZERO),
            Duration::from_secs(90)
        );
    }

    #[test]
    fn a_lane_is_cut_to_what_is_left_of_the_run_rather_than_extending_it() {
        // The run holds ONE lock across every lane, so a lane starting late
        // gets the remainder and not its own full deadline.
        assert_eq!(
            lane_budget(
                DEFAULT_LANE_DEADLINE,
                RUN_DEADLINE - Duration::from_secs(60)
            ),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn a_run_already_past_its_own_deadline_leaves_a_lane_nothing() {
        assert!(lane_budget(DEFAULT_LANE_DEADLINE, RUN_DEADLINE * 2).is_zero());
    }
}
