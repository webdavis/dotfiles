use super::*;

// --- the alert path ---------------------------------------------------------

#[test]
fn away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner() {
    // Matrix row "away: phone card regardless of any client's display". The
    // banner belongs to the desk, and nobody is at it: a banner nobody sees
    // was the old always-on rule this replaced.
    let sandbox = Sandbox::new("alert-path");
    run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a summary"]));
    assert!(sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("macos-banner"), "away raises no banner");
}

#[test]
fn at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery() {
    // Matrix row "desk, origin hidden: banner". No card, because the operator
    // is right here.
    let sandbox = Sandbox::new("desk-hidden");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("macos-banner"));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("mobile"), "the desk gets no card");
}

#[test]
fn at_the_desk_watching_the_pane_only_the_log_fires() {
    // Matrix row "desk watching: suppressed entirely". The pane is on screen,
    // so the event is already in front of the operator.
    let sandbox = Sandbox::new("desk-watching");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("macos-banner"), "the pane is in plain sight");
    assert!(!sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"));
}

#[test]
fn the_alert_path_labels_the_hermes_leg_silent_on_the_wire() {
    // The NAME is the whole claim. This used to say delivery stayed off the
    // caller's critical path, which it never checked and which is not true
    // anyway: the dispatch waits for the channel either way. What the label
    // selects is whether the leg reports its outcome.
    let sandbox = Sandbox::new("hermes-async");
    run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert_eq!(sandbox.event("hermes")["mode"], "async");
}

#[test]
fn a_channel_is_handed_the_rendered_event_not_the_raw_arguments() {
    let sandbox = Sandbox::new("rendered-event");
    run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--branch", "main"])
        .args(["--detail", "a summary"]));
    let event = sandbox.event("mobile");
    assert_eq!(event["agent"], "claude");
    for rendered in ["title", "message", "preview"] {
        // A MISSING key indexes to Null, and Null is != "", so the absence
        // has to be refused by type rather than by inequality.
        assert!(
            event[rendered]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "{rendered} must be a non-empty rendered string: {event}"
        );
    }
}
