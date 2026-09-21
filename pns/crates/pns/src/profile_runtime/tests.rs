use super::*;

/// 2025-08-24T01:46:40Z, months from the nearest daylight-saving transition
/// of the zones this suite runs in, the same anchor `pns_adapters::clock`'s
/// own tests use, so a day added to it is exactly a day wherever this runs.
const AUGUST_INSTANT: u64 = 1_756_000_000;

#[test]
fn a_bound_ending_later_the_same_day_is_a_bare_clock_time() {
    let same_day =
        until_clock(Some(AUGUST_INSTANT), AUGUST_INSTANT + 60).expect("a readable local zone");
    assert!(!same_day.contains(" on "), "same calendar day: {same_day}");
}

#[test]
fn a_bound_ending_on_a_different_day_carries_the_date() {
    let tomorrow = until_clock(Some(AUGUST_INSTANT), AUGUST_INSTANT + 24 * 3_600)
        .expect("a readable local zone");
    assert!(tomorrow.contains(" on "), "a day later: {tomorrow}");
}
