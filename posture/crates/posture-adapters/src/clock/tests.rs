use super::*;
use std::time::{Duration, UNIX_EPOCH};
#[test]
fn epoch_and_seconds_within_day_are_utc() {
    assert_eq!(
        wall_time(UNIX_EPOCH),
        Ok(WallTime {
            seconds: 0,
            utc_day: "1970-01-01".into()
        })
    );
    assert_eq!(
        wall_time(UNIX_EPOCH + Duration::from_secs(10_000)),
        Ok(WallTime {
            seconds: 10_000,
            utc_day: "1970-01-01".into()
        })
    );
}
#[test]
fn utc_day_changes_at_midnight_and_handles_leap_day() {
    assert_eq!(
        wall_time(UNIX_EPOCH + Duration::from_secs(86_399))
            .unwrap()
            .utc_day,
        "1970-01-01"
    );
    assert_eq!(
        wall_time(UNIX_EPOCH + Duration::from_secs(86_400))
            .unwrap()
            .utc_day,
        "1970-01-02"
    );
    assert_eq!(
        wall_time(UNIX_EPOCH + Duration::from_secs(1_709_164_800))
            .unwrap()
            .utc_day,
        "2024-02-29"
    );
}
#[test]
fn a_pre_epoch_clock_is_unavailable_instead_of_zero() {
    assert_eq!(
        wall_time(UNIX_EPOCH - Duration::from_nanos(1)),
        Err(ClockUnavailable)
    );
}
