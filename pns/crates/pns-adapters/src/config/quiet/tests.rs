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
        "[quiet.calendar]\nenabled = true\ncommand = [\"busy-window\", \"--json\"]\npoll_secs = 300\n",
    )
    .unwrap()
    .quiet_calendar;
    assert_eq!(calendar.armed(), Some(300));
    assert_eq!(calendar.command, ["busy-window", "--json"]);
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
        refusal.detail().contains("poll_secs"),
        "the refusal lists what the table serves: {refusal:?}"
    );
}

#[test]
fn an_interval_outside_the_range_is_refused_rather_than_clamped() {
    for out_of_range in ["poll_secs = 5", "poll_secs = 86400", "deadline_secs = 0"] {
        assert!(
            parse_config(&format!("[quiet.calendar]\n{out_of_range}\n")).is_err(),
            "{out_of_range} is refused"
        );
    }
}
