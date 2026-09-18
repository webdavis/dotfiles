use super::*;

pub(super) fn event(request: &Request) -> (pns_domain::EventArgs, Attempt) {
    let state = request.state.as_str();
    let attempt = Attempt::of_state(state);
    (
        pns_domain::EventArgs {
            agent: request.producer.as_str().into(),
            state: state.into(),
            // A PRODUCER STATES ITS STATE, so this is never a guess: an
            // approval it asked for is a real wait even mid-loop.
            guessed: false,
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
            // WHAT THE EVENT IS, as its producer stated it, and a producer
            // that stated nothing gets the ordinary session default. The
            // route it lands on is decided from this and never named here:
            // a producer knows its own work failed and nothing about which
            // channels a gateway has (operator ruling, 2026-09-15).
            kind: match request.kind {
                None | Some(pns_protocol::Kind::Agent) => pns_domain::routes::Kind::Agent,
                Some(pns_protocol::Kind::Health) => pns_domain::routes::Kind::Health,
            },
        },
        attempt,
    )
}

#[cfg(test)]
mod tests;
