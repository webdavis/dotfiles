use super::*;
/// Whether another run is inside the return moment right now, and the claim it
/// left behind when it is not.
///
/// MATCHED ON THE MARKER'S OWN CLAIM PREFIX and nothing looser, which is
/// `stranded_claims`' rule: the journal and the turn marker claim themselves
/// in this directory too, and a wider match would hand one of their values
/// back as a window's near edge.
///
/// AT MOST ONE OF THESE CAN EXIST AT A TIME, because a claim is only ever made
/// by renaming the ONE marker or by renaming an existing claim, and a run that
/// finds one live makes none of its own. The loop still answers `Live` for the
/// first live one it meets rather than assuming that, because the directory is
/// a plain directory another hand can reach.
pub(super) enum StrandedWindow {
    /// A run that still exists holds the marker.
    Live,
    /// A claim nobody is inside any more, and so the near edge it is holding.
    Abandoned(std::path::PathBuf),
    /// Nothing is holding anything: no marker was ever published here.
    None,
}
pub(super) fn stranded_window_claim(state: &Path, now: Option<u64>) -> StrandedWindow {
    let prefix = format!("{LAST_PRESENT}.claim.");
    let Ok(entries) = std::fs::read_dir(state) else {
        return StrandedWindow::None;
    };
    let mut abandoned = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(owner) = name.strip_prefix(&prefix) else {
            continue;
        };
        if !window_claim_is_free(owner, now) {
            return StrandedWindow::Live;
        }
        abandoned = Some(entry.path());
    }
    abandoned.map_or(StrandedWindow::None, StrandedWindow::Abandoned)
}
/// What a window claim is named after the prefix: the id of the run that took
/// it, and the epoch it was taken at when that run had a clock to read.
///
/// THE EPOCH IS THE CLAIM'S OWN AGE and cannot be taken off the file instead:
/// a rename carries the marker's mtime, which is the time of the last PRESENT
/// event and can be hours before the claim was made. It costs nothing to
/// record, because the caller already holds this event's clock read.
pub(super) fn window_claim_suffix(now: Option<u64>) -> String {
    match now {
        Some(now) => format!("claim.{}.{now}", std::process::id()),
        None => format!("claim.{}", std::process::id()),
    }
}
/// Whether a window claim may be taken: nobody is inside it.
///
/// THREE WAYS IT IS FREE, and the first two are `claim_by_rename`'s own. It is
/// THIS RUN'S, so nothing else can be inside it; or its owner has EXITED, so
/// nothing is; or it is far OLDER than any run could still be holding it.
///
/// THE AGE TEST IS WHAT A PID CANNOT ANSWER. A claim is held for two renames
/// and a small read, so a claim minutes old is one whose owner died mid-claim
/// and whose id the machine has since handed to something long-lived. Without
/// it that claim reads as live for as long as the new process runs, and every
/// return moment on the machine stands down behind it: no card, no recap and
/// no edge, until that process happens to exit. The bound is deliberately five
/// minutes, four orders of magnitude past what holding one costs, so a real
/// holder can never be stolen from and a stranded one can never wedge for long.
pub(super) fn window_claim_is_free(owner: &str, now: Option<u64>) -> bool {
    let mut named = owner.split('.');
    let took_it = named.next().unwrap_or_default();
    if took_it == std::process::id().to_string() || owner_is_gone(owner) {
        return true;
    }
    match (named.next().and_then(|at| at.parse::<u64>().ok()), now) {
        (Some(taken), Some(now)) => now.saturating_sub(taken) > STALE_WINDOW_CLAIM_SECS,
        // A CLAIM WITH NO EPOCH, or a run with no clock to compare it against,
        // falls back on the pid alone, which is `abandoned_hold`'s own answer
        // and its own accepted price.
        _ => false,
    }
}
/// How long a window claim may stand before it is taken to be stranded
/// whatever its process id says. See `window_claim_is_free`.
pub(super) const STALE_WINDOW_CLAIM_SECS: u64 = 300;
