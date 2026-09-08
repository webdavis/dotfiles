//! The two exhaustive tables: which surface a pair of clocks arbitrates to,
//! and what one event is planned to do from surface, visibility and tier.
//! The individual rules behind them are pinned in `rules`.

use super::{DeliveryPlan, Surface, Visibility, plan, surface};

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
