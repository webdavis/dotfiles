//! The hermes channel, native: the durable Discord log, one signed POST to
//! the local gateway.
//!
//! SYNC MODE EXISTS TO BE SEEN. The weekly job records must be able to see a
//! delivery failure: a 401 swallowed silently leaves the Discord channel
//! empty, and an empty channel looks like the jobs stopped running. So sync
//! posts print the HTTP status and the no-key case says so aloud, while
//! async stays silent like every other leg. The signing key never reaches
//! argv, a child environment, or any printed line; the signature is computed
//! in-process over the exact body bytes.

use super::{Delivery, Event};
use pns_application::{DeliveryRequest, DestinationId, NotificationDestination};
use pns_domain::registry::Routing;
use pns_domain::routing::ReportMode;
use pns_hermes::{SignedPost, delivered, outcome_line, sign, skipped_line};
use std::time::Duration;

/// The gateway when `PNS_HERMES_URL` says nothing: the local hermes
/// webhook route.
pub const DEFAULT_HERMES_URL: &str = "http://127.0.0.1:8644/webhooks/pns";

/// The gateway body carries the original request id, agent, state, project
/// and the FULL message as detail, because Discord has no preview ceiling.
pub fn hermes_body(event: &Event, request_id: &str) -> String {
    body_with_id(event, Some(request_id))
}

fn body_with_id(event: &Event, request_id: Option<&str>) -> String {
    let mut body = serde_json::json!({
        "agent": event.agent,
        "state": event.state,
        "project": event.project,
        "detail": event.message,
    });
    if let Some(id) = request_id {
        body["request_id"] = serde_json::json!(id);
    }
    body.to_string()
}

/// The URL for a NAMED route on the same gateway the default posts to: the
/// base URL with its final path segment swapped for the route. Names, not
/// URLs, cross the CLI: the gateway and its route table stay the single
/// source of truth in the hermes config, and a caller says only WHERE.
///
/// `None` for a route that could not safely become a path segment; the
/// caller says so and posts to the default, because a misrouted notification
/// on the loud route beats a silently dropped one.
pub fn channel_url(base_url: &str, route: &str) -> Option<String> {
    if !pns_domain::safety::route_name_is_usable(route) {
        return None;
    }
    let (prefix, _default_route) = base_url.rsplit_once('/')?;
    Some(format!("{prefix}/{route}"))
}

/// The deadline an ASYNC leg posts under. Not configurable: nobody is waiting
/// on the answer, so this only bounds how long a background process lingers.
const ASYNC_DEADLINE: Duration = Duration::from_secs(10);

/// The default SYNC deadline, the one a caller waits out. Short because the
/// caller is blocked on it, and configurable for the same reason.
const DEFAULT_SYNC_DEADLINE_SECS: u64 = 5;

/// The ceiling a configured sync deadline is clamped to: a day is already
/// longer than any notification can matter, and it keeps an absurd value out
/// of ureq's deadline arithmetic.
const MAX_SYNC_DEADLINE_SECS: u64 = 86_400;

/// The sync deadline: `PNS_REMOTE_TIMEOUT` validated as a count, else 5
/// seconds, because a garbled deadline must not become zero or forever.
pub fn remote_deadline(env_value: Option<&str>) -> Option<Duration> {
    let seconds = env_value
        .and_then(pns_domain::count::parse_count)
        .unwrap_or(DEFAULT_SYNC_DEADLINE_SECS);
    // Zero is curl's `-m 0`: no deadline at all, and caller intent rather
    // than a default.
    (seconds != 0).then(|| Duration::from_secs(seconds.min(MAX_SYNC_DEADLINE_SECS)))
}

/// The native hermes plugin.
pub struct HermesChannel<P: SignedPost> {
    pub post: P,
    /// The signing key, read from the config at the composition root. None
    /// is the not-set-up case.
    pub key: Option<String>,
    /// `PNS_HERMES_URL` override, else the default.
    pub url: String,
    /// The sync deadline, already validated at the edge; None is curl's
    /// explicit no-deadline.
    pub sync_deadline: Option<Duration>,
}

impl<P: SignedPost + Send + Sync> NotificationDestination for HermesChannel<P> {
    fn id(&self) -> &DestinationId {
        const ID: DestinationId = DestinationId::new("hermes");
        &ID
    }

    fn capabilities(&self) -> Routing {
        Routing {
            local: false,
            presence_gated: false,
            durable: true,
            event_dispatched: true,
        }
    }

    fn deliver(&self, request: &DeliveryRequest<'_>) -> Delivery {
        let body = body_with_id(request.event, request.request_id);
        let Some(signature) = self.key.as_deref().and_then(|key| sign(key, &body)) else {
            // NOT SET UP IS A FAILED VERDICT, because from the record's point
            // of view it reads the same as a refusal: the entry is not there.
            // The sentence still says which of the two it was, and an empty
            // Discord channel otherwise looks like the jobs stopped.
            return Delivery::Failed(skipped_line());
        };

        let deadline = match request.mode {
            ReportMode::ReportOutcome => self.sync_deadline,
            ReportMode::Silent => Some(ASYNC_DEADLINE),
        };
        let outcome = self
            .post
            .post(&self.url, &body, &signature, request.request_id, deadline);
        let line = outcome_line(outcome);
        if delivered(outcome) {
            Delivery::Delivered(line)
        } else {
            Delivery::Failed(line)
        }
    }
}

#[cfg(test)]
#[path = "hermes/tests/mod.rs"]
mod tests;
