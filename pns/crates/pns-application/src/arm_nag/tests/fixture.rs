use crate::{Claimed, JobSpool, NagRecords};
use pns_domain::{
    EventArgs,
    jobs::Job,
    nag::{Dropped, Record},
};
use std::cell::RefCell;

#[derive(Default)]
pub(super) struct Recorder {
    pub steps: RefCell<Vec<String>>,
    pub jobs: RefCell<Vec<Job>>,
    pub records: RefCell<Vec<Record>>,
    pub fail: &'static str,
}
impl Recorder {
    fn effect(&self, operation: &str) -> Result<(), String> {
        self.steps.borrow_mut().push(operation.to_string());
        if self.fail.split(',').any(|fail| fail == operation) {
            Err(operation.to_string())
        } else {
            Ok(())
        }
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
        unreachable!("not a cancellation")
    }
}
impl NagRecords for Recorder {
    fn claim_fire(&self, _: u64) -> bool {
        unreachable!("not a fire")
    }
    fn claim_due(&self, _: u64) -> Vec<Claimed> {
        unreachable!("not a fire")
    }
    fn drop_claim(&self, _: &str, _: Option<Dropped>) {
        unreachable!("not a fire")
    }
    fn release_fire(&self) {
        unreachable!("not a fire")
    }
    fn mark_answered(&self, _: &str) -> Result<(), String> {
        self.effect("mark")
    }
    fn clear_answered(&self, _: &str) -> Result<(), String> {
        self.effect("clear")
    }
    fn publish(&self, _: &str, record: &Record) -> Result<(), String> {
        self.records.borrow_mut().push(record.clone());
        self.effect("publish")
    }
    fn drop_record(&self, _: &str) -> Result<(), String> {
        self.effect("drop")
    }
    fn clear_pending(&self) -> usize {
        unreachable!("not disabled cleanup")
    }
}
pub(super) fn event() -> EventArgs {
    EventArgs {
        agent: "claude".into(),
        detail: "private approval question".into(),
        project: "project".into(),
        branch: "branch".into(),
        pane: "%2".into(),
        ..Default::default()
    }
}
