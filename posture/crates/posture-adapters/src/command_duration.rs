use std::{
    ffi::{CStr, CString},
    ptr,
    time::{Duration, Instant},
};

pub fn parse_command_duration(value: &str) -> Option<Duration> {
    let value = CString::new(value).ok()?;
    let mut end = ptr::null_mut();
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
    if seconds <= 0. || !seconds.is_finite() {
        return None;
    }
    Duration::try_from_secs_f64(seconds)
        .ok()
        .filter(|duration| !duration.is_zero() && Instant::now().checked_add(*duration).is_some())
}
