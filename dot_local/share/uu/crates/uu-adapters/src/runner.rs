//! The lane subjects, spawned under the deadline their lane declared.
//!
//! THE ONE PROCESS BOUNDARY THAT SPAWNS. Everything the library decides is a
//! total function of its arguments; this is where a lane subject actually
//! runs, which is why the whole watchdog lives here rather than beside the
//! policy it enforces.

use std::io::Write;
use std::process::{ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::lanes::{CommandRunner, Ran, Verdict, failure_reason};
use uu_protocol::{DEFERRED_EXIT_CODE, PENDING_EXIT_CODE};

use crate::watchdog::{Ended, Finished, Spawned, bounded_spawn};

mod bounds;
mod environment;
mod overrun;

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

    fn clean_output(&self, finished: Finished) -> Result<String, String> {
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

    /// What is left of this lane's budget.
    fn remaining(&self) -> Duration {
        self.budget.saturating_sub(self.started.elapsed())
    }

    /// This lane's overrun line, with the budget bookkeeping filled in.
    fn overrun(&self, ended: &Ended, stderr: &[u8]) -> String {
        overrun::overrun(&self.lane, self.budget, self.declared, ended, stderr)
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
            Spawned::SpawnStuck => Err(overrun::spawn_stuck(
                &self.lane,
                self.budget,
                self.declared,
                program,
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
    fn run_in(
        &self,
        program: &str,
        args: &[&str],
        env: &std::collections::BTreeMap<String, String>,
    ) -> Result<String, String> {
        environment::run(self, program, args, env)
    }

    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let finished = self.spawn(program, args, Stdio::null())?;
        self.clean_output(finished)
    }

    /// One step under a bound of its own; `bounds` owns the reasoning.
    fn run_with_deadline(
        &self,
        program: &str,
        args: &[&str],
        most: Duration,
    ) -> Result<String, String> {
        bounds::run_step(self, program, args, most)
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
                // later", while 100 is successful work awaiting operator action. Other
                // non-zero codes stay real failures.
                if status.code() == Some(DEFERRED_EXIT_CODE) {
                    Verdict::Deferred(reason)
                } else if status.code() == Some(PENDING_EXIT_CODE) {
                    Verdict::Pending(reason)
                } else {
                    Verdict::Failed(reason)
                }
            }
        };
        Ok(Ran {
            stderr: String::from_utf8_lossy(&finished.stderr).to_string(),
            stdout: String::from_utf8_lossy(&finished.stdout).to_string(),
            verdict,
        })
    }
}

#[cfg(test)]
mod tests;
