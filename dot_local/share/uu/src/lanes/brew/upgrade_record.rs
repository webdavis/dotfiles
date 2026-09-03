//! The durable record of what one run moved, for something days later to ask
//! whether an upgrade plausibly explains a file that changed.
//!
//! WHO READS IT. The osquery file-integrity page fires when a watched file
//! leaves its known-good manifest, and a vendor update and a tamper used to
//! render the same body. That page carries a correlation line built from this
//! file, by the literal path in
//! `~/.local/libexec/osquery/results-alerter/file-integrity-triage.sh`. The
//! record is a LEAD there and is labelled as one: it lives in an
//! operator-writable state directory, so it is not a trust input, and nothing
//! about it can suppress or downgrade a page.
//!
//! PUBLISHED TWICE, ONE RUN, at the same timestamp: the run line alone before
//! the first upgrade step, then the whole thing again with the rows once the
//! after reading is taken. Written only at the end, the whole upgrade window
//! is uncovered, and a file rewritten in the first seconds of a run is
//! correlated against the PREVIOUS week.

use std::fs;
use std::path::PathBuf;

use crate::config::BrewLane;
use crate::record::RunFacts;

/// Persist what this run moved, or answer WHY nothing was written.
///
/// BEST EFFORT: upgrading matters more than bookkeeping, so nothing here is a
/// failed step. A silently absent record is the invisibility it exists to end,
/// though, so the caller states the reason in the record.
///
/// THE RUN'S OWN CLOCK, read once by uu and carried in `RunFacts`: the epoch
/// is what the reader does arithmetic on and the ISO string is what it
/// renders, and two readings could disagree, which here would make one run
/// look like two.
pub fn publish(
    lane: &BrewLane,
    facts: &RunFacts,
    comparable: bool,
    rows: &[String],
) -> Option<String> {
    if lane.upgrade_record.is_empty() {
        return Some("no `upgrade_record` is configured".to_string());
    }
    if !comparable {
        return Some("the package listing could not be read".to_string());
    }
    if facts.started_epoch <= 0 {
        return Some("this run's clock could not be read".to_string());
    }
    write(
        &lane.upgrade_record,
        facts.started_epoch,
        facts.started_iso,
        rows,
    )
    .err()
}

/// Write the record, atomically. `rows` is empty for the opening publish and
/// carries one `tuple_row` per moved name for the closing one.
///
/// TEMP FILE AND RENAME, so a reader mid-write sees the previous record whole
/// rather than a torn one.
pub fn write(path: &str, epoch: i64, iso: &str, rows: &[String]) -> Result<(), String> {
    let destination = PathBuf::from(path);
    if let Some(parent) = destination.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let mut body = format!("{epoch}\t{iso}\n");
    for row in rows {
        body.push_str(row);
        body.push('\n');
    }
    let temporary = PathBuf::from(format!("{path}.tmp"));
    fs::write(&temporary, body)
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, &destination).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("could not install {path}: {error}")
    })
}

#[cfg(test)]
mod tests {
    use super::super::repairs::tests::lane;
    use super::super::tests::facts;
    use super::*;

    /// A directory of this test's own. Removed at the end of each test; a
    /// panicking test leaves one behind in TMPDIR, which is the trade for not
    /// taking a dependency to own a temp directory.
    fn scratch(tag: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "uu-brew-upgrade-record-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a scratch directory");
        directory
    }

    #[test]
    fn the_opening_publish_dates_the_run_and_names_nothing_yet() {
        let directory = scratch("opening");
        let path = directory.join("last-upgrade-changes.tsv");
        write(
            path.to_str().unwrap(),
            1_760_000_000,
            "2025-10-09T07:33:20Z",
            &[],
        )
        .expect("the record is written");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "1760000000\t2025-10-09T07:33:20Z\n"
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_closing_publish_keeps_the_same_timestamp_and_adds_what_moved() {
        // The two publishes are ONE record at two levels of detail. A second
        // clock reading would make one run look like two to the reader.
        let directory = scratch("closing");
        let path = directory.join("last-upgrade-changes.tsv");
        let name = path.to_str().unwrap();
        write(name, 1_760_000_000, "2025-10-09T07:33:20Z", &[]).expect("the opening publish");
        write(
            name,
            1_760_000_000,
            "2025-10-09T07:33:20Z",
            &["jq\tchanged\t1.7.0\t1.7.1".to_string()],
        )
        .expect("the closing publish");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "1760000000\t2025-10-09T07:33:20Z\njq\tchanged\t1.7.0\t1.7.1\n"
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_state_directory_is_created_rather_than_assumed() {
        let directory = scratch("missing-parent");
        let path = directory.join("state").join("last-upgrade-changes.tsv");
        write(path.to_str().unwrap(), 1, "1970-01-01T00:00:01Z", &[])
            .expect("the parent is created on the way");
        assert!(path.exists());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn nothing_is_left_beside_the_record_once_it_is_installed() {
        // The temp file is the write's own; a reader that globbed the state
        // directory would otherwise find two records.
        let directory = scratch("no-leftovers");
        let path = directory.join("last-upgrade-changes.tsv");
        write(path.to_str().unwrap(), 1, "1970-01-01T00:00:01Z", &[]).expect("written");
        let left: Vec<String> = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(left, vec!["last-upgrade-changes.tsv".to_string()]);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_run_that_could_read_what_is_installed_publishes_the_record() {
        let directory = scratch("published");
        let path = directory.join("last-upgrade-changes.tsv");
        let mut recording = lane();
        recording.upgrade_record = path.to_str().unwrap().to_string();
        assert_eq!(publish(&recording, &facts(), true, &[]), None);
        assert!(path.exists());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_listing_that_could_not_be_read_writes_no_record_and_says_why() {
        // A record dated this run and naming nothing reads to the
        // file-integrity page exactly like a week that moved nothing.
        let directory = scratch("unreadable");
        let path = directory.join("last-upgrade-changes.tsv");
        let mut recording = lane();
        recording.upgrade_record = path.to_str().unwrap().to_string();
        let why = publish(&recording, &facts(), false, &[]).expect("nothing is written");
        assert!(why.contains("could not be read"), "{why}");
        assert!(!path.exists(), "no record is better than a blind one");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_lane_with_no_record_configured_says_so_rather_than_guessing_a_path() {
        let why = publish(&lane(), &facts(), true, &[]).expect("nothing is written");
        assert!(why.contains("upgrade_record"), "{why}");
    }

    #[test]
    fn a_clock_that_could_not_be_read_writes_no_record() {
        // uu renders an unreadable clock as epoch 0, and a record dated 1970
        // would be older than every window the page asks about.
        let directory = scratch("no-clock");
        let path = directory.join("last-upgrade-changes.tsv");
        let mut recording = lane();
        recording.upgrade_record = path.to_str().unwrap().to_string();
        let mut stopped = facts();
        stopped.started_epoch = 0;
        let why = publish(&recording, &stopped, true, &[]).expect("nothing is written");
        assert!(why.contains("clock"), "{why}");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&directory);
    }
}
