use super::*;

#[test]
fn a_producer_invocation_led_by_a_stray_word_is_refused_and_names_it() {
    // A word pns skipped in silence was a caller whose real flags may have
    // gone nowhere, so `send` refuses it by name and delivers nothing.
    let sandbox = Sandbox::new("stray-leading-word");
    let output = run_expecting(
        2,
        sandbox
            .pns()
            .args(["send", "stray", "--producer", "claude", "--state", "done"])
            .args(["--detail", "a summary"]),
    );
    assert_eq!(stderr(&output), "pns: stray is not a flag pns takes\n");
    assert!(!sandbox.fired("phone"));
    assert!(!sandbox.fired("hermes"));
}

#[test]
fn a_bare_send_is_still_the_empty_event_the_contract_calls_valid() {
    // THE OTHER MIRROR. `EventArgs` defaults every field, so `send` naming no
    // field at all still renders and delivers an empty event; the subcommand
    // is what asked for a notification.
    let sandbox = Sandbox::new("bare-send");
    run(sandbox.pns().arg("send"));
    assert!(sandbox.fired("phone"));
    assert!(sandbox.fired("hermes"));
}

#[test]
fn producer_flags_with_no_subcommand_are_refused_rather_than_delivered() {
    // THE POINT OF THE SUBCOMMAND. `pns --state done` used to render an empty
    // event and page about it; the flags alone no longer ask for anything.
    for argv in [&[][..], &["--state", "done"][..]] {
        let sandbox = Sandbox::new(&format!("no-subcommand-{}", argv.len()));
        let output = sandbox.pns().args(argv).output().expect("the engine runs");
        assert_eq!(output.status.code(), Some(2), "{argv:?}: {output:?}");
        assert!(stderr(&output).contains("usage"), "{argv:?}: {output:?}");
        assert!(!sandbox.fired("phone"), "{argv:?}: {output:?}");
        assert!(!sandbox.fired("hermes"), "{argv:?}: {output:?}");
    }
}

#[test]
fn the_delivered_event_is_newline_terminated_for_line_oriented_channels() {
    let sandbox = Sandbox::new("newline-terminated");
    sandbox.stub_channel(
        "hermes",
        &format!(
            "set -e\nIFS= read -r event\nprintf %s \"$event\" >\"{}/line.event\"",
            sandbox.display()
        ),
    );
    run(sandbox.pns().args([
        "send",
        "--producer",
        "claude",
        "--state",
        "done",
        "--detail",
        "x",
    ]));
    let line = std::fs::read_to_string(sandbox.path("line.event")).expect("one whole line");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("a whole JSON line");
    assert_eq!(parsed["agent"], "claude");
}
