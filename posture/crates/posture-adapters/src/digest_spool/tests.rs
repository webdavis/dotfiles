//! Every expectation was read off `executable_digest.sh`, whose file dance is
//! the part of the digest that must never lose a batch.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture {
    root: PathBuf,
    store: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "posture-digest-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let store = root.join("state").join("digest-spool");
        prepare_spool_directory(&store).unwrap();
        Self { root, store }
    }

    fn spool(&self, seconds: u64) -> DigestSpoolFile {
        DigestSpoolFile::new(self.store.clone(), seconds, std::process::id())
    }

    fn write_spool(&self, contents: &str) {
        fs::write(&self.store, contents).unwrap();
    }

    fn spool_contents(&self) -> String {
        fs::read_to_string(&self.store).unwrap_or_default()
    }

    fn kept(&self) -> Option<String> {
        fs::read_to_string(self.root.join("state").join("digest-spool.last")).ok()
    }

    /// Everything beside the spool itself, so a leftover claim is visible.
    fn strays(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.root.join("state"))
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "digest-spool")
            .collect();
        names.sort();
        names
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn line(detector: &str, identity: &str) -> String {
    posture_protocol::encode(&posture_protocol::DigestRecord {
        detector: Some(detector.into()),
        identity: Some(identity.into()),
        summary: Some(format!("{detector} saw {identity}")),
        ..posture_protocol::DigestRecord::default()
    })
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn an_absent_or_empty_spool_offers_no_batch() {
    // Two spellings of the same fact, and the digest treats them alike: a run
    // that claimed an empty file would send a message with nothing in it.
    let fixture = Fixture::new();
    assert!(fixture.spool(100).claim().is_none());
    fixture.write_spool("");
    assert!(fixture.spool(100).claim().is_none());
}

#[test]
fn a_spool_of_only_blank_lines_is_claimed_away_rather_than_left_to_accumulate() {
    // Nothing to say, but the lines have to go, or every later run re-reads
    // them and the file grows forever.
    let fixture = Fixture::new();
    fixture.write_spool("\n\n  \n");
    assert!(fixture.spool(100).claim().is_none());
    assert_eq!(fixture.spool_contents(), "");
    assert!(fixture.strays().is_empty(), "{:?}", fixture.strays());
}

#[test]
fn claiming_moves_the_batch_aside_so_the_alerter_writes_into_a_fresh_spool() {
    // THE WHOLE PROTOCOL IS THIS RENAME. Reading in place would race every
    // append the alerter makes while the digest renders.
    let fixture = Fixture::new();
    fixture.write_spool(&format!("{}\n", line("alpha", "one")));
    let batch = fixture.spool(100).claim().unwrap();
    assert_eq!(fixture.spool_contents(), "");
    assert!(Path::new(&batch.handle).exists());
}

#[test]
fn a_claimed_batch_carries_its_rows_and_counts_the_lines_that_arrived() {
    // The count answers "how much arrived" and the rows answer "how much could
    // be read", so a torn line raises one without the other.
    let fixture = Fixture::new();
    fixture.write_spool(&format!(
        "{}\n{{\"detector\":\n{}\n",
        line("alpha", "one"),
        line("beta", "two")
    ));
    let batch = fixture.spool(100).claim().unwrap();
    assert_eq!(batch.item_count, 3);
    assert_eq!(batch.rows.len(), 2);
    assert_eq!(batch.rows[0].detector.as_deref(), Some("alpha"));
    assert_eq!(batch.rows[1].detector.as_deref(), Some("beta"));
}

#[test]
fn a_kept_batch_is_rotated_to_one_forensic_copy_readable_only_by_its_owner() {
    // The spool carries full filesystem paths, which is what makes it useful
    // for triage and also what makes it private.
    let fixture = Fixture::new();
    fixture.write_spool(&format!("{}\n", line("alpha", "one")));
    let spool = fixture.spool(100);
    let batch = spool.claim().unwrap();
    spool.keep(&batch);
    assert!(fixture.kept().unwrap().contains("alpha"));
    assert_eq!(
        mode(&fixture.root.join("state").join("digest-spool.last")),
        0o600
    );
    assert!(
        fixture.strays() == ["digest-spool.last"],
        "{:?}",
        fixture.strays()
    );
}

#[test]
fn keeping_a_batch_leaves_no_claim_for_a_later_sweep_to_send_twice() {
    // A claim that survived its own delivery would be folded back by the next
    // run's sweep, and the day's findings would arrive a second time.
    let fixture = Fixture::new();
    fixture.write_spool(&format!("{}\n", line("alpha", "one")));
    let spool = fixture.spool(100);
    let batch = spool.claim().unwrap();
    spool.keep(&batch);
    fixture.spool(200).sweep_orphans();
    assert_eq!(fixture.spool_contents(), "");
}

#[test]
fn a_restored_batch_is_appended_so_a_concurrent_append_survives_it() {
    // A RENAME WOULD CLOBBER. The alerter can add a finding to the fresh spool
    // while the digest is building, and putting the batch back must not destroy
    // it. Order inside a grouped digest carries no meaning, so appending costs
    // nothing.
    let fixture = Fixture::new();
    fixture.write_spool(&format!("{}\n", line("alpha", "one")));
    let spool = fixture.spool(100);
    let batch = spool.claim().unwrap();
    fixture.write_spool(&format!("{}\n", line("beta", "two")));
    spool.restore(&batch);
    let back = fixture.spool_contents();
    assert!(back.contains("alpha"), "{back}");
    assert!(back.contains("beta"), "{back}");
    assert!(fixture.strays().is_empty(), "{:?}", fixture.strays());
}

#[test]
fn a_restored_batch_is_claimed_whole_by_the_next_run() {
    // Round trip: whatever went back has to come out again as one batch, or
    // "restore for the next run" is only half true.
    let fixture = Fixture::new();
    fixture.write_spool(&format!(
        "{}\n{}\n",
        line("alpha", "one"),
        line("beta", "two")
    ));
    let first = fixture.spool(100);
    let batch = first.claim().unwrap();
    first.restore(&batch);
    let second = fixture.spool(200).claim().unwrap();
    assert_eq!(second.item_count, 2);
    assert_eq!(second.rows.len(), 2);
}

#[test]
fn a_batch_a_killed_run_abandoned_is_folded_back_by_the_next_sweep() {
    // A RUN KILLED BETWEEN CLAIM AND ROTATE leaves its batch owned by nobody,
    // and no later run would think to look for it. Sweeping is what makes a
    // power loss cost a day's delay rather than a day's findings.
    let fixture = Fixture::new();
    fixture.write_spool(&format!("{}\n", line("alpha", "one")));
    let abandoned = fixture.spool(100);
    let _orphan = abandoned.claim().unwrap();
    std::mem::forget(_orphan);
    fixture.write_spool(&format!("{}\n", line("beta", "two")));

    let next = fixture.spool(200);
    next.sweep_orphans();
    let batch = next.claim().unwrap();
    assert_eq!(batch.item_count, 2);
}

#[test]
fn a_sweep_with_nothing_abandoned_leaves_the_spool_exactly_as_it_found_it() {
    // The steady state, which is every day nothing crashed.
    let fixture = Fixture::new();
    let contents = format!("{}\n", line("alpha", "one"));
    fixture.write_spool(&contents);
    fixture.spool(100).sweep_orphans();
    assert_eq!(fixture.spool_contents(), contents);
}

#[test]
fn a_folded_batch_never_runs_two_lines_together() {
    // A spool whose last append was interrupted before its newline would
    // otherwise glue that torn line onto the first swept one, turning two
    // damaged records into one and losing both.
    let fixture = Fixture::new();
    fixture.write_spool(&line("alpha", "one"));
    let abandoned = fixture.spool(100);
    let orphan = abandoned.claim().unwrap();
    std::mem::forget(orphan);
    fixture.write_spool(&line("beta", "two"));

    let next = fixture.spool(200);
    next.sweep_orphans();
    assert_eq!(next.claim().unwrap().rows.len(), 2);
}

#[test]
fn the_spool_directory_is_private_before_any_file_lands_in_it() {
    // A file's own mode is set after it exists, so the parent is what covers
    // the window in between.
    let fixture = Fixture::new();
    assert_eq!(mode(&fixture.root.join("state")), 0o700);
}
