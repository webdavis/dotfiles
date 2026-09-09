use super::*;

/// `[lights]`, the lamp policy: one interval, five behaviour shapes and three
/// levels of routing, each starting at its default and moved only by a key that
/// states it.
///
/// EVERY UNKNOWN KEY IS REFUSED BY NAME, and that refusal is the whole argument
/// for parsing this table here rather than inside the hue plugin. A plugin's
/// settings are free-form at this layer, so a mistyped key there is silently
/// ignored; the failure that costs is a lamp that never lights and a config
/// that looks right, which an operator standing in a dark room cannot falsify.
pub(super) fn parse_lights(value: toml::Value) -> Result<Lights, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid("`lights` is not a table".to_string()));
    };
    let mut lights = Lights::default();
    for (key, setting) in table {
        admits_flat("lights", &key)?;
        match key.as_str() {
            "refresh_secs" => {
                lights.refresh_secs =
                    bounded("lights", &key, &setting, MIN_REFRESH_SECS, MAX_REFRESH_SECS)?;
            }
            "done" => lights.done = parse_pulse("lights.done", &setting, lights.done)?,
            "failed" => lights.failed = parse_pulse("lights.failed", &setting, lights.failed)?,
            "blocked" => {
                lights.blocked = parse_blocked(&setting, lights.blocked)?;
            }
            "dim" => lights.dim = parse_breath("lights.dim", &setting, lights.dim)?,
            "unread" => lights.unread = parse_unread(&setting, lights.unread)?,
            "loop" => lights.looping = parse_looping(&setting, lights.looping)?,
            "lamp" => lights.lamps = parse_targets("lamp", &setting)?,
            "room" => lights.rooms = parse_targets("room", &setting)?,
            "zone" => lights.zones = parse_targets("zone", &setting)?,
            _ => {
                return Err(unknown_key("lights", "lights", &key));
            }
        }
    }
    Ok(lights)
}

/// The keys a behaviour's own table serves, read into whichever of the shapes
/// that behaviour has.
///
/// THE DEFAULT ARRIVES AS A VALUE rather than being rebuilt here, so a table
/// that states one key moves that one and leaves the rest where the locked
/// figures put them.
pub(super) fn parse_pulse(
    where_it_is: &str,
    setting: &toml::Value,
    mut pulse: Pulse,
) -> Result<Pulse, ConfigError> {
    for (key, stated) in behaviour_table(where_it_is, setting)? {
        admits_flat(where_it_is, key)?;
        match key.as_str() {
            "duration_ms" => {
                pulse.duration_ms = bounded(where_it_is, key, stated, MIN_FADE_MS, MAX_FADE_MS)?;
            }
            "brightness" => pulse.brightness = percent(where_it_is, key, stated)?,
            _ => return Err(unknown_key(where_it_is, where_it_is, key)),
        }
    }
    Ok(pulse)
}

pub(super) fn parse_breath(
    where_it_is: &str,
    setting: &toml::Value,
    mut breath: Breath,
) -> Result<Breath, ConfigError> {
    for (key, stated) in behaviour_table(where_it_is, setting)? {
        admits_flat(where_it_is, key)?;
        breath_key(where_it_is, key, stated, &mut breath)?;
    }
    ends_agree(where_it_is, &breath)?;
    Ok(breath)
}

pub(super) fn parse_blocked(
    setting: &toml::Value,
    mut blocked: Blocked,
) -> Result<Blocked, ConfigError> {
    const WHERE: &str = "lights.blocked";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        if key == "give_up_after_secs" {
            blocked.give_up_after_secs = bounded(
                WHERE,
                key,
                stated,
                MIN_LEASE_TIMEOUT_SECS,
                MAX_GIVE_UP_AFTER_SECS,
            )?;
            continue;
        }
        breath_key(WHERE, key, stated, &mut blocked.breath)?;
    }
    ends_agree(WHERE, &blocked.breath)?;
    Ok(blocked)
}

pub(super) fn parse_unread(
    setting: &toml::Value,
    mut unread: Unread,
) -> Result<Unread, ConfigError> {
    const WHERE: &str = "lights.unread";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        if key == "after_secs" {
            // ZERO IS ALLOWED AND MEANS "AT ONCE", which is the failure
            // flavour's own behaviour spelled for the success one. It is not a
            // switch that turns anything off, so it needs no floor.
            unread.after_secs = bounded(WHERE, key, stated, 0, MAX_THRESHOLD_SECS)?;
            continue;
        }
        breath_key(WHERE, key, stated, &mut unread.breath)?;
    }
    ends_agree(WHERE, &unread.breath)?;
    Ok(unread)
}

pub(super) fn parse_looping(
    setting: &toml::Value,
    mut looping: Looping,
) -> Result<Looping, ConfigError> {
    const WHERE: &str = "lights.loop";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        match key.as_str() {
            "threshold_secs" => {
                looping.threshold_secs =
                    bounded(WHERE, key, stated, MIN_THRESHOLD_SECS, MAX_THRESHOLD_SECS)?;
            }
            "lease_timeout_secs" => {
                looping.lease_timeout_secs = bounded(
                    WHERE,
                    key,
                    stated,
                    MIN_LEASE_TIMEOUT_SECS,
                    MAX_THRESHOLD_SECS,
                )?;
            }
            "flare" => {
                looping.breathe_then_flare.flare = percent(WHERE, key, stated)?;
            }
            "flare_ms" => {
                looping.breathe_then_flare.flare_ms =
                    bounded(WHERE, key, stated, MIN_FADE_MS, MAX_FADE_MS)?;
            }
            _ => breath_key(WHERE, key, stated, &mut looping.breathe_then_flare.breath)?,
        }
    }
    ends_agree(WHERE, &looping.breathe_then_flare.breath)?;
    accent_agrees(WHERE, &looping.breathe_then_flare)?;
    Ok(looping)
}
