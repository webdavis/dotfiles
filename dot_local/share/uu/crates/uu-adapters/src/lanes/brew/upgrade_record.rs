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
use uu_domain::RunFacts;

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
mod tests;
