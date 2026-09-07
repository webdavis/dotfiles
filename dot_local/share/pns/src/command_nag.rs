use crate::*;

// --- the nag ----------------------------------------------------------------

/// `pns nag`: one card about every approval nobody has answered, or silence.
///
/// RUN BY THE DAEMON AND TYPEABLE BY THE OPERATOR, which is what makes the
/// drill forceable without waiting out a timer. It PRINTS what it did, one
/// line, in `recap`'s shape.
///
/// OWNERSHIP IS TAKEN AT TWO LEVELS, and they answer two different questions.
/// The WINDOW is claimed once, before anything is enumerated (`claim_fire`), so
/// two processes woken by two jobs in one tick produce one card between them
/// rather than one card each. Each RECORD is then claimed by rename before it
/// is read for anything, which is what stops a single approval being counted
/// twice by a fire that broke in after a stale window claim aged out. Both are
/// renames because a plain unlink does not arbitrate on this filesystem; the
/// measurement is in
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`.
///
/// THE ORDER IS THE SAFE ONE AT EVERY STEP. The markers are written BEFORE the
/// card and the claims removed AFTER it: a crash before the card leaves
/// approvals marked and silent, a crash after it leaves claims nothing
/// re-enumerates, and neither ordering can produce a SECOND card, which is the
/// property that matters.
pub(crate) fn nag_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, per the house rule that an unknown argument
    // never falls through to help with exit 0. `pns nag <session>` is a command
    // an operator would believe narrowed the fire, and coalescing means nothing
    // here can honour it.
    if std::env::args_os().nth(2).is_some() {
        eprintln!("{NAG_USAGE}");
        return 2;
    }
    let state = state_dir();
    let directory = pns::nag::nag_dir(&state);
    // A CONFIG THAT TURNED THE FEATURE OFF BETWEEN ARMING AND FIRING MEANS NO
    // NUDGE, and the records go with it: the operator cancelled the timer, and
    // a card from it would be the feature ignoring them.
    let after_secs = nag_after_secs();
    if after_secs == NAG_OFF {
        let dropped = record_entries(&directory)
            .iter()
            .filter(|record| std::fs::remove_file(record).is_ok())
            .count();
        println!("pns nag: the nag is off; {dropped} waiting approval(s) dropped");
        return 0;
    }
    // NO CLOCK IS NO NUDGE. Every input this cannot read resolves to silence,
    // and a wait nothing can measure is one of them.
    let Some(now) = now_secs() else {
        eprintln!("pns nag: this machine has no clock to measure a wait against");
        return 0;
    };
    // THE DIRECTORY BEFORE THE LOCK THAT LIVES IN IT. The arm makes this
    // directory, but an operator running the fire by hand before anything has
    // ever armed (drill step 10) has no directory to take a lock in, and a
    // fire that could not say "nothing is waiting" would read as broken.
    let _ = std::fs::create_dir_all(&directory);
    // AND THE WHOLE FIRE CLAIMED ONCE, BEFORE ANYTHING IS ENUMERATED. See
    // `claim_fire`: the per-record claim is per-approval crash safety and does
    // not arbitrate a WINDOW, so without this two woken processes split the
    // outstanding records between them and card twice.
    let Some(fire) = claim_fire(&directory, now) else {
        // A LOSER SAYS NOTHING AT ALL, on either stream, and exits 0. The
        // window belongs to another process whose one card names every approval
        // this one would have, so a line here would be noise about work that is
        // being done.
        return 0;
    };

    let mut held: Vec<(std::path::PathBuf, pns::nag::Record, String)> = Vec::new();
    for record in record_entries(&directory) {
        // SOMEBODY ELSE OWNS IT, or it is not a regular file: either way this
        // process never opened it and never counts it.
        let Some(claim) = claim_record(&record) else {
            continue;
        };
        // A NAME THAT IS NOT A SESSION IS DROPPED, LOUDLY, AND ONLY ONCE. This
        // is the unreadable-CONTENT case one branch down wearing a different
        // coat, and it gets the same answer for the same stated reason: a file
        // skipped in silence sits at a record's name being re-read on every
        // fire forever. Nothing can be resolved from it (no marker, no job and
        // no card has a name to be written under), so there is nothing to
        // degrade to.
        let Some(session) = record
            .file_name()
            .and_then(|name| pns::nag::session_of(&name.to_string_lossy()))
        else {
            eprintln!(
                "pns nag: {} is not named for a session this can act on; it is dropped",
                record.display()
            );
            let _ = std::fs::remove_file(&claim);
            continue;
        };
        let parsed = std::fs::read_to_string(&claim)
            .ok()
            .as_deref()
            .and_then(pns::nag::parse);
        let answered = pns::nag::marker_name(&session)
            .is_some_and(|marker| marker_path(&state, &marker).exists());
        match (
            pns::nag::fate(parsed.as_ref(), answered, now, after_secs),
            parsed,
        ) {
            (pns::nag::Fate::Count, Some(record)) => held.push((claim, record, session)),
            // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS ONLY BEEN ATTEMPTED:
            // a file at a record's path that this could not read is somebody
            // else's write, and dropping it in silence is how one would sit
            // there being re-claimed on every fire forever.
            (pns::nag::Fate::Drop(pns::nag::Dropped::Unreadable), _) => {
                eprintln!(
                    "pns nag: {} is not a record this can read; it is dropped",
                    record.display()
                );
                let _ = std::fs::remove_file(&claim);
            }
            (_, _) => {
                let _ = std::fs::remove_file(&claim);
            }
        }
    }

    // OLDEST FIRST, so the card is built from the approval that has waited
    // longest: it is the one whose wait the multi-case names, and the one whose
    // pane is likeliest to still be the one worth focusing.
    held.sort_by_key(|(_, record, _)| record.armed);
    let Some((_, oldest, _)) = held.first() else {
        release_fire(&fire);
        println!("pns nag: nothing is waiting");
        return 0;
    };
    // THE MARKERS FIRST, FOR EVERY COUNTED RECORD. Those approvals have now
    // spent their one nudge, and the marker is what makes each of their OWN
    // daemon jobs drop silently when its turn comes; without it the siblings
    // would each wake a process that found nothing and said so.
    for (_, _, session) in &held {
        let Some(marker) = pns::nag::marker_name(session) else {
            continue;
        };
        if let Err(error) = write_marker(&state, &marker) {
            eprintln!("pns nag: an answered marker could not be written ({error})");
        }
    }
    // ONE CARD, WHATEVER THE COUNT, which is the operator's coalescing ruling
    // and the structural rate limit it buys: at most one nudge card per
    // `after_secs`, however many approvals are waiting.
    //
    // `PNS_SKIP_PHONE` IS NOT IN PLAY HERE. It is set by `blocking_event` in
    // that process only, and this is a different process minutes later that
    // never inherits it, so the nudge reaches the phone the first card was
    // suppressed from. That is deliberate and must not be "tidied" into the
    // record by a later refactor.
    run_event(
        &pns::args::EventArgs {
            agent: oldest.agent.clone(),
            // THE STATE WORD STAYS `blocked`. A new word would fall out of
            // `missed_notifications::NEEDS_YOU`, and an unanswered approval is
            // exactly what that section is for.
            state: BLOCKED_STATE.to_string(),
            project: oldest.project.clone(),
            branch: oldest.branch.clone(),
            detail: pns::nag::nudge(held.len(), now.saturating_sub(oldest.armed), &oldest.detail),
            pane: oldest.pane.clone(),
            ..Default::default()
        },
        &system_probes(),
        // NO PAYLOAD, and coalescing is why: one card stands for every record
        // in `held`, so naming one of their sessions would be inventing an
        // identity the card does not have. A nudge returns before the lamps'
        // needs marker is touched at all, so this is the honest default rather
        // than a value chosen to be ignored.
        &HookPayload::default(),
        Attempt::Nudge,
    );
    for (claim, _, _) in &held {
        if let Err(error) = std::fs::remove_file(claim) {
            eprintln!(
                "pns nag: the working file {} could not be removed ({error}); it is left behind",
                claim.display()
            );
        }
    }
    release_fire(&fire);
    // ATTEMPTED, NEVER SENT. `run_event` answers nothing about delivery and
    // this mode cannot know whether a single leg fired: a mute, a named Focus
    // or a plan that selected nothing all mean the nudge did not happen. The
    // drill reads this line, and an action reported as done when it was
    // suppressed is bug class 19 spoken out loud.
    println!("pns nag: {} waiting; one card attempted", held.len());
    0
}
const NAG_USAGE: &str = "pns: usage: pns nag (it takes no arguments: one fire cards every \
outstanding approval at once)";
/// Every file in the nag directory that could be a record, sorted so a fire is
/// deterministic.
///
/// THE SUFFIX IS THE WHOLE FILTER, which is what keeps a claim out of this: a
/// held claim is `<name>.claim.<pid>` and can never end in the record suffix,
/// so a record another process is mid-fire on is never re-enumerated here.
fn record_entries(directory: &Path) -> Vec<std::path::PathBuf> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|entry| {
            entry
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(pns::nag::RECORD_SUFFIX))
        })
        .collect();
    entries.sort();
    entries
}

/// One record taken by rename, or None when somebody else has it.
///
/// THE RENAME IS THE OWNERSHIP TEST, in `consume_turn_marker`'s exact shape and
/// for `take_claim`'s measured reason: a plain unlink reports success to EVERY
/// racer on APFS, so a remove could tell two processes they each own this
/// record.
///
/// NOT THE SAME GUARANTEE AS THE FIRE CLAIM, and not made redundant by it. The
/// fire claim is what stops two processes carding in one window; this is what
/// stops ONE approval being counted twice when a second process is legitimately
/// running, which is what happens after a crashed fire's window claim ages out
/// while its records are still on disk. NO TEST IN THIS SUITE KILLS THIS
/// RENAME: reading each record in place and removing it afterwards passes
/// everything, because every fire in the suite bar one is single-process, and
/// that one is arbitrated a level up. It is kept on the measurement, not on a
/// test.
///
/// AN IRREGULAR FILE GOES BACK WHERE IT WAS AND IS NEVER OPENED, following
/// `append_ring_line`'s own refusal at a state path: a FIFO here would park the
/// read forever. The rename is still what tests it, because only the winner is
/// entitled to look at all.
fn claim_record(record: &Path) -> Option<std::path::PathBuf> {
    let claim = pns::nag::claim_path(record, std::process::id());
    // NEVER RENAMED OVER A CLAIM ALREADY THERE, for `claim_by_rename`'s reason:
    // the name carries this process's id, so anything sitting at it is a record
    // this pid claimed and could not finish, and a rename would land the new one
    // on top of it.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return None;
    }
    std::fs::rename(record, &claim).ok()?;
    if !matches!(std::fs::symlink_metadata(&claim), Ok(found) if found.is_file()) {
        let _ = std::fs::rename(&claim, record);
        return None;
    }
    Some(claim)
}

/// The whole fire owned ONCE, or None when this process is not the one holding
/// this window.
///
/// NOT A DUPLICATE OF THE PER-RECORD CLAIM, which answers a different question.
/// That one is per-approval crash safety: it is what stops one record being
/// counted by two processes, and it stays. But ownership taken per record lets
/// two woken processes each win a DISJOINT, NON-EMPTY subset and each card its
/// own true count, which is one card per FIRE rather than one card per fire
/// WINDOW, and that is precisely what the coalescing ruling forbids. Measured
/// on the build before this: sixteen concurrent fires over one directory
/// produced sixteen cards. The window is what has to be owned, so it is.
///
/// AN EXCLUSIVE CREATE IS THE ARBITRATION, NOT A RENAME, and the difference is
/// measured rather than stylistic. A rename claim moves the contended name OUT
/// of the way: the winner renames `fire.lock` to its own claim, so a racer that
/// looked for a holder a moment earlier finds no lock at that name, creates one
/// and takes it too. That form delivered TWO cards from four concurrent fires,
/// reproducibly, under load. An exclusive create leaves the lock sitting at its
/// name for the whole fire, so every later racer is refused by the same atomic
/// operation, whenever it arrives. The rename survives below, in the one place
/// a remove would be unsafe.
///
/// AND AGED OUT AT A MINUTE, so a crash mid-fire cannot wedge the feature for
/// good. A minute is a wide margin over the work the lock has to cover: the
/// holder claims every record by rename before it delivers anything, so a fire
/// that broke in later finds an empty directory in any case. What the wait
/// costs when the holder really did die is one nudge window, which is the safe
/// direction.
fn claim_fire(directory: &Path, now: u64) -> Option<std::path::PathBuf> {
    let lock = directory.join(pns::nag::FIRE_LOCK);
    claim_lock(&lock, now, pns::nag::FIRE_STALE_SECS).then_some(lock)
}
/// The fire given up, so the next window can be claimed without waiting out
/// `FIRE_STALE_SECS`.
///
/// SAID WHEN IT FAILS, and the consequence is named rather than implied: the
/// feature is not broken by a claim left behind, it is DELAYED, because the age
/// test is what recovers it.
fn release_fire(fire: &Path) {
    if let Err(error) = std::fs::remove_file(fire) {
        eprintln!(
            "pns nag: the fire claim {} could not be given up ({error}); the next fire waits it out",
            fire.display()
        );
    }
}
