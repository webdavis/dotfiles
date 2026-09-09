use super::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

mod fixture;
use fixture::Fixture;

#[test]
fn a_prepared_tree_keeps_the_named_bytes_in_one_private_owned_copy() {
    let fixture = Fixture::new();
    let staged = fixture.prepare().unwrap();
    let source = staged.source(ConvergeFile::Configuration);
    let root = source.parent().unwrap().to_path_buf();
    assert_ne!(root, fixture.desired);
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o7777,
        0o700
    );
    fs::write(fixture.desired.join("osquery.conf"), b"substituted").unwrap();
    for (file, bytes) in fixture::FILES {
        let file = ConvergeFile::from_relative_path(file).unwrap();
        assert_eq!(fs::read(staged.source(file)).unwrap(), bytes);
    }
    drop(staged);
    assert!(!root.exists(), "the prepared copy outlived its owner");
    assert_eq!(fs::read(fixture.neighbor()).unwrap(), b"unrelated");
}

#[test]
fn a_relative_staging_directory_is_refused_without_resolving_it() {
    let fixture = Fixture::new();
    let path = PathBuf::from("relative/desired");
    let result = DesiredStaging::new(path.clone(), fixture.scratch.clone()).prepare();
    assert_eq!(
        result.unwrap_err(),
        [StagingRefusal::RelativeDirectory(path)]
    );
    fixture.assert_scratch_untouched();
}

#[test]
fn a_missing_staging_directory_is_a_refusal() {
    let fixture = Fixture::new();
    let path = fixture.root.join("missing");
    let result = DesiredStaging::new(path.clone(), fixture.scratch.clone()).prepare();
    assert_eq!(
        result.unwrap_err(),
        [StagingRefusal::MissingDirectory(path)]
    );
    fixture.assert_scratch_untouched();
}

#[test]
fn a_staging_directory_reached_through_any_symlink_is_refused() {
    let fixture = Fixture::new();
    for (name, target, tail) in [
        ("leaf-link", &fixture.desired, ""),
        ("parent-link", &fixture.root, "desired"),
    ] {
        let link = fixture.root.join(name);
        symlink(target, &link).unwrap();
        let result = DesiredStaging::new(link.join(tail), fixture.scratch.clone()).prepare();
        assert_eq!(
            result.unwrap_err(),
            [StagingRefusal::SymlinkComponent(link)]
        );
    }
    fixture.assert_scratch_untouched();
}

#[test]
fn unlisted_files_are_refused_by_exact_relative_name() {
    let fixture = Fixture::new();
    let paths = ["packs/*.conf", "intrusion-detection.conf"];
    for path in paths {
        fs::write(fixture.desired.join(path), b"planted").unwrap();
    }
    let refused = fixture.prepare().unwrap_err();
    assert_eq!(refused.len(), 2);
    for path in paths {
        assert!(refused.contains(&StagingRefusal::UnlistedEntry(fixture.desired.join(path))));
    }
    fixture.assert_scratch_untouched();
}

#[test]
fn symlinks_anywhere_in_the_desired_tree_are_refused() {
    for name in [
        "osquery.conf",
        "packs/unlisted-link",
        "packs/linked-directory",
    ] {
        let fixture = Fixture::new();
        let path = fixture.desired.join(name);
        if path.exists() {
            fs::rename(&path, fixture.root.join("saved")).unwrap();
        }
        symlink(&fixture.scratch, &path).unwrap();
        assert!(
            fixture
                .prepare()
                .unwrap_err()
                .contains(&StagingRefusal::SymlinkEntry(path))
        );
        fixture.assert_scratch_untouched();
    }
}

#[test]
fn every_missing_desired_file_is_reported_before_a_snapshot_is_returned() {
    let fixture = Fixture::new();
    let paths = ["osquery.flags", "packs/intrusion-detection.conf"];
    for (index, path) in paths.into_iter().enumerate() {
        fs::rename(
            fixture.desired.join(path),
            fixture.root.join(format!("saved-{index}")),
        )
        .unwrap();
    }
    let refused = fixture.prepare().unwrap_err();
    assert_eq!(refused.len(), 2);
    for path in paths {
        assert!(refused.contains(&StagingRefusal::MissingFile(fixture.desired.join(path))));
    }
    fixture.assert_scratch_untouched();
}

#[test]
fn an_incomplete_listing_cannot_return_a_partial_snapshot() {
    let fixture = Fixture::new();
    let hidden = fixture.desired.join("packs/unreadable");
    fs::create_dir(&hidden).unwrap();
    fs::write(hidden.join("planted"), b"not listed").unwrap();
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o0)).unwrap();
    assert!(
        fs::read_dir(&hidden).is_err(),
        "the fixture must actually refuse enumeration"
    );
    let result = fixture.prepare();
    fs::set_permissions(&hidden, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        result.unwrap_err(),
        [StagingRefusal::IncompleteListing(hidden)]
    );
    fixture.assert_scratch_untouched();
}

#[test]
fn a_failed_copy_removes_only_its_owned_partial_stage() {
    let fixture = Fixture::new();
    let path = fixture.desired.join("osquery.flags");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    assert!(
        fs::read(&path).is_err(),
        "the fixture must actually refuse reading"
    );
    let result = fixture.prepare();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(result.unwrap_err(), [StagingRefusal::CopyFailed(path)]);
    fixture.assert_scratch_untouched();
}
