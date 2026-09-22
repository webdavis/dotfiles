use super::{DEFAULT_PROFILE, Override, Rule};

/// Everything the choice rests on, read ONCE by the composition root and
/// passed in. `None` is a reading nobody could take, which is never the same
/// as a reading that did not match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    /// The local weekday, 0 for Sunday, as `libc` numbers it.
    pub weekday: Option<u32>,
    /// Minutes since local midnight.
    pub minutes_of_day: Option<u16>,
    /// The NAME `[profiles.locations]` gives this network, never a fingerprint.
    pub location: Option<String>,
    /// The asserted macOS Focus mode's display name.
    pub focus: Option<String>,
    pub calendar_busy: Option<bool>,
}

/// What chose the active profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chose {
    /// The operator typed it. `until` is the epoch it ends at, if it has one.
    Manual { until: Option<u64> },
    /// A rule, counted from ONE, the way the load refusals count.
    Rule { index: usize },
    /// No rule matched, which is `DEFAULT_PROFILE`.
    Fallback,
}

/// The answer, with enough beside it to say why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub profile: String,
    pub chose: Chose,
    /// The names of the inputs the winning rule matched on, in rule order.
    pub matched: Vec<&'static str>,
}

/// The active profile. A PURE FUNCTION: no clock, no file, no probe and no
/// environment inside it, which is what puts the whole rule matrix under unit
/// test with no daemon.
pub fn resolve(
    rules: &[Rule],
    inputs: &Inputs,
    standing: Option<&Override>,
    now_secs: Option<u64>,
) -> Resolved {
    if let Some(standing) = standing.filter(|standing| still_standing(standing, now_secs)) {
        return Resolved {
            profile: standing.profile.clone(),
            chose: Chose::Manual {
                until: standing.until,
            },
            matched: Vec::new(),
        };
    }
    for (offset, rule) in rules.iter().enumerate() {
        if let Some(matched) = matches(rule, inputs) {
            return Resolved {
                profile: rule.profile.clone(),
                chose: Chose::Rule { index: offset + 1 },
                matched,
            };
        }
    }
    Resolved {
        profile: DEFAULT_PROFILE.to_string(),
        chose: Chose::Fallback,
        matched: Vec::new(),
    }
}

/// HALF OPEN, `mute::is_muted`'s own rule: an override ends when it says it
/// does. A clock nobody could read leaves it standing, because not knowing
/// cannot be the thing that ends something the operator typed by hand.
fn still_standing(standing: &Override, now_secs: Option<u64>) -> bool {
    match (standing.until, now_secs) {
        (Some(until), Some(now)) => now < until,
        _ => true,
    }
}

/// The names of the inputs this rule matched on, or None for a rule that did
/// not match. A rule naming NO input matches on an empty list, which is why
/// the two cases cannot be one `Vec`.
fn matches(rule: &Rule, inputs: &Inputs) -> Option<Vec<&'static str>> {
    let mut matched = Vec::new();
    if !rule.days.is_empty() {
        let weekday = inputs.weekday?;
        if !rule.days.contains(&weekday) {
            return None;
        }
        matched.push("days");
    }
    if let Some(window) = rule.hours.as_ref() {
        let minutes = inputs.minutes_of_day?;
        // THE LAMPS' OWN WINDOW, which already joins the two ends of a
        // midnight-crossing window and already has its tests.
        if !crate::lamps::quiet_now(Some(window), Some(minutes)) {
            return None;
        }
        matched.push("hours");
    }
    if let Some(named) = rule.location.as_deref() {
        if inputs.location.as_deref()? != named {
            return None;
        }
        matched.push("location");
    }
    if let Some(named) = rule.focus.as_deref() {
        if inputs.focus.as_deref()? != named {
            return None;
        }
        matched.push("focus");
    }
    if let Some(wanted) = rule.calendar_busy {
        if inputs.calendar_busy? != wanted {
            return None;
        }
        matched.push("calendar_busy");
    }
    Some(matched)
}

#[cfg(test)]
mod tests;
