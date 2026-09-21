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

/// The durable activity table, which is where the recap's agents section
/// comes from. NOT THE RING: the ring keeps a bounded tail of rendered lines
/// for the return card and rolls off by count, where this keeps the fields
/// themselves and rolls off by age.
pub trait ActivityEvents {
    fn activity_between(&self, since: u64, until: u64) -> Vec<pns_domain::recap::activity::Event>;
}

/// One `[recap.sources]` command, run over a window or over none at all.
///
/// `since` AND `until` ARE OPTIONAL because `open` has no window: a command
/// asked with no bounds is the same command with nothing substituted into it,
/// which is how the design says the unbounded listings are taken.
pub trait SourceCommands {
    fn run(
        &self,
        argv: &[String],
        since: Option<u64>,
        until: Option<u64>,
    ) -> pns_domain::recap::external::Sourcing;
}

pub trait ReviewNoteSource {
    fn notes(&self, pattern: &str, since: u64, until: u64) -> Option<Fetched>;
}

pub trait Summarizer {
    fn summarize(&self, argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>>;
}

/// How many delivery legs the retry policy gave up on and nobody has cleared.
///
/// THE SAME LISTING `pns failures` PRINTS, filtered to the dead-lettered rows,
/// so the recap's count and the command it points at cannot disagree.
pub trait DeadLetteredLegs {
    fn dead_lettered(&self) -> usize;
}
