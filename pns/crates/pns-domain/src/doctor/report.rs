//! The SHAPE of the doctor's report, with no opinion about how it looks.
//!
//! The report used to be a flat run of sentences, each carrying its own `pns
//! doctor:` prefix, which is how it grew to twenty lines that read as one
//! paragraph. The information was all there; nothing said which lines belonged
//! together, or which of them was the one to act on.
//!
//! NO COLOR, NO GLYPHS AND NO WIDTHS LIVE HERE. This says what a line MEANS,
//! and the command that prints it decides what that looks like on the terminal
//! it is writing to. That split is what lets the same report render in color
//! for an operator and in plain text down a pipe without either rendering being
//! a second copy of the report.

/// How one row reads at a glance.
///
/// THE MARK IS THE SUMMARY. An operator scanning the report reads the marks and
/// stops at the first one that is not good, so a row's mark is a claim about
/// whether anything needs doing, never about how interesting it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// It works.
    Good,
    /// It does not work, and the operator has to do something. ONLY THIS MARK
    /// reaches the closing list of issues, so it is the one to be careful with.
    Bad,
    /// It works less than fully, or it is off, and that may be deliberate.
    Warn,
    /// A reading, graded neither way. Most of the report is this: what a
    /// setting is, what the log last recorded, what the lamps last did.
    Note,
    /// A continuation of the row above, for the sentence that would not fit on
    /// it.
    Detail,
}

/// One piece of the report, in the order it is printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A heading, and one line saying what the rows under it are for.
    ///
    /// THE BLURB IS THE POINT of the heading, not decoration: a section named
    /// "Routes" tells an operator nothing they did not already read in its
    /// rows, and "whether the gateway would accept what pns posts" tells them
    /// whether to care.
    Section {
        title: &'static str,
        blurb: &'static str,
    },
    /// A line of the report.
    Row { mark: Mark, text: String },
}

impl Item {
    /// A row.
    pub fn row(mark: Mark, text: impl Into<String>) -> Self {
        Self::Row {
            mark,
            text: text.into(),
        }
    }

    /// A row that is a reading rather than a grade.
    pub fn note(text: impl Into<String>) -> Self {
        Self::row(Mark::Note, text)
    }

    /// A heading.
    pub fn section(title: &'static str, blurb: &'static str) -> Self {
        Self::Section { title, blurb }
    }

    /// The words this item carries, with no mark and no heading rule.
    ///
    /// FOR A READER THAT HAS NO TERMINAL: a test asserting what the report says
    /// rather than how it looks, and any future caller that wants the report as
    /// text. The presentation lives with the command that prints it.
    pub fn text(&self) -> &str {
        match self {
            Self::Section { title, .. } => title,
            Self::Row { text, .. } => text,
        }
    }
}
