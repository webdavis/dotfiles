//! The agents section: what ran in the window, grouped by project and then by
//! session.
//!
//! ONE SECTION FOR ANY WINDOW. It was the night's alone when the only caller
//! was the return moment; the window it covers is now whichever one the
//! operator named, and the header above it is what says which.

use super::activity::{Event, Project, Session};
use super::budget::{Clock, Trim};
use super::external::LINE_PREFIX;
use super::external::cites;
use super::prompt::SUMMARIZER_SILENT;
use super::sections::Section;
use super::sections::{Page, Timeline};

/// The window's agents, oldest project first. THE ONLY TRIMMABLE SECTION OF
/// THE FIXED THREE, because it is the one whose length follows the window's,
/// and the only one a summarizer is allowed to write.
///
/// A SUMMARIZED SECTION IS TRIMMABLE TOO, which is the budget re-applied to
/// the model's own answer: a backend that ignored the line count it was asked
/// for is cut here exactly as a loud window is.
///
/// AND EVERY SUMMARIZED LINE IS PREFIXED AND COUNTED, which is what makes the
/// substitution structural rather than hopeful. A line whose whole text is
/// `OPEN` or a second window header is ordinary printable text that `safe_line`
/// has no reason to touch, so it would render AS a heading; a leading `- `
/// costs two characters of the width and makes that impossible, exactly as a
/// mechanical line's own clock does. And an answer longer than the window it
/// summarizes is cut to the window's own event count, because `fit`'s
/// remainder counts this section's lines and COUNT NEVER LIES is the rule this
/// whole module is arranged around.
///
/// THE QUIET SUMMARIZER IS NAMED IN THE HEADING rather than in a section of
/// its own, so the note and the list it describes cannot be separated.
pub(super) fn agents_section(page: &Page) -> Section {
    if page.projects.is_empty() {
        // SAID RATHER THAN LEFT BLANK, for `NOTHING_OPEN`'s own reason: a
        // heading with nothing under it reads as a section that broke. IT
        // ANSWERS EVERY VARIANT, because a window with no events has no work
        // for anybody to have summarized.
        return Section::held(vec![
            AGENTS_HEADING.to_string(),
            NOTHING_HAPPENED.to_string(),
        ]);
    }
    if let Timeline::Summarized(lines) = page.timeline {
        let mut summarized = vec![AGENTS_HEADING.to_string()];
        summarized.extend(
            lines
                .iter()
                .take(page.counted)
                .map(|line| format!("{LINE_PREFIX}{line}")),
        );
        return Section {
            lines: summarized,
            trim: Trim::Always,
            omitted: 0,
            at_least: false,
        };
    }
    let pull_requests = page.pull_request_rows();
    let mut lines = vec![match page.timeline {
        Timeline::Unanswered => format!("{AGENTS_HEADING} {SUMMARIZER_SILENT}"),
        _ => AGENTS_HEADING.to_string(),
    }];
    for project in page.projects {
        lines.push(project_heading(project));
        for session in &project.sessions {
            lines.push(format!(
                "{LINE_PREFIX}{}",
                session_line(session, &pull_requests)
            ));
            if page.verbose {
                lines.push(format!("  {}", detail_line(session)));
                lines.extend(
                    session
                        .events
                        .iter()
                        .map(|event| format!("  {}", event_line(event, page.clock))),
                );
            }
        }
    }
    Section {
        lines,
        trim: Trim::Always,
        omitted: 0,
        at_least: false,
    }
}

/// The project a run of sessions belongs to, or the one word that says there
/// was none. A session composed outside a repository has an empty project, and
/// a blank line there would read as a heading that failed to render.
fn project_heading(project: &Project) -> String {
    match project.project.is_empty() {
        true => NO_PROJECT.to_string(),
        false => project.project.clone(),
    }
}

/// One session: what it was called, which harness ran it, on what branch, for
/// how long, where it got to, and the pull request it moved if one is on the
/// page.
///
/// NO DANGLING SEPARATOR for a session that carries no title or no branch,
/// which is `described`'s own rule: a line of empty columns reads as truncated
/// rather than as a harness that said nothing.
pub(super) fn session_line(session: &Session, pull_requests: &[String]) -> String {
    let mut columns = vec![
        match session.title.is_empty() {
            true => session.session.clone(),
            false => session.title.clone(),
        },
        session.harness.clone(),
        session.branch.clone(),
        ran_for(session.duration_secs),
        session.last_state.clone(),
    ];
    columns.extend(pull_request_for(&session.branch, pull_requests));
    columns
        .into_iter()
        .filter(|column| !column.is_empty())
        .collect::<Vec<_>>()
        .join("  ")
}

/// The pull request a row on this same page names on this branch, if one does.
///
/// THE ROWS ARE THE ONLY SOURCE, which is what keeps this section from
/// spawning anything: the `pull_requests` command already ran for its own
/// section, and this reads what it said rather than asking again.
///
/// A WHOLE-TOKEN MATCH ON THE BRANCH, through `cites`, so `feat/x` does not
/// claim a row about `feat/x-2`.
fn pull_request_for(branch: &str, rows: &[String]) -> Option<String> {
    if branch.is_empty() {
        return None;
    }
    rows.iter()
        .find(|row| cites(row, branch))
        .and_then(|row| row.split_whitespace().find(|word| is_number_token(word)))
        .map(str::to_string)
}

/// Whether a word is a `#<digits>` pull request number and nothing else.
fn is_number_token(word: &str) -> bool {
    word.strip_prefix('#').is_some_and(|digits| {
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// How long a session ran inside the window, in the widths an operator reads
/// at a glance rather than in seconds.
pub(super) fn ran_for(seconds: u64) -> String {
    let minutes = seconds / 60;
    match minutes / 60 {
        0 => format!("{minutes}m"),
        hours => format!("{hours}h {}m", minutes % 60),
    }
}

/// What `-v` adds under a session line: the identifiers a person needs to go
/// back to the pane it ran in.
fn detail_line(session: &Session) -> String {
    [
        session.session.as_str(),
        session.pane.as_str(),
        session.workspace.as_str(),
        session.model.as_str(),
    ]
    .into_iter()
    .filter(|field| !field.is_empty())
    .collect::<Vec<_>>()
    .join("  ")
}

/// One event as a line of the timeline: when, in what state, and what it said.
fn event_line(event: &Event, clock: Clock) -> String {
    format!(
        "{} {} {}",
        clock(Some(event.at)),
        mark(&event.state),
        described(event)
    )
}

/// One event as a sentence: who, in what state, on what, and what it said.
///
/// NO DANGLING PUNCTUATION for an event that carries no project or no detail,
/// which is `rendered`'s own rule in the journal: a line ending in a colon
/// reads as truncated rather than complete.
pub fn described(event: &Event) -> String {
    let agent = if event.agent.is_empty() {
        "pns"
    } else {
        &event.agent
    };
    let state = if event.state.is_empty() {
        "done"
    } else {
        &event.state
    };
    let mut line = format!("{agent}/{state}");
    if !event.project.is_empty() {
        line.push(' ');
        line.push_str(&event.project);
    }
    if !event.detail.is_empty() {
        line.push_str(": ");
        line.push_str(&event.detail);
    }
    line
}
/// The timeline's marker for one state word, chosen MECHANICALLY and never by
/// a model: a finished turn, a turn that died, and a turn waiting on the
/// operator are the three the eye is scanning for, and everything else is
/// deliberately unmarked so those three stand out.
pub(super) fn mark(state: &str) -> &'static str {
    if crate::missed::NEEDS_YOU.contains(&state) {
        // `failed` is in that list AND is its own kind of news, so it is
        // answered before the waiting mark rather than inside it.
        return if state == "failed" { "!" } else { "?" };
    }
    if state == "done" { "+" } else { " " }
}
pub(super) const AGENTS_HEADING: &str = "AGENTS";
/// A session composed outside any repository. See `project_heading`.
const NO_PROJECT: &str = "(no project)";
/// An empty window, which the event path never posts (nothing is under every
/// threshold) and a hand-run `pns recap` reaches whenever it is pointed at a
/// quiet stretch.
const NOTHING_HAPPENED: &str = "- nothing was recorded in this window";
