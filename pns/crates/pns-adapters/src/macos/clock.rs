/// Minutes since LOCAL midnight for an epoch second, or None when the system
/// cannot say.
///
/// THE ONE PLACE the local zone is read. Which hour an epoch second falls in
/// is a system fact (a zone database, a `TZ` variable and two transitions a
/// year), not a calculation, so it is asked of libc rather than derived here,
/// and the answer leaves as a plain number: every rule about quiet hours is a
/// value function over this minute, with no clock inside it.
pub fn local_minutes_since_midnight(epoch_secs: u64) -> Option<u16> {
    let seconds = libc::time_t::try_from(epoch_secs).ok()?;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `localtime_r` writes the broken-down time into the `tm` it is
    // handed and returns either that same pointer or null. `seconds` points
    // at a live `time_t` on this frame, `broken_down` is an aligned `tm` this
    // frame owns for the whole call and nothing else aliases, and the
    // reentrant form writes its answer only into that buffer, which is what
    // makes it the thread-safe one to call from here. The buffer is read ONLY
    // after a non-null return, which is what proves it was initialized.
    let local = unsafe {
        if libc::localtime_r(&seconds, broken_down.as_mut_ptr()).is_null() {
            return None;
        }
        broken_down.assume_init()
    };
    // Range-checked rather than trusted: this is an FFI boundary, and a minute
    // of day is what every caller is promised.
    u16::try_from(local.tm_hour.checked_mul(60)?.checked_add(local.tm_min)?)
        .ok()
        .filter(|minutes| *minutes < 1440)
}

/// The epoch second a local calendar moment falls on, or None when the system
/// cannot say or the fields name no such day.
///
/// THE SAME PLACE THE LOCAL ZONE IS READ, from the other direction:
/// `mktime` applies the zone database, the `TZ` variable and the two
/// transitions a year, with `tm_isdst` left at -1 so it decides for itself
/// which side of a transition the moment sits on.
///
/// A DAY THAT DOES NOT EXIST IS REFUSED rather than normalized: `mktime`
/// rolls February 30th forward to March, and a window silently moved is a
/// window the operator believes they asked for.
pub fn local_epoch(
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<u64> {
    let mut broken_down: libc::tm = unsafe { std::mem::zeroed() };
    broken_down.tm_year = i32::try_from(year).ok()?.checked_sub(1900)?;
    broken_down.tm_mon = i32::try_from(month).ok()?.checked_sub(1)?;
    broken_down.tm_mday = i32::try_from(day).ok()?;
    broken_down.tm_hour = i32::try_from(hour).ok()?;
    broken_down.tm_min = i32::try_from(minute).ok()?;
    broken_down.tm_sec = i32::try_from(second).ok()?;
    broken_down.tm_isdst = -1;
    // SAFETY: `mktime` reads and writes only the `tm` it is handed, which this
    // frame owns for the whole call with nothing else aliasing it, and returns
    // -1 for a moment it cannot express.
    let seconds = unsafe { libc::mktime(&mut broken_down) };
    // Read back rather than trusted: the normalized fields are what say the
    // moment was a real one. A DST gap normalizes the hour (and sometimes the
    // minute) the same way an impossible date normalizes the day, so both are
    // caught here.
    let kept = broken_down.tm_mon == i32::try_from(month).ok()? - 1
        && broken_down.tm_mday == i32::try_from(day).ok()?
        && broken_down.tm_hour == i32::try_from(hour).ok()?
        && broken_down.tm_min == i32::try_from(minute).ok()?;
    if seconds == -1 || !kept {
        return None;
    }
    u64::try_from(seconds).ok()
}

/// One epoch second as an RFC 3339 instant in UTC, or None when the system
/// cannot say.
///
/// UTC AND NOT THE LOCAL ZONE, which is the whole reason this is a second
/// function rather than a format applied to the first. The only caller states a
/// window to a REMOTE search service, and a bare local time would be read there
/// as an hour the operator did not mean, twice a year by a different amount.
/// The `Z` is what makes the instant unambiguous wherever it is parsed.
pub fn utc_timestamp(epoch_secs: u64) -> Option<String> {
    let seconds = libc::time_t::try_from(epoch_secs).ok()?;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `gmtime_r`'s contract is `localtime_r`'s above, and for the same
    // reasons: `seconds` points at a live `time_t` on this frame, `broken_down`
    // is an aligned `tm` this frame owns for the whole call with nothing else
    // aliasing it, the reentrant form writes only into that buffer, and the
    // buffer is read ONLY after a non-null return, which is what proves it was
    // initialized.
    let utc = unsafe {
        if libc::gmtime_r(&seconds, broken_down.as_mut_ptr()).is_null() {
            return None;
        }
        broken_down.assume_init()
    };
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        utc.tm_year.checked_add(1900)?,
        utc.tm_mon.checked_add(1)?,
        utc.tm_mday,
        utc.tm_hour,
        utc.tm_min,
        utc.tm_sec
    ))
}

#[cfg(test)]
mod tests;
