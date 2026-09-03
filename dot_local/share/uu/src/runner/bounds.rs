//! Running ONE step under a bound of its own, and every test that puts a real
//! child up against a clock.
//!
//! Its own file because it is its own question. `runner` decides what a child
//! ran and what it printed; this decides how long any of it may take, and the
//! tests that answer that have to spawn real children and wait on them, which
//! is nothing the rest of that module does.

use std::time::Duration;

use unattended_upgrades::lanes::CommandRunner;

use super::SystemRunner;

/// `run`, under the smaller of the step's own bound and what is left of the
/// lane's.
///
/// THE LANE DEADLINE IS THE WHOLE LANE'S, so a step that takes all of it costs
/// every step after it. A subject known to wedge rather than fail (the App
/// Store hangs indefinitely on a broken session) is bounded here instead, so
/// one wedged step costs itself and the rest of the lane still runs.
///
/// A SECOND RUNNER FOR THE ONE STEP, holding that bound and its own clock.
/// Delegating to `run` keeps every overrun sentence composed in one place, and
/// naming the step in the label stops a step's bound being reported as the
/// lane's. The lane's own clock is untouched, so the step is still charged
/// against it.
///
/// DECLARED IS THE EFFECTIVE BOUND, not the step's own request. A step bound
/// is not a `deadline_secs` and no config key carries it, so declaring a
/// larger value would print a setting the operator can never find; the number
/// in the sentence is always the bound that actually expired.
pub fn run_step(
    lane: &SystemRunner,
    program: &str,
    args: &[&str],
    most: Duration,
) -> Result<String, String> {
    let bound = most.min(lane.remaining());
    SystemRunner::for_lane(&format!("{} step {program}", lane.lane), bound, bound)
        .run(program, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watchdog::tests::within;
    use unattended_upgrades::lanes::Verdict;

    /// A runner whose budget no honest test child comes near, so what
    /// expires is the STEP's own bound and nothing else.
    fn roomy() -> SystemRunner {
        SystemRunner::for_lane("test", Duration::from_secs(30), Duration::from_secs(30))
    }

    /// A runner whose budget is spent almost at once, so a deadline test is
    /// over in a fraction of a second.
    fn impatient(lane: &str) -> SystemRunner {
        SystemRunner::for_lane(lane, Duration::from_millis(200), Duration::from_millis(200))
    }

    #[test]
    fn a_child_that_runs_past_the_lane_deadline_fails_naming_it() {
        let failure = within(Duration::from_secs(3), || {
            impatient("slow").run("/bin/sh", &["-c", "sleep 30"])
        })
        .expect_err("this command outlives its lane's deadline");
        assert!(failure.contains("lane `slow`"), "{failure}");
        assert!(failure.contains("200ms deadline"), "{failure}");
    }

    #[test]
    fn a_child_that_exits_while_a_grandchild_holds_the_pipe_still_hits_the_deadline() {
        // THE HANG THIS EXISTS FOR, and it is not simply a slow child: the
        // child exits at once and something it left behind keeps stdout open,
        // so waiting on the child returns immediately and the READ is what
        // blocks. A deadline that only bounded the wait would not bound this
        // at all.
        //
        // THE GRANDCHILD OUTLIVES THE WHOLE WATCHDOG on purpose. At 30 seconds
        // it cannot exit on its own inside the deadline plus both kill graces,
        // so a run that finishes here finished because something killed it.
        let ran = within(Duration::from_secs(3), || {
            impatient("orphan").run_with_input(
                "/bin/sh",
                &["-c", "sleep 30 & printf 'got this far\\n'; exit 0"],
                "the run event\n",
            )
        })
        .expect("the child ran, it just left something behind");
        // WHAT IT PRINTED IS KEPT. Those lines are how far the lane got, and
        // a mutant that dropped stdout on the overrun path alone would satisfy
        // every assertion below.
        assert_eq!(ran.stdout, "got this far\n");
        let Verdict::Failed(reason) = ran.verdict else {
            panic!("an overrun is a failure, not {:?}", ran.verdict);
        };
        assert!(reason.contains("lane `orphan`"), "{reason}");
        assert!(reason.contains("200ms deadline"), "{reason}");
    }

    #[test]
    fn a_lane_that_spent_its_budget_refuses_the_next_command_without_running_it() {
        // The budget belongs to the LANE, not to each spawn: the herdr lane
        // alone spawns two commands per plugin, and a bound that reset every
        // time would let a long roster hold the run lock for a multiple of the
        // deadline the operator wrote.
        let runner = impatient("spent");
        std::thread::sleep(Duration::from_millis(250));
        // A PROGRAM THAT IS NOT THERE, so the refusal proves nothing was
        // spawned: a runner that attempted the spawn would report the missing
        // program instead of the deadline it had already blown.
        let refused = runner
            .run("/no/such/uu-test-program", &[])
            .expect_err("nothing may run once the lane is out of time");
        assert!(refused.contains("lane `spent`"), "{refused}");
        assert!(refused.contains("200ms deadline"), "{refused}");
        assert!(!refused.contains("could not run"), "{refused}");
    }

    #[test]
    fn a_step_with_its_own_bound_is_killed_at_that_bound_rather_than_holding_the_lane() {
        // The App Store wedges indefinitely on a broken session, and the lane
        // deadline is the WHOLE lane's: a step that takes all of it costs
        // every step after it. A step bound cannot be reported as the lane's,
        // either, or the operator is sent to a `deadline_secs` that reads
        // correct.
        let runner = roomy();
        let failure = within(Duration::from_secs(5), move || {
            runner.run_with_deadline("/bin/sh", &["-c", "sleep 30"], Duration::from_millis(200))
        })
        .expect_err("a step that outlives its own bound is stopped there");
        assert!(failure.contains("200ms"), "{failure}");
        assert!(failure.contains("step"), "{failure}");
        assert!(failure.contains("killed"), "{failure}");
    }

    #[test]
    fn a_step_bound_larger_than_the_lane_has_left_does_not_extend_the_lane() {
        // THE STEP BOUND IS A CEILING, NEVER A GRANT. Whichever of the two is
        // smaller is the one that expires, or a 180-second App Store step on
        // a lane with milliseconds left would hold the run lock well past the
        // deadline the operator wrote, and every lane after it in name order
        // would pay for it.
        let failure = within(Duration::from_secs(5), || {
            impatient("brew").run_with_deadline(
                "/bin/sh",
                &["-c", "sleep 30"],
                Duration::from_secs(30),
            )
        })
        .expect_err("the lane's remaining time is what expires here");
        assert!(failure.contains("brew step /bin/sh"), "{failure}");
        assert!(failure.contains("killed"), "{failure}");
        // NEVER THE STEP'S OWN 30 SECONDS, and never a `deadline_secs` the
        // operator could go looking for: the number in the sentence has to be
        // the bound that actually expired.
        assert!(!failure.contains("30s"), "{failure}");
        assert!(!failure.contains("deadline_secs"), "{failure}");
    }

    #[test]
    fn a_step_that_finishes_inside_its_bound_answers_with_what_it_printed() {
        // The positive control: a mutant that failed every bounded step would
        // satisfy the timeout test above on its own.
        let stdout = roomy()
            .run_with_deadline(
                "/bin/sh",
                &["-c", "printf '3 upgraded\\n'"],
                Duration::from_secs(5),
            )
            .expect("a quick command runs well inside its bound");
        assert_eq!(stdout, "3 upgraded\n");
    }

    #[test]
    fn a_lane_cut_short_by_the_run_says_so_rather_than_naming_its_own_setting() {
        // One lock covers every lane, so a lane starting late gets what is
        // left of the RUN rather than its own deadline. Reporting that as
        // "exceeded its 200ms deadline" would send the operator to a config
        // key that says 21600 and looks correct.
        let runner = SystemRunner::for_lane(
            "cut",
            Duration::from_millis(200),
            Duration::from_secs(21600),
        );
        let failure = within(Duration::from_secs(3), move || {
            runner.run("/bin/sh", &["-c", "sleep 30"])
        })
        .expect_err("this command outlives what the run had left");
        assert!(failure.contains("run's budget"), "{failure}");
        assert!(failure.contains("deadline_secs is 21600s"), "{failure}");
    }
}
