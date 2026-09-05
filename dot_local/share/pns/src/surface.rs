//! The surface and visibility model: WHICH display the operator is looking at
//! picks the notifier, and WHETHER the origin pane is visible there decides
//! suppression. Confirmed by the operator on 2026-08-11 after live testing,
//! with newest-signal-wins added 2026-08-12; the full model and its drill
//! ladder live in the drill ledger.
//!
//! Architecture fact the model rests on: herdr is a server, and ghostty
//! (desk) and moshi (phone) are both clients presenting the same session, so
//! pane visibility is ONE session-level fact; only the surface differs.

/// Where the operator's eyes are. Picks the notifier: Desk = banner,
/// Mobile = phone card, Away = phone card. A banner NEVER fires while the
/// surface is Mobile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Desk,
    Mobile,
    Away,
}

/// Whether the origin pane can be seen on whatever client shows the session.
/// Unknown never suppresses: a notification wrongly delivered costs a glance,
/// a notification wrongly suppressed is the product failing silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Unknown,
}

/// One reading of the session's display state, from herdr's own CLI:
/// `workspace list` for the focused workspace's active tab, and `pane layout`
/// on the origin pane for that tab's id, focused pane and zoom.
///
/// EVERY FIELD HERE IS SESSION-GLOBAL, and building one from a caller-relative
/// answer is the bug class this type keeps inviting. `herdr pane current`
/// resolves against the CALLER'S `HERDR_PANE_ID`, and the caller is always the
/// pane the event fired from, so it reports the origin as focused no matter
/// what is on screen: the view then says Visible for the very pane that fired,
/// and every desk notification suppresses itself. Drill D4 found exactly that
/// on 2026-08-13. Anything addressed by an explicit pane id is safe; anything
/// meaning "mine" is not.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionView {
    /// The tab the ORIGIN pane belongs to.
    pub origin_tab: String,
    /// The tab the session is currently showing: the focused workspace's
    /// active tab.
    pub focused_tab: String,
    /// The focused pane inside the ORIGIN's tab, which is the pane on screen
    /// exactly when that tab is also the focused one, and that is the only
    /// case visibility consults it.
    pub focused_pane: String,
    /// Tab-level zoom: true means the focused pane fills the window and
    /// every sibling is hidden (operator-confirmed herdr semantics).
    pub zoomed: bool,
}

/// What one event should do, given surface, visibility and tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryPlan {
    pub banner: bool,
    pub phone_card: bool,
    /// The lights signal rides on top in EVERY >=300s scenario.
    pub pulse: bool,
}

/// Whether the origin pane can be seen on the client showing the session.
///
/// One session-level fact, because herdr is the server and every client
/// presents the same panes. Hidden needs PROOF: a different tab, or a zoom
/// that is covering this pane. Anything unreadable is Unknown, and Unknown
/// never suppresses.
pub fn visibility(origin: &str, view: &SessionView) -> Visibility {
    if origin.is_empty() || view.origin_tab.is_empty() || view.focused_tab.is_empty() {
        return Visibility::Unknown;
    }
    if view.origin_tab != view.focused_tab {
        return Visibility::Hidden;
    }
    // Zoom hides every sibling, so only the focused pane survives it.
    if view.zoomed && view.focused_pane != origin {
        return Visibility::Hidden;
    }
    Visibility::Visible
}

/// A reading still inside the freshness window, or nothing at all.
fn fresh_age(age: Option<u64>, fresh_secs: u64) -> Option<u64> {
    age.filter(|seconds| *seconds < fresh_secs)
}

/// Whether a reading is recent enough to speak for its surface.
///
/// ONE definition of fresh, exported so there is only ever one: the
/// arbitration below and the mobile-visibility rule beside it must not be able
/// to disagree about whether the phone was just used.
pub fn is_fresh(age: Option<u64>, fresh_secs: u64) -> bool {
    fresh_age(age, fresh_secs).is_some()
}

/// Where the operator's eyes are, by NEWEST SIGNAL WINS.
///
/// TWO CLOCKS OF THE SAME KIND, which is the whole amendment (operator
/// confirmed 2026-08-15). The desk reports when its keyboard was last touched
/// and the phone reports when the mosh client's pty was last WRITTEN TO by its
/// reader; both answer "how long since a human last did something here", so
/// the fresher one is where the operator is. A signal older than the freshness
/// window counts for nothing, which is what retired the marker's fixed TTL: a
/// signal holds its surface for as long as nothing newer contradicts it.
///
/// WHY THE PHONE NEEDED ITS OWN CLOCK. The reading it replaces was a
/// one-second sample of bytes moving over moshi, and passive viewing moves
/// almost none: drill D5(i) had the operator reading the session on the phone
/// while the sample came in under the floor, fresh desk input won on a desk
/// nobody was at, and the banner fired into an empty room. Input is what the
/// desk was always measuring, so measuring it on the phone too puts the two on
/// one comparable footing.
///
/// THE TAP AND THE PTY ARE ONE CLASS. A Back Tap is manual phone input by
/// another route, so it does not outrank the client's own clock and is not
/// outranked by it; the fresher of the two speaks for the phone, and that
/// combined reading is what meets the desk.
///
/// A missing reading is never fresh, so every unknown falls toward Away rather
/// than Desk: getting a card while at the desk costs a glance, missing one
/// while away costs the event.
///
/// A LOCKED SCREEN DISQUALIFIES THE DESK CLOCK and nothing else. That is
/// newest-signal-wins rather than an exception to it: locking necessarily
/// postdates the last desk input, because typing again means unlocking first,
/// so the lock is the newest fact about the desk. It is deliberately NOT a
/// blanket Away, because it says nothing about the phone: locking the laptop
/// and picking it up is the canonical case, and Away always cards while
/// Mobile lets a watched pane suppress.
///
/// ONLY `Some(true)` LOCKS. `Some(false)` and `None` leave every clock exactly
/// as it was, so a reading nobody could take costs one freshness window of the
/// behavior that shipped before this, where inventing a lock would kill the
/// desk banner permanently wherever the reading stops working.
pub fn surface(
    desk_input_age: Option<u64>,
    phone_input_age: Option<u64>,
    marker_age: Option<u64>,
    desk_fresh_secs: u64,
    screen_locked: Option<bool>,
) -> Surface {
    let fresh = |age: Option<u64>| fresh_age(age, desk_fresh_secs);
    // Smallest age is the most recent, and an unreadable one simply does not
    // compete: two ways of touching the phone, one verdict for the phone.
    let phone = [fresh(phone_input_age), fresh(marker_age)]
        .into_iter()
        .flatten()
        .min();
    let desk = fresh(desk_input_age).filter(|_| screen_locked != Some(true));
    match (desk, phone) {
        // The tie goes to the desk, where the operator has to be sitting for
        // the reading to exist at all.
        (Some(desk), Some(phone)) => {
            if desk <= phone {
                Surface::Desk
            } else {
                Surface::Mobile
            }
        }
        (Some(_), None) => Surface::Desk,
        (None, Some(_)) => Surface::Mobile,
        (None, None) => Surface::Away,
    }
}

/// The visibility the DELIVERY decision runs on, which is not always the one
/// the session reports.
///
/// A MOBILE SURFACE REACHED BY THE BACK TAP ALONE IS WATCHING NOTHING. Two
/// different things put the operator on mobile, and only one of them means a
/// screen is in front of them: the phone's pty clock says moshi is open and
/// taking input, while the tap says only that they reached for the phone. When
/// the tap is the fresher signal and the pty clock is not fresh at all, moshi
/// is not open in their hand, so the session is on screen nowhere they can see
/// it. The desk display showing the origin pane is showing it to an empty
/// chair.
///
/// Drill D6 caught exactly that on 2026-08-19: a Back Tap with moshi closed
/// produced NOTHING, because the session view answered Visible for a pane
/// focused on the unattended desk display and mobile-plus-visible suppresses.
/// The operator's confirmed mobile matrix has that row firing the card.
///
/// When the pty clock IS fresh the session view governs unchanged, because
/// moshi really is open and what it shows is what the operator sees. That is
/// the D5 behavior and this rule must never reach it.
pub fn effective_visibility(
    surface: Surface,
    phone_input_fresh: bool,
    session: Visibility,
) -> Visibility {
    if surface == Surface::Mobile && !phone_input_fresh {
        // Nothing is on screen for them, so nothing can suppress.
        return Visibility::Hidden;
    }
    session
}

/// What one event should do. The operator-confirmed matrix, as three rules.
///
/// The pulse rides on top of every long-running event, whatever else is
/// decided. The banner belongs to the desk alone and fires only when the
/// origin pane is not already on screen. The card belongs to the phone: always
/// when away, and on mobile unless the operator is watching the pane already,
/// where it takes the opt-in toggle to say anything at all.
pub fn plan(
    surface: Surface,
    visibility: Visibility,
    long_running: bool,
    mobile_watch_card: bool,
) -> DeliveryPlan {
    let watching = visibility == Visibility::Visible;
    DeliveryPlan {
        banner: surface == Surface::Desk && !watching,
        phone_card: match surface {
            Surface::Desk => false,
            Surface::Mobile => !watching || (long_running && mobile_watch_card),
            Surface::Away => true,
        },
        pulse: long_running,
    }
}

#[cfg(test)]
mod tests {
    use super::{DeliveryPlan, SessionView, Surface, Visibility};
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

    // ------------------------------------------------------------------
    // Surface: newest signal wins between two input clocks, the desk's
    // keyboard and the phone's pty (with the Back Tap as manual phone
    // input); nothing fresh means Away.
    // ------------------------------------------------------------------

    #[test]
    fn every_surface_case_in_the_matrix_arbitrates_correctly() {
        // Ages are seconds-ago; None = reading unavailable.
        // (label, desk input age, phone input age, marker age, fresh window,
        //  expected)
        type Case = (
            &'static str,
            Option<u64>,
            Option<u64>,
            Option<u64>,
            u64,
            Surface,
        );
        let matrix: [Case; 11] = [
            (
                "typing at the desk, phone untouched",
                Some(2),
                None,
                None,
                120,
                Surface::Desk,
            ),
            (
                "scrolling moshi now beats desk touched 90s ago",
                Some(90),
                Some(5),
                None,
                120,
                Surface::Mobile,
            ),
            (
                "passive viewing still wins: the pty moved, the desk did not",
                Some(600),
                Some(60),
                None,
                120,
                Surface::Mobile,
            ),
            (
                "desk input AFTER the last phone input cancels it",
                Some(5),
                Some(60),
                None,
                120,
                Surface::Desk,
            ),
            (
                "the tie goes to the desk, where the operator is sitting",
                Some(30),
                Some(30),
                None,
                120,
                Surface::Desk,
            ),
            (
                "tap newer than the last desk input wins: mobile",
                Some(300),
                None,
                Some(30),
                120,
                Surface::Mobile,
            ),
            (
                "desk input AFTER the tap cancels it",
                Some(5),
                None,
                Some(60),
                120,
                Surface::Desk,
            ),
            (
                "the tap speaks for the phone when it is the fresher of the two",
                Some(50),
                Some(100),
                Some(10),
                120,
                Surface::Mobile,
            ),
            (
                "and the pty speaks for it when IT is the fresher of the two",
                Some(50),
                Some(10),
                Some(100),
                120,
                Surface::Mobile,
            ),
            (
                "nothing fresh anywhere is away",
                Some(600),
                Some(600),
                Some(600),
                120,
                Surface::Away,
            ),
            (
                "no readings at all fails toward away, never desk",
                None,
                None,
                None,
                120,
                Surface::Away,
            ),
        ];
        for (label, desk_input_age, phone_input_age, marker_age, desk_fresh, expected) in matrix {
            assert_eq!(
                surface(
                    desk_input_age,
                    phone_input_age,
                    marker_age,
                    desk_fresh,
                    None
                ),
                expected,
                "case: {label}"
            );
        }
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

    // ------------------------------------------------------------------
    // The delivery plan: the operator-confirmed matrix, one row per rule.
    // ------------------------------------------------------------------

    #[test]
    fn every_delivery_row_in_the_confirmed_matrix_plans_correctly() {
        // (case label, surface, visibility, long_running, mobile_watch_card
        //  config, expected {banner, phone_card, pulse})
        let no = DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        };
        let matrix = [
            (
                "desk watching: suppressed entirely",
                Surface::Desk,
                Visibility::Visible,
                false,
                false,
                no,
            ),
            (
                "desk watching, long command: pulse only",
                Surface::Desk,
                Visibility::Visible,
                true,
                false,
                DeliveryPlan {
                    banner: false,
                    phone_card: false,
                    pulse: true,
                },
            ),
            (
                "desk, origin hidden: banner",
                Surface::Desk,
                Visibility::Hidden,
                false,
                false,
                DeliveryPlan {
                    banner: true,
                    phone_card: false,
                    pulse: false,
                },
            ),
            (
                "desk, origin hidden, long: banner plus pulse",
                Surface::Desk,
                Visibility::Hidden,
                true,
                false,
                DeliveryPlan {
                    banner: true,
                    phone_card: false,
                    pulse: true,
                },
            ),
            (
                "desk, visibility unknown: deliver, never suppress on doubt",
                Surface::Desk,
                Visibility::Unknown,
                false,
                false,
                DeliveryPlan {
                    banner: true,
                    phone_card: false,
                    pulse: false,
                },
            ),
            (
                "mobile watching: suppressed",
                Surface::Mobile,
                Visibility::Visible,
                false,
                false,
                no,
            ),
            (
                "mobile watching, long, card toggle OFF (default): pulse only",
                Surface::Mobile,
                Visibility::Visible,
                true,
                false,
                DeliveryPlan {
                    banner: false,
                    phone_card: false,
                    pulse: true,
                },
            ),
            (
                "mobile watching, long, card toggle ON: pulse plus card",
                Surface::Mobile,
                Visibility::Visible,
                true,
                true,
                DeliveryPlan {
                    banner: false,
                    phone_card: true,
                    pulse: true,
                },
            ),
            (
                "mobile, origin hidden: card only, banner NEVER",
                Surface::Mobile,
                Visibility::Hidden,
                false,
                false,
                DeliveryPlan {
                    banner: false,
                    phone_card: true,
                    pulse: false,
                },
            ),
            (
                "mobile, visibility unknown: card, banner still never",
                Surface::Mobile,
                Visibility::Unknown,
                false,
                false,
                DeliveryPlan {
                    banner: false,
                    phone_card: true,
                    pulse: false,
                },
            ),
            (
                "away: phone card regardless of any client's display",
                Surface::Away,
                Visibility::Visible,
                false,
                false,
                DeliveryPlan {
                    banner: false,
                    phone_card: true,
                    pulse: false,
                },
            ),
            (
                "away, long: card plus pulse",
                Surface::Away,
                Visibility::Hidden,
                true,
                false,
                DeliveryPlan {
                    banner: false,
                    phone_card: true,
                    pulse: true,
                },
            ),
        ];
        for (label, s, v, long_running, watch_card, expected) in matrix {
            assert_eq!(
                plan(s, v, long_running, watch_card),
                expected,
                "case: {label}"
            );
        }
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
}
