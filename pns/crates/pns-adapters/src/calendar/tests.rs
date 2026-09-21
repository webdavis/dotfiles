use super::command::{NO_ANSWER, NOT_THE_DOCUMENT, parse_calendar};
use super::*;
use crate::state_fixtures::scratch;

/// One command source through the dispatch, which is the only way in.
fn read_calendar(argv: &[String], deadline: Duration) -> Result<Vec<Event>, String> {
    super::read_calendar(
        &CalendarSource::Command(argv.to_vec()),
        &scratch("quiet-calendar-dispatch"),
        1_789_658_187,
        deadline,
    )
}

/// A doubled calendar command: `sh -c <script>`, which is how a test states a
/// command that fails, stalls or answers nonsense without a fixture file.
fn doubled(script: &str) -> Vec<String> {
    vec!["/bin/sh".into(), "-c".into(), script.into()]
}

const DEADLINE: Duration = Duration::from_millis(500);

#[test]
fn a_documented_answer_reads_back_as_its_events() {
    let events = read_calendar(
        &doubled(
            r#"printf '{"events":[{"start":10,"end":20,"busy":true},{"start":30,"end":40,"busy":false}]}'"#,
        ),
        DEADLINE,
    )
    .expect("the documented document reads");
    assert_eq!(
        events,
        vec![
            Event {
                start: 10,
                end: 20,
                busy: true
            },
            Event {
                start: 30,
                end: 40,
                busy: false
            },
        ]
    );
}

#[test]
fn a_command_that_exits_non_zero_is_no_answer_rather_than_an_empty_calendar() {
    let refusal = read_calendar(&doubled("exit 3"), DEADLINE).expect_err("a failure is refused");
    assert_eq!(refusal, NO_ANSWER);
}

#[test]
fn a_command_that_answers_something_unparseable_is_refused_without_quoting_it() {
    let refusal = read_calendar(
        &doubled("printf 'Standup with Dana <dana@example.com>'"),
        DEADLINE,
    )
    .expect_err("nonsense is refused");
    assert!(
        !refusal.contains("Dana") && !refusal.contains("example.com"),
        "a refusal never carries what the calendar said: {refusal}"
    );
    assert_eq!(refusal, NOT_THE_DOCUMENT);
}

#[test]
fn a_command_that_hangs_past_its_deadline_is_no_answer() {
    let refusal = read_calendar(&doubled("sleep 30"), Duration::from_millis(200))
        .expect_err("a hang is refused");
    assert_eq!(refusal, NO_ANSWER);
}

#[test]
fn an_event_missing_a_field_refuses_the_whole_answer_rather_than_dropping_it() {
    // A DROPPED EVENT IS A MEETING THAT SILENTLY DOES NOT MUTE, and it looks
    // exactly like a clear calendar.
    assert_eq!(
        parse_calendar(r#"{"events":[{"start":10,"end":20}]}"#),
        Err(NOT_THE_DOCUMENT.to_string())
    );
    assert_eq!(
        parse_calendar(r#"{"events":[{"start":10.5,"end":20,"busy":true}]}"#),
        Err(NOT_THE_DOCUMENT.to_string())
    );
    assert_eq!(parse_calendar("{}"), Err(NOT_THE_DOCUMENT.to_string()));
}

#[test]
fn an_empty_calendar_is_an_answer_and_not_a_refusal() {
    assert_eq!(parse_calendar(r#"{"events":[]}"#), Ok(Vec::new()));
}

#[test]
fn an_unknown_field_rides_along_rather_than_refusing_the_document() {
    assert_eq!(
        parse_calendar(r#"{"events":[{"start":10,"end":20,"busy":true,"calendar":"work"}]}"#),
        Ok(vec![Event {
            start: 10,
            end: 20,
            busy: true
        }])
    );
}

#[test]
fn the_state_file_carries_both_numbers_across_a_publish() {
    let state = scratch("quiet-calendar-state");
    let held = CalendarState {
        armed_until: Some(4_600),
        declined_until: Some(9_000),
    };
    write_calendar_state(&state, &held).expect("the state publishes");
    assert_eq!(read_calendar_state(&state), held);
}

#[test]
fn a_missing_or_damaged_state_file_reads_as_no_state() {
    let state = scratch("quiet-calendar-damaged");
    assert_eq!(read_calendar_state(&state), CalendarState::default());
    std::fs::write(state.join(CALENDAR_STATE), "not a state line\n").expect("plant");
    assert_eq!(read_calendar_state(&state), CalendarState::default());
}
