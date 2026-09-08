mod agents;
mod audit;
mod page;

use crate::{CanaryEpoch, CanaryFreshness, canary_freshness};
pub use agents::{
    Agent, AgentExit, AgentJudgment, AgentReading, AgentState, ExitCode, judge_agent,
};
pub use audit::{
    AuditFingerprint, AuditJudgment, AuditMemory, audit_fingerprint_input, judge_audit,
};
pub use page::{WatchdogPage, watchdog_page};

// A live process alone does not prove its schedule is running. A trustworthy clock
// must precede the shared canary judgment; results.log mtime is deliberately inert.
pub fn osquery_problem(
    now: Option<u64>,
    running: bool,
    canary: Option<CanaryEpoch>,
    max_age: u64,
) -> Option<String> {
    let Some(now) = now else {
        return Some("the watchdog cannot read the system clock, so it cannot verify osqueryd is producing scheduled results".into());
    };
    if !running {
        return Some("osqueryd is not running".into());
    }
    match canary_freshness(now, canary, max_age) {
        CanaryFreshness::Fresh { .. } => None,
        CanaryFreshness::Missing => Some("osqueryd is not producing scheduled results (the heartbeat canary is MISSING); the daemon is stopped or wedged".into()),
        CanaryFreshness::Stale { age } => Some(format!("osqueryd is not producing scheduled results (the heartbeat canary is STALE, {age}s old); the daemon is stopped or wedged")),
        CanaryFreshness::Implausible { skew } => Some(format!("osqueryd heartbeat canary timestamp is IMPLAUSIBLE ({skew}s in the future); clock skew or a bad row, not a trustworthy liveness signal")),
    }
}
pub fn route_problem(status: Option<&str>, url: &str) -> Option<String> {
    let code = status
        .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or("000");
    if code == "405" || (code.len() == 3 && code.starts_with('2')) {
        return None;
    }
    Some(format!(
        "hermes #priority route unhealthy (HTTP {code}) at {url}"
    ))
}
pub fn state_problem(writable: bool, path: &str) -> Option<String> {
    (!writable).then(|| format!("the watchdog cannot persist its state ({path}); the crash-loop and backlog-growth alarms are degraded until this is fixed"))
}
#[cfg(test)]
mod tests;
