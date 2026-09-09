use super::*;

/// The measured answers. An unsigned post is refused for want of a signature by
/// a route that exists, and refused as unknown by one that does not, which is
/// what makes the probe able to tell them apart without sending a page.
#[test]
fn a_refusal_for_want_of_a_signature_proves_the_route_exists() {
    assert_eq!(
        RouteVerdict::read(DeliveryOutcome::Status(401)),
        RouteVerdict::Served
    );
    assert_eq!(
        RouteVerdict::read(DeliveryOutcome::Status(403)),
        RouteVerdict::Served
    );
    assert_eq!(
        RouteVerdict::read(DeliveryOutcome::Status(404)),
        RouteVerdict::Missing
    );
    assert_eq!(
        RouteVerdict::read(DeliveryOutcome::Status(410)),
        RouteVerdict::Missing
    );
}

/// A gateway that is down is a DIFFERENT problem from a route that is missing,
/// and calling it missing would raise a false alarm on every restart. This is
/// the case the design names explicitly.
#[test]
fn an_unreachable_gateway_names_no_route_as_missing() {
    for outcome in [
        DeliveryOutcome::NoResponse,
        DeliveryOutcome::NoStatus,
        DeliveryOutcome::Status(500),
        DeliveryOutcome::Status(502),
        DeliveryOutcome::Status(503),
    ] {
        let verdict = RouteVerdict::read(outcome);
        assert!(
            matches!(verdict, RouteVerdict::Unknown(_)),
            "{outcome:?} must be unknown rather than missing"
        );
        assert!(!verdict.is_missing(), "{outcome:?}");
    }
}

/// A gateway that takes an UNSIGNED post is not a healthy route: it is a
/// gateway that has stopped checking signatures, and reporting it as served
/// would hide that behind a tick.
#[test]
fn a_gateway_that_accepts_an_unsigned_post_is_reported_rather_than_ticked() {
    let verdict = RouteVerdict::read(DeliveryOutcome::Status(200));
    assert!(!verdict.is_missing());
    assert_ne!(verdict, RouteVerdict::Served);
    let RouteVerdict::Unknown(reason) = verdict else {
        panic!("expected an unknown verdict");
    };
    assert!(reason.contains("not checking signatures"), "{reason}");
}

/// The missing line names the route and the file to edit, because the reader is
/// at a terminal and the next thing they do is open one.
#[test]
fn the_missing_line_names_the_route_and_the_file_to_edit() {
    let line = route_line("testpath", &RouteVerdict::Missing);
    assert!(line.contains("testpath"), "{line}");
    assert!(line.contains("~/.hermes/config.yaml"), "{line}");
}

/// An unknown line says WHY it is unknown, so the reader can tell a gateway
/// that is down from a URL that could not be built.
#[test]
fn an_unknown_line_carries_the_reason_it_could_not_be_answered() {
    assert!(
        route_line("testpath", &RouteVerdict::read(DeliveryOutcome::NoResponse))
            .contains("did not answer")
    );
    assert!(
        route_line("testpath", &RouteVerdict::read(DeliveryOutcome::NoStatus))
            .contains("URL could not be built")
    );
}

/// The summary counts missing and unknown SEPARATELY, because the two call for
/// different things: an edit, and a gateway to start before asking again.
#[test]
fn the_summary_counts_missing_and_unknown_apart() {
    let verdicts = vec![
        ("a".to_string(), RouteVerdict::Served),
        ("b".to_string(), RouteVerdict::Missing),
        ("c".to_string(), RouteVerdict::Unknown("down".into())),
        ("d".to_string(), RouteVerdict::Unknown("down".into())),
    ];
    assert_eq!(
        routes_summary(&verdicts),
        "pns doctor: 4 route(s) checked, 1 missing, 2 unknown"
    );
}

/// Nothing posted yet is not a clean bill of health, and the line says which of
/// the two it is.
#[test]
fn no_routes_to_check_says_so_rather_than_reporting_zero_missing() {
    let summary = routes_summary(&[]);
    assert!(summary.contains("nothing has been posted yet"), "{summary}");
    assert!(!summary.contains("0 missing"), "{summary}");
}
