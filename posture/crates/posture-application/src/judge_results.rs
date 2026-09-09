//! The alerter's transaction: read what is new, judge it, deliver it, and only
//! then remember how far it got.
//!
//! THE CHECKPOINT IS THE WHOLE DESIGN. Every other ordering here follows from
//! one rule: the cursor advances only after a batch is durably delivered or
//! durably stored. A crash anywhere before that re-reads the same rows, which
//! is at-least-once and costs a duplicate page. A cursor that advanced first
//! would be at-most-once, and the row it skipped is the one that mattered.
//!
//! ONE RUN AT A TIME. launchd fires this on a WatchPaths trigger, and a burst
//! of writes fires it several times over. Two runs would read the same cursor,
//! judge the same rows, page twice and race each other's checkpoint, so a run
//! that cannot take the lock is a clean no-op rather than a queued one.
//!
//! IO-FREE AND JSON-FREE. Every reading arrives through a port and every
//! decision belongs to `posture-domain`; this file owns the order they happen
//! in and nothing else.

use crate::{Alert, AlertSignal, AlertSink, Submission};
use posture_domain::{Advance, LiveLog, StoredCursor, advance, complete_records};

mod judgment;
pub use judgment::{BatchPage, JudgeFindings, JudgedBatch};

/// The results log, as this run needs to see it.
pub trait ResultsLog {
    /// The log's inode and size, or `None` when there is no log to read.
    ///
    /// ONE READING, TAKEN ONCE. The size read here bounds the span read below,
    /// so a row appended mid-run lands in the next batch instead of being
    /// consumed early and re-fired.
    fn reading(&self) -> Option<LiveLog>;
    /// `length` bytes starting at `from`.
    ///
    /// A SHORT ANSWER IS NOT AN ERROR. The file can only have grown, so a read
    /// that returns less than asked still returns whole bytes from the right
    /// place, and the cursor advances by what actually arrived.
    fn span(&self, from: u64, length: u64) -> String;
}

/// Where the alerter records how far it has read.
pub trait CursorStore {
    fn read(&self) -> Option<StoredCursor>;
    /// Publish a new cursor. A failure leaves the old one in place, which
    /// re-reads rather than skips.
    fn write(&self, cursor: StoredCursor);
}

/// The single-instance lock every invocation contends on.
pub trait RunLock {
    /// `true` when this run holds it. A run that does not hold it does nothing.
    fn taken(&self) -> bool;
}

/// What one run did, which is what its tests read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeOutcome {
    /// Another run holds the lock. Nothing was read, judged or written.
    Contended,
    /// There was no log, or nothing new in it.
    Quiet,
    /// Rows were judged and the cursor advanced. `paged` says whether any of
    /// them earned a page.
    Advanced { paged: bool },
    /// A page could be neither delivered nor stored, so the cursor stayed put
    /// and the next run retries these same rows.
    Retained,
}

/// The alerter, over its log, its cursor, its judge and its sink.
pub struct JudgeResults<'a, L, C, K, J> {
    pub lock: &'a dyn RunLock,
    pub log: &'a L,
    pub cursor: &'a C,
    pub judge: &'a mut J,
    pub sink: &'a mut K,
    pub occurred_at: Option<u64>,
}

impl<L: ResultsLog, C: CursorStore, K: AlertSink, J: JudgeFindings> JudgeResults<'_, L, C, K, J> {
    pub fn run(&mut self) -> JudgeOutcome {
        if !self.lock.taken() {
            return JudgeOutcome::Contended;
        }
        let Some(live) = self.log.reading() else {
            return JudgeOutcome::Quiet;
        };
        let stored = self.cursor.read();
        let Advance::Read { from, reset } = advance(stored, live) else {
            return JudgeOutcome::Quiet;
        };
        if reset {
            self.report_reset(live);
        }
        let snapshot = self.log.span(from, live.size.saturating_sub(from));
        let records = complete_records(&snapshot);
        // A SNAPSHOT OF ONE TORN LINE ADVANCES NOTHING, so the run stops here
        // rather than checkpointing at an offset it did not read past.
        if records.bytes == 0 {
            return JudgeOutcome::Quiet;
        }
        let checkpoint = StoredCursor {
            inode: live.inode,
            offset: from + records.bytes,
        };
        let batch = self.judge.judge(records.text);
        self.deliver(batch, from, checkpoint)
    }

    /// Send the batch's page, if it has one, then checkpoint if that held.
    fn deliver(
        &mut self,
        batch: JudgedBatch,
        from: u64,
        checkpoint: StoredCursor,
    ) -> JudgeOutcome {
        let Some(page) = batch.page else {
            // NO PAGE IS NOT NO WORK. The rows were spooled to the digest or
            // dropped as log-only, both of which the judge has already done, so
            // the cursor advances over them.
            self.cursor.write(checkpoint);
            return JudgeOutcome::Advanced { paged: false };
        };
        let alert = Alert {
            // THE BYTE RANGE IS THE IDENTITY. A retry of this same batch reads
            // the same rows and derives the same id, so the store and the
            // gateway both recognize it rather than delivering it twice.
            occurrence_id: Some(format!("{}:{}:{}", checkpoint.inode, from, checkpoint.offset)),
            event: "alert",
            signal: AlertSignal::NeedsAttention,
            occurred_at: self.occurred_at,
            title: page.title,
            detail: page.body,
        };
        match self.sink.submit(&alert) {
            Submission::Accepted => {
                self.cursor.write(checkpoint);
                JudgeOutcome::Advanced { paged: true }
            }
            // NOTHING WAS STORED, so the cursor stays where it is and the next
            // run judges these rows again. The failure is already loud on its
            // own path; this run's job is only to not lose the rows.
            Submission::NotAccepted(_) => JudgeOutcome::Retained,
        }
    }

    /// Say out loud that the cursor was missing or unreadable.
    ///
    /// A LOST CURSOR IS AN ALERTING FAILURE. The replay below re-surfaces every
    /// finding in the log on its own, so this is not about the findings: it is
    /// the meta-signal that the alerter's own state was disturbed, which is the
    /// first thing anyone tampering with this machine would reach for.
    fn report_reset(&mut self, live: LiveLog) {
        let alert = Alert {
            // REPEATED RESETS OVER ONE LOG SHARE AN ID, so a machine resetting
            // every tick raises one page rather than a storm.
            occurrence_id: Some(format!("cursor-reset:{}:{}", live.inode, live.size)),
            event: "cursor-reset",
            signal: AlertSignal::NeedsAttention,
            occurred_at: self.occurred_at,
            title: String::from("🔴 **osquery cursor reset**"),
            detail: String::from(
                "**The osquery alerter cursor was missing or corrupt, so monitoring state was disturbed.**\n\
                 - The current log was replayed in full, so findings are re-surfaced below rather than lost.\n\
                 - If you did not clear ~/.local/state, something else reset it. **Investigate now.**",
            ),
        };
        // BEST EFFORT, DELIBERATELY. This warning must never stand between the
        // batch and its own delivery, which has its own gate below.
        let _ = self.sink.submit(&alert);
    }
}

#[cfg(test)]
mod tests;
