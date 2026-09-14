use super::{ConfigError, error, keys, required_string};
use lights_domain::{Aliases, PresetStep, PresetTarget, Presets};
use std::collections::BTreeMap;

pub(super) fn parse(root: &toml::Table, aliases: &Aliases) -> Result<Presets, ConfigError> {
    let Some(value) = root.get("presets") else {
        return Ok(Presets::default());
    };
    let table = value
        .as_table()
        .ok_or_else(|| error("invalid presets table"))?;
    let mut presets = BTreeMap::new();
    for (name, value) in table {
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(error("invalid preset name"));
        }
        let rows = value
            .as_array()
            .ok_or_else(|| error(&format!("preset {name} must be a list of steps")))?;
        if rows.is_empty() {
            return Err(error(&format!("preset {name} has no steps")));
        }
        presets.insert(
            name.clone(),
            rows.iter()
                .map(|row| step(row, aliases, name))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    Ok(Presets::new(presets))
}

fn step(row: &toml::Value, aliases: &Aliases, preset: &str) -> Result<PresetStep, ConfigError> {
    let row = row
        .as_table()
        .ok_or_else(|| error(&format!("preset {preset} step must be a table")))?;
    keys(row, &["room", "scene", "off"])?;
    let room = aliases
        .resolve(&required_string(row, "room")?)
        .map_err(|e| error(e.0))?;
    // EXACTLY ONE TARGET. `off = false` says nothing, so it is refused by the
    // same arm that refuses naming neither: a step that does nothing is a
    // typo, not a setting.
    let target = match (row.contains_key("scene"), row.get("off")) {
        (true, None) => PresetTarget::Scene(required_string(row, "scene")?),
        (false, Some(toml::Value::Boolean(true))) => PresetTarget::Off,
        _ => {
            return Err(error(&format!(
                "preset {preset} step needs one of scene or off = true"
            )));
        }
    };
    Ok(PresetStep { room, target })
}
