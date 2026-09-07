use super::*;

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
pub(super) fn publish_job(spool: &Path, job: &Job) -> std::io::Result<()> {
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
pub(super) fn pending_for(spool: &Path, id: &str) -> PathBuf {
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
pub(super) fn publish(path: &Path, pending: &Path, line: &str) -> std::io::Result<()> {
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
pub(super) fn publish_if_absent(path: &Path, pending: &Path, line: &str) -> std::io::Result<bool> {
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
pub(super) fn stage(path: &Path, pending: &Path, line: &str) -> std::io::Result<()> {
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
