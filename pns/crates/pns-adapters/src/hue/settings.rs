use pns_domain::lamps::{QuietWindow, parse_window};

/// The rooms the bash pulsed when `HUE_PULSE_ROOMS` said nothing.
pub const DEFAULT_ROOMS: &[&str] = &["3F - Studio", "2F - Kitchen"];

/// Everything the pulse needs from the config, or None for the not-set-up
/// silence: a bridge and key are required, rooms come from the environment
/// override (newline-separated, room names carry spaces), else the settings
/// array, else the defaults.
#[derive(Debug, PartialEq)]
pub struct HueSettings {
    pub bridge: String,
    pub key: String,
    pub rooms: Vec<String>,
}

pub fn hue_settings(settings: &toml::Table, rooms_env: Option<&str>) -> Option<HueSettings> {
    let text = |key: &str| -> Option<String> {
        settings
            .get(key)?
            .as_str()
            .filter(|value| !value.is_empty())
            .map(String::from)
    };
    let from_env: Vec<String> = rooms_env
        .unwrap_or_default()
        .lines()
        .filter(|room| !room.is_empty())
        .map(String::from)
        .collect();
    Some(HueSettings {
        bridge: text("bridge")?,
        key: text("key")?,
        rooms: if from_env.is_empty() {
            settings
                .get("rooms")
                .and_then(|rooms| rooms.as_array())
                .map(|rooms| {
                    rooms
                        .iter()
                        .filter_map(|room| room.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .filter(|rooms| !rooms.is_empty())
                .unwrap_or_else(|| DEFAULT_ROOMS.iter().map(|room| room.to_string()).collect())
        } else {
            from_env
        },
    })
}

/// The window the operator configured, or None for no window at all.
///
/// A value that is not a `HH:MM-HH:MM` string is a REFUSAL rather than a
/// silent no-window: an operator who asked for quiet hours and mistyped them
/// would otherwise be flashed at 3am and told nothing.
pub fn quiet_window(settings: &toml::Table) -> Result<Option<QuietWindow>, String> {
    let Some(stated) = settings.get("quiet_hours") else {
        return Ok(None);
    };
    let Some(text) = stated.as_str() else {
        return Err(quiet_hours_refusal(stated.type_str()));
    };
    // EMPTY IS ABSENT, the rule the bridge and key beside it already follow.
    if text.is_empty() {
        return Ok(None);
    }
    parse_window(text)
        .map(Some)
        .ok_or_else(|| quiet_hours_refusal(&format!("{text:?}")))
}

/// The refusal, in the shape the config layer already refuses a setting by
/// name: what was written, and what it cost.
fn quiet_hours_refusal(offender: &str) -> String {
    format!("pns: config error (hue.quiet_hours is {offender}, not a HH:MM-HH:MM window); no pulse")
}
