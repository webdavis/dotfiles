use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{io, path::Path};
fn root() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "posture-poll-state-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn owner_mode(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}
fn modes(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}
fn put(path: &Path, bytes: &str, mode: u32) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}
mod baseline;
mod markers;
mod publication;

mod encoding;

/// The port and the inherent methods answer alike, which is the only thing the
/// bridge above can get wrong.
mod port {
    use super::*;

    #[test]
    fn the_port_remembers_and_reads_back_what_the_inherent_form_would() {
        let files = PollStateFiles::new(root().join("baseline.json"));
        let members = vec!["one".to_string(), "two".to_string()];
        posture_application::PollMarkers::remember(&files, PollGap::Readings, &members)
            .expect("the marker is written");
        assert_eq!(
            posture_application::PollMarkers::covered(&files, PollGap::Readings),
            members
        );
    }

    #[test]
    fn the_two_gaps_keep_separate_markers() {
        // One shared file would make a persistence gap silence a readings gap.
        let files = PollStateFiles::new(root().join("baseline.json"));
        posture_application::PollMarkers::remember(
            &files,
            PollGap::Readings,
            &["readings".to_string()],
        )
        .expect("the readings marker");
        assert!(posture_application::PollMarkers::covered(&files, PollGap::Persistence).is_empty());
    }

    #[test]
    fn clearing_a_marker_that_was_never_written_is_not_a_failure() {
        // The first tick of a healthy machine clears a marker it never wrote,
        // and a failure there would turn "nothing is wrong" into an error.
        let files = PollStateFiles::new(root().join("baseline.json"));
        assert!(posture_application::PollMarkers::clear(&files, PollGap::Readings).is_ok());
    }

    #[test]
    fn a_cleared_marker_covers_nothing() {
        let files = PollStateFiles::new(root().join("baseline.json"));
        posture_application::PollMarkers::remember(&files, PollGap::Readings, &["x".to_string()])
            .expect("the marker");
        posture_application::PollMarkers::clear(&files, PollGap::Readings).expect("the clear");
        assert!(posture_application::PollMarkers::covered(&files, PollGap::Readings).is_empty());
    }
}
