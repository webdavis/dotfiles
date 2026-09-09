//! Converging one hosts file on one pin.
//!
//! THE WALK IS THE USE CASE. Everything the file says about the pin is settled
//! in one pass over its lines, the domain decides from that whether anything
//! needs doing, and the rebuild is a second pass over the same reading. Two
//! walkers disagreeing about what the last line of one file says is exactly the
//! drift the convergence rule exists to close, so both passes read a line
//! through the same function.

use tailnet_pin_domain::{LOOPBACK, Pin, Survey};
use tailnet_pin_protocol as hosts;

mod lines;
pub use lines::{Line, split_lines};

/// The one thing this needs of a filesystem, so the use case can be driven
/// without one.
///
/// A TRAIT AND NOT A CLOSURE, unlike the smaller seams in this repository: it
/// carries four operations that must agree about the same file, and four
/// closures threaded through one call would be a data clump with no name.
pub trait HostsFile {
    /// The file's bytes, or `None` when it could not be read.
    ///
    /// A SOURCE THAT CANNOT BE READ IS NOT AN EMPTY SOURCE. Treating it as one
    /// collapsed the rebuild to the pin record alone, which is a root rewrite
    /// that throws the file away.
    fn read(&self) -> Option<Vec<u8>>;

    /// Write `contents` somewhere beside the target and install it over the
    /// target atomically, carrying the target's own mode and owner.
    fn install(&self, contents: &[u8]) -> Result<(), String>;
}

/// What a run did, or would not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The file already said exactly the right thing.
    Converged,
    /// The file was rebuilt and installed.
    Written,
    /// Nothing was changed, and this is why.
    Refused(Refusal),
}

/// Why a run changed nothing.
///
/// A PURPOSE-BUILT ENUM RATHER THAN A STRING, because two of these are the
/// safety gates this tool exists for and a caller that could not tell them apart
/// from an ordinary I/O failure could not report them differently either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The file could not be read, and an unreadable hosts file is not an empty
    /// one.
    Unreadable,
    /// No line the rebuild KEEPS maps the loopback address to a name, so
    /// nothing would be left for localhost to resolve through.
    LostLoopback,
    /// Installing the rebuild failed.
    NotInstalled(String),
}

/// Converge `file` on `pin`.
pub fn reconcile(file: &impl HostsFile, pin: &Pin) -> Outcome {
    let Some(contents) = file.read() else {
        return Outcome::Refused(Refusal::Unreadable);
    };
    let lines = split_lines(&contents);
    let record = pin.record();
    if survey(&lines, pin, &record).is_converged() {
        return Outcome::Converged;
    }
    let kept = rebuild(&lines, pin);
    // THE GATE, on the KEPT lines and BEFORE the pin's own record joins them.
    // That ordering is the gate: a pin whose own record maps the loopback
    // address would otherwise vouch for a file that has nothing else left.
    if !kept.iter().any(|line| is_loopback_record(line)) {
        return Outcome::Refused(Refusal::LostLoopback);
    }
    let mut rebuilt = terminated(&kept);
    rebuilt.extend_from_slice(&record);
    rebuilt.push(b'\n');
    match file.install(&rebuilt) {
        Ok(()) => Outcome::Written,
        Err(reason) => Outcome::Refused(Refusal::NotInstalled(reason)),
    }
}

/// Everything one pass over the file learns about the pin.
fn survey(lines: &[Line<'_>], pin: &Pin, record: &[u8]) -> Survey {
    let names = pin.names();
    let mut survey = Survey {
        ends_with_terminator: lines.last().is_none_or(|line| line.terminated),
        ..Survey::default()
    };
    for line in lines {
        let read = line.as_the_resolver_reads_it();
        if hosts::claims_any(read, &names) {
            survey.claiming_lines += 1;
            if read == record {
                survey.desired_record_present = true;
            }
        }
    }
    survey
}

/// Every line the rebuild keeps: the pin's own claims dropped, everything else
/// copied through, comments and blanks included.
fn rebuild<'a>(lines: &[Line<'a>], pin: &Pin) -> Vec<&'a [u8]> {
    let names = pin.names();
    lines
        .iter()
        .map(Line::as_the_resolver_reads_it)
        .filter(|line| !hosts::claims_any(line, &names))
        .collect()
}

fn is_loopback_record(line: &[u8]) -> bool {
    hosts::is_record_for_address(line, LOOPBACK)
}

/// The kept lines, each ending with a terminator.
///
/// THE FINAL LINE OF AN UNTERMINATED SOURCE GAINS ONE, without which the
/// appended record would join onto it. That is one of the two departures from
/// byte fidelity this rebuild makes, and the other, the carriage return the
/// resolver never read, was already made by the reading above.
fn terminated(lines: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for line in lines {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
#[path = "lib/tests.rs"]
mod tests;
