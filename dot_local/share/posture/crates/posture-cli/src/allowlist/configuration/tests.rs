use super::*;
use std::collections::BTreeMap;
#[test]
fn explicit_configuration_keeps_unsplit_paths_and_nonempty_manifest_override() {
    let values = BTreeMap::from([
        ("HOME", "/private home"),
        ("PATH", "/absent"),
        ("OSQUERY_LAUNCHD_ALLOWLIST", "/deployed list"),
        ("OSQUERYI", "/query tool"),
        ("CHEZMOI", "/source tool"),
        ("OSQUERY_PIPELINE_MANIFEST_RUNNER", "/manifest runner"),
    ]);
    let c = Configuration::read(|key| values.get(key).map(OsString::from));
    assert_eq!(c.home, PathBuf::from("/private home"));
    assert_eq!(c.deployed, PathBuf::from("/deployed list"));
    assert_eq!(c.osqueryi, PathBuf::from("/query tool"));
    assert_eq!(c.chezmoi, PathBuf::from("/source tool"));
    assert_eq!(c.manifest, Some("/manifest runner".into()));
}
#[test]
fn empty_overrides_use_home_and_missing_executable_fallbacks() {
    let c = Configuration::read(|key| {
        Some(OsString::from(match key {
            "HOME" => "/private home",
            "PATH" => "/absent",
            _ => "",
        }))
    });
    assert_eq!(
        c.deployed,
        PathBuf::from("/private home/.config/osquery/page-launchd-allowlist.txt")
    );
    assert_eq!(c.osqueryi, PathBuf::from("/usr/local/bin/osqueryi"));
    assert_eq!(c.chezmoi, PathBuf::from("chezmoi"));
    assert_eq!(c.manifest, None);
    let empty = Configuration::read(|_| Some(OsString::from("")));
    assert_eq!(
        empty.deployed,
        PathBuf::from("/.config/osquery/page-launchd-allowlist.txt")
    );
}
#[test]
fn executable_discovery_skips_nonexecutable_files_and_keeps_path_order() {
    let root = std::env::temp_dir().join(format!("posture-discovery-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for (name, mode) in [("a", 0o600), ("b", 0o700), ("c", 0o700)] {
        let directory = root.join(name);
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("osqueryi");
        std::fs::write(&file, b"inert").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    let path = std::env::join_paths([root.join("a"), root.join("b"), root.join("c")]).unwrap();
    assert_eq!(executable("osqueryi", &path), Some(root.join("b/osqueryi")));
}
