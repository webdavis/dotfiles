//! Opening a failure's full record, and what happens when that does not work.
//!
//! THE LADDER IS WHAT MAKES A CLICK WORTH WIRING. A click that silently does
//! nothing is worse than a banner with no click at all: the operator learns the
//! gesture is unreliable and stops using it, and the record they were reaching
//! for is the one this whole design exists to put in front of them.

use crate::CommandRunner;
use pns_domain::failure::ClickView;

/// What a click did, in the order it tried things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClickOutcome {
    /// Every argv tried, in order. One entry is a click that worked first time.
    pub tried: Vec<Vec<String>>,
    /// Whether anything opened.
    pub opened: bool,
    /// Whether the LAST resort was reached: nothing opened and the operator has
    /// to be told some other way.
    pub exhausted: bool,
}

/// Open failure `id` through `view`, falling back to a plain window.
///
/// THE FALLBACK IS `window` AND NOTHING ELSE, whichever view was configured.
/// It needs no session, no config and no operator string, so it is the one that
/// cannot fail for a reason the previous attempt already failed for. Trying the
/// configured view twice, or trying herdr after a command, would just spend
/// another second reaching the same place.
pub fn open_failure<R: CommandRunner>(
    runner: &R,
    view: &ClickView,
    id: u64,
    herdr_path: &str,
    pns_path: &str,
) -> ClickOutcome {
    let mut tried = Vec::new();
    if let Some(argv) = view.argv(id, herdr_path, pns_path) {
        tried.push(argv.clone());
        if run(runner, &argv) {
            return ClickOutcome {
                tried,
                opened: true,
                exhausted: false,
            };
        }
    }
    // A view that is ALREADY the fallback does not get a second identical try.
    if matches!(view, ClickView::Window) {
        return ClickOutcome {
            tried,
            opened: false,
            exhausted: true,
        };
    }
    let Some(argv) = ClickView::Window.argv(id, herdr_path, pns_path) else {
        return ClickOutcome {
            tried,
            opened: false,
            exhausted: true,
        };
    };
    tried.push(argv.clone());
    let opened = run(runner, &argv);
    ClickOutcome {
        tried,
        opened,
        exhausted: !opened,
    }
}

fn run<R: CommandRunner>(runner: &R, argv: &[String]) -> bool {
    let Some((program, args)) = argv.split_first() else {
        return false;
    };
    runner
        .run(
            program,
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .is_some()
}

/// The log line for a click that did not open anything, naming every command it
/// tried so the operator can run one by hand and see the real error.
pub fn click_failure_line(id: u64, outcome: &ClickOutcome) -> String {
    let tried = outcome
        .tried
        .iter()
        .map(|argv| argv.join(" "))
        .collect::<Vec<_>>()
        .join("; then ");
    if tried.is_empty() {
        return format!("pns: could not open failure {id}: the configured click has no command");
    }
    format!("pns: could not open failure {id}; tried: {tried}")
}

/// The LAST RESORT: the title and message of the banner raised when no view
/// opened at all.
///
/// A CLICK IS THE ONE PLACE WITH NOBODY WATCHING STDERR, which is what this is
/// for: the log line above is written for an operator who goes looking, and a
/// click is a gesture by someone who is not. The message names the command
/// that reads the record from any terminal, because a view is what just failed
/// and offering another one would be the same failure again.
///
/// The banner this text goes on carries NO CLICK. Clicking a failed click to
/// be told the click failed is the loop this stops.
pub fn click_banner(id: u64) -> (String, String) {
    (
        format!("pns could not open failure {id}"),
        format!("run `pns failures {id}` in a terminal"),
    )
}

#[cfg(test)]
#[path = "open_failure/tests.rs"]
mod tests;
