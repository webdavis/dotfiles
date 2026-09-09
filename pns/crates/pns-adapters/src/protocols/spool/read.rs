use super::*;

/// How many jobs are spooled. A COUNT AND NEVER THE CONTENTS, following the
/// missed journal's structural privacy rule: the doctor answers "is anything
/// scheduled" and nothing here becomes a reader of what.
/// REGULAR FILES ONLY, so the word "job" in the doctor's sentence is earned. A
/// FIFO or a directory in the spool is something the loop refuses to open and
/// will never run, and counting it would report a job that cannot exist.
pub fn job_count(state_dir: &Path) -> usize {
    spool_entries(&spool_dir(state_dir))
        .into_iter()
        .filter(|entry| matches!(std::fs::symlink_metadata(entry), Ok(found) if found.is_file()))
        .count()
}

/// Every spool entry that could be a job, sorted so a tick is deterministic.
///
/// THIS MODULE'S OWN WORKING FILES ARE SKIPPED by their prefix, and an id can
/// never carry it, so a claim in flight is never mistaken for a job.
pub fn spool_entries(spool: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(spool)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with(WORKING_PREFIX)
        })
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries
}

/// What one look at a spool entry found.
#[derive(Debug, PartialEq, Eq)]
pub enum Peeked {
    /// A record this daemon will act on.
    Job(Box<Job>),
    /// Not a regular file. LEFT ALONE AND NEVER OPENED, following
    /// `append_ring_line`'s own refusal at a state path: a FIFO here would
    /// block the read forever and stall every later tick, and a symlink is a
    /// write somewhere this tool did not choose.
    Irregular,
    /// A regular file that is not a usable record. Dropped rather than guessed
    /// at, carrying the reason.
    Unusable(String),
}

/// One look at a spool entry, taken WITHOUT claiming it.
///
/// THE PEEK IS READ-ONLY, so a job that is merely waiting is left exactly where
/// it was found: nothing is renamed, nothing is rewritten, and a registration
/// arriving in the same second cannot be overwritten by a put-back of the
/// record this tick had already read. A read-only peek is enough to decide to
/// do NOTHING; every decision that acts is taken again on a claimed record.
///
/// `expect_id` IS THE ID THE SPOOL FILENAME PROMISED, and a record that says a
/// different one is refused rather than acted on. The id is what a repeat
/// republishes under and what a cancel removes, so a file `A` whose record says
/// `id=B` would let a job re-arm itself on top of an unrelated one. On the
/// claim path the same name is passed, because a claim is the same record under
/// a working name and its id must still be the one it was published as.
pub fn peek(entry: &Path, expect_id: &str) -> Peeked {
    if !matches!(std::fs::symlink_metadata(entry), Ok(found) if found.is_file()) {
        return Peeked::Irregular;
    }
    let mut text = String::new();
    let read = std::fs::File::open(entry).and_then(|file| {
        // CAPPED AT ONE BYTE PAST THE RECORD CAP, so a file over the cap still
        // arrives over it and the parse refuses it rather than reading a
        // truncated record as a whole one.
        Read::take(file, RECORD_MAX as u64 + 1).read_to_string(&mut text)
    });
    if let Err(error) = read {
        return Peeked::Unusable(format!("it could not be read ({})", error.kind()));
    }
    match parse(text.trim_end_matches('\n')) {
        Err(refusal) => Peeked::Unusable(refusal),
        // THE SAME RULES THE REGISTRATION APPLIED, so a hand-edited spool file
        // cannot do what a registration could not.
        Ok(job) if job.id != expect_id => Peeked::Unusable(format!(
            "its `id` is `{}`, which is not the `{expect_id}` it was spooled as",
            job.id
        )),
        Ok(job) => match validate_shape(&job) {
            Err(refusal) => Peeked::Unusable(refusal),
            Ok(()) => Peeked::Job(Box::new(job)),
        },
    }
}

/// Whether the marker that cancels this job is there.
///
/// `symlink_metadata`, so a dangling symlink still counts as present: the
/// question is whether something wrote the marker, not whether it resolves.
/// A job with no marker is never cancelled by one.
///
/// THE DIRECTORY IS CHECKED BEFORE ANY NAME INSIDE IT, and a symlink standing
/// where it should be is refused, matching the spool's own startup refusal. A
/// validated name cannot escape the state directory by itself, but a link at
/// the directory carries the whole lookup somewhere this tool did not choose,
/// which turns the field back into the general filesystem probe the name rule
/// exists to prevent.
///
/// A REFUSED DIRECTORY READS AS NO MARKER, so the job runs. That is the fail
/// direction the rest of this crate takes: a marker that cannot be trusted
/// cancels nothing, and the cost is one extra card rather than a cancellation
/// somebody else's symlink decided.
pub fn marker_exists(state_dir: &Path, job: &Job) -> bool {
    let Some(marker) = job.unless_marker.as_ref() else {
        return false;
    };
    if !name_is_safe(marker) {
        return false;
    }
    let directory = marker_dir(state_dir);
    if !matches!(std::fs::symlink_metadata(&directory), Ok(found) if found.is_dir()) {
        return false;
    }
    std::fs::symlink_metadata(directory.join(marker)).is_ok()
}

/// One spool entry taken by rename, or None when it is already gone.
///
/// THE RENAME IS THE OWNERSHIP TEST, and a plain unlink is not one: measured on
/// macOS 26.2 (APFS) and recorded in `take_claim`'s own doc comment, eight
/// processes unlinking one path were every one of them told they had succeeded,
/// while 40 rounds of eight racers renaming gave exactly one winner every time.
/// So two daemons cannot both run one occurrence: the claim is taken BEFORE the
/// record is read for anything the daemon acts on, and the loser reads nothing.
///
/// THE HELD NAME CARRIES A PER-RUN SEQUENCE as well as the pid, for the reason
/// `take_claim`'s does: one name per process couples every claim in a run to
/// the first one, and a claim the run could not finish then occupies the name.
pub fn claim(entry: &Path) -> Option<PathBuf> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CLAIM_SEQ: AtomicU32 = AtomicU32::new(0);
    let name = entry.file_name()?;
    let claim = entry.with_file_name(format!(
        "{WORKING_PREFIX}claim.{}.{}.{}",
        std::process::id(),
        CLAIM_SEQ.fetch_add(1, Ordering::Relaxed),
        name.to_string_lossy()
    ));
    // NEVER RENAMED OVER A CLAIM ALREADY THERE, because a rename OVERWRITES and
    // the name is this run's alone: anything sitting at it is a job this
    // process claimed and could not finish, and losing it silently is worse
    // than leaving it.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return None;
    }
    std::fs::rename(entry, &claim).ok()?;
    Some(claim)
}
