use super::*;
use pns_protocol::{Context, Name, RequestId, State};

#[test]
fn normalized_state_scope_and_elapsed_choose_policy_without_using_source_event_name() {
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("source").unwrap(),
        Name::new("failed").unwrap(),
        State::Done,
    );
    request.context = Context {
        project: Some("project".into()),
        branch: Some("branch".into()),
        pane: Some("w:p".into()),
    };
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
        request.elapsed_secs = elapsed;
        assert_eq!(event(&request).0.long_running, long);
    }
}

#[test]
fn a_producer_that_states_a_kind_has_it_read_and_one_that_states_none_is_a_session_event() {
    // THE PRODUCER NAMES A KIND, NEVER A ROUTE. This is the submission half
    // of the same rule the `--kind` flag carries on the argv half, so a
    // failed upgrade posted as an envelope pages the way one spawned with
    // flags does.
    let routes = pns_domain::routes::Routes::named("logbook", "sirens");
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("uu").unwrap(),
        Name::new("lane-failed").unwrap(),
        State::Failed,
    );
    for (stated, kind) in [
        (None, pns_domain::routes::Kind::Agent),
        (
            Some(pns_protocol::Kind::Agent),
            pns_domain::routes::Kind::Agent,
        ),
        (
            Some(pns_protocol::Kind::Health),
            pns_domain::routes::Kind::Health,
        ),
    ] {
        request.kind = stated;
        assert_eq!(event(&request).0.kind, kind, "{stated:?}");
    }
    request.kind = Some(pns_protocol::Kind::Health);
    assert_eq!(
        event(&request).0.routed(&routes).channel,
        "sirens",
        "a failed health submission stayed off the urgent route"
    );
    request.state = State::Done;
    assert!(
        event(&request).0.routed(&routes).channel.is_empty(),
        "a health submission that succeeded paged the operator"
    );
}

#[test]
fn a_route_the_producer_named_still_outranks_the_kind_it_stated() {
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("uu").unwrap(),
        Name::new("lane-failed").unwrap(),
        State::Failed,
    );
    request.kind = Some(pns_protocol::Kind::Health);
    request.route = Some(Name::new("posture-pages").unwrap());
    assert_eq!(
        event(&request)
            .0
            .routed(&pns_domain::routes::Routes::named("logbook", "sirens"))
            .channel,
        "posture-pages"
    );
}
