use herdr_smart_nav_domain::{Action, decide};

pub fn navigate(
    pane: Option<&str>,
    direction: &str,
    chord: &str,
    foreground: impl FnOnce(&str) -> bool,
) -> Action {
    let is_nvim = pane.map(foreground).unwrap_or(false);
    decide(pane, is_nvim, direction, chord)
}
