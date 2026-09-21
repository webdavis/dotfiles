//! The one source that is not a command: the review notes glob.

use crate::{Fetched, ReviewNoteSource, Summarizer};
use pns_domain::recap::{
    Recap,
    external::{Sourced, Sourcing},
};
use std::time::Duration;

/// The review notes, or the state that says why there are none.
///
/// UNSET IS THE OFF STATEMENT, as it is for every source command: with no
/// pattern the directory is never opened.
pub(super) fn notes(
    source: &impl ReviewNoteSource,
    recap: &Recap,
    since: u64,
    until: u64,
) -> Sourcing {
    let Some(pattern) = recap.review_notes_glob.as_deref() else {
        return Sourcing::Unconfigured;
    };
    match source.notes(pattern, since, until) {
        None => Sourcing::Unavailable,
        Some(Fetched { sources, truncated }) => Sourcing::Read(sources, truncated),
    }
}

/// What the summarizer said about the notes, folded onto the section it is
/// about.
///
/// NOT OVER AN EMPTY SOURCE, which is `assemble`'s own rule about an empty
/// window applied a second time: a model handed nothing to select from is a
/// process spawned to summarize nothing and an invitation to invent.
pub(super) fn summarize_notes(
    summarizer: &impl Summarizer,
    recap: &Recap,
    left: &mut impl FnMut() -> Duration,
    sourcing: &Sourcing,
    prompt: fn(&[Sourced]) -> String,
) -> Option<Vec<String>> {
    let (Some(argv), Sourcing::Read(sources, _)) = (recap.summarizer.as_deref(), sourcing) else {
        return None;
    };
    if sources.is_empty() {
        return None;
    }
    summarizer.summarize(argv, left(), &prompt(sources))
}
