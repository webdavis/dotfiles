mod allowlist_fixture;
use allowlist_fixture::Fixture;
use std::fs;
#[test]
fn native_add_publishes_the_captured_tuple_after_retained_raw_source_lines() {
    let f = Fixture::new();
    let old =
        b"# header\n{\"label\":\"my.agent\",\"old\":true}\n{\"label\":\"other\",\"raw\":NaN}\n";
    fs::write(&f.source, old).unwrap();
    let result = f.run(&["add", "my.agent", "ignored"]);
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        result.stdout,
        b"allowed: my.agent -> /owned program --arg\n"
    );
    assert!(result.stderr.is_empty());
    let expected = "# header\n{\"label\":\"other\",\"raw\":NaN}\n{\"label\":\"my.agent\",\"path\":\"~/agent.plist\",\"program\":\"/owned program --arg\",\"sha256\":\"b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9\"}\n";
    assert_eq!(fs::read(&f.source).unwrap(), expected.as_bytes());
    assert_eq!(fs::read(&f.deployed).unwrap(), expected.as_bytes());
    assert_eq!(f.calls(), "query\nsource-path\napply\nmanifest\n");
}
#[test]
fn native_deny_filters_source_and_list_reads_the_deployed_raw_bytes() {
    let f = Fixture::new();
    fs::write(
        &f.source,
        b"# header\n{\"label\":\"my.agent\"}\n{\"label\":\"other\"}\n",
    )
    .unwrap();
    let result = f.run(&["deny", "my.agent"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"denied: my.agent\n");
    assert!(result.stderr.is_empty());
    assert_eq!(
        fs::read(&f.source).unwrap(),
        b"# header\n{\"label\":\"other\"}\n"
    );
    assert_eq!(f.calls(), "source-path\napply\nmanifest\n");
    fs::write(&f.deployed, b"\0# comment\n  # kept\n{bad\n\xff\n").unwrap();
    let result = f.run(&["list"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"  # kept\n{bad\n\xff\n");
    assert!(result.stderr.is_empty());
    assert_eq!(f.calls(), "source-path\napply\nmanifest\n");
}
#[test]
fn native_apply_failure_restores_source_and_reports_partial_deployment_without_manifest() {
    let f = Fixture::new();
    fs::write(f.root.join("apply-fails"), b"").unwrap();
    let result = f.run(&["add", "my.agent"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Deployment may be partial"));
    assert_eq!(fs::read(&f.source).unwrap(), b"# old source\n");
    assert_ne!(fs::read(&f.deployed).unwrap(), b"old deployed\n");
    assert_eq!(f.calls(), "query\nsource-path\napply\n");
}
#[test]
fn native_manifest_failure_keeps_both_new_copies_and_reports_stale() {
    let f = Fixture::new();
    fs::write(f.root.join("manifest-fails"), b"").unwrap();
    let result = f.run(&["add", "my.agent"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("manifest is now STALE"));
    assert_ne!(fs::read(&f.source).unwrap(), b"# old source\n");
    assert_eq!(fs::read(&f.source).unwrap(), fs::read(&f.deployed).unwrap());
}
#[test]
fn native_deny_literal_miss_skips_a_corrupt_source_and_all_publication() {
    let f = Fixture::new();
    fs::write(&f.source, b"{bad\n").unwrap();
    let result = f.run(&["deny", "my.agent"]);
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"not present: my.agent\n");
    assert!(result.stderr.is_empty());
    assert_eq!(fs::read(&f.source).unwrap(), b"{bad\n");
    assert_eq!(f.calls(), "source-path\n");
}
#[test]
fn native_lock_setup_failure_precedes_validation_and_runs_no_commands() {
    let mut f = Fixture::new();
    f.deployed = f.source.join("impossible-parent/list");
    let result = f.run(&["add", "com.apple.invalid"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .starts_with("failed to set up the allowlist write lock")
    );
    assert_eq!(f.calls(), "");
}
