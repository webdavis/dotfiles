//! What a recap body is made of, and the order the sections run in.

use super::activity::Project;
use super::budget::Trim;
use super::budget::{Clock, MAX_LINES, fit};
use super::external::External;
use super::external::LINE_PREFIX;
use super::external::external_section;
use super::night::agents_section;
use super::prompt::voice;

/// One part of the body, and whether the budget may cut it.
///
/// THE HEADING IS THE FIRST LINE, which is what makes trimming a tail slice:
/// a cut section keeps its heading, as many lines as fit, and one line naming
/// the true remainder.
#[derive(Debug, PartialEq)]
pub struct Section {
    pub lines: Vec<String>,
    pub trim: Trim,
    /// What this section left out BEFORE the budget ever saw it: sources no
    /// surviving line speaks for. `fit` adds whatever it cuts itself and
    /// renders one remainder line for the sum, so a section cannot end up
    /// carrying two counts that disagree.
    pub omitted: usize,
    /// Whether `omitted` is a FLOOR rather than a total, which is what a cap
    /// on the fetch leaves behind: pns stopped reading, so it cannot say how
    /// many more there were.
    pub at_least: bool,
}
impl Section {
    /// A section the budget may not cut, with nothing left out of it.
    pub(super) fn held(lines: Vec<String>) -> Section {
        Section {
            lines,
            trim: Trim::Never,
            omitted: 0,
            at_least: false,
        }
    }
}
/// What the agents section is made of, and the ONLY thing a summarizer can
/// change.
///
/// THE SUBSTITUTION POINT IS A TYPE rather than a rule in a prompt, which is
/// what makes SELECTION-NOT-RECONSTRUCTION structural: the header's count, what
/// is open, and every other section are composed the same way whichever variant
/// this is, so a model that answered with a different count or with nothing
/// urgent in it cannot move either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Timeline<'lines> {
    /// One line per session, composed here. The setting of a machine with no
    /// summarizer configured, and the floor every other outcome falls to.
    Mechanical,
    /// What the summarizer said, already flattened and capped by `answer`.
    Summarized(&'lines [String]),
    /// A summarizer was configured and did not answer. The mechanical lines,
    /// and one line saying that is what they are.
    Unanswered,
}

/// Which window the header names, and therefore what its first words are.
///
/// TWO SPELLINGS, ONE HEADER. A window the operator typed is named
/// (`morning 06:00-12:00`); the return moment's window has no name, only the
/// span the operator was away for, and its header is the sentence the phone
/// card and the forum thread title have always carried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Heading<'name> {
    Named(&'name str),
    WhileYouWereAway,
    /// `pns recap open`, which has no window and therefore no count: the
    /// header is where the operator was instead, which is the question that
    /// form exists to answer.
    Where(&'name str),
}

/// What is still waiting on a person, from the three places that know.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Open {
    pub sessions: Vec<String>,
    pub pull_requests: Vec<String>,
    pub applies: Vec<String>,
    /// The standing population of delivery legs the retry policy gave up on.
    /// The watchdog pages on growth, so the number itself is read here.
    pub dead_lettered: usize,
}

impl Open {
    fn is_empty(&self) -> bool {
        self.sessions.is_empty()
            && self.pull_requests.is_empty()
            && self.applies.is_empty()
            && self.dead_lettered == 0
    }
    fn lines(&self) -> Vec<String> {
        [&self.sessions, &self.pull_requests, &self.applies]
            .into_iter()
            .flatten()
            .map(|item| format!("{LINE_PREFIX}{item}"))
            .chain(dead_letter_line(self.dead_lettered).map(|line| format!("{LINE_PREFIX}{line}")))
            .collect()
    }
}

/// One line for the dead-lettered legs, and nothing at all when there are
/// none: a zero said out loud is a line the operator reads past every day.
fn dead_letter_line(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 leg dead-lettered, run pns failures".to_string()),
        many => Some(format!("{many} legs dead-lettered, run pns failures")),
    }
}

/// One recap's whole input, named rather than passed as a row of arguments.
///
/// ONE NAMED VALUE, for `Recap`'s reason: five of these fields are strings or
/// counts, and adjacent in a call they would be transposable with nothing to
/// catch a swap.
pub struct Page<'page> {
    pub heading: Heading<'page>,
    pub from: &'page str,
    pub to: &'page str,
    pub counted: usize,
    pub projects: &'page [Project],
    /// Whether the agents section was asked for at all. `pns recap open` and
    /// a `--section` that names other sections turn it off, and OFF MEANS
    /// ABSENT rather than an empty section: "nothing was recorded" under a
    /// heading nobody asked for is a claim about a window this page is not
    /// reporting on.
    pub shows_agents: bool,
    pub timeline: Timeline<'page>,
    /// The five list sections, in the order the page prints them, each named
    /// by its config key so the heading and the document field cannot drift.
    pub sources: &'page [(&'page str, External<'page>)],
    pub open: &'page Open,
    /// Whether the `open` section was asked for. It is never OMITTED for
    /// being empty and never shed by the budget; a `--section` naming other
    /// sections is the one thing that leaves it out.
    pub shows_open: bool,
    pub rows_per_section: usize,
    pub verbose: bool,
    pub clock: Clock<'page>,
}

/// The body, in order: the window header, what the agents did, the configured
/// list sections, and what is open.
///
/// `open` IS LAST AND IS NEVER OMITTED, which is the design's own ruling and
/// the one the budget already served under another name: an empty "OPEN" is the
/// news the page exists to carry, and it is also the whole page under
/// `pns recap open`.
///
/// A LIST SECTION SAYS WHICH OF ITS STATES IT IS IN rather than being omitted
/// when it has something to say. A section that vanished would be
/// indistinguishable from a window with nothing in it, which is a different
/// claim, and so is a command that would not answer. An EMPTY one is omitted
/// unless `-v` asks for it, because a page of five "nothing" lines buries the
/// two sections that had news.
impl Page<'_> {
    /// The rows the `pull_requests` command gave this page, which is where the
    /// agents section looks for a session's pull request. Empty when the
    /// command is not configured or did not answer.
    pub(super) fn pull_request_rows(&self) -> Vec<String> {
        self.sources
            .iter()
            .filter(|(name, _)| *name == PULL_REQUESTS)
            .flat_map(|(_, external)| match external.found {
                super::external::Found::Read(sources) => sources
                    .iter()
                    .map(|source| source.line.clone())
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect()
    }
}

/// The section whose rows name a branch's pull request, which is the one
/// section another section reads.
pub const PULL_REQUESTS: &str = "pull_requests";

pub fn sections(page: &Page) -> Vec<Section> {
    let mut parts = vec![Section::held(vec![header(
        page.heading,
        page.counted,
        page.from,
        page.to,
    )])];
    if page.shows_agents {
        parts.push(agents_section(page));
    }
    parts.extend(
        page.sources
            .iter()
            // A SOURCE NOBODY CONFIGURED IS ABSENT ENTIRELY, from the page,
            // from the document and from `--section`'s accepted names. It is
            // not a state of the section; it is the section not existing.
            .filter(|(_, external)| external.found != super::external::Found::Unconfigured)
            .filter_map(|(name, external)| {
                // ONLY THE "NOTHING IN THIS WINDOW" STATE IS OMITTED. A
                // command that would not run and one that exited non-zero are
                // both one line long too, and both are news.
                let empty = external.found == super::external::Found::Read(&[]);
                (page.verbose || !empty)
                    .then(|| external_section(&voice(name), external, page.rows_per_section))
            }),
    );
    if page.shows_open {
        parts.push(open_section(page.open));
    }
    parts
}
/// The whole body, fitted to the delivery budget and joined into one message.
///
/// THE BUDGET IS DELIVERY'S, NOT THE TERMINAL'S. A page printed to a terminal
/// scrolls; a page posted to Discord is one message with a character ceiling,
/// so `fit` runs on the way out rather than on the way in.
pub fn body(page: &Page) -> String {
    fit(&sections(page), MAX_LINES).join("\n")
}
/// The first line, which is also the thread's title when the route is a forum
/// channel: hermes names a new forum thread after the message's first line.
///
/// THE COUNT IS THE EVENTS THAT WERE READ, never the ones that survived the
/// budget and never a claim about everything that happened. The store prunes
/// to `[recap] retain`, so over a very long absence this is a floor rather
/// than a total.
///
/// AND IT IS THE CARD'S OWN SENTENCE, from `event_count`, so the two layers of
/// one return cannot pluralize the same number two ways.
pub(super) fn header(heading: Heading, counted: usize, from: &str, to: &str) -> String {
    let opening = match heading {
        Heading::Named(name) => format!("{name} {from}-{to}"),
        Heading::WhileYouWereAway => format!("While you were away, {from}-{to}"),
        Heading::Where(line) => return line.to_string(),
    };
    format!("{opening} · {}", crate::missed::event_count(counted))
}
/// What is still waiting on a person, and never cut.
pub(super) fn open_section(open: &Open) -> Section {
    let mut lines = vec![OPEN_HEADING.to_string()];
    if open.is_empty() {
        lines.push(NOTHING_OPEN.to_string());
    } else {
        lines.extend(open.lines());
    }
    Section::held(lines)
}
pub(super) const OPEN_HEADING: &str = "OPEN";
/// Said rather than left blank: an empty section reads as a section that
/// broke, and this one is the reason the page exists.
pub(super) const NOTHING_OPEN: &str = "- nothing is waiting on you";
