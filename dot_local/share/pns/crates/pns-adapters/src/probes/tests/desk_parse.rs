use super::*;

// --- parse_idle_nanoseconds --------------------------------------------

#[test]
fn the_idle_count_is_the_last_field_of_the_line_that_carries_the_key() {
    let output = "    | |   \"HIDIdleTime\" = 5000000000\n    | |   \"Other\" = 1\n";
    assert_eq!(parse_idle_nanoseconds(output), Some("5000000000"));
}

#[test]
fn the_first_idle_line_wins_so_a_second_device_cannot_override_it() {
    let output = "\"HIDIdleTime\" = 5000000000\n\"HIDIdleTime\" = 99\n";
    assert_eq!(parse_idle_nanoseconds(output), Some("5000000000"));
}

#[test]
fn output_without_the_idle_key_reads_as_unknown_rather_than_zero() {
    assert_eq!(parse_idle_nanoseconds("\"Other\" = 1\n"), None);
    assert_eq!(parse_idle_nanoseconds(""), None);
}

#[test]
fn contaminated_idle_output_reads_as_unknown_rather_than_a_reading() {
    // The bash reference (grep, in binary mode) refuses NUL-bearing output
    // outright. Trusting a corrupted stream can coerce to 0, which reads
    // as "actively typing" and silently suppresses the push; U+FFFD is
    // where the runner replaced invalid bytes, the same corruption.
    assert_eq!(parse_idle_nanoseconds("\0\"HIDIdleTime\" = 0\n"), None);
    assert_eq!(
        parse_idle_nanoseconds("\u{FFFD}\"HIDIdleTime\" = 5000000000\n"),
        None
    );
}

#[test]
fn the_console_key_saying_yes_is_a_locked_screen() {
    assert_eq!(parse_screen_locked(ROOT_LOCKED), Some(true));
}

#[test]
fn the_console_key_saying_no_is_an_unlocked_screen_whatever_the_session_array_says() {
    // THE PRECISION TEST. `CGSSessionScreenIsLocked` rides inside the
    // `"IOConsoleUsers"` line one row ABOVE, where the parser meets it
    // first, so a parser that searches for anything less specific than the
    // aggregate's own key answers from a per-session flag and reports a
    // locked screen at an occupied desk.
    assert_eq!(parse_screen_locked(ROOT_UNLOCKED_WITH_DECOY), Some(false));
}

#[test]
fn a_console_key_that_is_missing_or_says_something_else_reads_as_no_reading() {
    // None is not "unlocked", it is "nobody could tell", and only the
    // decision above states what to do about that. Reporting either
    // verdict here would put a guess where a reading belongs.
    assert_eq!(
        parse_screen_locked("+-o Root\n    {\n      \"OS Build Version\" = \"25C56\"\n    }\n"),
        None,
        "the key is not in this dictionary at all"
    );
    assert_eq!(parse_screen_locked(""), None, "and no output is no reading");
    assert_eq!(
        parse_screen_locked("      \"IOConsoleLocked\" = Maybe\n"),
        None,
        "a value this parser does not know is not a verdict"
    );
    assert_eq!(
        parse_screen_locked("      \"IOConsoleLocked\"\n"),
        None,
        "and neither is a key printed with no value beside it"
    );
}
