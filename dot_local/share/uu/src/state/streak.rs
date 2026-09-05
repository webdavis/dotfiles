//! One lane's non-success streak: the file per lane that makes a lane going
//! quiet visible, and the pruning of the ones a config no longer declares.
//!
//! `staleness` decides what the next count is and when it trips; this reads
//! and publishes it. NOTHING HERE FAILS OPEN: a streak this run could not
//! trust is never read as zero, and a write that did not land is returned to
//! the caller rather than swallowed.

use std::path::{Path, PathBuf};

use unattended_upgrades::config::Config;

/// Where a lane's non-success streak lives: one small file per lane, named for
/// the lane itself so two lanes never share bookkeeping.
pub fn path(home: &str, lane: &str) -> PathBuf {
    lanes_dir(home).join(lane).join("streak")
}

fn lanes_dir(home: &str) -> PathBuf {
    super::dir(home).join("lanes")
}

/// What reading a lane's streak file found.
///
/// `Absent` COVERS ONLY `NotFound`: that is the one case that legitimately
/// means a fresh lane, or one that has never had a non-success run. Anything
/// else the file could say (unreadable, a directory sitting where the file
/// belongs, content that is not a plain count) is `Unreadable`, never a
/// silent zero: zero would forgive whatever streak the file actually held,
/// which is the fail-open this whole capability exists to refuse.
#[derive(Debug, PartialEq, Eq)]
pub enum Streak {
    Absent,
    Value(u32),
    Unreadable(String),
}

pub fn read(path: &Path) -> Streak {
    match std::fs::read_to_string(path) {
        Ok(text) => match text.trim().parse() {
            Ok(value) => Streak::Value(value),
            Err(_) => Streak::Unreadable(format!("{:?} is not a plain count", text.trim())),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Streak::Absent,
        Err(error) => Streak::Unreadable(error.to_string()),
    }
}

/// Publish a lane's streak, ATOMICALLY: a sibling temp file, written in full
/// and then renamed over the target. `rename` only needs write permission on
/// the DIRECTORY, never on the file it replaces, so a streak file an earlier
/// run left read-only no longer blocks every write after it the way
/// truncating in place did; only a directory this run cannot write to still
/// fails, and that failure is returned rather than swallowed, so the caller
/// can make it loud instead of silent.
pub fn write(path: &Path, value: u32) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "the streak path has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    // NAMED FOR THE TARGET, not only the process: every lane's own streak
    // lives in its own directory in production, but the unit tests below
    // exercise several DIFFERENT target files under the same shared temp
    // parent, in the same process, in parallel. A temp name keyed on the
    // process id alone collided across those targets; keying it on the
    // target's own file name as well makes two DIFFERENT targets in the SAME
    // directory use different temp files even when the process id matches.
    let target_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("streak");
    let temp = parent.join(format!(".{target_name}-{}.tmp", std::process::id()));
    std::fs::write(&temp, format!("{value}\n")).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, path).map_err(|error| error.to_string())
}

/// The lane names this config no longer declares are dropped here, under the
/// run lock: a directory left behind by a removed or renamed lane would leak
/// forever otherwise, and a NEW lane reusing the old name would inherit its
/// streak and could alert on its very first miss. Best effort and silent on
/// its own failure, matching every other piece of this bookkeeping: a stale
/// directory that resists cleanup costs nothing but a few bytes, never a
/// wrong verdict.
pub fn prune_removed_lanes(home: &str, config: &Config) {
    let Ok(entries) = std::fs::read_dir(lanes_dir(home)) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !config.lanes.contains_key(&name) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::scratch;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_path_with_no_streak_at_it_is_absent_which_is_the_only_case_that_reads_as_zero() {
        assert_eq!(read(&scratch("streak-absent")), Streak::Absent);
    }

    #[test]
    fn a_streak_file_that_does_not_parse_as_a_count_is_unreadable_not_a_silent_zero() {
        // BEFORE THIS FIX this read as zero, which silently forgives whatever
        // streak a half-written or corrupted file actually held: a lane one
        // run short of tripping would quietly restart its count from
        // scratch instead of the operator ever hearing about it.
        let path = scratch("streak-garbage");
        std::fs::write(&path, "not-a-number\n").expect("the file");
        let read = read(&path);
        std::fs::remove_file(&path).ok();
        assert!(matches!(read, Streak::Unreadable(_)), "{read:?}");
    }

    #[test]
    fn a_streak_file_this_process_cannot_read_is_unreadable_not_absent() {
        // Distinct from `Absent`: the file IS there, so a lane whose history
        // this run cannot see must not be told it has none.
        let path = scratch("streak-unreadable-mode");
        std::fs::write(&path, "2\n").expect("the file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("mode");
        let read = read(&path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).ok();
        std::fs::remove_file(&path).ok();
        assert!(matches!(read, Streak::Unreadable(_)), "{read:?}");
    }

    #[test]
    fn a_written_streak_is_the_streak_read_back() {
        let path = scratch("streak-roundtrip");
        write(&path, 2).expect("the write");
        let read = read(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(read, Streak::Value(2));
    }

    #[test]
    fn writing_a_streak_creates_its_parent_directory() {
        let path = std::env::temp_dir().join(format!(
            "uu-main-streak-parent-{}/lanes/mine/streak",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
        write(&path, 1).expect("the write");
        let read = read(&path);
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).ok();
        assert_eq!(read, Streak::Value(1));
    }

    #[test]
    fn a_streak_file_made_read_only_is_still_overwritten_by_the_next_run() {
        // ROW 2, DIRECTION B reproduced by hand before this fix: a streak
        // file made read-only after being written stayed stuck at its old
        // value forever, because a plain `fs::write` truncates the EXISTING
        // file in place and needs write permission on it. Publishing through
        // a rename needs write permission on the DIRECTORY only, so the same
        // read-only file no longer blocks the write at all.
        let path = scratch("streak-readonly-file");
        write(&path, 2).expect("the first write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("mode");
        let result = write(&path, 3);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).ok();
        result.expect("a rename over a read-only file must still succeed");
        let read = read(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(read, Streak::Value(3));
    }

    #[test]
    fn a_streak_write_whose_directory_cannot_be_created_reports_why_rather_than_staying_silent() {
        // ROW 2, DIRECTION A reproduced by hand before this fix: a plain FILE
        // sitting where the lane's directory belongs makes `create_dir_all`
        // fail on every run, and the old `write_streak` only printed to
        // stderr and moved on, so a lane stuck this way never once reached
        // the staleness threshold: the count could never actually persist.
        let blocker = scratch("streak-blocked-parent");
        std::fs::write(&blocker, "").expect("a plain file occupying the would-be directory");
        let path = blocker.join("streak");
        let error = write(&path, 1).expect_err("a file cannot become a directory");
        std::fs::remove_file(&blocker).ok();
        assert!(!error.is_empty(), "{error:?}");
    }
}
