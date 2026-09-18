use super::{Submission, delivered_decision, event, run, submission};
use pns_domain::{EventArgs, Overrides};

/// The two records the marker guard writes together, or not at all.
fn armed(guessed: bool, state: &str, loop_live: bool) -> bool {
    let (decision, overrides) = (delivered_decision(), Overrides::default());
    let event = EventArgs {
        state: state.to_string(),
        guessed,
        ..event()
    };
    let steps = run(Submission {
        loop_live,
        ..submission(&event, &decision, &overrides)
    });
    let armed = steps.iter().any(|step| step.starts_with("marker"));
    assert_eq!(
        armed,
        steps.contains(&"wait".to_string()),
        "the marker and the wait must be written together: {steps:?}"
    );
    armed
}

#[test]
fn a_guessed_wait_inside_a_live_loop_arms_nothing() {
    assert!(!armed(true, "asking", true));
    // The condenser's other wait word, guessed off the same prose.
    assert!(!armed(true, "blocked", true));
}

#[test]
fn a_guessed_wait_with_no_loop_arms_the_marker() {
    assert!(armed(true, "asking", false));
}

#[test]
fn a_hook_driven_wait_arms_the_marker_with_or_without_a_loop() {
    for state in ["asked", "blocked", "asking"] {
        assert!(armed(false, state, true), "{state}");
        assert!(armed(false, state, false), "{state}");
    }
}

#[test]
fn a_guessed_finish_inside_a_live_loop_still_clears_the_marker() {
    // ONLY A START IS WITHHELD. `done` ends a wait, and a loop that swallowed
    // the clear would leave a marker armed over a turn that had finished.
    assert!(armed(true, "done", true));
}
