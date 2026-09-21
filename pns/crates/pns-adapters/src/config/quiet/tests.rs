use super::*;
use crate::config::parse_config;

#[test]
fn a_file_with_no_quiet_table_leaves_the_feature_off() {
    // THE DEFAULT, AND THE CASE THAT MATTERS MOST: every machine that never
    // wrote the table runs no calendar command at all.
    let calendar = parse_config("").unwrap().quiet_calendar;
    assert_eq!(calendar, QuietCalendar::default());
    assert_eq!(calendar.armed(), None);
}

#[test]
fn a_command_without_the_switch_is_still_off() {
    let calendar = parse_config("[quiet.calendar]\ncommand = [\"busy-window\"]\n")
        .unwrap()
        .quiet_calendar;
    assert_eq!(calendar.armed(), None);
}

#[test]
fn the_switch_without_a_command_is_off_rather_than_a_refusal() {
    let calendar = parse_config("[quiet.calendar]\nenabled = true\n")
        .unwrap()
        .quiet_calendar;
    assert_eq!(calendar.armed(), None);
}

#[test]
fn both_together_arm_the_poll_at_its_interval() {
    let calendar = parse_config(
        "[quiet.calendar]\nenabled = true\ncommand = [\"busy-window\", \"--json\"]\npoll_interval = \"5m\"\n",
    )
    .unwrap()
    .quiet_calendar;
    assert_eq!(calendar.armed(), Some(300));
    assert_eq!(
        calendar.source,
        CalendarSource::Command(vec!["busy-window".into(), "--json".into()])
    );
}

#[test]
fn a_shell_string_for_the_command_is_refused_by_name() {
    let refusal = parse_config("[quiet.calendar]\ncommand = \"busy-window --json\"\n").unwrap_err();
    assert!(
        refusal.detail().contains("command"),
        "the offender is named: {refusal:?}"
    );
}

#[test]
fn an_unknown_key_in_the_table_is_refused_with_the_vocabulary() {
    let refusal = parse_config("[quiet.calendar]\npoll_seconds = 120\n").unwrap_err();
    assert!(
        refusal.detail().contains("poll_interval"),
        "the refusal lists what the table serves: {refusal:?}"
    );
}

#[test]
fn an_interval_outside_the_range_is_refused_rather_than_clamped() {
    // A BARE COUNT IS REFUSED TOO: both are durations, and a number with no
    // unit is the ambiguity the duration vocabulary exists to end.
    for out_of_range in [
        "poll_interval = \"5s\"",
        "poll_interval = \"24h\"",
        "deadline = \"0s\"",
        "poll_interval = 120",
    ] {
        assert!(
            parse_config(&format!("[quiet.calendar]\n{out_of_range}\n")).is_err(),
            "{out_of_range} is refused"
        );
    }
}

/// THE MUTANT THIS PINS: either old spelling quietly admitted again, which
/// would leave a calendar polled at a figure the operator believes they set.
#[test]
fn the_timing_keys_are_refused_under_their_old_spellings_with_the_new_ones_listed() {
    for (retired, replacement) in [
        ("poll_secs = 300", "poll_interval"),
        ("deadline_secs = 20", "deadline"),
    ] {
        let refusal = parse_config(&format!("[quiet.calendar]\n{retired}\n")).unwrap_err();
        let said = refusal.detail();
        assert!(said.contains(retired.split(' ').next().unwrap()), "{said}");
        assert!(said.contains(replacement), "{said}");
    }
}

/// The rename moves the SPELLING and nothing else: each new key resolves to
/// the value its old spelling resolved to.
#[test]
fn the_new_spellings_resolve_to_what_the_old_ones_did() {
    let calendar = parse_config(
        "[quiet.calendar]\nenabled = true\ncommand = [\"busy-window\"]\n\
         poll_interval = \"5m\"\ndeadline = \"20s\"\n",
    )
    .unwrap()
    .quiet_calendar;
    assert_eq!((calendar.poll_secs, calendar.deadline_secs), (300, 20));
}

/// The table every machine already has: no `type` at all reads as the command
/// source, so a file written before the google reader existed means the same
/// thing it did.
#[test]
fn a_table_naming_no_type_reads_as_the_command_source() {
    let calendar = parse_config("[quiet.calendar]\ncommand = [\"busy-window\"]\n")
        .unwrap()
        .quiet_calendar;
    assert_eq!(
        calendar.source,
        CalendarSource::Command(vec!["busy-window".into()])
    );
}

#[test]
fn a_google_table_carrying_all_three_credentials_arms_the_poll() {
    let calendar = parse_config(
        "[quiet.calendar]\nenabled = true\ntype = \"google\"\ncalendars = [\"primary\", \"team\"]\n\
         client_id = \"id\"\nclient_secret = \"secret\"\nrefresh_token = \"refresh\"\n",
    )
    .unwrap()
    .quiet_calendar;
    assert_eq!(calendar.armed(), Some(DEFAULT_CALENDAR_POLL_SECS));
    assert_eq!(
        calendar.source,
        CalendarSource::Google(GoogleCalendar {
            calendars: vec!["primary".into(), "team".into()],
            client_id: "id".into(),
            client_secret: "secret".into(),
            refresh_token: "refresh".into(),
        })
    );
}

/// A GOOGLE TABLE NAMING NO CALENDAR READS THE ONE EVERY ACCOUNT HAS, which is
/// what the shipped file states and what a table that leaves the key out means.
#[test]
fn a_google_table_naming_no_calendar_reads_the_primary_one() {
    let calendar = parse_config(
        "[quiet.calendar]\ntype = \"google\"\nclient_id = \"id\"\nclient_secret = \"secret\"\n\
         refresh_token = \"refresh\"\n",
    )
    .unwrap()
    .quiet_calendar;
    assert_eq!(
        calendar.source,
        CalendarSource::Google(GoogleCalendar {
            calendars: vec!["primary".into()],
            client_id: "id".into(),
            client_secret: "secret".into(),
            refresh_token: "refresh".into(),
        })
    );
}

/// EVERY MISMATCH NAMES THE KEY. A table half-written is a calendar that
/// silently does not mute, so it is refused at load rather than at the first
/// poll.
#[test]
fn a_table_mixing_the_two_readers_or_missing_a_credential_is_refused_by_key() {
    for (file, named) in [
        (
            "[quiet.calendar]\ntype = \"google\"\nclient_secret = \"secret\"\nrefresh_token = \"refresh\"\n",
            "client_id",
        ),
        (
            "[quiet.calendar]\ntype = \"google\"\nclient_id = \"id\"\nrefresh_token = \"refresh\"\n",
            "client_secret",
        ),
        (
            "[quiet.calendar]\ntype = \"google\"\nclient_id = \"id\"\nclient_secret = \"secret\"\n",
            "refresh_token",
        ),
        (
            "[quiet.calendar]\ntype = \"command\"\ncommand = [\"busy-window\"]\nclient_id = \"id\"\n",
            "client_id",
        ),
        (
            "[quiet.calendar]\ntype = \"google\"\ncommand = [\"busy-window\"]\nclient_id = \"id\"\n\
             client_secret = \"secret\"\nrefresh_token = \"refresh\"\n",
            "command",
        ),
        ("[quiet.calendar]\ntype = \"dam\"\n", "type"),
    ] {
        let refusal = parse_config(file).unwrap_err();
        assert!(
            refusal.detail().contains(named),
            "the refusal names `{named}`: {refusal:?}"
        );
    }
}
