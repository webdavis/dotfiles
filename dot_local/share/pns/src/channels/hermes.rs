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
use crate::routing::ReportMode;
use std::time::Duration;

/// The gateway when `PNS_HERMES_URL` says nothing: the local hermes
/// webhook route.
pub const DEFAULT_HERMES_URL: &str = "http://127.0.0.1:8644/webhooks/pns";

/// What one signed POST came back with: a status code, or no response at
/// all, which sync mode reports as its own distinct failure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostOutcome {
    Status(u16),
    /// A request that went out and got no HTTP status back: curl's 000.
    NoResponse,
    /// A request that could not even be attempted (a malformed URL): curl's
    /// empty status, reported with its own wording.
    NoStatus,
}

/// The POST seam: body plus signature header in, outcome out. The production
/// impl honors the deadline; a fake records everything.
pub trait SignedPost {
    /// `deadline` None means NO deadline, curl's `-m 0`: explicit caller
    /// intent, not a default.
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome;
}

/// The gateway body: agent, state, project, and the FULL message as the
/// detail, because Discord has no length ceiling for the preview to serve.
pub fn hermes_body(event: &Event) -> String {
    serde_json::json!({
        "agent": event.agent,
        "state": event.state,
        "project": event.project,
        "detail": event.message,
    })
    .to_string()
}

/// The lowercase hex HMAC-SHA256 of the body under the signing key, or None
/// when the key is empty, which is the not-set-up case.
pub fn sign(secret: &str, body: &str) -> Option<String> {
    use hmac::{Hmac, KeyInit, Mac};
    if secret.is_empty() {
        return None;
    }
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(body.as_bytes());
    Some(
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

pub use pns_adapters::hermes_secret;

/// The URL for a NAMED route on the same gateway the default posts to: the
/// base URL with its final path segment swapped for the route. Names, not
/// URLs, cross the CLI: the gateway and its route table stay the single
/// source of truth in the hermes config, and a caller says only WHERE.
///
/// `None` for a route that could not safely become a path segment; the
/// caller says so and posts to the default, because a misrouted notification
/// on the loud route beats a silently dropped one.
pub fn channel_url(base_url: &str, route: &str) -> Option<String> {
    if !crate::safety::route_name_is_usable(route) {
        return None;
    }
    let (prefix, _default_route) = base_url.rsplit_once('/')?;
    Some(format!("{prefix}/{route}"))
}

/// The status codes that mean the record reached the gateway. WRITTEN ONCE:
/// both the sentence and the verdict read it, so the rule cannot be moved for
/// one and left standing for the other, which would have a doctor call a post
/// good while the printed line called it FAILED.
const DELIVERED_STATUS: std::ops::Range<u16> = 200..300;

/// Whether one answer means the record arrived.
pub fn delivered(outcome: PostOutcome) -> bool {
    matches!(outcome, PostOutcome::Status(code) if DELIVERED_STATUS.contains(&code))
}

/// The line sync mode prints for one outcome, exactly as the bash spells it
/// minus the `pns: ` prefix, which the one print site adds.
pub fn outcome_line(outcome: PostOutcome) -> String {
    match outcome {
        PostOutcome::Status(code) if delivered(outcome) => format!("posted HTTP {code}"),
        PostOutcome::Status(code) => format!("post FAILED HTTP {code}"),
        PostOutcome::NoStatus => "post FAILED (curl reported no HTTP status at all)".to_string(),
        PostOutcome::NoResponse => {
            "post FAILED HTTP 000 (no response; is the hermes gateway up?)".to_string()
        }
    }
}

/// The line sync mode prints when there is no signing key. It names the
/// config key to write, because "not set up" without an address sends the
/// operator hunting.
pub fn skipped_line() -> String {
    "post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent"
        .to_string()
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
        .and_then(crate::parse_count)
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

impl<P: SignedPost> HermesChannel<P> {
    pub fn deliver(&self, event: &Event, mode: ReportMode) -> Delivery {
        let body = hermes_body(event);
        let Some(signature) = self.key.as_deref().and_then(|key| sign(key, &body)) else {
            // NOT SET UP IS A FAILED VERDICT, because from the record's point
            // of view it reads the same as a refusal: the entry is not there.
            // The sentence still says which of the two it was, and an empty
            // Discord channel otherwise looks like the jobs stopped.
            return Delivery::Failed(skipped_line());
        };

        let deadline = match mode {
            ReportMode::ReportOutcome => self.sync_deadline,
            ReportMode::Silent => Some(ASYNC_DEADLINE),
        };
        let outcome = self.post.post(&self.url, &body, &signature, deadline);
        let line = outcome_line(outcome);
        if delivered(outcome) {
            Delivery::Delivered(line)
        } else {
            Delivery::Failed(line)
        }
    }
}

/// The production POST: one agent, no redirects (following one would send the
/// signed body to whatever host the gateway names), the deadline per call.
/// An HTTP error status IS the answer sync mode prints, so a status-carrying
/// error is unwrapped rather than collapsed, matching the bash's missing -f.
pub struct UreqSignedPost;

impl SignedPost for UreqSignedPost {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        deadline: Option<Duration>,
    ) -> PostOutcome {
        let sent = ureq::Agent::config_builder()
            // None is no deadline at all, so the option passes straight
            // through rather than being defaulted back into one.
            .timeout_global(deadline)
            .max_redirects(0)
            .build()
            .new_agent()
            .post(url)
            .content_type("application/json")
            .header("X-Webhook-Signature", signature_hex)
            .send(body);
        match sent {
            Ok(response) => PostOutcome::Status(response.status().as_u16()),
            Err(ureq::Error::StatusCode(code)) => PostOutcome::Status(code),
            // The request was never put on the wire: a URI ureq refuses, or a
            // header the http crate refuses to build. Curl prints no status
            // at all for these, which is a different report from a silent
            // gateway.
            Err(ureq::Error::BadUri(_) | ureq::Error::Http(_)) => PostOutcome::NoStatus,
            Err(_) => PostOutcome::NoResponse,
        }
    }
}

#[cfg(test)]
#[path = "hermes/tests/mod.rs"]
mod tests;

#[cfg(test)]
mod channel_url_tests {
    use super::{DEFAULT_HERMES_URL, channel_url};
    use crate::safety::route_name_is_usable;

    #[test]
    fn one_rule_judges_a_route_name_wherever_it_is_read() {
        // THE PREDICATE IS THE SHARED HALF: the config read that resolves a
        // route by name and the URL swap that spends it must agree about what
        // a name is, or a value the config waved through becomes a URL the
        // swap refuses (or worse, the other way around).
        for usable in ["priority", "unattended-upgrades", "pns", "log_2", "A9"] {
            assert!(route_name_is_usable(usable), "case: {usable:?}");
        }
        // Every form `channel_url` refuses, refused here too AND still refused
        // through it: the extraction is only worth anything if the caller kept
        // asking.
        for hostile in [
            "", "a/b", "../x", "a b", "a?x=1", "a#f", ".", "a\nb", "%2e%2e", "café",
        ] {
            assert!(!route_name_is_usable(hostile), "case: {hostile:?}");
            assert_eq!(
                channel_url(DEFAULT_HERMES_URL, hostile),
                None,
                "case: {hostile:?}"
            );
        }
    }

    #[test]
    fn a_route_name_swaps_the_default_urls_final_segment() {
        assert_eq!(
            channel_url(DEFAULT_HERMES_URL, "unattended-upgrades").as_deref(),
            Some("http://127.0.0.1:8644/webhooks/unattended-upgrades")
        );
    }

    #[test]
    fn a_name_that_could_not_be_a_path_segment_is_refused_not_glued() {
        // The name is about to become part of a URL, so this is a trust
        // boundary like the site id's: nothing traversal-shaped passes.
        for hostile in ["", "a/b", "../x", "a b", "a?x=1", "a#f", "."] {
            assert_eq!(
                channel_url(DEFAULT_HERMES_URL, hostile),
                None,
                "case: {hostile:?}"
            );
        }
    }

    #[test]
    fn a_base_without_a_path_yields_nothing_rather_than_a_bogus_url() {
        assert_eq!(channel_url("no-slashes-here", "log"), None);
    }
}
