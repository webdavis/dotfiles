use std::env;

use herdr_smart_nav_adapters::Herdr;
use herdr_smart_nav_application::navigate;
use herdr_smart_nav_domain::direction_to_chord;

mod pane;

fn main() {
    let direction = env::args().nth(1).unwrap_or_default();
    let Some(chord) = direction_to_chord(&direction) else {
        eprintln!("herdr-smart-nav: usage: herdr-smart-nav left|down|up|right");
        std::process::exit(2);
    };
    let pane = pane::resolve_pane(
        env::var("HERDR_PANE_ID").ok(),
        env::var("HERDR_ACTIVE_PANE_ID").ok(),
    );
    let herdr = Herdr::new(env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string()));
    let action = navigate(pane.as_deref(), &direction, chord, |pane| {
        herdr.is_nvim(pane)
    });
    herdr.execute(&action);
}
