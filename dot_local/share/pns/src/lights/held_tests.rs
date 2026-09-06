//! The lamps, pinned: held.

use super::fixtures::*;

// --- the blocked lamp ---------------------------------------------------

#[test]
fn a_live_wait_holds_the_blocked_lamp_and_an_abandoned_one_stops_holding_it() {
    const BOUND: u64 = 1_800;
    assert!(
        any_blocked(&[NOW - 5_000, NOW - 400], NOW, BOUND),
        "one live marker among expired ones is a wait"
    );
    assert!(
        any_blocked(&[NOW - BOUND], NOW, BOUND),
        "exactly at the bound is still live: both edges closed"
    );
    assert!(
        !any_blocked(&[NOW - BOUND - 1], NOW, BOUND),
        "one second past it, an abandoned session can no longer hold a lamp blocked"
    );
    assert!(!any_blocked(&[], NOW, BOUND), "no marker is no wait");
    // A MARKER FROM THE FUTURE IS LIVE. A clock that stepped backwards is
    // not a wait that ended, and the saturating subtraction reads it as
    // zero seconds old rather than as an age that would delete it.
    assert!(any_blocked(&[NOW + 500], NOW, BOUND));
}

// --- per-lamp arbitration -----------------------------------------------

/// Every held state at once, which is what makes the ranking observable.
const ALL_HELD: House = House {
    blocked: true,
    looping: true,
    unread: Some(Unread::Failure),
};

fn shows(behaviours: &[Behaviour]) -> Vec<Behaviour> {
    behaviours.to_vec()
}

#[test]
fn every_held_state_is_active_at_once_and_they_rank_blocked_loop_then_unread() {
    assert_eq!(
        active_held(&ALL_HELD),
        vec![Held::Blocked, Held::Looping, Held::UnreadFailure],
        "the house holds all of them at once, most urgent first"
    );
    assert_eq!(
        active_held(&House {
            unread: Some(Unread::Success),
            ..ALL_HELD
        }),
        vec![Held::Blocked, Held::Looping, Held::UnreadSuccess],
        "and the unread flavour is the one the arming answered"
    );
    assert_eq!(
        active_held(&House::default()),
        Vec::new(),
        "a house holding nothing is a dark house"
    );
}

#[test]
fn one_lamp_shows_the_most_urgent_state_it_is_routed_for_and_nothing_it_is_not() {
    let active = active_held(&ALL_HELD);
    assert_eq!(
        shown(&active, &shows(&[Behaviour::Blocked, Behaviour::Unread])),
        Some(Held::Blocked),
        "a lamp routed for both shows the more urgent"
    );
    assert_eq!(
        shown(&active, &shows(&[Behaviour::Unread])),
        Some(Held::UnreadFailure),
        "a lamp routed for only the calmer one shows that, which is how one \
         house state reaches two lamps saying different things"
    );
    assert_eq!(
        shown(&active, &shows(&[Behaviour::Done, Behaviour::Failed])),
        None,
        "a pulse-only lamp holds no state at all"
    );
    assert_eq!(
        shown(&[], &shows(&[Behaviour::Blocked])),
        None,
        "and a routed lamp with nothing active is dark"
    );
}

#[test]
fn a_pulse_fires_on_a_lamp_it_is_routed_for_unless_a_held_state_has_that_lamp() {
    const FREE: bool = false;
    const HELD: bool = true;
    assert!(
        pulse_fires(
            &shows(&[Behaviour::Done, Behaviour::Failed]),
            Behaviour::Done,
            FREE
        ),
        "a routed lamp with no state on it flashes"
    );
    assert!(
        !pulse_fires(&shows(&[Behaviour::Done]), Behaviour::Failed, FREE),
        "and a lamp routed for one pulse does not carry the other"
    );
    // THE DEDICATED LAMP, which is the operator's "it helps out when free"
    // ruling generalised: it joins the pulse lamps whenever no held state
    // has it, and stops the moment one does.
    assert!(
        !pulse_fires(
            &shows(&[Behaviour::Done, Behaviour::Blocked]),
            Behaviour::Done,
            HELD
        ),
        "a held state preempts the pulse on the lamp that is holding it"
    );
    assert!(
        !pulse_fires(&shows(&[Behaviour::Blocked]), Behaviour::Done, FREE),
        "and a lamp that is not routed for the pulse never flashes, held or free"
    );
}
