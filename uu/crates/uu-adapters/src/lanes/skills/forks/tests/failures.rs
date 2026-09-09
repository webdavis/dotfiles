use super::*;
fn failure(kind: &'static str, state: &str) -> (LaneReport, Git) {
    let (root, lock) = setup();
    let git = Git::new(kind);
    let mut report = LaneReport::new("skills");
    SkillsForkWatch::read(&lock).report(&root, &git, &mut report);
    assert_eq!(report.failures(), 0);
    assert!(
        report.lines.iter().any(|line| line.contains(state)),
        "{:?}",
        report.lines
    );
    (report, git)
}
#[test]
fn an_unreachable_upstream_is_named_and_compared_never() {
    let (report, git) = failure("unreachable", "fork-upstream-unreachable");
    assert_eq!(git.calls.borrow().len(), 1);
    assert!(
        report.lines[0].contains("fixture remote rejected")
            && report.lines[0].contains("not compared")
    );
}
#[test]
fn a_recorded_path_the_upstream_no_longer_has_is_its_own_state() {
    let (report, git) = failure("path", "fork-path-missing");
    assert_eq!(git.calls.borrow().len(), 3);
    assert!(
        report.lines[0].contains("skillPath")
            && report.lines[0].contains("leave lastComparedTreeHash")
    );
}
#[test]
fn a_headless_clone_is_distinct_from_a_missing_skill_path() {
    let (report, git) = failure("headless", "fork-upstream-headless");
    assert_eq!(git.calls.borrow().len(), 2);
    assert!(report.lines[0].contains("default branch"));
}
#[test]
fn a_clone_deadline_is_reported_without_failing_the_lane() {
    failure("timeout", "fork-clone-timeout");
}
#[test]
fn a_clone_workspace_that_cannot_be_created_is_named_without_fetching() {
    let (root, lock) = setup();
    let temp = root.join("not-a-directory");
    std::fs::write(&temp, "preserved").unwrap();
    let git = Git::new("");
    let mut report = LaneReport::new("skills");
    SkillsForkWatch::read(&lock).report(&temp, &git, &mut report);
    assert!(report.lines[0].contains("fork-clone-unstageable"));
    assert!(git.calls.borrow().is_empty());
    assert_eq!(std::fs::read_to_string(temp).unwrap(), "preserved");
}
#[test]
fn absent_broken_and_tableless_fork_locks_name_the_distinct_remedies() {
    let root = directory();
    for (file, bytes, state) in [
        ("missing", None, "fork-lock-missing"),
        ("broken", Some("{}{}"), "fork-lock-broken"),
        ("absent", Some("{}"), "fork-table-absent"),
        ("scalar", Some("{\"forks\":false}"), "fork-lock-broken"),
    ] {
        let path = root.join(file);
        if let Some(bytes) = bytes {
            std::fs::write(&path, bytes).unwrap();
        }
        let mut report = LaneReport::new("skills");
        let git = Git::new("");
        SkillsForkWatch::read(&path).report(&root, &git, &mut report);
        assert!(
            report
                .lines
                .iter()
                .any(|s| s.contains(state) && s.contains(path.to_str().unwrap())),
            "{file}: {:?}",
            report.lines
        );
        assert_eq!(report.failures(), 0);
        assert!(git.calls.borrow().is_empty());
    }
}
#[test]
fn malformed_fork_fields_are_advisory_and_do_not_skip_later_entries() {
    let (root, lock) = setup();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
    value["forks"]["broken"] =
        serde_json::json!({"sourceUrl":42,"skillPath":"x\u{0}","lastComparedTreeHash":""});
    write_roster(&lock, &value);
    let git = Git::new("");
    let mut report = LaneReport::new("skills");
    SkillsForkWatch::read(&lock).report(&root, &git, &mut report);
    for expected in [
        "fork-lock-broken",
        "sourceUrl",
        "skillPath",
        "lastComparedTreeHash",
    ] {
        assert!(report.lines[0].contains(expected));
    }
    assert_eq!(git.calls.borrow().len(), 3);
    assert_eq!(report.failures(), 0);
}
#[test]
fn an_incomplete_fork_walk_names_the_unchecked_remainder() {
    let mut report = LaneReport::new("skills");
    finish_walk(1, 3, &mut report);
    assert!(report.lines[0].contains("fork-walk-incomplete") && report.lines[0].contains("1 of 3"));
    assert_eq!(report.failures(), 0);
}
