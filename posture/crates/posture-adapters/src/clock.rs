use posture_application::{Clock, ClockUnavailable, WallTime};
use std::time::{SystemTime, UNIX_EPOCH};
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        wall_time(SystemTime::now())
    }
}
fn wall_time(time: SystemTime) -> Result<WallTime, ClockUnavailable> {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ClockUnavailable)?
        .as_secs();
    let timestamp: libc::time_t = seconds.try_into().map_err(|_| ClockUnavailable)?;
    let mut day = std::mem::MaybeUninit::<libc::tm>::uninit();
    // gmtime_r writes a complete tm into our exclusive output when it returns non-null.
    let result = unsafe { libc::gmtime_r(&timestamp, day.as_mut_ptr()) };
    if result.is_null() {
        return Err(ClockUnavailable);
    }
    // The successful gmtime_r call initialized every field read below.
    let day = unsafe { day.assume_init() };
    let year = day.tm_year.checked_add(1900).ok_or(ClockUnavailable)?;
    Ok(WallTime {
        seconds,
        utc_day: format!("{year:04}-{:02}-{:02}", day.tm_mon + 1, day.tm_mday),
    })
}
#[cfg(test)]
mod tests;
