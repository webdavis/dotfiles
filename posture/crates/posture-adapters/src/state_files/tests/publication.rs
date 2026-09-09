use super::*;
fn captured(name: &str) -> serde_json::Value {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("publication.json")).unwrap();
    rows.into_iter().find(|x| x["name"] == name).unwrap()
}
#[test]
fn baseline_publication_replaces_sibling_temporary_content_and_finishes_owner_only() {
    for name in ["normal", "existing-temporary"] {
        let c = captured(name);
        let store = PollStateFiles::new(root().join("state"));
        put(&store.baseline, "old\n", 0o600);
        if name == "existing-temporary" {
            put(&store.sibling(".tmp"), "stale", 0o644);
        }
        store
            .write_with(c["baseline_json"].as_str().unwrap(), owner_mode)
            .unwrap();
        assert_eq!(
            fs::read_to_string(&store.baseline).unwrap(),
            c["state"].as_str().unwrap()
        );
        assert_eq!(modes(&store.baseline), 0o600);
        assert!(!store.sibling(".tmp").exists());
    }
}
#[test]
fn publication_failures_preserve_actual_pre_and_post_rename_file_outcomes() {
    let dir = root();
    let store = PollStateFiles::new(dir.join("state"));
    put(&store.baseline, "old\n", 0o600);
    let tmp = store.sibling(".tmp");
    fs::create_dir(&tmp).unwrap();
    assert!(store.write_with("new", owner_mode).is_err());
    assert_eq!(fs::read_to_string(&store.baseline).unwrap(), "old\n");
    assert!(tmp.is_dir());
    let dir = root();
    let store = PollStateFiles::new(dir.join("state"));
    put(&store.baseline, "old\n", 0o600);
    put(&store.sibling(".tmp"), "stale", 0o644);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    assert!(store.write_with("new", owner_mode).is_err());
    assert_eq!(fs::read_to_string(&store.baseline).unwrap(), "old\n");
    assert_eq!(fs::read_to_string(store.sibling(".tmp")).unwrap(), "new\n");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    let temporary = store.sibling(".tmp");
    drop(store);
    assert!(!temporary.exists());
    let c = captured("chmod-refused");
    let store = PollStateFiles::new(root().join("state"));
    put(&store.baseline, "old\n", 0o600);
    put(&store.sibling(".tmp"), "stale", 0o644);
    let result = store.write_with(c["baseline_json"].as_str().unwrap(), |path| {
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            c["state"].as_str().unwrap()
        );
        Err(io::ErrorKind::PermissionDenied.into())
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(&store.baseline).unwrap(),
        c["state"].as_str().unwrap()
    );
    assert_eq!(modes(&store.baseline), 0o644);
    assert!(!store.sibling(".tmp").exists());
}
