use super::*;
use crate::config::parse_config;

/// A values table naming every core and opt-in table, all literals: the
/// shape `Answers::values()` produces once every question is answered.
fn every_table_armed() -> toml::Table {
    toml::toml! {
        [plugins.mobile]
        token = "moshi-secret"

        [plugins.hermes]
        key = "hermes-secret"

        [plugins.hue]
        bridge = "192.168.1.9"
        key = "hue-secret"
        rooms = ["Studio", "Kitchen"]

        [plugins.router]
        type = "unifi"
        router_url = "https://192.168.1.1"
        api_key = "router-secret"
        device_hostname = "phone"

        [focus]
        silence = ["Sleep"]

        [nag]
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

mod defaults;
mod layout;
mod literals;
mod notes;
mod secrets;
mod selection;
