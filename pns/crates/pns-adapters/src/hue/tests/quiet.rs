use super::fixtures::*;
// --- the quiet window ---------------------------------------------------

#[test]
fn a_table_that_names_no_quiet_hours_has_no_window() {
    assert_eq!(
        quiet_window(&table("bridge = \"b\"\nkey = \"k\"")),
        Ok(None),
        "an operator who never asked to be quieted keeps today's behavior"
    );
}

#[test]
fn a_window_parses_to_minutes_since_local_midnight() {
    assert_eq!(
        quiet_window(&table("quiet_hours = \"22:00-07:00\"")),
        Ok(parse_window("22:00-07:00")),
        "22:00 is 1320 minutes in and 07:00 is 420"
    );
    assert_eq!(
        quiet_window(&table("quiet_hours = \"22:00-07:00\""))
            .expect("parses")
            .expect("a window")
            .ends_at(),
        420,
        "and the END minute is the one a bare mute reads off it: answering \
         the start would run a bedtime mute almost a whole day"
    );
}

#[test]
fn a_quiet_hours_that_is_not_two_clock_readings_is_refused_by_name() {
    for stated in [
        "22:00",
        "24:00-07:00",
        "22:60-07:00",
        "10pm-7am",
        "2:00-07:00",
        "22:00-07:00 ",
        "   ",
    ] {
        let refusal = quiet_window(&table(&format!("quiet_hours = \"{stated}\"")))
            .expect_err("a window this shape names no hours");
        assert!(
            refusal.contains("hue.quiet_hours") && refusal.contains(stated),
            "the refusal names the key and echoes what was written: {refusal}"
        );
    }
}

#[test]
fn a_quiet_hours_of_the_wrong_type_is_refused_by_name_and_by_type() {
    for (stated, kind) in [("2200", "integer"), ("true", "boolean"), ("[]", "array")] {
        let refusal = quiet_window(&table(&format!("quiet_hours = {stated}")))
            .expect_err("a window that is not a string names no hours");
        assert!(
            refusal.contains("hue.quiet_hours") && refusal.contains(kind),
            "the refusal names the key and what was written instead: {refusal}"
        );
    }
}

#[test]
fn a_blanked_quiet_hours_is_no_window_rather_than_a_refusal() {
    assert_eq!(
        quiet_window(&table("quiet_hours = \"\"")),
        Ok(None),
        "blanking a value plainly means none, the way an empty bridge or key does"
    );
}

#[test]
fn a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end() {
    // 22:00 to 23:00, the plainest same-day window.
    let evening = parse_window("22:00-23:00").expect("valid window");
    assert!(
        !quiet_now(Some(&evening), Some(1319)),
        "the minute before the window is loud"
    );
    assert!(
        quiet_now(Some(&evening), Some(1320)),
        "the start is inside the window"
    );
    assert!(
        quiet_now(Some(&evening), Some(1379)),
        "and so is the last minute before its end"
    );
    assert!(
        !quiet_now(Some(&evening), Some(1380)),
        "the end is loud on the dot, so two adjacent windows cannot overlap"
    );
}

#[test]
fn a_window_whose_start_is_after_its_end_is_quiet_on_both_sides_of_midnight() {
    // 22:00-07:00, the window the template documents.
    let overnight = parse_window("22:00-07:00").expect("valid window");
    for (minute, quiet, moment) in [
        (1319, false, "21:59, before it opens"),
        (1320, true, "22:00, the start"),
        (1439, true, "23:59, the last minute of the day"),
        (0, true, "00:00, the first minute of the next one"),
        (419, true, "06:59, still inside"),
        (420, false, "07:00, the end"),
        (720, false, "noon, nowhere near it"),
    ] {
        assert_eq!(
            quiet_now(Some(&overnight), Some(minute)),
            quiet,
            "{moment} is on the wrong side of a window that wraps"
        );
    }
}

#[test]
fn a_window_whose_start_equals_its_end_is_never_quiet() {
    // An empty half-open range, and deliberately not a special case: the
    // all-day mute already exists as `enabled = false`. Every minute of
    // the day is checked, because "never" is the whole claim.
    let empty = parse_window("10:00-10:00").expect("valid window");
    for minute in 0..1440 {
        assert!(
            !quiet_now(Some(&empty), Some(minute)),
            "minute {minute} fell inside a window that spans no time"
        );
    }
}
