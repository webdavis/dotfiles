use pns_adapters::SetupFailure;
use pns_domain::doctor::{Item, Mark};
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
        // IT NAMES THE TWO SETTINGS TO CHECK, because this is the reading the
        // operator actually meets: the probe cannot tell a router that refused
        // the key from one that never answered, so "unknown" on its own leaves
        // them nowhere to go. `clients()` reports one `None` for a rejected
        // key, a timeout and an unparseable body alike.
        HomePresence::Unknown => concat!(
            "unknown: the router returned no readable client list, so nothing was established; ",
            "check router_url and api_key in [plugins.router] ",
            "(a rejected key reads the same here as an unreachable router)"
        )
        .to_string(),
    }
}

/// What `pns doctor` says about one reading: the verdict, then one EVIDENCE
/// row per configured key, then the staleness warning for whatever the caller
/// hands in as news. PURE, so the words and the reading cannot drift apart
/// untested.
///
/// THE EVIDENCE IS NEVER WITHHELD. A hand-run report answers "why did it read
/// that" as much as "what did it read", so every configured key says what it
/// found on every run, however many times it has said it before. The rows
/// are `Detail`, which is what makes them read as the verdict's explanation
/// rather than as four more findings.
///
/// RENDERING ONLY: `news` arrives already decided, because the caller that
/// decides it is also the one that REMEMBERS it. Deriving the episode here
/// as well would settle one fact twice per run off two call sites, and the
/// day either grows a condition (a channel gate, a quiet window) the line
/// the operator read and the episode the file recorded could disagree about
/// what they were told.
pub(crate) fn rows(reading: &HomeReading, news: Option<&Staleness>) -> Vec<Item> {
    let mut rows = vec![Item::row(
        verdict_mark(&reading.presence),
        format!("home: {}", verdict_line(&reading.presence)),
    )];
    // EVERY CONFIGURED KEY GETS A ROW, whatever it found, because the report's
    // job is to show the disagreement rather than the winner.
    rows.extend(
        reading
            .keys
            .iter()
            .map(|key| Item::row(Mark::Detail, evidence_line(key))),
    );
    // ONLY THE ALERT-SHAPED LINE IS DEDUPED. The evidence above it says the
    // same thing in more words every single run; this one sentence is the
    // one a consumer would act on, so it is said once per state.
    if let Some(staleness) = news {
        rows.push(Item::row(Mark::Warn, stale_warning(staleness)));
    }
    rows
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

/// How a verdict reads at a glance.
///
/// NOTHING HERE IS `Bad`, and none of it moves the exit code. An unread
/// router costs the away reading, which no notification path depends on; a
/// doctor that graded it as broken would teach the operator that a red doctor
/// does not mean their notifications are broken.
fn verdict_mark(presence: &HomePresence) -> Mark {
    match presence {
        HomePresence::Home { .. } => Mark::Good,
        // OUT OF THE HOUSE IS NOT A FAULT, and it is the ordinary reading.
        HomePresence::NotHome => Mark::Note,
        // UNKNOWN IS A WARNING, NOT A VERDICT. The router did not answer, so
        // nothing was established either way, and reading it as "not home" is
        // the mistake this arm exists to prevent. A warning withholds the
        // report's all-clear, which is what makes it visible at all: this is
        // the reading a machine with a rejected key gives on every run.
        HomePresence::Unknown => Mark::Warn,
    }
}

/// The row for a setup failure, which is every way the probe can be unread
/// before a router is dialled at all.
///
/// A PROBE NOBODY SET UP IS A NOTE, and a probe somebody set up WRONG is a
/// warning. The first three arms are an absence the operator chose, and
/// grading a choice as a fault is how a reader learns to skim the marks; the
/// rest are a `[plugins.router]` table that was written and does not work,
/// which is an edit waiting to be made.
pub(crate) fn setup_row(failure: &SetupFailure) -> Item {
    let mark = match failure {
        SetupFailure::NoConfigFile
        | SetupFailure::NoRouterPlugin
        | SetupFailure::RouterDisabled => Mark::Note,
        SetupFailure::ConfigError(_)
        | SetupFailure::NoType
        | SetupFailure::UnknownType(_)
        | SetupFailure::InvalidRouterTable
        | SetupFailure::NoDeviceIdentifier
        | SetupFailure::InvalidDeviceKey { .. }
        | SetupFailure::NoApiKey => Mark::Warn,
    };
    Item::row(mark, setup_report(failure))
}

/// The one line for a setup failure. PURE for the same reason as `rows`.
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
