//! The direct path: posture signs one page and posts it to a hermes webhook
//! route itself, with one key per route.
//!
//! ONE KEY PER ROUTE, AND A KEYLESS ROUTE IS REFUSED. The gateway reads a
//! route's secret as a literal, so a page signed with another route's key
//! answers 401 and vanishes. A route this machine holds no key for therefore
//! refuses the page and raises the local banner, exactly as a broken engine
//! does: a security finding that went nowhere is never reported as delivered.
//!
//! THE BODY SERVES BOTH PROMPT SHAPES the gateway's routes are written in.
//! Hermes renders `{placeholder}` out of the posted JSON and emits an unknown
//! one as its own literal text, so a body missing a key the route names
//! delivers that key's braces to Discord. The flat four (`agent`, `state`,
//! `project`, `detail`) serve a route templated the way the agent channel is,
//! and the nested `alert` object serves one templated `{alert.title}` and
//! `{alert.detail}`. Both cost one key each and the alternative is a page
//! nobody can read.

mod body;

use crate::signed_post::{PostOutcome, SignedPost, delivered, sign};
use crate::sink::{delivery_failed, tier_route};
use posture_application::{Alert, AlertSink, IndependentAlarm, Submission, SubmissionFailure};
use posture_producer_wire::Name;
use std::collections::BTreeMap;
use std::time::Duration;

/// What one page's post may take. It matches the budget every caller gives the
/// producer path, because the caller waits the same way for either.
const POST_DEADLINE: Duration = Duration::from_secs(5);

/// The title the local banner carries when a page could not be posted at all.
const ALARM_TITLE: &str = "Posture page could not be delivered";

pub struct HermesWebhook<P, A> {
    post: P,
    base_url: String,
    keys: BTreeMap<String, String>,
    route: Name,
    alarm: A,
}

impl<P: SignedPost, A: IndependentAlarm> HermesWebhook<P, A> {
    pub fn new(
        post: P,
        base_url: String,
        keys: BTreeMap<String, String>,
        route: Name,
        alarm: A,
    ) -> Self {
        Self {
            post,
            base_url,
            keys,
            route,
            alarm,
        }
    }

    /// `<base>/<route>`, which is how the gateway addresses one route.
    fn url(&self, route: &Name) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), route.as_str())
    }
}

impl<P: SignedPost, A: IndependentAlarm> AlertSink for HermesWebhook<P, A> {
    fn submit(&mut self, alert: &Alert) -> Submission {
        let route = tier_route(alert).unwrap_or_else(|| self.route.clone());
        let body = body::encode(alert, &route);
        // A ROUTE WITH NO KEY REFUSES THE PAGE AND SAYS SO. An unsigned post
        // is rejected by the gateway, and an empty key is the not-set-up case
        // rather than a signature, so both land here.
        let Some(signature) = self
            .keys
            .get(route.as_str())
            .and_then(|key| sign(key, &body))
        else {
            return delivery_failed(
                &mut self.alarm,
                ALARM_TITLE,
                alert,
                SubmissionFailure::Refused,
            );
        };
        let outcome = self
            .post
            .post(&self.url(&route), &body, &signature, Some(POST_DEADLINE));
        if delivered(outcome) {
            return Submission::Accepted;
        }
        delivery_failed(
            &mut self.alarm,
            ALARM_TITLE,
            alert,
            match outcome {
                // The gateway answered, so the request was made and refused:
                // a wrong key, a route the gateway does not serve, or a
                // body it would not take. All three are a config to fix.
                PostOutcome::Status(_) | PostOutcome::NoStatus => SubmissionFailure::Failed,
                PostOutcome::NoResponse => SubmissionFailure::Unavailable,
            },
        )
    }
}

#[cfg(test)]
mod tests;
