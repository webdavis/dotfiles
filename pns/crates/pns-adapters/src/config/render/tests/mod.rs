use super::*;
use crate::config::parse_config;

/// A values table naming every core and opt-in table, all literals: the
/// shape `Answers::values()` produces once every question is answered.
fn every_table_armed() -> toml::Table {
    toml::toml! {
        [plugins.phone]
        device_token = "moshi-secret"

        [plugins.log.keys]
        pns-events = "hermes-secret"

        [plugins.lights]
        bridge_host = "192.168.1.9"
        api_key = "hue-secret"
        rooms = ["Studio", "Kitchen"]

        [plugins.home_presence]
        type = "unifi"
        url = "https://192.168.1.1"
        api_key = "router-secret"
        device_hostname = "phone"

        [focus]
        modes = ["Sleep"]

        [remind]
        delay = "5m"
    }
}

/// A secret marker for one keepassxc entry and field.
fn secret(entry: &str, field: &str) -> toml::Value {
    let mut table = toml::Table::new();
    table.insert(
        "keepassxc".to_string(),
        toml::Value::String(entry.to_string()),
    );
    table.insert("field".to_string(), toml::Value::String(field.to_string()));
    toml::Value::Table(table)
}

/// A secret marker for a CUSTOM ATTRIBUTE on one keepassxc entry, which is a
/// different chezmoi function rather than a third field name.
fn attribute_secret(entry: &str, attribute: &str) -> toml::Value {
    let mut table = toml::Table::new();
    table.insert(
        "keepassxc".to_string(),
        toml::Value::String(entry.to_string()),
    );
    table.insert(
        "attribute".to_string(),
        toml::Value::String(attribute.to_string()),
    );
    toml::Value::Table(table)
}

mod defaults;
mod layout;
mod literals;
mod notes;
mod secrets;
mod selection;
