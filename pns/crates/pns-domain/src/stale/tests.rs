use super::{Blocked, Gate, Suppressed, gate, page, waited};
use crate::surface::Surface;

/// An hour, which is the shipped window.
const WINDOW: u64 = 3_600;

fn blocked() -> Blocked {
    Blocked {
        session: "a1b2c3d4".to_string(),
        harness: "claude".to_string(),
        project: "dotfiles".to_string(),
        branch: "feat/posture-alert-cutover".to_string(),
        title: "arm posture alert and retire the Bash alerter".to_string(),
        since: 1_000,
    }
}

#[test]
fn the_page_goes_to_the_urgent_route_the_config_named() {
    // THE PAGE NAMES NO ROUTE ITSELF. It carries the health kind, and the
    // route that kind takes is the one `[routes] urgent` spells.
    let raised = page(&blocked(), 4_780);
    assert!(
        raised.channel.is_empty(),
        "the page pinned a route name: {}",
        raised.channel
    );
    assert_eq!(
        raised
            .routed(&crate::routes::Routes::named("logbook", "sirens"))
            .channel,
        "sirens"
    );
}

#[test]
fn the_page_says_how_long_the_block_has_stood() {
    assert_eq!(
        page(&blocked(), 4_780).detail,
        "blocked 63 minutes, no answer"
    );
    assert_eq!(
        waited(90),
        "blocked 1 minute, no answer",
        "one minute is not one minutes"
    );
}

#[test]
fn an_away_operator_is_never_paged() {
    assert_eq!(
        gate(Surface::Away, Some(false), Some(10), WINDOW),
        Gate::Skip(Suppressed::Away)
    );
}

#[test]
fn a_screen_locked_through_the_whole_window_is_not_paged() {
    assert_eq!(
        gate(Surface::Mobile, Some(true), Some(WINDOW), WINDOW),
        Gate::Skip(Suppressed::LockedThroughWindow)
    );
}

#[test]
fn a_screen_locked_inside_the_window_is_still_paged() {
    // The operator was at the desk, stepped away and locked it, and the page
    // is what tells them a session is stuck.
    assert_eq!(
        gate(Surface::Mobile, Some(true), Some(WINDOW - 1), WINDOW),
        Gate::Page
    );
}
