mod policy;
mod presets;
mod windows;

use lights_domain::{Aliases, CertificatePin, PresetWindows, Presets, RoomName, Rotation};
use std::path::Path;

pub struct Settings {
    pub default_room: RoomName,
    pub aliases: Aliases,
    pub rotation: Rotation,
    pub remember_position: bool,
    pub presets: Presets,
    pub preset_windows: PresetWindows,
    pub step: u8,
    pub notify: bool,
    pub controller: HueSettings,
}

pub struct HueSettings {
    pub address: String,
    key: String,
    pub timeout_secs: u64,
    /// The one certificate the bridge may present. REQUIRED, and a value
    /// rather than an option: the bridge's certificate carries no name any
    /// verifier can check, so its fingerprint is the whole of what makes the
    /// address in the config the device the operator meant.
    pub certificate: CertificatePin,
}

impl HueSettings {
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfigError(pub String);

/// Where the bridge is and how long it may take, WITHOUT the pin.
///
/// ENROLLMENT IS THE COMMAND THAT PRODUCES THE PIN, so it cannot be made to
/// require one: a config parse that refuses for want of a certificate would
/// refuse the one command that hands the operator a certificate to save.
pub struct Endpoint {
    pub address: String,
    pub timeout_secs: u64,
}

/// The controller's keys, in one place, because two entry points read them.
const CONTROLLER_KEYS: &[&str] = &["type", "address", "certificate", "key", "timeout_secs"];

/// The root's keys, in one place, because two entry points read them.
const ROOT_KEYS: &[&str] = &[
    "controller",
    "default_room",
    "rooms",
    "scenes",
    "brightness",
    "notify",
    "presets",
    "preset_windows",
];

pub fn endpoint(text: &str) -> Result<Endpoint, ConfigError> {
    let root = root(text)?;
    keys(&root, ROOT_KEYS)?;
    let controller = table(&root, "controller")?;
    keys(controller, CONTROLLER_KEYS)?;
    endpoint_in(controller)
}

pub fn load_endpoint(path: &Path) -> Result<Endpoint, ConfigError> {
    endpoint(&read(path)?)
}

fn endpoint_in(controller: &toml::Table) -> Result<Endpoint, ConfigError> {
    let kind = required_string(controller, "type")?;
    if kind != "hue" {
        return Err(error(&format!("unknown controller type {kind:?}")));
    }
    let address = required_string(controller, "address")?;
    if !address
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b".:-[]".contains(&c))
    {
        return Err(error("invalid controller address"));
    }
    let timeout_secs = integer(controller, "timeout_secs", 2)?;
    if timeout_secs == 0 {
        return Err(error("timeout_secs must be positive"));
    }
    Ok(Endpoint {
        address,
        timeout_secs,
    })
}

pub fn parse(text: &str) -> Result<Settings, ConfigError> {
    let root = root(text)?;
    keys(&root, ROOT_KEYS)?;
    let controller = table(&root, "controller")?;
    keys(controller, CONTROLLER_KEYS)?;
    let Endpoint {
        address,
        timeout_secs,
    } = endpoint_in(controller)?;
    let key = required_string(controller, "key")?;
    if key.chars().any(char::is_control) {
        return Err(error("invalid controller key"));
    }
    // FAIL CLOSED. A bridge and key with no certificate is a controller
    // somebody armed and did not finish, so it is refused here, where the
    // operator can see it, rather than trusted over a connection that verifies
    // nothing.
    let certificate =
        CertificatePin::parse(&required_string(controller, "certificate").map_err(|_| {
            error(
                "controller certificate missing or not a string; run \
`lights enroll --bridge-id <id>` and save the line it prints",
            )
        })?)
        .map_err(|why| error(why.0))?;
    policy::parse(
        &root,
        HueSettings {
            address,
            key,
            timeout_secs,
            certificate,
        },
    )
}

pub fn load(path: &Path) -> Result<Settings, ConfigError> {
    parse(&read(path)?)
}

fn read(path: &Path) -> Result<String, ConfigError> {
    std::fs::read_to_string(path).map_err(|_| error("cannot read config"))
}

fn root(text: &str) -> Result<toml::Table, ConfigError> {
    text.parse::<toml::Table>()
        .map_err(|_| error("malformed config"))
}

fn error(message: &str) -> ConfigError {
    ConfigError(message.into())
}

fn keys(table: &toml::Table, allowed: &[&str]) -> Result<(), ConfigError> {
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(error(&format!("unknown config key {key:?}")));
        }
    }
    Ok(())
}

fn table<'a>(root: &'a toml::Table, name: &str) -> Result<&'a toml::Table, ConfigError> {
    root.get(name)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| error(&format!("missing or invalid {name} table")))
}

fn required_string(table: &toml::Table, name: &str) -> Result<String, ConfigError> {
    table
        .get(name)
        .and_then(toml::Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| error(&format!("missing or invalid {name}")))
}

fn integer(table: &toml::Table, name: &str, default: u64) -> Result<u64, ConfigError> {
    match table.get(name) {
        None => Ok(default),
        Some(value) => value
            .as_integer()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| error(&format!("invalid {name}"))),
    }
}

#[cfg(test)]
mod tests;
