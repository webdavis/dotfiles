use super::*;
use pns_domain::jobs::Job;
use std::cell::RefCell;

#[derive(Default)]
struct Spool {
    pending: Option<Job>,
    written: RefCell<Vec<Job>>,
}
impl JobSpool for Spool {
    fn pending(&self, id: &str) -> Option<Job> {
        assert_eq!(id, LIGHTS_JOB);
        self.pending.clone()
    }
    fn schedule(&self, job: &Job, _: u64) -> Result<(), String> {
        self.written.borrow_mut().push(job.clone());
        Err("write refused".into())
    }
    fn cancel(&self, _: &str) -> Result<bool, String> {
        unreachable!()
    }
}
fn job(due: u64) -> Job {
    Job {
        id: LIGHTS_JOB.into(),
        due,
        until: due + 1,
        every: Some(1),
        unless_marker: None,
        args: Vec::new(),
    }
}

#[test]
fn a_pending_lights_tick_keeps_its_due_time_while_the_lease_is_refreshed() {
    let spool = Spool {
        pending: Some(job(110)),
        ..Spool::default()
    };
    schedule_lights_tick(
        &spool,
        &pns_domain::lamps::config::Lights::default(),
        100,
        300,
    );
    let jobs = spool.written.borrow();
    assert_eq!(jobs.len(), 1);
    assert_eq!((jobs[0].due, jobs[0].until), (110, 400));
    assert_eq!(jobs[0].args, ["lights", "tick"]);
}

#[test]
fn an_absent_or_due_lights_tick_starts_one_refresh_out_with_a_valid_lease() {
    let lights = pns_domain::lamps::config::Lights::default();
    for pending in [None, Some(job(100)), Some(job(99))] {
        let spool = Spool {
            pending,
            ..Spool::default()
        };
        schedule_lights_tick(&spool, &lights, 100, 0);
        let jobs = spool.written.borrow();
        assert_eq!(
            (jobs[0].due, jobs[0].until),
            (100 + lights.refresh_secs, 100 + lights.refresh_secs)
        );
        assert_eq!(jobs[0].every, Some(lights.refresh_secs));
        assert_eq!(jobs[0].unless_marker, None);
    }
}
