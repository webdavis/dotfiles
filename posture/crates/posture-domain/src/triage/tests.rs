use super::*;

const HEADER: &str = "9000\t1970-01-01T02:30:00Z\n";

#[test]
fn added_and_removed_versions_keep_their_empty_columns() {
    for (row, transition) in [
        ("tool\tadded\t\t2", "none -> 2"),
        ("tool\tremoved\t1\t", "1 -> none"),
        ("tool\tchanged\t1\t2", "1 -> 2"),
    ] {
        let line = upgrade_correlation(&format!("{HEADER}{row}"), "tool", Some(10_000)).unwrap();
        assert_eq!(
            line,
            format!(
                "recorded upgrade: tool {transition} at 1970-01-01T02:30:00Z (the name matches this file, which is not proof)"
            )
        );
    }
}

#[test]
fn future_and_old_records_are_not_explanations_but_the_boundary_is_included() {
    let snapshot = format!("{HEADER}tool\tchanged\t1\t2\n");
    for now in [8999, 9000 + 259_201] {
        assert_eq!(
            upgrade_correlation(&snapshot, "tool", Some(now)).unwrap(),
            "no recorded upgrade in the last 3 days; the newest record is from 1970-01-01T02:30:00Z"
        );
    }
    for now in [9000, 9000 + 259_200] {
        assert!(
            upgrade_correlation(&snapshot, "tool", Some(now))
                .unwrap()
                .starts_with("recorded upgrade:")
        );
    }
    assert_eq!(
        upgrade_correlation(&snapshot, "tool", None),
        Err(UpgradeRecordRefusal::ClockUnavailable)
    );
}

#[test]
fn a_bad_row_refuses_the_whole_record_even_after_a_name_match() {
    for bad in ["tool\tunknown\t1\t2", "tool\tadded", "tool\tchanged\t1"] {
        assert_eq!(
            upgrade_correlation(
                &format!("{HEADER}tool\tadded\t\t1\n{bad}"),
                "tool",
                Some(9000)
            ),
            Err(UpgradeRecordRefusal::Row)
        );
    }
    let rows = format!("{HEADER}{}", "tool\tadded\t\t1\n".repeat(499));
    assert!(upgrade_correlation(&rows, "tool", Some(9000)).is_ok());
    assert_eq!(
        upgrade_correlation(&(rows + "extra\tadded\t\t1"), "tool", Some(9000)),
        Err(UpgradeRecordRefusal::TooManyRows)
    );
}

#[test]
fn unmatched_records_name_at_most_five_packages_and_do_not_claim_completion() {
    assert_eq!(
        upgrade_correlation(HEADER, "tool", Some(9000)).unwrap(),
        "no recorded upgrade names this file; the run at 1970-01-01T02:30:00Z recorded no package change"
    );
    let snapshot = format!(
        "{HEADER}{}",
        (1..=7)
            .map(|n| format!("pkg{n}\tadded\t\t1\n"))
            .collect::<String>()
    );
    assert_eq!(
        upgrade_correlation(&snapshot, "tool", Some(9000)).unwrap(),
        "no recorded upgrade names this file; the run at 1970-01-01T02:30:00Z changed: pkg1, pkg2, pkg3, pkg4, pkg5, and 2 more"
    );
}

#[test]
fn malformed_headers_are_refused() {
    for header in [
        "",
        "9000",
        "-1\t1970-01-01T02:30:00Z",
        "123456789012\t1970-01-01T02:30:00Z",
        "9000\t1970-01-01T02:30:00Z\r",
        "9000\t1970-01-01 02:30:00Z",
    ] {
        assert_eq!(
            upgrade_correlation(header, "tool", Some(9000)),
            Err(UpgradeRecordRefusal::Header)
        );
    }
}

#[test]
fn recorded_hash_requires_a_full_digest_and_exact_path_but_keeps_spaces() {
    let path = "/a path/tool";
    let text = format!("bad 0600 1 {path}\n{}\t 0600  1\t{path}", "A".repeat(64));
    assert_eq!(recorded_hash(&text, path), Some("aaaaaaaaaaaa".into()));
    assert_eq!(recorded_hash(&text, "/a path/other"), None);
    assert_eq!(
        recorded_hash(&format!("{} 0600 1 {path}", "a".repeat(63)), path),
        None
    );
}
