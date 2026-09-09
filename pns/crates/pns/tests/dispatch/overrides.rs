use super::*;

// --- the narrowing flags ----------------------------------------------------

#[test]
fn local_only_keeps_the_banner_and_reaches_nothing_off_the_machine() {
    let sandbox = Sandbox::new("local-only");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .arg("--local-only"));
    assert!(sandbox.fired("macos-banner"));
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("hermes"));
}

#[test]
fn remote_only_delivers_through_hermes_alone() {
    let sandbox = Sandbox::new("remote-only");
    run(sandbox
        .pns()
        .args(["--agent", "weekly", "--state", "done"])
        .args(["--project", "skills", "--detail", "ran", "--remote-only"]));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("macos-banner"));
}

#[test]
fn hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible() {
    let sandbox = Sandbox::new("hermes-sync");
    run(sandbox
        .pns()
        .args(["--agent", "weekly", "--state", "done", "--detail", "ran"])
        .arg("--remote-only"));
    assert_eq!(sandbox.event("hermes")["mode"], "sync");
}

#[test]
fn both_narrowing_flags_together_deliver_nothing_and_say_so() {
    let sandbox = Sandbox::new("both-flags");
    let output = run(sandbox
        .pns()
        .args(["--agent", "x", "--state", "done", "--detail", "y"])
        .args(["--local-only", "--remote-only"])
        .args(["--pane", "w:p; invalid"]));
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("hermes"));
    assert!(!sandbox.fired("macos-banner"));
    assert_eq!(
        stdout(&output),
        "pns: post SKIPPED, --local-only and --remote-only were both given, which suppresses every channel; nothing was sent\n"
    );
    assert_eq!(stderr(&output), "", "no pane warning before refusal");
    assert!(
        !sandbox.state().exists(),
        "refused argv reached domain state"
    );
}

// --- presence ---------------------------------------------------------------

#[test]
fn at_the_desk_the_phone_is_skipped_and_only_the_phone() {
    let sandbox = Sandbox::new("at-the-desk");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(!sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn relay_skip_phone_drops_the_phone_and_only_the_phone() {
    // The caller has already raised the card on the phone through moshi-hook's
    // own round trip, so the push here would be the same event twice; the
    // banner and the paper trail are still wanted.
    let sandbox = Sandbox::new("skip-phone");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_SKIP_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn relay_skip_phone_beats_relay_force_phone() {
    // "I have already sent it" is more specific than a standing override, and
    // the override is the one thing that could reintroduce the double push.
    let sandbox = Sandbox::new("skip-beats-force");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_SKIP_PHONE", "1")
        .env("PNS_FORCE_PHONE", "1")
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"]));
    assert!(!sandbox.fired("mobile"));
}

#[test]
fn relay_force_phone_overrides_presence() {
    let sandbox = Sandbox::new("force-phone");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("mobile"));
}

// --- a channel's own failures -----------------------------------------------

#[test]
fn a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings() {
    let sandbox = Sandbox::new("channel-fails");
    sandbox.stub_channel("mobile", "exit 9");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("macos-banner"));
}

#[test]
fn an_absent_channel_is_simply_not_installed() {
    let sandbox = Sandbox::new("absent-channel");
    std::fs::remove_file(sandbox.root.join("channels/hermes.sh")).expect("remove the channel");
    let output = run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(sandbox.fired("macos-banner"));
    // AND IT IS STILL A NON-EVENT. hermes runs sync on this path, so a launch
    // failure that reported itself would print here; the hand-run check is the
    // only caller that reads one.
    assert_eq!(
        stdout(&output),
        "",
        "a channel nobody installed is not news on the notification path"
    );
}
