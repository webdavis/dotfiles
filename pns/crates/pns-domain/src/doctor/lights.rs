use super::pairing::PREFIX;

/// What the doctor found about the lamps.
///
/// SIX STATES AND NO GRADE. This section reports; it never moves the exit
/// code, for the reason the decision section does not: a dark lamp is not a
/// broken notifier, and the exit code is what the operator's automation reads
/// as "notifications are broken".
pub enum LightsReport {
    /// No `[lights]` table: the state every machine was in before it existed.
    Off,
    /// A table, and no `[plugins.hue]` table at all. Told apart from the
    /// switch below because they are different jobs: one config was never
    /// finished, the other was finished and turned off, and sending an
    /// operator to flip a switch that does not exist is a wrong direction
    /// they will act on.
    HueMissing,
    /// A table, with hue's own switch off. ONE SWITCH: hue is the transport
    /// and lights is the policy, so a policy with no transport lights nothing.
    HueDisabled,
    /// A table and an enabled hue, with no bridge and key to dial. Told apart
    /// from the state below for `hue_resolves`' own reason: one is a config to
    /// fix and the other is a network to fix.
    NoBridge,
    /// A bridge that answered no listing at all.
    Unreachable,
    Resolved(crate::lamps::Routing),
}

/// The lamps' own lines: how many lamps carry each behaviour, what could not be
/// resolved, and what was refused outright.
///
/// COUNTS AND NAMES ONLY, following the missed journal's structural privacy
/// rule: no colours, no session ids, no detail text.
pub fn lights_lines(report: &LightsReport) -> Vec<String> {
    let routing = match report {
        LightsReport::Off => {
            return vec![format!(
                "{PREFIX}lights: off in the config, so the pulse uses the [plugins.hue] rooms"
            )];
        }
        LightsReport::HueMissing => {
            return vec![format!(
                "{PREFIX}lights: configured, but there is no [plugins.hue] table to \
                 light them through"
            )];
        }
        LightsReport::HueDisabled => {
            return vec![format!(
                "{PREFIX}lights: configured, but [plugins.hue] enabled is false, so nothing lights"
            )];
        }
        LightsReport::NoBridge => {
            return vec![format!(
                "{PREFIX}lights: no [plugins.hue] bridge and key, so no lamp could be resolved"
            )];
        }
        LightsReport::Unreachable => {
            return vec![format!(
                "{PREFIX}lights: the bridge listed nothing, so no lamp resolved"
            )];
        }
        LightsReport::Resolved(routing) => routing,
    };

    // PER BEHAVIOUR RATHER THAN PER LAMP, because the question an operator opens
    // this section with is "did the thing I routed reach a bulb", and a lamp
    // count answers a different one. A behaviour NOTHING carries is listed at
    // zero, because "the word I wrote is missing from the report" is not a state
    // anybody should have to infer from an absence.
    let counted: Vec<String> = crate::lamps::config::BEHAVIOUR_WORDS
        .iter()
        .map(|(word, behaviour)| {
            let lamps = routing
                .lamps
                .iter()
                .filter(|routed| routed.shows.contains(behaviour))
                .count();
            format!("{word} {lamps}")
        })
        .collect();
    let mut lines = vec![format!("{PREFIX}lights: {}", counted.join(", "))];
    // THE SENTENCE ITSELF IS THE CHANNEL'S, so the tick reports an unresolved
    // lamp in the same words this does and only the prefix differs.
    for missing in &routing.unresolved {
        lines.push(format!(
            "{PREFIX}{}",
            crate::lamps::missing_sentence(missing)
        ));
    }
    for refusal in &routing.refusals {
        lines.push(format!("{PREFIX}{refusal}"));
    }
    lines
}
