/// What one resolution has to say for itself: every declared name the bridge
/// could not answer, and every declaration it refused.
///
/// ONE WORDING FOR BOTH READERS, the tick's and the event path's, because a
/// typo reported in two spellings is two entries in two say-once memories and
/// an operator reading the same problem twice.
///
/// `pns ` AND NOT `pns lights: `, because every sentence already begins
/// `lights: ` (the doctor prefixes the same sentences `pns doctor: `).
pub(crate) fn routing_complaints(routing: &pns_domain::lamps::Routing) -> Vec<String> {
    routing
        .unresolved
        .iter()
        .map(|missing| format!("pns {}", pns_domain::lamps::missing_sentence(missing)))
        .chain(
            routing
                .refusals
                .iter()
                .map(|refusal| format!("pns {refusal}")),
        )
        .collect()
}
