//! The three predicates over a `Decision`: what counts as missed, what counts
//! as the operator's return, and what proves they were here.

use super::{is_present, should_replay, was_missed};
use crate::decision::{Decision, GateInputs, Overrides};
use crate::surface::{DeliveryPlan, Surface, Visibility};

/// A decision over the three values the predicate reads, with every other
/// reading absent. NOTHING HERE IS A DOUBLE: `Decision` and `GateInputs`
/// are the crate's own value types and the predicate is a total function
/// of them, so a test states the values rather than driving a probe to
/// produce them.
fn decided(surface: Surface, visibility: Visibility, plan: DeliveryPlan) -> Decision {
    Decision {
        legs: Vec::new(),
        plan,
        pane_dropped: false,
        inputs: GateInputs {
            desk_input_age: None,
            phone_input_age: None,
            marker_age: None,
            screen_locked: None,
            desk_fresh_secs: None,
            surface,
            // The two agree everywhere except the Back Tap row, which
            // states its own disagreement below.
            session_visibility: visibility,
            visibility,
            now_secs: Some(1_756_500_000),
            long_running: false,
            mobile_watch_card: false,
            local_only: false,
            remote_only: false,
            pane_present: true,
        },
    }
}

/// A plan that decorated nothing, which is what a mute leaves behind.
const NOTHING: DeliveryPlan = DeliveryPlan {
    banner: false,
    phone_card: false,
    pulse: false,
};

#[test]
fn a_plan_that_decorated_nothing_with_nobody_watching_the_pane_is_missed() {
    // THE CASE THE JOURNAL EXISTS FOR: the plan said nothing and the
    // operator was not looking at the pane, so the event reached them
    // through no surface at all.
    assert!(was_missed(
        &decided(Surface::Desk, Visibility::Hidden, NOTHING),
        &Overrides::default()
    ));
}

#[test]
fn a_plan_that_decorated_something_is_not_missed_whichever_decoration_it_was() {
    // THE PLAN AFTER ARBITRATION, never the matrix underneath it: the
    // banner and the card are two separate ways the operator was told,
    // and either one on its own is a delivery.
    let banner = DeliveryPlan {
        banner: true,
        ..NOTHING
    };
    assert!(!was_missed(
        &decided(Surface::Desk, Visibility::Hidden, banner),
        &Overrides::default()
    ));
    let card = DeliveryPlan {
        phone_card: true,
        ..NOTHING
    };
    assert!(!was_missed(
        &decided(Surface::Away, Visibility::Hidden, card),
        &Overrides::default()
    ));
}

#[test]
fn an_event_suppressed_while_the_pane_was_on_screen_is_not_missed() {
    // THE ROW THAT KILLS THE NAIVE PREDICATE. Nothing was decorated here
    // either, but the operator was looking straight at the pane the event
    // came from, which is why the matrix suppressed it in the first place.
    for surface in [Surface::Desk, Surface::Mobile] {
        assert!(
            !was_missed(
                &decided(surface, Visibility::Visible, NOTHING),
                &Overrides::default()
            ),
            "{surface:?} watching the origin pane"
        );
    }
}

#[test]
fn an_away_event_is_missed_even_when_the_session_reported_the_pane_visible() {
    // A DESK DISPLAY SHOWING THE ORIGIN PANE TO AN EMPTY CHAIR is exactly
    // the reading that must not suppress. `surface::plan` consults
    // visibility only in its Desk and Mobile arms, so the surface half of
    // this clause is that rule restated rather than a second rule.
    assert!(was_missed(
        &decided(Surface::Away, Visibility::Visible, NOTHING),
        &Overrides::default()
    ));
}

#[test]
fn a_card_skipped_because_another_route_already_raised_one_is_not_missed() {
    // DELIVERED BY ANOTHER ROUTE, never missed: the environment sets
    // `PNS_SKIP_PHONE` exactly when the moshi approval forward really
    // happened, so an approval is already sitting on the phone. Replaying
    // a stale approval card later would be actively wrong.
    let skipped = Overrides {
        skip_phone: true,
        ..Overrides::default()
    };
    for surface in [Surface::Desk, Surface::Mobile, Surface::Away] {
        for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
            assert!(
                !was_missed(&decided(surface, visibility, NOTHING), &skipped),
                "{surface:?} / {visibility:?}"
            );
        }
    }
}

#[test]
fn a_muted_event_the_surface_would_have_decorated_is_the_journals_queue() {
    // THE MUTE'S QUEUE, which is what this file mostly holds: the mute
    // zeroes the plan LAST, after the matrix already decided to decorate,
    // so the predicate never reads `muted` itself and reads the plan the
    // mute left behind instead.
    let muted = Overrides {
        muted: true,
        ..Overrides::default()
    };
    // A desk with the pane out of sight would have had a banner.
    assert!(was_missed(
        &decided(Surface::Desk, Visibility::Hidden, NOTHING),
        &muted
    ));
    // Away would have had a card.
    assert!(was_missed(
        &decided(Surface::Away, Visibility::Hidden, NOTHING),
        &muted
    ));
    // THE BACK TAP ROW, and the reason the predicate reads `visibility`
    // rather than `session_visibility`: the operator tapped the phone with
    // moshi closed, so the session still reports the pane Visible while
    // nothing is on screen for them. `effective_visibility` has already
    // rewritten that to Hidden, and reading the session's own answer here
    // would call an empty screen a watched one.
    let back_tap = Decision {
        inputs: crate::decision::GateInputs {
            session_visibility: Visibility::Visible,
            visibility: Visibility::Hidden,
            ..decided(Surface::Mobile, Visibility::Hidden, NOTHING).inputs
        },
        ..decided(Surface::Mobile, Visibility::Hidden, NOTHING)
    };
    assert!(was_missed(&back_tap, &muted));
}

// --- the replay condition ----------------------------------------------

/// A plan that decorated the desk.
const BANNER: DeliveryPlan = DeliveryPlan {
    banner: true,
    phone_card: false,
    pulse: false,
};

/// A plan that decorated the phone.
const CARD: DeliveryPlan = DeliveryPlan {
    banner: false,
    phone_card: true,
    pulse: false,
};

#[test]
fn a_decision_that_earned_a_banner_at_the_desk_says_replay() {
    // THE RETURN TRANSITION IS THIS EVENT. A banner fired means the
    // operator is at the desk with something on screen for them, which is
    // the moment a queued notification can be perceived.
    assert!(should_replay(&decided(
        Surface::Desk,
        Visibility::Hidden,
        BANNER
    )));
}

#[test]
fn a_decision_that_earned_a_card_on_mobile_says_replay() {
    // THE SAME RULE ON THE OTHER SURFACE. A card fired means the phone in
    // the operator's hand just lit up, so the queue can ride along.
    assert!(should_replay(&decided(
        Surface::Mobile,
        Visibility::Hidden,
        CARD
    )));
}

#[test]
fn an_away_decision_never_says_replay_however_much_it_carded() {
    // AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED.
    // The Away row always cards, so without this clause the journal is
    // flushed at the phone of an operator who has not come back, which is
    // the opposite of what "return" means. Every visibility, because an
    // away operator is watching nothing whatever the session reported.
    for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
        assert!(
            !should_replay(&decided(Surface::Away, visibility, CARD)),
            "{visibility:?}"
        );
        assert!(
            !should_replay(&decided(Surface::Away, visibility, BANNER)),
            "{visibility:?}"
        );
    }
}

#[test]
fn a_decision_that_decorated_nothing_says_no_replay() {
    // ONE CLAUSE, TWO PROPERTIES. A mute zeroes the plan, so a muted run
    // cannot flush the queue it is filling; and a run whose plan decorated
    // nothing is exactly a run that JOURNALS, so no event can ever replay
    // its own miss. The two are mutually exclusive by construction rather
    // than by an ordering rule at the record site.
    for surface in [Surface::Desk, Surface::Mobile] {
        for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
            assert!(
                !should_replay(&decided(surface, visibility, NOTHING)),
                "{surface:?} / {visibility:?}"
            );
        }
    }
}

// --- the presence marker's own predicate --------------------------------

#[test]
fn every_surface_but_away_proves_the_operator_was_here() {
    // AWAY IS THE ONLY THING THAT DOES NOT COUNT. Desk and Mobile are both
    // a human within reach of a screen, and Away is the state the whole
    // recap exists to bracket.
    for surface in [Surface::Desk, Surface::Mobile] {
        for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
            assert!(
                is_present(&decided(surface, visibility, NOTHING)),
                "{surface:?} / {visibility:?}"
            );
        }
    }
}

#[test]
fn an_away_decision_never_moves_the_windows_near_edge() {
    // VISIBILITY IS DELIBERATELY NOT READ, on either side of this. An
    // operator at the desk looking at a different pane is still present,
    // and reading visibility here would make the window's edge depend on
    // which pane happened to fire.
    for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
        assert!(
            !is_present(&decided(Surface::Away, visibility, CARD)),
            "{visibility:?}"
        );
    }
}
