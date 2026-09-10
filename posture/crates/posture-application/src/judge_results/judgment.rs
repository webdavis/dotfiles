//! What a batch of records turns into, and who turns it.
//!
//! THE JUDGE IS A PORT, not a function called here, because judging a row
//! reaches for things the transaction has no business knowing: the allowlist
//! file, the known-good manifest, the deployed state on disk, the enricher's
//! spawned inspections, and the digest spool it appends the non-paging rows to.
//! Keeping it behind one boundary is what lets the transaction's own ordering
//! (lock, read, judge, deliver, checkpoint) be tested against a double that
//! touches nothing.
//!
//! THE SPOOL WRITE HAPPENS INSIDE THE JUDGE, on purpose. A digest row is
//! DELIVERED the moment it is appended: the daily digest owns it from then on,
//! and nothing here needs to know it happened. Only a page has a delivery this
//! run can fail.

/// The page a batch earned, if it earned one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPage {
    pub title: String,
    pub body: String,
}

/// What judging a batch produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JudgedBatch {
    /// `None` when every row was spooled or dropped, which is the ordinary day.
    pub page: Option<BatchPage>,
}

/// Turn complete result-log records into the batch's page, spooling and
/// dropping the rest on the way.
pub trait JudgeFindings {
    /// `records` is whole lines only, ending in a newline.
    ///
    /// A MALFORMED ROW YIELDS NOTHING FOR THAT ROW and never fails the batch.
    /// osquery can be killed mid-append, and losing a day of findings to one
    /// torn line is not a trade worth making.
    fn judge(&mut self, records: &str) -> JudgedBatch;
}
