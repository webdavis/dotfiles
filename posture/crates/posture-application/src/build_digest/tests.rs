//! Every expectation was read off `executable_digest.sh`, whose whole design is
//! about never losing the batch.

use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct Spool {
    batch: Option<ClaimedBatch>,
    calls: RefCell<Vec<&'static str>>,
}

impl Spool {
    fn holding(item_count: usize, rows: Vec<DigestRow>) -> Self {
        Self {
            batch: Some(ClaimedBatch {
                handle: "work-file".into(),
                item_count,
                rows,
            }),
            calls: RefCell::new(Vec::new()),
        }
    }
    fn did(&self) -> Vec<&'static str> {
        self.calls.borrow().clone()
    }
}

impl DigestSpool for Spool {
    fn sweep_orphans(&self) {
        self.calls.borrow_mut().push("sweep");
    }
    fn claim(&self) -> Option<ClaimedBatch> {
        self.calls.borrow_mut().push("claim");
        self.batch.clone()
    }
    fn keep(&self, _: &ClaimedBatch) {
        self.calls.borrow_mut().push("keep");
    }
    fn restore(&self, _: &ClaimedBatch) {
        self.calls.borrow_mut().push("restore");
    }
}

#[derive(Default)]
struct Sink {
    refuse: bool,
    sent: Vec<Alert>,
}

impl AlertSink for Sink {
    fn submit(&mut self, alert: &Alert) -> Submission {
        self.sent.push(alert.clone());
        if self.refuse {
            Submission::NotAccepted(crate::SubmissionFailure::Failed)
        } else {
            Submission::Accepted
        }
    }
}

fn row(detector: &str, identity: &str) -> DigestRow {
    DigestRow {
        detector: Some(detector.into()),
        identity: Some(identity.into()),
        summary: Some(format!("{detector} {identity}")),
    }
}

fn run(spool: &Spool, sink: &mut Sink) -> DigestOutcome {
    BuildDigest {
        spool,
        sink,
        utc_day: "2026-09-09",
        occurred_at: Some(100),
    }
    .run()
}

#[test]
fn orphans_are_swept_before_the_batch_is_claimed() {
    // A run killed between claim and rotate leaves a batch owned by nobody, and
    // no later run would think to look for it. Sweeping after the claim would
    // fold it into a spool this run has already taken.
    let spool = Spool::default();
    run(&spool, &mut Sink::default());
    assert_eq!(spool.did(), ["sweep", "claim"]);
}

#[test]
fn an_empty_spool_says_nothing_and_keeps_nothing() {
    // A daily message that says "nothing happened" every day is one the
    // operator stops reading.
    let spool = Spool::default();
    let mut sink = Sink::default();
    assert_eq!(run(&spool, &mut sink), DigestOutcome::Silent);
    assert!(sink.sent.is_empty());
}

#[test]
fn a_batch_of_findings_is_sent_once_and_kept_for_forensics() {
    let spool = Spool::holding(2, vec![row("alpha", "one"), row("beta", "two")]);
    let mut sink = Sink::default();
    assert_eq!(run(&spool, &mut sink), DigestOutcome::Sent);
    assert_eq!(sink.sent.len(), 1);
    assert_eq!(spool.did(), ["sweep", "claim", "keep"]);
}

#[test]
fn the_title_carries_the_day_and_the_count() {
    let spool = Spool::holding(2, vec![row("alpha", "one"), row("beta", "two")]);
    let mut sink = Sink::default();
    run(&spool, &mut sink);
    assert_eq!(
        sink.sent[0].title,
        "🗒️ osquery daily digest · 2026-09-09 · 2 item(s)"
    );
}

#[test]
fn a_torn_line_still_counts_toward_the_title_even_though_it_cannot_be_rendered() {
    // The count answers "how much arrived" and the body answers "how much could
    // be read". A title that hid the difference would send the operator looking
    // for a finding the renderer had already dropped.
    // Two lines arrived; only one decoded, so the adapter counted two.
    let spool = Spool::holding(2, vec![row("alpha", "one")]);
    let mut sink = Sink::default();
    assert_eq!(run(&spool, &mut sink), DigestOutcome::Sent);
    assert!(
        sink.sent[0].title.contains("2 item(s)"),
        "{:?}",
        sink.sent[0].title
    );
    assert!(sink.sent[0].detail.contains("alpha"));
    assert!(!sink.sent[0].detail.contains("beta"));
}

#[test]
fn a_batch_whose_every_line_is_unreadable_is_kept_rather_than_sent_or_retried() {
    // Sending a count with an empty body would promise findings the message does
    // not show, and re-rendering the same bytes tomorrow renders empty again.
    let spool = Spool::holding(2, Vec::new());
    let mut sink = Sink::default();
    assert_eq!(run(&spool, &mut sink), DigestOutcome::Silent);
    assert!(sink.sent.is_empty());
    assert_eq!(spool.did(), ["sweep", "claim", "keep"]);
}

#[test]
fn a_refused_send_puts_the_batch_back_for_the_next_run() {
    // NOT ACCEPTED MEANS NOTHING WAS STORED, so restoring cannot double-send.
    let spool = Spool::holding(1, vec![row("alpha", "one")]);
    let mut sink = Sink {
        refuse: true,
        ..Sink::default()
    };
    assert_eq!(run(&spool, &mut sink), DigestOutcome::Restored);
    assert_eq!(spool.did(), ["sweep", "claim", "restore"]);
}

#[test]
fn the_digest_is_an_observation_and_never_a_page() {
    // It is by definition everything that did not earn a page; delivering it as
    // one would undo the tiering that put it here.
    let spool = Spool::holding(1, vec![row("alpha", "one")]);
    let mut sink = Sink::default();
    run(&spool, &mut sink);
    assert_eq!(sink.sent[0].signal, AlertSignal::Observation);
    assert_eq!(sink.sent[0].event, "digest");
    assert_eq!(sink.sent[0].occurrence_id, None);
}
