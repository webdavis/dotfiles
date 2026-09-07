//! The lamps' states and their precedence.
//!
//! PURE AND TOTAL, like every other policy module: no network, no files, no
//! clock and no environment. Only the WORKING-FILE NAME GRAMMAR has arrived so
//! far, because the safety predicates beside it read the same names and cannot
//! move without it; the rest of the lighting policy follows.

pub mod breath;
pub mod held;
pub mod looping;
pub mod mute;
pub mod phase;
pub mod streak;
pub mod unread;

/// The run that owns a WORKING FILE in a marker directory, or None for an
/// ordinary marker.
///
/// TWO SUFFIXES AND ONE ANSWER. A publish writes `<name>.new.<pid>` beside the
/// marker it is about to rename over, and a sweep writes `<name>.sweep.<pid>`
/// when it takes one to remove it. Both are one run's private working name,
/// both carry that run's own process id, and a sweep has to tell them from the
/// markers it is there to judge.
///
/// THE PID IS WHAT MAKES IT DECIDABLE, and matching the bare suffix was not.
/// Pane ids and session ids are opaque words from another program, and both
/// alphabets admit a dot: a pane called `a.new.b` produced a lease file every
/// sweep stepped over, so it aged out never, while a working file whose own run
/// had died was never collected either. A name is a working file only when what
/// follows the LAST such marker is a positive process id, which is a name only
/// this crate's own writers produce.
///
/// THE RIGHTMOST OF THE TWO SUFFIXES, compared by OFFSET rather than tried one
/// after the other: `a.new.b.sweep.1` is the sweep's own working file on a
/// marker shaped like a publish, and trying `.new.` first found it, read the
/// marker's own name as the owner, failed to parse it as a pid and answered
/// `None`, so that working file was never collected. Two runs never write
/// both suffixes into one name, so only one candidate is ever real; comparing
/// offsets picks it without caring which writer's shape it was.
pub fn working_owner(name: &str) -> Option<&str> {
    let pending = name.rfind(WORKING_PENDING).map(|at| (at, WORKING_PENDING));
    let sweep = name.rfind(WORKING_SWEEP).map(|at| (at, WORKING_SWEEP));
    let (at, marker) = match (pending, sweep) {
        (Some(pending), Some(sweep)) => {
            if pending.0 >= sweep.0 {
                pending
            } else {
                sweep
            }
        }
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return None,
    };
    let owner = &name[at + marker.len()..];
    (crate::count::parse_count(owner)? > 0).then_some(owner)
}

/// The two working-file markers, in the spelling their writers use.
const WORKING_PENDING: &str = ".new.";
pub const WORKING_SWEEP: &str = ".sweep.";

#[cfg(test)]
mod tests;
