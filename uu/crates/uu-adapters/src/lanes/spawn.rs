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
    pub stderr: String,
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
    Pending(String),
    Failed(String),
}

/// The environment one child runs in.
///
/// ONE VALUE FOR EVERY LANE THAT NEEDS ONE, so no lane composes an
/// environment out of argv words and no lane spawns a helper to set them.
/// `path_prefix` is its own field rather than a `PATH` entry in `variables`
/// because the rest of that value is uu's own inherited `PATH`, which lives
/// in the process and not in the lane's arguments: the adapter joins the two
/// at spawn time and a lane stays a function of what it was given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    /// Set on the child, over whatever uu inherited unless `only_these`.
    pub variables: std::collections::BTreeMap<String, String>,
    /// Put ahead of the inherited `PATH`.
    pub path_prefix: Option<String>,
    /// Hand the child `variables` alone and nothing uu inherited.
    pub only_these: bool,
}

impl Environment {
    /// Whatever uu inherited, unchanged.
    pub fn inheriting() -> Self {
        Self::default()
    }

    /// `variables` and nothing else.
    pub fn only(variables: &std::collections::BTreeMap<String, String>) -> Self {
        Environment {
            variables: variables.clone(),
            path_prefix: None,
            only_these: true,
        }
    }

    pub fn with(mut self, key: &str, value: String) -> Self {
        self.variables.insert(key.to_string(), value);
        self
    }

    /// `dir` first on the child's `PATH`, then everything uu inherited.
    pub fn prepending_path(mut self, dir: &str) -> Self {
        self.path_prefix = Some(dir.to_string());
        self
    }
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
    fn run_to_file(
        &self,
        _program: &str,
        _args: &[&str],
        _input: std::fs::File,
        _output: std::fs::File,
    ) -> Result<(), String> {
        Err("runner does not support file output".into())
    }
    /// `run`, in `env`, and under `most` as well as the lane's deadline when
    /// the caller names one.
    fn run_in(
        &self,
        _program: &str,
        _args: &[&str],
        _env: &Environment,
        _most: Option<Duration>,
    ) -> Result<String, String> {
        Err("runner does not support a child environment".into())
    }

    /// `run_in`, keeping what the child printed and how it ended instead of
    /// failing on a non-clean exit: the smoke test reads a child's stderr and
    /// its verdict, not only its stdout.
    fn run_reporting_in(
        &self,
        _program: &str,
        _args: &[&str],
        _env: &Environment,
    ) -> Result<Ran, String> {
        Err("runner does not support a child environment".into())
    }
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
