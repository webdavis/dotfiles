use super::{ConfigError, error, keys, required_string};
use lights_domain::{MinuteOfDay, PresetWindow, PresetWindows, Presets};

/// AN ARRAY OF TABLES, not a key inside `[presets]`: every key of that table is
/// a preset name the operator chose, so a reserved one there would steal a name
/// and be read as a preset with no steps.
pub(super) fn parse(root: &toml::Table, presets: &Presets) -> Result<PresetWindows, ConfigError> {
    let Some(value) = root.get("preset_windows") else {
        return Ok(PresetWindows::default());
    };
    let rows = value
        .as_array()
        .ok_or_else(|| error("preset_windows must be a list of windows"))?;
    if rows.is_empty() {
        return Err(error("preset_windows has no windows"));
    }
    let windows = rows
        .iter()
        .map(window)
        .collect::<Result<Vec<_>, _>>()
        .map(PresetWindows::new)?;
    // A window naming a preset that does not exist is a typo the operator
    // would only meet at the minute it fires, so it is refused at load with
    // every other settings error.
    for name in windows.presets() {
        if presets.plan(name).is_none() {
            return Err(error(&format!(
                "preset window names unknown preset {name:?}"
            )));
        }
    }
    Ok(windows)
}

fn window(row: &toml::Value) -> Result<PresetWindow, ConfigError> {
    let row = row
        .as_table()
        .ok_or_else(|| error("preset window must be a table"))?;
    keys(row, &["start", "end", "preset"])?;
    PresetWindow::new(
        minute(row, "start")?,
        minute(row, "end")?,
        required_string(row, "preset")?,
    )
    .map_err(|e| error(e.0))
}

/// `HH:MM` on a 24 hour clock, which is how the operator reads a clock and the
/// only spelling accepted: a bare hour or a `9:5` would each have to be guessed
/// at.
fn minute(row: &toml::Table, name: &str) -> Result<MinuteOfDay, ConfigError> {
    let text = required_string(row, name)?;
    let invalid = || error(&format!("preset window {name} must be HH:MM"));
    let (hours, minutes) = text.split_once(':').ok_or_else(invalid)?;
    if hours.len() != 2 || minutes.len() != 2 {
        return Err(invalid());
    }
    let hours: MinuteOfDay = hours.parse().map_err(|_| invalid())?;
    let minutes: MinuteOfDay = minutes.parse().map_err(|_| invalid())?;
    if hours > 23 || minutes > 59 {
        return Err(invalid());
    }
    Ok(hours * 60 + minutes)
}
