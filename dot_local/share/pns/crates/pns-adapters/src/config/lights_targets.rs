use super::*;

/// The five words, in the spelling a config uses, and the order the refusal
/// lists them in.
pub const BEHAVIOUR_WORDS: [(&str, Behaviour); 5] = [
    ("done", Behaviour::Done),
    ("failed", Behaviour::Failed),
    ("blocked", Behaviour::Blocked),
    ("unread", Behaviour::Unread),
    ("loop", Behaviour::Looping),
];

/// `[lights.lamp]`, `[lights.room]` and `[lights.zone]`: one table per declared
/// name, at one of the three levels.
///
/// A NAME IS NOT JUDGED against the bridge here. This layer reads a file, and
/// only the bridge's own listings can say which lamps, rooms and zones exist;
/// an unresolvable name is reported by the tick and by `pns doctor` in their
/// own words, once there is a listing to judge it against.
pub(super) fn parse_targets(
    level: &str,
    setting: &toml::Value,
) -> Result<BTreeMap<String, Target>, ConfigError> {
    let Some(table) = setting.as_table() else {
        return Err(ConfigError::Invalid(format!(
            "`lights` key `{level}` has type `{}`, not a table of {level} names",
            setting.type_str()
        )));
    };
    let mut targets = BTreeMap::new();
    for (name, entry) in table {
        let where_it_is = format!("lights.{level}.{name}");
        let Some(settings) = entry.as_table() else {
            return Err(ConfigError::Invalid(format!(
                "`{where_it_is}` has type `{}`, not a table of settings",
                entry.type_str()
            )));
        };
        let mut target = Target::default();
        let mut states_behaviours = false;
        for (key, stated) in settings {
            admits(TARGET_KEYS, &where_it_is, key)?;
            match key.as_str() {
                "shows" => target.shows = Some(behaviours(&where_it_is, key, stated)?),
                "dim_window" => target.dim_window = Some(text(&where_it_is, key, stated)?),
                "dim_behaviours" => {
                    target.dim_behaviours = behaviours(&where_it_is, key, stated)?;
                    states_behaviours = true;
                }
                _ => {
                    return Err(unknown_key(TARGET_KEYS, &where_it_is, key));
                }
            }
        }
        // NO DEAD KNOBS, which is the config ruling reaching the one pair of
        // keys that can be half written. The enables RIDE the window (they are
        // resolved as one answer), so a declaration that names which behaviours
        // run dimmed and never says WHEN is a list nothing reads: the operator
        // gets a lamp that strobes all night and a file that says it should
        // not. STATED rather than non-empty, because an empty list with no
        // window is the same dead knob and the two must not disagree.
        if states_behaviours && target.dim_window.is_none() {
            return Err(ConfigError::Invalid(format!(
                "`{where_it_is}` states `dim_behaviours` with no `dim_window` for \
                 them to run in, so nothing would ever read them"
            )));
        }
        targets.insert(name.clone(), target);
    }
    Ok(targets)
}

/// A list of behaviour words off the closed set, refused BY NAME.
///
/// THE REFUSAL LISTS THE WHOLE SET, which is worth the extra words here and
/// nowhere else in this file: the failure it prevents is a lamp that stays dark
/// while the operator is sure they routed it, and their only evidence is a lamp
/// doing nothing.
pub(super) fn behaviours(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
) -> Result<Vec<Behaviour>, ConfigError> {
    let words = strings(where_it_is, key, "a list of behaviour names", stated)?;
    words
        .iter()
        .map(|word| {
            BEHAVIOUR_WORDS
                .iter()
                .find(|(spelling, _)| spelling == word)
                .map(|(_, behaviour)| *behaviour)
                .ok_or_else(|| {
                    let known: Vec<&str> = BEHAVIOUR_WORDS.iter().map(|(word, _)| *word).collect();
                    ConfigError::Invalid(format!(
                        "`{where_it_is}` key `{key}` names `{word}`, which is no behaviour; \
                         the lamps say {}",
                        known.join(", ")
                    ))
                })
        })
        .collect()
}
