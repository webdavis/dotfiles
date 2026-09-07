mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live() {
        // REMOVAL IS CHEAP AND CREATION IS NOT, which is why one gate cannot
        // serve both. Gating the whole update on the feature switches stopped
        // the marker being CLEARED as well: a wait that ended while hue was off
        // stayed on disk, and re-enabling hue inside the backstop bound
        // put blocked back on a lamp for a session nobody is waiting on.
        let state = scratch("needs-marker-end-ungated");
        let marker = pns::lights::blocked_marker(&state, "s1").expect("a usable session id");
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
            pns::lights::blocked_marker(&state, "s2").expect("a usable session id");
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
}
