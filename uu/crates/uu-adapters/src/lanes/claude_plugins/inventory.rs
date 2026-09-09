use super::super::changes::Listing;
use serde_json::Value;

pub(super) fn read_inventory(text: &str) -> Result<Listing, String> {
    let document: Value = serde_json::from_str(text)
        .map_err(|error| format!("inventory is not one JSON document: {error}"))?;
    let plugins = document
        .get("plugins")
        .and_then(Value::as_object)
        .ok_or("inventory .plugins is not an object")?;
    if plugins.is_empty() {
        return Err("inventory .plugins is empty".into());
    }
    let mut rows = Vec::new();
    for (name, entries) in plugins {
        let entries = entries
            .as_array()
            .ok_or("plugin records are not an array")?;
        for entry in entries {
            let entry = entry.as_object().ok_or("plugin record is not an object")?;
            let scope = entry
                .get("scope")
                .and_then(Value::as_str)
                .ok_or("plugin record .scope is not a string")?;
            if scope != "user" {
                continue;
            }
            let version = entry
                .get("version")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && *value != "unknown");
            let commit = entry
                .get("gitCommitSha")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty());
            rows.push((field(name), field(version.or(commit).unwrap_or("unknown"))));
        }
    }
    rows.sort();
    Ok(rows)
}

// Keep the bash reporter's jq @tsv field encoding so imported history compares byte for byte.
fn field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests;
