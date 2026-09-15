use std::{
    ffi::{CStr, CString},
    ptr,
    time::{Duration, Instant},
};

/// What a disabled or unrepresentable timeout becomes. `timeout(1)` runs with
/// no limit at all for a duration of 0, and warns and then runs with no limit
/// for `inf` or a value too large for its timer. A runner needs a real
/// deadline, so those collapse to one ceiling far past any poll interval, and
/// every shorter timeout is honoured as written.
pub const COMMAND_DURATION_CEILING: Duration = Duration::from_secs(86_400);

pub fn parse_command_duration(value: &str) -> Option<Duration> {
    let value = CString::new(value).ok()?;
    let mut end = ptr::null_mut();
    // strtod rather than f64::from_str, which rejects the hex-float literals GNU
    // timeout accepts: the captured `timeout_hex` case passes 0x1p-1 as 0.5s.
    // SAFETY: the input is NUL-terminated and lives through strtod and the suffix read.
    // The new locale is owned here, installed only on this thread, restored before freeing,
    // and never exposed to Rust code that could unwind while it is installed.
    let seconds = unsafe {
        let locale = libc::newlocale(libc::LC_ALL_MASK, c"C".as_ptr(), ptr::null_mut());
        if locale.is_null() {
            return None;
        }
        let previous = libc::uselocale(locale);
        if previous.is_null() {
            libc::freelocale(locale);
            return None;
        }
        let seconds = libc::strtod(value.as_ptr(), &mut end);
        libc::uselocale(previous);
        libc::freelocale(locale);
        seconds
    };
    // SAFETY: strtod sets end within the still-live CString, including on conversion failure.
    let suffix = unsafe { CStr::from_ptr(end) }.to_bytes();
    let scale = match suffix {
        b"" | b"s" => 1.,
        b"m" => 60.,
        b"h" => 3600.,
        b"d" => 86400.,
        _ => return None,
    };
    let seconds = seconds * scale;
    // What timeout(1) calls an invalid time interval, so the 125 a refusal
    // reports is the exit code timeout would itself have produced.
    if seconds.is_nan() || seconds < 0. {
        return None;
    }
    if seconds == 0. || seconds.is_infinite() {
        return Some(COMMAND_DURATION_CEILING);
    }
    Some(
        Duration::try_from_secs_f64(seconds)
            .ok()
            .filter(|duration| Instant::now().checked_add(*duration).is_some())
            .map_or(COMMAND_DURATION_CEILING, |duration| {
                duration.min(COMMAND_DURATION_CEILING)
            }),
    )
}
