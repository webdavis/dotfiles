//! Whether the gateway serves a route, asked without sending anything.
//!
//! THE PROBE IS AN UNSIGNED POST, and that is not a trick: the signature is
//! what authorizes a delivery, so a request without one cannot become a page.
//! Measured against the gateway on 2026-09-09: an unsigned POST answers 401 for
//! a route that exists and 404 for one that does not, while GET and HEAD answer
//! 405 for every path, real or not, and so distinguish nothing.
//!
//! This is the check the whole failure-reporting design is for. The defect it
//! catches is a `--channel` name that is a valid path segment and names no
//! route: pns builds it into a URL, posts it, and the page is gone. Asking here
//! finds that when the route is introduced rather than when a page is lost.

use crate::retry::DeliveryOutcome;

/// What the gateway said about one route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteVerdict {
    /// The gateway knows this route. It refused the probe for want of a
    /// signature, which is the correct answer to a request that carries none.
    Served,
    /// The gateway has no route by this name. A page sent here is lost.
    Missing,
    /// The gateway could not be asked, or answered something this rule does
    /// not read. NOT a failure: a gateway that is down is a different problem
    /// from a route that is missing, and reporting it as missing routes would
    /// be a false alarm on every restart.
    Unknown(String),
}

impl RouteVerdict {
    /// The gateway's answer to an unsigned probe.
    pub fn read(outcome: DeliveryOutcome) -> Self {
        match outcome {
            // The route exists and the signature is what was wrong, which is
            // exactly what a probe carrying none should be told.
            DeliveryOutcome::Status(401 | 403) => RouteVerdict::Served,
            DeliveryOutcome::Status(404 | 410) => RouteVerdict::Missing,
            DeliveryOutcome::NoResponse => {
                RouteVerdict::Unknown("the gateway did not answer".into())
            }
            DeliveryOutcome::NoStatus => RouteVerdict::Unknown("the URL could not be built".into()),
            // A 2xx to an UNSIGNED post means the gateway is not checking
            // signatures, which is worth saying out loud rather than reporting
            // as a healthy route.
            DeliveryOutcome::Status(code) if (200..300).contains(&code) => {
                RouteVerdict::Unknown(format!(
                    "the gateway accepted an UNSIGNED post (HTTP {code}); it is not checking signatures"
                ))
            }
            DeliveryOutcome::Status(code) => {
                RouteVerdict::Unknown(format!("the gateway answered HTTP {code}"))
            }
        }
    }

    /// Whether this verdict is one the operator has to act on. Only a route the
    /// gateway denies knowing is; an unanswered gateway is not.
    pub fn is_missing(&self) -> bool {
        matches!(self, RouteVerdict::Missing)
    }
}

/// The doctor's line for one route.
pub fn route_line(route: &str, verdict: &RouteVerdict) -> String {
    match verdict {
        RouteVerdict::Served => format!("pns doctor: route {route}: served by the gateway"),
        RouteVerdict::Missing => format!(
            "pns doctor: route {route}: THE GATEWAY HAS NO SUCH ROUTE; \
             a page sent here is lost. Add it to ~/.hermes/config.yaml"
        ),
        RouteVerdict::Unknown(reason) => {
            format!("pns doctor: route {route}: unknown, {reason}")
        }
    }
}

/// The doctor's one-line summary over every route it asked about.
///
/// It says how many are UNKNOWN separately from how many are missing, because
/// the two call for different things: a missing route is an edit, and an
/// unknown one is a gateway to start before asking again.
pub fn routes_summary(verdicts: &[(String, RouteVerdict)]) -> String {
    if verdicts.is_empty() {
        return "pns doctor: no routes to check; nothing has been posted yet".to_string();
    }
    let missing = verdicts
        .iter()
        .filter(|(_, verdict)| verdict.is_missing())
        .count();
    let unknown = verdicts
        .iter()
        .filter(|(_, verdict)| matches!(verdict, RouteVerdict::Unknown(_)))
        .count();
    format!(
        "pns doctor: {} route(s) checked, {missing} missing, {unknown} unknown",
        verdicts.len()
    )
}

#[cfg(test)]
#[path = "routes/tests.rs"]
mod tests;

/// How a route's verdict reads at a glance.
///
/// A MISSING ROUTE IS A WARNING RATHER THAN A FAULT, for the reason it does not
/// move the exit code: the roster is derived from what pns has posted to, so a
/// route retired on the gateway is missing forever with nothing the operator
/// can do to clear it. A check that cannot be satisfied is one they learn to
/// ignore.
pub fn route_mark(verdict: &RouteVerdict) -> super::Mark {
    match verdict {
        RouteVerdict::Served => super::Mark::Good,
        RouteVerdict::Missing => super::Mark::Warn,
        RouteVerdict::Unknown(_) => super::Mark::Note,
    }
}
