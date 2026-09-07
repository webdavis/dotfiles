//! The spawn seam every lane reaches its subject through, and the verdict that
//! comes back.
//!
//! ONE TRAIT, so the thing under test is what a lane DECIDES and never what
//! herdr or Homebrew does. The only production implementation is the binary's
//! own `SystemRunner`, which is where a child is actually spawned.

use std::time::Duration;

/// What a command lane's child did, when it could be run at all. `stdout` is
/// kept EVEN ON A NON-CLEAN EXIT (a failed or deferred child's own record
/// lines are not the thing that failed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub stdout: String,
    pub verdict: Verdict,
}

/// How a command lane's child ended. `Deferred` and `Failed` each carry the
/// one line `failure_reason` composes (how it ended, plus the tail of what it
/// said on stderr): a deferring lane explains itself on stderr as often as a
/// failing one does, and that explanation belongs in the record either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Clean,
    Deferred(String),
    Failed(String),
}

/// The spawn seam. `run`'s `Ok` carries the command's stdout, `Err` why it did
/// not succeed, already fit to print.
///
/// `run_with_input` is for a child that is HANDED something on stdin (a
/// command lane's run event): it separates "could not run this at all" (the
/// `Err`, e.g. a missing executable) from "ran, but did not exit clean"
/// (`Ran::verdict`), because the second case still has stdout worth
/// recording.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String>;

    /// `run`, under a bound of ITS OWN as well as the lane's.
    ///
    /// THE LANE DEADLINE IS THE WHOLE LANE'S, so a step that takes all of it
    /// costs every step after it. A subject known to wedge rather than fail
    /// (the App Store hangs indefinitely on a broken session) is bounded here
    /// instead, so one wedged step costs itself and the rest of the lane
    /// still runs. Whichever bound is smaller, the step's own or what is left
    /// of the lane's, is the one that expires.
    fn run_with_deadline(
        &self,
        program: &str,
        args: &[&str],
        most: Duration,
    ) -> Result<String, String>;

    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String>;
}
