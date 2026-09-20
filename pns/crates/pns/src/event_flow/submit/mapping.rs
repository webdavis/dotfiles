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
            project: request.project.clone().unwrap_or_default(),
            branch: request.branch.clone().unwrap_or_default(),
            pane: request.pane.clone().unwrap_or_default(),
            // THE ENVELOPE'S SESSION STAYS WHERE IT IS. A producer names one
            // for correlation, not for attribution: the header's second line
            // is about which of the operator's own agent sessions sent an
            // event, and `posture`, `uu` and `pns` each have exactly one.
            session: String::new(),
            session_title: String::new(),
            // THE PRODUCER'S ID TRAVELS BESIDE THE EVENT on this path, in the
            // submission identity the ledger is keyed on, so there is nothing
            // to carry here.
            request_id: String::new(),
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
            long_running: request.elapsed.is_some_and(|elapsed| {
                elapsed.as_secs() >= pns_domain::pulse::DEFAULT_LONG_SESSION_SECS
            }),
            // WHAT THE EVENT IS FOR DELIVERY, as its producer stated it, and
            // a producer that stated nothing carries no class at all. The
            // route it lands on is decided from this and never named here:
            // a producer knows its own work failed and nothing about which
            // channels a gateway has (operator ruling, 2026-09-15).
            delivery_class: request
                .delivery_class
                .as_ref()
                .map_or_else(String::new, |class| class.as_str().into()),
        },
        attempt,
    )
}

/// This request's reminder, through the SAME resolution the flag path runs:
/// `"remind": true` is `--remind`, `"remind": "5m"` is `--remind=<duration>`,
/// `"remind": false` is `--no-remind`, and an absent field falls through to
/// `[producer.<name>] remind` and then to off.
///
/// ONLY A BLOCKED REQUEST ARMS ONE. A reminder nudges an approval nobody
/// answered, and the producer's config entry states that producer's approvals
/// rather than every event it sends, so every other state resolves to off
/// whatever the table says.
pub(super) fn reminder(request: &Request) -> Result<Reminder, String> {
    match request.state {
        State::Blocked => remind_delay(request.remind, request.producer.as_str()),
        _ => Ok(Reminder {
            after_secs: REMIND_OFF,
            answered_signal: false,
        }),
    }
}

#[cfg(test)]
mod tests;
