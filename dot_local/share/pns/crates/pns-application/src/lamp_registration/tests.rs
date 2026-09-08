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

fn decision(now: Option<u64>) -> pns_domain::Decision {
    use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
    use pns_domain::{Decision, GateInputs};
    Decision {
        legs: Vec::new(),
        pane_dropped: false,
        plan: DeliveryPlan {
            banner: true,
            phone_card: true,
            pulse: true,
        },
        inputs: GateInputs {
            desk_input_age: None,
            phone_input_age: None,
            marker_age: None,
            screen_locked: None,
            desk_fresh_secs: None,
            surface: Surface::Away,
            session_visibility: Visibility::Hidden,
            visibility: Visibility::Hidden,
            now_secs: now,
            long_running: false,
            mobile_watch_card: false,
            scope: pns_domain::DeliveryScope::Automatic,
            pane_present: true,
        },
    }
}

#[test]
fn registration_uses_the_actual_miss_answer_for_both_lease_lengths() {
    let lights = pns_domain::lamps::config::Lights::default();
    for (actual_miss, expected) in [(true, 43_300), (false, 400)] {
        let spool = Spool::default();
        register_lights_tick(&spool, Some(&lights), &decision(Some(100)), actual_miss);
        let jobs = spool.written.borrow();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].until, expected);
        assert_eq!(jobs[0].due, 100 + lights.refresh_secs);
    }
}

#[test]
fn no_clock_or_no_lights_does_not_register_either_lease() {
    let lights = pns_domain::lamps::config::Lights::default();
    for actual_miss in [false, true] {
        let spool = Spool::default();
        register_lights_tick(&spool, Some(&lights), &decision(None), actual_miss);
        register_lights_tick(&spool, None, &decision(Some(100)), actual_miss);
        assert!(spool.written.borrow().is_empty());
    }
}
