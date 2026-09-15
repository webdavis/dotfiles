//! The two rules both delivery paths share, written once so the path a page
//! takes cannot depend on which way it left.

use posture_application::{Alert, IndependentAlarm, Submission, SubmissionFailure};
use posture_domain::severity_route;
use posture_producer_wire::Name;

/// The route this page's own tier names, when it has one.
///
/// A compiled-in route name cannot fail the identifier rules, so a `None` here
/// only ever means an untiered page: the heartbeat, the digest and the
/// cursor-reset warning, which keep whatever route their caller configured.
pub(crate) fn tier_route(alert: &Alert) -> Option<Name> {
    severity_route(alert.severity).and_then(|route| Name::new(route).ok())
}

/// Report a delivery that broke rather than refused, on the one channel that
/// does not depend on delivery working, and answer the caller that its state
/// must not advance.
pub(crate) fn delivery_failed(
    alarm: &mut impl IndependentAlarm,
    title: &str,
    alert: &Alert,
    failure: SubmissionFailure,
) -> Submission {
    let _ = alarm.alarm(title, &format!("{}\n{}", alert.title, alert.detail));
    Submission::NotAccepted(failure)
}
