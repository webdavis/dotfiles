//! The page itself: one frame, then a block per section, then nothing.
//!
//! The width is fixed rather than measured. A brief is read in a terminal, in
//! a pane and in an agent transcript, and the same bytes in all three is worth
//! more than filling a wide window.

/// How wide the frame is drawn.
const WIDTH: usize = 68;

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

/// The whole page, ending in one newline.
///
/// `rows` is what keeps this a PAGE. A ledger with seventy open operator items
/// answers "where do I start today" worse than the first few do, so a section
/// prints its first `rows` rows and then says how many it held back.
pub fn render(heading: &str, sections: &[Section], rows: usize) -> String {
    let rows = rows.max(1);
    let mut page = frame(heading);
    for section in sections {
        page.push('\n');
        page.push_str(&section.title);
        page.push('\n');
        match &section.body {
            SectionBody::Lines(lines) if lines.is_empty() => page.push_str("  nothing\n"),
            SectionBody::Lines(lines) => {
                for line in lines.iter().take(rows) {
                    page.push_str("  ");
                    page.push_str(&clip(line, WIDTH - 2));
                    page.push('\n');
                }
                if lines.len() > rows {
                    page.push_str(&format!("  ... {} more\n", lines.len() - rows));
                }
            }
            SectionBody::Unavailable(reason) => {
                page.push_str("  unavailable: ");
                page.push_str(reason);
                page.push('\n');
            }
        }
    }
    page
}

/// A row cut to the page's width, so a ledger paragraph does not wrap into a
/// block that hides the row under it.
fn clip(line: &str, width: usize) -> String {
    match line.chars().count() > width {
        true => format!("{}...", line.chars().take(width - 3).collect::<String>()),
        false => line.to_string(),
    }
}

fn frame(heading: &str) -> String {
    let inner = WIDTH - 2;
    let text: String = heading.chars().take(inner - 2).collect();
    let padding = inner - 2 - text.chars().count();
    format!(
        "╭{rule}╮\n│ {text}{space} │\n╰{rule}╯\n",
        rule = "─".repeat(inner),
        space = " ".repeat(padding)
    )
}

#[cfg(test)]
mod tests;
