//! What the recap tests build from: one event, one clock, one page.

use crate::recap::activity::{Event, Project, by_project};
use crate::recap::external::External;
use crate::recap::sections::{Heading, Open, Page, Timeline};

/// A fixed clock, so the fixtures state a time rather than reading one.
pub(super) fn clock(at: Option<u64>) -> String {
    match at {
        Some(epoch) => format!("{:02}:{:02}", (epoch / 3600) % 24, (epoch / 60) % 60),
        None => "--:--".to_string(),
    }
}

/// One event in the window: an epoch, a state, and text naming its place.
/// EACH IN ITS OWN SESSION, so a window of `n` events is `n` lines of the
/// agents section, which is what the budget tests are measuring.
pub(super) fn acted(at: u64, state: &str, detail: &str) -> Event {
    Event {
        at,
        agent: "claude".to_string(),
        state: state.to_string(),
        project: "dotfiles".to_string(),
        session: format!("s{at}"),
        session_title: detail.to_string(),
        detail: detail.to_string(),
        ..Event::default()
    }
}

/// A window of `count` finished turns, each naming its own index.
pub(super) fn window(count: usize) -> Vec<Event> {
    (0..count)
        .map(|which| {
            acted(
                1_756_500_000 + which as u64 * 60,
                "done",
                &format!("turn {which}"),
            )
        })
        .collect()
}

/// The window's events, grouped the way the page reads them.
pub(super) fn grouped(events: &[Event]) -> Vec<Project> {
    by_project(events)
}

/// A page over one window, with whatever sections and open items a test hands
/// it. THE FIELDS THE TESTS DO NOT VARY ARE FIXED HERE, so an assertion names
/// only what it is about.
pub(super) fn page<'a>(
    projects: &'a [Project],
    counted: usize,
    sources: &'a [(&'a str, External<'a>)],
    open: &'a Open,
    timeline: Timeline<'a>,
) -> Page<'a> {
    Page {
        summary: None,
        heading: Heading::WhileYouWereAway,
        from: "23:04",
        to: "06:15",
        counted,
        projects,
        shows_agents: true,
        shows_open: true,
        timeline,
        sources,
        open,
        rows_per_section: ROWS_PER_SECTION,
        verbose: false,
        clock: &clock,
    }
}

/// How many rows a list section spends in these fixtures. Four, which is what
/// the two external sections were fixed at before `rows_per_section` existed,
/// so the budget arithmetic the tests below measure is unchanged.
pub(super) const ROWS_PER_SECTION: usize = 4;

/// The ring's own field cap, stated here so the fixture below is the
/// widest line the engine can actually write rather than an invented one.
pub(super) const ACTIVITY_MAX_CHARS: usize = 120;
