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
/// `pane get` for the origin pane's tab, `pane layout` for the focused pane
/// and the tab-level zoom, `workspace list` for which workspace is showing.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionView {
    /// The tab the ORIGIN pane belongs to.
    pub origin_tab: String,
    /// The tab the session is currently showing.
    pub focused_tab: String,
    /// The focused pane inside the focused tab.
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

/// Where the operator's eyes are, by NEWEST SIGNAL WINS.
///
/// Two signals compete: physical input at the desk, and the deliberate Back
/// Tap marker. Whichever happened more recently wins, and a signal older than
/// the freshness window counts for nothing, which is what retired the marker's
/// fixed TTL: a tap holds mobile for as long as nothing newer contradicts it,
/// not for five minutes.
///
/// Streaming outranks both. Bytes moving over moshi are the operator's eyes
/// on the session right now, which beats even fresh desk input, since the desk
/// reading cannot tell typing from a cat on the keyboard.
///
/// A missing reading is never fresh, so every unknown falls toward Away rather
/// than Desk: getting a card while at the desk costs a glance, missing one
/// while away costs the event.
pub fn surface(
    desk_input_age: Option<u64>,
    marker_age: Option<u64>,
    moshi_streaming: bool,
    desk_fresh_secs: u64,
) -> Surface {
    if moshi_streaming {
        return Surface::Mobile;
    }
    let fresh = |age: Option<u64>| age.filter(|seconds| *seconds < desk_fresh_secs);
    match (fresh(desk_input_age), fresh(marker_age)) {
        (Some(desk), Some(tap)) => {
            if desk <= tap {
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
    use super::{plan, surface, visibility};

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
    // Surface: newest signal wins between the Back Tap marker and desk
    // input; moshi streaming means Mobile; nothing fresh means Away.
    // ------------------------------------------------------------------

    #[test]
    fn every_surface_case_in_the_matrix_arbitrates_correctly() {
        // (case label, last desk input age secs, marker age secs,
        //  moshi streaming, desk fresh threshold, expected)
        // Ages are seconds-ago; None = reading unavailable.
        // (label, desk input age, marker age, streaming, fresh window, expected)
        type Case = (&'static str, Option<u64>, Option<u64>, bool, u64, Surface);
        let matrix: [Case; 8] = [
            (
                "typing at the desk, no tap, no streaming",
                Some(2),
                None,
                false,
                120,
                Surface::Desk,
            ),
            (
                "tap newer than the last desk input wins: mobile",
                Some(300),
                Some(30),
                false,
                120,
                Surface::Mobile,
            ),
            (
                "desk input AFTER the tap cancels it",
                Some(5),
                Some(60),
                false,
                120,
                Surface::Desk,
            ),
            (
                "moshi actively streaming is mobile even with no tap",
                Some(600),
                None,
                true,
                120,
                Surface::Mobile,
            ),
            (
                "streaming outranks a stale desk reading",
                Some(90),
                None,
                true,
                120,
                Surface::Mobile,
            ),
            (
                "nothing fresh anywhere is away",
                Some(600),
                None,
                false,
                120,
                Surface::Away,
            ),
            (
                "no readings at all fails toward away, never desk",
                None,
                None,
                false,
                120,
                Surface::Away,
            ),
            (
                "an old tap alone does not hold mobile forever",
                Some(3000),
                Some(2400),
                false,
                120,
                Surface::Away,
            ),
        ];
        for (label, desk_input_age, marker_age, streaming, desk_fresh, expected) in matrix {
            assert_eq!(
                surface(desk_input_age, marker_age, streaming, desk_fresh),
                expected,
                "case: {label}"
            );
        }
    }

    #[test]
    fn the_tap_needs_no_expiry_window_while_it_stays_the_newest_signal() {
        // Newest-signal-wins replaced the fixed five-minute marker TTL: a
        // 20-minute-old tap still means mobile when the desk has been idle
        // longer and moshi has the session open enough to matter. The tap
        // only loses to newer desk input or to full away-ness.
        assert_eq!(
            surface(Some(2000), Some(1200), true, 120),
            Surface::Mobile,
            "old tap + streaming stays mobile"
        );
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
