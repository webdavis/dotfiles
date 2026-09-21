use super::{Admits, Chose, Profile};

/// Why this profile is the active one, in one clause.
///
/// `until` IS HANDED IN ALREADY RENDERED, because a local time is a system
/// fact this crate has no business reading. A bounded override with no
/// rendered time says so rather than printing an epoch second at an operator.
pub fn because(chose: &Chose, matched: &[&str], until: Option<&str>) -> String {
    match chose {
        Chose::Fallback => "no rule matched".to_string(),
        Chose::Rule { index } if matched.is_empty() => format!("rule {index}: every time"),
        Chose::Rule { index } => format!("rule {index}: {}", matched.join(", ")),
        Chose::Manual { until: None } => "manual, until cleared".to_string(),
        Chose::Manual { .. } => match until {
            Some(clock) => format!("manual, until {clock}"),
            None => "manual, until an hour this machine cannot read".to_string(),
        },
    }
}

/// The hush and the four surfaces, in the order the config writes them.
pub fn surfaces_line(profile: &Profile) -> String {
    let hush = if profile.quiet { "on" } else { "off" };
    format!(
        "quiet {hush}; banner {}, Discord {}, phone {}, lights {}",
        profile.banner.word(),
        profile.discord.word(),
        profile.phone.word(),
        profile.lights.word()
    )
}

/// One transition, as the ledger row and the gateway's own line both carry it.
///
/// ONE SPELLING FOR BOTH, so the row an operator reads back and the line they
/// saw scroll past cannot describe the same moment differently.
pub fn transition_line(from: &str, to: &str, reason: &str) -> String {
    format!("profile {from} -> {to} ({reason})")
}

#[cfg(test)]
mod tests;
