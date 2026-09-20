use super::*;

// --- the narrowing flags ----------------------------------------------------

#[test]
fn local_only_keeps_the_banner_and_reaches_nothing_off_the_machine() {
    let sandbox = Sandbox::new("local-only");
    run(sandbox
        .pns()
        .env("PNS_SCREEN_IDLE", "0")
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
        .args(["--scope", "local_only"]));
    assert!(sandbox.fired("banner"));
    assert!(!sandbox.fired("phone"));
    assert!(!sandbox.fired("hermes"));
}

#[test]
fn remote_only_delivers_through_hermes_alone() {
    let sandbox = Sandbox::new("remote-only");
    run(sandbox
        .pns()
        .args(["send", "--producer", "weekly", "--state", "done"])
        .args([
            "--project",
            "skills",
            "--detail",
            "ran",
            "--scope",
            "remote_only",
        ]));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("phone"));
    assert!(!sandbox.fired("banner"));
}

#[test]
fn hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible() {
    let sandbox = Sandbox::new("hermes-sync");
    run(sandbox
        .pns()
        .args([
            "send",
            "--producer",
            "weekly",
            "--state",
            "done",
            "--detail",
            "ran",
        ])
        .args(["--scope", "remote_only"]));
    assert_eq!(sandbox.event("hermes")["mode"], "sync");
}

// --- presence ---------------------------------------------------------------

#[test]
fn at_the_desk_the_phone_is_skipped_and_only_the_phone() {
    let sandbox = Sandbox::new("at-the-desk");
    run(sandbox.pns().env("PNS_SCREEN_IDLE", "0").args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]));
    assert!(!sandbox.fired("phone"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("banner"));
}

#[test]
fn relay_skip_phone_drops_the_phone_and_only_the_phone() {
    // The caller has already raised the card on the phone through moshi-hook's
    // own round trip, so the push here would be the same event twice; the
    // banner and the paper trail are still wanted.
    let sandbox = Sandbox::new("skip-phone");
    run(sandbox
        .pns()
        .env("PNS_SCREEN_IDLE", "0")
        .env("PNS_SKIP_PHONE", "1")
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "blocked",
            "--detail",
            "x",
        ]));
    assert!(!sandbox.fired("phone"));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("banner"));
}

#[test]
fn relay_skip_phone_beats_relay_force_phone() {
    // "I have already sent it" is more specific than a standing override, and
    // the override is the one thing that could reintroduce the double push.
    let sandbox = Sandbox::new("skip-beats-force");
    run(sandbox
        .pns()
        .env("PNS_SCREEN_IDLE", "0")
        .env("PNS_SKIP_PHONE", "1")
        .env("PNS_FORCE_PHONE", "1")
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "blocked",
            "--detail",
            "x",
        ]));
    assert!(!sandbox.fired("phone"));
}

#[test]
fn relay_force_phone_overrides_presence() {
    let sandbox = Sandbox::new("force-phone");
    run(sandbox
        .pns()
        .env("PNS_SCREEN_IDLE", "0")
        .env("PNS_FORCE_PHONE", "1")
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ]));
    assert!(sandbox.fired("phone"));
}

// --- a channel's own failures -----------------------------------------------

#[test]
fn a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings() {
    let sandbox = Sandbox::new("channel-fails");
    sandbox.stub_channel("phone", "exit 9");
    run(sandbox.pns().env("PNS_SCREEN_IDLE", "0").args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]));
    assert!(sandbox.fired("hermes"));
    assert!(sandbox.fired("banner"));
}

#[test]
fn an_absent_channel_is_simply_not_installed() {
    let sandbox = Sandbox::new("absent-channel");
    std::fs::remove_file(sandbox.root.join("channels/hermes.sh")).expect("remove the channel");
    let output = run(sandbox.pns().env("PNS_SCREEN_IDLE", "0").args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]));
    assert!(sandbox.fired("banner"));
    // AND IT IS STILL A NON-EVENT. hermes runs sync on this path, so a launch
    // failure that reported itself would print here; the hand-run check is the
    // only caller that reads one.
    assert_eq!(
        stdout(&output),
        "",
        "a channel nobody installed is not news on the notification path"
    );
}
