use super::*;
use std::time::Duration;

/// Midnight UTC on 1970-01-01, read back in whatever zone the test host is
/// set to. The MINUTE is what varies with the zone; that the answer is a
/// minute of a real day is what this pins.
#[test]
fn an_epoch_instant_reads_back_as_a_minute_of_the_day() {
    let minute = local_minute(UNIX_EPOCH).unwrap();
    assert!(minute < lights_domain::MINUTES_PER_DAY);
    let hour_later = local_minute(UNIX_EPOCH + Duration::from_secs(3600)).unwrap();
    assert_eq!(hour_later, (minute + 60) % lights_domain::MINUTES_PER_DAY);
}

#[test]
fn the_system_clock_answers_with_a_minute_of_the_day() {
    assert!(SystemClock.local_minute().unwrap() < lights_domain::MINUTES_PER_DAY);
}
