use crate::SnapshotsLog;
use posture_domain::{canary_freshness, heartbeat_text};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallTime {
    pub seconds: u64,
    pub utc_day: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockUnavailable;
pub trait Clock {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSignal {
    Observation,
    NeedsAttention,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub occurrence_id: Option<String>,
    pub event: &'static str,
    pub signal: AlertSignal,
    pub occurred_at: Option<u64>,
    pub title: String,
    pub detail: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionFailure {
    Unavailable,
    Failed,
    TimedOut,
    Unparseable,
    Refused,
    NotCommitted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submission {
    Accepted,
    NotAccepted(SubmissionFailure),
}
// Accepted promises a committed retriable obligation for this request, before dispatch.
// The future pns adapter must establish that from the engine's actual diagnostic contract.
pub trait AlertSink {
    fn submit(&mut self, alert: &Alert) -> Submission;
}
pub struct Heartbeat<C, L, S> {
    pub clock: C,
    pub snapshots: L,
    pub sink: S,
    pub maximum_age: u64,
}
impl<C: Clock, L: SnapshotsLog, S: AlertSink> Heartbeat<C, L, S> {
    pub fn run(&mut self) {
        let time = self.clock.now().ok();
        let freshness = time.as_ref().map(|time| {
            canary_freshness(
                time.seconds,
                self.snapshots.newest_canary().ok().flatten(),
                self.maximum_age,
            )
        });
        let text = heartbeat_text(
            freshness,
            time.as_ref().map_or("", |time| time.utc_day.as_str()),
            self.maximum_age,
        );
        // This daily observation advances no state. The sink owns durable delivery;
        // a refusal does not turn the heartbeat into a retry loop or a security page.
        let _ = self.sink.submit(&Alert {
            occurrence_id: None,
            event: "heartbeat",
            signal: AlertSignal::Observation,
            occurred_at: time.map(|time| time.seconds),
            title: text.title,
            detail: text.detail,
        });
    }
}
#[cfg(test)]
mod tests;
