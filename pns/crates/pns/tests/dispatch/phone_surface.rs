use super::*;

// --- the attention override -------------------------------------------------

#[test]
fn a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile() {
    // Matrix row "tap newer than the last desk input wins: mobile". The
    // marker is a file the phone touches; nothing else has to know why.
    let sandbox = Sandbox::new("tap-newer");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(sandbox.fired("mobile"));
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
}

#[test]
fn desk_input_after_the_tap_cancels_it() {
    // Matrix row "desk input AFTER the tap cancels it": newest signal wins,
    // which is what retired the marker's fixed five-minute TTL.
    let sandbox = Sandbox::new("tap-cancelled");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_PHONE_MARKER_FILE", &marker);
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("mobile"), "the desk is newer than the tap");
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight() {
    // Drill D6, run (i), 2026-08-19: Back Tap with moshi closed produced
    // NOTHING. The tap moved the surface to mobile, the origin pane sat
    // focused on the desk display nobody was at, and mobile-plus-visible
    // suppressed. `pns()` states the phone's pty clock as a day untouched,
    // which is exactly the closed-moshi half of the repro.
    let sandbox = Sandbox::new("tap-moshi-closed");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker);
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("mobile"), "the tap asked for the card");
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
}

#[test]
fn a_narrowing_flag_still_beats_a_fresh_tap() {
    let sandbox = Sandbox::new("tap-local-only");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .arg("--local-only")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("mobile"));
}

#[test]
fn skip_phone_still_beats_a_fresh_tap() {
    let sandbox = Sandbox::new("tap-skip");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "300")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .env("PNS_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("mobile"));
}

// --- the pane the operator is looking at ------------------------------------

#[test]
fn a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log() {
    // Matrix row "mobile watching: suppressed". The card would describe the
    // pane already filling the phone's screen.
    let sandbox = Sandbox::new("watched-pane");
    let mut command = sandbox.pns();
    command.env("PNS_PHONE_INPUT_AGE", "0");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("macos-banner"), "mobile never banners");
    assert!(sandbox.fired("hermes"));
}

#[test]
fn a_phone_in_hand_showing_another_tab_still_cards() {
    // Matrix row "mobile, origin hidden: card only".
    let sandbox = Sandbox::new("other-pane");
    let mut command = sandbox.pns();
    command.env("PNS_PHONE_INPUT_AGE", "0");
    sandbox.stub_herdr(&mut command, false);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("mobile"));
    assert!(!sandbox.fired("macos-banner"));
}

#[test]
fn an_unreadable_view_delivers_rather_than_suppressing_on_doubt() {
    // Matrix row "desk, visibility unknown: deliver, never suppress on
    // doubt". No herdr on PATH at all, which is the probe failing.
    let sandbox = Sandbox::new("unknown-view");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn force_phone_is_caller_intent_and_beats_the_whole_surface_model() {
    let sandbox = Sandbox::new("force-phone-watched");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1");
    sandbox.stub_herdr(&mut command, true);
    run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2"]));
    assert!(sandbox.fired("mobile"));
}
