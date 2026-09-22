use super::Override;

/// The stored override, out of the row's body.
///
/// ONE ROW RATHER THAN TWO, for `mute::expiry_from_state`'s own reason: a name
/// and an expiry in two rows are two values that can disagree about whether an
/// override is standing.
///
/// THE ONLY LENIENCY IS THE ONE TRAILING NEWLINE the writer itself may leave.
/// Anything else was written by another hand, and the fail-open rule is that
/// an unreadable body reads as NO override, which puts the rules back in
/// charge rather than pinning the machine to a profile nobody can see.
pub fn parse_override(body: &str) -> Option<Override> {
    let held = body.strip_suffix('\n').unwrap_or(body);
    if held.is_empty() || held.contains('\n') {
        return None;
    }
    let mut words = held.split(' ');
    let profile = words.next().filter(|name| !name.is_empty())?.to_string();
    let until = match words.next() {
        None => None,
        Some(epoch) => Some(crate::count::parse_count(epoch)?),
    };
    words
        .next()
        .is_none()
        .then_some(Override { profile, until })
}

/// The row's body for an override, which `parse_override` reads back exactly.
pub fn format_override(standing: &Override) -> String {
    match standing.until {
        None => standing.profile.clone(),
        Some(until) => format!("{} {until}", standing.profile),
    }
}

#[cfg(test)]
mod tests;
