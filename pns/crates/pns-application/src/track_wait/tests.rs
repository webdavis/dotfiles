use super::track_wait;
use crate::{JobSpool, SessionWaits};
use pns_domain::jobs::Job;
use std::cell::RefCell;

#[derive(Default)]
struct Recorder {
    steps: RefCell<Vec<String>>,
    jobs: RefCell<Vec<Job>>,
    fail: &'static str,
}
impl Recorder {
    fn effect(&self, operation: &str) -> Result<(), String> {
        self.steps.borrow_mut().push(operation.to_string());
        if self.fail == operation {
            Err(operation.to_string())
        } else {
            Ok(())
        }
    }
}
impl SessionWaits for Recorder {
    fn begin(&self, _: &str, _: u64) -> Result<(), String> {
        self.effect("begin")
    }
    fn end(&self, _: &str) -> Result<(), String> {
        self.effect("end")
    }
}
impl JobSpool for Recorder {
    fn pending(&self, _: &str) -> Option<Job> {
        None
    }
    fn schedule(&self, job: &Job, _now: u64) -> Result<(), String> {
        self.jobs.borrow_mut().push(job.clone());
        self.effect("schedule")
    }
    fn cancel(&self, _: &str) -> Result<bool, String> {
        unreachable!("nothing cancels a wait")
    }
}

const WINDOW: u64 = 3_600;

fn tracked(recorder: &Recorder, state: &str) {
    track_wait(
        recorder,
        recorder,
        "session",
        state,
        WINDOW,
        Some(100),
        |warning| panic!("unexpected warning: {warning}"),
    );
}

#[test]
fn a_wait_starting_event_records_the_row_and_schedules_one_leased_job() {
    let recorder = Recorder::default();
    tracked(&recorder, "blocked");
    assert_eq!(*recorder.steps.borrow(), ["begin", "schedule"]);
    let jobs = recorder.jobs.borrow();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "stale:session");
    assert_eq!(
        (jobs[0].due, jobs[0].until, jobs[0].every),
        (100 + WINDOW, 100 + 2 * WINDOW, None)
    );
    assert_eq!(jobs[0].args, ["stale"]);
    assert_eq!(
        jobs[0].unless_marker, None,
        "the row is the authority, not the nag's answered marker"
    );
}

#[test]
fn every_waiting_state_arms_it_and_a_later_event_clears_the_row() {
    // The list is `pulse::LAMP_BLOCKED`, read rather than copied, so
    // `plan-ready` and `asking` are waits by the same definition as `blocked`.
    for state in pns_domain::pulse::LAMP_BLOCKED {
        let recorder = Recorder::default();
        tracked(&recorder, state);
        assert_eq!(*recorder.steps.borrow(), ["begin", "schedule"], "{state}");
    }
    for state in ["done", "failed", "resolved", "anything-else"] {
        let recorder = Recorder::default();
        tracked(&recorder, state);
        assert_eq!(*recorder.steps.borrow(), ["end"], "{state}");
        assert!(recorder.jobs.borrow().is_empty(), "{state}");
    }
}

#[test]
fn a_window_of_zero_arms_nothing_and_still_clears() {
    let recorder = Recorder::default();
    track_wait(
        &recorder,
        &recorder,
        "session",
        "blocked",
        0,
        Some(100),
        |warning| panic!("unexpected warning: {warning}"),
    );
    assert!(recorder.steps.borrow().is_empty(), "the feature is off");
    track_wait(
        &recorder,
        &recorder,
        "session",
        "done",
        0,
        Some(100),
        |_| {},
    );
    assert_eq!(*recorder.steps.borrow(), ["end"]);
}

#[test]
fn a_row_that_could_not_be_written_schedules_no_job_and_says_so() {
    let recorder = Recorder {
        fail: "begin",
        ..Default::default()
    };
    let mut warnings = Vec::new();
    track_wait(
        &recorder,
        &recorder,
        "session",
        "blocked",
        WINDOW,
        Some(100),
        |warning| warnings.push(warning.to_string()),
    );
    assert_eq!(*recorder.steps.borrow(), ["begin"]);
    assert_eq!(
        warnings,
        [
            "pns: state error (this session's wait could not be recorded: begin); a stale block will not be escalated"
        ]
    );
}

#[test]
fn a_session_id_that_cannot_be_a_filename_records_nothing_at_all() {
    let recorder = Recorder::default();
    tracked(&recorder, "blocked");
    recorder.steps.borrow_mut().clear();
    for id in ["", "../escape", "a/b"] {
        track_wait(
            &recorder,
            &recorder,
            id,
            "blocked",
            WINDOW,
            Some(100),
            |warning| panic!("unexpected warning: {warning}"),
        );
    }
    assert!(recorder.steps.borrow().is_empty());
}
