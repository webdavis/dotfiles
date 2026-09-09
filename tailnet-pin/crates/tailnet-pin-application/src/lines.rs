//! Splitting a hosts file into the lines the resolver sees.

use tailnet_pin_protocol as hosts;

/// One line of a hosts file, and whether the file gave it a terminator.
///
/// THE TERMINATOR IS CARRIED RATHER THAN INFERRED, because only the final line
/// can lack one and both the convergence rule and the reading of that line turn
/// on the answer. Asking the file again later is how two walkers came to
/// disagree about the same last line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    bytes: &'a [u8],
    pub terminated: bool,
}

impl<'a> Line<'a> {
    /// The line as the RESOLVER reads it, which is the only reading anything
    /// downstream uses.
    ///
    /// For a terminated line that is the line. For the final unterminated one
    /// it is the line minus the carriage return the resolver never read there;
    /// see [`hosts::without_unread_carriage_return`] for the measurement.
    pub fn as_the_resolver_reads_it(&self) -> &'a [u8] {
        if self.terminated {
            self.bytes
        } else {
            hosts::without_unread_carriage_return(self.bytes)
        }
    }
}

/// Split a hosts file into its lines.
///
/// AN EMPTY FILE HAS NO LINES, and therefore ends with a terminator by the
/// convergence rule's accounting: it has no final line to be missing one, and
/// answering no there would send every run of an empty hosts file through a
/// rebuild the loopback gate refuses anyway.
pub fn split_lines(contents: &[u8]) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut rest = contents;
    while let Some(end) = rest.iter().position(|byte| *byte == b'\n') {
        lines.push(Line {
            bytes: &rest[..end],
            terminated: true,
        });
        rest = &rest[end + 1..];
    }
    if !rest.is_empty() {
        lines.push(Line {
            bytes: rest,
            terminated: false,
        });
    }
    lines
}

#[cfg(test)]
#[path = "lines/tests.rs"]
mod tests;
