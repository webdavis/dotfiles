//! The recap engine: one window of activity, gathered once and then rendered
//! as a page, as a delivered message, or as a machine-readable document.
//!
//! GATHERED ONCE, RENDERED THREE WAYS. A source command is a process and a
//! review note is a file read, so running them per output form would mean the
//! page and the document a consumer diffs against it could disagree. `assemble`
//! is the only thing that touches the world; everything after it is pure.

use crate::{ActivityEvents, DeadLetteredLegs, ReviewNoteSource, SourceCommands, Summarizer};
use pns_domain::recap::{
    Recap,
    activity::{Event, Project, by_project},
    budget::Clock,
    external::{External, Sourcing},
    prompt::{note_prompt, prompt},
    sections::{Heading, Open, Page, Timeline},
};
use std::time::Duration;
mod document;
mod open;
mod sources;
pub use document::document;

/// The world this engine reads, named once.
pub struct BuildRecap<'ports, A, C, N, S, F> {
    pub activity: &'ports A,
    pub commands: &'ports C,
    pub notes: &'ports N,
    pub summarizer: &'ports S,
    pub failures: &'ports F,
}

/// One recap the caller asked for: which window, which sections, and how much
/// of each.
pub struct Request<'request> {
    pub recap: &'request Recap,
    /// The window's name, or None for the span the return moment was away
    /// for.
    pub window: Option<String>,
    pub previous: bool,
    pub since: u64,
    pub until: u64,
    /// The sections the caller named, or empty for every configured one.
    pub sections: Vec<String>,
    pub verbose: bool,
    /// `--limit`, overriding `rows_per_section` for this run.
    pub limit: Option<usize>,
    /// Whether this run has a window at all. `pns recap open` does not, so
    /// the windowed sections are never gathered and the source commands are
    /// run with no `{since}` to substitute.
    pub windowed: bool,
}

/// Everything one recap holds, owned, before anything renders it.
pub struct Assembled {
    pub(crate) window: Option<String>,
    pub(crate) previous: bool,
    pub(crate) since: u64,
    pub(crate) until: u64,
    from: String,
    to: String,
    events: Vec<Event>,
    pub(crate) projects: Vec<Project>,
    pub(crate) sources: Vec<Gathered>,
    pub(crate) open: Open,
    rows: usize,
    verbose: bool,
    pub(crate) sections: Vec<String>,
    /// Where the operator was last, for the windowless form's own header.
    where_last: Option<String>,
    /// The summarizer's answer, or None when none was configured. `Some(None)`
    /// is a summarizer that was asked and said nothing usable.
    answered: Option<Option<Vec<String>>>,
}

impl<A: ActivityEvents, C: SourceCommands, N: ReviewNoteSource, S: Summarizer, F: DeadLetteredLegs>
    BuildRecap<'_, A, C, N, S, F>
{
    /// Run every source this request needs, once.
    ///
    /// THE WINDOWED SOURCES ARE SKIPPED ENTIRELY when the request has no
    /// window, which is what makes `pns recap open` cheap: it is the one form
    /// an unlock automation runs, and it must not wait on a task tool.
    ///
    /// ONE EPISODE, ONE BUDGET. `summarizer_deadline` is what the WHOLE
    /// episode may spend, so each call is bounded by what is left of it.
    /// Per-call deadlines meant a 240-second key could hold two processes for
    /// twelve minutes while the card had already said the recap was in #pns.
    pub fn assemble<B: FnMut() -> Duration>(
        &self,
        request: &Request,
        wall_clock: impl Fn(Option<u64>) -> String,
        episode: impl FnOnce(Duration) -> B,
    ) -> Assembled {
        let recap = request.recap;
        let windowed = request.windowed;
        // THE STORE IS READ WHATEVER THE FORM IS. `open` has no window, but
        // it still needs the sessions: its bounds are the retention the store
        // keeps, so a session blocked three days ago is still on the list.
        let events = self.activity.activity_between(request.since, request.until);
        // A WINDOWLESS RUN GATHERS NO LIST SECTION. `pns recap open` prints
        // one section, and its two commands are run without bounds by
        // `open::gather`; running the windowed ones as well would spawn four
        // processes for a page that shows none of them.
        let mut sources: Vec<Gathered> = Vec::new();
        if windowed {
            for (name, command) in recap.sources.each() {
                if !wanted(&request.sections, name) {
                    continue;
                }
                sources.push(Gathered {
                    name,
                    sourcing: match command {
                        None => Sourcing::Unconfigured,
                        Some(argv) => {
                            self.commands
                                .run(argv, Some(request.since), Some(request.until))
                        }
                    },
                    answered: None,
                });
            }
            if wanted(&request.sections, REVIEW_NOTES) {
                sources.push(Gathered {
                    name: REVIEW_NOTES,
                    sourcing: sources::notes(self.notes, recap, request.since, request.until),
                    answered: None,
                });
            }
        }
        let open = match wanted(&request.sections, OPEN) {
            true => open::gather(self.commands, self.failures, recap, &events),
            false => Open::default(),
        };
        // THE ANSWER IS TAKEN BEFORE THE BODY IS COMPOSED and nothing else
        // waits on it: this may run in a process nobody is standing over.
        // AND NOT OVER AN EMPTY WINDOW. A window with nothing in it has
        // nothing to select from, and the model would be handed "nothing was
        // recorded in this window" under an instruction to rewrite it as a
        // timeline, which is a process spawned to summarize nothing and an
        // invitation to invent.
        let mut left = episode(recap.summarizer_deadline);
        let answered = recap
            .summarizer
            .as_deref()
            .filter(|_| !events.is_empty())
            .map(|argv| {
                self.summarizer
                    .summarize(argv, left(), &prompt(&events, &|at| wall_clock(at)))
            });
        // ONE SUMMARIZER CALL PER SECTION, and each falls back on its own.
        // They are different questions over different sets of text, so one
        // call answering both would need the backend to keep them apart.
        for gathered in &mut sources {
            if gathered.name == REVIEW_NOTES {
                gathered.answered = sources::summarize_notes(
                    self.summarizer,
                    recap,
                    &mut left,
                    &gathered.sourcing,
                    note_prompt,
                );
            }
        }
        let where_last = (!windowed).then(|| open::where_last(&events));
        Assembled {
            projects: by_project(&events),
            from: wall_clock(Some(request.since)),
            to: wall_clock(Some(request.until)),
            window: request.window.clone(),
            previous: request.previous,
            since: request.since,
            until: request.until,
            events,
            sources,
            open,
            rows: request.limit.unwrap_or(recap.rows_per_section),
            verbose: request.verbose,
            sections: request.sections.clone(),
            where_last,
            answered,
        }
    }
}

impl Assembled {
    /// Every list section as the renderer reads it, with whatever the
    /// summarizer said over each.
    pub fn externals(&self) -> Vec<(&str, External<'_>)> {
        self.sources
            .iter()
            .map(|gathered| {
                (
                    gathered.name,
                    gathered.sourcing.external(gathered.answered.as_deref()),
                )
            })
            .collect()
    }

    /// The page this recap composes, over the sections `externals` holds.
    pub fn page<'page>(
        &'page self,
        externals: &'page [(&'page str, External<'page>)],
        clock: Clock<'page>,
    ) -> Page<'page> {
        Page {
            heading: match (&self.window, &self.where_last) {
                (_, Some(line)) => Heading::Where(line),
                (Some(name), None) => Heading::Named(name),
                (None, None) => Heading::WhileYouWereAway,
            },
            from: &self.from,
            to: &self.to,
            counted: self.events.len(),
            projects: &self.projects,
            shows_agents: self.shows_agents(),
            shows_open: self.shows_open(),
            timeline: match &self.answered {
                None => Timeline::Mechanical,
                Some(None) => Timeline::Unanswered,
                Some(Some(lines)) => Timeline::Summarized(lines),
            },
            sources: externals,
            open: &self.open,
            rows_per_section: self.rows,
            verbose: self.verbose,
            clock,
        }
    }

    /// Whether the agents section was asked for at all, which is what
    /// `pns recap open` turns off.
    pub fn shows_agents(&self) -> bool {
        wanted(&self.sections, AGENTS)
    }
    pub fn shows_open(&self) -> bool {
        wanted(&self.sections, OPEN)
    }
}

/// One list section as it was gathered: what it is called, what its source
/// held, and what a summarizer said about it.
pub(crate) struct Gathered {
    pub(crate) name: &'static str,
    pub(crate) sourcing: Sourcing,
    answered: Option<Vec<String>>,
}

/// Whether one section was asked for. AN EMPTY LIST IS EVERY SECTION, which
/// is what a caller that named none means.
pub(crate) fn wanted(sections: &[String], name: &str) -> bool {
    sections.is_empty() || sections.iter().any(|named| named == name)
}

/// The two sections whose source is not `[recap.sources]`.
pub const AGENTS: &str = "agents";
pub const REVIEW_NOTES: &str = "review_notes";
pub const OPEN: &str = "open";

mod window;
pub use window::{LocalCivilTime, RECAP_USAGE, recap_bounds, recap_wall_clock};

#[cfg(test)]
mod tests;
