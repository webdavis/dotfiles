use super::*;

// --- the turn marker --------------------------------------------------------

#[test]
fn the_first_prompt_of_a_turn_writes_a_marker_and_a_later_one_does_not_reset_it() {
    let sandbox = Sandbox::new("hook-prompt-marker");
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );
    let first = std::fs::read_to_string(marker(&sandbox, "s1")).expect("a marker");
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );
    assert_eq!(
        std::fs::read_to_string(marker(&sandbox, "s1")).expect("still a marker"),
        first,
        "a second prompt inside one turn must not restart the clock"
    );
}

#[test]
fn a_session_id_carrying_a_path_traversal_never_becomes_a_filename() {
    let sandbox = Sandbox::new("hook-prompt-traversal");
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"../../etc/passwd"}"#,
    );
    assert!(output.status.success());
    assert!(
        !sandbox.path("state").exists()
            || std::fs::read_dir(sandbox.path("state")).unwrap().count() == 0,
        "nothing may be written for an id that is not a name"
    );
}

#[test]
fn a_payload_with_no_session_id_is_a_silent_no_op() {
    let sandbox = Sandbox::new("hook-prompt-no-session");
    let output = hook_with(with_state_dir(&sandbox), &sandbox, "prompt", r#"{}"#);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn stopping_consumes_the_marker_so_a_second_stop_cannot_re_fire_the_tier() {
    let sandbox = Sandbox::new("hook-stop-consumes");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "1").expect("marker");
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s1","last_assistant_message":"done here"}"#,
    );
    assert!(
        !marker(&sandbox, "s1").exists(),
        "the marker is consumed, not left for the next turn"
    );
}

#[test]
fn a_corrupt_marker_declines_rather_than_crashing_and_is_still_consumed() {
    let sandbox = Sandbox::new("hook-stop-corrupt");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "not-a-timestamp").expect("marker");
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s1","last_assistant_message":"done here"}"#,
    );
    assert!(
        output.status.success(),
        "a hand-edited marker is not a crash"
    );
    assert!(
        !marker(&sandbox, "s1").exists(),
        "and it cannot wedge later turns"
    );
}

// --- the twins sol found weaker ---------------------------------------------

#[test]
fn a_second_stop_cannot_re_fire_the_tier_because_the_marker_is_claimed_once() {
    // Run Stop TWICE through the real path: the first claims the marker, the
    // second finds nothing and cannot report a long turn.
    let sandbox = Sandbox::new("hook-stop-twice");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "1").expect("marker");
    let payload = r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#;
    hook_with(with_state_dir(&sandbox), &sandbox, "stop", payload);
    std::fs::remove_file(sandbox.path("hermes.event")).expect("clear");
    hook_with(with_state_dir(&sandbox), &sandbox, "stop", payload);
    assert!(
        sandbox.path("hermes.event").exists(),
        "the second Stop still notifies"
    );
    assert!(
        !marker(&sandbox, "s1").exists(),
        "and the marker stays consumed"
    );
}

#[test]
fn a_prompt_arriving_while_the_previous_stop_condenses_keeps_its_own_marker() {
    // Stop is asynchronous. Consuming the marker at the END meant a prompt
    // submitted during a slow condenser saw the old marker, wrote nothing,
    // and then had its clock deleted by the Stop that was still running.
    let sandbox = Sandbox::new("hook-prompt-during-condense");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    // A HANDSHAKE, not a fixed sleep: the stub signals "condensing" the
    // instant it starts and blocks on "release" rather than a timed sleep,
    // so "mid-condense" is a fact this test observes instead of a duration
    // it guesses (the pattern at dispatch.rs's summarizer-parks test).
    // BOUNDED ANYWAY at ten seconds, so a broken build fails rather than
    // hangs.
    write_script(
        &bin.join("codex"),
        &format!(
            "cat >/dev/null\n\
             touch \"{root}/condensing\"\n\
             for _ in $(seq 1 200); do [ -e \"{root}/release\" ] && break; sleep 0.05; done\n\
             printf 'done|late\\n'",
            root = sandbox.display()
        ),
    );
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "1").expect("marker");

    let mut slow = with_state_dir(&sandbox);
    slow.env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"));
    let mut stop = spawn_hook(slow, "stop");
    write_payload(
        &mut stop,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    // The next prompt lands mid-condense: proven by the stub's own signal
    // rather than a guess about how long condensing takes.
    assert!(
        support::poll_until(|| sandbox.path("condensing").exists().then_some(())).is_some(),
        "the condenser never started"
    );
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );
    // THE HANDSHAKE'S OWN PRECONDITION: nothing below proves Prompt ran
    // while Stop was still condensing unless Stop is provably still alive
    // right here, before the release is written. Without this, a stub that
    // fell through early would still leave the persistent "condensing" file
    // behind and let a consume-at-end regression pass unnoticed.
    assert!(
        stop.try_wait().expect("poll").is_none(),
        "Stop must still be running when Prompt returns, or this test proves \
         nothing about the handshake"
    );
    std::fs::write(sandbox.path("release"), "").expect("the release");
    assert_eq!(finished_within(stop, HANG_LIMIT), Some(0));
    assert!(
        marker(&sandbox, "s1").exists(),
        "the new turn's clock must survive the previous Stop finishing"
    );
}
