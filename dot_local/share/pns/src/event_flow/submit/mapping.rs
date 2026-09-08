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
            detail: request.detail.clone(),
            channel: request
                .route
                .as_ref()
                .map_or_else(String::new, |route| route.as_str().into()),
            local_only: request.scope == DeliveryScope::LocalOnly,
            remote_only: request.scope == DeliveryScope::RemoteOnly,
            long_running: request
                .elapsed_secs
                .is_some_and(|seconds| seconds >= pns_domain::pulse::DEFAULT_LONG_SESSION_SECS),
        },
        attempt,
    )
}

#[cfg(test)]
mod tests;
