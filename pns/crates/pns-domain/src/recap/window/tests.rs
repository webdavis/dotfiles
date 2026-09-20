use super::*;

/// 2026-09-19 was a Saturday, which `tm_wday` numbers 6.
const SATURDAY: u32 = 6;

fn moment(day: u32, hour: u32, minute: u32) -> LocalCivilTime {
    LocalCivilTime {
        year: 2026,
        month: 9,
        day,
        hour,
        minute,
        second: 0,
    }
}

fn span(
    window: Window,
    previous: bool,
    now: LocalCivilTime,
) -> (LocalCivilTime, LocalCivilTime) {
    resolve(
        window,
        previous,
        &Periods::default(),
        WeekStart::Monday,
        now,
        SATURDAY,
    )
}

#[test]
fn a_named_period_that_has_started_today_is_todays_instance() {
    // 15:00: this morning, complete.
    assert_eq!(
        span(Window::Morning, false, moment(19, 15, 0)),
        (moment(19, 6, 0), moment(19, 12, 0))
    );
}

#[test]
fn a_named_period_that_has_not_started_yet_is_yesterdays_instance() {
    // 10:00 is inside the morning, so the most recent evening is last night's.
    assert_eq!(
        span(Window::Evening, false, moment(19, 10, 0)),
        (moment(18, 17, 0), moment(18, 22, 0))
    );
}

#[test]
fn a_period_in_progress_is_the_one_the_bare_name_picks() {
    // 10:00: this morning so far. The bound is the window's own end, not now;
    // a window in progress has nothing recorded past it either way.
    assert_eq!(
        span(Window::Morning, false, moment(19, 10, 0)),
        (moment(19, 6, 0), moment(19, 12, 0))
    );
}

#[test]
fn overnight_crosses_midnight_in_both_directions() {
    // At 08:00 the most recent overnight started last night.
    assert_eq!(
        span(Window::Overnight, false, moment(19, 8, 0)),
        (moment(18, 22, 0), moment(19, 6, 0))
    );
    // At 23:00 it started tonight and ends tomorrow.
    assert_eq!(
        span(Window::Overnight, false, moment(19, 23, 0)),
        (moment(19, 22, 0), moment(20, 6, 0))
    );
    // And at 03:00, inside it, it is still last night's start.
    assert_eq!(
        span(Window::Overnight, false, moment(19, 3, 0)),
        (moment(18, 22, 0), moment(19, 6, 0))
    );
}

#[test]
fn previous_steps_back_exactly_one_instance() {
    assert_eq!(
        span(Window::Overnight, true, moment(19, 8, 0)),
        (moment(17, 22, 0), moment(18, 6, 0))
    );
    assert_eq!(
        span(Window::Afternoon, true, moment(19, 18, 0)),
        (moment(18, 12, 0), moment(18, 17, 0))
    );
}

#[test]
fn today_runs_from_midnight_to_now_and_yesterday_is_the_whole_day_before() {
    assert_eq!(
        span(Window::Today, false, moment(19, 13, 30)),
        (moment(19, 0, 0), moment(19, 13, 30))
    );
    assert_eq!(
        span(Window::Today, true, moment(19, 13, 30)),
        (moment(18, 0, 0), moment(19, 0, 0))
    );
}

#[test]
fn a_month_boundary_steps_back_by_the_calendar_rather_than_by_seconds() {
    assert_eq!(
        span(Window::Today, true, moment(1, 9, 0)),
        (
            LocalCivilTime {
                month: 8,
                day: 31,
                ..moment(1, 0, 0)
            },
            moment(1, 0, 0)
        )
    );
}

#[test]
fn the_week_starts_on_the_configured_day() {
    let now = moment(19, 13, 30);
    // Saturday the 19th: Monday was the 14th, Sunday the 13th.
    let monday = resolve(
        Window::Week,
        false,
        &Periods::default(),
        WeekStart::Monday,
        now,
        SATURDAY,
    );
    assert_eq!(monday, (moment(14, 0, 0), now));
    let sunday = resolve(
        Window::Week,
        false,
        &Periods::default(),
        WeekStart::Sunday,
        now,
        SATURDAY,
    );
    assert_eq!(sunday, (moment(13, 0, 0), now));
}

#[test]
fn last_week_is_the_seven_days_before_this_week_started() {
    assert_eq!(
        span(Window::Week, true, moment(19, 13, 30)),
        (moment(7, 0, 0), moment(14, 0, 0))
    );
}

#[test]
fn the_window_that_most_recently_ended_is_what_a_bare_recap_takes() {
    let periods = Periods::default();
    for (hour, expected) in [
        (8, Window::Overnight),
        (13, Window::Morning),
        (18, Window::Afternoon),
        (23, Window::Evening),
        // Before any of today's ends, last night's evening is the answer.
        (3, Window::Evening),
        // Exactly on an end, that window has just ended.
        (6, Window::Overnight),
    ] {
        assert_eq!(
            most_recently_ended(&periods, hour * 60),
            expected,
            "at {hour}:00"
        );
    }
}

#[test]
fn the_four_default_periods_tile_the_day() {
    assert_eq!(tiling_fault(&Periods::default()), None);
}

#[test]
fn a_gap_and_an_overlap_are_both_refused_with_the_two_windows_named() {
    let mut gapped = Periods::default();
    gapped.morning.start = 6 * 60 + 30;
    let fault = tiling_fault(&gapped).expect("a gap is a fault");
    assert!(fault.contains("`overnight` ends at 06:00"), "{fault}");
    assert!(fault.contains("`morning` starts at 06:30"), "{fault}");

    let mut overlapping = Periods::default();
    overlapping.afternoon.start = 11 * 60;
    let fault = tiling_fault(&overlapping).expect("an overlap is a fault");
    assert!(fault.contains("`afternoon`"), "{fault}");
}

#[test]
fn yesterday_and_last_week_are_their_windows_one_step_back() {
    assert_eq!(parse_window("yesterday"), Some((Window::Today, true)));
    assert_eq!(parse_window("last-week"), Some((Window::Week, true)));
    assert_eq!(parse_window("morning"), Some((Window::Morning, false)));
    assert_eq!(parse_window("tomorrow"), None);
}

#[test]
fn every_word_the_refusal_lists_parses() {
    for word in WINDOW_WORDS {
        assert!(parse_window(word).is_some(), "{word}");
    }
}

#[test]
fn a_leap_day_survives_the_round_trip_through_the_day_count() {
    let leap_day = LocalCivilTime {
        year: 2028,
        month: 2,
        day: 29,
        hour: 0,
        minute: 0,
        second: 0,
    };
    assert_eq!(leap_day.plus_days(1).day, 1);
    assert_eq!(leap_day.plus_days(1).month, 3);
    assert_eq!(leap_day.plus_days(-1).day, 28);
}
