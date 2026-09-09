use super::*;

/// The `[plugins.presence]` settings, typed.
///
/// THE ONLY BACKEND IS THE BRIDGE, so `type` is required and refused by name
/// the way `[plugins.mobile]`'s and `[plugins.router]`'s are: a table naming a
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
    // ONLY `rooms`, deliberately: an `exclude` entry is compared and never
    // published, so a name this would refuse costs nothing and refusing it
    // would turn a working config into a refused one.
    if let Some(room) = rooms.iter().find(|room| !room_fits(room)) {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `rooms` names {room:?}, which the presence state file cannot carry: \
             a room is 1 to {} characters and holds no control characters",
            ROOM_MAX
        )));
    }
    let exclude = match settings.get("exclude") {
        Some(setting) => strings("presence", "exclude", "a list of room names", setting)?,
        None => Vec::new(),
    };
    // AN EMPTY NAME MATCHES NO BRIDGE ROOM, so it can only ever be a typo.
    //
    // `exclude` ALONE, because `room_fits` above already refuses an empty
    // entry in `rooms` and is strictly stronger there. `exclude` is
    // deliberately not held to that bar, for the reason stated above it, so
    // the empty case is still worth saying here: an exclusion that names
    // nothing excludes nothing, silently and for good.
    if exclude.iter().any(String::is_empty) {
        return Err(ConfigError::Invalid(
            "`presence` key `exclude` has an empty room name in it; no bridge room \
             answers to an empty name"
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
            // THE TWO KEYS WOULD CONTRADICT EACH OTHER. `exclude` says never
            // light that room and `desk_room` says light it whenever the desk
            // is warm; loaded, the desk branch wins and the exclusion is
            // silently a lie.
            if exclude.iter().any(|room| room == named) {
                return Err(ConfigError::Invalid(format!(
                    "`presence` key `desk_room` is `{named}`, which `exclude` also names; \
                     a room the config never wants lit cannot be the one the desk lights"
                )));
            }
            Some(named.to_string())
        }
        None => None,
    };
    // ZERO IS THE DESK OVERRIDE OFF BY ACCIDENT: no desk reading can ever be
    // fresher than a bound of no seconds at all, so the desk would never speak
    // for where the operator is and the key would look set.
    let desk_stale_after_secs = presence_count(
        settings,
        "desk_stale_after_secs",
        DEFAULT_DESK_STALE_AFTER_SECS,
    )?;
    if desk_stale_after_secs == 0 {
        return Err(ConfigError::Invalid(
            "`presence` key `desk_stale_after_secs` is 0, which is the desk override \
             switched off by accident: no desk reading is fresher than no seconds at all"
                .to_string(),
        ));
    }
    // AND A BOUND OF HOURS IS THE DESK PINNED ON. The key is a tuning knob for
    // how long a keystroke keeps speaking for where a body is; stretched past
    // an operational maximum it stops tuning anything and parks the lamps in
    // `desk_room` for good, because no reading can ever be fresher than a desk
    // that never goes stale. Unbounded, one mistyped digit, a pasted
    // millisecond count or an `i64::MAX` did exactly that, silently and for
    // years.
    if desk_stale_after_secs > MAX_DESK_STALE_AFTER_SECS {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `desk_stale_after_secs` is {desk_stale_after_secs}, over the \
             {MAX_DESK_STALE_AFTER_SECS}-second maximum; a keyboard nobody has touched for \
             longer than that says nothing about which room they are standing in"
        )));
    }
    let poll_secs = presence_count(settings, "poll_secs", DEFAULT_PRESENCE_POLL_SECS)?;
    if !(MIN_PRESENCE_POLL_SECS..=MAX_PRESENCE_POLL_SECS).contains(&poll_secs) {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `poll_secs` is {poll_secs}, outside \
             {MIN_PRESENCE_POLL_SECS}..{MAX_PRESENCE_POLL_SECS}"
        )));
    }
    let stale_after_secs = presence_count(
        settings,
        "stale_after_secs",
        DEFAULT_PRESENCE_STALE_AFTER_SECS,
    )?;
    // A BOUND UNDER THE INTERVAL IS THE FEATURE OFF BY ACCIDENT: every reading
    // would age past it before the next poll could refresh it, so the sensor
    // would answer Unknown for good and say nothing about why.
    if stale_after_secs < poll_secs {
        return Err(ConfigError::Invalid(format!(
            "`presence` key `stale_after_secs` is {stale_after_secs}, under the \
             {poll_secs}-second `poll_secs`, so every reading would be stale before \
             the next poll replaced it"
        )));
    }
    Ok(Some(Presence {
        rooms,
        exclude,
        desk_room,
        desk_stale_after_secs,
        poll_secs,
        stale_after_secs,
    }))
}

/// One whole-second count off `[plugins.presence]`, or the default when the
/// table states none. NAMED FOR ITS TABLE, because the refusal it writes names
/// that table too: a second caller under a generic name would report its own
/// key under `presence`.
pub(super) fn presence_count(
    settings: &toml::Table,
    key: &str,
    default: u64,
) -> Result<u64, ConfigError> {
    let Some(stated) = settings.get(key) else {
        return Ok(default);
    };
    stated
        .as_integer()
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| {
            ConfigError::Invalid(format!(
                "`presence` key `{key}` has type `{}`, not a count of seconds",
                stated.type_str()
            ))
        })
}
