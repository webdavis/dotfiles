use super::*;

fn window(sandbox: &crate::test_sandbox::Sandbox) -> std::path::PathBuf {
    sandbox.path().join("state/critical-copy-window.json")
}

/// Claim `count` distinct findings at `now`, named after their index.
fn fill(path: &Path, count: usize, now: u64) {
    for index in 0..count {
        assert_eq!(
            claim(
                path,
                &format!("key-{index}"),
                &format!("finding {index}"),
                now
            ),
            Claim::Granted,
            "finding {index} is inside the threshold"
        );
    }
}

#[test]
fn the_first_claim_on_a_finding_is_granted_and_a_repeat_inside_the_hour_is_not() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let path = window(&sandbox);
    assert_eq!(claim(&path, "key-a", "finding a", 1_000), Claim::Granted);
    for _ in 0..6 {
        assert_eq!(
            claim(&path, "key-a", "finding a", 1_500),
            Claim::AlreadyCopied
        );
    }
    // The repeats spent no budget, so the rest of the threshold still fits.
    for index in 1..STORM_THRESHOLD {
        assert_eq!(
            claim(&path, &format!("key-{index}"), "another finding", 1_500),
            Claim::Granted
        );
    }
}

#[test]
fn the_finding_that_crosses_the_threshold_carries_the_whole_hours_list() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let path = window(&sandbox);
    fill(&path, STORM_THRESHOLD, 1_000);
    let crossing = claim(&path, "key-last", "the finding that tipped it", 1_001);
    let Claim::Storm(findings) = crossing else {
        panic!("the threshold was crossed, got {crossing:?}");
    };
    assert_eq!(findings.len(), STORM_THRESHOLD + 1);
    assert_eq!(
        findings.first().map(String::as_str),
        Some("finding 0"),
        "oldest first, which is the order they happened in"
    );
    assert_eq!(
        findings.last().map(String::as_str),
        Some("the finding that tipped it")
    );
}

#[test]
fn every_distinct_finding_after_the_crossing_one_is_already_covered_by_that_message() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let path = window(&sandbox);
    fill(&path, STORM_THRESHOLD, 1_000);
    assert!(matches!(
        claim(&path, "key-crossing", "the crossing finding", 1_000),
        Claim::Storm(_)
    ));
    for index in 0..4 {
        assert_eq!(
            claim(
                &path,
                &format!("key-after-{index}"),
                "a later finding",
                1_000
            ),
            Claim::Storming,
            "the hour's one message has already been sent"
        );
    }
    // A repeat is still a repeat during a storm.
    assert_eq!(
        claim(&path, "key-crossing", "the crossing finding", 1_000),
        Claim::AlreadyCopied
    );
}

#[test]
fn a_claim_older_than_the_hour_is_forgotten_by_the_repeat_rule_below_the_threshold() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let path = window(&sandbox);
    fill(&path, STORM_THRESHOLD - 1, 1_000);
    // At the hour itself every earlier entry has left the window, so a
    // finding claimed before is granted again as if new.
    assert_eq!(
        claim(&path, "key-0", "finding 0", 1_000 + WINDOW),
        Claim::Granted
    );
}

#[test]
fn a_storm_stays_a_storm_for_the_full_hour_even_as_its_entries_age_out() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let path = window(&sandbox);
    // Five findings a second apart, then the one that crosses the threshold.
    for index in 0..STORM_THRESHOLD {
        assert_eq!(
            claim(
                &path,
                &format!("key-{index}"),
                &format!("finding {index}"),
                1_000 + index as u64
            ),
            Claim::Granted
        );
    }
    let crossing_at = 1_000 + STORM_THRESHOLD as u64;
    assert!(matches!(
        claim(&path, "key-crossing", "the crossing finding", crossing_at),
        Claim::Storm(_)
    ));
    // Every entry that built the storm ages out of the map well inside the
    // hour the storm itself is still remembered for.
    let still_within_the_storms_hour = crossing_at + WINDOW - 1;
    assert_eq!(
        claim(
            &path,
            "key-fresh",
            "a fresh distinct finding",
            still_within_the_storms_hour
        ),
        Claim::Storming,
        "a new distinct finding inside the storm's hour must not re-cross and send a second combined message"
    );
}

#[test]
fn a_window_file_that_cannot_be_read_grants_rather_than_withholding_the_copy() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let directory = sandbox.path().join("state/window.json");
    std::fs::create_dir_all(&directory).expect("a directory where the file belongs");
    assert_eq!(
        claim(&directory, "key-a", "finding a", 1_000),
        Claim::Granted
    );

    let corrupt = sandbox.path().join("state/broken.json");
    std::fs::create_dir_all(corrupt.parent().unwrap()).expect("the state directory");
    std::fs::write(&corrupt, "{ not json").expect("a corrupt window");
    assert_eq!(claim(&corrupt, "key-a", "finding a", 1_000), Claim::Granted);
}

#[test]
fn a_granted_claim_survives_the_process_that_granted_it() {
    let sandbox = crate::test_sandbox::Sandbox::new("copy-window");
    let path = window(&sandbox);
    assert_eq!(claim(&path, "key-a", "finding a", 1_000), Claim::Granted);
    // Nothing of the first call is held in memory: the second reads the file
    // the first wrote, which is what makes the hour survive a launchd run.
    assert_eq!(
        claim(&path, "key-a", "finding a", 1_000),
        Claim::AlreadyCopied
    );
}
