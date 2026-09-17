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
//! PAST THE THRESHOLD THE HOUR IS ONE MESSAGE. A handful of unrelated
//! findings inside one hour is a machine in trouble rather than a handful of
//! separate things to read about, so the finding that crosses the threshold
//! carries the whole hour's list and everything after it in the hour is
//! already covered by that message.
//!
//! THE STORM ITSELF IS REMEMBERED, NOT RECOMPUTED FROM THE ENTRY COUNT. Each
//! entry is still pruned individually once it is an hour old, so a storm
//! raised by five findings a second apart would otherwise un-cross the
//! threshold as the oldest entries age out, and the next distinct finding
//! would cross it again and send a second combined message. `stormed_at`
//! stays set for a full hour from the crossing regardless of how the entry
//! count drifts underneath it.
//!
//! IT FAILS OPEN. A window file that cannot be read or written grants the
//! claim: the copy is a second post of a page already delivered, so losing the
//! file costs at worst a few extra copies, while withholding on a read error
//! would silently switch the feature off and nothing would say so.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many DISTINCT findings inside one rolling hour are explained one at a
/// time before the hour is a storm.
///
/// THIS IS THE SPAM LINE, AND IT ALSO BOUNDS COST. Every copy costs a request
/// the far side acts on, and a number high enough to let a real storm through
/// one finding at a time is a number that ships twenty separate messages about
/// one machine. `.chezmoidata/macos_posture_controls.yaml` declares eight
/// controls and the osquery detectors add more, so five distinct critical
/// findings in one hour is already several unrelated things failing at once:
/// past it the useful message is the list, not the next explanation. Repeats
/// are free, because this counts distinct findings.
pub(crate) const STORM_THRESHOLD: usize = 5;

/// How long a claim is remembered, in seconds. One hour, matching the
/// gateway's own duplicate window.
pub(crate) const WINDOW: u64 = 3600;

/// One distinct finding the hour has already seen, and what it says, which is
/// what the combined message lists.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Seen {
    at: u64,
    finding: String,
}

/// The hour on disk: every distinct finding seen, and whether the hour has
/// already crossed into a storm.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Window {
    #[serde(default)]
    seen: BTreeMap<String, Seen>,
    /// When the hour crossed the threshold, kept independent of `seen` so a
    /// storm stays a storm for the full hour even as individual entries age
    /// out of the map.
    #[serde(default)]
    stormed_at: Option<u64>,
}

/// What the window says about one finding's copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Claim {
    /// A distinct finding inside the threshold: copy this page.
    Granted,
    /// This finding was already counted inside the hour.
    AlreadyCopied,
    /// This finding crossed the threshold: send one combined message listing
    /// every distinct finding the hour holds, this one last.
    Storm(Vec<String>),
    /// The hour is already storming and its combined message has been sent.
    Storming,
}

/// Decide this finding's copy against the last hour recorded at `path`, and
/// record the decision, so the next call in this run and the next run after it
/// agree.
///
/// A GRANT IS SPENT ON THE ATTEMPT, not on a delivery. What the cap bounds is
/// cost, and a copy the far side refused cost the same request as one it took.
pub(crate) fn claim(path: &Path, key: &str, finding: &str, now: u64) -> Claim {
    let mut window = read(path);
    window
        .seen
        .retain(|_, entry| now.saturating_sub(entry.at) < WINDOW);
    if window
        .stormed_at
        .is_some_and(|at| now.saturating_sub(at) >= WINDOW)
    {
        window.stormed_at = None;
    }
    if window.seen.contains_key(key) {
        write(path, &window);
        return Claim::AlreadyCopied;
    }
    window.seen.insert(
        key.to_string(),
        Seen {
            at: now,
            finding: finding.to_string(),
        },
    );
    let claim = if window.stormed_at.is_some() {
        Claim::Storming
    } else {
        decide(&window.seen)
    };
    if let Claim::Storm(_) = claim {
        window.stormed_at = Some(now);
    }
    write(path, &window);
    claim
}

/// The pure rule, over an hour this finding is already counted in, called
/// only while no storm is already active: any count past the threshold is a
/// fresh crossing.
fn decide(seen: &BTreeMap<String, Seen>) -> Claim {
    if seen.len() <= STORM_THRESHOLD {
        Claim::Granted
    } else {
        Claim::Storm(listed(seen))
    }
}

/// The hour's findings, oldest first, which is the order they happened in.
fn listed(seen: &BTreeMap<String, Seen>) -> Vec<String> {
    let mut entries: Vec<&Seen> = seen.values().collect();
    entries.sort_by_key(|entry| entry.at);
    entries.iter().map(|entry| entry.finding.clone()).collect()
}

/// The hour on disk, and an empty hour for every reason it cannot be read.
fn read(path: &Path) -> Window {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Best effort, because the copy is worth more than the bookkeeping: a state
/// directory that cannot be made or written costs repeat copies, not a page.
fn write(path: &Path, window: &Window) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(window) {
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
