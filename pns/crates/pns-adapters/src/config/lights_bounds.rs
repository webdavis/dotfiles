use super::*;

/// The floor under it, and it is the TRANSPORT DEADLINE rather than a round
/// number: a tick makes bounded bridge calls whose own limit is ten seconds
/// (`BRIDGE_DEADLINE`), so an interval shorter than one call can start a tick
/// while the last one is still dialling. Below this the knob is asking for a
/// pile of children rather than a faster lamp.
pub const MIN_ARM_INTERVAL_SECS: u64 = 10;

/// And the ceiling: THE LONGEST INTERVAL A BREATHING LAMP MAY BE GIVEN.
///
/// THE DAEMON'S CHILD BOUND IS DERIVED FROM THIS NUMBER, not the other way
/// round, and that direction is deliberate. A breathing tick sleeps for almost
/// all of its interval issuing fades, so the interval is the longest part of
/// what a child has to do; `child_bound` in the daemon adds the one write that
/// can still be in flight when the interval ends and the reap tick that
/// notices the child afterwards. Reading the dependency the other way had the
/// supported thirty-second interval equal to a thirty-second child, which killed
/// a legal last write before the tick could record where it landed.
///
/// IT IS ALSO UNDER THE ORDINARY LEASE: the tick is registered with `until` at
/// least as far as its own first due second, so an interval longer than that
/// lease would EXTEND it, and the two lease lengths would stop being the fixed
/// numbers they are documented as.
///
/// THIRTY SECONDS IS NOT A NARROW LAMP EITHER. It holds eight full cycles of the
/// locked blocked shape and four of the slow one, so nothing an operator would
/// want is out of reach above it.
pub const MAX_ARM_INTERVAL_SECS: u64 = 30;

/// How long any arming delay or lease in this table may be. A day, which is the
/// bound the working streak already carried: work that has been going for
/// longer has stalled, and a delay past it describes a lamp that never lights
/// at all.
pub(super) const MIN_ARM_AFTER_SECS: u64 = 1;
pub(super) const MAX_LIGHTS_TIMING_SECS: u64 = 86_400;

/// The floor under a lease expiry. A minute, because the lease is renewed by
/// event traffic and anything shorter drops a live loop between two turns.
///
/// SHARED WITH THE BLOCKED BACKSTOP'S OWN FLOOR, which needs no separate
/// number: a minute is the same floor for the same reason, a value too small
/// to mean anything below the granularity real event traffic arrives at.
pub(super) const MIN_LEASE_EXPIRY_SECS: u64 = 60;

/// The ceiling on the blocked backstop alone. Every OTHER delay or lease in
/// this table caps at a day (`MAX_LIGHTS_TIMING_SECS`), but an abandoned wait
/// can span a weekend away, so this one gets a week instead of sharing that
/// ceiling.
pub(super) const MAX_BLOCKED_LEASE_EXPIRY_SECS: u64 = 7 * 24 * 60 * 60;

/// `[lights] arm_interval`'s range, as the duration parser takes it.
pub(super) fn arm_interval_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_ARM_INTERVAL_SECS)..=Duration::from_secs(MAX_ARM_INTERVAL_SECS)
}

/// `[lights.loop] arm_after`'s range.
pub(super) fn loop_arm_after_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_ARM_AFTER_SECS)..=Duration::from_secs(MAX_LIGHTS_TIMING_SECS)
}

/// `[lights.unseen] arm_after`'s range. ITS FLOOR IS ZERO, which means "at
/// once": the failure flavour's own behaviour spelled for the success one,
/// rather than a switch that turns anything off.
pub(super) fn unseen_arm_after_range() -> RangeInclusive<Duration> {
    Duration::ZERO..=Duration::from_secs(MAX_LIGHTS_TIMING_SECS)
}

/// `[lights.loop] lease_expiry`'s range.
pub(super) fn loop_lease_expiry_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_LEASE_EXPIRY_SECS)..=Duration::from_secs(MAX_LIGHTS_TIMING_SECS)
}

/// `[lights.blocked] lease_expiry`'s range.
pub(super) fn blocked_lease_expiry_range() -> RangeInclusive<Duration> {
    Duration::from_secs(MIN_LEASE_EXPIRY_SECS)..=Duration::from_secs(MAX_BLOCKED_LEASE_EXPIRY_SECS)
}

/// One `[lights]` duration key whose floor is a real floor.
///
/// `duration_key` CARVES `"0s"` OUT for the keys that are their own switch,
/// and none of these is one: an interval, a lease and an arming delay all
/// describe a schedule, so zero is a lamp that never runs rather than a
/// feature turned off, and it is refused by name like any other value under
/// the floor.
pub(super) fn positive_duration(
    table: &str,
    key: &str,
    setting: &toml::Value,
    range: RangeInclusive<Duration>,
) -> Result<u64, ConfigError> {
    Ok(positive_duration_value(table, key, setting, range)?.as_secs())
}

/// The same refusal for a duration whose range is finer than a second, kept
/// whole so a caller can take the milliseconds it was written in.
pub(super) fn positive_duration_value(
    table: &str,
    key: &str,
    setting: &toml::Value,
    range: RangeInclusive<Duration>,
) -> Result<Duration, ConfigError> {
    use pns_domain::duration::spelled;
    let (low, high) = (spelled(*range.start()), spelled(*range.end()));
    let stated = duration_value(table, key, setting, range)?;
    if stated.is_zero() {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` \"0s\" is outside {low} to {high}"
        )));
    }
    Ok(stated)
}

/// One fade or accent, in the milliseconds the driver issues it in.
///
/// ITS OWN RANGE IS THE ONLY ONE IN THE TABLE, so the range is not a parameter:
/// every `duration` and `flare_duration` key across the six behaviour tables
/// shares the fade bounds.
pub(super) fn fade_duration(
    table: &str,
    key: &str,
    setting: &toml::Value,
) -> Result<u64, ConfigError> {
    let stated = positive_duration_value(table, key, setting, fade_range())?;
    Ok(u64::try_from(stated.as_millis()).expect("bounded at MAX_FADE_MS, which fits a u64"))
}

/// The fade range, as the duration parser takes it.
pub(super) fn fade_range() -> RangeInclusive<Duration> {
    Duration::from_millis(MIN_FADE_MS)..=Duration::from_millis(MAX_FADE_MS)
}

/// How long ONE fade may take, in milliseconds.
///
/// THE CEILING IS WHAT MAKES THE DRIVER TOTAL. `breath_fades` needs room for
/// at least one fade inside a tick's budget, and that budget is what is LEFT
/// of `MIN_ARM_INTERVAL_SECS` after the resolve, so a fade past this ceiling could
/// be asked for a schedule the shortest interval the config allows has no
/// room left to even start.
pub(super) const MIN_FADE_MS: u64 = 200;
pub(super) const MAX_FADE_MS: u64 = 5000;

/// Percent, so the two ends are the two ends. ZERO IS REFUSED rather than read
/// as off: a dark signal is a lamp that says nothing, and the way to say
/// nothing is to leave the behaviour off that lamp's `behaviours` list.
pub(super) const MIN_BRIGHTNESS: u8 = 1;
pub(super) const MAX_BRIGHTNESS: u8 = 100;

/// The three keys every breathing shape shares, so `unseen` and `loop` read
/// them through the same arm the two plain breaths do.
pub(super) fn breath_key(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
    breath: &mut Breath,
) -> Result<(), ConfigError> {
    match key {
        "duration" => breath.duration_ms = fade_duration(where_it_is, key, stated)?,
        "high_percent" => breath.high = percent(where_it_is, key, stated)?,
        "low_percent" => breath.low = percent(where_it_is, key, stated)?,
        _ => return Err(unknown_key(where_it_is, where_it_is, key)),
    }
    Ok(())
}

/// A breath whose `low` is above its `high` is REFUSED rather than rendered
/// upside down: every fade the driver issues moves toward one of these two
/// named values, so with the ends swapped a fade to `high` would move the
/// lamp DOWN and one to `low` would move it up.
pub(super) fn ends_agree(where_it_is: &str, breath: &Breath) -> Result<(), ConfigError> {
    if breath.low > breath.high {
        return Err(ConfigError::Invalid(format!(
            "`{where_it_is}` has low_percent {} above high_percent {}, so a fade \
             to `high_percent` would move the lamp down and one to `low_percent` \
             would move it up",
            breath.low, breath.high
        )));
    }
    Ok(())
}

/// An accent that does not rise ABOVE the peak, or that is not BRIEFER than
/// the fades it sits between, is refused rather than run as something that is
/// no longer the locked motion.
///
/// THE SECOND HALF ALSO HOLDS THE SCHEDULING MARGIN. A resumed breath may start
/// as much as one leg's step into what is left of a tick's interval, so the
/// worst case a config can produce is its LONGEST leg; keeping the accent under
/// `duration` keeps that longest leg the breath's own, exactly as it was
/// before the accent existed. Without it, `flare_duration` would be a second way to
/// write a leg too slow for the interval it runs in.
pub(super) fn accent_agrees(
    where_it_is: &str,
    motion: &BreatheThenFlare,
) -> Result<(), ConfigError> {
    if motion.flare <= motion.breath.high {
        return Err(ConfigError::Invalid(format!(
            "`{where_it_is}` has flare_percent {} at or below high_percent {}, \
             so the accent would not rise above the peak it is meant to accent",
            motion.flare, motion.breath.high
        )));
    }
    if motion.flare_ms >= motion.breath.duration_ms {
        return Err(ConfigError::Invalid(format!(
            "`{where_it_is}` has flare_duration {} at or above duration {}, so \
             the accent would be a third fade of the breath rather than a flash \
             at its peak",
            motion.flare_ms, motion.breath.duration_ms
        )));
    }
    Ok(())
}

/// One behaviour's own table, refused by name when it is not a table at all.
pub(super) fn behaviour_table<'setting>(
    where_it_is: &str,
    setting: &'setting toml::Value,
) -> Result<&'setting toml::map::Map<String, toml::Value>, ConfigError> {
    setting.as_table().ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`{where_it_is}` has type `{}`, not a table of settings",
            setting.type_str()
        ))
    })
}

/// The two ends of a colour coordinate. CIE xy, which is what the bridge
/// takes and what every colour in `pulse.rs` is spelled in.
pub(super) const MIN_COORDINATE: f64 = 0.0;
pub(super) const MAX_COORDINATE: f64 = 1.0;

/// One colour, as the `[x, y]` pair a config states it with.
///
/// TWO NUMBERS AND NOTHING ELSE. A longer array, a shorter one, a string or a
/// bare number is refused by name rather than read for whatever it holds: the
/// operator wrote something that is not a coordinate, and guessing which half
/// they meant is how a lamp ends up a colour nobody picked.
pub(super) fn coordinate(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
) -> Result<pns_domain::pulse::PulseColor, ConfigError> {
    let refused = |detail: &str| {
        ConfigError::Invalid(format!(
            "`{where_it_is}` key `{key}` {detail}; a colour is an `[x, y]` pair of \
             numbers, each in the range {MIN_COORDINATE} to {MAX_COORDINATE}"
        ))
    };
    let Some([x, y]) = stated.as_array().map(Vec::as_slice) else {
        return Err(refused(&format!(
            "has type `{}`, not a pair",
            stated.type_str()
        )));
    };
    let mut pair = [0.0; 2];
    for (slot, stated) in pair.iter_mut().zip([x, y]) {
        let Some(number) = stated
            .as_float()
            .or_else(|| stated.as_integer().map(|whole| whole as f64))
        else {
            return Err(refused(&format!(
                "holds a `{}`, not a number",
                stated.type_str()
            )));
        };
        if !(MIN_COORDINATE..=MAX_COORDINATE).contains(&number) {
            return Err(refused(&format!("holds {number}, which is out of range")));
        }
        *slot = number;
    }
    Ok(pns_domain::pulse::PulseColor {
        x: pair[0],
        y: pair[1],
    })
}

/// One brightness, in percent, refused by name outside the range.
pub(super) fn percent(
    where_it_is: &str,
    key: &str,
    stated: &toml::Value,
) -> Result<u8, ConfigError> {
    let count = bounded(
        where_it_is,
        key,
        stated,
        MIN_BRIGHTNESS.into(),
        MAX_BRIGHTNESS.into(),
    )?;
    // THE BOUND ABOVE ALREADY HELD, so this cannot fail and a fallback here
    // would be a second, silent answer to a question `bounded` has already
    // refused by name.
    Ok(u8::try_from(count).expect("bounded at MAX_BRIGHTNESS, which is a percent and fits a u8"))
}
