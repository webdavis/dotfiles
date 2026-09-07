use crate::*;
use pns_adapters::nag_records::{claim_fire, claim_record, record_entries, release_fire};

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
    let directory = pns_adapters::nag_records::nag_dir(&state);
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

    let mut held: Vec<(std::path::PathBuf, pns_domain::nag::Record, String)> = Vec::new();
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
            .and_then(|name| pns_domain::nag::session_of(&name.to_string_lossy()))
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
            .and_then(pns_adapters::nag_records::parse);
        let answered = pns_domain::nag::marker_name(&session)
            .is_some_and(|marker| marker_path(&state, &marker).exists());
        match (
            pns_domain::nag::fate(parsed.as_ref(), answered, now, after_secs),
            parsed,
        ) {
            (pns_domain::nag::Fate::Count, Some(record)) => held.push((claim, record, session)),
            // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS ONLY BEEN ATTEMPTED:
            // a file at a record's path that this could not read is somebody
            // else's write, and dropping it in silence is how one would sit
            // there being re-claimed on every fire forever.
            (pns_domain::nag::Fate::Drop(pns_domain::nag::Dropped::Unreadable), _) => {
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
        let Some(marker) = pns_domain::nag::marker_name(session) else {
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
            detail: pns_domain::nag::nudge(
                held.len(),
                now.saturating_sub(oldest.armed),
                &oldest.detail,
            ),
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
