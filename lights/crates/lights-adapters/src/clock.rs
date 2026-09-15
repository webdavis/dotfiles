use lights_application::Clock;
use lights_domain::MinuteOfDay;
use std::time::{SystemTime, UNIX_EPOCH};

/// THE ONE PLACE the local zone is read. Which minute of the day an instant
/// falls in is a system fact (a zone database, a `TZ` variable and two
/// transitions a year), not a calculation, so it is asked of libc and leaves
/// as a plain number.
pub struct SystemClock;

impl Clock for SystemClock {
    fn local_minute(&self) -> Option<MinuteOfDay> {
        local_minute(SystemTime::now())
    }
}

fn local_minute(time: SystemTime) -> Option<MinuteOfDay> {
    let seconds = libc::time_t::try_from(time.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()?;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `localtime_r` writes the broken-down time into the `tm` it is
    // handed and returns either that same pointer or null. `seconds` points at
    // a live `time_t` on this frame, `broken_down` is an aligned `tm` this
    // frame owns for the whole call and nothing else aliases, and the
    // reentrant form writes its answer only into that buffer, which is what
    // makes it the thread-safe one to call. The buffer is read ONLY after a
    // non-null return, which is what proves it was initialized.
    let local = unsafe {
        if libc::localtime_r(&seconds, broken_down.as_mut_ptr()).is_null() {
            return None;
        }
        broken_down.assume_init()
    };
    // Range-checked rather than trusted: this is an FFI boundary, and a minute
    // of the day is what the caller is promised.
    MinuteOfDay::try_from(local.tm_hour.checked_mul(60)?.checked_add(local.tm_min)?)
        .ok()
        .filter(|minute| *minute < lights_domain::MINUTES_PER_DAY)
}

#[cfg(test)]
mod tests;
