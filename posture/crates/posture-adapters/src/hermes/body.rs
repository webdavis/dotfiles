//! The JSON one page is posted as, and the only place the gateway's rendered
//! placeholders are named.

use posture_application::{Alert, AlertSignal};
use posture_domain::Severity;
use posture_producer_wire::Name;

/// The posted body. `state` is the page's own tier, `project` is the source
/// event, and `route` rides along so a delivery can be read back against the
/// route it was signed for.
pub(super) fn encode(alert: &Alert, route: &Name) -> String {
    serde_json::json!({
        "agent": "posture",
        "state": state(alert),
        "project": alert.event,
        "detail": format!("{}\n{}", alert.title, alert.detail),
        "route": route.as_str(),
        "alert": { "title": alert.title, "detail": alert.detail },
    })
    .to_string()
}

/// The word the channel shows for this page: its tier when it was judged at
/// one, and otherwise what the submission is.
fn state(alert: &Alert) -> &'static str {
    match alert.severity {
        Some(Severity::Critical) => "critical",
        Some(Severity::Notice) => "notice",
        Some(Severity::Info) => "info",
        None => match alert.signal {
            AlertSignal::NeedsAttention => "needs-attention",
            AlertSignal::Observation => "observation",
        },
    }
}

#[cfg(test)]
mod tests;
