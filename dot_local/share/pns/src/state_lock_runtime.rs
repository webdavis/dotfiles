use crate::*;

/// One named lock taken, or false when somebody live already holds it.
///
/// THE SHAPE EVERY LOCK IN THIS BINARY USES, and it is one function because its
/// two halves are only correct together: an exclusive create arbitrates between
/// racers, and the age rule is what stops a holder that died from wedging the
/// path forever. What differs between callers is the NAME and how long a holder
/// is believed, so those are the parameters and the mechanism is not repeated.
pub(crate) fn claim_lock(lock: &Path, now: u64, stale_secs: u64) -> bool {
    if publish_lock(lock).is_ok() {
        return true;
    }
    // Somebody holds it. A live holder is one this process stands down for.
    if !lock_aged_out(lock, now, stale_secs) {
        return false;
    }
    // THE DEAD LOCK IS TAKEN BY RENAME AND NEVER BY REMOVE, which is the one
    // place arbitration is still needed on this path: a remove reports success
    // to EVERY racer on APFS (measured, eight racers all told they had
    // succeeded), so two processes clearing one dead lock would each then create
    // a fresh one and both would own the window. A rename does arbitrate.
    let claim = pns::nag::claim_path(lock, std::process::id());
    if std::fs::symlink_metadata(&claim).is_ok() {
        return false;
    }
    if std::fs::rename(lock, &claim).is_err() {
        return false;
    }
    let _ = std::fs::remove_file(&claim);
    publish_lock(lock).is_ok()
}

/// The lock published, or an error when somebody already holds it.
///
/// EXCLUSIVE, so of any number of processes racing this exactly one is told it
/// succeeded, and it NEVER FOLLOWS A LINK: an exclusive create fails on a
/// symlink at the path rather than opening what it points at.
fn publish_lock(lock: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(lock)
        .map(|_| ())
}

/// Whether a lock already on disk is old enough to be the leavings of a crash.
///
/// A LOCK WHOSE OWN CLOCK CANNOT BE READ COUNTS AS LIVE and stands the caller
/// down. That is the safe direction (one window lost, never two holders), and
/// the case behind it is a lock that vanished between the failed create and the
/// question, which the next attempt resolves anyway.
fn lock_aged_out(lock: &Path, now: u64, stale_secs: u64) -> bool {
    std::fs::symlink_metadata(lock)
        .ok()
        .as_ref()
        .and_then(modified_at)
        .is_some_and(|at| now.saturating_sub(at.as_secs()) > stale_secs)
}
/// A lock held for as long as this value is alive, and given back when it is
/// dropped. Shared by every `claim_lock` caller with more than one exit path
/// (the lights tick and a ring append today), not just the tick: a second
/// hand-written guard is how one of them ends up leaking its lock on a path
/// the other already covered.
///
/// A GUARD RATHER THAN A RELEASE AT EVERY EXIT: the lights tick stands down
/// from four places and a ring append from several early returns, and a lock
/// left behind stands every later claimant down for a whole stale window.
/// `Drop` is the one exit all of them share.
///
/// THE MESSAGE NAMES NEITHER CALLER, deliberately: it is printed by the type
/// both share, and naming one subsystem in it would misdescribe the other's
/// failure the day this is reused a third time.
pub(crate) struct HeldLock(pub(crate) std::path::PathBuf);

impl Drop for HeldLock {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            eprintln!(
                "pns: the lock {} could not be given up ({error}); \
                 the next claimant waits it out",
                self.0.display()
            );
        }
    }
}
