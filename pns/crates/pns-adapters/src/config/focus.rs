use super::*;

/// `[focus]`'s one key: the Focus modes that mean it.
///
/// NO `enabled` KEY, deliberately. Naming no mode and switching the feature off
/// are the same statement, so a second way to say it is a second thing that can
/// disagree with the first: a table reading `enabled = true` with an empty
/// `silence`, or `enabled = false` with three modes listed, would each need a
/// rule nobody has stated.
///
/// AN EMPTY LIST IS NOT REFUSED, unlike `recap`'s empty `summarizer` and
/// `repos`. Those name a thing pns would then try and fail to use; this names
/// the modes that silence, and none of them is a working, readable setting that
/// says exactly what it does.
pub(super) fn parse_focus(value: toml::Value) -> Result<Vec<String>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`focus` is not a table".to_string()));
    };
    let mut silence = Vec::new();
    for (key, setting) in table {
        admits_flat("focus", &key)?;
        match key.as_str() {
            "silence" => silence = modes(&setting)?,
            _ => {
                return Err(unknown_key("focus", "focus", &key));
            }
        }
    }
    Ok(silence)
}

/// `silence`, the Focus modes that mean it: a list of display NAMES as Control
/// Center shows them, raw `modeIdentifier` strings, or a mix.
///
/// THE LIST MAY BE EMPTY AND AN ENTRY MAY NOT, which is not two rules but one
/// applied to two different statements. An empty list says "no mode silences
/// pns", which is the feature off and exactly what it reads as. An empty
/// STRING says nothing at all: no Focus mode is named by it, so it is a policy
/// the operator wrote and pns would never act on. That is precisely the state
/// the misspelled-key refusal one function up exists to prevent, and `repos`
/// refuses its own empty entry by name for the same reason.
///
/// THE NAME ITSELF IS NOT JUDGED BEYOND THAT. A name that matches no mode is
/// an ordinary thing to write (a Focus you keep on another Mac), and `pns
/// doctor` is where an operator learns whether the mode they named is the one
/// that is on.
pub(super) fn modes(setting: &toml::Value) -> Result<Vec<String>, ConfigError> {
    let names = strings("focus", "silence", "a list of Focus mode names", setting)?;
    if names.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`focus` key `silence` names a mode that is the empty string, which is no Focus at all"
                .to_string(),
        ));
    }
    Ok(names)
}
