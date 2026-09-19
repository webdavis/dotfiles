//! The page's shape: what to say, with no idea how it will look.
//!
//! Every function here is pure data, text in, a shape out. Painting it, the
//! colors, the rule, the measured width, lives entirely in
//! `morning-adapters`'s style module, so this crate never emits an escape
//! sequence and a narrow terminal never breaks anything the domain produced.

/// One block of the page.
pub struct Section {
    pub title: String,
    pub body: SectionBody,
}

/// What a section has to say. A source that could not be read says so here
/// rather than leaving the page a line shorter.
pub enum SectionBody {
    /// What the source reported, already one line per row.
    Lines(Vec<String>),
    /// Why the source could not be reported.
    Unavailable(String),
}

impl Section {
    pub fn lines(title: &str, lines: Vec<String>) -> Self {
        Self {
            title: title.to_string(),
            body: SectionBody::Lines(lines),
        }
    }

    pub fn unavailable(title: &str, reason: impl Into<String>) -> Self {
        Self {
            title: title.to_string(),
            body: SectionBody::Unavailable(reason.into()),
        }
    }
}

/// One line of the page's content, before any paint.
pub enum Line {
    /// The tool's name, at the top.
    Header(String),
    /// A section's title.
    Heading(String),
    /// One row of a section's content, not yet clipped to any width.
    Row(String),
    /// A section held back this many rows beyond what it printed.
    More(usize),
    /// A section reported nothing.
    Nothing,
    /// A section could not be read, and why.
    Unavailable(String),
}

/// The whole page, as a shape rather than text.
///
/// `rows` is what keeps this a PAGE. A ledger with seventy open operator items
/// answers "where do I start today" worse than the first few do, so a section
/// keeps only its first `rows` rows and says how many it held back.
pub fn render(heading: &str, sections: &[Section], rows: usize) -> Vec<Line> {
    let rows = rows.max(1);
    let mut lines = vec![Line::Header(heading.to_string())];
    for section in sections {
        lines.push(Line::Heading(section.title.clone()));
        match &section.body {
            SectionBody::Lines(items) if items.is_empty() => lines.push(Line::Nothing),
            SectionBody::Lines(items) => {
                lines.extend(items.iter().take(rows).cloned().map(Line::Row));
                if items.len() > rows {
                    lines.push(Line::More(items.len() - rows));
                }
            }
            SectionBody::Unavailable(reason) => lines.push(Line::Unavailable(reason.clone())),
        }
    }
    lines
}

#[cfg(test)]
mod tests;
