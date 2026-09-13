use crate::{Alert, AlertSignal, AlertSink, Clock, IndependentAlarm, SnapshotsLog, Submission};
use posture_domain::{
    Agent, AgentReading, AgentState, AuditFingerprint, AuditMemory, QueueCounts, QueueKind,
    QueueMemory, judge_agent, judge_audit, judge_queue, osquery_problem, route_problem,
    state_problem, watchdog_page,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WatchdogState {
    pub agents: [Option<AgentState>; 6],
    pub legacy_pending: QueueMemory,
    pub pns_pending: QueueMemory,
    pub pipeline_audit: AuditMemory,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonHealth {
    Unloaded,
    NotRunning,
    Running,
}
pub trait WatchdogProcesses {
    fn osquery_running(&mut self) -> bool;
    fn agent(&mut self, agent: Agent) -> AgentReading<'_>;
    fn pns_daemon(&mut self) -> DaemonHealth;
}
pub trait GatewayHealth {
    fn status(&mut self) -> Option<u16>;
}
pub trait QueueHealth {
    fn counts(&mut self) -> QueueCounts;
}
pub struct AuditObservation {
    pub completed: bool,
    pub report: String,
    pub fingerprint: Option<AuditFingerprint>,
}
pub trait WatchdogIntegrity {
    fn pipeline(&mut self) -> AuditObservation;
    fn pns_problem(&mut self) -> Option<String>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchdogStateFailure;
pub trait WatchdogStateStore {
    fn load(&mut self) -> WatchdogState;
    fn writable(&mut self) -> bool;
    fn publish(&mut self, state: &WatchdogState) -> Result<(), WatchdogStateFailure>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogOutcome {
    Healthy,
    HealthyStateLost,
    Reported,
    DeliveryFailed,
    IndependentAlarmFailed,
    StateFailed,
}
pub struct Watchdog<'a> {
    pub clock: &'a mut dyn Clock,
    pub snapshots: &'a mut dyn SnapshotsLog,
    pub processes: &'a mut dyn WatchdogProcesses,
    pub gateway: &'a mut dyn GatewayHealth,
    pub legacy_queue: &'a mut dyn QueueHealth,
    pub pns_ledger: &'a mut dyn QueueHealth,
    pub integrity: &'a mut dyn WatchdogIntegrity,
    pub state: &'a mut dyn WatchdogStateStore,
    pub sink: &'a mut dyn AlertSink,
    pub alarm: &'a mut dyn IndependentAlarm,
    pub maximum_age: u64,
    pub gateway_url: &'a str,
    pub state_path: &'a str,
}
impl Watchdog<'_> {
    pub fn run(&mut self) -> WatchdogOutcome {
        let previous = self.state.load();
        let mut next = WatchdogState::default();
        let mut problems = Vec::new();
        problems.extend(state_problem(self.state.writable(), self.state_path));
        let now = self.clock.now().ok().map(|time| time.seconds);
        problems.extend(osquery_problem(
            now,
            self.processes.osquery_running(),
            self.snapshots.newest_canary().ok().flatten(),
            self.maximum_age,
        ));
        for (index, agent) in Agent::ALL.into_iter().enumerate() {
            let judgment = judge_agent(
                agent,
                self.processes.agent(agent),
                previous.agents[index].unwrap_or_default(),
            );
            next.agents[index] = judgment.state;
            problems.extend(judgment.problem);
        }
        let status = self.gateway.status().map(|status| status.to_string());
        problems.extend(route_problem(status.as_deref(), self.gateway_url));
        let (memory, queue_problems) = judge_queue(
            QueueKind::Legacy,
            self.legacy_queue.counts(),
            previous.legacy_pending,
        );
        next.legacy_pending = memory;
        problems.extend(queue_problems);
        let audit = self.integrity.pipeline();
        let judged = judge_audit(
            audit.completed,
            &audit.report,
            audit.fingerprint,
            &previous.pipeline_audit,
        );
        next.pipeline_audit = judged.next;
        problems.extend(judged.problem);

        let mut independent = Vec::new();
        independent.extend(self.integrity.pns_problem());
        independent.extend(match self.processes.pns_daemon() {
            DaemonHealth::Unloaded => {
                Some("LaunchAgent not loaded: com.webdavis.pns-daemon".to_owned())
            }
            DaemonHealth::NotRunning => {
                Some("LaunchAgent is not running: com.webdavis.pns-daemon".to_owned())
            }
            DaemonHealth::Running => None,
        });
        let (memory, queue_problems) = judge_queue(
            QueueKind::Pns,
            self.pns_ledger.counts(),
            previous.pns_pending,
        );
        next.pns_pending = memory;
        independent.extend(queue_problems);
        let mut alarm_failed = false;
        for problem in &independent {
            if self
                .alarm
                .alarm("Posture notification engine unhealthy", problem)
                .is_err()
            {
                alarm_failed = true;
            }
        }
        problems.extend(independent);
        let Some(page) = watchdog_page(&problems) else {
            return if self.state.publish(&next).is_ok() {
                WatchdogOutcome::Healthy
            } else {
                WatchdogOutcome::HealthyStateLost
            };
        };
        let submitted = self.sink.submit(&Alert {
            occurrence_id: None,
            event: "watchdog",
            signal: AlertSignal::NeedsAttention,
            occurred_at: now,
            title: page.title,
            detail: page.body,
        });
        // A forged accepted response cannot acknowledge a failed independent alarm.
        // Preserve every prior baseline so the next tick can report the same evidence.
        if alarm_failed {
            return WatchdogOutcome::IndependentAlarmFailed;
        }
        if submitted != Submission::Accepted {
            return WatchdogOutcome::DeliveryFailed;
        }
        if self.state.publish(&next).is_err() {
            return WatchdogOutcome::StateFailed;
        }
        WatchdogOutcome::Reported
    }
}
#[cfg(test)]
mod tests;
