use std::time::Duration;

/// What one external source held, and whether a cap stopped the read short of
/// everything there was.
///
/// TRUNCATION TRAVELS WITH THE SOURCES rather than being recomputed from their
/// length, because the two caps are different facts: a listing that came back at
/// exactly `GH_LIMIT` may have more behind it, and a glob matching more files
/// than `MAX_NOTES` certainly does. Only the fetch knows which, and the message
/// says "at least" on either.
pub struct Fetched {
    pub sources: Vec<pns_domain::recap::external::Sourced>,
    pub truncated: bool,
}

pub trait MergedPullRequestSource {
    fn merged(&self, repos: &[String], since: u64, until: u64) -> Option<Fetched>;
}

pub trait ReviewNoteSource {
    fn notes(&self, pattern: &str, since: u64, until: u64) -> Option<Fetched>;
}

pub trait Summarizer {
    fn summarize(&self, argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>>;
}
