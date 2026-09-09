use super::*;
use std::cell::RefCell;

/// Records every request and answers a fixed outcome, so a probe can be
/// inspected for what it actually put on the wire.
struct Recorder {
    answer: PostOutcome,
    seen: RefCell<Vec<(String, String, String, bool)>>,
}

impl SignedPost for Recorder {
    fn post(
        &self,
        url: &str,
        body: &str,
        signature_hex: &str,
        idempotency_key: Option<&str>,
        _deadline: Option<Duration>,
    ) -> PostOutcome {
        self.seen.borrow_mut().push((
            url.to_string(),
            body.to_string(),
            signature_hex.to_string(),
            idempotency_key.is_some(),
        ));
        self.answer
    }
}

fn recorder(answer: PostOutcome) -> Recorder {
    Recorder {
        answer,
        seen: RefCell::new(Vec::new()),
    }
}

const BASE: &str = "http://127.0.0.1:8644/webhooks/pns";

/// THE WHOLE SAFETY ARGUMENT. The signature is what authorizes a delivery, so a
/// probe that carried one would be a page. This is the case that fails if the
/// probe ever starts signing.
#[test]
fn a_probe_carries_no_signature_no_key_and_no_event() {
    let post = recorder(PostOutcome::Status(401));
    probe_route(&post, BASE, "testpath");
    let seen = post.seen.borrow();
    assert_eq!(seen.len(), 1);
    let (url, body, signature, keyed) = &seen[0];
    assert_eq!(url, "http://127.0.0.1:8644/webhooks/testpath");
    assert_eq!(signature, "", "a signed probe is a delivery");
    assert_eq!(body, "{}", "the probe carries no event to deliver");
    assert!(
        !keyed,
        "a probe must not enrol in the gateway's replay memory"
    );
}

/// The three answers the gateway actually gives, each read as the design says.
#[test]
fn the_gateways_measured_answers_read_as_served_missing_and_unknown() {
    assert_eq!(
        probe_route(&recorder(PostOutcome::Status(401)), BASE, "pns"),
        RouteVerdict::Served
    );
    assert_eq!(
        probe_route(&recorder(PostOutcome::Status(404)), BASE, "nope"),
        RouteVerdict::Missing
    );
    assert!(matches!(
        probe_route(&recorder(PostOutcome::NoResponse), BASE, "pns"),
        RouteVerdict::Unknown(_)
    ));
}

/// A route name that cannot become a path segment never reaches the wire, and
/// says so rather than being posted somewhere else.
#[test]
fn a_route_that_cannot_become_a_path_segment_is_never_posted() {
    let post = recorder(PostOutcome::Status(401));
    let verdict = probe_route(&post, BASE, "../escape");
    assert!(matches!(verdict, RouteVerdict::Unknown(_)));
    assert!(post.seen.borrow().is_empty(), "nothing may be sent");
}

/// Every route is asked about, in the order given, so the report reads in the
/// order the listing does.
#[test]
fn every_route_is_asked_about_in_order() {
    let post = recorder(PostOutcome::Status(404));
    let routes = vec!["one".to_string(), "two".to_string(), "three".to_string()];
    let verdicts = probe_routes(&post, BASE, &routes);
    assert_eq!(
        verdicts
            .iter()
            .map(|(route, _)| route.clone())
            .collect::<Vec<_>>(),
        routes
    );
    assert!(verdicts.iter().all(|(_, verdict)| verdict.is_missing()));
    assert_eq!(post.seen.borrow().len(), 3);
}
