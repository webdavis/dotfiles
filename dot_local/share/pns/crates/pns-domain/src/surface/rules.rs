//! One rule per test: what visibility reads, where the freshness window
//! closes, what a locked screen disqualifies, when a mobile surface is
//! watching nothing, and the properties every plan row obeys. The two
//! exhaustive tables are in `matrix`.

use super::{SessionView, Surface, Visibility};
use super::{effective_visibility, is_fresh, plan, surface, visibility};

// ------------------------------------------------------------------
// Visibility: one session-level fact, computed from herdr's own view.
// ------------------------------------------------------------------

fn view(origin_tab: &str, focused_tab: &str, focused_pane: &str, zoomed: bool) -> SessionView {
    SessionView {
        origin_tab: origin_tab.to_string(),
        focused_tab: focused_tab.to_string(),
        focused_pane: focused_pane.to_string(),
        zoomed,
    }
}

#[test]
fn every_visibility_case_in_the_matrix_reads_correctly() {
    // (case label, origin pane, view, expected)
    let matrix = [
        (
            "origin is the focused pane",
            "t1:p1",
            view("t1", "t1", "t1:p1", false),
            Visibility::Visible,
        ),
        (
            "origin is the focused pane and zoomed",
            "t1:p1",
            view("t1", "t1", "t1:p1", true),
            Visibility::Visible,
        ),
        (
            "unzoomed sibling in the focused tab",
            "t1:p1",
            view("t1", "t1", "t1:p2", false),
            Visibility::Visible,
        ),
        (
            "sibling hidden behind another pane's zoom",
            "t1:p1",
            view("t1", "t1", "t1:p2", true),
            Visibility::Hidden,
        ),
        (
            "different tab entirely",
            "t1:p1",
            view("t1", "t2", "t2:p9", false),
            Visibility::Hidden,
        ),
        (
            "different tab, zoom irrelevant",
            "t1:p1",
            view("t1", "t2", "t2:p9", true),
            Visibility::Hidden,
        ),
    ];
    for (label, origin, session_view, expected) in matrix {
        assert_eq!(visibility(origin, &session_view), expected, "case: {label}");
    }
}

#[test]
fn a_session_view_that_cannot_be_read_is_unknown_never_visible() {
    // The probe failing (herdr down, pane gone) must never SUPPRESS a
    // notification: Unknown routes like Hidden at plan time, but is its
    // own reading so the arbitration cannot mistake it for a positive.
    assert_eq!(
        visibility("t1:p1", &view("", "", "", false)),
        Visibility::Unknown
    );
}

#[test]
fn an_empty_origin_pane_reads_unknown() {
    // Events with no --pane carry no origin; nothing can be "watched".
    assert_eq!(
        visibility("", &view("t1", "t1", "t1:p1", false)),
        Visibility::Unknown
    );
}

#[test]
fn an_age_of_exactly_the_window_is_already_stale_because_the_window_is_half_open() {
    // THE BOUNDARY SECOND ITSELF, which no other case reaches: the matrix
    // above runs ages of 2 to 600 against a window of 120 and never one
    // AT 120, so `<` and `<=` agree on every row of it. They disagree
    // here, and the half-open reading is the one the window is documented
    // with: an age is fresh while it is strictly under.
    assert!(is_fresh(Some(119), 120));
    assert!(!is_fresh(Some(120), 120));

    // And the arbitration reads the same boundary, so the two cannot come
    // apart: a desk clock at exactly the window is out of the running,
    // which with nothing else fresh is Away rather than Desk.
    assert_eq!(surface(Some(119), None, None, 120, None), Surface::Desk);
    assert_eq!(surface(Some(120), None, None, 120, None), Surface::Away);
}

#[test]
fn a_phone_signal_needs_no_expiry_window_while_it_stays_the_newest_one() {
    // Newest-signal-wins replaced the fixed five-minute marker TTL: an
    // hour-old session still reads mobile while the phone's clock is the
    // fresher of the two, and only newer desk input or full staleness
    // takes it back.
    assert_eq!(
        surface(Some(2000), Some(30), Some(3600), 120, None),
        Surface::Mobile,
        "a long-open session whose pty just moved is still mobile"
    );
}

#[test]
fn a_phone_reading_that_could_not_be_taken_never_counts_as_fresh() {
    // The discovery chain has four steps and any of them can come back
    // with nothing. Reading that as "just used" would park the operator
    // on a phone that is not in their hand and silence every banner.
    assert_eq!(surface(Some(5), None, None, 120, None), Surface::Desk);
    assert_eq!(surface(Some(600), None, None, 120, None), Surface::Away);
    assert_eq!(surface(None, None, None, 120, None), Surface::Away);
}

#[test]
fn a_stale_phone_reading_loses_to_the_desk_rather_than_holding_mobile() {
    // Mosh sessions outlive the attention paid to them: the client stays
    // attached for days. Presence is the pty's CLOCK, never the session's
    // existence, so an attached-but-untouched session decides nothing.
    assert_eq!(
        surface(Some(5), Some(9_000), None, 120, None),
        Surface::Desk
    );
    assert_eq!(
        surface(Some(9_000), Some(9_000), None, 120, None),
        Surface::Away
    );
}

// ------------------------------------------------------------------
// The screen lock disqualifies the DESK CLOCK, and nothing else.
// ------------------------------------------------------------------

#[test]
fn a_locked_screen_takes_the_desk_out_of_the_running_however_fresh_its_clock_is() {
    // The whole point: a lock necessarily postdates the last desk input,
    // because typing again means unlocking first. So it is the newest
    // fact about the desk, and the freshness window under it says nothing.
    assert_eq!(
        surface(Some(2), None, None, 120, Some(true)),
        Surface::Away,
        "keyboard touched two seconds ago, then locked: nobody is there"
    );
}

#[test]
fn a_locked_screen_with_a_fresh_pty_clock_is_still_the_phone_and_never_away() {
    // The canonical case the blanket-Away reading gets wrong: lock the
    // laptop, pick up the phone. Away always cards, while Mobile lets a
    // pane the operator is already watching on moshi suppress, so the two
    // are not interchangeable.
    assert_eq!(
        surface(Some(2), Some(5), None, 120, Some(true)),
        Surface::Mobile,
        "the lock speaks for the desk alone; the phone still answers"
    );
}

#[test]
fn a_locked_screen_with_a_fresh_back_tap_is_still_the_phone_and_never_away() {
    // The tap is manual phone input by another route, so it speaks for
    // the phone on exactly the terms the pty clock does. A lock must not
    // demote it.
    assert_eq!(
        surface(Some(2), None, Some(5), 120, Some(true)),
        Surface::Mobile,
        "tapped after locking: they reached for the phone"
    );
}

#[test]
fn an_unlocked_or_unreadable_console_leaves_every_verdict_exactly_as_it_was() {
    // THE FAIL DIRECTION, pinned. `None` is "nobody could read the
    // console", and treating it as locked would kill the desk banner for
    // good on any machine where the key is renamed or dropped, where
    // treating it as unlocked costs one freshness window of the behavior
    // that shipped before the override existed.
    // (label, desk age, phone age, marker age, the verdict both readings owe)
    type Case = (&'static str, Option<u64>, Option<u64>, Option<u64>, Surface);
    let matrix: [Case; 3] = [
        (
            "fresh desk, nothing else",
            Some(2),
            None,
            None,
            Surface::Desk,
        ),
        ("fresher phone", Some(90), Some(5), None, Surface::Mobile),
        (
            "nothing fresh",
            Some(600),
            Some(600),
            Some(600),
            Surface::Away,
        ),
    ];
    for (label, desk, phone, marker, expected) in matrix {
        for reading in [None, Some(false)] {
            assert_eq!(
                surface(desk, phone, marker, 120, reading),
                expected,
                "case: {label}, lock reading {reading:?}"
            );
        }
    }
}

// ------------------------------------------------------------------
// Effective visibility: a mobile surface the Back Tap alone reached is
// watching nothing, whatever any client's display shows (drill D6).
// ------------------------------------------------------------------

#[test]
fn every_effective_visibility_case_adjusts_or_passes_through_correctly() {
    // (case label, surface, the phone's pty clock is fresh, what the
    //  session reports, what the delivery decision must run on)
    let matrix = [
        (
            "D6 REPRO: tapped with moshi closed, pane on the unattended desk",
            Surface::Mobile,
            false,
            Visibility::Visible,
            Visibility::Hidden,
        ),
        (
            "tapped with moshi closed and no readable view at all",
            Surface::Mobile,
            false,
            Visibility::Unknown,
            Visibility::Hidden,
        ),
        (
            "D5 GUARD: moshi open on the pane itself, which really is watched",
            Surface::Mobile,
            true,
            Visibility::Visible,
            Visibility::Visible,
        ),
        (
            "moshi open, showing another tab: hidden either way",
            Surface::Mobile,
            true,
            Visibility::Hidden,
            Visibility::Hidden,
        ),
        (
            "the desk is never adjusted: a cold phone says nothing about it",
            Surface::Desk,
            false,
            Visibility::Visible,
            Visibility::Visible,
        ),
        (
            "and away is never adjusted either",
            Surface::Away,
            false,
            Visibility::Visible,
            Visibility::Visible,
        ),
    ];
    for (label, surface, phone_input_fresh, session, expected) in matrix {
        assert_eq!(
            effective_visibility(surface, phone_input_fresh, session),
            expected,
            "case: {label}"
        );
    }
}

#[test]
fn the_rule_rewrites_nothing_but_a_mobile_surface_the_phone_never_earned() {
    // The blast radius, measured over the whole input space rather than
    // argued from the rows above: eighteen combinations, and only the
    // three with a mobile surface and a cold pty clock may come back
    // saying anything other than what the session reported.
    for surface in [Surface::Desk, Surface::Mobile, Surface::Away] {
        for phone_input_fresh in [false, true] {
            for session in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
                let adjusted = effective_visibility(surface, phone_input_fresh, session);
                let may_be_rewritten = surface == Surface::Mobile && !phone_input_fresh;
                assert!(
                    may_be_rewritten || adjusted == session,
                    "{surface:?}, pty fresh {phone_input_fresh}, {session:?} \
                     was rewritten to {adjusted:?}"
                );
            }
        }
    }
}

#[test]
fn a_back_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_on_screen() {
    // Drill D6, 2026-08-19: the tap put the surface on mobile, the origin
    // pane sat focused on the desk display nobody was at, and the card
    // never fired. Row 1 of the operator's confirmed mobile matrix says
    // a tap with moshi never opened has to produce one.
    let delivery = plan(
        Surface::Mobile,
        effective_visibility(Surface::Mobile, false, Visibility::Visible),
        false,
        false,
    );
    assert!(delivery.phone_card, "the tap asked for the card");
    assert!(!delivery.banner, "and mobile never banners");
}

#[test]
fn moshi_open_on_the_origin_pane_still_suppresses_the_card() {
    // The D5 guard, in the same composed form: the D6 rule must not reach
    // the case that already passed its drill. A card describing the pane
    // filling the phone's screen is the noise the model exists to remove.
    let delivery = plan(
        Surface::Mobile,
        effective_visibility(Surface::Mobile, true, Visibility::Visible),
        false,
        false,
    );
    assert!(!delivery.phone_card, "the pane is already on the phone");
}

#[test]
fn no_plan_row_can_ever_banner_on_the_mobile_surface() {
    // The property behind the matrix: terminal-notifier NEVER fires
    // while the operator is on mobile, whatever the other inputs say.
    for v in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
        for long_running in [false, true] {
            for watch_card in [false, true] {
                let p = plan(Surface::Mobile, v, long_running, watch_card);
                assert!(
                    !p.banner,
                    "mobile bannered: visibility {v:?}, long {long_running}, card {watch_card}"
                );
            }
        }
    }
}

#[test]
fn every_long_running_row_pulses_whatever_else_it_decides() {
    for s in [Surface::Desk, Surface::Mobile, Surface::Away] {
        for v in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
            assert!(
                plan(s, v, true, false).pulse,
                "no pulse on long-running: {s:?} {v:?}"
            );
        }
    }
}
