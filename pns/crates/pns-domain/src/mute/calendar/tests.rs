use super::*;

/// One busy event, nine to ten.
fn meeting() -> Event {
    Event {
        start: 1_000,
        end: 4_600,
        busy: true,
    }
}

#[test]
fn a_busy_event_covering_now_arms_the_mute_until_it_ends() {
    let (mv, state) = decide(&[meeting()], 1_200, None, CalendarState::default());
    assert_eq!(mv, Move::Arm(4_600));
    assert_eq!(state.armed_until, Some(4_600));
}

#[test]
fn the_end_of_the_event_clears_the_mute_the_calendar_armed() {
    let armed = CalendarState {
        armed_until: Some(4_600),
        declined_until: None,
    };
    let (mv, state) = decide(&[], 4_700, Some(4_600), armed);
    assert_eq!(mv, Move::Clear);
    assert_eq!(state, CalendarState::default());
}

#[test]
fn an_event_that_is_not_marked_busy_arms_nothing() {
    // THE MUTANT THIS PINS: the busy filter dropped. A day of free time,
    // travel and tentative holds would mute the machine from dawn to dusk.
    let free = Event {
        busy: false,
        ..meeting()
    };
    let (mv, state) = decide(&[free], 1_200, None, CalendarState::default());
    assert_eq!(mv, Move::Leave);
    assert_eq!(state, CalendarState::default());
}

#[test]
fn an_event_that_has_not_started_or_has_finished_arms_nothing() {
    for now in [999, 4_600] {
        let (mv, _) = decide(&[meeting()], now, None, CalendarState::default());
        assert_eq!(mv, Move::Leave, "at {now}");
    }
}

#[test]
fn a_mute_set_by_hand_during_a_meeting_is_neither_extended_nor_cleared() {
    // THE MUTANT THIS PINS: ownership dropped, so the calendar writes over
    // whatever mute it finds. The operator's own two hours would be cut back
    // to the end of the meeting, and then cleared when it finished.
    let by_hand = Some(90_000);
    let (during, state) = decide(&[meeting()], 1_200, by_hand, CalendarState::default());
    assert_eq!(during, Move::Leave);
    let (after, _) = decide(&[], 4_700, by_hand, state);
    assert_eq!(after, Move::Leave);
}

#[test]
fn a_mute_set_by_hand_that_runs_out_mid_meeting_lets_the_rest_be_armed() {
    let ran_out = Some(1_100);
    let (mv, _) = decide(&[meeting()], 1_200, ran_out, CalendarState::default());
    assert_eq!(mv, Move::Arm(4_600));
}

#[test]
fn quiet_switched_off_by_hand_during_a_meeting_is_not_switched_back_on() {
    // THE MUTANT THIS PINS: the override forgotten, so the next poll re-arms
    // the mute the operator just cleared, every two minutes, all meeting.
    let armed = CalendarState {
        armed_until: Some(4_600),
        declined_until: None,
    };
    let (mv, state) = decide(&[meeting()], 1_300, None, armed);
    assert_eq!(mv, Move::Leave);
    assert_eq!(state.declined_until, Some(4_600));
    let (again, _) = decide(&[meeting()], 1_400, None, state);
    assert_eq!(again, Move::Leave);
}

#[test]
fn an_override_expires_with_the_event_it_was_against() {
    let declined = CalendarState {
        armed_until: None,
        declined_until: Some(4_600),
    };
    let next = Event {
        start: 5_000,
        end: 8_000,
        busy: true,
    };
    let (mv, state) = decide(&[next], 5_100, None, declined);
    assert_eq!(mv, Move::Arm(8_000));
    assert_eq!(state.declined_until, None);
}

#[test]
fn an_event_that_moved_re_arms_the_mute_to_its_new_end() {
    let armed = CalendarState {
        armed_until: Some(4_600),
        declined_until: None,
    };
    let longer = Event {
        end: 6_000,
        ..meeting()
    };
    let (mv, state) = decide(&[longer], 1_200, Some(4_600), armed);
    assert_eq!(mv, Move::Arm(6_000));
    assert_eq!(state.armed_until, Some(6_000));
}

#[test]
fn a_mute_already_standing_at_the_events_end_is_left_alone() {
    let armed = CalendarState {
        armed_until: Some(4_600),
        declined_until: None,
    };
    let (mv, state) = decide(&[meeting()], 1_200, Some(4_600), armed);
    assert_eq!(mv, Move::Leave);
    assert_eq!(state.armed_until, Some(4_600));
}

#[test]
fn overlapping_meetings_are_one_stretch_ending_at_the_latest_end() {
    let second = Event {
        start: 4_000,
        end: 9_000,
        busy: true,
    };
    let (mv, _) = decide(&[meeting(), second], 4_100, None, CalendarState::default());
    assert_eq!(mv, Move::Arm(9_000));
}
