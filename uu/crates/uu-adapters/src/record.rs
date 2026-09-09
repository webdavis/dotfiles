//! The records path: one entry per run, posted straight to the hermes
//! gateway.
//!
//! THREE QUESTIONS, three files. This one composes THE ENTRY: what the run
//! amounted to and the detail a human reads. The protocol crate encodes its body.
//! `marker` owns the last-successful-run timestamp and the gap sentence every
//! entry opens with; `event` maps the run facts into the child protocol.

mod event;
mod marker;

pub use event::event_for;
pub use marker::{gap_line, marker_contents, parse_marker};

use uu_domain::LaneReport;

/// The record's `state` field: what the whole run amounted to.
///
/// ONE FAILURE ANYWHERE MAKES THE RUN FAILED, which wins over a deferral in
/// the same run: the record is read at a glance, and a partial success
/// reported as a success is exactly the reading the record exists to
/// prevent. With no failure, a run that DEFERRED at least one lane is its
/// own state rather than "completed": nothing happened is a different week
/// from nothing needed to happen, and collapsing the two into "completed"
/// is exactly the reading a deferral must never get.
pub fn record_state(failures: usize, deferred: usize, pending: usize) -> &'static str {
    if failures > 0 {
        "failed"
    } else if deferred > 0 {
        "deferred"
    } else if pending > 0 {
        "pending"
    } else {
        "completed"
    }
}

/// The record's `detail`: the header, the gap, every lane's own lines, and the
/// count that closes it.
///
/// A RUN THAT RAN NO LANE SAYS SO. An entry listing nothing reads identically
/// to an entry whose lanes all passed quietly, and those are very different
/// weeks: one of them is a config that turned everything off.
///
/// A DEFERRED LANE IS NAMED `deferred`, never `0 failure(s)`: the two read
/// identically to a lane that ran clean, and "nothing happened" is not the
/// same week as "nothing needed to happen".
pub fn record_detail(host: &str, now_iso: &str, gap: &str, lanes: &[LaneReport]) -> String {
    let mut out = format!("run at {now_iso} on {host}\n{gap}\n");
    if lanes.is_empty() {
        out.push_str("no lane is enabled in this config, so nothing was updated\n");
    }
    let mut failures = 0;
    let mut deferred = 0;
    let mut pending = 0;
    for lane in lanes {
        failures += lane.failures();
        let verdict = if lane.verdict() == uu_domain::LaneVerdict::Deferred {
            deferred += 1;
            "deferred".to_string()
        } else if lane.verdict() == uu_domain::LaneVerdict::Pending {
            pending += 1;
            "pending".to_string()
        } else {
            format!("{} failure(s)", lane.failures())
        };
        out.push_str(&format!("\n{}: {verdict}\n", lane.name));
        for line in &lane.lines {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str(&format!(
        "\n=== done, {failures} failure(s), {deferred} deferred, {pending} pending ===\n"
    ));
    out
}

#[cfg(test)]
mod tests;
