//! Asking the gateway whether it serves a route, without sending anything.
//!
//! THE PROBE CARRIES NO SIGNATURE, and that is what makes it safe: the
//! signature is what authorizes a delivery, so a request without one cannot
//! become a page. Measured against the gateway on 2026-09-09: an unsigned POST
//! answers 401 for a route that exists and 404 for one that does not, while GET
//! and HEAD answer 405 for every path, real or not, and so distinguish nothing.

use super::channel_url;
use pns_domain::doctor::RouteVerdict;
use pns_domain::retry::DeliveryOutcome;
use pns_hermes::{PostOutcome, SignedPost};
use std::time::Duration;

/// The deadline one probe spends. Short, because the doctor asks once per route
/// and the operator is waiting on the whole report; a gateway that has not
/// answered in this long is reported unknown, which is the honest verdict.
const PROBE_DEADLINE: Duration = Duration::from_secs(3);

/// The body the probe posts. Empty rather than a rendered event, so that a
/// gateway which somehow accepted it would have nothing to deliver.
const PROBE_BODY: &str = "{}";

/// Ask the gateway about one route.
pub fn probe_route<P: SignedPost>(post: &P, base_url: &str, route: &str) -> RouteVerdict {
    let Some(url) = channel_url(base_url, route) else {
        return RouteVerdict::read(DeliveryOutcome::NoStatus);
    };
    // NO SIGNATURE AND NO IDEMPOTENCY KEY. The empty signature is what makes
    // this a question rather than a delivery, and a key would enrol a probe in
    // the gateway's replay memory under an id no event owns.
    let outcome = post.post(&url, PROBE_BODY, "", None, Some(PROBE_DEADLINE));
    RouteVerdict::read(match outcome {
        PostOutcome::Status(code) => DeliveryOutcome::Status(code),
        PostOutcome::NoResponse => DeliveryOutcome::NoResponse,
        PostOutcome::NoStatus => DeliveryOutcome::NoStatus,
    })
}

/// Ask about every route, in the order given.
pub fn probe_routes<P: SignedPost>(
    post: &P,
    base_url: &str,
    routes: &[String],
) -> Vec<(String, RouteVerdict)> {
    routes
        .iter()
        .map(|route| (route.clone(), probe_route(post, base_url, route)))
        .collect()
}

#[cfg(test)]
#[path = "probe/tests.rs"]
mod tests;
