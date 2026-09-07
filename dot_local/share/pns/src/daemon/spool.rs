//! The spool directory itself: where a job's record lives, how one is
//! published, claimed, handed back and cancelled, and what a hostile directory
//! costs.
//!
//! EVERY TRANSACTION IS A RENAME, so a reader never sees a half-written
//! record. What a job MEANS is decided in `pns-domain`; this decides only what
//! reaches the disk.

use super::{
    Heartbeat, Job, RECORD_MAX, name_is_safe, parse, render, render_heartbeat,
    validate_registration, validate_shape,
};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Where jobs are spooled, one file per job named by its id.
pub fn spool_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon")
}

/// Where the markers that cancel jobs live. A job carries a marker NAME, never
/// a path, and it is resolved here, so the field cannot become a general
/// filesystem probe.
pub fn marker_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon-markers")
}

/// Where the daemon says it is alive.
///
/// BESIDE THE SPOOL AND NOT INSIDE IT: a heartbeat file in the spool directory
/// would be read as a job every tick, refused as unparseable and dropped, so
/// the daemon would spend its life deleting its own pulse.
pub fn heartbeat_path(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon-heartbeat")
}

/// The mode every file this module writes carries, matching every other state
/// file the crate publishes.
const STATE_FILE_MODE: u32 = 0o600;

/// The prefix this module's own working files carry, and which no valid id can
/// start with.
///
/// `~` IS OUTSIDE THE ID CHARSET, which is what makes this a rule rather than a
/// convention: a claim and a pending write both live in the spool directory,
/// and the scan has to be able to tell them from a job without parsing them.
const WORKING_PREFIX: &str = "~";

/// What a start found where the spool should be.
#[derive(Debug, PartialEq, Eq)]
pub enum Startup {
    /// The spool is a directory and the loop may run.
    Ready,
    /// It may not, and the line saying why.
    ///
    /// EVERY REFUSAL HERE IS PERMANENT, which is the whole reason this is a
    /// type rather than a bool: relaunching cannot turn a symlink into a
    /// directory or make an unwritable state directory writable, so the caller
    /// exits 0 and lets `KeepAlive { SuccessfulExit = false }` keep the job
    /// DOWN. Exiting non-zero would relaunch it every ten seconds forever,
    /// which is the atuin restart loop (~6000 attempts in production) arriving
    /// through the refusal door instead of the crash door. A transient failure
    /// would belong in a second variant and there is none today.
    Refused(String),
}

/// The spool directory, made if it is missing and REFUSED rather than repaired
/// if something else is standing there.
///
/// `create_dir_all` FOLLOWS A SYMLINK, so a link where the spool should be
/// would silently put every job somewhere this tool did not choose. Checked
/// with `symlink_metadata` first, following `append_ring_line`'s own refusal at
/// a state path.
pub fn prepare_spool(state_dir: &Path) -> Startup {
    let spool = spool_dir(state_dir);
    if let Ok(found) = std::fs::symlink_metadata(&spool)
        && !found.is_dir()
    {
        return Startup::Refused(format!(
            "{} is not a directory; refusing to start",
            spool.display()
        ));
    }
    if let Err(error) = std::fs::create_dir_all(&spool) {
        return Startup::Refused(format!("the spool directory could not be made ({error})"));
    }
    Startup::Ready
}

/// Register one job: validated, then written by rename.
///
/// THE ERROR IS RETURNED, NEVER PRINTED. Every caller states its own fail
/// direction, and the one this exists for (a hook registering a nudge) drops it
/// the way a log line is dropped: silently, locally, and without touching the
/// return value of the thing that called it.
pub fn schedule(state_dir: &Path, job: &Job, now: u64) -> Result<(), String> {
    validate_registration(job, now)?;
    publish_job(&spool_dir(state_dir), job)
        .map_err(|error| format!("the spool write failed: {error}"))
}

/// Forget one job by id. Answers whether there was one.
pub fn cancel(state_dir: &Path, id: &str) -> Result<bool, String> {
    if !name_is_safe(id) {
        return Err(format!("`{id}` is not a job id"));
    }
    match std::fs::remove_file(spool_dir(state_dir).join(id)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("the spool entry could not be removed: {error}")),
    }
}

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

/// One job written into the spool by rename, replacing whatever the id named.
///
/// A CLIENT'S WRITE, and the overwrite is the point: re-registering an id is a
/// REFRESH rather than a second job, so newest-signal-wins is what a rename
/// gives for free.
///
/// PRIVATE, WHICH IS THE ENFORCEMENT. `schedule` is the only way in, and the
/// daemon's own side of the library has `hand_back` and nothing else, so the
/// loop CANNOT overwrite a client's registration even by mistake: the call that
/// would do it is not in scope where the loop is written.
fn publish_job(spool: &Path, job: &Job) -> std::io::Result<()> {
    publish(
        &spool.join(&job.id),
        &pending_for(spool, &job.id),
        &render(job),
    )
}

/// One record the DAEMON holds put back into the spool, answering `true` when
/// it went back under its id and `false` when a client had already written
/// there and its record was left alone.
///
/// THE DAEMON'S ONLY WRITE, AND IT NEVER OVERWRITES A CLIENT. A re-arm and a
/// put-back are both this daemon restating a record it read moments ago; a
/// client registering the same id in that window has published a NEWER signal,
/// and a rename would silently replace it with the older one, taking its due,
/// its lease and its argv with it. `hard_link` fails with `AlreadyExists`
/// instead, so the client's record stands and the daemon's stale copy is thrown
/// away. That is the invariant the whole id-is-the-filename refresh rule rests
/// on, and it is the one a peek-then-claim loop could not keep.
///
/// `hard_link` RATHER THAN `create_new`, so the file that lands is the one the
/// temp already carries: mode, bytes and all, published in one step the way the
/// rename publishes. There is no window in which a reader can see the name with
/// nothing behind it.
pub fn hand_back(spool: &Path, job: &Job) -> std::io::Result<bool> {
    publish_if_absent(
        &spool.join(&job.id),
        &pending_for(spool, &job.id),
        &render(job),
    )
}

/// The private name a pending write is staged under. One per process and per
/// id, and outside the id charset, so a stage in flight is never read as a job.
fn pending_for(spool: &Path, id: &str) -> PathBuf {
    spool.join(format!(
        "{WORKING_PREFIX}pending.{}.{id}",
        std::process::id()
    ))
}

/// The daemon's own pulse, published the same way.
pub fn publish_heartbeat(state_dir: &Path, beat: &Heartbeat) -> std::io::Result<()> {
    publish(
        &heartbeat_path(state_dir),
        &state_dir.join(format!(
            "{WORKING_PREFIX}pending.{}.daemon-heartbeat",
            std::process::id()
        )),
        &render_heartbeat(beat),
    )
}

/// One line published atomically at 0600: `publish_state_line`'s shape, stated
/// here because that one is private to the composition root.
///
/// PUBLISHED BY RENAME. A plain write truncates first, so a reader landing
/// between the truncate and the bytes sees an empty file, which every reader of
/// these files reads as no state at all. The pending path sits in the SAME
/// directory, because a rename across filesystems is not one, and it carries
/// this process's id so two runs publishing at once cannot share one.
fn publish(path: &Path, pending: &Path, line: &str) -> std::io::Result<()> {
    stage(path, pending, line)?;
    if let Err(error) = std::fs::rename(pending, path) {
        // Nothing half-written is left for the next tick to trip over.
        let _ = std::fs::remove_file(pending);
        return Err(error);
    }
    Ok(())
}

/// The same line published only when the name is FREE, answering whether it
/// landed there.
///
/// A LINK RATHER THAN A RENAME, because a rename has no create-if-absent form
/// and `link(2)` is the one call that publishes a complete file and refuses an
/// occupied name in the same step. The temp is unlinked either way, so a name
/// somebody else won leaves nothing behind.
fn publish_if_absent(path: &Path, pending: &Path, line: &str) -> std::io::Result<bool> {
    stage(path, pending, line)?;
    let landed = match std::fs::hard_link(pending, path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    };
    let _ = std::fs::remove_file(pending);
    landed
}

/// The bytes written to their private name, ready to be published under the
/// real one.
fn stage(path: &Path, pending: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // THE PENDING FILE CARRIES THE MODE, because publishing it is a rename or a
    // link and neither one sets a mode.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(pending)?;
    // AND AGAIN AFTER THE OPEN, because `mode` above applies only when the open
    // CREATES the file, and a run interrupted before its publish leaves one for
    // the next run of that pid to reuse.
    file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))?;
    file.write_all(format!("{line}\n").as_bytes())?;
    Ok(())
}
#[cfg(test)]
mod spool_tests;
