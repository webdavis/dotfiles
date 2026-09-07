use super::*;

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
