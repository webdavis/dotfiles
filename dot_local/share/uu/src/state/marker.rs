//! Reading and writing the last-successful-run marker.

use std::path::{Path, PathBuf};

use unattended_upgrades::record::{Marker, marker_contents, parse_marker};

use crate::system::iso;

pub fn path(home: &str) -> PathBuf {
    super::dir(home).join("last-success")
}

/// The marker, with an ABSENT path told from a BROKEN LINK. Both read
/// NotFound, and they are opposite states: nothing there is a machine that has
/// never finished a run, while a link whose target went away is bookkeeping
/// that stopped resolving and has to be said out loud.
pub fn read(path: &Path) -> Marker {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_marker(&text),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Marker::NeverRecorded
        }
        Err(_) => Marker::Unreadable,
    }
}

/// Best effort, and never silent: a job must not fail because it could not
/// write its own bookkeeping, but a failure to write would have the next entry
/// measure its gap from a run that did not happen.
pub fn write(path: &Path, epoch: i64) {
    let written = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, marker_contents(epoch, &iso(epoch))));
    if let Err(error) = written {
        eprintln!(
            "uu: could not record the successful-run timestamp at {}: {error}; the next entry \
             will report a stale or absent gap",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::scratch;

    #[test]
    fn a_path_with_no_marker_at_it_is_a_machine_that_never_recorded_a_run() {
        assert_eq!(read(&scratch("absent")), Marker::NeverRecorded);
    }

    #[test]
    fn a_dangling_marker_symlink_is_unreadable_rather_than_never_recorded() {
        // A broken link reads NotFound exactly like an absent path, and the
        // two are opposite states: nothing recorded yet is a fresh machine,
        // while a link whose target went away is bookkeeping that STOPPED
        // resolving. Read as "never recorded" it reports a fresh machine
        // forever and no gap is ever measured again.
        let link = scratch("marker-dangling");
        std::os::unix::fs::symlink("uu-absent-target", &link).expect("the link");
        let marker = read(&link);
        std::fs::remove_file(&link).ok();
        assert_eq!(marker, Marker::Unreadable);
    }
}
