//! The composition root: read the press, resolve the pane, run the decision.

use std::env;

use herdr_smart_nav_adapters::Herdr;
use herdr_smart_nav_application::navigate;
use herdr_smart_nav_domain::Direction;

mod pane;

/// The `herdr` binary a keybinding's environment names, or the bare word for a
/// press that arrived with none.
const DEFAULT_HERDR: &str = "herdr";

const USAGE: &str = "herdr-smart-nav: usage: herdr-smart-nav left|down|up|right";

fn main() {
    let word = env::args_os()
        .nth(1)
        .and_then(|argument| argument.into_string().ok())
        .unwrap_or_default();
    // A WORD THIS DOES NOT SERVE MOVES NOTHING. A keybinding passes no
    // arguments, so the direction is baked into each action's argv in the
    // manifest: a word that reaches here and is not one of the four is a
    // manifest that disagrees with this binary, which is worth a usage line
    // rather than a guessed direction.
    let Some(direction) = Direction::parse(&word) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let pane = pane::resolve_pane(
        env::var("HERDR_PANE_ID").ok(),
        env::var("HERDR_ACTIVE_PANE_ID").ok(),
    );
    let herdr =
        Herdr::new(env::var("HERDR_BIN_PATH").unwrap_or_else(|_| DEFAULT_HERDR.to_string()));
    let action = navigate(pane.as_deref(), direction, |pane| herdr.is_nvim(pane));
    herdr.execute(&action);
}
