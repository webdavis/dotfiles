use super::*;
use std::os::unix::fs::PermissionsExt;
#[test]
fn an_invalid_uu_snapshot_fails_without_importing_legacy_or_reseeding() {
    let f = Fixture::new("invalid-uu");
    f.set(f.snapshot(), "malformed");
    f.set(f.legacy(), "alpha\t1\n");
    assert_eq!(f.run(false).failures(), 1);
    assert_eq!(f.current(), "malformed");
    assert_eq!(fs::read_to_string(f.legacy()).unwrap(), "alpha\t1\n");
}
#[test]
fn a_snapshot_publication_failure_is_not_reported_as_a_completed_comparison() {
    let f = Fixture::new("publish-fail");
    f.set(f.snapshot(), "alpha\t1\n");
    // A read-only parent keeps the existing baseline readable but refuses its sibling temp file.
    let parent = f.snapshot().parent().unwrap().to_path_buf();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let report = f.run(false);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(report.failures(), 1, "{report:?}");
    assert_eq!(f.current(), "alpha\t1\n");
}
#[test]
fn a_non_regular_inventory_is_refused_without_touching_history() {
    let mut f = Fixture::new("nonregular");
    f.set(f.snapshot(), "alpha\t1\n");
    f.lane.inventory = f.root.to_str().unwrap().into();
    let report = f.run(false);
    assert_eq!(report.failures(), 1, "{report:?}");
    assert!(report.lines.join("\n").contains("regular file"));
    assert_eq!(f.current(), "alpha\t1\n");
}
#[test]
fn an_imported_snapshot_preserves_escaped_fields_and_its_exact_bytes() {
    let f = Fixture::new("escaped");
    let old = "alpha\\tname\t1\\n2\\r3\\\\4\n";
    f.set(f.legacy(), old);
    fs::write(
        &f.lane.inventory,
        serde_json::json!({"plugins":{"alpha\tname":[{"scope":"user","version":"1\n2\r3\\4"}]}})
            .to_string(),
    )
    .unwrap();
    completed(&f.run(true));
    assert_eq!(f.current(), old);
    let report = f.run(false);
    completed(&report);
    assert!(
        report
            .lines
            .join("\n")
            .contains("0 of 1 tracked entries changed"),
        "{report:?}"
    );
    assert_eq!(f.current(), old);
    assert_eq!(fs::read_to_string(f.legacy()).unwrap(), old);
}
#[test]
fn a_new_plugin_snapshot_and_its_directory_are_owner_only() {
    let f = Fixture::new("permissions");
    completed(&f.run(true));
    assert_eq!(
        fs::metadata(f.snapshot()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(f.snapshot().parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}
