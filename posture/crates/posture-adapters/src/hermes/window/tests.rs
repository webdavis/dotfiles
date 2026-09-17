use super::*;

#[test]
fn the_first_claim_on_a_finding_is_granted_and_a_repeat_inside_the_hour_is_not() {
    let root = crate::test_sandbox::Sandbox::new("copy-window");
    let path = root.path().join("state/critical-copy-window.json");
    assert_eq!(claim(&path, "finding-a", 1_000), Claim::Granted);
    for _ in 0..6 {
        assert_eq!(claim(&path, "finding-a", 1_500), Claim::AlreadyCopied);
    }
    // The repeats spent no budget, so nineteen more distinct findings still fit.
    for index in 0..19 {
        assert_eq!(
            claim(&path, &format!("other-{index}"), 1_500),
            Claim::Granted
        );
    }
    assert_eq!(claim(&path, "one-too-many", 1_500), Claim::CapReached);
}

#[test]
fn the_cap_is_twenty_distinct_findings_and_the_twenty_first_is_refused() {
    let root = crate::test_sandbox::Sandbox::new("copy-window");
    let path = root.path().join("state/critical-copy-window.json");
    for index in 0..DISTINCT_FINDINGS_PER_HOUR {
        assert_eq!(
            claim(&path, &format!("finding-{index}"), 1_000),
            Claim::Granted
        );
    }
    assert_eq!(claim(&path, "finding-20", 1_000), Claim::CapReached);
}

#[test]
fn a_claim_older_than_the_hour_is_forgotten_by_both_the_cap_and_the_repeat_rule() {
    let root = crate::test_sandbox::Sandbox::new("copy-window");
    let path = root.path().join("state/critical-copy-window.json");
    for index in 0..DISTINCT_FINDINGS_PER_HOUR {
        assert_eq!(
            claim(&path, &format!("finding-{index}"), 1_000),
            Claim::Granted
        );
    }
    // One second short of the hour the window is still full, and at the hour
    // itself every entry has left it.
    assert_eq!(claim(&path, "finding-20", 999 + WINDOW), Claim::CapReached);
    assert_eq!(claim(&path, "finding-20", 1_000 + WINDOW), Claim::Granted);
    assert_eq!(claim(&path, "finding-0", 1_000 + WINDOW), Claim::Granted);
}

#[test]
fn a_window_file_that_cannot_be_read_grants_rather_than_withholding_the_copy() {
    let root = crate::test_sandbox::Sandbox::new("copy-window");
    let directory = root.path().join("state/window.json");
    std::fs::create_dir_all(&directory).expect("a directory where the file belongs");
    assert_eq!(claim(&directory, "finding-a", 1_000), Claim::Granted);

    let corrupt = root.path().join("state/broken.json");
    std::fs::create_dir_all(corrupt.parent().unwrap()).expect("the state directory");
    std::fs::write(&corrupt, "{ not json").expect("a corrupt window");
    assert_eq!(claim(&corrupt, "finding-a", 1_000), Claim::Granted);
}

#[test]
fn a_granted_claim_survives_the_process_that_granted_it() {
    let root = crate::test_sandbox::Sandbox::new("copy-window");
    let path = root.path().join("state/critical-copy-window.json");
    assert_eq!(claim(&path, "finding-a", 1_000), Claim::Granted);
    // Nothing of the first call is held in memory: the second reads the file
    // the first wrote, which is what makes the hour survive a launchd run.
    assert_eq!(claim(&path, "finding-a", 1_000), Claim::AlreadyCopied);
}
