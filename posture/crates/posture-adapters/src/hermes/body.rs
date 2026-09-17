//! The JSON one page is posted as, and the only place the gateway's rendered
//! placeholders are named.
//!
//! EVERY KEY IS ALWAYS PRESENT, because the gateway renders `{placeholder}`
//! out of the posted body with no conditionals and emits an unknown one as its
//! own literal text. A body missing a key its route's prompt names therefore
//! delivers that key's braces to Discord instead of a page anyone can read.
//! Three prompt shapes are in use across the gateway's routes, so all three
//! are served at once: the flat `agent`/`state`/`project`/`detail` four, the
//! nested `alert.title` and `alert.detail` pair, and the
//! `header`/`subheader`/`body` triple. Each costs one key.

use crate::wire::Name;
use posture_application::{Alert, AlertSignal};
use posture_domain::Severity;

/// The name this tool answers to wherever a body names its sender.
const AGENT: &str = "posture";

/// The posted body. `state` is the page's own tier, `project` is the source
/// event, and `route` rides along so a delivery can be read back against the
/// route it was signed for. A copy leg posts this body unchanged to a second
/// route, so `route` there still names the page's own route, not the one the
/// copy travels on.
pub(super) fn encode(alert: &Alert, route: &Name) -> String {
    serde_json::json!({
        "agent": AGENT,
        "state": state(alert),
        "project": alert.event,
        "detail": format!("{}\n{}", alert.title, alert.detail),
        "route": route.as_str(),
        "alert": { "title": alert.title, "detail": alert.detail },
        "header": alert.title,
        "subheader": format!("{AGENT} \u{b7} {}", state(alert)),
        "body": alert.detail,
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
