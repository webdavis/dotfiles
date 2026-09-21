use super::*;
use pns_domain::jobs::Job;
use std::cell::RefCell;

#[derive(Default)]
struct Spool {
    pending: RefCell<Option<Job>>,
}

impl crate::JobSpool for Spool {
    fn pending(&self, _id: &str) -> Option<Job> {
        self.pending.borrow().clone()
    }
    fn schedule(&self, job: &Job, _now: u64) -> Result<(), String> {
        *self.pending.borrow_mut() = Some(job.clone());
        Ok(())
    }
    fn cancel(&self, _id: &str) -> Result<bool, String> {
        Ok(self.pending.borrow_mut().take().is_some())
    }
}

#[test]
fn an_armed_calendar_registers_the_poll_at_the_interval_it_was_handed() {
    let spool = Spool::default();
    ensure_calendar_poll(&spool, PollSetting::Every(120), 1_000);
    let job = spool.pending.borrow().clone().expect("a registered job");
    assert_eq!(job.id, CALENDAR_JOB);
    assert_eq!(job.args, ["mute", "calendar"]);
    assert_eq!(job.every, Some(120));
    assert_eq!(job.due, 1_000);
    assert_eq!(job.until, 1_900);
}

#[test]
fn an_unreadable_config_keeps_the_registered_poll_and_renews_its_lease() {
    let spool = Spool::default();
    ensure_calendar_poll(&spool, PollSetting::Every(120), 1_000);
    let registered = spool.pending.borrow().clone().expect("a registered job");
    *spool.pending.borrow_mut() = Some(Job {
        due: 1_100,
        ..registered
    });

    ensure_calendar_poll(&spool, PollSetting::Unreadable, 1_500);

    let job = spool
        .pending
        .borrow()
        .clone()
        .expect("the poll was dropped");
    assert_eq!((job.due, job.every, job.until), (1_100, Some(120), 2_400));
}

#[test]
fn a_calendar_that_is_off_cancels_the_poll_it_had_registered() {
    // THE MUTANT THIS PINS: the cancel dropped, which leaves a command the
    // operator switched off still being run every two minutes.
    let spool = Spool::default();
    ensure_calendar_poll(&spool, PollSetting::Every(120), 1_000);
    ensure_calendar_poll(&spool, PollSetting::Off, 1_060);
    assert!(spool.pending.borrow().is_none());
}

#[test]
fn a_poll_already_waiting_keeps_its_own_due_second_across_a_sweep() {
    // THE MUTANT THIS PINS: `due` recomputed from `now` on every sweep, which
    // pushes the next poll thirty seconds further out every thirty seconds
    // and leaves the calendar read nobody ever reaches.
    let spool = Spool::default();
    ensure_calendar_poll(&spool, PollSetting::Every(120), 1_000);
    spool
        .pending
        .borrow_mut()
        .as_mut()
        .expect("a registered job")
        .due = 1_120;
    ensure_calendar_poll(&spool, PollSetting::Every(120), 1_030);
    assert_eq!(
        spool.pending.borrow().as_ref().map(|job| job.due),
        Some(1_120)
    );
}
