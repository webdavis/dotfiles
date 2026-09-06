//! The job policy's own tests: the heartbeat bound, what the loop decides
//! about one job on one tick, and how a fired job re-arms.
//!
//! The codec's tests and the shape rules' stay in the legacy package, with the
//! `render` whose output they measure against.

use super::{HEARTBEAT_STALE_SECS, Job, Reason, Verdict, decide, rearm};

fn full() -> Job {
    Job {
        id: "nag:sess-123".to_string(),
        due: 1_700_000_000,
        until: 1_700_000_300,
        every: Some(30),
        unless_marker: Some("answered-sess-123".to_string()),
        args: vec![
            "--agent".to_string(),
            "pns".to_string(),
            "--detail".to_string(),
            "a nudge with spaces".to_string(),
        ],
    }
}

#[test]
fn a_slower_tick_than_the_bound_reads_a_healthy_daemon_as_not_running() {
    // S206, pinned. The bound is ten of the DEFAULT tick and is fixed at
    // compile time, while `PNS_DAEMON_TICK_MS` is read at run time and
    // admits far more. On any tick longer than the bound the previous beat
    // is ALREADY STALE when the next one is written, so the grader reads a
    // daemon that is running perfectly as not running.
    let beating = |age: u64| age <= HEARTBEAT_STALE_SECS;
    assert!(beating(HEARTBEAT_STALE_SECS));
    // One tick of a daemon told to look every thirty seconds.
    assert!(
        !beating(30),
        "a 30s tick outruns a {HEARTBEAT_STALE_SECS}s bound"
    );
    // The bound does not follow it, which is the whole statement.
    assert_eq!(HEARTBEAT_STALE_SECS, 10);
}

/// BOTH EDGES ARE CLOSED, and both are asserted, because a one-sided
/// bound is the bug class this window is most likely to acquire.
#[test]
fn a_job_fires_only_inside_its_window_and_both_edges_are_closed() {
    let job = full();
    for (case, now, expected) in [
        ("a second before due", job.due - 1, Verdict::Wait),
        ("exactly at due", job.due, Verdict::Fire),
        ("inside the window", job.due + 1, Verdict::Fire),
        ("exactly at until", job.until, Verdict::Fire),
        (
            "a second past until",
            job.until + 1,
            Verdict::Drop(Reason::LeaseExpired),
        ),
    ] {
        assert_eq!(decide(&job, now, false, false), expected, "case: {case}");
    }
}

/// THE LATE-STORM RULE. A laptop that slept through a job wakes to a lease
/// that expired while it was down, and the job is dropped rather than run
/// late, because "the machine was asleep" and "the nudge is now pointless"
/// are the same condition.
#[test]
fn a_job_whose_lease_expired_while_the_machine_slept_is_dropped_never_run_late() {
    let now = 1_700_003_600;
    let job = Job {
        due: now - 3_600,
        until: now - 3_540,
        ..full()
    };
    assert_eq!(
        decide(&job, now, false, false),
        Verdict::Drop(Reason::LeaseExpired)
    );
}

/// The nag primitive: an answer that arrived cancels the nudge before
/// anything runs.
#[test]
fn a_present_marker_cancels_the_job_before_anything_runs() {
    let job = full();
    // Squarely inside the window, so nothing but the marker can be what
    // dropped it.
    assert_eq!(
        decide(&job, job.due + 1, true, false),
        Verdict::Drop(Reason::MarkerPresent)
    );
    assert_eq!(decide(&job, job.due + 1, false, false), Verdict::Fire);
}

/// THE SEAMLESS BREATH'S OWN GUARD. A schedule that ends with its last
/// fade still in flight can no longer promise the previous child is gone
/// by the time the next occurrence is due, so a live child answers `Wait`
/// rather than `Fire`, exactly like a due second that has not arrived yet.
#[test]
fn a_running_child_holds_the_next_occurrence_to_a_wait_rather_than_a_fire() {
    let job = full();
    assert_eq!(
        decide(&job, job.due, false, true),
        Verdict::Wait,
        "due, with no marker, but its own child is still running"
    );
    assert_eq!(
        decide(&job, job.due, false, false),
        Verdict::Fire,
        "the control: the same job, the same second, with nothing running"
    );
}

/// A REPEAT CANNOT EXTEND ITS OWN LEASE, which is the assertion that
/// matters here: a job that renewed `until` as well as `due` would run
/// forever with nobody refreshing it, and the lamp it drives would lie in
/// exactly the direction the lease exists to prevent.
#[test]
fn a_repeating_job_re_arms_at_now_plus_every_and_a_one_shot_does_not_re_arm() {
    let job = full();
    let now = job.due;
    let next = rearm(&job, now).expect("a repeating job re-arms");
    assert_eq!(next.due, now + 30);
    assert_eq!(next.until, job.until, "the lease is UNCHANGED");
    assert_eq!(next.id, job.id);
    assert_eq!(next.args, job.args);

    assert_eq!(
        rearm(
            &Job {
                every: None,
                ..full()
            },
            now
        ),
        None,
        "a one-shot leaves nothing behind"
    );

    // FROM NOW, NEVER FROM `due`. A job the loop reaches late (a busy
    // tick, a woken laptop) whose next due were `due + every` would still
    // be in the past, so the daemon would fire it again immediately and
    // keep firing until it caught up: one burst instead of one repeat.
    let late = now + 100;
    let caught_up = rearm(&job, late).expect("a repeating job re-arms");
    assert_eq!(caught_up.due, late + 30);
    assert_ne!(caught_up.due, job.due + 30);

    // AND THE LEASE IS WHAT ENDS A REPEAT. A next occurrence past `until`
    // can never fire, so the job leaves nothing behind rather than a
    // record whose own due sits outside its lease.
    let last = job.until - 1;
    assert_eq!(rearm(&job, last), None);
    assert_eq!(
        rearm(&job, job.until - 30).map(|next| next.due),
        Some(job.until),
        "a next occurrence landing exactly on the lease still re-arms"
    );
}
