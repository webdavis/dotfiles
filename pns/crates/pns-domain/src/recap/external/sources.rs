use super::super::sanitize::safe_line;
use super::EXTERNAL_TEXT_CHARS;

/// One thing an external source said, in the three shapes the recap needs it:
/// the receipt a line must carry to claim it, the line pns writes with no model
/// at all, and the text a model is shown.
///
/// EVERY FIELD IS ALREADY FLATTENED AND CAPPED, by the constructor rather than
/// by the caller. A pull request body and a review note are both somebody
/// else's text, arriving in a message pns signs its name to; making the
/// composition root responsible for cleaning them would put that duty on the
/// one layer that also holds the IO, and a caller that forgot would leak raw
/// bytes into a Discord message and into a model's prompt at once.
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced {
    /// What a line has to name to be tied back to this source: a pull
    /// request's `#213`, a note's own file name.
    pub cite: String,
    /// What pns says about this source with no model anywhere: already a
    /// finished line, cite first.
    pub line: String,
    /// What a summarizer is shown about this source.
    pub source: String,
}

/// One merged pull request as this recap will speak about it.
///
/// WHAT IT DOES NOW COMES OUT OF THE BODY'S OWN SUMMARY, which is the section
/// the author wrote to answer exactly that question, and the title is the
/// fallback for a body that has no such heading. Neither is trusted further
/// than a line: both go through `safe_line`, so a body that spans paragraphs,
/// carries terminal control bytes or hides a reordering character arrives as
/// one line of visible text.
///
/// THE NUMBER LEADS, because it is the receipt. Every line the operator reads
/// here names the pull request it came from, so the tail pointer is followable
/// per line rather than per message.
pub fn merged(number: u64, title: &str, body: &str) -> Sourced {
    let summary = summary_of(body);
    let said = safe_line(
        if summary.trim().is_empty() {
            title
        } else {
            &summary
        },
        SOURCE_MAX_CHARS,
    );
    let cite = format!("#{number}");
    Sourced {
        line: crate::render::clipped(&format!("{cite} {said}"), EXTERNAL_TEXT_CHARS),
        source: said,
        cite,
    }
}
/// One review note as this recap will speak about it: the file's own name is
/// the receipt, its first heading is what pns can say about it with no model,
/// and its text is what a model is shown.
///
/// THE NAME IS CAPPED LIKE ANY OTHER TEXT, because it comes off a directory an
/// operator's other tools also write into, and it goes into a line and into a
/// prompt. It is capped ONCE, here, so the token a line has to carry and the
/// token the model was shown cannot differ.
pub fn noted(name: &str, contents: &str) -> Sourced {
    let cite = safe_line(name, CITE_MAX_CHARS);
    let heading = safe_line(&first_heading(contents), SOURCE_MAX_CHARS);
    let line = if heading.is_empty() {
        cite.clone()
    } else {
        format!("{cite}: {heading}")
    };
    Sourced {
        line: crate::render::clipped(&line, EXTERNAL_TEXT_CHARS),
        source: safe_line(contents, NOTE_SOURCE_CHARS),
        cite,
    }
}
/// One review note that matched the glob and the window and could not be read.
///
/// SAID RATHER THAN DROPPED, which is `Found`'s own three-states rule one level
/// down. A file the operator's own pattern named and whose clock puts it in the
/// window is a finding somebody wrote; silently leaving it out renders a night
/// in which it never existed, and the mode, the race or the device entry that
/// stopped the read is exactly what they would want to see.
pub fn unreadable(name: &str) -> Sourced {
    let cite = safe_line(name, CITE_MAX_CHARS);
    Sourced {
        line: crate::render::clipped(&format!("{cite}: {UNREADABLE}"), EXTERNAL_TEXT_CHARS),
        source: UNREADABLE.to_string(),
        cite,
    }
}
/// What a note that would not open says, in its line and in the prompt alike:
/// one sentence, so a model shown it can only say the same thing back.
pub(super) const UNREADABLE: &str = "could not be read";

/// The paragraph under a pull request body's own Summary heading, or nothing
/// when it has none.
///
/// BY HEADING AND NOT BY POSITION, because a body opens with whatever its
/// author or template put first. Everything up to the NEXT heading is taken,
/// and `safe_line` makes one line of it: a summary somebody wrote as three
/// sentences is still what the section wants to say, cut to a line's width by
/// the same rule every other line here is.
pub(super) fn summary_of(body: &str) -> String {
    body.lines()
        .skip_while(|line| !summary_heading(line))
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
}
pub(super) fn summary_heading(line: &str) -> bool {
    let text = line.trim_start();
    text.starts_with('#')
        && text
            .trim_start_matches('#')
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("summary")
}
/// A note's own first heading, which is the one thing a mechanical line can
/// say about a report written for a person.
pub(super) fn first_heading(contents: &str) -> String {
    contents
        .lines()
        .find(|line| line.trim_start().starts_with('#'))
        .map(|line| line.trim_start().trim_start_matches('#').trim().to_string())
        .unwrap_or_default()
}

/// How much of a source a summarizer is shown.
///
/// A PULL REQUEST'S SUMMARY IS A PARAGRAPH and a review note is a report, so
/// the two are capped differently: enough of the first to say what shipped,
/// enough of the second to hold the findings, and a hard ceiling on both
/// because the prompt is built out of somebody else's text.
pub(super) const SOURCE_MAX_CHARS: usize = 400;
pub(super) const NOTE_SOURCE_CHARS: usize = 1_200;
/// How long a receipt itself may be. A pull request number is short by
/// construction; a file name comes off a directory other tools write into.
pub(super) const CITE_MAX_CHARS: usize = 60;
