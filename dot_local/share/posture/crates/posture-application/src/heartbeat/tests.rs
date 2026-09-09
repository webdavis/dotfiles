use super::*;
use crate::SnapshotReadFailure;
use posture_domain::CanaryEpoch;
use std::{cell::RefCell, rc::Rc};
struct Fixture {
    calls: Rc<RefCell<Vec<&'static str>>>,
    time: Result<WallTime, ClockUnavailable>,
    canary: Result<Option<CanaryEpoch>, SnapshotReadFailure>,
    result: Submission,
    alerts: Vec<Alert>,
}
impl Clock for Fixture {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        self.calls.borrow_mut().push("clock");
        self.time.clone()
    }
}
impl SnapshotsLog for Fixture {
    fn newest_canary(&mut self) -> Result<Option<CanaryEpoch>, SnapshotReadFailure> {
        self.calls.borrow_mut().push("snapshots");
        self.canary
    }
}
impl AlertSink for Fixture {
    fn submit(&mut self, alert: &Alert) -> Submission {
        self.calls.borrow_mut().push("submit");
        self.alerts.push(alert.clone());
        self.result
    }
}
fn subject() -> Heartbeat<Fixture, Fixture, Fixture> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fixture = || Fixture {
        calls: calls.clone(),
        time: Ok(WallTime {
            seconds: 10_000,
            utc_day: "2026-09-07".into(),
        }),
        canary: Ok(CanaryEpoch::parse("9970")),
        result: Submission::Accepted,
        alerts: Vec::new(),
    };
    Heartbeat {
        clock: fixture(),
        snapshots: fixture(),
        sink: fixture(),
        maximum_age: HeartbeatWindow::from_override(None),
    }
}
#[test]
fn fresh_canary_submits_one_silent_observation_after_clock_and_log() {
    let mut sut = subject();
    sut.run();
    assert_eq!(*sut.clock.calls.borrow(), ["clock", "snapshots", "submit"]);
    assert_eq!(sut.sink.alerts.len(), 1);
    let alert = &sut.sink.alerts[0];
    assert_eq!(alert.event, "heartbeat");
    assert_eq!(alert.signal, AlertSignal::Observation);
    assert_eq!(alert.occurred_at, Some(10_000));
    assert_eq!(alert.title, "✅ osquery pipeline healthy · 2026-09-07");
    assert!(alert.detail.contains("canary 30s ago"));
}
#[test]
fn unknown_clock_submits_unverified_without_reading_snapshots() {
    let mut sut = subject();
    sut.clock.time = Err(ClockUnavailable);
    sut.run();
    assert_eq!(*sut.clock.calls.borrow(), ["clock", "submit"]);
    assert_eq!(sut.sink.alerts.len(), 1);
    let alert = &sut.sink.alerts[0];
    assert_eq!(alert.signal, AlertSignal::Observation);
    assert_eq!(alert.occurred_at, None);
    assert_eq!(alert.title, "⚠️ osquery heartbeat · time unknown");
    assert!(alert.detail.contains("cannot determine the current time"));
}
#[test]
fn missing_unreadable_stale_and_implausible_are_observations() {
    for (canary, word) in [
        (Ok(None), "MISSING"),
        (Err(SnapshotReadFailure), "MISSING"),
        (Ok(CanaryEpoch::parse("1")), "STALE"),
        (Ok(CanaryEpoch::parse("99999")), "IMPLAUSIBLE"),
    ] {
        let mut sut = subject();
        sut.snapshots.canary = canary;
        sut.run();
        assert_eq!(sut.sink.alerts.len(), 1);
        assert_eq!(sut.sink.alerts[0].signal, AlertSignal::Observation);
        assert!(sut.sink.alerts[0].detail.contains(word), "{word}");
    }
}
#[test]
fn every_submission_failure_is_fire_and_forget_without_retry_or_state() {
    for failure in [
        SubmissionFailure::Unavailable,
        SubmissionFailure::Failed,
        SubmissionFailure::TimedOut,
        SubmissionFailure::Unparseable,
        SubmissionFailure::Refused,
        SubmissionFailure::NotCommitted,
    ] {
        let mut sut = subject();
        sut.sink.result = Submission::NotAccepted(failure);
        sut.run();
        assert_eq!(*sut.clock.calls.borrow(), ["clock", "snapshots", "submit"]);
        assert_eq!(sut.sink.alerts.len(), 1);
    }
}
