use super::{DeviceKey, verdict_line};
use super::{HomeReading, KeyOutcome, Staleness, UNIFI_TYPE, stale_warning};
pub use pns_adapters::{
    SetupFailure, device_identity, enabled_router_table, router_api_key, router_settings,
    stale_alert_channel,
};
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
pub fn report(reading: &HomeReading, news: Option<&Staleness>) -> String {
    let mut lines = vec![verdict_line(&reading.presence)];
    lines.extend(reading.keys.iter().map(|key| {
        format!(
            "home:   {} {:?} {}",
            key.key.config_key(),
            key.value,
            match &key.outcome {
                // THE CLIENT THE VERDICT NAMES, which is all the scan
                // established. "this device" would claim identity with the
                // operator's own hardware, and when the winning key is
                // itself the stale one (a reclaimed lease answering for a
                // phone that left) the entry it names belongs to somebody
                // else. The evidence surface exists to be read on exactly
                // that reading, so it says what it knows.
                KeyOutcome::MatchedDevice => "matched the client the verdict names".to_string(),
                KeyOutcome::MatchedOtherClient { client } =>
                    format!("matched a different client {client}"),
                KeyOutcome::MatchedNothing => "matched no client".to_string(),
            }
        )
    }));
    // ONLY THE ALERT-SHAPED LINE IS DEDUPED. The evidence above it says the
    // same thing in more words every single run; this one sentence is the
    // one a consumer would act on, so it is said once per state.
    if let Some(staleness) = news {
        lines.push(stale_warning(staleness));
    }
    lines.join("\n")
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
