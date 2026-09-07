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
