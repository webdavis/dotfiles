use crate::style::{self, Paint, Tone};
use pns_adapters::SetupFailure;
use pns_domain::home::{
    DeviceKey, HomePresence, HomeReading, KeyOutcome, Staleness, UNIFI_TYPE, stale_warning,
};

/// The one line for the verdict itself. PURE for the same reason as its
/// caller: a swap of the two sentences below survived every suite before
/// this was a function of its own.
fn verdict_line(presence: &HomePresence) -> String {
    match presence {
        // The matched value is DEBUG-QUOTED, the same escape `spell` gives a
        // config value: the value came from the router's own listing, so a
        // client name carrying a quote or a control byte would otherwise reach
        // a terminal verbatim. A plain name reads exactly as it did before.
        HomePresence::Home { matched_by, value } => format!(
            "on the home network, matched by {} {value:?}",
            matched_by.config_key()
        ),
        HomePresence::NotHome => {
            "NOT on the home network: no configured identifier matched a client".to_string()
        }
        HomePresence::Unknown => {
            "unknown: the router was unreachable or its answer unreadable".to_string()
        }
    }
}

/// What `pns home` says for one reading: the verdict, then one EVIDENCE line
/// per configured key, then the staleness warning for whatever the caller
/// hands in as news. PURE, so the words and the reading cannot drift apart
/// untested.
///
/// THE EVIDENCE IS NEVER WITHHELD. A hand-run diagnostic answers "why did it
/// read that" as much as "what did it read", so every configured key says
/// what it found on every run, however many times it has said it before.
///
/// RENDERING ONLY: `news` arrives already decided, because the caller that
/// decides it is also the one that REMEMBERS it. Deriving the episode here
/// as well would settle one fact twice per run off two call sites, and the
/// day either grows a condition (a channel gate, a quiet window) the line
/// the operator read and the episode the file recorded could disagree about
/// what they were told.
pub(crate) fn report(paint: Paint, reading: &HomeReading, news: Option<&Staleness>) -> String {
    let mut lines = vec![
        style::heading(paint, "Verdict", "what the router's client list says"),
        String::new(),
        style::row(
            paint,
            verdict_tone(&reading.presence),
            verdict_glyph(&reading.presence),
            2,
            &verdict_line(&reading.presence),
        ),
    ];
    if !reading.keys.is_empty() {
        lines.push(String::new());
        lines.push(style::heading(
            paint,
            "Evidence",
            "what each configured identifier matched",
        ));
        lines.push(String::new());
        // EVERY CONFIGURED KEY GETS A ROW, whatever it found, because the
        // diagnostic's job is to show the disagreement rather than the winner.
        lines.extend(
            reading
                .keys
                .iter()
                .map(|key| style::row(paint, Tone::Quiet, "\u{b7}", 2, &evidence_line(key))),
        );
    }
    // ONLY THE ALERT-SHAPED LINE IS DEDUPED. The evidence above it says the
    // same thing in more words every single run; this one sentence is the
    // one a consumer would act on, so it is said once per state.
    if let Some(staleness) = news {
        lines.push(String::new());
        lines.push(style::row(
            paint,
            Tone::Warn,
            "\u{26a0}",
            2,
            &stale_warning(staleness),
        ));
    }
    lines.join("\n")
}

/// One evidence row's sentence: the key, what it was set to, and what the
/// router's client list did with it.
///
/// THE VALUE AND THE CLIENT LABEL ARE DEBUG-QUOTED. Both came from outside:
/// the value from the operator's config and the label from the router's own
/// listing, so either could carry a quote or a control byte that would
/// otherwise reach a terminal verbatim.
fn evidence_line(key: &pns_domain::home::KeyReading) -> String {
    format!(
        "{:<18}{:?}   {}",
        key.key.config_key(),
        key.value,
        match &key.outcome {
            // THE CLIENT THE VERDICT NAMES, which is all the scan
            // established. "this device" would claim identity with the
            // operator's own hardware, and when the winning key is itself the
            // stale one (a reclaimed lease answering for a phone that left)
            // the entry it names belongs to somebody else.
            KeyOutcome::MatchedDevice => "matched the client the verdict names".to_string(),
            KeyOutcome::MatchedOtherClient { client } =>
                format!("matched a different client {client}"),
            KeyOutcome::MatchedNothing => "matched no client".to_string(),
        }
    )
}

/// A verdict is good, bad or unknown, and the glyph says which without reading.
fn verdict_tone(presence: &HomePresence) -> Tone {
    match presence {
        HomePresence::Home { .. } => Tone::Good,
        HomePresence::NotHome => Tone::Quiet,
        // UNKNOWN IS A WARNING, NOT A VERDICT. The router did not answer, so
        // nothing was established either way, and reading it as "not home"
        // is the mistake this arm exists to prevent.
        HomePresence::Unknown => Tone::Warn,
    }
}

fn verdict_glyph(presence: &HomePresence) -> &'static str {
    match presence {
        HomePresence::Home { .. } => "\u{2713}",
        HomePresence::NotHome => "\u{b7}",
        HomePresence::Unknown => "\u{26a0}",
    }
}
/// The one line for a setup failure. PURE for the same reason as `report`.
pub fn setup_report(failure: &SetupFailure) -> String {
    match failure {
        SetupFailure::NoConfigFile => "home: not configured (no config file)".to_string(),
        SetupFailure::ConfigError(detail) => format!("home: config error ({detail})"),
        SetupFailure::NoRouterPlugin => {
            "home: not configured (no [plugins.router] table)".to_string()
        }
        SetupFailure::RouterDisabled => {
            "home: [plugins.router] is present but enabled = false".to_string()
        }
        SetupFailure::NoType => {
            format!("home: no type in [plugins.router] (the only type is \"{UNIFI_TYPE}\")")
        }
        SetupFailure::UnknownType(named) => format!(
            "home: [plugins.router] has type {named:?}, which no compiled-in backend \
             answers (the only type is \"{UNIFI_TYPE}\")"
        ),
        SetupFailure::InvalidRouterTable => {
            "home: the [plugins.router] table is present but router_url is missing, empty, \
             or not a string"
                .to_string()
        }
        SetupFailure::NoDeviceIdentifier => format!(
            "home: no device to look for in [plugins.router] (set at least one of {}, {}, {})",
            DeviceKey::Mac.config_key(),
            DeviceKey::Hostname.config_key(),
            DeviceKey::Ipv4.config_key()
        ),
        SetupFailure::InvalidDeviceKey { key, found } => {
            let shape = match key {
                DeviceKey::Mac => {
                    "a MAC address (six hex pairs under one separator, e.g. \"2e:11:ab:6d:b0:4f\")"
                }
                DeviceKey::Hostname => "a client name (a non-empty string)",
                DeviceKey::Ipv4 => "an IPv4 address (a dotted quad, e.g. \"192.168.1.169\")",
            };
            format!(
                "home: {} = {found} in [plugins.router] is not {shape}",
                key.config_key()
            )
        }
        SetupFailure::NoApiKey => {
            "home: no api_key in the [plugins.router] table (the probe is not set up)".to_string()
        }
    }
}

#[cfg(test)]
mod tests;
