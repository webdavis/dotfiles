use super::*;

/// 2025-08-24T01:46:40Z, months from the nearest daylight-saving
/// transition of the zones this suite runs in (2025-03-09 and 2025-11-02
/// on the developer's, none at all on a UTC runner), so the minute after
/// it is a minute later there.
const AUGUST_INSTANT: u64 = 1_756_000_000;

#[test]
fn the_local_clock_reads_a_minute_of_the_day_for_the_second_it_was_given() {
    let minutes = local_minutes_since_midnight(AUGUST_INSTANT).expect("a readable local zone");
    assert!(
        minutes < 1440,
        "a minute of the day, whatever the zone: {minutes}"
    );
    let later = local_minutes_since_midnight(AUGUST_INSTANT + 60).expect("a readable local zone");
    assert_eq!(
        (later + 1440 - minutes) % 1440,
        1,
        "and it reads the second it was handed, not the wall clock"
    );
}

#[test]
fn the_utc_instant_is_the_same_second_wherever_the_suite_runs() {
    // THE ZONE IS NOT READ, which is the property: the only caller states a
    // window to a remote search, and a local hour would be read there as an
    // hour nobody meant. Pinned as an exact string, so a build that
    // reached for the local zone fails on the developer's machine and on a
    // UTC runner alike.
    assert_eq!(
        utc_timestamp(AUGUST_INSTANT).as_deref(),
        Some("2025-08-24T01:46:40Z")
    );
}

#[test]
fn a_local_calendar_moment_reads_back_as_the_second_it_was_broken_down_from() {
    // THE ZONE IS NOT PINNED HERE AND DOES NOT NEED TO BE: the property is
    // that this is `localtime_r`'s inverse, which holds in whatever zone the
    // suite runs in. The instant is months from either transition, so no
    // hour it lands on is an ambiguous one.
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    let seconds = libc::time_t::try_from(AUGUST_INSTANT).expect("an expressible second");
    let local = unsafe {
        assert!(!libc::localtime_r(&seconds, broken_down.as_mut_ptr()).is_null());
        broken_down.assume_init()
    };
    assert_eq!(
        local_epoch(
            u32::try_from(local.tm_year + 1900).expect("a year"),
            u32::try_from(local.tm_mon + 1).expect("a month"),
            u32::try_from(local.tm_mday).expect("a day"),
            u32::try_from(local.tm_hour).expect("an hour"),
            u32::try_from(local.tm_min).expect("a minute"),
            u32::try_from(local.tm_sec).expect("a second"),
        ),
        Some(AUGUST_INSTANT)
    );
}

#[test]
fn a_day_the_calendar_does_not_have_is_refused_rather_than_rolled_forward() {
    // A WINDOW SILENTLY MOVED is a window the operator believes they asked
    // for, and `mktime` on its own rolls February 30th into March.
    assert_eq!(local_epoch(2026, 2, 30, 0, 0, 0), None);
    assert_eq!(local_epoch(2025, 2, 29, 0, 0, 0), None);
    assert!(local_epoch(2024, 2, 29, 0, 0, 0).is_some());
}
