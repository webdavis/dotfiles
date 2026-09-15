//! The window the digest's claim opens under an appender that is mid-write.
//!
//! Every fixture here keys its directory on a clock reading rather than on the
//! process id, because a reused process id finds the previous run's files.

use super::*;
use crate::DigestSpoolFile;
use posture_application::DigestSpool;
use std::sync::atomic::{AtomicUsize, Ordering};
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

    let found: std::collections::HashSet<String> = race.identities().into_iter().collect();
    let missing: Vec<String> = (0..WRITERS)
        .flat_map(|writer| (0..APPENDS).map(move |n| format!("live-{writer}-{n}")))
        .filter(|identity| !found.contains(identity))
        .collect();
    assert!(
        missing.is_empty(),
        "{} lines were lost to one claim: {missing:?}",
        missing.len()
    );
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

#[test]
fn a_spool_renamed_again_and_again_still_takes_each_line_at_most_twice() {
    // The re-append is a retry, not a chase. A loop that kept following the
    // renames would write the same finding into every spool it lost a race to.
    const APPENDS: usize = 400;
    let race = Race::new();
    let appender = DigestAppendFile::new(race.store.clone());
    // Seed the directory so the renamer has somewhere to work from the start.
    assert!(appender.append(&finding("seed"), &mut std::io::sink()));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let renamer = {
        let store = race.store.clone();
        let stop = std::sync::Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut moved = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let aside = store.with_extension(format!("aside-{moved}"));
                if std::fs::rename(&store, &aside).is_ok() {
                    moved += 1;
                }
            }
        })
    };

    for n in 0..APPENDS {
        assert!(appender.append(&finding(&format!("chased-{n}")), &mut std::io::sink()));
    }
    stop.store(true, Ordering::Relaxed);
    renamer.join().unwrap();

    let written = race.identities();
    let mut twice = 0;
    for n in 0..APPENDS {
        let identity = format!("chased-{n}");
        let copies = written.iter().filter(|found| **found == identity).count();
        assert!(
            (1..=2).contains(&copies),
            "{identity} was written {copies} times"
        );
        twice += usize::from(copies == 2);
    }
    // Between a third and a half of these lines meet a rename in practice, so
    // a run where none did means the renamer never raced anything and the
    // bound above was never tested.
    assert!(twice > 0, "no line met a rename");
}
