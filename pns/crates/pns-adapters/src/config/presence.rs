use super::*;

/// The `[plugins.presence]` settings, typed.
///
/// THE ONLY BACKEND IS THE BRIDGE, so `type` is required and refused by name
/// the way `[plugins.mobile]`'s and `[plugins.home_presence]`'s are: a table naming a
/// backend nothing implements contributes no settings at all rather than
/// having its numbers read as this one's.
///
/// `Ok(None)` IS THE INERT TABLE, absent or switched off, which is the reading
/// `armed_mobile` and `enabled_hue_table` already give theirs.
pub fn parse_presence(config: &Config) -> Result<Option<Presence>, ConfigError> {
    let Some(entry) = config
        .plugins
        .get(pns_domain::registry::PRESENCE)
        .filter(|entry| entry.enabled)
    else {
        return Ok(None);
    };
    let settings = &entry.settings;
    match settings.get("type").and_then(toml::Value::as_str) {
        Some(PRESENCE_TYPE) => {}
        Some(named) => {
            return Err(ConfigError::Invalid(format!(
                "[plugins.presence] has type `{named}`, which no compiled-in backend answers; \
                 the only type is `{PRESENCE_TYPE}`"
            )));
        }
        None => {
            return Err(ConfigError::Invalid(format!(
                "no `type` in [plugins.presence]; the only type is `{PRESENCE_TYPE}`"
            )));
        }
    }

    let rooms = match settings.get("rooms") {
        Some(setting) => strings("presence", "rooms", "a list of room names", setting)?,
        None => Vec::new(),
    };
    // A ROOM THE STATE FILE CANNOT CARRY IS A CONFIGURATION ERROR, read with
    // the file's own predicate so the two cannot drift. `render` refuses such a
    // name, which is correct and silent: the poll publishes nothing, the daemon
    // re-arms at its normal interval and ignores the exit status, and the
    // doctor can only report a reading that is stale or absent. Said here, the
    // doctor's configuration-error line names the room instead.
    //
    // ONLY `rooms`, deliberately: an `excluded_rooms` entry is compared and
    // never published, so a name this would refuse costs nothing and refusing
    // it would turn a working config into a refused one.
    if let Some(room) = rooms.iter().find(|room| !room_fits(room)) {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `rooms` names {room:?}, which the presence state file cannot carry: \
             a room is 1 to {} characters and holds no control characters",
            ROOM_MAX
        )));
    }
    let excluded_rooms = match settings.get("excluded_rooms") {
        Some(setting) => strings(
            "presence",
            "excluded_rooms",
            "a list of room names",
            setting,
        )?,
        None => Vec::new(),
    };
    // AN EMPTY NAME MATCHES NO BRIDGE ROOM, so it can only ever be a typo.
    //
    // `excluded_rooms` ALONE, because `room_fits` above already refuses an
    // empty entry in `rooms` and is strictly stronger there. `excluded_rooms`
    // is deliberately not held to that bar, for the reason stated above it, so
    // the empty case is still worth saying here: an exclusion that names
    // nothing excludes nothing, silently and for good.
    if excluded_rooms.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`presence` key `excluded_rooms` has an empty room name in it; no bridge \
             room answers to an empty name"
                .to_string(),
        ));
    }
    // REFUSED BY NAME RATHER THAN DROPPED, because a desk in a room no reading
    // can ever name narrows the lamps to a room the poll never watches, and it
    // would do it silently and for good.
    let desk_room = match settings.get("desk_room") {
        Some(setting) => {
            let Some(named) = setting.as_str() else {
                return Err(ConfigError::Invalid(format!(
                    "`presence` key `desk_room` has type `{}`, not a room name",
                    setting.type_str()
                )));
            };
            if named.is_empty() {
                return Err(ConfigError::Invalid(
                    "`presence` key `desk_room` is empty; no bridge room answers to an \
                     empty name"
                        .to_string(),
                ));
            }
            if !rooms.iter().any(|room| room == named) {
                return Err(ConfigError::Invalid(format!(
                    "`presence` key `desk_room` is `{named}`, which is not in `rooms`; \
                     the desk's room has to be one this config watches"
                )));
            }
            // THE TWO KEYS WOULD CONTRADICT EACH OTHER. `excluded_rooms` says
            // never light that room and `desk_room` says light it whenever the
            // desk is warm; loaded, the desk branch wins and the exclusion is
            // silently a lie.
            if excluded_rooms.iter().any(|room| room == named) {
                return Err(ConfigError::Invalid(format!(
                    "`presence` key `desk_room` is `{named}`, which `excluded_rooms` also \
                     names; a room the config never wants lit cannot be the one the desk \
                     lights"
                )));
            }
            Some(named.to_string())
        }
        None => None,
    };
    // ZERO IS THE DESK OVERRIDE OFF BY ACCIDENT: no desk reading can ever be
    // fresher than a bound of no seconds at all, so the desk would never speak
    // for where the operator is and the key would look set. It is said here
    // because `"0s"` is carved out of every duration range in this file.
    //
    // THE HOUR CEILING IS THE RANGE ITSELF. The key is a tuning knob for how
    // long a keystroke keeps speaking for where a body is; stretched past an
    // operational maximum it stops tuning anything and parks the lamps in
    // `desk_room` for good, because no reading can ever be fresher than a desk
    // that never goes stale.
    let desk_input_max_age_secs = presence_duration(
        settings,
        "desk_input_max_age",
        DEFAULT_DESK_INPUT_MAX_AGE_SECS,
        desk_input_max_age_range(),
    )?;
    if desk_input_max_age_secs == 0 {
        return Err(ConfigError::Invalid(
            "`presence` key `desk_input_max_age` is 0, which is the desk override \
             switched off by accident: no desk reading is fresher than no seconds at all"
                .to_string(),
        ));
    }
    let poll_interval_secs = presence_duration(
        settings,
        "poll_interval",
        DEFAULT_POLL_INTERVAL_SECS,
        poll_interval_range(),
    )?;
    if poll_interval_secs < MIN_POLL_INTERVAL_SECS {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `poll_interval` is {poll_interval_secs}, under the \
             {MIN_POLL_INTERVAL_SECS}-second floor the bridge is read at"
        )));
    }
    let reading_max_age_secs = presence_duration(
        settings,
        "reading_max_age",
        DEFAULT_READING_MAX_AGE_SECS,
        reading_max_age_range(),
    )?;
    // A BOUND UNDER THE INTERVAL IS THE FEATURE OFF BY ACCIDENT: every reading
    // would age past it before the next poll could refresh it, so the sensor
    // would answer Unknown for good and say nothing about why.
    if reading_max_age_secs < poll_interval_secs {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `reading_max_age` is {reading_max_age_secs}, under the \
             {poll_interval_secs}-second `poll_interval`, so every reading would be \
             stale before the next poll replaced it"
        )));
    }
    Ok(Some(Presence {
        rooms,
        excluded_rooms,
        desk_room,
        desk_input_max_age_secs,
        poll_interval_secs,
        reading_max_age_secs,
    }))
}

/// One duration off `[plugins.presence]`, in whole seconds, or the default
/// when the table states none. NAMED FOR ITS TABLE, because the refusal it
/// writes names that table too: a second caller under a generic name would
/// report its own key under `presence`.
pub(super) fn presence_duration(
    settings: &toml::Table,
    key: &str,
    default_secs: u64,
    range: RangeInclusive<Duration>,
) -> Result<u64, ConfigError> {
    let Some(stated) = settings.get(key) else {
        return Ok(default_secs);
    };
    duration_key("presence", key, stated, range)
}
