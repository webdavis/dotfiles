use super::*;

/// What `[focus]` carries: the switch, and the Focus modes that mean it.
pub(super) struct FocusTable {
    pub enabled: bool,
    pub modes: Vec<String>,
}

/// See `Config::focus_enabled`.
pub(super) const DEFAULT_FOCUS_ENABLED: bool = true;

/// `[focus]`'s two keys, in `parse_daemon`'s shape: the roster of modes and
/// the switch that decides whether any of them is read, each refused BY NAME
/// when the key is unknown or the value is the wrong shape.
///
/// THE ROSTER IS NOT THE SWITCH. `modes` names the Focus modes that silence
/// and `enabled` says whether pns reads Focus at all, so an operator who
/// wants their list kept and the feature off for a week writes one word
/// rather than commenting the list out and losing it.
///
/// AN EMPTY LIST IS NOT REFUSED, unlike `recap`'s empty `summarizer` and
/// `repositories`. Those name a thing pns would then try and fail to use;
/// this names the modes that silence, and nothing silences until one is
/// named.
pub(super) fn parse_focus(value: toml::Value) -> Result<FocusTable, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`focus` is not a table".to_string()));
    };
    let mut focus = FocusTable {
        enabled: DEFAULT_FOCUS_ENABLED,
        modes: Vec::new(),
    };
    for (key, setting) in table {
        admits_flat("focus", &key)?;
        match key.as_str() {
            "enabled" => {
                focus.enabled = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`focus` key `enabled` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "modes" => focus.modes = modes(&setting)?,
            _ => {
                return Err(unknown_key("focus", "focus", &key));
            }
        }
    }
    Ok(focus)
}

/// `modes`, the Focus modes that silence: a list of display NAMES as Control
/// Center shows them, raw `modeIdentifier` strings, or a mix.
///
/// THE LIST MAY BE EMPTY AND AN ENTRY MAY NOT, which is not two rules but one
/// applied to two different statements. An empty list says "no mode silences
/// pns", which is the roster empty and exactly what it reads as. An empty
/// STRING says nothing at all: no Focus mode is named by it, so it is a policy
/// the operator wrote and pns would never act on. That is precisely the state
/// the misspelled-key refusal one function up exists to prevent, and `repositories`
/// refuses its own empty entry by name for the same reason.
///
/// THE NAME ITSELF IS NOT JUDGED BEYOND THAT. A name that matches no mode is
/// an ordinary thing to write (a Focus you keep on another Mac), and `pns
/// doctor` is where an operator learns whether the mode they named is the one
/// that is on.
pub(super) fn modes(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let names = strings("focus", "modes", "a list of Focus mode names", setting)?;
    if names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`focus` key `modes` names a mode that is the empty string, which is no Focus at all"
                .to_string(),
        ));
    }
    Ok(names)
}
