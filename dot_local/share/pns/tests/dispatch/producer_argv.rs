use super::*;

#[test]
fn a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event() {
    let sandbox = Sandbox::new("pane-scrub");
    let output = run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "wW:p1; curl evil | sh"]));
    assert!(sandbox.fired("macos-banner"));
    assert_eq!(sandbox.event("macos-banner")["pane"], "");
    assert!(
        stderr(&output).contains("dropped a pane id with shell metacharacters"),
        "{output:?}"
    );
}

#[test]
fn a_scrub_warning_is_not_printed_when_no_channel_will_run() {
    let sandbox = Sandbox::new("scrub-silent");
    let output = run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--pane", "wW:p1; curl evil | sh"])
        .args(["--local-only", "--remote-only"]));
    assert!(!stderr(&output).contains("dropped a pane id"), "{output:?}");
}

#[test]
fn a_non_unicode_argument_never_breaks_the_exit_zero_edge() {
    // The engine sits on an always-exit-0 path; a stray byte in argv must
    // degrade like any unknown token, not abort the notification.
    let sandbox = Sandbox::new("non-unicode");
    let output = run(sandbox
        .pns()
        .arg(OsStr::from_bytes(&[0xff]))
        .args(["--local-only", "--remote-only"]));
    assert!(stdout(&output).contains("SKIPPED"), "{output:?}");
}

#[test]
fn the_help_flag_prints_the_usage_and_reaches_nothing_at_all() {
    // A help print used to be an EVENT: it loaded the config, spawned every
    // presence probe and delivered a notification reading "pns · done" to
    // whatever channel was configured. Nothing about printing the commands
    // needs the machine read, so nothing here may reach it.
    for spelling in ["--help", "-h"] {
        let (sandbox, mut command) = desk_with_a_native_banner("help");
        let output = run(command.arg(spelling));

        let printed = stdout(&output);
        assert!(printed.contains("usage"), "{spelling}: {printed}");
        assert_eq!(stderr(&output), "", "help is not a complaint: {output:?}");
        assert_eq!(
            sandbox.spawned(),
            "",
            "a help print spawns nothing: {output:?}"
        );
        assert!(
            !sandbox.state().exists(),
            "and writes no state either: {output:?}"
        );
    }
}

#[test]
fn a_word_that_names_no_command_is_refused_and_delivers_nothing() {
    // THE HOUSE RULE `pns nag` already keeps, moved up to the top-level
    // dispatch: an unknown argument never falls through to a fire. A mistyped
    // subcommand used to reach the lenient producer parser, which skipped the
    // word it did not know and notified about an empty event, so `pns stpo`
    // raised a banner and could card the operator's phone.
    //
    // THE SECOND ARGV IS THE PARSER'S OWN FLAG LIST BEING CONSULTED rather
    // than a lookalike. A refusal that asked "does anything here start with a
    // dash" instead of "is this a flag the parser knows" reads `--wat` as a
    // producer invocation and delivers the empty event again: the same bug,
    // reached by mistyping the flag as well as the word.
    for argv in [&["stpo"][..], &["stpo", "--wat"][..]] {
        let (sandbox, mut command) = desk_with_a_native_banner("typo");
        let output = command.args(argv).output().expect("the engine runs");

        assert_eq!(
            output.status.code(),
            Some(2),
            "{argv:?}: a refusal, never exit 0"
        );
        let complaint = stderr(&output);
        assert!(
            complaint.contains("usage"),
            "and the usage is on stderr: {output:?}"
        );
        assert_eq!(
            stdout(&output),
            "",
            "nothing was delivered to say: {output:?}"
        );
        assert_eq!(sandbox.spawned(), "", "a typo spawns nothing: {output:?}");
        assert!(
            !sandbox.state().exists(),
            "and writes no state either: {output:?}"
        );
    }
}

#[test]
fn a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event() {
    // R5-1: `first.starts_with('-')` used to make ANY dash-led argv[1] a
    // producer invocation, so a mistyped flag delivered an empty event in
    // silence, the `pns stpo` bug reopened for a typo that happens to start
    // with a dash. None of these carries a flag the parser recognizes.
    for word in [
        "--wat",
        "-",
        "--",
        "--help=x",
        "--HELP",
        "-help",
        "--agent=claude",
    ] {
        let sandbox = Sandbox::new("dash-led-typo");
        let output = sandbox.pns().arg(word).output().expect("the engine runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{word:?}: a refusal, never exit 0"
        );
        assert!(stderr(&output).contains("usage"), "{word:?}: {output:?}");
        assert!(!sandbox.fired("mobile"), "{word:?} delivered: {output:?}");
        assert!(!sandbox.fired("hermes"), "{word:?} delivered: {output:?}");
    }
}

#[test]
fn a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it() {
    // R5-3: `unwrap_or_default().is_empty()` conflated "no argv[1] at all"
    // with "argv[1] is the literal empty string", so `pns ""` delivered the
    // same empty event a truly bare `pns` does. `doctor` and `setup` already
    // refuse an unknown extra argument; this is that same rule reaching the
    // top-level dispatch.
    let sandbox = Sandbox::new("typed-empty-word");
    let output = sandbox.pns().arg("").output().expect("the engine runs");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(stderr(&output).contains("usage"), "{output:?}");
    assert!(!sandbox.fired("mobile"), "{output:?}");
    assert!(!sandbox.fired("hermes"), "{output:?}");
}

#[test]
fn help_in_flag_position_wins_wherever_it_reaches_the_event_parser() {
    // R5-2 + H-A: help was checked at argv[1] only, so `--agent claude
    // --help` delivered the event with `--help` unconsumed, and `--
    // --help`/`stray --help` reached the lenient producer parser and did the
    // same. `is_producer_argv` now counts `--help`/`-h` too, so all four
    // shapes reach the parser's own help arm instead.
    for argv in [
        &["--agent", "claude", "--help"][..],
        &["--local-only", "--help"][..],
        &["--", "--help"][..],
        &["stray", "--help"][..],
    ] {
        let sandbox = Sandbox::new("help-anywhere");
        let output = sandbox.pns().args(argv).output().expect("the engine runs");
        assert_eq!(output.status.code(), Some(0), "{argv:?}: {output:?}");
        assert!(stdout(&output).contains("usage"), "{argv:?}: {output:?}");
        assert_eq!(stderr(&output), "", "{argv:?}: {output:?}");
        assert!(
            !sandbox.fired("mobile"),
            "{argv:?} spawned a delivery: {output:?}"
        );
        assert!(
            !sandbox.fired("hermes"),
            "{argv:?} spawned a delivery: {output:?}"
        );
    }
}

#[test]
fn help_in_value_position_is_still_just_a_value() {
    // H-F, PINNED so nobody "fixes" this by adding `--help` to
    // `is_producer_flag`: doing that would flip the value rule and make
    // `--agent --help` warn-and-drop instead of delivering an agent whose
    // name literally is "--help". States are free-form the same way.
    let sandbox = Sandbox::new("help-as-agent-value");
    run(sandbox.pns().args(["--agent", "--help", "--state", "done"]));
    assert_eq!(sandbox.event("mobile")["agent"], "--help");

    let sandbox = Sandbox::new("help-as-state-value");
    run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "--help"]));
    assert_eq!(sandbox.event("mobile")["state"], "--help");
}

#[test]
fn a_missing_value_warning_keeps_its_exact_sentence() {
    let sandbox = Sandbox::new("missing-value-warning");
    let output = run(sandbox
        .pns()
        .args(["--detail", "--local-only", "--remote-only"]));
    assert_eq!(
        stderr(&output),
        "pns: --detail given without a value; ignoring\n"
    );
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("hermes"));
}
