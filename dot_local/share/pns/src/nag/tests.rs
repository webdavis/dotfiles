//! The codec and the path builders: what a record round-trips as, and which
//! session ids name a record, a marker, a job and a claim.

use super::{
    MAX_SESSION_ID_CHARS, Record, claim_path, job_id, marker_name, parse, record_path, render,
    session_of,
};
use std::path::Path;

#[test]
fn a_record_with_every_field_set_round_trips_through_its_on_disk_form() {
    let record = Record {
        agent: "claude".to_string(),
        project: "dotfiles".to_string(),
        branch: "main".to_string(),
        // The operator's own question, carrying every character a
        // line-oriented `key=value` form would forge a second record out
        // of.
        detail: "Bash: cargo test\t\"quoted\"\nsecond line".to_string(),
        pane: "wW:p21".to_string(),
        armed: 1_700_000_000,
    };
    assert_eq!(parse(&render(&record)), Some(record));
}

#[test]
fn a_record_missing_a_key_degrades_to_a_thinner_one_and_a_line_that_is_not_json_is_refused() {
    // THE JOURNAL'S OWN RULE, applied here: every value has an empty
    // reading, so a short record still cards. A missing `armed` reads as
    // second zero, which the staleness cap refuses, so the degraded case
    // resolves to SILENCE rather than to a nudge about an unknown moment.
    assert_eq!(
        parse(r#"{"detail":"may I"}"#),
        Some(Record {
            detail: "may I".to_string(),
            ..Record::default()
        })
    );
    // And text that is not an object has nothing to degrade to.
    for refused in ["", "not json", "[1,2]", "\"a string\"", "{"] {
        assert_eq!(parse(refused), None, "{refused} is not a record");
    }
}

// --- the names ---------------------------------------------------------

#[test]
fn an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim() {
    let state = Path::new("/s");
    assert_eq!(
        record_path(state, "abc-123"),
        Some(Path::new("/s/nag/abc-123.pending").to_path_buf())
    );
    assert_eq!(marker_name("abc-123"), Some("nag-abc-123".to_string()));
    assert_eq!(job_id("abc-123"), Some("nag:abc-123".to_string()));
    // AND BACK: the fire enumerates files and has only the name to work
    // from, so the record's own name is where the session id comes from.
    assert_eq!(session_of("abc-123.pending"), Some("abc-123".to_string()));
    assert_eq!(session_of("abc-123"), None, "a record ends in .pending");
    assert_eq!(
        session_of("abc-123.pending.claim.7"),
        None,
        "a claim is outside the enumeration the fire matches on"
    );
}

#[test]
fn a_session_id_that_cannot_be_a_filename_names_nothing_at_all() {
    // FAIL IN THE SAFE DIRECTION, which here is arming nothing: an id that
    // cannot become a name is one no record, marker or job is written for.
    let state = Path::new("/s");
    for refused in [
        "",
        "..",
        "../etc/passwd",
        "a/b",
        ".hidden",
        "a\u{7}b",
        "a\nb",
        "a b",
        "a:b",
        &"x".repeat(MAX_SESSION_ID_CHARS + 1),
    ] {
        assert_eq!(
            record_path(state, refused),
            None,
            "{refused:?} is not a name"
        );
        assert_eq!(marker_name(refused), None, "{refused:?} is not a name");
        assert_eq!(job_id(refused), None, "{refused:?} is not a name");
    }
    // THE CEILING IS EXACTLY WHERE THE DAEMON STOPS. `nag:<id>` is the job
    // id and `nag-<id>` the marker, and both are refused past the daemon's
    // own cap: a longer id would write a record no registration could ever
    // schedule a nudge for, so it is refused where it is cheap.
    assert!(record_path(state, &"x".repeat(MAX_SESSION_ID_CHARS)).is_some());
}

#[test]
fn two_ids_that_differ_only_after_a_dot_claim_two_different_names() {
    // THE ROW THAT MATTERS. A claim name taken from anything but the WHOLE
    // file name collapses `a.b` and `a.c` onto one claim: one session loses
    // its nudge and the other can be delivered twice. Dots are legal in a
    // harness session id (`session_id_is_safe` admits them), so this is a
    // real pair rather than a hypothetical one.
    let state = Path::new("/s");
    let first = claim_path(&record_path(state, "a.b").expect("a.b names a record"), 7);
    let second = claim_path(&record_path(state, "a.c").expect("a.c names a record"), 7);
    assert_ne!(first, second);
    assert_eq!(
        first,
        Path::new("/s/nag/a.b.pending.claim.7").to_path_buf(),
        "the whole file name, suffix and all, carries into the claim"
    );
}
