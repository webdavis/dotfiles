use super::*;
use crate::state_fixtures::*;

#[test]
fn a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live() {
    // REMOVAL IS CHEAP AND CREATION IS NOT, which is why one gate cannot
    // serve both. Gating the whole update on the feature switches stopped
    // the marker being CLEARED as well: a wait that ended while hue was off
    // stayed on disk, and re-enabling hue inside the backstop bound
    // put blocked back on a lamp for a session nobody is waiting on.
    let state = scratch("needs-marker-end-ungated");
    let marker = crate::marker_files::blocked_marker(&state, "s1").expect("a usable session id");
    std::fs::create_dir_all(marker.parent().expect("the needs directory"))
        .expect("the needs directory");
    std::fs::write(&marker, "1000\n").expect("a wait in progress");

    update_blocked_marker(&state, "s1", "done", false, Some(1_000));
    assert!(
        !marker.exists(),
        "the wait ended, so the marker goes, lamps live or not: it is one \
             unlink and it clears a leftover from when they were"
    );

    update_blocked_marker(&state, "s1", "blocked", false, Some(1_000));
    assert!(
        !marker.exists(),
        "but STARTING one stays gated: a machine that never asked for the \
             lamps must not accumulate files that nothing will ever sweep"
    );

    update_blocked_marker(&state, "s1", "blocked", true, Some(1_000));
    assert!(
        marker.exists(),
        "and a machine with them live starts the wait, which is what makes \
             the two assertions above a difference rather than a dead path"
    );
    assert_eq!(
        std::fs::read_to_string(&marker).expect("the marker"),
        "1000\n",
        "the marker holds the DECISION's clock, not a fresh wall-clock read \
             taken inside this function"
    );

    // NO CLOCK IS NO MARKER: an unreadable clock must not default to
    // epoch zero, which would write a marker that reads as already
    // expired the moment it lands, or that never ages out at all read
    // the other way. SEEDED, not absent: a `None` case starting with no
    // marker on disk cannot tell "correctly wrote nothing" apart from a
    // `None => remove_file(marker)` mutant, since removing a file that
    // was never there is itself a silent no-op.
    let unreadable_clock_marker =
        crate::marker_files::blocked_marker(&state, "s2").expect("a usable session id");
    std::fs::create_dir_all(
        unreadable_clock_marker
            .parent()
            .expect("the needs directory"),
    )
    .expect("the needs directory");
    std::fs::write(&unreadable_clock_marker, "999\n").expect("a wait already in progress");
    update_blocked_marker(&state, "s2", "blocked", true, None);
    assert_eq!(
        std::fs::read_to_string(&unreadable_clock_marker).expect("the marker"),
        "999\n",
        "an unreadable clock must touch no marker at all, neither writing \
             one at epoch zero nor removing the one already there"
    );
}

/// Every name one session's marker directory holds, so a claim left behind is
/// visible rather than merely absent from the marker's own path.
fn marker_names(state: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(crate::marker_files::blocked_dir(state))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn seed_wait(state: &std::path::Path, session_id: &str, at: u64) -> std::path::PathBuf {
    let marker =
        crate::marker_files::blocked_marker(state, session_id).expect("a usable session id");
    std::fs::create_dir_all(marker.parent().expect("the needs directory"))
        .expect("the needs directory");
    std::fs::write(&marker, format!("{at}\n")).expect("a wait in progress");
    marker
}

#[test]
fn an_end_older_than_the_wait_it_finds_leaves_that_wait_armed() {
    // THE ANSWERED-WAIT RACE, from the losing side. Every clearing arm is
    // asynchronous, so an End is unordered against the next
    // PermissionRequest: a Stop still condensing, or a question's own answer,
    // reaches this line after a SECOND wait has already been published, and
    // an unconditional unlink took it. The marker holds the second it was
    // armed and the caller states its own moment, so the newer wait survives.
    let state = scratch("needs-end-older-than-the-wait");
    let marker = seed_wait(&state, "s1", 2_000);

    update_blocked_marker(&state, "s1", "done", true, Some(1_999));

    assert!(
        marker.exists(),
        "a wait armed after this End's own moment is not this End's to remove"
    );
    assert_eq!(
        std::fs::read_to_string(&marker).expect("the marker"),
        "2000\n",
        "and it is restored at its own path with its own epoch, not rewritten"
    );
    assert_eq!(
        marker_names(&state),
        vec!["s1".to_string()],
        "the claim the compare was read off is not left behind as litter"
    );
}

#[test]
fn an_end_at_or_past_the_waits_own_second_removes_it() {
    // THE ORDINARY CASE, and the equal second is on the removing side: a wait
    // armed and answered inside one second is answered, and refusing there
    // would hold the lamp until the session's next event for every fast
    // answer there is.
    let state = scratch("needs-end-at-or-past-the-wait");
    for moment in [2_000, 2_001] {
        let marker = seed_wait(&state, "s1", 2_000);
        update_blocked_marker(&state, "s1", "done", true, Some(moment));
        assert!(
            !marker.exists(),
            "an End at {moment} clears a wait from 2000"
        );
        assert!(
            marker_names(&state).is_empty(),
            "and leaves no claim behind at {moment}"
        );
    }
}

#[test]
fn an_end_with_no_clock_behind_it_still_clears_the_wait() {
    // NO CLOCK IS NO COMPARE, and a removal is what this has always done: an
    // End cannot be withheld on a reading nobody has, and a wait left armed
    // by an unreadable clock would hold the lamp for the whole backstop.
    let state = scratch("needs-end-without-a-clock");
    let marker = seed_wait(&state, "s1", 2_000);
    update_blocked_marker(&state, "s1", "done", true, None);
    assert!(!marker.exists());
}

#[test]
fn an_end_reading_a_wait_it_cannot_parse_clears_it() {
    // AN UNREADABLE EPOCH IS SWEPT, in `sweep_markers`'s own reading: nothing
    // can ever age out a marker whose epoch no reader will vouch for, so
    // keeping it here is the same unbounded hold through a different door.
    let state = scratch("needs-end-unparseable-wait");
    let marker = seed_wait(&state, "s1", 2_000);
    std::fs::write(&marker, "not a second\n").expect("a marker some other hand rewrote");
    update_blocked_marker(&state, "s1", "done", true, Some(1_000));
    assert!(!marker.exists());
}
