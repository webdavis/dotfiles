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
            "arm_interval" => {
                lights.arm_interval_secs =
                    positive_duration("lights", &key, &setting, arm_interval_range())?;
            }
            "done" => lights.done = parse_pulse("lights.done", &setting, lights.done)?,
            "failed" => lights.failed = parse_pulse("lights.failed", &setting, lights.failed)?,
            "blocked" => {
                lights.blocked = parse_blocked(&setting, lights.blocked)?;
            }
            "dim" => lights.dim = parse_breath("lights.dim", &setting, lights.dim)?,
            "checks" => lights.checks = parse_checks(&setting, lights.checks)?,
            "unseen" => lights.unseen = parse_unseen(&setting, lights.unseen)?,
            "loop" => lights.looping = parse_looping(&setting, lights.looping)?,
            "lamp" => lights.lamps = parse_targets("lamp", &setting)?,
            "room" => lights.rooms = parse_targets("room", &setting)?,
            "zone" => lights.zones = parse_targets("zone", &setting)?,
            "dim_window" => lights.dim_window = Some(text("lights", &key, &setting)?),
            _ => {
                return Err(unknown_key("lights", "lights", &key));
            }
        }
    }
    dim_behaviours_have_a_window(&lights)?;
    Ok(lights)
}

/// NO DEAD KNOBS, which is the config ruling reaching the one pair of keys that
/// can be half written. The enables RIDE a window (they are resolved as one
/// answer), so a declaration that names which behaviours run dimmed with no
/// window anywhere for them to run in is a list nothing reads: the operator
/// gets a lamp that strobes all night and a file that says it should not.
///
/// READ HERE RATHER THAN IN `parse_targets` because the window a declaration
/// may be leaning on is `[lights] dim_window`, and only the whole table in hand
/// can say whether one was written.
fn dim_behaviours_have_a_window(lights: &Lights) -> Result<(), ConfigError> {
    if lights.dim_window.is_some() {
        return Ok(());
    }
    for (level, targets) in [
        ("lamp", &lights.lamps),
        ("room", &lights.rooms),
        ("zone", &lights.zones),
    ] {
        for (name, target) in targets {
            // STATED rather than non-empty, because an empty list with no
            // window is the same dead knob and the two must not disagree.
            if target.dim_behaviours.is_some() && target.dim_window.is_none() {
                return Err(ConfigError::Invalid(format!(
                    "`lights.{level}.{name}` states `dim_behaviours` with no \
                     `dim_window` of its own and no `lights` key `dim_window` \
                     for them to run in, so nothing would ever read them"
                )));
            }
        }
    }
    Ok(())
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
            "duration" => pulse.duration_ms = fade_duration(where_it_is, key, stated)?,
            "brightness_percent" => pulse.brightness = percent(where_it_is, key, stated)?,
            _ => return Err(unknown_key(where_it_is, where_it_is, key)),
        }
    }
    Ok(pulse)
}

/// `[lights.checks]`: the one blink that carries its own two COLOURS.
///
/// THE PAIR IS PARSED LIKE ANY OTHER KEY and refused by name, rather than
/// being read as a bare array and validated later: a coordinate outside the
/// unit square is not a colour the bridge can be asked for, and a lamp armed
/// with one would either clamp somewhere nobody chose or not light at all.
pub(super) fn parse_checks(
    setting: &toml::Value,
    mut checks: Checks,
) -> Result<Checks, ConfigError> {
    const WHERE: &str = "lights.checks";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        match key.as_str() {
            "duration" => checks.pulse.duration_ms = fade_duration(WHERE, key, stated)?,
            "brightness_percent" => checks.pulse.brightness = percent(WHERE, key, stated)?,
            "pass_color" => checks.pass_color = coordinate(WHERE, key, stated)?,
            "fail_color" => checks.fail_color = coordinate(WHERE, key, stated)?,
            _ => return Err(unknown_key(WHERE, WHERE, key)),
        }
    }
    Ok(checks)
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
        if key == "lease_expiry" {
            blocked.lease_expiry_secs =
                positive_duration(WHERE, key, stated, blocked_lease_expiry_range())?;
            continue;
        }
        breath_key(WHERE, key, stated, &mut blocked.breath)?;
    }
    ends_agree(WHERE, &blocked.breath)?;
    Ok(blocked)
}

pub(super) fn parse_unseen(
    setting: &toml::Value,
    mut unseen: Unseen,
) -> Result<Unseen, ConfigError> {
    const WHERE: &str = "lights.unseen";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        if key == "arm_after" {
            // ZERO IS ALLOWED AND MEANS "AT ONCE", which is the failure
            // flavour's own behaviour spelled for the success one. It is not a
            // switch that turns anything off, so it needs no floor.
            unseen.arm_after_secs = duration_key(WHERE, key, stated, unseen_arm_after_range())?;
            continue;
        }
        breath_key(WHERE, key, stated, &mut unseen.breath)?;
    }
    ends_agree(WHERE, &unseen.breath)?;
    Ok(unseen)
}

pub(super) fn parse_looping(
    setting: &toml::Value,
    mut looping: Looping,
) -> Result<Looping, ConfigError> {
    const WHERE: &str = "lights.loop";
    for (key, stated) in behaviour_table(WHERE, setting)? {
        admits_flat(WHERE, key)?;
        match key.as_str() {
            "arm_after" => {
                looping.arm_after_secs =
                    positive_duration(WHERE, key, stated, loop_arm_after_range())?;
            }
            "lease_expiry" => {
                looping.lease_expiry_secs =
                    positive_duration(WHERE, key, stated, loop_lease_expiry_range())?;
            }
            "flare_percent" => {
                looping.breathe_then_flare.flare = percent(WHERE, key, stated)?;
            }
            "flare_duration" => {
                looping.breathe_then_flare.flare_ms = fade_duration(WHERE, key, stated)?;
            }
            _ => breath_key(WHERE, key, stated, &mut looping.breathe_then_flare.breath)?,
        }
    }
    ends_agree(WHERE, &looping.breathe_then_flare.breath)?;
    accent_agrees(WHERE, &looping.breathe_then_flare)?;
    Ok(looping)
}
