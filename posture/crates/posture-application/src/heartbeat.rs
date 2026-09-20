use crate::SnapshotsLog;
use posture_domain::{HeartbeatWindow, Severity, canary_freshness, heartbeat_text};
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
    /// The tier this submission was judged at, which decides its route.
    ///
    /// `None` for everything that is not a judged finding, and the sink then
    /// keeps the route it was configured with. See `severity_route`.
    pub severity: Option<Severity>,
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
// A delivery sink establishes that from the engine's correlated ledger_committed diagnostic.
pub trait AlertSink {
    fn submit(&mut self, alert: &Alert) -> Submission;
}
/// A boxed sink IS a sink, so a composition root that picks between delivery
/// paths at run time hands every use case one word for "wherever a page goes"
/// rather than making each of them generic over the choice.
impl<S: AlertSink + ?Sized> AlertSink for Box<S> {
    fn submit(&mut self, alert: &Alert) -> Submission {
        (**self).submit(alert)
    }
}
pub struct Heartbeat<C, L, S> {
    pub clock: C,
    pub snapshots: L,
    pub sink: S,
    pub maximum_age: HeartbeatWindow,
}
impl<C: Clock, L: SnapshotsLog, S: AlertSink> Heartbeat<C, L, S> {
    /// Raise the daily observation and answer whether a destination took it.
    ///
    /// THE ANSWER IS THE POINT. The heartbeat exists to prove the pipeline is
    /// alive, so a run that could not deliver has proven nothing and its
    /// caller must say so rather than exit as though it had.
    pub fn run(&mut self) -> Submission {
        let time = self.clock.now().ok();
        let freshness = time.as_ref().map(|time| {
            canary_freshness(
                time.seconds,
                self.snapshots.newest_canary().ok().flatten(),
                self.maximum_age.seconds(),
            )
        });
        let text = heartbeat_text(
            freshness,
            time.as_ref().map_or("", |time| time.utc_day.as_str()),
            self.maximum_age.display(),
        );
        // This daily observation advances no state. The sink owns durable delivery;
        // a refusal does not turn the heartbeat into a retry loop or a security page.
        self.sink.submit(&Alert {
            occurrence_id: None,
            event: "heartbeat",
            signal: AlertSignal::Observation,
            severity: None,
            occurred_at: time.map(|time| time.seconds),
            title: text.title,
            detail: text.detail,
        })
    }
}
#[cfg(test)]
mod tests;
