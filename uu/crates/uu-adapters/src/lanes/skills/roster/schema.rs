use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(super) fn segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.contains('/')
        && !value.chars().any(char::is_control)
}
pub(super) fn text(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("missing or invalid `{key}`"))
}
pub(super) fn table(value: &Value, key: &str) -> Result<Map<String, Value>, String> {
    let entries = match value.get(key) {
        None => Map::new(),
        Some(value) => value
            .as_object()
            .cloned()
            .ok_or_else(|| format!("`{key}` is not an object"))?,
    };
    for name in entries.keys() {
        if !segment(name) {
            return Err(format!("`{key}` has unsafe name `{name}`"));
        }
    }
    Ok(entries)
}
pub(super) fn profiles(value: &Value) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or("profiles must be an array")?
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|s| segment(s))
                .map(str::to_owned)
                .ok_or_else(|| "invalid profile name".into())
        })
        .collect()
}
#[derive(Debug)]
pub struct HermesRegistryEntry {
    pub profiles: Vec<String>,
    pub source: String,
    pub identifier: String,
    pub lock_key: String,
    pub held: bool,
}
pub(super) fn registry(value: &Value) -> Result<BTreeMap<String, HermesRegistryEntry>, String> {
    table(value, "hermesRegistry")?
        .into_iter()
        .map(|(name, row)| {
            let profiles = profiles(row.get("profiles").ok_or("missing profiles")?)?;
            let held = match row.get("held") {
                None => false,
                Some(v) => v.as_bool().ok_or("held must be boolean")?,
            };
            Ok((
                name,
                HermesRegistryEntry {
                    profiles,
                    source: text(&row, "source")?,
                    identifier: text(&row, "identifier")?,
                    lock_key: text(&row, "lockKey")?,
                    held,
                },
            ))
        })
        .collect()
}
