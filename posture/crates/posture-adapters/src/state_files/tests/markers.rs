use super::*;
fn cases() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("markers.json")).unwrap()
}
#[test]
fn marker_coverage_keeps_literal_spaces_and_does_not_cover_newline_or_tab_members() {
    for case in cases() {
        let store = PollStateFiles::new(root().join("state"));
        let marker = store.marker(PollGap::Readings);
        if let Some(value) = case["input"].as_str() {
            put(&marker, value, 0o644);
        }
        let covered = store.covered(PollGap::Readings);
        let members: Vec<_> = case["members"].as_str().unwrap().split(' ').collect();
        let new = members.iter().any(|x| !covered.iter().any(|c| c == x));
        assert_eq!(
            new,
            !case["calls"].as_array().unwrap().is_empty(),
            "{}",
            case["name"]
        );
    }
}
#[test]
fn markers_refresh_members_clear_recovery_and_report_unwritable_paths_separately() {
    let store = PollStateFiles::new(root().join("state"));
    let gap = store.marker(PollGap::Readings);
    let persist = store.marker(PollGap::Persistence);
    put(&gap, "a b\n", 0o644);
    store.remember(PollGap::Readings, &["b".into()]).unwrap();
    assert_eq!(fs::read_to_string(&gap).unwrap(), "b\n");
    assert_eq!(modes(&gap), 0o644);
    store
        .remember(PollGap::Persistence, &["baseline_persist".into()])
        .unwrap();
    assert_eq!(fs::read_to_string(&persist).unwrap(), "baseline_persist\n");
    store.clear(PollGap::Readings).unwrap();
    assert!(!gap.exists());
    assert!(persist.exists());
    store.clear(PollGap::Readings).unwrap();
    fs::create_dir(&gap).unwrap();
    assert!(store.remember(PollGap::Readings, &["a".into()]).is_err());
    assert!(store.clear(PollGap::Readings).is_err());
    assert!(gap.is_dir());
}
