//! The rolling-hour decision for the critical copy, kept in one timestamp file
//! beside posture's other state.
//!
//! IT COUNTS DISTINCT FINDINGS, NOT PAGES. One finding paging six times in an
//! hour is one thing wrong with the machine, and counting its pages would let
//! a loop crowd out every other finding's copy, which is the exact shape the
//! digest's own repeat handling had to correct. So the first page of a finding
//! claims the budget and every repeat of that finding inside the hour claims
//! nothing and is copied no second time.
//!
//! IT FAILS OPEN. A window file that cannot be read or written grants the
//! claim: the copy is a second post of a page already delivered, so losing the
//! file costs at worst a few extra copies, while withholding on a read error
//! would silently switch the feature off and nothing would say so.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many DISTINCT findings may be copied inside one rolling hour.
///
/// THIS BOUNDS COST, NOT NOISE. Every copy costs a request the far side acts
/// on, and twenty is sized against the measured ceiling of things posture can
/// page about at all, which is under twenty: reaching it means every single
/// thing posture watches failed at once and each still got its copy, and
/// anything past it is a loop rather than a report. Repeats are already free,
/// because this counts distinct findings, so lowering the number cannot make a
/// storm quieter. It can only withhold the copy of a finding nobody has seen
/// yet, which is why lowering it to quiet a channel has to argue past this
/// comment first.
pub(crate) const DISTINCT_FINDINGS_PER_HOUR: usize = 20;

/// How long a claim is remembered, in seconds. One hour, matching the
/// gateway's own duplicate window.
pub(crate) const WINDOW: u64 = 3600;

/// What the window says about one finding's copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Claim {
    /// The first page of this finding inside the hour: copy it.
    Granted,
    /// This finding was already copied inside the hour.
    AlreadyCopied,
    /// The hour already holds [`DISTINCT_FINDINGS_PER_HOUR`] other findings.
    CapReached,
}

/// Decide this finding's copy against the last hour recorded at `path`, and
/// record the decision, so the next call in this run and the next run after it
/// agree.
///
/// A GRANT IS SPENT ON THE ATTEMPT, not on a delivery. What the cap bounds is
/// cost, and a copy the far side refused cost the same request as one it took.
pub(crate) fn claim(path: &Path, finding: &str, now: u64) -> Claim {
    let mut claimed = read(path);
    claimed.retain(|_, at| now.saturating_sub(*at) < WINDOW);
    let claim = decide(&claimed, finding);
    if claim == Claim::Granted {
        claimed.insert(finding.to_string(), now);
    }
    write(path, &claimed);
    claim
}

/// The pure rule, over an already-pruned hour.
fn decide(claimed: &BTreeMap<String, u64>, finding: &str) -> Claim {
    if claimed.contains_key(finding) {
        return Claim::AlreadyCopied;
    }
    if claimed.len() >= DISTINCT_FINDINGS_PER_HOUR {
        return Claim::CapReached;
    }
    Claim::Granted
}

/// The hour on disk, and an empty hour for every reason it cannot be read.
fn read(path: &Path) -> BTreeMap<String, u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Best effort, because the copy is worth more than the bookkeeping: a state
/// directory that cannot be made or written costs repeat copies, not a page.
fn write(path: &Path, claimed: &BTreeMap<String, u64>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(claimed) {
        let _ = std::fs::write(path, text);
    }
}

/// Wall-clock seconds, and zero for a clock before the epoch, which prunes the
/// window rather than trusting a time nothing can be measured against.
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
