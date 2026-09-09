//! The doctor's report, painted.
//!
//! It owns the report's SHAPE on a terminal (a frame, then sections, then a
//! closing list of what to act on) and borrows every color and glyph from
//! `style`, so a second command growing a report gets the same look without
//! copying anything out of here.

use crate::style::{self, Paint, Tone};
use pns_domain::doctor::{Item, Mark};

/// What each mark looks like.
///
/// GLYPHS SURVIVE PLAIN MODE and only color is dropped. Plain mode is for a
/// pipe or a file, where an escape sequence is corruption; a mark is the row's
/// meaning, so stripping it would leave the plain report saying less than the
/// painted one rather than the same thing unpainted.
fn appearance(mark: Mark) -> (Tone, &'static str) {
    match mark {
        Mark::Good => (Tone::Good, "✓"),
        Mark::Bad => (Tone::Bad, "✗"),
        Mark::Warn => (Tone::Warn, "⚠"),
        Mark::Note => (Tone::Quiet, "·"),
        Mark::Detail => (Tone::Quiet, "→"),
    }
}

/// Renders the report, and remembers what went wrong so it can say so at the
/// end.
pub(crate) struct Report {
    paint: Paint,
    issues: Vec<String>,
    opened: bool,
}

impl Report {
    pub(crate) fn new(paint: Paint) -> Self {
        Self {
            paint,
            issues: Vec::new(),
            opened: false,
        }
    }

    /// The frame the report opens with.
    ///
    /// EVERY METHOD RETURNS ITS LINES rather than printing them, so the caller
    /// prints each one as it is produced and a test can read them without
    /// capturing a process's output. The doctor's own ordering comments depend
    /// on that progressive printing: the lamps section touches the network
    /// last, so a bridge that hangs must not delay a line above it.
    /// NOTHING IN THE FRAME FLOATS. The line under the command used to be a
    /// bare sentence, `every suppression gate is bypassed` sitting under
    /// `pns doctor`, and a reader had no way to tell whether that was a
    /// description, a status or an error. It reads like something went wrong.
    /// The `Note` label is what says which of the three it is, and it costs one
    /// word.
    pub(crate) fn open(&self, note: &str) -> Vec<String> {
        style::header(
            self.paint,
            "pns doctor",
            &[style::HeaderLine {
                label: "Note",
                text: note,
            }],
        )
    }

    /// One piece of the report.
    pub(crate) fn item(&mut self, item: &Item) -> Vec<String> {
        match item {
            Item::Section { title, blurb } => {
                // A blank line BEFORE each heading and never after the last
                // row, so the gap belongs to the section it opens and the
                // report never ends on whitespace.
                self.opened = true;
                vec![String::new(), style::heading(self.paint, title, blurb)]
            }
            Item::Row { mark, text } => {
                if *mark == Mark::Bad {
                    self.issues.push(text.clone());
                }
                let (tone, glyph) = appearance(*mark);
                let indent = if *mark == Mark::Detail { 4 } else { 2 };
                vec![style::row(self.paint, tone, glyph, indent, text)]
            }
        }
    }

    /// The closing list of what to act on.
    ///
    /// IT REPEATS ROWS ALREADY PRINTED, on purpose. The report is long enough
    /// that the one failing row scrolls off, and an operator who reads only the
    /// last few lines still learns what is broken.
    pub(crate) fn close(&self) -> Vec<String> {
        let mut lines = vec![String::new(), style::rule(self.paint)];
        if self.issues.is_empty() {
            lines.push(format!("  {} nothing to act on", self.paint.good("✓")));
            return lines;
        }
        let count = self.issues.len();
        let plural = if count == 1 { "" } else { "s" };
        lines.push(format!(
            "  {}",
            self.paint.bad(&format!("{count} issue{plural} to fix:"))
        ));
        for (index, issue) in self.issues.iter().enumerate() {
            lines.push(format!("  {}. {issue}", index + 1));
        }
        lines
    }
}

#[cfg(test)]
#[path = "doctor_style/tests.rs"]
mod tests;
