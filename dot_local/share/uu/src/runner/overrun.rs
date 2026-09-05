//! What the record and the alert say when a lane ran out of time.
//!
//! ITS OWN FILE BECAUSE IT IS PURE. Every sentence here is a total function of
//! the lane's name, the budget it actually had and the one its block declared,
//! so the wording is pinned without spawning anything.

use std::time::Duration;

use unattended_upgrades::lanes::failure_reason;

use crate::watchdog::Ended;

/// How this lane ran out of time, naming the RUN when the run's remaining
/// budget is what cut the lane short rather than the lane's own setting.
pub fn out_of_time(lane: &str, budget: Duration, declared: Duration) -> String {
    if budget < declared {
        format!(
            "lane `{lane}` was stopped at {budget:?}, all that was left of the run's budget (its \
             own deadline_secs is {}s)",
            declared.as_secs()
        )
    } else {
        format!("lane `{lane}` exceeded its {budget:?} deadline")
    }
}

/// The whole failure line for a lane the deadline stopped. The stderr tail
/// rides along the way it does on every other failure: the child is gone by
/// the time the record is composed, and what it printed on the way to the
/// deadline is the only clue to where it stopped.
///
/// AN UNVERIFIED KILL IS NEVER REPORTED AS ONE. `Escaped` means something
/// outlived TERM and KILL and may still be running and still writing after uu
/// drops the run lock, which is a fact the operator has to be handed rather
/// than one dressed up as a clean stop.
pub fn overrun(
    lane: &str,
    budget: Duration,
    declared: Duration,
    ended: &Ended,
    stderr: &[u8],
) -> String {
    let expired = out_of_time(lane, budget, declared);
    let how = match ended {
        Ended::Escaped => format!(
            "{expired}; something it left behind outlived TERM and KILL and may still be running"
        ),
        _ => format!("{expired}, so its process group was killed"),
    };
    failure_reason(&how, &String::from_utf8_lossy(stderr))
}

/// The line for a spawn that never returned. No pid exists in that case, so
/// nothing could be signalled and only the caller giving up bounded it.
pub fn spawn_stuck(lane: &str, budget: Duration, declared: Duration, program: &str) -> String {
    format!(
        "{}, and the spawn of {program} never returned, so there was no pid to signal",
        out_of_time(lane, budget, declared)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIX: Duration = Duration::from_secs(6);

    #[test]
    fn a_lane_that_spent_its_own_deadline_names_that_deadline_and_not_the_run() {
        let said = out_of_time("brew", SIX, SIX);
        assert!(said.contains("exceeded its"), "{said}");
        assert!(!said.contains("run's budget"), "{said}");
    }

    #[test]
    fn a_lane_cut_short_by_the_run_names_the_run_and_still_states_its_own_setting() {
        // The two numbers must not be confused: the operator reads this to
        // decide whether to raise a lane's own deadline_secs, and a lane the
        // RUN cut short would not be helped by raising it.
        let said = out_of_time("brew", Duration::from_secs(1), SIX);
        assert!(
            said.contains("all that was left of the run's budget"),
            "{said}"
        );
        assert!(said.contains("deadline_secs is 6s"), "{said}");
    }

    #[test]
    fn an_escaped_group_is_never_reported_as_a_kill_that_worked() {
        let escaped = overrun("brew", SIX, SIX, &Ended::Escaped, b"");
        assert!(escaped.contains("may still be running"), "{escaped}");
        assert!(!escaped.contains("was killed"), "{escaped}");
        let stopped = overrun("brew", SIX, SIX, &Ended::Stopped, b"");
        assert!(stopped.contains("process group was killed"), "{stopped}");
    }

    #[test]
    fn an_overrun_carries_the_tail_of_what_the_child_printed_on_stderr() {
        let said = overrun("brew", SIX, SIX, &Ended::Stopped, b"the last thing it said");
        assert!(said.contains("the last thing it said"), "{said}");
    }
}
