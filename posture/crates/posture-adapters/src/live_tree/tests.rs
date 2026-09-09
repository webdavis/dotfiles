use super::*;
use crate::DesiredStaging;
use posture_application::{ConvergeRefusal, ConvergeStaging, prepare_converge};
use posture_domain::{Drift, LiveAttributes, file_drift};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
mod fixture;
use fixture::Fixture;

#[test]
fn live_metadata_keeps_special_mode_bits_and_numeric_ownership() {
    let fixture = Fixture::new();
    fs::write(fixture.config(), b"desired bytes\n").unwrap();
    fs::set_permissions(fixture.config(), fs::Permissions::from_mode(0o4644)).unwrap();
    let owner = fs::metadata(&fixture.root).unwrap();
    let (entry, equal) = fixture.live().file(
        ConvergeFile::Configuration,
        &fixture.desired.join("osquery.conf"),
    );
    assert_eq!(
        entry,
        LiveEntry::File(LiveAttributes {
            mode: 0o4644,
            uid: owner.uid(),
            gid: owner.gid()
        })
    );
    assert_eq!(equal, ContentComparison::Equal);
    assert_eq!(file_drift(entry, equal), Drift::Mode);
    fs::set_permissions(
        fixture.target.join("packs"),
        fs::Permissions::from_mode(0o775),
    )
    .unwrap();
    assert_eq!(
        fixture.live().directory(ConvergeDirectory::Packs),
        LiveEntry::Directory(LiveAttributes {
            mode: 0o775,
            uid: owner.uid(),
            gid: owner.gid()
        })
    );
}

#[test]
fn a_live_symlink_is_irregular_even_when_its_referent_matches() {
    let fixture = Fixture::new();
    symlink(fixture.desired.join("osquery.conf"), fixture.config()).unwrap();
    let (entry, _) = fixture.live().file(
        ConvergeFile::Configuration,
        &fixture.desired.join("osquery.conf"),
    );
    assert_eq!(entry, LiveEntry::Irregular);
    symlink(
        fixture.root.join("missing"),
        fixture.target.join("osquery.flags"),
    )
    .unwrap();
    assert_eq!(
        fixture
            .live()
            .file(ConvergeFile::Flags, &fixture.desired.join("osquery.flags"))
            .0,
        LiveEntry::Irregular
    );
}

#[test]
fn missing_live_paths_are_absent_and_file_directories_are_irregular() {
    let fixture = Fixture::new();
    let mut missing = InstalledTree::new(fixture.root.join("missing"));
    assert_eq!(
        missing.directory(ConvergeDirectory::Target),
        LiveEntry::Absent
    );
    assert_eq!(
        missing
            .file(
                ConvergeFile::Configuration,
                &fixture.desired.join("osquery.conf")
            )
            .0,
        LiveEntry::Absent
    );
    fs::create_dir(fixture.config()).unwrap();
    let (entry, equal) = fixture.live().file(
        ConvergeFile::Configuration,
        &fixture.desired.join("osquery.conf"),
    );
    assert_eq!(file_drift(entry, equal), Drift::Irregular);
}

#[test]
fn an_unreadable_live_file_is_never_an_equal_comparison() {
    let fixture = Fixture::new();
    fs::write(fixture.config(), b"desired bytes\n").unwrap();
    fs::set_permissions(fixture.config(), fs::Permissions::from_mode(0o0)).unwrap();
    let (entry, equal) = fixture.live().file(
        ConvergeFile::Configuration,
        &fixture.desired.join("osquery.conf"),
    );
    assert_eq!(equal, ContentComparison::Unreadable);
    assert_eq!(file_drift(entry, equal), Drift::Unreadable);
}

#[test]
fn comparisons_keep_using_the_private_copy_after_deployed_bytes_change() {
    let fixture = Fixture::new();
    fs::write(fixture.config(), b"desired bytes\n").unwrap();
    let staged = fixture.staging().prepare().unwrap();
    fs::write(
        fixture.desired.join("osquery.conf"),
        b"later deployed change",
    )
    .unwrap();
    let (_, equal) = fixture.live().file(
        ConvergeFile::Configuration,
        &staged.source(ConvergeFile::Configuration),
    );
    assert_eq!(equal, ContentComparison::Equal);
    assert_eq!(
        fs::read(staged.source(ConvergeFile::Configuration)).unwrap(),
        b"desired bytes\n"
    );
}

#[test]
fn different_live_bytes_are_reported_without_rewriting_either_tree() {
    let fixture = Fixture::new();
    fs::write(fixture.config(), b"changed live bytes").unwrap();
    let (entry, equal) = fixture.live().file(
        ConvergeFile::Configuration,
        &fixture.desired.join("osquery.conf"),
    );
    assert_eq!(equal, ContentComparison::Different);
    assert_eq!(file_drift(entry, equal), Drift::Content);
    assert_eq!(fs::read(fixture.config()).unwrap(), b"changed live bytes");
    assert_eq!(
        fs::read(fixture.desired.join("osquery.conf")).unwrap(),
        b"desired bytes\n"
    );
}

#[test]
fn an_irregular_packs_directory_refuses_the_real_plan_without_target_changes() {
    let fixture = Fixture::new();
    let target = fixture.root.join("irregular-target");
    let neighbor = fixture.root.join("neighbor");
    fs::create_dir(&target).unwrap();
    fs::create_dir(&neighbor).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&neighbor, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&neighbor, target.join("packs")).unwrap();
    let outcome = prepare_converge(&fixture.staging(), &mut InstalledTree::new(target.clone()));
    assert_eq!(
        outcome.unwrap_err(),
        ConvergeRefusal::IrregularDirectory(ConvergeDirectory::Packs)
    );
    assert_eq!(fs::metadata(&target).unwrap().mode() & 0o7777, 0o700);
    assert_eq!(fs::metadata(&neighbor).unwrap().mode() & 0o7777, 0o700);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    assert_eq!(fs::read_dir(&fixture.scratch).unwrap().count(), 0);
}
