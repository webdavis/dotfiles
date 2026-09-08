use crate::{ActivityRing, MergedPullRequestSource, ReviewNoteSource, Summarizer};
use pns_domain::recap::{
    Recap,
    external::{External, Externals, Sourced},
    prompt::{merge_prompt, note_prompt, prompt},
    sections::{Timeline, body},
};
use std::time::Duration;
mod sources;
use sources::{found, read_sources, truncated};

pub struct BuildReturnRecap<'a, A, M, N, S> {
    pub activity: &'a A,
    pub merges: &'a M,
    pub notes: &'a N,
    pub summarizer: &'a S,
}

impl<A: ActivityRing, M: MergedPullRequestSource, N: ReviewNoteSource, S: Summarizer>
    BuildReturnRecap<'_, A, M, N, S>
{
    pub fn run<B: FnMut() -> Duration>(
        &self,
        recap: &Recap,
        since: u64,
        until: u64,
        wall_clock: impl Fn(Option<u64>) -> String,
        episode: impl FnOnce(Duration) -> B,
    ) -> String {
        let entries = self.activity.entries_between(since, until);
        // THE TWO EXTERNAL SOURCES ARE READ ONLY WHEN A KEY NAMES THEM, and both
        // are read HERE, in the process nobody is waiting on. A repository listing
        // is a network call somebody else's machine answers and a glob is a
        // directory read; neither belongs anywhere near the card, and neither is
        // allowed to cost the rest of the recap anything when it does not come
        // back.
        let fetched_merges =
            (!recap.repos.is_empty()).then(|| self.merges.merged(&recap.repos, since, until));
        let fetched_notes = recap
            .review_notes
            .as_deref()
            .map(|pattern| self.notes.notes(pattern, since, until));
        // ONE EPISODE, ONE BUDGET. The locked "the LLM runs once at the return
        // moment" is a moment rather than a call: this recap asks up to three
        // questions (the night, the merges, the notes) and `summarizer_deadline_secs`
        // is what the WHOLE episode may spend, so each call is bounded by what is
        // left of it. Per-call deadlines meant a 240-second key could hold two
        // processes for twelve minutes while the card had already said the recap
        // was in #pns, and a laptop that sleeps inside that window loses the recap
        // entirely. Adjudicated 2026-08-29.
        let mut left = episode(Duration::from_secs(recap.summarizer_deadline_secs));
        // THE ANSWER IS TAKEN BEFORE THE BODY IS COMPOSED and nothing else waits on
        // it: this process was started so that a model could be slow somewhere
        // nobody is standing.
        // AND NOT OVER AN EMPTY WINDOW. A night with nothing in it has nothing to
        // select from, and the model would be handed "nothing was recorded in this
        // window" under an instruction to rewrite it as a timeline. That is a
        // process spawned to summarize nothing and an invitation to invent, on the
        // one path an operator reaches by hand.
        let answered = recap
            .summarizer
            .as_deref()
            .filter(|_| !entries.is_empty())
            .map(|argv| {
                self.summarizer
                    .summarize(argv, left(), &prompt(&entries, &|at| wall_clock(at)))
            });
        let timeline = match &answered {
            None => Timeline::Mechanical,
            Some(None) => Timeline::Unanswered,
            Some(Some(lines)) => Timeline::Summarized(lines),
        };
        // ONE SUMMARIZER CALL PER SECTION, and each falls back on its own. They are
        // three different questions over three different sets of text, so one call
        // answering all three would need the backend to keep them apart in its
        // answer, and a section would then be lost to a separator a model got wrong
        // rather than to anything pns could see. THEY SHARE ONE DEADLINE, above: a
        // call reached with the episode's budget already spent is never started.
        let merge_lines = read_sources(&fetched_merges).and_then(|sources| {
            summarized(self.summarizer, recap, &mut left, sources, merge_prompt)
        });
        let note_lines = read_sources(&fetched_notes).and_then(|sources| {
            summarized(self.summarizer, recap, &mut left, sources, note_prompt)
        });
        let externals = Externals {
            merges: External {
                found: found(&fetched_merges),
                answered: merge_lines.as_deref(),
                truncated: truncated(&fetched_merges),
            },
            notes: External {
                found: found(&fetched_notes),
                answered: note_lines.as_deref(),
                truncated: truncated(&fetched_notes),
            },
        };
        body(
            &entries,
            &wall_clock(Some(since)),
            &wall_clock(Some(until)),
            &|at| wall_clock(at),
            timeline,
            &externals,
        )
    }
}

/// What the summarizer said about one external section, or None for every way
/// of not having an answer.
///
/// NOT OVER AN EMPTY SOURCE, which is `recap_mode`'s own rule about an empty
/// window applied a second time: a model handed nothing to select from is a
/// process spawned to summarize nothing and an invitation to invent.
fn summarized(
    summarizer: &impl Summarizer,
    recap: &Recap,
    left: &mut impl FnMut() -> Duration,
    sources: &[Sourced],
    prompt: fn(&[Sourced]) -> String,
) -> Option<Vec<String>> {
    summarizer.summarize(recap.summarizer.as_deref()?, left(), &prompt(sources))
}

mod window;
pub use window::{RECAP_USAGE, recap_bounds, recap_wall_clock};

#[cfg(test)]
mod tests;
