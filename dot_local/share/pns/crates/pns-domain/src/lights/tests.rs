//! The working-file grammar's own tests: which names are one run's private
//! working file, and which are the markers a sweep is there to judge.

use super::working_owner;

#[test]
fn a_working_file_is_told_from_a_marker_by_the_process_id_that_owns_it() {
    // THE COLLISION THIS EXISTS TO CLOSE. Pane ids and session ids are
    // opaque words from another program and both alphabets admit a dot, so
    // a name matched on the bare suffix put a real marker beyond every
    // sweep: it aged out never and its lamp could not be released.
    assert_eq!(working_owner("s1.new.4321"), Some("4321"));
    assert_eq!(working_owner("wW:p21.sweep.99"), Some("99"));
    assert_eq!(
        working_owner("a.new.b"),
        None,
        "a pane whose own name spells the suffix is a MARKER, not a publish"
    );
    for marker in [
        "s1",
        "wW:p21",
        "a.new.",
        "a.new.0",
        "a.sweep.-1",
        "a.new.b.c",
    ] {
        assert_eq!(working_owner(marker), None, "{marker:?} is a marker");
    }
}

#[test]
fn a_working_file_is_told_by_its_rightmost_suffix_not_its_first() {
    // A MARKER NAMED FOR THE SUFFIX ITSELF SITS TO THE LEFT of the sweep's
    // own working file on that marker: `a.new.b` is the marker, and the
    // sweep taking it writes `a.new.b.sweep.<pid>` beside it. The first
    // `rsplit_once` this used to run found `.new.` and stopped there,
    // reading the marker's own name as the owner and failing to parse it
    // as a pid, so the sweep's working file was judged a marker too and
    // was never collected: one abandoned run leaks a working file forever.
    assert_eq!(
        working_owner("a.new.b.sweep.1"),
        Some("1"),
        "the sweep's working file on a marker shaped like a publish"
    );
    assert_eq!(
        working_owner("a.sweep.1.new.2"),
        Some("2"),
        "and the reverse nesting reads the same way"
    );
    assert_eq!(
        working_owner("x.sweep.1"),
        Some("1"),
        "a plain sweep working file is unaffected"
    );
    // THE SAME SUFFIX TWICE, which is the one shape the two cases above
    // cannot judge: each of them carries one `.new.` and one `.sweep.`, so
    // `rfind` and `find` return the same offset for both and the rightmost
    // rule is decided by the comparison alone. Here the comparison has
    // nothing to do and the SEARCH DIRECTION is the whole answer: `find`
    // stops at the left occurrence, reads `b.new.5` as the owner, fails to
    // parse it as a pid and calls a real working file a marker.
    assert_eq!(
        working_owner("a.new.b.new.5"),
        Some("5"),
        "a publish on a marker whose own name spells the publish suffix"
    );
    assert_eq!(
        working_owner("a.sweep.b.sweep.7"),
        Some("7"),
        "and the sweep suffix reads the same way"
    );
}
