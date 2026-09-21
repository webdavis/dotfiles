use super::*;

/// `minimum_events`, the volume threshold. A negative or fractional value is
/// refused BY NAME rather than clamped: the operator asked for a threshold, and
/// a silently corrected one is a threshold they believe they set.
pub(super) fn threshold(setting: &toml::Value) -> Result<usize, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `minimum_events` has type `{}`, not a count",
            setting.type_str()
        )));
    };
    // AND ZERO IS REFUSED BY NAME TOO, for the same reason a negative is.
    // `counted.len() >= 0` is always true, so a zero threshold recaps EVERY
    // event, including one over an empty window, which is the state
    // `recap::NOTHING_HAPPENED` says the event path never posts. An operator
    // calibrating the knob downward gets a card and a Discord recap on every
    // event, each saying nothing was recorded. One is the floor: it means "any
    // activity at all".
    if count == 0 {
        return Err(ConfigError::Invalid(
            "`recap` key `minimum_events` is 0, which is not a threshold; 1 is the floor"
                .to_string(),
        ));
    }
    Ok(count)
}

/// One of the four periods: a two-element list of `HH:MM` times, start then
/// end.
///
/// A PAIR RATHER THAN TWO KEYS, because a window is one fact. Two keys could
/// be half-written, which is a window the operator believes they moved.
///
/// THE TIMES ARE JUDGED HERE, not at the render: `24:00` and `7:5` are both
/// refused by name, because a window silently normalized is a window nobody
/// asked for.
pub(super) fn period(key: &str, setting: &toml::Value) -> Result<Period, ConfigError> {
    const TABLE: &str = "recap";
    let written = strings(TABLE, key, "a start and an end time", setting)?;
    let [start, end] = written.as_slice() else {
        return Err(ConfigError::Invalid(format!(
            "`{TABLE}` key `{key}` has {} times, not a start and an end",
            written.len()
        )));
    };
    Ok(Period {
        start: minute_of_day(key, start)?,
        end: minute_of_day(key, end)?,
    })
}

/// One `HH:MM` as minutes since midnight, in the widths a clock reads.
fn minute_of_day(key: &str, written: &str) -> Result<u32, ConfigError> {
    let refuse = || {
        ConfigError::Invalid(format!(
            "`recap` key `{key}` has time `{written}`, which is not an `HH:MM` time of day"
        ))
    };
    let (hour, minute) = written.split_once(':').ok_or_else(refuse)?;
    let field = |text: &str, ceiling: u32| -> Result<u32, ConfigError> {
        let plain = text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_digit());
        plain
            .then(|| text.parse::<u32>().ok())
            .flatten()
            .filter(|value| *value < ceiling)
            .ok_or_else(refuse)
    };
    Ok(field(hour, 24)? * 60 + field(minute, 60)?)
}

/// `week_starts_on`, one of the two days a week is counted from.
pub(super) fn week_start(setting: &toml::Value) -> Result<WeekStart, ConfigError> {
    setting.as_str().and_then(WeekStart::parse).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "`recap` key `week_starts_on` is `{}`, which is neither `monday` nor `sunday`",
            setting.as_str().unwrap_or(setting.type_str())
        ))
    })
}

/// `rows_per_section`, how many rows a list section prints before its
/// remainder line. Zero is refused by name for `minimum_events`' reason: a
/// section of nothing but a remainder is a section that says nothing.
pub(super) fn rows_per_section(setting: &toml::Value) -> Result<usize, ConfigError> {
    let Some(count) = setting
        .as_integer()
        .and_then(|count| usize::try_from(count).ok())
        .filter(|count| *count > 0)
    else {
        return Err(ConfigError::Invalid(format!(
            "`recap` key `rows_per_section` has type `{}`, not a count of one or more",
            setting.type_str()
        )));
    };
    Ok(count)
}
