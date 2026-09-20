/// The window bounds off argv, or None for anything this will not vouch for.
///
/// EVERY UNKNOWN WORD IS A REFUSAL, never a silent default: a recap over a
/// window nobody asked for is worse than none. A bound is a local date, a
/// local date-time, a duration ago, or a plain epoch count through the crate's
/// one numeric gate, and a window that runs backwards is refused rather than
/// read as empty.
///
/// THE EPOCH PAIR IS ALL OR NOTHING, as it always was: a machine that spawns
/// this writes both flags. Only the operator's own spellings read a missing
/// `--until` as now.
///
/// THE ZONE IS THE CALLER'S, handed in as `local_epoch`, because which second
/// a local date starts at is a system fact and this crate asks nothing of the
/// system.
pub fn recap_bounds(
    arguments: &[String],
    now: u64,
    local_epoch: impl Fn(LocalCivilTime) -> Option<u64>,
) -> Option<(u64, u64)> {
    let mut since = None;
    let mut until = None;
    let mut tokens = arguments.iter();
    while let Some(token) = tokens.next() {
        let (bound, epoch) = match token.as_str() {
            "--since-epoch" => (&mut since, true),
            "--until-epoch" => (&mut until, true),
            "--since" => (&mut since, false),
            "--until" => (&mut until, false),
            _ => return None,
        };
        // A REPEATED FLAG IS A REFUSAL TOO, and so is the same bound written
        // both ways: two windows were asked for and only one can be answered.
        if bound.is_some() {
            return None;
        }
        let written = tokens.next()?;
        let moment = match epoch {
            true => pns_domain::count::parse_count(written)?,
            false => moment(written, now, &local_epoch)?,
        };
        *bound = Some((moment, epoch));
    }
    let (since, since_epoch) = since?;
    let until = match until {
        Some((until, _)) => until,
        None if since_epoch => return None,
        None => now,
    };
    (since <= until).then_some((since, until))
}

/// A local date, a local date-time or a duration ago, as an epoch second.
fn moment(
    written: &str,
    now: u64,
    local_epoch: &impl Fn(LocalCivilTime) -> Option<u64>,
) -> Option<u64> {
    match civil_time(written) {
        Some(civil) => local_epoch(civil),
        None => now.checked_sub(duration_ago(written)?.as_secs()),
    }
}

/// `YYYY-MM-DD`, or `YYYY-MM-DDTHH:MM` with the seconds optional, in the
/// widths and ranges a calendar allows and nothing else. A date with no time
/// of day is that day's local midnight.
fn civil_time(written: &str) -> Option<LocalCivilTime> {
    let (date, time) = written.split_once('T').unwrap_or((written, "00:00"));
    let mut date = date.split('-');
    let mut time = time.split(':');
    let civil = LocalCivilTime {
        year: field(date.next()?, 4, 1970..=9999)?,
        month: field(date.next()?, 2, 1..=12)?,
        day: field(date.next()?, 2, 1..=31)?,
        hour: field(time.next()?, 2, 0..=23)?,
        minute: field(time.next()?, 2, 0..=59)?,
        second: time.next().map_or(Some(0), |text| field(text, 2, 0..=59))?,
    };
    (date.next().is_none() && time.next().is_none()).then_some(civil)
}

/// One zero-padded field of a written date, inside the range its position
/// allows.
fn field(written: &str, width: usize, range: std::ops::RangeInclusive<u32>) -> Option<u32> {
    let plain = written.len() == width && written.bytes().all(|byte| byte.is_ascii_digit());
    plain
        .then(|| written.parse().ok())
        .flatten()
        .filter(|value| range.contains(value))
}

/// How long ago a bound sits, through the crate's ONE duration parser.
///
/// `d` IS SPELLED AS HOURS rather than taught to that parser, because a day is
/// a window word and the parser serves config fields whose refusals and ranges
/// are written in `ms|s|m|h`.
fn duration_ago(written: &str) -> Option<std::time::Duration> {
    let written = match written.strip_suffix('d') {
        Some(days) => format!(
            "{}h",
            pns_domain::count::parse_count(days)?.checked_mul(24)?
        ),
        None => written.to_string(),
    };
    pns_domain::duration::parse_duration(
        "recap window",
        &written,
        std::time::Duration::ZERO..=MAX_AGO,
    )
    .ok()
}

/// A moment on the operator's own calendar, before any zone is applied to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalCivilTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The furthest back a duration ago may reach: a year, which is past every
/// window the activity store is pruned to hold.
const MAX_AGO: std::time::Duration = std::time::Duration::from_secs(366 * 24 * 3_600);

/// One epoch as the operator's own wall clock reads it, or a placeholder of the
/// same width when there is no readable time. ONE FUNCTION for the header's two
/// bounds and every timeline line, so the recap cannot render two clocks.
pub fn recap_wall_clock(
    epoch: Option<u64>,
    local_minutes_since_midnight: impl FnOnce(u64) -> Option<u16>,
) -> String {
    epoch
        .and_then(local_minutes_since_midnight)
        .map(|minutes| format!("{:02}:{:02}", minutes / 60, minutes % 60))
        .unwrap_or_else(|| NO_WALL_CLOCK.to_string())
}
/// What a recap typed wrong is told.
///
/// THREE FORMS, ONE USAGE. The window form is the one the event path spawns;
/// the other two are an agent's, and every one of them exits 2 on a word this
/// will not vouch for, because a recap that swallowed a typo is a recap the
/// operator believes was posted.
pub const RECAP_USAGE: &str = "pns: usage: pns recap --since <date|date-time|duration-ago> \
                               [--until <date|date-time|duration-ago>]\n\
                               pns: usage: pns recap --since-epoch <epoch> --until-epoch <epoch>\n\
                               pns: usage: pns recap agent --stdin\n\
                               pns: usage: pns recap git";

/// What a line shows for a moment whose clock could not be read: the same width
/// as a time, so the timeline still lines up.
const NO_WALL_CLOCK: &str = "--:--";

#[cfg(test)]
#[path = "window/tests.rs"]
mod command_recap_tests;
