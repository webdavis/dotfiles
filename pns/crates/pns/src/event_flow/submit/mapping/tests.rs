use super::*;
use pns_protocol::{Name, RequestId, State};

#[test]
fn normalized_state_scope_and_elapsed_choose_policy_without_using_source_event_name() {
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("source").unwrap(),
        State::Done,
    );
    request.project = Some("project".into());
    request.branch = Some("branch".into());
    request.pane = Some("w:p".into());
    request.route = Some(Name::new("priority").unwrap());
    for (stated, state, attempt) in [
        (State::Done, "done", Attempt::First),
        (State::Failed, "failed", Attempt::First),
        (State::Blocked, "blocked", Attempt::First),
        (State::Resolved, "resolved", Attempt::First),
        (State::Observation, "observation", Attempt::Observation),
        (State::Progress, "progress", Attempt::Observation),
    ] {
        request.state = stated;
        let (event, actual) = event(&request);
        assert_eq!(event.state, state);
        assert_eq!(actual, attempt);
        assert_eq!(
            (
                &*event.project,
                &*event.branch,
                &*event.pane,
                &*event.channel
            ),
            ("project", "branch", "w:p", "priority")
        );
    }
    for (scope, expected) in [
        (
            DeliveryScope::Automatic,
            pns_domain::DeliveryScope::Automatic,
        ),
        (
            DeliveryScope::LocalOnly,
            pns_domain::DeliveryScope::LocalOnly,
        ),
        (
            DeliveryScope::RemoteOnly,
            pns_domain::DeliveryScope::RemoteOnly,
        ),
    ] {
        request.scope = scope;
        let (event, _) = event(&request);
        assert_eq!(event.scope, expected);
    }
    for (elapsed, long) in [
        (None, false),
        (Some(0), false),
        (Some(299), false),
        (Some(300), true),
        (Some(301), true),
    ] {
        request.elapsed = elapsed.map(std::time::Duration::from_secs);
        assert_eq!(event(&request).0.long_running, long);
    }
}

#[test]
fn a_producer_that_states_a_delivery_class_has_it_read_and_one_that_states_none_carries_nothing() {
    // THE PRODUCER NAMES A DELIVERY CLASS, NEVER A ROUTE. This is the
    // submission half of the same rule the `--delivery-class` flag carries on
    // the argv half, so a failed upgrade posted as an envelope pages the way
    // one spawned with flags does: both reach `sirens`.
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("uu").unwrap(),
        State::Failed,
    );
    for stated in [None, Some("agent"), Some("health"), Some("security")] {
        request.delivery_class = stated.map(|word| Name::new(word).unwrap());
        assert_eq!(
            event(&request).0.delivery_class,
            stated.unwrap_or_default(),
            "{stated:?}"
        );
    }
    request.delivery_class = Some(Name::new("health").unwrap());
    assert_eq!(
        event(&request).0.routed(Some("sirens")).channel,
        "sirens",
        "a failed health submission stayed off the urgent route"
    );
    request.state = State::Done;
    assert!(
        event(&request).0.routed(Some("sirens")).channel.is_empty(),
        "a health submission that succeeded paged the operator"
    );
}

#[test]
fn a_route_the_producer_named_still_outranks_the_delivery_class_it_stated() {
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("uu").unwrap(),
        State::Failed,
    );
    request.delivery_class = Some(Name::new("health").unwrap());
    request.route = Some(Name::new("posture-pages").unwrap());
    assert_eq!(
        event(&request).0.routed(Some("sirens")).channel,
        "posture-pages"
    );
}
