/// The routing narrowed to the room the operator is in, with the decision
/// appended to its ring.
///
/// THE RECORD IS FAIL-QUIET, in `record_decision`'s exact style and for its
/// exact reason: both callers run where a printed line about the state
/// directory would be a line in every hook's output or in a tick that runs
/// three times a minute forever.
pub(crate) fn narrow_to_presence(
    records: &impl crate::PresenceDecisions,
    routing: pns_domain::lamps::Routing,
    presence: Option<&pns_domain::Snapshot>,
) -> pns_domain::lamps::Routing {
    let Some(snapshot) = presence else {
        return routing;
    };
    let (narrowed, decision) = pns_domain::narrow(routing, snapshot);
    records.record(snapshot, &decision);
    narrowed
}
