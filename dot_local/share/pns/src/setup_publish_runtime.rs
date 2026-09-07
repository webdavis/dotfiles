use crate::*;

/// Publish the composed config, keeping the old one when replacing it.
///
/// CREATE-IF-ABSENT, NEVER A BLANKET RENAME, on both paths: a config that
/// appeared between the check in `setup_mode` and this moment is another
/// writer's, and this run has not read it. The link failing with
/// `AlreadyExists` IS that refusal. NOTHING ASKS WHETHER A CONFIG IS THERE
/// either, because the answer stops being true the instant it is given: what
/// `--force` moves aside is the file it found at the name, and what it
/// publishes into is a name it emptied itself.
///
/// THE OLD CONFIG IS MOVED ASIDE RATHER THAN COPIED ASIDE, so the backup holds
/// what was actually replaced rather than what stood there when a copy ran, and
/// the old config is at one of the two names at every instant.
///
/// THE PENDING FILE CARRIES THE MODE, because it is what gets published:
/// writing at the umask would publish a config whose plugin secrets any
/// process on the machine can read.
pub(crate) fn publish_config(
    path: &Path,
    composed: &str,
    force: bool,
) -> Result<Option<PathBuf>, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory to write in", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("{} could not be created: {error}", parent.display()))?;
    let pending = parent.join(pending_name());
    // CREATED OR NOT AT ALL, and never opened. A pending file is a second name
    // for the live config between the link that publishes it and the unlink
    // that removes it, so an abandoned run leaves one behind and process ids
    // are reused: an open that truncates would empty a config this run has not
    // read, and the backup taken next would hold the replacement.
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&pending)
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;
    let published = write_then_publish(path, &pending, file, composed, force);
    // WHICHEVER WAY IT WENT, and only ever the file the line above made: a
    // pending file left in the config directory would be read by nobody and
    // found by everybody, and removing one this run did not create is the
    // mirror of the write it refuses to do.
    let _ = std::fs::remove_file(&pending);
    published
}

/// The name the composed config is written under before it is published.
///
/// THE MOMENT AS WELL AS THE PROCESS, because the create above is exclusive: a
/// leftover from an abandoned run of the same id would otherwise refuse a
/// wizard nobody can unblock, and a name nothing else is holding is also a
/// name nothing else can be waiting at.
fn pending_name() -> String {
    format!(
        "config.toml.new.{}.{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.subsec_nanos())
    )
}

/// The publish itself, with `publish_config` owning the cleanup around it.
fn write_then_publish(
    path: &Path,
    pending: &Path,
    mut file: std::fs::File,
    composed: &str,
    force: bool,
) -> Result<Option<PathBuf>, String> {
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: the mode an
    // open asks for is masked by the umask, and a config published without the
    // operator's own bits is one they cannot read.
    file.set_permissions(std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
        .map_err(|error| format!("{} could not be secured: {error}", pending.display()))?;
    file.write_all(composed.as_bytes())
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;

    // THE FORCED PATH EMPTIES THE NAME FIRST, and what it moves out of the way
    // is the backup. Nothing here asks whether a config is there: the move
    // itself is the answer, and it is the same answer a moment later.
    let kept = if force { keep_aside(path)? } else { None };
    // AND BOTH PATHS PUBLISH THE SAME WAY. A link that refuses an occupied
    // name cannot write over a config this run never read: after the dangling
    // symlink pre-check in `setup_mode`, the only way a config can be
    // standing here is a genuine arrival while the questions were being
    // answered, so "appeared" below is exact rather than one of two guesses.
    match std::fs::hard_link(pending, path) {
        Ok(()) => Ok(kept),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
            "{} appeared while the questions were being answered; \
             nothing was written over it{}",
            path.display(),
            also_kept(kept.as_deref())
        )),
        Err(error) => Err(format!(
            "{} could not be written: {error}{}",
            path.display(),
            also_kept(kept.as_deref())
        )),
    }
}

/// The tail a refusal carries when this run had already moved a config aside,
/// so nobody is left hunting for a file the wizard took the name of.
fn also_kept(kept: Option<&Path>) -> String {
    kept.map_or_else(String::new, |backup| {
        format!(
            "; the config that was there is kept at {}",
            backup.display()
        )
    })
}

/// Move the existing config aside, and answer with where it went.
///
/// A MOVE RATHER THAN A COPY, which is what makes the answer true: a copy says
/// only what stood at the name when the copy ran, and the publish that follows
/// replaces whatever stands there THEN. Moving it is the one act that both
/// keeps the old config and frees the name, so the two can never disagree.
///
/// NOTHING TO MOVE IS NOT A FAILURE: `--force` on a machine with no config is
/// an ordinary first run.
fn keep_aside(path: &Path) -> Result<Option<PathBuf>, String> {
    let now = now_secs().ok_or_else(|| {
        "the clock cannot be read, so the config already there cannot be named \
         and kept; nothing was written"
            .to_string()
    })?;
    keep_aside_at(path, now)
}

/// `keep_aside` with the moment NAMED rather than read.
///
/// THE SPLIT EXISTS FOR THE TEST, and the test is what makes it worth having.
/// With the clock read in here, a test that pre-claims a backup name has to
/// read the clock itself and hope neither read lands on the far side of a
/// second boundary. Pre-claiming both candidate names only narrows that
/// window: a thread parked across more than one boundary still picks a third
/// name and the test fails on a working build. Naming the second removes the
/// race instead of shrinking it.
fn keep_aside_at(path: &Path, epoch_secs: u64) -> Result<Option<PathBuf>, String> {
    let backup = pns::setup::backup_path(path, epoch_secs).ok_or_else(|| {
        format!(
            "{} cannot be named for keeping, so the config already there \
             cannot be kept; nothing was written",
            path.display()
        )
    })?;
    // THE NAME IS CLAIMED BEFORE ANYTHING MOVES ONTO IT, so a second forced run
    // inside the same second refuses rather than writing over the copy the
    // first one kept: a rename would replace that copy without a word.
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&backup)
        .map_err(|error| match error.kind() {
            // THE NAME BEING TAKEN PROVES NOTHING ABOUT WHAT IT HOLDS: a run
            // killed between this claim and the rename that follows it
            // leaves an empty file at the same name, so the refusal says
            // only that the name is spoken for, not what a prior run "kept"
            // there.
            std::io::ErrorKind::AlreadyExists => format!(
                "{} is already claimed by another run this same second; \
                 nothing was written",
                backup.display()
            ),
            // ANY OTHER FAILURE IS ITS OWN REASON: naming the same-second
            // collision for a permission refusal would blame a run that
            // never happened.
            _ => format!("{} could not be claimed: {error}", backup.display()),
        })?;
    if let Err(error) = std::fs::rename(path, &backup) {
        // THE CLAIM GOES WITH THE RUN THAT MADE IT, whether there was nothing
        // to move or the move could not be made: an empty file named like a
        // backup is worse than no backup at all.
        let _ = std::fs::remove_file(&backup);
        return match error.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            // THE BACKUP WAS NEVER THE PROBLEM HERE: it is a fresh file this
            // call just created, and what could not be moved onto it is
            // `path` itself, so the refusal names that instead.
            _ => Err(format!(
                "{} could not be moved aside to keep it: {error}",
                path.display()
            )),
        };
    }
    // AS PRIVATE AS THE CONFIG IT HOLDS, when what moved is a file at all: the
    // mode of a symlink is the mode of what it points at, and this one points
    // at a file this run did not replace and has no business changing.
    if backup.symlink_metadata().is_ok_and(|entry| entry.is_file()) {
        std::fs::set_permissions(&backup, std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
            .map_err(|error| format!("{} could not be secured: {error}", backup.display()))?;
    }
    Ok(Some(backup))
}

/// The config carries every plugin's secret, so it is the operator's alone.
const CONFIG_FILE_MODE: u32 = 0o600;

#[cfg(test)]
#[path = "setup_publish_runtime/tests.rs"]
mod setup_publish_runtime_tests;
