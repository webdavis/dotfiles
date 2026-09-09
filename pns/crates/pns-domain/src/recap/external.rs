//! The summarized sections, and what a line must cite to survive.

use super::budget::Trim;
use super::prompt::{SUMMARIZER_SILENT, Voice};
use super::sections::Section;

mod sources;
pub use sources::{Sourced, merged, noted, unreadable};

/// What an external section found, which is three different claims about the
/// night and never one.
///
/// A SECTION NEVER VANISHES, whichever of these it is. "Nobody told pns where
/// to look", "the place pns was told to look would not answer" and "the place
/// answered, and nothing was there" are three sentences, and collapsing any
/// two of them would let a broken source read as a quiet night.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Found<'sources> {
    /// No key names this source, so nothing was read and nothing was run.
    #[default]
    Unconfigured,
    /// A key names it and the read did not come back. See the composition
    /// root: a missing tool, a refusal and a deadline are one outcome here.
    Unavailable,
    /// What the source held. EMPTY IS AN ANSWER, not an absence.
    Read(&'sources [Sourced]),
}
/// One external section's whole input: what pns found, and what a summarizer
/// said about it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct External<'sources> {
    pub found: Found<'sources>,
    /// The summarizer's lines, already through `answer`. None is every way of
    /// not having one, and the mechanical lines stand.
    pub answered: Option<&'sources [String]>,
    /// Whether a cap stopped the fetch short, so what `found` holds is a
    /// FLOOR. The remainder line then says "at least", because pns stopped
    /// reading and cannot say how many more there were: a listing cut at its
    /// own limit and a glob matching more files than one recap considers are
    /// both counts that would otherwise read as totals.
    pub truncated: bool,
}
/// The two sections whose source is not pns, named rather than passed as a
/// pair.
///
/// ONE NAMED VALUE, for `config::Recap`'s reason: both halves have the same
/// type, so as two arguments they would sit adjacent and transposable, and a
/// swap would file the night's merges under the review notes with nothing to
/// catch it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Externals<'sources> {
    pub merges: External<'sources>,
    pub notes: External<'sources>,
}
/// One external section, in whichever of its four states it is in.
///
/// RECEIPTS OR CUT, AND IT IS A CHECK RATHER THAN A SENTENCE IN A PROMPT. pns
/// holds the receipts it read, so a summarized line is kept only when a source
/// it actually fetched VOUCHES for it: a line citing a pull request nobody
/// fetched, and a line citing nothing at all, are both text a model produced
/// and nobody can follow, and they are dropped whatever else they say.
///
/// THE ANSWER IS JUDGED AS IT CAME, BEFORE ANY CLIP. Cutting a line to the
/// section's width can turn `#2130` into `#213` and an ellipsis, which reads as
/// a receipt this section holds; filtering first and clipping only what
/// survives means the width can never decide what is true.
///
/// ONE SOURCE VOUCHES FOR AT MOST ONE LINE. Membership alone let four lines
/// citing the same merge stand for four merges, so nine of ten fetched sources
/// went unmentioned under a message saying six were missing. A source is spent
/// by the first surviving line that names it, and a later line with no unspent
/// receipt left is dropped exactly as an uncited one is.
///
/// AND WHAT IS MISSING IS COUNTED BY SOURCE, never by subtracting lines: the
/// remainder is the number of fetched sources that NO surviving line names, so
/// however few lines the model wrote, the count is about the things pns read.
/// `at_least` is what it says when a cap stopped the fetch itself, because past
/// that point pns cannot know how many more there were.
///
/// AN ANSWER THAT SURVIVES NOTHING FALLS TO THE MECHANICAL LINES, which is
/// `Timeline::Unanswered`'s rung applied here: pns already holds a finished line
/// per source, so a backend that ignored "start every line with the number"
/// must not cost the section text that needed no model at all. The heading says
/// which of the two lists this is, exactly as the night's does.
///
/// CUT LAST AND ONLY WHEN SOMETHING HAS TO BE. `fit` hands its room to the
/// night in document order, so a section always trimmable here would be starved
/// by a long night on exactly the loud window it exists for; a section never
/// trimmable pushed a loud window past one Discord message, MEASURED at six
/// waiting items where the same body without these two sections took twelve.
/// `Trim::WhenOver` is both: the whole section on an ordinary window, and a
/// heading plus a truthful remainder on the window that would not otherwise
/// fit.
pub(super) fn external_section(voice: &Voice, external: &External) -> Section {
    let sources = match external.found {
        Found::Unconfigured => return Section::held(vec![voice.unconfigured.to_string()]),
        Found::Unavailable => return Section::held(vec![voice.unavailable.to_string()]),
        Found::Read([]) => return Section::held(vec![voice.nothing.to_string()]),
        Found::Read(sources) => sources,
    };
    let mechanical = || -> Vec<&str> {
        sources
            .iter()
            .take(MAX_EXTERNAL_LINES)
            .map(|source| source.line.as_str())
            .collect()
    };
    let (heading, kept) = match external.answered.map(|answered| vouched(sources, answered)) {
        None => (voice.heading.to_string(), mechanical()),
        Some(kept) if kept.is_empty() => (
            format!("{} {SUMMARIZER_SILENT}", voice.heading),
            mechanical(),
        ),
        Some(kept) => (voice.heading.to_string(), kept),
    };
    let mut lines = vec![heading];
    lines.extend(kept.iter().map(|line| {
        format!(
            "{LINE_PREFIX}{}",
            crate::render::clipped(line, EXTERNAL_TEXT_CHARS)
        )
    }));
    Section {
        lines,
        trim: Trim::WhenOver,
        omitted: sources
            .iter()
            .filter(|source| !kept.iter().any(|line| cites(line, &source.cite)))
            .count(),
        at_least: external.truncated,
    }
}
/// The answer's lines that the fetched sources vouch for, ONE SOURCE EACH and
/// at most as many as the section may spend.
pub(super) fn vouched<'answer>(
    sources: &[Sourced],
    answered: &'answer [String],
) -> Vec<&'answer str> {
    let mut spent = vec![false; sources.len()];
    let mut kept = Vec::new();
    for line in answered {
        if kept.len() == MAX_EXTERNAL_LINES {
            break;
        }
        if let Some(which) =
            (0..sources.len()).find(|which| !spent[*which] && cites(line, &sources[*which].cite))
        {
            spent[which] = true;
            kept.push(line.as_str());
        }
    }
    kept
}
/// Whether one line carries one receipt AS A WHOLE TOKEN, bracketed at BOTH
/// ends. A receipt is the thing an operator follows, so anything glued to
/// either end of it names something else: `#2130` is not `#213`, and neither
/// `checklist-s17.md.bak` nor `old-checklist-s17.md` is `checklist-s17.md`.
pub(super) fn cites(line: &str, cite: &str) -> bool {
    line.match_indices(cite).any(|(at, _)| {
        !glued(line[..at].chars().next_back()) && !glued(line[at + cite.len()..].chars().next())
    })
}
/// Whether a character extends a receipt rather than ending it. Alphanumerics
/// carry on a number or a word, and `.`, `-` and `_` are what a file name is
/// built out of, so a receipt with one of them on either side is part of a
/// longer name.
pub(super) fn glued(character: Option<char>) -> bool {
    character.is_some_and(|character| {
        character.is_alphanumeric() || matches!(character, '.' | '-' | '_')
    })
}
/// How many lines either external section may spend.
///
/// FOUR, and it is arithmetic rather than taste. Both sections are PROTECTED,
/// so every line here is reserved out of the twenty-five before the night gets
/// any: at four lines each plus a heading and a remainder the pair costs at
/// most twelve, which leaves the header, NEEDS YOU, the tail and a night of
/// nine. A night is the section that absorbs a cut, and these two are the ones
/// the operator asked to be told about, so this is the direction the budget
/// leans on purpose.
pub(super) const MAX_EXTERNAL_LINES: usize = 4;
/// How wide one RENDERED line of either external section may be.
///
/// EIGHTY-EIGHT, and it is the same arithmetic. `fit` reserves these characters
/// before working out what a timeline line may spend, so twelve reserved lines
/// at this width cost around 1,050 of the 1,800 the message has. It is wider
/// than a timeline line because a timeline line already spends its head on a
/// clock and a marker.
///
/// THE PREFIX IS INSIDE IT, which is what makes this a width rather than a
/// claim: the text is cut to what is left after `- `, so the line the operator
/// reads is this many characters and the reservation `fit` counted is the one
/// the message actually spends.
pub(super) const EXTERNAL_MAX_CHARS: usize = 88;
/// What the text of an external line may spend, once its prefix is paid for.
pub(super) const EXTERNAL_TEXT_CHARS: usize = EXTERNAL_MAX_CHARS - LINE_PREFIX.len();
/// What marks a line as content rather than a heading, wherever this module
/// writes somebody else's words: two characters that cost the width and make a
/// forged heading impossible.
pub(super) const LINE_PREFIX: &str = "- ";
