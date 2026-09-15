//! The window the digest's claim opens under an appender that is mid-write.
//!
//! Every fixture here keys its directory on a clock reading rather than on the
//! process id, because a reused process id finds the previous run's files.

use super::*;
use crate::DigestSpoolFile;
use posture_application::DigestSpool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A spool directory of this run's own, removed when the test ends.
struct Race {
    root: PathBuf,
    store: PathBuf,
}

impl Race {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "posture-append-race-{unique}-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let store = root.join("state").join("digest-spool");
        Self { root, store }
    }

    /// Every identity written under this directory, spool and claims alike, so
    /// a line that landed in a claim still counts as written somewhere.
    fn identities(&self) -> Vec<String> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(self.store.parent().unwrap())
            .unwrap()
            .flatten()
        {
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for record in posture_protocol::decode_spool(&text) {
                found.push(record.identity.unwrap_or_default());
            }
        }
        found
    }
}

impl Drop for Race {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn finding(identity: &str) -> posture_protocol::DigestRecord {
    posture_protocol::DigestRecord {
        detector: Some("suid_bin_unexpected".into()),
        identity: Some(identity.into()),
        summary: Some("a binary appeared".into()),
        ..posture_protocol::DigestRecord::default()
    }
}

/// Rounds of the race below. ONE CLAIM EACH, because a second rename inside
/// one append would beat the single re-append by design; what needs many
/// tries is the narrow window itself, and a fresh round is how it gets them.
/// Measured against a retry-disabled build: 10 rounds caught the loss on 70
/// of 70 runs here, so it stays the floor rather than a smaller count that
/// only usually catches it.
const ROUNDS: usize = 10;
const WRITERS: usize = 8;
const APPENDS: usize = 20;
/// Lines the claim waits for, so it lands in the middle of the writing rather
/// than before it starts.
const APPENDS_BEFORE_THE_CLAIM: usize = 10;

#[test]
fn a_line_appended_while_the_digest_claims_the_spool_is_not_lost() {
    // THE CLAIM RENAMES THE FILE OUT FROM UNDER AN OPEN HANDLE. Without the
    // re-append, a line written through that handle lands in a batch the
    // digest has already read, and nobody ever hears the finding.
    for _ in 0..ROUNDS {
        one_race();
    }
}

fn one_race() {
    let race = Race::new();
    let written = std::sync::Arc::new(AtomicUsize::new(0));
    let writers: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let store = race.store.clone();
            let written = std::sync::Arc::clone(&written);
            std::thread::spawn(move || {
                let appender = DigestAppendFile::new(store);
                for n in 0..APPENDS {
                    let record = finding(&format!("live-{writer}-{n}"));
                    assert!(appender.append(&record, &mut std::io::sink()));
                    written.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    // Spin rather than yield: the whole round is milliseconds long, and a
    // claim that gives up its core arrives after the writing is over. Bounded
    // by a deadline, not just the counter: a writer that panics before
    // reaching APPENDS_BEFORE_THE_CLAIM (a spool the fixture could not
    // create, EMFILE, a full temp filesystem) must not spin this thread
    // forever waiting for a count that will never arrive. The join below
    // surfaces that panic once the wait gives up.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while written.load(Ordering::Relaxed) < APPENDS_BEFORE_THE_CLAIM
        && std::time::Instant::now() < deadline
    {
        std::hint::spin_loop();
    }
    let spool = DigestSpoolFile::new(race.store.clone(), 1, std::process::id());
    if let Ok(Some(batch)) = spool.claim() {
        spool.restore(&batch);
    }
    for writer in writers {
        writer.join().unwrap();
    }

    let mut copies: HashMap<String, usize> = HashMap::new();
    for identity in race.identities() {
        *copies.entry(identity).or_default() += 1;
    }
    let missing: Vec<String> = (0..WRITERS)
        .flat_map(|writer| (0..APPENDS).map(move |n| format!("live-{writer}-{n}")))
        .filter(|identity| !copies.contains_key(identity))
        .collect();
    assert!(
        missing.is_empty(),
        "{} lines were lost to one claim: {missing:?}",
        missing.len()
    );
    // ONE CLAIM COSTS AT MOST ONE REPEAT, which is the ceiling's whole
    // justification: the loop only runs again while the spool keeps moving.
    let chased: Vec<_> = copies.iter().filter(|(_, count)| **count > 2).collect();
    assert!(
        chased.is_empty(),
        "one claim wrote a line more than twice: {chased:?}"
    );
}

/// A digest thread that claims the spool by rename and reads the claim, so a
/// line landing in one after its read is lost exactly as in production.
/// Three of them, spinning, is the pathological condition the ceiling exists
/// for: a real digest claims once a day.
const DIGESTS: usize = 4;
const REPRODUCER_APPENDS: usize = 1000;

#[test]
fn a_digest_that_keeps_claiming_loses_no_line_without_saying_so() {
    // ONE RE-APPEND WAS NOT ENOUGH. The retry can land in a second claim that
    // has also already been read, and that line is gone: this reproducer at
    // 20000 appends lost 9 to 14 lines silently against the single retry and
    // none against the loop. So the appender re-appends until the file it
    // wrote to is still the file at the spool path.
    //
    // WHAT IS ASSERTED IS SILENCE, not survival. Claims here arrive thousands
    // of times a second rather than once a day, which is the one condition
    // that can reach the ceiling, and a line the appender gave up on was
    // reported to its caller. `one_retry_is_not_enough_for_a_spool_that_moved_twice`
    // is the deterministic version of the same defect.
    let race = Race::new();
    std::fs::create_dir_all(race.store.parent().unwrap()).unwrap();
    let collected = Arc::new(std::sync::Mutex::new(HashSet::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let digests: Vec<_> = (0..DIGESTS)
        .map(|digest| {
            let store = race.store.clone();
            let collected = Arc::clone(&collected);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut round = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    let claim = store.with_extension(format!("claim-{digest}-{round}"));
                    if std::fs::rename(&store, &claim).is_ok() {
                        round += 1;
                        if let Ok(text) = std::fs::read_to_string(&claim) {
                            let mut seen = collected.lock().unwrap();
                            for row in posture_protocol::decode_spool(&text) {
                                seen.insert(row.identity.unwrap_or_default());
                            }
                        }
                        // THE CLAIM IS KEPT, not deleted. A deleted claim's
                        // inode number can be recycled into the fresh spool,
                        // and the appender's device-and-inode check would then
                        // read a line that landed in the dead claim as one
                        // that landed in the live file.
                        let _ = std::fs::rename(&claim, claim.with_extension("read"));
                    }
                }
            })
        })
        .collect();

    let appender = DigestAppendFile::new(race.store.clone());
    let mut reported = HashSet::new();
    for n in 0..REPRODUCER_APPENDS {
        let identity = format!("kept-{n}");
        if !appender.append(&finding(&identity), &mut std::io::sink()) {
            reported.insert(identity);
        }
    }
    stop.store(true, Ordering::Relaxed);
    for digest in digests {
        digest.join().unwrap();
    }

    let mut seen = collected.lock().unwrap().clone();
    seen.extend(race.identities());
    let lost: Vec<String> = (0..REPRODUCER_APPENDS)
        .map(|n| format!("kept-{n}"))
        .filter(|identity| !seen.contains(identity) && !reported.contains(identity))
        .collect();
    assert!(
        lost.is_empty(),
        "{} lines vanished with nothing said about them: {lost:?}",
        lost.len()
    );
}

#[test]
fn one_retry_is_not_enough_for_a_spool_that_moved_twice() {
    // THE RESIDUAL, deterministically. Two appends is what shipped, and a
    // spool that moved under both of them leaves the line in a batch already
    // read. Only more appends can settle it, which is what the shipped
    // ceiling has to leave room for.
    let moved_twice = || {
        let mut moved = [true, true, false].into_iter();
        move || Ok(moved.next().unwrap())
    };
    let store = Path::new("/nowhere/digest-spool");
    assert!(append_until_the_spool_stops_moving(store, 2, &mut moved_twice()).is_err());
    assert!(
        append_until_the_spool_stops_moving(store, APPEND_ATTEMPTS, &mut moved_twice()).is_ok()
    );
}

#[test]
fn a_spool_that_will_not_settle_is_given_up_on_by_name_and_by_count() {
    // AN UNBOUNDED LOOP HERE IS WORSE THAN THE BUG. Reaching the ceiling is a
    // reported failure naming the spool and how many appends it took, not a
    // spin and not a silent drop: the line is on disk after every attempt.
    let mut attempts = 0;
    let store = Path::new("/nowhere/state/digest-spool");
    let error = append_until_the_spool_stops_moving(store, 4, &mut || {
        attempts += 1;
        Ok(true)
    })
    .unwrap_err();
    assert_eq!(attempts, 4, "the line is written on every attempt");
    let said = error.to_string();
    assert!(said.contains("/nowhere/state/digest-spool"), "{said}");
    assert!(said.contains('4'), "{said}");
}

#[test]
fn a_spool_that_settles_on_a_later_attempt_is_not_a_failure() {
    // The ceiling is not a budget the quiet path spends: a spool that stops
    // moving is done, however many claims it took to get there.
    let mut moved = [true, true, false].into_iter();
    append_until_the_spool_stops_moving(Path::new("/nowhere/digest-spool"), 4, &mut || {
        Ok(moved.next().unwrap())
    })
    .unwrap();
}

#[test]
fn a_line_no_claim_raced_is_written_exactly_once() {
    // The re-append is for a rename that happened, and for nothing else: a
    // quiet spool must not report every finding twice.
    let race = Race::new();
    let appender = DigestAppendFile::new(race.store.clone());
    for n in 0..5 {
        assert!(appender.append(&finding(&format!("quiet-{n}")), &mut std::io::sink()));
    }
    let mut written = race.identities();
    written.sort();
    assert_eq!(
        written,
        (0..5).map(|n| format!("quiet-{n}")).collect::<Vec<_>>()
    );
}
