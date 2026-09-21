use super::*;

#[test]
fn a_utc_time_reads_as_its_epoch_second() {
    assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_rfc3339("2026-09-17T15:16:27Z"), Some(1_789_658_187));
}

#[test]
fn a_fraction_is_dropped_and_an_offset_is_applied() {
    assert_eq!(
        parse_rfc3339("2026-09-17T15:16:27.482Z"),
        parse_rfc3339("2026-09-17T15:16:27Z")
    );
    // The same moment, written in two zones.
    assert_eq!(
        parse_rfc3339("2026-09-17T17:16:27+02:00"),
        parse_rfc3339("2026-09-17T15:16:27Z")
    );
    assert_eq!(
        parse_rfc3339("2026-09-17T11:16:27-04:00"),
        parse_rfc3339("2026-09-17T15:16:27Z")
    );
}

/// THE MUTANT THIS PINS: a looser parser that reads a near-miss as a time.
/// A bound that reads wrong is a meeting muted at the wrong hour, or not at
/// all.
#[test]
fn anything_that_is_not_an_rfc_3339_time_reads_as_none() {
    for stated in [
        "",
        "2026-09-17",
        "2026-09-17T15:16:27",
        "2026-9-17T15:16:27Z",
        "2026-09-17 15:16:27Z",
        "2026-13-17T15:16:27Z",
        "2026-09-17T25:16:27Z",
        "2026-09-17T15:61:27Z",
        "2026-09-17T15:16:27+2:00",
        "2026-09-17T15:16:27+02",
        "1969-12-31T23:59:59Z",
        "Standup with Dana",
    ] {
        assert_eq!(parse_rfc3339(stated), None, "{stated} is not a time");
    }
}

#[test]
fn what_the_formatter_writes_is_what_the_parser_reads_back() {
    for epoch in [0, 1, 86_399, 951_782_400, 1_789_658_187, 4_102_444_800] {
        let written = format_rfc3339(epoch);
        assert_eq!(parse_rfc3339(&written), Some(epoch), "{written}");
    }
    assert_eq!(format_rfc3339(1_789_658_187), "2026-09-17T15:16:27Z");
}

/// A LEAP DAY AND A CENTURY NON-LEAP YEAR, which is where a hand-written
/// civil-date conversion goes wrong if it goes wrong at all.
#[test]
fn the_gregorian_leap_rules_hold_at_both_edges() {
    assert_eq!(format_rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(
        parse_rfc3339("2100-03-01T00:00:00Z"),
        parse_rfc3339("2100-02-28T00:00:00Z").map(|epoch| epoch + 86_400)
    );
}
