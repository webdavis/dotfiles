use super::{ConfigError, HueSettings, Settings, error, integer, keys, required_string};
use lights_domain::{Aliases, RoomName, Rotation};
use std::collections::BTreeMap;

pub(super) fn parse(root: &toml::Table, controller: HueSettings) -> Result<Settings, ConfigError> {
    let notify = match root.get("notify") {
        None => false,
        Some(toml::Value::Boolean(value)) => *value,
        Some(_) => return Err(error("invalid notify")),
    };
    let default_room =
        RoomName::new(string_or(root, "default_room", "3F - Studio")?).map_err(|e| error(e.0))?;
    let brightness = optional_table(root, "brightness")?;
    keys(&brightness, &["step"])?;
    let step = integer(&brightness, "step", 15)?;
    if !(1..=100).contains(&step) {
        return Err(error("step must be 1 through 100"));
    }
    let mut aliases = BTreeMap::new();
    for (alias, room) in [
        ("studio", "3F - Studio"),
        ("bedroom", "3F - Master Bedroom"),
        ("kitchen", "2F - Kitchen"),
    ] {
        aliases.insert(alias.into(), RoomName::new(room).map_err(|e| error(e.0))?);
    }
    let rooms = optional_table(root, "rooms")?;
    for alias in rooms.keys() {
        if alias.trim().is_empty() || alias.chars().any(char::is_control) {
            return Err(error("invalid room alias"));
        }
        aliases.insert(
            alias.clone(),
            RoomName::new(required_string(&rooms, alias)?).map_err(|e| error(e.0))?,
        );
    }
    let scenes = optional_table(root, "scenes")?;
    keys(&scenes, &["rotation", "fallback"])?;
    let names = match scenes.get("rotation") {
        None => ["Dimmed", "Read", "Energize", "Concentrate"]
            .map(str::to_owned)
            .to_vec(),
        Some(value) => value
            .as_array()
            .ok_or_else(|| error("invalid rotation"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| error("invalid rotation scene"))
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    let rotation =
        Rotation::new(names, string_or(&scenes, "fallback", "Read")?).map_err(|e| error(e.0))?;
    Ok(Settings {
        controller,
        notify,
        default_room,
        aliases: Aliases::new(aliases),
        rotation,
        step: step as u8,
    })
}

fn optional_table(root: &toml::Table, name: &str) -> Result<toml::Table, ConfigError> {
    match root.get(name) {
        None => Ok(toml::Table::new()),
        Some(value) => value
            .as_table()
            .cloned()
            .ok_or_else(|| error(&format!("invalid {name} table"))),
    }
}

fn string_or(table: &toml::Table, name: &str, default: &str) -> Result<String, ConfigError> {
    if table.contains_key(name) {
        required_string(table, name)
    } else {
        Ok(default.into())
    }
}
