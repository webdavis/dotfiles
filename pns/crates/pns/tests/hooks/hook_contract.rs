use super::*;

#[test]
fn an_ordinary_stop_never_reaches_moshi() {
    // THE STUB IS THE TRIPWIRE, the same one the `asked` and `denied` arms
    // carry: it records its argv and exits 42, so a Stop swept into the
    // submission path would leave that file behind and hand the 42 back as a
    // decision on a path whose contract is always zero. AWAY is what arms it;
    // at the desk the submission declines on its own and the tripwire would
    // stay un-run for the wrong reason.
    //
    // Stop is the highest-volume event there is and the one a "moshi is just
    // another channel" design sweeps in first. That would break the
    // single-submitter rule and put an Allow/Deny card in front of an operator
    // for a turn that has already finished.
    //
    // MECHANISM-BOUND, IN THE DANGEROUS DIRECTION: an absent record is
    // absently true for a build that submits over some other transport, so
    // the absence reads through `submissions` and item 25's duty is to
    // RE-POINT that one function at the new record. Spelled as a filename,
    // the guard written against this exact regression would go quiet on the
    // switch that causes it.
    let sandbox = Sandbox::new("hook-stop-no-round-trip");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"done here"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        submissions(&sandbox).is_empty(),
        "a finished turn is news, not a question for the phone"
    );
    // Absence alone would also be green for an arm that does nothing at all.
    assert!(
        sandbox.fired("hermes"),
        "and the operator still hears the turn ended"
    );
}

#[test]
fn nothing_that_goes_wrong_building_a_notification_fails_the_harness_turn() {
    let sandbox = Sandbox::new("hook-garbage");
    for payload in ["", "not json", r#"{"session_id":null}"#] {
        let output = hook(&sandbox, "stop", payload);
        assert_eq!(output.status.code(), Some(0), "payload {payload:?}");
    }
    let output = hook(&sandbox, "no-such-event", r#"{}"#);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn a_hook_word_this_binary_does_not_serve_says_so_and_notifies_nobody() {
    // The match gains an arm every time a harness gains an event, and a word
    // that reaches none of them must not fall through to the nearest one:
    // `stop-failed` is one letter from the arm that reports a dead turn, and
    // reporting one for it would be a card about an event that never
    // happened. It costs a stderr line, no notification, and a zero exit,
    // because an unserved event is not an error the harness should hear about
    // on a notification path.
    let sandbox = Sandbox::new("hook-unknown-event");
    let output = hook(
        &sandbox,
        "stop-failed",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","error":"API Error: 500"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "pns: unknown hook event `stop-failed`"
    );
    assert!(
        !sandbox.fired("hermes"),
        "an event nobody serves reaches no channel"
    );
}
