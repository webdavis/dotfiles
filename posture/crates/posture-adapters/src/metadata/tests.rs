use super::*;
#[test]
fn missing_metadata_remains_not_applicable() {
    assert_eq!(
        metadata(Path::new("/fixture/missing-enrichment-metadata")),
        None
    );
}

#[test]
fn symbolic_modes_keep_file_kind_special_bits_and_their_execute_partner() {
    assert_eq!(&mode_text(libc::S_IFREG as u32 | 0o4755), b"-rwsr-xr-x");
    assert_eq!(&mode_text(libc::S_IFREG as u32 | 0o4644), b"-rwSr--r--");
    assert_eq!(&mode_text(libc::S_IFDIR as u32 | 0o2770), b"drwxrws---");
    assert_eq!(&mode_text(libc::S_IFDIR as u32 | 0o2660), b"drw-rwS---");
    assert_eq!(&mode_text(libc::S_IFDIR as u32 | 0o1777), b"drwxrwxrwt");
    assert_eq!(&mode_text(libc::S_IFDIR as u32 | 0o1666), b"drw-rw-rwT");
    assert_eq!(&mode_text(libc::S_IFLNK as u32 | 0o755), b"lrwxr-xr-x");
}

#[test]
fn the_named_root_owner_is_not_replaced_by_a_numeric_identifier() {
    assert_eq!(owner_name(0), b"root");
}

#[test]
fn metadata_time_keeps_the_legacy_format() {
    // All tm members are integers or nullable pointers; mktime fills the derived fields.
    let mut calendar: libc::tm = unsafe { std::mem::zeroed() };
    calendar.tm_year = 126;
    calendar.tm_mon = 8;
    calendar.tm_mday = 7;
    calendar.tm_isdst = -1;
    // A local midnight keeps the fixture valid under the process's supplied timezone.
    let seconds = unsafe { libc::mktime(&mut calendar) };
    assert_eq!(
        modified_time(seconds),
        Some(b"2026-09-07T00:00:00Z".to_vec())
    );
}

#[test]
fn metadata_describes_a_live_symlink_but_not_a_broken_one() {
    let directory =
        std::env::temp_dir().join(format!("posture-metadata-link-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("private fixture directory");
    let target = directory.join("target");
    std::fs::write(&target, b"inert fixture").expect("fixture contents");
    let live = directory.join("live");
    std::os::unix::fs::symlink(&target, &live).expect("owned live symlink");
    let fact = metadata(&live).expect("live link has context");
    assert!(fact.windows(8).any(|part| part == b", mode l"));
    let broken = directory.join("broken");
    std::os::unix::fs::symlink(directory.join("absent"), &broken).expect("owned broken symlink");
    assert_eq!(metadata(&broken), None);
}
