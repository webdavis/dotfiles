use super::*;

#[test]
fn a_producer_invocation_led_by_a_stray_word_still_delivers() {
    // THE MIRROR OF THE REFUSAL ABOVE, and the reason the refusal reads the
    // whole of argv rather than its first word. The parser deliberately skips
    // an unrecognized token in front of the real flags, so an invocation
    // carrying producer flags is a producer invocation whatever leads it, and
    // refusing one would silently drop a notification instead of a typo.
    let sandbox = Sandbox::new("stray-leading-word");
    run(sandbox
        .pns()
        .args(["stray", "--agent", "claude", "--state", "done"])
        .args(["--detail", "a summary"]));
    assert!(sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"));
}

#[test]
fn a_bare_invocation_is_still_the_empty_event_the_contract_calls_valid() {
    // THE OTHER MIRROR. `EventArgs` defaults every field, so argv naming
    // nothing at all has always rendered and delivered an empty event; a
    // refusal that read no argument as no command would swallow that whole arm
    // while looking exactly like the typo fix.
    let sandbox = Sandbox::new("bare-invocation");
    run(&mut sandbox.pns());
    assert!(sandbox.fired("mobile"));
    assert!(sandbox.fired("hermes"));
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
    run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    let line = std::fs::read_to_string(sandbox.path("line.event")).expect("one whole line");
    let parsed: serde_json::Value = serde_json::from_str(&line).expect("a whole JSON line");
    assert_eq!(parsed["agent"], "claude");
}
