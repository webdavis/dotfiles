//! The daily digest: drain the spool, group it, send one silent message.
//!
//! Everything the alerter judged interesting but not worth waking anyone for
//! accumulates in a spool, one line per finding. Once a day this claims that
//! batch, renders it into one grouped message, and rotates it aside.
//!
//! THE WHOLE FLOW IS ABOUT NOT LOSING THE BATCH. Two processes share the spool
//! file: the alerter appends to it while this reads it, so the batch is CLAIMED
//! by moving it aside rather than read in place. Every path after that claim
//! either rotates the batch to a forensic copy or puts it back for the next run,
//! and there is no path that drops it. A day of security findings is not
//! something to lose to a failed send.
//!
//! EMPTY IS SILENT, at three separate gates, because a daily message that says
//! "nothing happened" every day is one the operator stops reading, and the one
//! day it says something else looks the same as the rest.

use crate::{Alert, AlertSignal, AlertSink, Submission};
use posture_domain::{DigestEntry, render_digest};

/// One spooled finding, as the batch carries it.
///
/// OWNED STRINGS AND NO WIRE TYPE. `posture-protocol` owns the spool's line
/// format and only `posture-adapters` may depend on it, so the decode happens
/// out there and arrives here already shaped. The alternative, letting policy
/// import the codec, would put the crate that decides things one edit away from
/// knowing what a line looks like on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DigestRow {
    pub detector: Option<String>,
    pub identity: Option<String>,
    pub summary: Option<String>,
}

/// A batch this run has taken ownership of.
///
/// OPAQUE ON PURPOSE. The handle names whatever the adapter needs to find the
/// claimed file again; policy only ever hands it back, and giving it a shape
/// here would make the flow below depend on the spool being a file at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedBatch {
    pub handle: String,
    /// How many findings arrived, counted from the raw lines by whoever read
    /// them.
    ///
    /// A TORN LINE STILL COUNTS, which is why this is not `rows.len()`. The
    /// count answers "how much arrived" and the body answers "how much could be
    /// read"; collapsing them would send the operator looking for a finding the
    /// renderer had already dropped.
    pub item_count: usize,
    pub rows: Vec<DigestRow>,
}

/// Why a batch could not be claimed, in the words the command prints.
///
/// A SENTENCE RATHER THAN AN `io::Error`: the port says nothing about the spool
/// being a file, and the only thing done with this is writing one line, so
/// whoever hit the error names what it was reaching for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimFailure(pub String);

impl std::fmt::Display for ClaimFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// What the spool can be asked to do. Every method is a file move; none decides
/// anything.
pub trait DigestSpool {
    /// Fold any batch a killed run left behind back into the live spool.
    ///
    /// A RUN KILLED BETWEEN CLAIM AND ROTATE leaves its batch owned by nobody:
    /// no later run would think to look for it, and no trap fires on a signal.
    /// Sweeping first is what makes a power loss cost a day's delay rather than
    /// a day's findings.
    fn sweep_orphans(&self);
    /// Take the current batch, leaving a fresh spool for the alerter.
    ///
    /// `Ok(None)` when there is nothing to take, which covers both an absent
    /// spool and an empty one. `Err` when the spool exists and could not be
    /// taken at all, which is a different fact and gets a different answer: an
    /// empty day is silent, a broken one is reported.
    fn claim(&self) -> Result<Option<ClaimedBatch>, ClaimFailure>;
    /// Keep the batch as the forensic copy. Used when it was delivered, and
    /// when it was unrenderable and re-rendering would only fail again.
    fn keep(&self, batch: &ClaimedBatch);
    /// Put the batch back for the next run to retry.
    fn restore(&self, batch: &ClaimedBatch);
}

/// What one run of the digest did, which is what its exit code and its tests
/// read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestOutcome {
    /// There was nothing to say, so nothing was said.
    Silent,
    /// One message went out and the batch was kept for forensics.
    Sent,
    /// The send failed and the batch went back for the next run.
    Restored,
    /// The batch could not be claimed. Whatever is on disk is still on disk for
    /// the next run's sweep, and nothing was sent.
    NotClaimed(ClaimFailure),
}

/// What one run did, and what it could not read while doing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestReport {
    pub outcome: DigestOutcome,
    /// Lines that arrived and decoded into nothing, so the body never showed
    /// them.
    ///
    /// REPORTED RATHER THAN COUNTED IN SILENCE. Dropping a torn line is what
    /// keeps one of them from wedging every later run, but a drop is still a
    /// finding nobody will ever read, so the command says how many.
    pub dropped: usize,
}

/// The digest, over its spool and its sink.
pub struct BuildDigest<'a, S, K> {
    pub spool: &'a S,
    pub sink: &'a mut K,
    /// Today, already formatted, because asking the clock is not this crate's.
    pub utc_day: &'a str,
    pub occurred_at: Option<u64>,
}

impl<S: DigestSpool, K: AlertSink> BuildDigest<'_, S, K> {
    pub fn run(&mut self) -> DigestReport {
        self.spool.sweep_orphans();
        let batch = match self.spool.claim() {
            Ok(Some(batch)) => batch,
            Ok(None) => return DigestReport::of(DigestOutcome::Silent),
            Err(failure) => return DigestReport::of(DigestOutcome::NotClaimed(failure)),
        };
        // WHAT ARRIVED MINUS WHAT RENDERS IS WHAT WAS LOST. A torn line counts
        // as arrived and decodes into nothing, so the difference is exactly the
        // set of findings this digest cannot show.
        let dropped = batch.item_count.saturating_sub(batch.rows.len());
        let entries: Vec<DigestEntry<'_>> = batch
            .rows
            .iter()
            .map(|row| DigestEntry {
                detector: row.detector.as_deref(),
                identity: row.identity.as_deref(),
                summary: row.summary.as_deref(),
            })
            .collect();
        let body = render_digest(&entries);
        if body.trim().is_empty() {
            // EVERY LINE WAS UNREADABLE. Sending a count with an empty body
            // would promise findings the message does not show, and re-rendering
            // the same bytes tomorrow would only render empty again, so the
            // batch is kept for forensics rather than retried forever.
            self.spool.keep(&batch);
            return DigestReport {
                outcome: DigestOutcome::Silent,
                dropped,
            };
        }
        let alert = Alert {
            occurrence_id: None,
            event: "digest",
            // AN OBSERVATION, NEVER A PAGE. The digest is by definition
            // everything that did not earn a page; delivering it as one would
            // undo the tiering that put it here.
            signal: AlertSignal::Observation,
            occurred_at: self.occurred_at,
            title: format!(
                "🗒️ osquery daily digest · {} · {} item(s)",
                self.utc_day, batch.item_count
            ),
            detail: body,
        };
        let outcome = match self.sink.submit(&alert) {
            Submission::Accepted => {
                self.spool.keep(&batch);
                DigestOutcome::Sent
            }
            // NOT ACCEPTED MEANS NOTHING WAS STORED, so restoring cannot
            // double-send: there is no committed obligation to collide with.
            Submission::NotAccepted(_) => {
                self.spool.restore(&batch);
                DigestOutcome::Restored
            }
        };
        DigestReport { outcome, dropped }
    }
}

impl DigestReport {
    /// A run that never reached a batch, so nothing could have been dropped.
    fn of(outcome: DigestOutcome) -> Self {
        Self {
            outcome,
            dropped: 0,
        }
    }
}

#[cfg(test)]
mod tests;
