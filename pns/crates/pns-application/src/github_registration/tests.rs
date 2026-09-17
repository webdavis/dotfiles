use super::*;
use pns_domain::jobs::Job;
use std::cell::RefCell;

/// One spool entry and a log of what was asked of it.
#[derive(Default)]
struct Spool {
    pending: RefCell<Option<Job>>,
    log: RefCell<Vec<String>>,
}

impl crate::JobSpool for Spool {
    fn pending(&self, _id: &str) -> Option<Job> {
        self.pending.borrow().clone()
    }
    fn schedule(&self, job: &Job, now: u64) -> Result<(), String> {
        self.log.borrow_mut().push(format!("schedule({now})"));
        *self.pending.borrow_mut() = Some(job.clone());
        Ok(())
    }
    fn cancel(&self, id: &str) -> Result<bool, String> {
        self.log.borrow_mut().push(format!("cancel({id})"));
        Ok(self.pending.borrow_mut().take().is_some())
    }
}

#[test]
fn an_armed_source_registers_the_poll_at_the_interval_it_was_handed() {
    let spool = Spool::default();
    ensure_github_poll(&spool, Some(60), 1000);
    let job = spool.pending.borrow().clone().expect("a registered job");
    assert_eq!(job.id, "github");
    // THE FLAG THE DAEMON ALONE PASSES, so the poll it launches knows nobody
    // is reading its stderr.
    assert_eq!(
        job.args,
        vec![
            "github".to_string(),
            "poll".to_string(),
            GITHUB_DAEMON_FLAG.to_string()
        ]
    );
    assert_eq!(job.every, Some(60));
    // DUE NOW, so the first listing arrives on the next tick rather than one
    // interval after the switch went on, and leased past it.
    assert_eq!(job.due, 1000);
    assert_eq!(job.until, 1900);
}

#[test]
fn a_source_that_is_off_cancels_the_poll_it_had_registered() {
    // THE MUTANT THIS PINS: the cancel dropped, which leaves a poll spending
    // the rate limit on a token the operator switched off. Off, absent and
    // refused all arrive here as one `None`.
    let spool = Spool::default();
    ensure_github_poll(&spool, Some(60), 1000);
    assert!(
        spool.pending.borrow().is_some(),
        "the fixture registered nothing"
    );

    ensure_github_poll(&spool, None, 1030);

    assert!(
        spool.pending.borrow().is_none(),
        "the poll outlived its own table"
    );
    assert!(
        spool
            .log
            .borrow()
            .iter()
            .any(|line| line == "cancel(github)"),
        "{:?}",
        spool.log.borrow()
    );
}

#[test]
fn a_sweep_refreshes_the_lease_without_moving_a_poll_that_is_already_due() {
    // The sweep runs every thirty seconds and the poll every sixty, so a
    // sweep that re-armed `due` would keep pushing the listing away from
    // itself and the source would never report at all.
    let spool = Spool::default();
    ensure_github_poll(&spool, Some(60), 1000);
    // As the daemon leaves it after firing once: due again sixty seconds on.
    let fired = Job {
        due: 1060,
        ..spool.pending.borrow().clone().expect("a registered job")
    };
    *spool.pending.borrow_mut() = Some(fired);

    ensure_github_poll(&spool, Some(60), 1002);

    let job = spool.pending.borrow().clone().expect("a registered job");
    assert_eq!(job.due, 1060, "the pending due moved");
    assert_eq!(job.until, 1902, "the lease was not refreshed");
}

#[test]
fn a_pending_poll_already_past_its_due_is_brought_forward_to_now() {
    // A laptop that slept leaves a due in the past: the lease has to outlast
    // `now`, or the sweep would register a job that expires before it runs.
    let spool = Spool::default();
    ensure_github_poll(&spool, Some(60), 1000);
    let stale = Job {
        due: 500,
        ..spool.pending.borrow().clone().expect("a registered job")
    };
    *spool.pending.borrow_mut() = Some(stale);

    ensure_github_poll(&spool, Some(60), 2000);

    let job = spool.pending.borrow().clone().expect("a registered job");
    assert_eq!(job.due, 2000);
    assert_eq!(job.until, 2900);
}

#[test]
fn the_interval_the_server_asked_for_is_what_gets_registered() {
    // "In times of high server load, the time may increase. Please obey the
    // header." Obeying it means the registration moves when the header does.
    let spool = Spool::default();
    ensure_github_poll(&spool, Some(300), 1000);
    assert_eq!(
        spool.pending.borrow().as_ref().and_then(|job| job.every),
        Some(300)
    );
}
