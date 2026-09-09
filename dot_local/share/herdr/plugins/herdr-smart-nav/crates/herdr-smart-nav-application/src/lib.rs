//! The one use case: decide who moves, having asked what is running.
//!
//! THE PROBE IS ONLY ASKED WHEN THERE IS A PANE TO ASK ABOUT. It shells out to
//! herdr and waits for an answer, and a press that arrived with no pane id has
//! nothing to name in that question.

use herdr_smart_nav_domain::{Action, Direction, decide};

/// Decide what this press does.
///
/// `foreground` answers whether the named pane is running Neovim. A CLOSURE
/// RATHER THAN A TRAIT: it is one injected reading with one production
/// implementation, and a trait here would be a one-method wrapper existing only
/// to be substituted in a test.
pub fn navigate(
    pane: Option<&str>,
    direction: Direction,
    foreground: impl FnOnce(&str) -> bool,
) -> Action {
    let is_nvim = pane.map(foreground).unwrap_or(false);
    decide(pane, is_nvim, direction)
}
