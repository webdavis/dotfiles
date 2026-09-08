mod policy;

use lights_domain::{Aliases, RoomName, Rotation};
use std::path::Path;

pub struct Settings {
    pub default_room: RoomName,
    pub aliases: Aliases,
    pub rotation: Rotation,
    pub step: u8,
    pub controller: HueSettings,
}

pub struct HueSettings {
    pub address: String,
    key: String,
    pub timeout_secs: u64,
}

impl HueSettings {
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfigError(pub String);

pub fn parse(text: &str) -> Result<Settings, ConfigError> {
    let root = text
        .parse::<toml::Table>()
        .map_err(|_| error("malformed config"))?;
    keys(
        &root,
        &[
            "controller",
            "default_room",
            "rooms",
            "scenes",
            "brightness",
            "notify",
        ],
    )?;
    let controller = table(&root, "controller")?;
    keys(controller, &["type", "address", "key", "timeout_secs"])?;
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
    let key = required_string(controller, "key")?;
    if key.chars().any(char::is_control) {
        return Err(error("invalid controller key"));
    }
    let timeout_secs = integer(controller, "timeout_secs", 2)?;
    if timeout_secs == 0 {
        return Err(error("timeout_secs must be positive"));
    }
    policy::parse(
        &root,
        HueSettings {
            address,
            key,
            timeout_secs,
        },
    )
}

pub fn load(path: &Path) -> Result<Settings, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|_| error("cannot read config"))?;
    parse(&text)
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
