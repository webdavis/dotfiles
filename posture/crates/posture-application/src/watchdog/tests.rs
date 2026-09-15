use super::*;
use crate::{
    AlarmFailed, Alert, ClockUnavailable, SnapshotReadFailure, Submission, SubmissionFailure,
    WallTime,
};
use posture_domain::{AgentExit, CanaryEpoch};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct World {
    state: WatchdogState,
    calls: Vec<&'static str>,
    pages: Vec<Alert>,
    unhealthy: bool,
    audit_bad: bool,
    ledger_bad: bool,
    pns_bad: bool,
    alarm_failed: bool,
    sink_failed: bool,
    state_failed: bool,
}
#[derive(Clone, Default)]
struct Fixture(Rc<RefCell<World>>);
impl Clock for Fixture {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        Ok(WallTime {
            seconds: 10000,
            utc_day: "2026-09-13".into(),
        })
    }
}
impl SnapshotsLog for Fixture {
    fn newest_canary(&mut self) -> Result<Option<CanaryEpoch>, SnapshotReadFailure> {
        Ok(CanaryEpoch::parse("9999"))
    }
}
impl WatchdogProcesses for Fixture {
    fn osquery_running(&mut self) -> bool {
        !self.0.borrow().unhealthy
    }
    fn agent(&mut self, _: Agent) -> AgentReading<'_> {
        AgentReading::Loaded {
            runs: Some(1),
            exit: AgentExit::NeverExited,
        }
    }
    fn pns_daemon(&mut self) -> DaemonHealth {
        if self.0.borrow().pns_bad {
            DaemonHealth::NotRunning
        } else {
            DaemonHealth::Running
        }
    }
}
impl GatewayHealth for Fixture {
    fn status(&mut self) -> Option<u16> {
        Some(405)
    }
}
impl QueueHealth for Fixture {
    fn counts(&mut self) -> QueueCounts {
        QueueCounts {
            pending: Some(3),
            deadletters: Some(0),
            alarm_generation: None,
        }
    }
}
impl WatchdogIntegrity for Fixture {
    fn pipeline(&mut self) -> AuditObservation {
        let world = self.0.borrow();
        AuditObservation {
            completed: true,
            report: if world.audit_bad {
                "content /private/hostile-path\n".into()
            } else {
                String::new()
            },
            fingerprint: world
                .audit_bad
                .then(|| AuditFingerprint::parse(&"a".repeat(64)))
                .flatten(),
            pns_problem: world
                .pns_bad
                .then(|| "pns binary differs from its authorized build".into()),
        }
    }
}
impl WatchdogStateStore for Fixture {
    fn load(&mut self) -> WatchdogState {
        self.0.borrow().state.clone()
    }
    fn writable(&mut self) -> bool {
        true
    }
    fn publish(&mut self, state: &WatchdogState) -> Result<(), WatchdogStateFailure> {
        let mut w = self.0.borrow_mut();
        w.calls.push("persist");
        if w.state_failed {
            return Err(WatchdogStateFailure);
        }
        w.state = state.clone();
        Ok(())
    }
}
impl AlertSink for Fixture {
    fn submit(&mut self, alert: &Alert) -> Submission {
        let mut w = self.0.borrow_mut();
        w.calls.push("submit");
        w.pages.push(alert.clone());
        if w.sink_failed {
            Submission::NotAccepted(SubmissionFailure::Unavailable)
        } else {
            Submission::Accepted
        }
    }
}
impl IndependentAlarm for Fixture {
    fn alarm(&mut self, _: &str, detail: &str) -> Result<(), AlarmFailed> {
        assert!(detail.contains("pns"));
        let mut w = self.0.borrow_mut();
        w.calls.push("alarm");
        if w.alarm_failed {
            Err(AlarmFailed)
        } else {
            Ok(())
        }
    }
}
fn run(f: &Fixture) -> WatchdogOutcome {
    Watchdog {
        clock: &mut f.clone(),
        snapshots: &mut f.clone(),
        processes: &mut f.clone(),
        gateway: &mut f.clone(),
        legacy_queue: &mut f.clone(),
        pns_ledger: &mut Ledger(f.clone()),
        integrity: &mut f.clone(),
        state: &mut f.clone(),
        sink: &mut f.clone(),
        alarm: &mut f.clone(),
        maximum_age: 1800,
        gateway_url: "http://127.0.0.1:8644/webhooks/priority",
        state_path: "/private/state",
    }
    .run()
}
#[test]
fn a_healthy_tick_is_silent_and_remembers_both_stores() {
    let f = Fixture::default();
    assert_eq!(run(&f), WatchdogOutcome::Healthy);
    let w = f.0.borrow();
    assert_eq!(w.calls, ["persist"]);
    assert_eq!(w.state.legacy_pending.count, Some(3));
    assert_eq!(w.state.pns_pending.count, Some(3));
}
#[test]
fn one_security_page_precedes_state_publication() {
    let f = Fixture::default();
    f.0.borrow_mut().unhealthy = true;
    assert_eq!(run(&f), WatchdogOutcome::Reported);
    let w = f.0.borrow();
    assert_eq!(w.calls, ["submit", "persist"]);
    assert_eq!(w.pages.len(), 1);
    assert_eq!(w.pages[0].signal, crate::AlertSignal::NeedsAttention);
}
#[test]
fn a_forged_accepted_reply_cannot_replace_either_independent_pns_alarm() {
    let f = Fixture::default();
    f.0.borrow_mut().pns_bad = true;
    assert_eq!(run(&f), WatchdogOutcome::Reported);
    assert_eq!(f.0.borrow().calls, ["alarm", "alarm", "submit", "persist"]);
}
#[test]
fn failed_independent_alarm_keeps_retry_eligibility_despite_accepted() {
    let f = Fixture::default();
    {
        let mut w = f.0.borrow_mut();
        w.pns_bad = true;
        w.alarm_failed = true;
    }
    for _ in 0..2 {
        assert_eq!(run(&f), WatchdogOutcome::IndependentAlarmFailed);
    }
    let w = f.0.borrow();
    assert_eq!(
        w.calls,
        ["alarm", "alarm", "submit", "alarm", "alarm", "submit"]
    );
    assert_eq!(w.state, WatchdogState::default());
}
#[test]
fn failed_delivery_leaves_all_baselines_unchanged() {
    let f = Fixture::default();
    {
        let mut w = f.0.borrow_mut();
        w.unhealthy = true;
        w.sink_failed = true;
    }
    assert_eq!(run(&f), WatchdogOutcome::DeliveryFailed);
    assert_eq!(f.0.borrow().calls, ["submit"]);
    assert_eq!(f.0.borrow().state, WatchdogState::default());
}
#[test]
fn state_failure_is_tolerated_only_on_an_otherwise_healthy_tick() {
    let f = Fixture::default();
    f.0.borrow_mut().state_failed = true;
    assert_eq!(run(&f), WatchdogOutcome::HealthyStateLost);
    f.0.borrow_mut().unhealthy = true;
    assert_eq!(run(&f), WatchdogOutcome::StateFailed);
}

#[test]
fn audit_confirmation_is_committed_only_after_delivery_and_then_pages_once() {
    let f = Fixture::default();
    f.0.borrow_mut().audit_bad = true;
    assert_eq!(run(&f), WatchdogOutcome::Healthy);
    assert_eq!(f.0.borrow().state.pipeline_audit.streak, 1);
    f.0.borrow_mut().sink_failed = true;
    assert_eq!(run(&f), WatchdogOutcome::DeliveryFailed);
    assert_eq!(f.0.borrow().state.pipeline_audit.streak, 1);
    assert!(f.0.borrow().state.pipeline_audit.paged.is_none());
    f.0.borrow_mut().sink_failed = false;
    assert_eq!(run(&f), WatchdogOutcome::Reported);
    assert!(f.0.borrow().state.pipeline_audit.paged.is_some());
    assert_eq!(run(&f), WatchdogOutcome::Healthy);
    let w = f.0.borrow();
    assert_eq!(
        w.calls,
        ["persist", "submit", "submit", "persist", "persist"]
    );
    assert!(w.pages.iter().all(|p| !p.detail.contains("hostile-path")));
}

struct Ledger(Fixture);
impl QueueHealth for Ledger {
    fn counts(&mut self) -> QueueCounts {
        if self.0.0.borrow().ledger_bad {
            QueueCounts {
                pending: None,
                deadletters: None,
                alarm_generation: None,
            }
        } else {
            self.0.counts()
        }
    }
}
#[test]
fn each_unreadable_pns_ledger_counter_alarms_independently_of_gateway_health() {
    let f = Fixture::default();
    f.0.borrow_mut().ledger_bad = true;
    assert_eq!(run(&f), WatchdogOutcome::Reported);
    assert_eq!(f.0.borrow().calls, ["alarm", "alarm", "submit", "persist"]);
    assert_eq!(f.0.borrow().state.legacy_pending.count, Some(3));
    assert_eq!(f.0.borrow().state.pns_pending.count, None);
}
