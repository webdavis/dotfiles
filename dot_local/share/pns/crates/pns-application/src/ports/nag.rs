use pns_domain::EventArgs;
use pns_domain::nag::Record as NagRecord;

/// This session's nag: the schedule that nudges about a wait nobody answered.
///
/// ARMING IS ONE STEP AND NOT THREE. It unlinks the session's answered marker,
/// publishes the record and re-arms, and a caller that could do two of those
/// would leave a session armed against a marker that says it is already
/// answered. Which agents nag at all, and after how long, is the adapter's
/// read of the config. Statements: S074, S237.
pub trait NagSchedule {
    fn arm(&self, session_id: &str, event: &EventArgs);
}

/// The nag's own records: one per session waiting on an approval.
///
/// THE FIRE LOCK IS ONE CLAIM AND NOT A FLAG. Exclusive creation keeps the
/// lock at its contested name throughout the fire. Renaming it away allowed
/// another contender to recreate it and send a duplicate card. Rename is for
/// stale-lock takeover and individual record claims, never initial creation.
///
/// SESSIONS AND NOT PATHS. Where a record lives, and the rename protocol that
/// claims it, are the filesystem adapter's business in PR 11.5; a use case
/// knows only which sessions are due and that it holds each one.
///
/// `src/command_nag.rs` owns fire claims and disabled cleanup;
/// `src/nag_schedule_runtime.rs` owns arming, rollback and answered markers.
/// Statements: S182, S236, S237, S241.
pub trait NagRecords {
    /// Take the single fire lock for this run, or answer false when another
    /// run holds it.
    fn claim_fire(&self, now: u64) -> bool;
    /// Every session due a nudge, each claimed by this run.
    fn claim_due(&self, now: u64) -> Vec<(String, NagRecord)>;
    fn release_fire(&self);
    /// Mark this session answered, so the backstop stops nudging about it.
    fn mark_answered(&self, session_id: &str);
    /// Clear the previous answer before publishing a new wait. Absence is
    /// success; other failures remain available to the caller's warning.
    fn clear_answered(&self, session_id: &str) -> Result<(), String>;
    /// Publish this wait. A failed publication must prevent scheduling it.
    fn publish(&self, session_id: &str, record: &NagRecord) -> Result<(), String>;
    /// Roll back a refused schedule. Any unlink error, including absence,
    /// remains distinct from successful removal in the caller's report.
    fn drop_record(&self, session_id: &str) -> Result<(), String>;
    /// Drop waiting records when the feature is disabled and count successful
    /// removals only. This operation needs neither a clock nor a fire claim.
    fn clear_pending(&self) -> usize;
}
