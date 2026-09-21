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
use pns_hermes::{PostOutcome, SignedPost, delivered, outcome_line, sign, skipped_line};
use std::time::Duration;

/// The gateway when `PNS_HERMES_URL` says nothing: the local hermes
/// webhook route.
///
/// ITS FINAL SEGMENT IS `routes::DEFAULT_ROUTE`, which a test in this module
/// pins: the route name is what selects the signing key, so a default URL
/// pointing at one route while the key came from another would sign every
/// unrouted post with the wrong secret.
pub const DEFAULT_HERMES_URL: &str = "http://127.0.0.1:8644/webhooks/pns-events";

/// The gateway body carries the original request id, agent, state, project
/// and the FULL message as detail, because Discord has no preview ceiling,
/// plus the two composed header lines and the bare body they head.
///
/// EVERY KEY IS ALWAYS PRESENT. The gateway renders `{key}` from the posted
/// body with no conditionals, so a key a body omits reaches the channel as
/// the literal text `{key}`; `thread_id` is therefore posted empty until
/// hermes can hand back a thread it created. The original five keys stay, so
/// a route nobody has retemplated keeps rendering.
///
/// THE TWO LINES ARE COMPOSED HERE, off the event's own parts, because this
/// is the one destination that renders them. A RETRY IS WHY THAT MATTERS:
/// the retry loop rebuilds its event out of the ledger row alone, which
/// carries the project, branch, state and agent, so a retried post's first
/// line is byte for byte the one the first attempt sent. The ledger keeps no
/// session, so a retried dim line names the agent alone rather than the
/// session and its title; adding a session column to the delivery ledger is
/// deliberately out of scope (design, 2026-09-14). The reminder's coalesced
/// nudge names the agent alone for a different reason, and deliberately: it
/// stands for every outstanding approval at once, so naming one of their
/// sessions would say something false.
pub fn hermes_body(event: &Event, request_id: &str) -> String {
    body_with_id(event, Some(request_id))
}

fn body_with_id(event: &Event, request_id: Option<&str>) -> String {
    let mut body = serde_json::json!({
        "agent": event.agent,
        "state": event.state,
        "project": event.project,
        "detail": event.message,
        "header": pns_domain::render::header(&event.project, &event.branch, &event.state),
        "subheader": pns_domain::render::subheader(
            &event.agent,
            &pns_domain::render::short_session(&event.session),
            &event.session_title,
        ),
        // The BARE detail, because `detail` above keeps the branch prefix the
        // banner needs and the header already names the branch.
        "body": event.detail,
        "thread_id": "",
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
/// caller is blocked on it, and configurable for the same reason: it is what
/// `[delivery] remote_deadline` ships at.
pub const DEFAULT_REMOTE_DEADLINE_SECS: u64 = 5;

/// The ceiling a configured sync deadline is clamped to: a day is already
/// longer than any notification can matter, and it keeps an absurd value out
/// of ureq's deadline arithmetic.
const MAX_SYNC_DEADLINE_SECS: u64 = 86_400;

/// The sync deadline `[delivery] remote_deadline` asked for, clamped.
pub fn remote_deadline(seconds: u64) -> Option<Duration> {
    // Zero is curl's `-m 0`: no deadline at all, and operator intent rather
    // than a default.
    (seconds != 0).then(|| Duration::from_secs(seconds.min(MAX_SYNC_DEADLINE_SECS)))
}

/// The native hermes plugin.
pub struct HermesChannel<P: SignedPost> {
    pub post: P,
    /// The route this channel posts to, resolved at the composition root and
    /// never empty: it is what `url` was built from and what `key` was looked
    /// up by, so the two cannot name different routes.
    pub route: String,
    /// The signing key FOR THAT ROUTE, looked up in `[plugins.log.keys]`
    /// at the composition root. None is the not-set-up case, which for a
    /// route is now its own state rather than the whole channel's.
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
            //
            // AND IT IS THE FAIL-CLOSED HALF OF PER-ROUTE KEYS. A route the
            // config names no key for posts nothing and says so, rather than
            // borrowing another route's key to get the page out.
            return Delivery::Failed(skipped_line(&self.route));
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
            return Delivery::Delivered(line);
        }
        // The channel REPORTS what happened; it does not decide whether the
        // gateway will ever accept this page. That judgement is one rule in
        // `pns_domain::retry`, applied once where the outcome is recorded, so a
        // second destination cannot disagree with this one about a 404.
        //
        // This used to name four statuses here and call everything else a plain
        // failure, which is how a 404 on a route that does not exist retried on
        // the same schedule as an unreachable gateway.
        match outcome {
            PostOutcome::Status(status) => Delivery::Rejected {
                status,
                detail: line,
            },
            // `NoStatus` (a request never put on the wire) is a DIFFERENT fault
            // from `NoResponse` (a request nothing answered), and it stays
            // `Failed` anyway: `Delivery::Unlaunched` never prints, and the
            // module rule above is that a sync post says its failure aloud.
            // So the ledger records both as `no-response`. Task 33's route
            // check is what tells the two apart for the operator, by asking the
            // gateway about the route instead of guessing from a stored kind.
            PostOutcome::NoStatus | PostOutcome::NoResponse => Delivery::Failed(line),
        }
    }
}

mod probe;
pub use probe::{probe_route, probe_routes};

#[cfg(test)]
mod default_route_tests {
    use super::{DEFAULT_HERMES_URL, channel_url};
    use pns_domain::routes::Routes;

    /// THE MUTANT THIS PINS: the shipped default route renamed without this
    /// URL. An unrouted post takes `DEFAULT_HERMES_URL` and is signed with the
    /// default route's key, so a disagreement between the two signs it with a
    /// key the gateway will not verify and the whole durable log goes quiet at
    /// 401.
    #[test]
    fn the_default_url_ends_at_the_default_route() {
        let shipped = Routes::default().default_route().to_string();
        assert!(
            DEFAULT_HERMES_URL.ends_with(&format!("/{shipped}")),
            "{DEFAULT_HERMES_URL} does not end at the {shipped} route"
        );
    }

    /// AND A CONFIGURED NAME CANNOT DISAGREE WITH IT EITHER, because the
    /// segment is swapped rather than assumed: `[routes] default = "logbook"`
    /// posts to the gateway's `logbook` route on the same host and port.
    #[test]
    fn a_renamed_default_route_moves_the_path_and_not_the_gateway() {
        assert_eq!(
            channel_url(DEFAULT_HERMES_URL, "logbook").as_deref(),
            Some("http://127.0.0.1:8644/webhooks/logbook")
        );
    }
}

#[cfg(test)]
#[path = "hermes/tests/mod.rs"]
mod tests;
