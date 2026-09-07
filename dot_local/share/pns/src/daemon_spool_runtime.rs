use crate::*;

/// One pass over the spool, under a protocol with THREE INVARIANTS.
///
/// 1. **A CLIENT ALWAYS WINS.** Every write this daemon makes into the spool
///    (a re-arm, a put-back) is create-if-absent, so a registration or a
///    refresh that landed while a record was claimed keeps its name and the
///    daemon's older copy is discarded. An overwriting rename here would put a
///    stale due, lease and argv back over the newest signal, which is the one
///    guarantee the id-is-the-filename refresh rule makes.
/// 2. **THE DAEMON ACTS ONLY ON WHAT IT OWNS.** A read-only peek decides one
///    thing and one only: whether there is nothing to do. Everything else
///    claims the entry by rename FIRST and re-reads the claim, so the record
///    that fires is the record this daemon took, never one a refresh replaced
///    between the look and the act. A `Wait` is never claimed, because a wait
///    performs no action and renaming a waiting job out and back would be the
///    very write invariant 1 forbids.
/// 3. **ONE OCCURRENCE RUNS ONCE.** The rename is still the arbiter and it is
///    now taken before the content is read, so of two daemons exactly one
///    holds the record and the loser reads nothing at all.
///
/// THE RESIDUAL WINDOWS, STATED HONESTLY. A refresh that lands AFTER the claim
/// is taken cannot stop the occurrence already claimed from running, so the
/// operator can see one card from the record that was in flight plus the
/// refreshed job afterwards. Nothing is LOST and nothing runs twice; the old
/// occurrence simply ran. A refresh that lands after the claim also wins the
/// re-arm's link, so the repeat continues on the client's terms rather than the
/// daemon's. And a claim this process took and could not remove holds its own
/// working name; the line naming it is printed either way, because a job that
/// vanished with nothing in the log is the failure that costs the most to find.
pub(crate) fn drain_spool(
    spool: &Path,
    state: &Path,
    now: u64,
    tick: Duration,
    children: &mut Vec<Bounded>,
    reported: &mut std::collections::BTreeSet<std::path::PathBuf>,
) {
    for entry in pns::daemon::spool_entries(spool) {
        let Some(id) = entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        match pns::daemon::peek(&entry, &id) {
            // SAID ONCE, never once a tick: the file is left where it is, so
            // the alternative is one line a second about a thing nobody is
            // going to fix while the daemon is watching.
            pns::daemon::Peeked::Irregular => {
                if reported.insert(entry.clone()) {
                    eprintln!(
                        "pns daemon: {} is not a regular file; left alone and never opened",
                        entry.display()
                    );
                }
            }
            // NOTHING TO DO, DECIDED WITHOUT TOUCHING IT. This is the only
            // verdict a peek is allowed to be the last word on.
            pns::daemon::Peeked::Job(job)
                if pns::daemon::decide(
                    &job,
                    now,
                    pns::daemon::marker_exists(state, &job),
                    children.iter().any(|bounded| bounded.id == job.id),
                ) == pns::daemon::Verdict::Wait => {}
            // Anything else is an ACTION, so the record is taken first and read
            // again afterwards. A failed claim means another run got there,
            // which is exactly what the rename is for.
            _ => {
                if let Some(claim) = pns::daemon::claim(&entry) {
                    act(&claim, &id, spool, state, now, tick, children);
                }
            }
        }
    }
}

/// One CLAIMED record, re-read and acted on.
///
/// THE RE-READ IS THE POINT. Between the peek that decided to act and the
/// rename that took the record, a client can have replaced it with a refresh
/// carrying a new due, a new lease and new arguments. Acting on the peek would
/// fire the old argv and then delete the new record on the way out; acting on
/// the claim fires whatever this daemon actually holds.
fn act(
    claim: &Path,
    id: &str,
    spool: &Path,
    state: &Path,
    now: u64,
    tick: Duration,
    children: &mut Vec<Bounded>,
) {
    match pns::daemon::peek(claim, id) {
        // A RENAME MOVES A REGULAR FILE AS A REGULAR FILE, so this is not
        // reachable by the paths above; it is still answered rather than
        // ignored, because the alternative is a claim held forever.
        pns::daemon::Peeked::Irregular => {
            println!("pns daemon: dropped `{id}`: it is not a regular file");
            release(claim);
        }
        pns::daemon::Peeked::Unusable(refusal) => {
            println!("pns daemon: dropped `{id}`: {refusal}");
            release(claim);
        }
        pns::daemon::Peeked::Job(job) => {
            // ASKED AGAIN, AND REDUNDANT WHILE THE PEEK ASKS IT TOO: the peek
            // stands a running job down before anything is claimed, so this is
            // only ever reached with no child of this id alive, and no test can
            // tell this argument from a literal `false`. It stays because the
            // peek is an optimisation over a re-read and this is the decision
            // the claim is actually acted on.
            let running = children.iter().any(|bounded| bounded.id == job.id);
            match pns::daemon::decide(&job, now, pns::daemon::marker_exists(state, &job), running) {
                // The refresh this daemon claimed is not due yet, so it goes
                // back CREATE-IF-ABSENT: a client that registered again in the
                // meantime keeps its own record and this copy is dropped.
                pns::daemon::Verdict::Wait => match pns::daemon::hand_back(spool, &job) {
                    Ok(_) => release(claim),
                    Err(error) => {
                        eprintln!("pns daemon: `{id}` could not be put back ({error})");
                        release(claim);
                    }
                },
                pns::daemon::Verdict::Drop(reason) => {
                    println!("pns daemon: dropped `{id}` because {}", reason.said());
                    release(claim);
                }
                pns::daemon::Verdict::Fire => fire(&job, spool, now, tick, claim, children),
            }
        }
    }
}

/// A working file this daemon is done with, removed and NAMED IF IT SURVIVES.
///
/// A CLAIM THAT COULD NOT BE REMOVED IS A LEAK, not a nothing: it is invisible
/// to the scan (the working prefix is outside the id charset), so it sits there
/// until a hand removes it, and `claim` refuses to reuse a name already taken,
/// which can wedge that one id after a pid is reused. One line naming the file
/// is the whole remedy, and it costs nothing on the path where the remove
/// works.
fn release(claim: &Path) {
    if let Err(error) = std::fs::remove_file(claim) {
        eprintln!(
            "pns daemon: the working file {} could not be removed ({error}); it is left behind",
            claim.display()
        );
    }
}

/// One claimed job re-armed and started, in that order.
///
/// THE RE-ARM IS DURABLE BEFORE THE SPAWN. Written the other way round, a
/// daemon killed between the two loses the repeat with the job already run,
/// which is the lamp going dark on a loop that is still alive.
///
/// AND THE RE-ARM IS CREATE-IF-ABSENT. A client that refreshed this id while
/// the occurrence was claimed published the newer signal, and a rename here
/// would overwrite it with the due and lease this daemon computed from the
/// record it had already taken.
fn fire(
    job: &pns::daemon::Job,
    spool: &Path,
    now: u64,
    tick: Duration,
    claim: &Path,
    children: &mut Vec<Bounded>,
) {
    if let Some(next) = pns::daemon::rearm(job, now) {
        match pns::daemon::hand_back(spool, &next) {
            Ok(true) => {}
            Ok(false) => println!(
                "pns daemon: `{}` was registered again while it ran, so its repeat stands down",
                job.id
            ),
            Err(error) => eprintln!("pns daemon: `{}` will not repeat ({error})", job.id),
        }
    }
    release(claim);
    // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS NOT BEEN PERFORMED: a spawn
    // that failed is said out loud, because the alternative is a job that
    // reports as run and delivered nothing.
    //
    // AND A SPAWN THAT WORKED SAYS NOTHING, which is the daemon's own
    // no-chatter rule applied to the thing it actually does. The lights tick
    // repeats every twelve seconds for as long as its lease holds, so a line
    // per firing is 300 an hour in the file the log rotation then rotates a
    // real log out of. What a job has to say, the job says itself: its stderr
    // is the daemon's now.
    match spawn_job(job) {
        Ok(child) => {
            children.push(Bounded {
                id: job.id.clone(),
                child,
                expires_at: std::time::Instant::now() + child_bound(tick, &job.id),
            });
        }
        Err(error) => eprintln!("pns daemon: `{}` could not start ({error})", job.id),
    }
}
