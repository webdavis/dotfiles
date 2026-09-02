//! The lane subjects, spawned under the deadline their lane declared.
//!
//! THE ONE PROCESS BOUNDARY THAT SPAWNS. Everything the library decides is a
//! total function of its arguments; this is where a lane subject actually
//! runs, which is why the whole watchdog lives here rather than beside the
//! policy it enforces.

use std::io::Write;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use unattended_upgrades::lanes::{CommandRunner, DEFERRED_EXIT_CODE, Ran, Verdict, failure_reason};

use crate::watchdog::{Ended, Finished, Spawned, bounded_spawn};

/// The event handed to a command lane's child cannot exceed this, or
/// `run_with_input`'s pre-filled pipe would have to write more than fits
/// before a reader exists. XNU's own floor is 16 KiB; measured capacity on
/// Darwin 25.2 is 64 KiB. The limit sits AT the floor, the one size a whole
/// event is guaranteed to fit at before any reader exists.
/// Past capacity `write_all` blocks forever with no reader and no deadline: a
/// silent hang of an unattended job. The event uu composes today is under
/// 1 KiB.
pub const MAX_EVENT_INPUT: usize = 16 * 1024;

/// The lane subjects, each spawn bounded by what is LEFT of its lane's
/// deadline.
///
/// ONE BUDGET FOR THE WHOLE LANE, not one per spawn. The herdr lane spawns
/// two commands per plugin on top of its own self-update, so a per-spawn
/// bound would let a twenty-plugin roster hold the run lock for sixty
/// deadlines and the lock is the thing this exists to protect.
pub struct SystemRunner {
    lane: String,
    /// What this lane may actually have: its own `deadline_secs`, or all that
    /// was left of the run's budget when it started.
    budget: Duration,
    /// What its own block declared, kept only so an overrun can say when the
    /// RUN is what cut it short rather than its own setting.
    declared: Duration,
    started: Instant,
}

impl SystemRunner {
    /// The runner for one lane, its clock starting now.
    pub fn for_lane(lane: &str, budget: Duration, declared: Duration) -> Self {
        SystemRunner {
            lane: lane.to_string(),
            budget,
            declared,
            started: Instant::now(),
        }
    }

    /// What is left of this lane's budget.
    fn remaining(&self) -> Duration {
        self.budget.saturating_sub(self.started.elapsed())
    }

    /// How this lane ran out of time, naming the RUN when the run's remaining
    /// budget is what cut the lane short rather than its own setting.
    fn out_of_time(&self) -> String {
        if self.budget < self.declared {
            format!(
                "lane `{}` was stopped at {:?}, all that was left of the run's budget (its own \
                 deadline_secs is {}s)",
                self.lane,
                self.budget,
                self.declared.as_secs()
            )
        } else {
            format!(
                "lane `{}` exceeded its {:?} deadline",
                self.lane, self.budget
            )
        }
    }

    /// What the record and the alert say when this lane ran out of time. The
    /// stderr tail rides along the way it does on every other failure: the
    /// child is gone by the time the record is composed, and what it printed
    /// on the way to the deadline is the only clue to where it stopped.
    ///
    /// AN UNVERIFIED KILL IS NEVER REPORTED AS ONE. `Escaped` means something
    /// outlived TERM and KILL and may still be running and still writing after
    /// uu drops the run lock, which is a fact the operator has to be handed
    /// rather than one dressed up as a clean stop.
    fn overrun(&self, ended: &Ended, stderr: &[u8]) -> String {
        let how = match ended {
            Ended::Escaped => format!(
                "{}; something it left behind outlived TERM and KILL and may still be running",
                self.out_of_time()
            ),
            _ => format!("{}, so its process group was killed", self.out_of_time()),
        };
        failure_reason(&how, &String::from_utf8_lossy(stderr))
    }

    /// The one place a lane subject is actually spawned, under what is left of
    /// this lane's budget.
    fn spawn(&self, program: &str, args: &[&str], stdin: Stdio) -> Result<Finished, String> {
        // NOTHING RUNS ON A SPENT BUDGET. A lane out of time that still
        // spawned would take another whole deadline per remaining command,
        // which is what the herdr lane's two-per-plugin loop would turn into.
        let budget = self.remaining();
        if budget.is_zero() {
            return Err(self.overrun(&Ended::Stopped, b""));
        }
        match bounded_spawn(program, args, stdin, budget) {
            Spawned::Ran(finished) => Ok(finished),
            Spawned::NotRunnable(why) => Err(why),
            Spawned::SpawnStuck => Err(format!(
                "{}, and the spawn of {program} never returned, so there was no pid to signal",
                self.out_of_time()
            )),
        }
    }
}

/// How a child ended, in the one line every failure path here reasons about.
fn exit_description(status: &ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit {code}"),
        None => "killed by a signal".to_string(),
    }
}

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let finished = self.spawn(program, args, Stdio::null())?;
        let Ended::Exited(status) = finished.ended else {
            return Err(self.overrun(&finished.ended, &finished.stderr));
        };
        if status.success() {
            return Ok(String::from_utf8_lossy(&finished.stdout).to_string());
        }
        // WHAT IT PRINTED, not only how it ended. The spawn captured stderr
        // either way, the child is gone by the time the record is composed,
        // and a weekly job's own log may have rotated before anyone reads it.
        Err(failure_reason(
            &exit_description(&status),
            &String::from_utf8_lossy(&finished.stderr),
        ))
    }

    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String> {
        if input.len() > MAX_EVENT_INPUT {
            return Err(format!(
                "the event for {program} is {} bytes, over the {MAX_EVENT_INPUT}-byte pipe limit",
                input.len()
            ));
        }
        // PRE-FILL THE PIPE. uu holds the read end until every byte is
        // written and the writer is dropped, so uu's own write can never see
        // EPIPE; the child then reads the event and EOF, in one pass, with no
        // thread and no write deadline. Writing AFTER spawn is the mutant
        // this avoids: main() resets SIGPIPE to its default disposition, so a
        // child that exits without reading would otherwise kill uu at 141.
        let (reader, mut writer) = std::io::pipe()
            .map_err(|error| format!("could not open a pipe for {program}'s input: {error}"))?;
        writer
            .write_all(input.as_bytes())
            .map_err(|error| format!("could not write {program}'s input: {error}"))?;
        drop(writer);
        let finished = self.spawn(program, args, Stdio::from(reader))?;
        let verdict = match finished.ended {
            // AN OVERRUN IS A FAILURE THAT STILL KEEPS ITS STDOUT. Those lines
            // are the record of how far the lane got before it stopped, which
            // is the whole of what anyone has to diagnose a hang with.
            ref ended @ (Ended::Stopped | Ended::Escaped) => {
                Verdict::Failed(self.overrun(ended, &finished.stderr))
            }
            Ended::Exited(status) if status.success() => Verdict::Clean,
            Ended::Exited(status) => {
                let reason = failure_reason(
                    &exit_description(&status),
                    &String::from_utf8_lossy(&finished.stderr),
                );
                // DEFERRED_EXIT_CODE, not "any non-zero": the two weekly jobs
                // this ported from use it to mean "nothing was attempted, try
                // later", and every other non-zero code stays a real failure.
                if status.code() == Some(DEFERRED_EXIT_CODE) {
                    Verdict::Deferred(reason)
                } else {
                    Verdict::Failed(reason)
                }
            }
        };
        Ok(Ran {
            stdout: String::from_utf8_lossy(&finished.stdout).to_string(),
            verdict,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watchdog::tests::within;

    /// A runner whose budget no honest test child comes near.
    fn runner() -> SystemRunner {
        SystemRunner::for_lane("test", Duration::from_secs(30), Duration::from_secs(30))
    }

    #[test]
    fn a_failed_command_reports_what_it_printed_and_not_only_its_status() {
        // The one place stderr is still readable is here: the child is gone by
        // the time the record is composed, and a weekly job's log may have
        // rotated before anyone reads it.
        let failure = runner()
            .run(
                "/bin/sh",
                &["-c", "printf 'no such repository\\n' >&2; exit 3"],
            )
            .expect_err("this command fails");
        assert!(failure.contains("exit 3"), "{failure}");
        assert!(failure.contains("no such repository"), "{failure}");
    }

    // --- run_with_input, against the real child process ------------------

    #[test]
    fn run_with_input_hands_the_child_its_input_on_stdin() {
        // ON A DEADLINE. cat reads until EOF, and EOF only arrives once every
        // write end is closed: a run_with_input that kept its writer open
        // through the spawn would leave cat waiting for uu and uu waiting for
        // cat, which an unbounded call would report as a hang, not a failure.
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            send.send(runner().run_with_input("/bin/cat", &[], "the run event\n"))
        });
        let ran = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("cat never saw EOF: uu is still holding the pipe's write end")
            .expect("cat runs");
        assert_eq!(ran.stdout, "the run event\n");
        assert_eq!(ran.verdict, Verdict::Clean);
    }

    #[test]
    fn a_child_that_never_reads_its_stdin_is_still_a_clean_run() {
        // The property the pre-filled pipe exists for: uu's write is finished
        // before the child exists, so a child that exits without touching
        // stdin cannot make that write fail. What this test CANNOT observe is
        // the 141 itself: the harness keeps SIGPIPE ignored, and a write made
        // after the spawn usually lands in the pipe before a child this quick
        // has exited anyway. That the pre-filled sequence survives such a
        // child under `main`'s SIG_DFL reset, where the write-after-spawn
        // order dies at 141, was checked by hand outside the harness.
        let ran = runner()
            .run_with_input("/bin/sh", &["-c", "exit 0"], "the run event\n")
            .expect("a child that ignores stdin still runs and exits cleanly");
        assert_eq!(ran.verdict, Verdict::Clean);
    }

    #[test]
    fn run_with_input_reports_a_non_zero_exit_as_a_failure_carrying_the_stderr_tail() {
        // The child prints to stdout BEFORE it fails, the way a partially
        // successful upgrade would: a mutant that blanks stdout on any
        // non-zero exit would still satisfy every assertion below that only
        // looks at `verdict`, so `ran.stdout` is pinned here too.
        let ran = runner()
            .run_with_input(
                "/bin/sh",
                &[
                    "-c",
                    "printf '3 upgraded\\n'; cat >/dev/null; printf 'no such repository\\n' >&2; exit 2",
                ],
                "the run event\n",
            )
            .expect("the child ran, it just failed");
        assert_eq!(ran.stdout, "3 upgraded\n");
        let Verdict::Failed(failure) = ran.verdict else {
            panic!("exit 2 is a failure, not {:?}", ran.verdict);
        };
        assert!(failure.contains("exit 2"), "{failure}");
        assert!(failure.contains("no such repository"), "{failure}");
    }

    #[test]
    fn run_with_input_reports_the_deferred_exit_code_as_deferred_not_failed() {
        // The distinction this whole capability exists for: DEFERRED_EXIT_CODE
        // (75) is a verdict of its own, never lumped in with every other
        // non-zero exit.
        let ran = runner()
            .run_with_input(
                "/bin/sh",
                &[
                    "-c",
                    "printf 'nothing was attempted\\n'; cat >/dev/null; \
                     printf 'another run holds the lock\\n' >&2; exit 75",
                ],
                "the run event\n",
            )
            .expect("the child ran, it just deferred");
        // A mutant that blanks stdout ONLY on the deferred path (leaving the
        // clean and failed paths alone) would satisfy every other assertion
        // here, since none of them look at `ran.stdout` at all.
        assert_eq!(ran.stdout, "nothing was attempted\n");
        let Verdict::Deferred(reason) = ran.verdict else {
            panic!("exit 75 is a deferral, not {:?}", ran.verdict);
        };
        assert!(reason.contains("exit 75"), "{reason}");
        assert!(reason.contains("another run holds the lock"), "{reason}");
    }

    #[test]
    fn run_with_input_treats_any_other_non_zero_exit_as_failed_never_deferred() {
        // A mutant widening DEFERRED_EXIT_CODE's check to "any non-zero" would
        // pass with only 74 tested; a mutant narrowing it to `>= 75` would
        // pass with only 74 and 75 tested and misclassify 76 as deferred. Both
        // neighbors of 75 are pinned here as still Failed.
        for code in [74, 76] {
            let ran = runner()
                .run_with_input(
                    "/bin/sh",
                    &["-c", &format!("exit {code}")],
                    "the run event\n",
                )
                .expect("the child ran, it just failed");
            assert!(
                matches!(ran.verdict, Verdict::Failed(_)),
                "exit {code}: {:?}",
                ran.verdict
            );
        }
    }

    #[test]
    fn run_with_input_names_the_missing_program_when_it_could_not_run_at_all() {
        let error = runner()
            .run_with_input("/no/such/uu-test-program", &[], "the run event\n")
            .expect_err("a missing program cannot be run");
        assert!(error.contains("could not run"), "{error}");
        assert!(error.contains("/no/such/uu-test-program"), "{error}");
    }

    #[test]
    fn run_with_input_refuses_an_input_over_16_kib_without_spawning_anything() {
        let huge = "x".repeat(MAX_EVENT_INPUT + 1);
        let error = runner()
            .run_with_input("/no/such/uu-test-program", &[], &huge)
            .expect_err("an oversized event must be refused");
        // Naming the actual size proves the refusal ran; a missing-program
        // message here instead would prove the size check let the spawn
        // through.
        assert!(
            error.contains(&(MAX_EVENT_INPUT + 1).to_string()),
            "{error}"
        );
        assert!(!error.contains("could not run"), "{error}");
    }

    #[test]
    fn run_with_input_refuses_by_byte_length_not_character_count() {
        // 4096 four-byte characters plus one ASCII byte is 16385 bytes but
        // only 4097 characters, well under MAX_EVENT_INPUT. A mutant that
        // measured `input.chars().count()` instead of `input.len()` would let
        // this through and only a multi-byte fixture can catch it.
        let huge = format!("{}x", "\u{1D11E}".repeat(MAX_EVENT_INPUT / 4));
        assert_eq!(huge.len(), MAX_EVENT_INPUT + 1);
        assert!(huge.chars().count() < MAX_EVENT_INPUT);
        let error = runner()
            .run_with_input("/no/such/uu-test-program", &[], &huge)
            .expect_err("an oversized event must be refused even when it is short in characters");
        assert!(
            error.contains(&(MAX_EVENT_INPUT + 1).to_string()),
            "{error}"
        );
        assert!(!error.contains("could not run"), "{error}");
    }

    #[test]
    fn run_with_input_allows_an_input_of_exactly_16_kib() {
        // The limit is a size AT which the input still fits, not one past
        // which it starts to fit: a `>=` mutant would refuse this legal
        // boundary case while every other test here stays green.
        let exact = "x".repeat(MAX_EVENT_INPUT);
        let error = runner()
            .run_with_input("/no/such/uu-test-program", &[], &exact)
            .expect_err("the program does not exist, but the size check must have let it through");
        assert!(
            error.contains("could not run"),
            "an exact-limit input must reach the spawn attempt: {error}"
        );
    }

    // --- the lane deadline, against real children -------------------------

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
    fn a_deadline_that_did_not_stop_the_group_never_claims_that_it_did() {
        // `Escaped` means something outlived TERM and KILL and may still be
        // running and writing after uu drops the run lock. Wording it as a
        // clean kill would hand the operator a stop uu never verified.
        let runner = runner();
        let escaped = runner.overrun(&Ended::Escaped, b"");
        assert!(escaped.contains("outlived TERM and KILL"), "{escaped}");
        assert!(!escaped.contains("was killed"), "{escaped}");
        let stopped = runner.overrun(&Ended::Stopped, b"");
        assert!(stopped.contains("process group was killed"), "{stopped}");
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
