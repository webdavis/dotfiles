use super::*;

/// Every live lease's epoch, with the ones past the timeout REMOVED on the way
/// through.
///
/// THE SWEEP LIVES WITH THE READ, for `sweep_blocked`'s reason: the tick is the
/// only process that ever looks in this directory, and a pane that ends without
/// `pns loop end` leaves a file nothing else would remove.
pub fn sweep_leases(state: &Path, now: u64, timeout_secs: u64) -> Vec<u64> {
    sweep_markers(&crate::marker_files::lease_dir(state), now, timeout_secs)
}

/// Every live epoch one marker directory holds, with everything past the bound
/// REMOVED on the way through.
///
/// ONE SWEEP FOR THE WAITS AND THE LEASES, because they are one mechanism twice:
/// a directory of one-epoch files, a bound, and a tick that is the only process
/// that ever looks. Written twice, the second copy is where the race fix, the
/// working-file rule and the collection of what a dead run left behind would
/// each have to be remembered a second time.
///
/// A REMOVAL IS OWNED BY RENAME AND NEVER READ-THEN-UNLINK. Concurrent unlink
/// does not arbitrate on this filesystem (see
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`), so a sweep that
/// read an expired epoch and then unlinked could delete a FRESH marker a racing
/// event had published in between. Taking the file by rename first means what
/// this removes is what this took, and the epoch is READ AGAIN off the claim: a
/// marker that turned out to be live in the meantime is put back rather than
/// destroyed.
///
/// THE LIVE PATH TOUCHES NOTHING, which is what keeps that safety free. A
/// marker still inside its bound is read and left exactly where it is, so the
/// ordinary tick renames nothing at all.
///
/// A PUT-BACK CAN OVERWRITE A NEWER PUBLISH, and that is the residue rather than
/// a rule: the epoch restored is live and at most one racing publish old, which
/// is seconds against bounds measured in hours.
///
/// A MARKER ALREADY NAMED FOR THE WORKING GRAMMAR IS A RESIDUAL, not a case
/// this handles: `pane_file_is_safe` and `session_id_is_safe` refuse a NEW id
/// `working_owner` would read as a working file, but a marker written under one
/// before that guard existed is read here as that pid's own working file
/// (`owner_is_gone` judges it, never `marker_is_live`), so it neither lights a
/// lamp nor ages out. No id this crate's own callers produce can spell the
/// shape (a UUID session id and a `wW:p21` pane cannot).
///
/// THE SHAPE IS `working_owner`'S, NOT `.new.<digits>` ALONE, which is what the
/// operator check has to match: the RIGHTMOST of `.new.` and `.sweep.` decides,
/// so `s.sweep.7` and a mixed `a.new.b.sweep.1` are residuals exactly as
/// `s.new.4321` is, and `a.new.b` (no pid after the last marker) is an ordinary
/// marker that sweeps normally. The check is therefore
/// `ls ~/.local/state/pns/lights-blocked ~/.local/state/pns/lights-loop` for any
/// name whose last `.new.` or `.sweep.` is followed by digits alone, removed by
/// hand.
///
/// AND THE SWEEP IS NOT WEAKENED TO REACH IT, which is a statement about this
/// function rather than a claim that the residual gets collected: while the pid
/// in the name belongs to a LIVE process it is never swept at all, and pid 1 is
/// launchd, so that name in particular is permanent until the operator removes
/// it. A code fix was weighed and refused. Sweeping a working file whose owner
/// is alive is the one thing this must never do, because it unlinks a publish
/// caught between its open and its rename and loses a wait with the agent still
/// waiting; and moving working files to a directory of their own is a state
/// layout migration that leaves the same legacy names behind at the other end.
/// The residual costs one stale file per legacy name and never grows, which is
/// less than either fix.
pub(super) fn sweep_markers(directory: &Path, now: u64, max_age_secs: u64) -> Vec<u64> {
    let mut live = Vec::new();
    for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // A WORKING FILE IS NOT A MARKER, and one whose run is GONE is litter
        // nothing else collects. A publish caught between its open and its
        // rename has no epoch in it yet, and unlinking it there wins the race
        // against the rename, which then publishes nothing: the wait is lost
        // with the agent still waiting on the operator.
        if let Some(owner) = pns_domain::lights::working_owner(&name) {
            if owner_is_gone(owner) {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        if let Some(at) = read_epoch(&path)
            && pns_domain::lights::held::marker_is_live(at, now, max_age_secs)
        {
            live.push(at);
            continue;
        }
        // EXPIRED, OR AN EPOCH NOBODY CAN READ, which is swept for the same
        // reason: nothing can ever age out a file whose epoch is unreadable, so
        // leaving it is the same unbounded growth through a different door.
        let claim = crate::marker_files::sweep_claim(directory, &name, std::process::id());
        if std::fs::rename(&path, &claim).is_err() {
            continue;
        }
        match read_epoch(&claim) {
            // IT CAME BACK LIVE, so a fresh publish landed between the read and
            // the claim and this run is holding it. Put it back.
            Some(at) if pns_domain::lights::held::marker_is_live(at, now, max_age_secs) => {
                live.push(at);
                if std::fs::rename(&claim, &path).is_err() {
                    let _ = std::fs::remove_file(&claim);
                }
            }
            _ => {
                let _ = std::fs::remove_file(&claim);
            }
        }
    }
    live
}

/// Every live wait's epoch, with the ones past the bound REMOVED on the way
/// through.
///
/// THE SWEEP LIVES WITH THE READ because the tick is the only process that
/// ever looks in this directory: a session that ends without another event
/// leaves a marker nothing else would ever remove, and one file per abandoned
/// session for the life of a machine is unbounded growth.
pub(super) fn sweep_blocked(state: &Path, now: u64, give_up_after_secs: u64) -> Vec<u64> {
    sweep_markers(
        &crate::marker_files::blocked_dir(state),
        now,
        give_up_after_secs,
    )
}

/// The blocked lamp's reading for this tick: the sweep that removes an aged
/// marker and the aggregate that lights the lamp, both handed the one
/// configured backstop.
///
/// ITS OWN FUNCTION SO ITS TEST SPAWNS NOTHING: the rest of the house asks
/// herdr and the idle probes, and this half never depends on either.
pub fn blocked_lamp(state: &Path, lights: &pns_domain::lamps::config::Lights, now: u64) -> bool {
    let give_up_after_secs = lights.blocked.give_up_after_secs;
    pns_domain::lights::held::any_blocked(
        &sweep_blocked(state, now, give_up_after_secs),
        now,
        give_up_after_secs,
    )
}
