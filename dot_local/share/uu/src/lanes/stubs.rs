//! The stand-ins every lane test runs against: a scripted `CommandRunner` and
//! one fixed set of run facts.
//!
//! SHARED ACROSS THE LANE MODULES, so a lane's own tests never spawn anything.
//! `#[cfg(test)]` at the parent, so none of this enters a production build.

use std::cell::RefCell;
use std::time::Duration;

use crate::lanes::{CommandRunner, Ran, Verdict};
use crate::record::{Marker, RunFacts};

/// A runner that answers from a script and records every call. The script
/// is keyed on the whole argument vector, so a test says exactly which
/// invocation fails without depending on call order.
pub(crate) struct ScriptedRunner {
    failing: Vec<Vec<String>>,
    deferring: Vec<Vec<String>>,
    /// A deferral whose REASON TEXT is the test's own choice, checked
    /// before `deferring`'s fixed "exit 75": proves `run_command` carries
    /// `Verdict::Deferred`'s own text through rather than a fixed string
    /// of its own, which a call keyed only by `deferring` cannot, since
    /// every one of those already answers the identical "exit 75".
    deferring_because: Vec<(Vec<String>, String)>,
    unrunnable: Vec<Vec<String>>,
    stdout: String,
    calls: RefCell<Vec<Vec<String>>>,
    inputs: RefCell<Vec<String>>,
    /// Every bounded call, with the bound it was given: what a lane test
    /// asserts a step's own deadline against without a real clock.
    deadlines: RefCell<Vec<(Vec<String>, Duration)>>,
}

impl ScriptedRunner {
    pub(crate) fn new(failing: &[&[&str]]) -> Self {
        ScriptedRunner {
            failing: failing
                .iter()
                .map(|call| call.iter().map(|word| word.to_string()).collect())
                .collect(),
            deferring: Vec::new(),
            deferring_because: Vec::new(),
            unrunnable: Vec::new(),
            stdout: String::new(),
            calls: RefCell::new(Vec::new()),
            inputs: RefCell::new(Vec::new()),
            deadlines: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn answering(mut self, stdout: &str) -> Self {
        self.stdout = stdout.to_string();
        self
    }

    /// A `run_with_input` call that exits `DEFERRED_EXIT_CODE`, distinct
    /// from `failing`'s "ran, but exited some other non-zero code".
    pub(crate) fn deferring(mut self, call: &[&str]) -> Self {
        self.deferring
            .push(call.iter().map(|word| word.to_string()).collect());
        self
    }

    /// The same deferral, with a reason unique to the call rather than
    /// `deferring`'s fixed "exit 75".
    pub(crate) fn deferring_because(mut self, call: &[&str], reason: &str) -> Self {
        self.deferring_because.push((
            call.iter().map(|word| word.to_string()).collect(),
            reason.to_string(),
        ));
        self
    }

    /// A `run_with_input` call the runner cannot make at all, the
    /// could-not-run path (a missing executable), distinct from
    /// `failing`'s "ran, but exited non-zero".
    pub(crate) fn unable_to_run(mut self, call: &[&str]) -> Self {
        self.unrunnable
            .push(call.iter().map(|word| word.to_string()).collect());
        self
    }

    pub(crate) fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    /// Every call made under a bound, with that bound.
    pub(crate) fn deadlines(&self) -> Vec<(Vec<String>, Duration)> {
        self.deadlines.borrow().clone()
    }

    /// Every `input` a call to `run_with_input` was given, in call order:
    /// the spy `run_with_input` itself never touches (BRIEF U8).
    pub(crate) fn inputs(&self) -> Vec<String> {
        self.inputs.borrow().clone()
    }
}

impl CommandRunner for ScriptedRunner {
    fn run_with_deadline(
        &self,
        program: &str,
        args: &[&str],
        most: Duration,
    ) -> Result<String, String> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|word| word.to_string()));
        self.deadlines.borrow_mut().push((call, most));
        self.run(program, args)
    }

    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|word| word.to_string()));
        self.calls.borrow_mut().push(call.clone());
        // The Nth repeat of a scripted failure is still a failure: the
        // retry has to be able to fail too.
        if self.failing.contains(&call) {
            return Err("exit 1".to_string());
        }
        Ok(self.stdout.clone())
    }

    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|word| word.to_string()));
        self.calls.borrow_mut().push(call.clone());
        self.inputs.borrow_mut().push(input.to_string());
        if self.unrunnable.contains(&call) {
            return Err(format!("could not run {program}: stubbed as unrunnable"));
        }
        let verdict = if let Some((_, reason)) =
            self.deferring_because.iter().find(|(key, _)| key == &call)
        {
            Verdict::Deferred(reason.clone())
        } else if self.deferring.contains(&call) {
            Verdict::Deferred("exit 75".to_string())
        } else if self.failing.contains(&call) {
            Verdict::Failed("exit 1".to_string())
        } else {
            Verdict::Clean
        };
        Ok(Ran {
            stdout: self.stdout.clone(),
            verdict,
        })
    }
}

/// The one fixed `RunFacts` every test here that does not care about its
/// contents can share; `record.rs` owns the tests that pin `lane_event`
/// itself against varied facts.
const STUB_MARKER: Marker = Marker::NeverRecorded;

pub(crate) fn stub_facts() -> RunFacts<'static> {
    RunFacts {
        host: "test-host",
        started_epoch: 0,
        started_iso: "1970-01-01T00:00:00Z",
        marker: &STUB_MARKER,
    }
}
