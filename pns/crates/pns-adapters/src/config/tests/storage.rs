use super::*;

// --- [storage] --------------------------------------------------------------

/// The write lock's bound, on `[stale] escalate_after`'s terms: a duration
/// string, its own range, and zero carved out as a statement rather than an
/// error.
///
/// IT REPLACED A TEST-ONLY ENVIRONMENT VARIABLE that production code read on
/// every connection, so the sub-second values a test needs must survive the
/// parse whole rather than through a seconds count that would read them as
/// zero.
#[test]
fn the_busy_deadline_defaults_to_five_seconds_and_is_kept_to_the_millisecond() {
    assert_eq!(
        parse_config("").unwrap().storage_busy_deadline,
        Duration::from_secs(5),
        "a file with no table carries the shipped bound"
    );
    assert_eq!(
        parse_config("[storage]\n").unwrap().storage_busy_deadline,
        Duration::from_secs(5),
        "and so does a table with nothing said"
    );
    assert_eq!(
        parse_config("[storage]\nbusy_deadline = \"250ms\"\n")
            .unwrap()
            .storage_busy_deadline,
        Duration::from_millis(250),
        "a sub-second bound survives whole"
    );
    assert_eq!(
        parse_config("[storage]\nbusy_deadline = \"0s\"\n")
            .unwrap()
            .storage_busy_deadline,
        Duration::ZERO,
        "and zero is SQLite's own \"do not wait\" rather than a refusal"
    );
}

#[test]
fn a_busy_deadline_outside_the_range_is_refused_by_name() {
    for outside in ["\"9ms\"", "\"61s\"", "\"2h\"", "5", "\"5\""] {
        let said = refusal(&format!("[storage]\nbusy_deadline = {outside}\n"));
        assert!(
            said.contains("storage") && said.contains("busy_deadline"),
            "{outside} is refused naming the table and the key: {said}"
        );
    }
    let said = refusal("[storage]\nbusy_timeout = \"5s\"\n");
    assert!(
        said.contains("`busy_timeout`") && said.contains("busy_deadline"),
        "and a near miss is told what the table does serve: {said}"
    );
}
