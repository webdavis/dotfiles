use super::*;

pub(super) fn event(request: &Request) -> (pns_domain::EventArgs, Attempt) {
    let state = match request.signal {
        Signal::Succeeded => "done",
        Signal::Failed => "failed",
        Signal::NeedsAttention | Signal::ApprovalRequested => "blocked",
        Signal::Resolved => "resolved",
        Signal::Observation => "observation",
        Signal::Progress => "progress",
    };
    let attempt = match request.signal {
        Signal::Observation | Signal::Progress => Attempt::Observation,
        _ => Attempt::First,
    };
    (
        pns_domain::EventArgs {
            agent: request.producer.as_str().into(),
            state: state.into(),
            project: request.context.project.clone().unwrap_or_default(),
            branch: request.context.branch.clone().unwrap_or_default(),
            pane: request.context.pane.clone().unwrap_or_default(),
            // THE ENVELOPE'S SESSION STAYS WHERE IT IS. A producer names one
            // for correlation, not for attribution: the header's second line
            // is about which of the operator's own agent sessions sent an
            // event, and `posture`, `uu` and `pns` each have exactly one.
            session: String::new(),
            session_title: String::new(),
            detail: request.detail.clone(),
            channel: request
                .route
                .as_ref()
                .map_or_else(String::new, |route| route.as_str().into()),
            scope: match request.scope {
                DeliveryScope::Automatic => pns_domain::DeliveryScope::Automatic,
                DeliveryScope::LocalOnly => pns_domain::DeliveryScope::LocalOnly,
                DeliveryScope::RemoteOnly => pns_domain::DeliveryScope::RemoteOnly,
            },
            long_running: request
                .elapsed_secs
                .is_some_and(|seconds| seconds >= pns_domain::pulse::DEFAULT_LONG_SESSION_SECS),
            // THE PRODUCER API NAMES A ROUTE, NEVER A KIND: version one of the
            // request carries `route`, so a submission that wants the urgent
            // one spells it, and the kind is the ordinary session default.
            kind: pns_domain::routes::Kind::default(),
        },
        attempt,
    )
}

#[cfg(test)]
mod tests;
