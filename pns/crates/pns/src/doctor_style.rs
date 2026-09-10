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
        Mark::Aside => (Tone::Quiet, ""),
    }
}

/// A row without the `pns doctor: ` the sentence carries for other readers.
///
/// THE PREFIX IS STRIPPED HERE AND NOT AT THE SOURCE, because most of these
/// sentences have a second caller. `routing_complaints` reaches `signal_lamps`
/// and `reconcile_lights` as well, and there the sentence arrives alone with no
/// heading above it, so it has to name what is speaking. Under a titled section
/// the same words repeat what the frame already said, once per row, which is
/// the noise the sections were introduced to remove.
fn unattributed(text: &str) -> &str {
    text.strip_prefix("pns doctor: ").unwrap_or(text)
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
    /// THE FRAME CARRIES NO NOTE ANY MORE. It used to hold one labelled line,
    /// `Note   every suppression gate is bypassed`, which failed twice over:
    /// "suppression gate" is this crate's internal word for the rules that
    /// normally silence a notification, so a reader met jargon before any
    /// finding; and the Channels blurb three lines below said the same thing
    /// again, in the one place where the surrounding rows give it a meaning.
    ///
    /// The fact is not lost, it moved to where it lands: the section that does
    /// the sending says what the sending ignores.
    pub(crate) fn open(&self) -> Vec<String> {
        style::header(self.paint, "pns doctor", &[])
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
                let text = unattributed(text);
                if *mark == Mark::Bad {
                    self.issues.push(text.to_string());
                }
                let (tone, glyph) = appearance(*mark);
                let indent = match mark {
                    Mark::Aside => 7,
                    Mark::Detail => 4,
                    _ => 2,
                };
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
