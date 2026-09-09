use crate::CanaryFreshness;
mod window;
pub use window::HeartbeatWindow;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatText {
    pub title: String,
    pub detail: String,
}
pub fn heartbeat_text(
    freshness: Option<CanaryFreshness>,
    day: &str,
    maximum_age: impl std::fmt::Display,
) -> HeartbeatText {
    let title = match freshness {
        None => "⚠️ osquery heartbeat · time unknown".to_owned(),
        Some(CanaryFreshness::Fresh { .. }) => format!("✅ osquery pipeline healthy · {day}"),
        _ => format!("⚠️ osquery heartbeat · {day}"),
    };
    let detail = match freshness {
        None => "- The heartbeat cannot determine the current time (the system clock read failed), so it cannot judge whether osqueryd is producing scheduled results. Treat this as unverified, not healthy. The uptime watchdog pages on a real outage; this note is the silent daily record.".to_owned(),
        Some(CanaryFreshness::Missing) => "- osqueryd scheduled heartbeat canary is MISSING (no canary snapshot found). The root daemon is not producing scheduled results, or has never run the schedule. The uptime watchdog pages on this; this note is the silent daily record.".to_owned(),
        Some(CanaryFreshness::Fresh { age }) => format!("- The root osqueryd daemon produced a scheduled heartbeat canary {age}s ago, so it was scheduling and producing results as recently as that. This is a recent observation, not a real-time check: the uptime watchdog owns real-time liveness and pages if a monitor is down. Silence since the last message means all clear."),
        Some(CanaryFreshness::Stale { age }) => format!("- osqueryd scheduled heartbeat canary is STALE (last {age}s ago, over {maximum_age}s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record."),
        Some(CanaryFreshness::Implausible { skew }) => format!("- osqueryd scheduled heartbeat canary timestamp is IMPLAUSIBLE ({skew}s in the future, over {maximum_age}s). This is clock skew or a bad row, not a trustworthy liveness signal. The uptime watchdog pages on a real outage; this note is the silent daily record."),
    };
    HeartbeatText { title, detail }
}
#[cfg(test)]
mod tests;
