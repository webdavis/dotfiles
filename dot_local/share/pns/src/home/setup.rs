//! Reading the router's own settings off the config, and saying what a setup
//! that cannot answer is missing.
//!
//! Split from `home.rs` for the size rule, along the seam the migration plan
//! already draws: these follow the config edge, the rest follows the router.

use super::{
    DeviceIdentity, DeviceKey, HomeReading, KeyOutcome, RouterSettings, Staleness, UNIFI_TYPE,
    normalized_mac, stale_warning, verdict_line,
};

/// The enabled router sensor's settings table, or the cause it could not be
/// had. The probe's config home is `[plugins.router]`, the sensor registered
/// in the roster, so the whole file is read by one schema and the operator has
/// one spelling to get right.
pub fn enabled_router_table(config: &crate::config::Config) -> Result<&toml::Table, SetupFailure> {
    let entry = config
        .plugins
        .get("router")
        .ok_or(SetupFailure::NoRouterPlugin)?;
    // A probe the operator SWITCHED OFF is not one they never wrote: one is
    // fixed by flipping the flag in front of them, the other by writing a
    // table, and a single "not configured" line sends half of them to the
    // wrong edit.
    if !entry.enabled {
        return Err(SetupFailure::RouterDisabled);
    }
    Ok(&entry.settings)
}
/// The settings out of the router sensor's table, or the cause they could not
/// be had. The TYPE is settled first, because every setting under it belongs
/// to whichever router it names.
///
/// THE SAME QUESTION `channels::moshi::mobile_backend` ASKS OF THE MOBILE
/// TABLE, and the two refusals are worded to match on purpose: name the table,
/// quote what was written, name the one type that answers. Reword one and
/// reword the other, or the rename that gave both tables one word leaves them
/// two sentences. THAT THE TYPE IS SETTLED FIRST is also what lets the doctor
/// read its two type refusals off this function rather than re-deriving them.
pub fn router_settings(router: &toml::Table) -> Result<RouterSettings, SetupFailure> {
    // Present but EMPTY is the key left blank, the same hole as absent: the
    // filter is what keeps `type = ""` from being quoted back as a type no
    // backend answers, which names a value the operator never typed.
    let named = router
        .get("type")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(SetupFailure::NoType)?;
    if named != UNIFI_TYPE {
        return Err(SetupFailure::UnknownType(named.to_string()));
    }
    // Present but empty is a hole, not a value, and present but the wrong
    // type is refused rather than coerced: both are one line for the operator
    // to fix, and neither is a router this probe could reach.
    let router_url = router
        .get("router_url")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(SetupFailure::InvalidRouterTable)?;
    Ok(RouterSettings {
        router_url,
        device: device_identity(router)?,
    })
}
/// The configured device out of the router's table, or the cause it could
/// not be had. ABSENT IS NOT CONFIGURED, and all three absent is refused: a
/// probe with nothing to look for would read NotHome forever.
pub fn device_identity(router: &toml::Table) -> Result<DeviceIdentity, SetupFailure> {
    // A client name is an arbitrary label, so there is no shape to check
    // beyond it being a string; an address is PARSED, so no unvalidated text
    // can reach a comparison.
    let hostname = read_device_key(router, DeviceKey::Hostname, |text| Some(text.to_string()))?;
    let ipv4 = read_device_key(router, DeviceKey::Ipv4, |text| text.parse().ok())?;
    let mac = read_device_key(router, DeviceKey::Mac, normalized_mac)?;
    if hostname.is_none() && ipv4.is_none() && mac.is_none() {
        return Err(SetupFailure::NoDeviceIdentifier);
    }
    Ok(DeviceIdentity {
        hostname,
        ipv4,
        mac,
    })
}
/// One optional device key, read through its own shape. ABSENT STAYS ABSENT;
/// present but empty, the wrong type, or unreadable as that shape is refused
/// by name. A blank key read as absent is the silent typo the output cannot
/// show: the probe would report having nothing to look for while the
/// operator is looking at the key they filled in.
fn read_device_key<T>(
    router: &toml::Table,
    key: DeviceKey,
    read: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, SetupFailure> {
    let Some(value) = router.get(key.config_key()) else {
        return Ok(None);
    };
    let refuse = || SetupFailure::InvalidDeviceKey {
        key,
        found: spell(value),
    };
    let text = value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(refuse)?;
    read(text).map(Some).ok_or_else(refuse)
}
/// One config value as the operator would see it in the file: a string in
/// quotes, anything else by its TOML type, because a non-string has no
/// spelling worth echoing back and its TYPE is what has to change.
fn spell(value: &toml::Value) -> String {
    match value.as_str() {
        Some(text) => format!("{text:?}"),
        None => format!("<{}>", value.type_str()),
    }
}
/// The router's API key out of its own table (`api_key`), or `None` for every
/// way the config can fail to provide one.
///
/// SEPARATE from the settings, and it stays that way: the key never enters a
/// type that derives Debug, so it cannot ride a formatted dump into a log
/// line. Mirrors `moshi_secret`: the path from config to request header must
/// never touch argv, the environment, or an error string.
pub fn router_api_key(router: &toml::Table) -> Option<String> {
    let key = router.get("api_key")?.as_str()?;
    (!key.is_empty()).then(|| key.to_string())
}
/// The hermes route the stale alert posts to, plus the complaint a value that
/// could not be one earns. EMPTY IS THE DEFAULT ROUTE (`/webhooks/pns`), the
/// same spelling `--channel` and `hermes_url_for` already use, so one
/// vocabulary covers all three.
///
/// VALIDATED HERE rather than where the URL is built, because the operator
/// TYPED THIS KEY: `hermes_url_for`'s own refusal names `--channel`, a flag
/// nobody passed on this path, and would send them hunting for it.
///
/// LOUD-WARD ON EVERY FAILURE. A value of the wrong type, an empty string and
/// a name no URL could carry all fall back to the default route with one
/// complaint, because they are one fix; the alert still goes out, on the route
/// the operator would have got had they written nothing. Refusing to alert
/// over a misspelled route would let a config typo silence the very warning it
/// is configuring, and a diagnostic that can be taken down by its own settings
/// is not one.
///
/// THE COMPLAINT IS RETURNED, not printed: this stays a value function, and
/// the composition root decides that a warning goes to stderr, exactly as
/// `select_plugins` hands its roster warning back.
pub fn stale_alert_channel(router: &toml::Table) -> (String, Option<String>) {
    let Some(value) = router.get("stale_alert_channel") else {
        return (String::new(), None);
    };
    match value
        .as_str()
        .filter(|route| crate::safety::route_name_is_usable(route))
    {
        Some(route) => (route.to_string(), None),
        None => (
            String::new(),
            Some(format!(
                "pns: config error (stale_alert_channel = {} in [plugins.router] is not a \
                 usable route name); the stale alert posts to the default route",
                spell(value)
            )),
        ),
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
/// Every way the probe can be not set up, as data: the diagnostic's whole
/// vocabulary in one place, each state with its own line, because one
/// "not configured" covering both a missing table and a mistyped value sent
/// the operator to write a table they already had.
#[derive(Debug, PartialEq)]
pub enum SetupFailure {
    NoConfigFile,
    ConfigError(String),
    NoRouterPlugin,
    RouterDisabled,
    NoType,
    UnknownType(String),
    InvalidRouterTable,
    NoDeviceIdentifier,
    InvalidDeviceKey { key: DeviceKey, found: String },
    NoApiKey,
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
