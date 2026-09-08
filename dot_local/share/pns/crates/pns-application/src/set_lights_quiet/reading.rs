use crate::LampMutes;

/// What an ad-hoc quiet is muting right now, and that same complaint.
///
/// A READING THIS CANNOT TAKE MUTES EVERYTHING, which is the fail direction
/// every lamp-path input takes and the OPPOSITE of what both halves used to do.
/// A record nobody can parse and a clock nobody can read each answered with an
/// empty list, which is a house with every lamp loud: exactly the 3am the mute
/// was armed to prevent, on the one night the machine could not tell anybody
/// why.
///
/// THE COMPLAINT IS STILL THE OTHER HALF. Going dark silently would be a lamp
/// that stopped working for a reason nobody can see, so the caller says it
/// once through `say_lights_once` and the state is repaired by the next
/// `pns lights quiet` write, which republishes the whole file.
pub fn ad_hoc_quiet(
    mutes: &impl LampMutes,
    now: Option<u64>,
) -> (pns_domain::lamps::Muting, Vec<String>) {
    let (entries, complaints) = mutes.read();
    if !complaints.is_empty() {
        return (pns_domain::lamps::Muting::Everything, complaints);
    }
    let Some(now) = now else {
        return (
            pns_domain::lamps::Muting::Everything,
            vec![pns_domain::lights::mute::NO_CLOCK_FOR_THE_MUTE.to_string()],
        );
    };
    (
        pns_domain::lamps::Muting::Places(pns_domain::lights::mute::muted_places(
            &entries,
            Some(now),
        )),
        complaints,
    )
}
