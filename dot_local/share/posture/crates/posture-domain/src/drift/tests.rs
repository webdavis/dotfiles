use super::*;

fn file(mode: u32, uid: u32, gid: u32) -> LiveEntry {
    LiveEntry::File(LiveAttributes { mode, uid, gid })
}

fn directory(mode: u32) -> LiveEntry {
    LiveEntry::Directory(LiveAttributes {
        mode,
        uid: 0,
        gid: 0,
    })
}

#[test]
fn test_a_file_matching_on_content_mode_owner_and_group_has_not_drifted() {
    assert_eq!(
        file_drift(file(0o644, 0, 0), ContentComparison::Equal),
        Drift::Ok
    );
}

#[test]
fn test_nothing_at_the_path_reads_as_absent() {
    assert_eq!(
        file_drift(LiveEntry::Absent, ContentComparison::Unreadable),
        Drift::Absent
    );
}

#[test]
fn test_a_symlink_standing_where_the_config_belongs_reads_as_irregular() {
    assert_eq!(
        file_drift(LiveEntry::Irregular, ContentComparison::Equal),
        Drift::Irregular
    );
}

#[test]
fn test_a_directory_standing_where_the_config_belongs_reads_as_irregular() {
    assert_eq!(
        file_drift(directory(0o755), ContentComparison::Unreadable),
        Drift::Irregular
    );
}

#[test]
fn test_differing_bytes_read_as_content_drift() {
    assert_eq!(
        file_drift(file(0o644, 0, 0), ContentComparison::Different),
        Drift::Content
    );
}

#[test]
fn test_correct_bytes_under_a_world_writable_mode_read_as_drift_not_as_ok() {
    assert_eq!(
        file_drift(file(0o666, 0, 0), ContentComparison::Equal),
        Drift::Mode
    );
}

#[test]
fn test_correct_bytes_owned_by_a_non_root_user_read_as_drift() {
    assert_eq!(
        file_drift(file(0o644, 501, 0), ContentComparison::Equal),
        Drift::Owner
    );
}

#[test]
fn test_correct_bytes_owned_by_a_non_wheel_group_read_as_drift() {
    assert_eq!(
        file_drift(file(0o644, 0, 20), ContentComparison::Equal),
        Drift::Group
    );
}

#[test]
fn test_a_state_that_could_not_be_read_reads_as_unreadable_never_as_ok() {
    assert_eq!(
        file_drift(LiveEntry::Unreadable, ContentComparison::Equal),
        Drift::Unreadable
    );
    assert_eq!(
        file_drift(file(0o644, 0, 0), ContentComparison::Unreadable),
        Drift::Unreadable
    );
    assert_eq!(
        file_drift(LiveEntry::Unreadable, ContentComparison::Different),
        Drift::Unreadable
    );
}

#[test]
fn test_content_drift_is_reported_ahead_of_an_attribute_that_also_drifted() {
    assert_eq!(
        file_drift(file(0o666, 501, 20), ContentComparison::Different),
        Drift::Content
    );
}

#[test]
fn test_a_directory_at_0755_root_wheel_has_not_drifted() {
    assert_eq!(directory_drift(directory(0o755)), Drift::Ok);
}

#[test]
fn test_a_group_writable_directory_reads_as_drift() {
    assert_eq!(directory_drift(directory(0o775)), Drift::Mode);
}

#[test]
fn test_a_missing_directory_reads_as_absent() {
    assert_eq!(directory_drift(LiveEntry::Absent), Drift::Absent);
}

#[test]
fn test_a_file_standing_where_the_packs_directory_belongs_reads_as_irregular() {
    assert_eq!(directory_drift(file(0o644, 0, 0)), Drift::Irregular);
}

#[test]
fn test_nothing_drifted_means_no_restart() {
    assert!(!restart_required(&[Drift::Ok, Drift::Ok, Drift::Ok]));
}

#[test]
fn test_any_single_drifted_path_warrants_a_restart() {
    assert!(restart_required(&[Drift::Ok, Drift::Mode, Drift::Ok]));
}

#[test]
fn test_a_drifted_path_in_the_last_position_still_warrants_a_restart() {
    assert!(restart_required(&[Drift::Ok, Drift::Ok, Drift::Content]));
}

#[test]
fn test_an_empty_verdict_list_means_no_restart() {
    assert!(!restart_required(&[]));
}
