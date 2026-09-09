//! The nag policy's own tests: what the card says, when a record is too old,
//! and what the fire decides about one.
//!
//! The codec's tests and the three name tests stay in the legacy package with
//! the path builders they drive.

use super::{Dropped, Fate, Record, fate, is_stale, nudge};

// --- what the card says ------------------------------------------------

#[test]
fn one_approval_is_nudged_with_its_own_question_and_how_long_it_has_waited() {
    assert_eq!(
        nudge(1, 300, "Bash: cargo test"),
        "still waiting 5m: Bash: cargo test"
    );
    // THE UNIT A HUMAN WOULD SAY IT IN, which under a minute is seconds:
    // the floor is thirty, so a drill really does read in seconds.
    assert_eq!(
        nudge(1, 45, "Bash: cargo test"),
        "still waiting 45s: Bash: cargo test"
    );
    // A record whose detail never arrived says the waiting and stops: a
    // trailing separator with nothing after it reads as a truncated card.
    assert_eq!(nudge(1, 300, ""), "still waiting 5m");
}

#[test]
fn several_approvals_are_one_card_naming_the_count_and_no_question_at_all() {
    // THE OPERATOR'S COALESCING RULING. Naming ONE question implies it is
    // THE one, and the card is capped on the phone anyway, so the multi
    // case names none.
    let said = nudge(3, 720, "Bash: cargo test");
    assert_eq!(said, "3 approvals waiting, oldest 12m");
    assert!(
        !said.contains("cargo test"),
        "no question text reaches a coalesced card"
    );
    // AND BOTH ARE STATEMENTS, NEVER QUESTIONS, which is what keeps the
    // card from reading as a second answerable prompt: a nudge goes through
    // `run_event` and structurally cannot carry Allow and Deny.
    assert!(!said.contains('?'));
    assert!(!nudge(1, 300, "Bash: cargo test").contains('?'));
}

// --- how old is too old ------------------------------------------------

#[test]
fn a_record_is_too_old_in_both_directions_and_never_in_only_one() {
    const AFTER: u64 = 300;
    const NOW: u64 = 1_700_000_000;
    for (case, armed, stale) in [
        ("armed a hundred seconds ago", NOW - 100, false),
        ("armed exactly at the cap", NOW - 2 * AFTER, false),
        ("armed one second past the cap", NOW - 2 * AFTER - 1, true),
        ("armed last night", NOW - 7_200, true),
        // BUG CLASS 2, and the half a one-sided implementation passes
        // without: a clock that moved backwards, or a hand-edited epoch,
        // must not read as fresh forever.
        ("armed one second in the future", NOW + 1, true),
        ("armed far in the future", NOW + 86_400, true),
    ] {
        assert_eq!(is_stale(armed, NOW, AFTER), stale, "{case}");
    }
}

// --- what the fire decides about one record ----------------------------

#[test]
fn a_record_is_counted_only_when_nothing_says_otherwise() {
    const AFTER: u64 = 300;
    const NOW: u64 = 1_700_000_000;
    let fresh = Record {
        armed: NOW - 100,
        ..Record::default()
    };
    let old = Record {
        armed: NOW - 7_200,
        ..Record::default()
    };
    for (case, record, marker, expected) in [
        ("nothing says otherwise", Some(&fresh), false, Fate::Count),
        (
            "the marker arrived while we were waking",
            Some(&fresh),
            true,
            Fate::Drop(Dropped::Answered),
        ),
        (
            "no marker, but the moment has passed",
            Some(&old),
            false,
            Fate::Drop(Dropped::Stale),
        ),
        // THE MARKER OUTRANKS THE CAP, so an approval that was answered is
        // reported as answered rather than as merely old.
        ("both", Some(&old), true, Fate::Drop(Dropped::Answered)),
        (
            "nothing readable was there at all",
            None,
            false,
            Fate::Drop(Dropped::Unreadable),
        ),
    ] {
        assert_eq!(fate(record, marker, NOW, AFTER), expected, "{case}");
    }
}
