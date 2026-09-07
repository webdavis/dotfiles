use super::*;

// --- the turn that died -----------------------------------------------------

#[test]
fn a_turn_that_died_notifies_as_failed_and_says_what_killed_it() {
    // Claude Code fires StopFailure and NOT Stop when a turn dies on an API
    // error, so before this arm existed the operator walked back to a dead
    // pane with no card, no banner and no Discord line. The payload carries
    // the partial `last_assistant_message` a real StopFailure sends, because
    // the question at a dead pane is why it stopped rather than what it had
    // managed to say first.
    let sandbox = Sandbox::new("hook-stop-failure");
    let mut command = sandbox.pns();
    command.env("HERDR_PANE_ID", "wX:p9");
    let output = hook_with(
        command,
        &sandbox,
        "stop-failure",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"half a turn","error":"API Error: 500 internal server error"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "failed");
    assert_eq!(
        event["detail"], "API Error: 500 internal server error",
        "the partial reply the payload carried never stands in for the error"
    );
    assert_eq!(event["project"], "dotfiles");
    // THE PANE RIDES THE FAILED CARD like the Stop card's: a click should
    // focus the dead pane. Pinned here because a 4b mutation dropped it and
    // nothing noticed.
    assert_eq!(event["pane"], "wX:p9");
}

#[test]
fn a_dead_turn_consumes_the_marker_so_the_next_turn_is_not_measured_from_its_start() {
    // The leak this arm exists to close. StopFailure fires INSTEAD of Stop, so
    // a marker left behind is found by the next prompt, which declines to
    // rewrite it, and the turn AFTER the dead one is measured from the dead
    // one's start. `long_running` is what raises the mobile watch card and the
    // pulse, so one API error used to promote every later short turn to the
    // long-running tier for the rest of the session.
    let sandbox = Sandbox::new("hook-stop-failure-consumes");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "1").expect("marker");
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop-failure",
        r#"{"session_id":"s1","error":"API Error: 500"}"#,
    );
    assert!(
        !marker(&sandbox, "s1").exists(),
        "the marker is consumed, not left for the next turn to inherit"
    );
}

#[test]
fn a_dead_turn_spawns_no_condenser_and_reads_no_transcript() {
    // The condenser is a model call on the one path where a model call has
    // just failed, and the reply's transcript fallback is a bounded loop of
    // sleeps spent recovering text that is not the news. THE STUB IS THE
    // TRIPWIRE for the condenser: it records having run, and its verdict would
    // rewrite both the state and the detail, so a green here is a condenser
    // that never started.
    //
    // THE CLOCK IS THE TRIPWIRE for the transcript, because a read that finds
    // nothing leaves no other trace. The payload states no
    // `last_assistant_message`, which is what used to hand the reply back
    // before it opened anything at all, and it names a transcript that was
    // never created, so a read that does happen waits out the whole re-read
    // loop. Both knobs are pinned rather than inherited: a default that moved
    // to one attempt or a shorter sleep would put that loop back under the
    // bound and make this green again on a path that reads.
    let sandbox = Sandbox::new("hook-stop-failure-no-condenser");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(
        &bin.join("codex"),
        &format!(
            "touch '{sandbox}/codex.ran'; cat >/dev/null; printf 'asking|the condenser ran\\n'",
            sandbox = sandbox.display()
        ),
    );
    let mut command = sandbox.pns();
    command
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"))
        .env("PNS_REPLY_REREAD_ATTEMPTS", "4")
        .env("PNS_REPLY_REREAD_INTERVAL", "2");
    prepend_path(&mut command, &bin);
    let mut child = spawn_hook(command, "stop-failure");
    write_payload(
        &mut child,
        format!(
            r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{}","error":"API Error: 500"}}"#,
            sandbox.path("never-written.jsonl").display()
        )
        .as_bytes(),
    );
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "a dead turn that sat through eight seconds of sleeps read the transcript"
    );
    assert!(
        !sandbox.path("codex.ran").exists(),
        "no model call on the one path where a model call has just failed"
    );
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "failed", "no verdict may restate it");
    assert_eq!(
        event["detail"], "API Error: 500",
        "neither the transcript nor a partial reply stands in for the error"
    );
}

#[test]
fn a_failed_turn_never_reaches_moshi() {
    // THE SAME TRIPWIRE ONE ARM OVER. A "moshi is just another channel" sweep
    // takes StopFailure in the edit that takes Stop, and a dead turn is even
    // less of a question than a finished one: no prompt is waiting on an
    // Allow, so a card offering one asks about something nothing is listening
    // to, and its answer would come back as an exit code on a path whose
    // contract is always zero.
    //
    // MECHANISM-BOUND, IN THE DANGEROUS DIRECTION: read through `submissions`
    // for the reason the Stop twin above states.
    let sandbox = Sandbox::new("hook-stop-failure-no-round-trip");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(
        command,
        &sandbox,
        "stop-failure",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","error":"API Error: 500"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        submissions(&sandbox).is_empty(),
        "a turn that died is news, not a question for the phone"
    );
    // Absence alone would also be green for an arm that does nothing at all.
    assert!(
        sandbox.fired("hermes"),
        "and the operator still hears that the turn died"
    );
}
