use super::*;

fn armed() -> QuietCalendar {
    QuietCalendar {
        enabled: true,
        source: CalendarSource::Command(vec!["busy-window".to_string()]),
        ..QuietCalendar::default()
    }
}

/// A doubled calendar: what the command answered, and whether it was asked.
fn answering(
    answer: Result<Vec<Event>, String>,
    asked: &mut bool,
) -> impl FnMut(&CalendarSource, Duration) -> Result<Vec<Event>, String> + '_ {
    move |_, _| {
        *asked = true;
        answer.clone()
    }
}

fn meeting() -> Vec<Event> {
    vec![Event {
        start: 1_000,
        end: 4_600,
        busy: true,
    }]
}

#[test]
fn an_absent_or_disabled_table_runs_no_command_at_all() {
    // THE SHIPPED DEFAULT, and the case that matters most: a machine that
    // never armed this spawns nothing, reads nothing and decides nothing.
    for calendar in [
        QuietCalendar::default(),
        QuietCalendar {
            enabled: false,
            ..armed()
        },
        QuietCalendar {
            source: CalendarSource::default(),
            ..armed()
        },
    ] {
        let mut asked = false;
        let outcome = poll(
            &calendar,
            Some(1_200),
            None,
            CalendarState::default(),
            &mut answering(Ok(meeting()), &mut asked),
        );
        assert!(matches!(outcome, Poll::Off));
        assert!(!asked, "an unarmed calendar must not be run");
    }
}

#[test]
fn a_machine_with_no_clock_runs_no_command_either() {
    let mut asked = false;
    let outcome = poll(
        &armed(),
        None,
        None,
        CalendarState::default(),
        &mut answering(Ok(meeting()), &mut asked),
    );
    assert!(matches!(outcome, Poll::Off));
    assert!(!asked);
}

#[test]
fn an_armed_calendar_carries_the_decision_back() {
    let mut asked = false;
    let outcome = poll(
        &armed(),
        Some(1_200),
        None,
        CalendarState::default(),
        &mut answering(Ok(meeting()), &mut asked),
    );
    assert!(asked);
    match outcome {
        Poll::Decided(chosen, next) => {
            assert_eq!(chosen, Move::Arm(4_600));
            assert_eq!(next.armed_until, Some(4_600));
        }
        _ => panic!("an armed calendar decides"),
    }
}

#[test]
fn a_command_that_could_not_be_read_decides_nothing() {
    // NOT A CLEAR AND NOT AN ARM: a calendar that failed, answered nonsense
    // or ran long leaves the mute exactly as it stands, so a meeting already
    // muted stays muted until its own expiry.
    let mut asked = false;
    let outcome = poll(
        &armed(),
        Some(1_200),
        Some(4_600),
        CalendarState {
            armed_until: Some(4_600),
            declined_until: None,
        },
        &mut answering(
            Err("the calendar command answered nothing".into()),
            &mut asked,
        ),
    );
    assert!(matches!(outcome, Poll::Unread(_)));
}
