use super::*;
use pns_application::{LedgerLeg, Submitted};
use pns_domain::routing::ReportMode;

fn leg(destination: &str, decorative: bool) -> LedgerLeg {
    LedgerLeg {
        destination: destination.into(),
        route: "testpath".into(),
        mode: ReportMode::Silent,
        decorative,
    }
}

fn attempted(outcomes: Vec<(LedgerLeg, pns_domain::Delivery)>) -> Result<Submitted, LedgerFailure> {
    Ok(Submitted::Attempted {
        sequence: Some(1),
        outcomes,
    })
}

use pns_application::LedgerFailure;

/// The ordinary case, and the one every producer sees when its gateway is up.
#[test]
fn a_page_that_reached_the_durable_log_landed() {
    let submitted = attempted(vec![(
        leg("hermes", false),
        pns_domain::Delivery::Delivered("posted".into()),
    )]);
    assert_eq!(landed(&submitted), Landed::Yes);
}

/// The whole point: a producer such as posture has no other way to learn that
/// the page it just sent is nowhere.
#[test]
fn a_durable_leg_that_was_refused_did_not_land() {
    let submitted = attempted(vec![(
        leg("hermes", false),
        pns_domain::Delivery::Rejected {
            status: 404,
            detail: "no such route".into(),
        },
    )]);
    assert_eq!(landed(&submitted), Landed::No);
}

/// A banner that could not spawn its notifier is a notification the operator
/// missed, not a page that is nowhere. Treating it as one would fail every
/// event on a machine without `terminal-notifier`.
#[test]
fn a_decorative_leg_that_failed_does_not_decide_it() {
    let submitted = attempted(vec![
        (
            leg("macos-banner", true),
            pns_domain::Delivery::Failed("no notifier".into()),
        ),
        (
            leg("hermes", false),
            pns_domain::Delivery::Delivered("posted".into()),
        ),
    ]);
    assert_eq!(landed(&submitted), Landed::Yes);
}

/// A plan with no durable leg at all, which is what `--local-only` produces,
/// landed vacuously: there was no page to lose.
#[test]
fn a_plan_with_no_durable_leg_landed() {
    let submitted = attempted(vec![(
        leg("macos-banner", true),
        pns_domain::Delivery::Delivered("posted".into()),
    )]);
    assert_eq!(landed(&submitted), Landed::Yes);
}

/// A duplicate of a submission already answered reports what that one did.
/// Answering it a second time with a failure would make a retried producer call
/// report a page that did arrive.
#[test]
fn a_ledger_that_refused_the_submission_did_not_land() {
    let refused: Result<Submitted, LedgerFailure> = Err(LedgerFailure::InvalidPlan);
    assert_eq!(landed(&refused), Landed::No);
}
