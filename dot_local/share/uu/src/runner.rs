//! The lane subjects: the one process boundary that actually spawns one.
//!
//! Everything the library decides is a total function of its arguments; this
//! is where a lane's own program runs, which is why it sits outside the lib
//! beside `main` rather than next to the policy it carries out.

use std::io::Write;
use std::process::{Command, Output, Stdio};

use unattended_upgrades::lanes::{CommandRunner, DEFERRED_EXIT_CODE, Ran, Verdict, failure_reason};

/// The event handed to a command lane's child cannot exceed this, or
/// `run_with_input`'s pre-filled pipe would have to write more than fits
/// before a reader exists. XNU's own floor is 16 KiB; measured capacity on
/// Darwin 25.2 is 64 KiB. The limit sits AT the floor, the one size a whole
/// event is guaranteed to fit at before any reader exists.
/// Past capacity `write_all` blocks forever with no reader and no deadline: a
/// silent hang of an unattended job. The event uu composes today is under
/// 1 KiB.
pub const MAX_EVENT_INPUT: usize = 16 * 1024;

/// The lane subjects. No deadline, matching the shell job this replaces: a
/// plugin install has no honest upper bound and launchd is what notices a job
/// that never ends.
pub struct SystemRunner;

impl SystemRunner {
    /// The one place a lane subject is actually spawned. `run` and
    /// `run_with_input` differ only in what `stdin` they hand it.
    fn spawn(&self, program: &str, args: &[&str], stdin: Stdio) -> Result<Output, String> {
        Command::new(program)
            .args(args)
            .stdin(stdin)
            .output()
            .map_err(|error| format!("could not run {program}: {error}"))
    }
}

/// How a child ended, in the one line every failure path here reasons about.
fn exit_description(output: &Output) -> String {
    match output.status.code() {
        Some(code) => format!("exit {code}"),
        None => "killed by a signal".to_string(),
    }
}

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let output = self.spawn(program, args, Stdio::null())?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
        // WHAT IT PRINTED, not only how it ended. `output` captured stderr
        // either way, the child is gone by the time the record is composed,
        // and a weekly job's own log may have rotated before anyone reads it.
        Err(failure_reason(
            &exit_description(&output),
            &String::from_utf8_lossy(&output.stderr),
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
        let output = self.spawn(program, args, Stdio::from(reader))?;
        let verdict = if output.status.success() {
            Verdict::Clean
        } else {
            let reason = failure_reason(
                &exit_description(&output),
                &String::from_utf8_lossy(&output.stderr),
            );
            // DEFERRED_EXIT_CODE, not "any non-zero": the two weekly jobs
            // this ported from use it to mean "nothing was attempted, try
            // later", and every other non-zero code stays a real failure.
            if output.status.code() == Some(DEFERRED_EXIT_CODE) {
                Verdict::Deferred(reason)
            } else {
                Verdict::Failed(reason)
            }
        };
        Ok(Ran {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            verdict,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_failed_command_reports_what_it_printed_and_not_only_its_status() {
        // The one place stderr is still readable is here: the child is gone by
        // the time the record is composed, and a weekly job's log may have
        // rotated before anyone reads it.
        let failure = SystemRunner
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
            send.send(SystemRunner.run_with_input("/bin/cat", &[], "the run event\n"))
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
        let ran = SystemRunner
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
        let ran = SystemRunner
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
        let ran = SystemRunner
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
            let ran = SystemRunner
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
        let error = SystemRunner
            .run_with_input("/no/such/uu-test-program", &[], "the run event\n")
            .expect_err("a missing program cannot be run");
        assert!(error.contains("could not run"), "{error}");
        assert!(error.contains("/no/such/uu-test-program"), "{error}");
    }

    #[test]
    fn run_with_input_refuses_an_input_over_16_kib_without_spawning_anything() {
        let huge = "x".repeat(MAX_EVENT_INPUT + 1);
        let error = SystemRunner
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
        let error = SystemRunner
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
        let error = SystemRunner
            .run_with_input("/no/such/uu-test-program", &[], &exact)
            .expect_err("the program does not exist, but the size check must have let it through");
        assert!(
            error.contains("could not run"),
            "an exact-limit input must reach the spawn attempt: {error}"
        );
    }
}
