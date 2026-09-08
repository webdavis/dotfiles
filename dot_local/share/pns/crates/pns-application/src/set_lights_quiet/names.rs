/// Every name `pns lights quiet` will take, for the command as it was typed.
///
/// THE GRAMMAR IS LAMP, ROOM AND ZONE, which are the BRIDGE'S nouns as much as
/// the config's: a lamp that inherits its room's declaration has a real name no
/// declaration writes, and refusing it sends the operator away from the room
/// they are standing in. So the bridge's own listing widens the vocabulary.
///
/// AND THE DIAL IS ON THE MISS PATH ALONE. A place a declaration already holds
/// is a name a mute can enforce whatever the bridge says, so the ordinary
/// command, muting a room the config routes, costs no network at all. Only a
/// word neither this run's declarations nor `off` can account for is worth
/// asking a bridge about, and `off` is allowed over any name because it can
/// only remove.
pub fn quiet_names(
    lights: &pns_domain::lamps::config::Lights,
    arguments: &[String],
    inventory: impl FnOnce() -> Option<pns_domain::lamps::Inventory>,
) -> Vec<String> {
    let declared = pns_domain::lamps::mutable_names(lights, None);
    if !asks_the_bridge(&declared, arguments) {
        return declared;
    }
    pns_domain::lamps::mutable_names(lights, inventory().as_ref())
}

/// Whether the typed command holds a word only a bridge listing could account
/// for.
///
/// THE FIRST ARGUMENT IS THE PLACE in every form that names one (`<place>`,
/// `<place> <duration>`, `<place> off`), and the bare report names none. A
/// second word of `off` needs no listing either: `off` is allowed over any
/// name, because it can only remove a mute the operator can see.
fn asks_the_bridge(declared: &[String], arguments: &[String]) -> bool {
    arguments.first().is_some_and(|place| {
        !declared.contains(place) && arguments.get(1).is_none_or(|word| word != "off")
    })
}

#[cfg(test)]
mod tests;
