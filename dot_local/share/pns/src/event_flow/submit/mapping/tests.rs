use super::*;
use pns_protocol::{Context, Name, RequestId};

#[test]
fn normalized_signal_scope_and_elapsed_choose_policy_without_using_source_event_name() {
    let mut request = Request::new(
        RequestId::new("id").unwrap(),
        Name::new("source").unwrap(),
        Name::new("failed").unwrap(),
        Signal::Succeeded,
    );
    request.context = Context {
        project: Some("project".into()),
        branch: Some("branch".into()),
        pane: Some("w:p".into()),
    };
    request.route = Some(Name::new("priority").unwrap());
    for (signal, state, attempt) in [
        (Signal::Succeeded, "done", Attempt::First),
        (Signal::Failed, "failed", Attempt::First),
        (Signal::NeedsAttention, "blocked", Attempt::First),
        (Signal::ApprovalRequested, "blocked", Attempt::First),
        (Signal::Resolved, "resolved", Attempt::First),
        (Signal::Observation, "observation", Attempt::Observation),
        (Signal::Progress, "progress", Attempt::Observation),
    ] {
        request.signal = signal;
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
    for (scope, local, remote) in [
        (DeliveryScope::Automatic, false, false),
        (DeliveryScope::LocalOnly, true, false),
        (DeliveryScope::RemoteOnly, false, true),
    ] {
        request.scope = scope;
        let (event, _) = event(&request);
        assert_eq!((event.local_only, event.remote_only), (local, remote));
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
