use super::*;

#[test]
fn the_marker_probe_reports_the_files_modification_time_in_whole_seconds() {
    // The marker is read straight off the filesystem, no subprocess: a
    // freshly written file's mtime must land within seconds of now, and
    // the bound is two-sided so a future mtime cannot read as fresh.
    let path = std::env::temp_dir().join(format!("pns-marker-test-{}", std::process::id()));
    std::fs::write(&path, b"").unwrap();
    let probes = SystemProbes::new(FakeRunner::failing(), path.to_string_lossy().into_owned());
    let reading = probes.marker_mtime_secs();
    std::fs::remove_file(&path).ok();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mtime = reading.expect("a marker that exists must yield a reading");
    assert!(now.abs_diff(mtime) <= 5);
}

#[test]
fn an_absent_marker_reports_unknown_which_the_marker_rule_fails_closed_on() {
    let probes = SystemProbes::new(
        FakeRunner::failing(),
        "/nonexistent/pns-marker-test".to_string(),
    );
    assert_eq!(probes.marker_mtime_secs(), None);
}

#[test]
fn the_marker_probe_reads_the_link_itself_never_its_target() {
    // BSD stat -f %m reads the link, and the Back Tap touch lands on the
    // path itself: a dangling link still has its own mtime, so following
    // it to a missing target must not erase the reading.
    let link = std::env::temp_dir().join(format!("pns-marker-link-{}", std::process::id()));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink("pns-nonexistent-target", &link).unwrap();
    let probes = SystemProbes::new(FakeRunner::failing(), link.to_string_lossy().into_owned());
    let reading = probes.marker_mtime_secs();
    std::fs::remove_file(&link).ok();
    assert!(
        reading.is_some(),
        "the dangling link's own mtime is the reading"
    );
}
