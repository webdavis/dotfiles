//! The harness hooks, end to end: a payload on stdin becomes the same event
//! any other caller would produce, and a blocking one becomes the operator's
//! decision. These are the twins of the bats suites the bash hooks carried.

mod support;

use std::io::Write;
use std::process::{Command, Stdio};
use support::{Sandbox, write_script};

/// One hook run: the payload on stdin, the output back.
fn hook(sandbox: &Sandbox, event: &str, payload: &str) -> std::process::Output {
    hook_with(sandbox.pns(), sandbox, event, payload)
}

fn hook_with(
    mut command: Command,
    _sandbox: &Sandbox,
    event: &str,
    payload: &str,
) -> std::process::Output {
    let mut child = command
        .args(["hook", event])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the engine runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload");
    child.wait_with_output().expect("output")
}

fn marker(sandbox: &Sandbox, session: &str) -> std::path::PathBuf {
    sandbox.path(&format!("state/session-{session}.start"))
}

fn with_state_dir(sandbox: &Sandbox) -> Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    command
}

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

// --- what the turn said -----------------------------------------------------

#[test]
fn the_payloads_own_final_text_becomes_the_detail_without_reading_a_transcript() {
    let sandbox = Sandbox::new("hook-stop-payload-reply");
    hook(
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"/nonexistent","last_assistant_message":"the payload reply"}"#,
    );
    let event = sandbox.event("hermes");
    assert_eq!(event["detail"], "the payload reply");
    assert_eq!(event["project"], "dotfiles");
}

#[test]
fn the_transcript_tail_is_the_fallback_when_the_harness_carried_no_text() {
    let sandbox = Sandbox::new("hook-stop-transcript");
    let transcript = sandbox.path("t.jsonl");
    std::fs::write(
        &transcript,
        "{\"type\":\"user\",\"message\":{\"content\":\"ask\"}}\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"from the transcript\"}]}}\n",
    )
    .expect("transcript");
    hook(
        &sandbox,
        "stop",
        &format!(
            r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{}"}}"#,
            transcript.display()
        ),
    );
    assert_eq!(sandbox.event("hermes")["detail"], "from the transcript");
}

#[test]
fn a_turn_with_nothing_readable_still_notifies_with_no_detail() {
    let sandbox = Sandbox::new("hook-stop-empty");
    let output = hook(
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"   "}"#,
    );
    assert!(output.status.success());
    let event = sandbox.event("hermes");
    assert_eq!(event["detail"], "");
    assert_eq!(event["state"], "done");
}

#[test]
fn a_condenser_line_is_used_state_and_all_and_a_blank_summary_falls_back() {
    let sandbox = Sandbox::new("hook-stop-condenser");
    let mut command = sandbox.pns();
    sandbox.stub_codex(&mut command, "asking|it wants a choice");
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a long turn"}"#,
    );
    let event = sandbox.event("hermes");
    assert_eq!(
        event["state"], "asking",
        "the condenser may override the state"
    );
    assert_eq!(event["detail"], "it wants a choice");

    let blank = Sandbox::new("hook-stop-condenser-blank");
    let mut command = blank.pns();
    blank.stub_codex(&mut command, "done|   ");
    hook_with(
        command,
        &blank,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a long turn"}"#,
    );
    assert_eq!(
        blank.event("hermes")["detail"],
        "a long turn",
        "a summary of spaces is as blank as no summary, so the reply stands"
    );
}

#[test]
fn the_re_entry_guard_keeps_a_condenser_run_from_condensing_itself() {
    let sandbox = Sandbox::new("hook-stop-reentry");
    let mut command = sandbox.pns();
    command.env("PNS_SUMMARIZING", "1");
    sandbox.stub_codex(&mut command, "asking|never asked");
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert_eq!(sandbox.event("hermes")["detail"], "a turn");
}

#[test]
fn the_herdr_pane_reaches_the_event_verbatim_and_a_hostile_one_is_scrubbed_downstream() {
    let sandbox = Sandbox::new("hook-stop-pane");
    let mut command = sandbox.pns();
    command.env("HERDR_PANE_ID", "wW:p21");
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#,
    );
    assert_eq!(sandbox.event("hermes")["pane"], "wW:p21");
}

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

// --- the blocking round trip ------------------------------------------------

#[test]
fn a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision() {
    let sandbox = Sandbox::new("hook-blocked-forward");
    let mut command = sandbox.pns();
    // Away, so the phone is the only way to answer.
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let payload = "{\"message\":\"may I\",\"session_id\":\"s1\"}\n";
    let output = hook_with(command, &sandbox, "blocked", payload);

    assert_eq!(
        output.status.code(),
        Some(42),
        "the exit code IS the operator's decision and must not be swallowed"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        payload,
        "a consumed-but-not-forwarded stream leaves moshi with an empty parse"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("moshi argv")
            .trim(),
        "claude-hook"
    );
}

#[test]
fn the_notification_still_goes_out_while_moshi_holds_the_card_but_not_to_the_phone() {
    let sandbox = Sandbox::new("hook-blocked-notifies");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 0);
    hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert!(sandbox.fired("hermes"), "the paper trail is still written");
    assert!(
        !sandbox.fired("moshi"),
        "moshi is raising the card itself; pns pushing too is the same event twice"
    );
}

#[test]
fn at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual() {
    let sandbox = Sandbox::new("hook-blocked-desk");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(0), "no opinion: prompt as usual");
    assert!(
        !sandbox.path("moshi.argv").exists(),
        "the operator is right here; the card would be noise"
    );
}

#[test]
fn a_phone_used_more_recently_than_the_desk_gets_the_approval_forwarded_to_it() {
    // THE GATE READS THE SAME ARBITRATION as the delivery plan, so the
    // phone-input amendment reaches it with no wiring of its own: the desk
    // was touched 90s ago and is still inside the freshness window, but the
    // phone was touched 5s ago and that is where the operator can answer.
    let sandbox = Sandbox::new("hook-blocked-phone-fresher");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "90")
        .env("PNS_PHONE_INPUT_AGE", "5");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert!(sandbox.path("moshi.argv").exists());
}

#[test]
fn moshi_not_being_installed_leaves_the_hook_a_silent_exit_zero() {
    // The card is suppressed for a round trip that DUPLICATES it, and this
    // one never started: an away operator with no moshi-hook installed lost
    // the only notification that could still reach them (sol, 2026-08-19).
    let sandbox = Sandbox::new("hook-blocked-no-moshi");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("MOSHI_HOOK_BIN", "/nonexistent/moshi-hook");
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(0));
    assert!(sandbox.fired("hermes"), "the notification still goes out");
    assert!(
        sandbox.fired("moshi"),
        "a forward that never spawned suppresses nothing"
    );
}

#[test]
fn a_payload_too_large_to_be_whole_is_never_forwarded_as_though_it_were() {
    // The reader caps stdin, so an over-cap payload is TRUNCATED mid-object.
    // Forwarding it hands moshi invalid JSON, which is the empty parse the
    // byte-for-byte contract exists to prevent; measured 2026-08-19 as
    // exactly 1,000,000 bytes forwarded out of a 1.2MB payload.
    let sandbox = Sandbox::new("hook-blocked-oversized");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let mut child = spawn_hook(command, "blocked");
    let payload = format!(r#"{{"message":"{}"}}"#, "x".repeat(1_200_000));
    write_payload(&mut child, payload.as_bytes());
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "an over-cap payload is not the operator's decision"
    );
    assert!(
        !sandbox.path("moshi.argv").exists(),
        "half an object must never reach moshi"
    );
    assert!(
        sandbox.fired("hermes"),
        "and the operator still hears that something is blocked"
    );
}

#[test]
fn a_moshi_that_never_reads_its_stdin_cannot_hold_the_notification() {
    // The write ran on this thread, so a child that does not read blocked it
    // once the pipe buffer filled: the permission request hung BEFORE the
    // notification went out and before the wait that is meant to be the only
    // place this waits on a person.
    let sandbox = Sandbox::new("hook-blocked-deaf-moshi");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(&bin.join("moshi-hook"), "sleep 30");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
    let mut child = spawn_hook(command, "blocked");
    // Past the 64KB pipe buffer, which is what turns a child that does not
    // read into a writer that never returns.
    let payload = format!(r#"{{"message":"{}"}}"#, "x".repeat(200_000));
    write_payload(&mut child, payload.as_bytes());
    let deadline = std::time::Instant::now() + HANG_LIMIT;
    while !sandbox.fired("hermes") && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let notified = sandbox.fired("hermes");
    // The hook is still waiting on the "human" by design, so the test ends it.
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        notified,
        "the notification must not wait on a child that never reads"
    );
}

#[test]
fn a_harness_pns_does_not_register_for_is_never_handed_to_moshi() {
    let sandbox = Sandbox::new("hook-blocked-unknown-agent");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999").env("PNS_AGENT", "pi");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(0));
    assert!(!sandbox.path("moshi.argv").exists());
}

// --- what the submission path owes the operator ------------------------------
//
// THE NO-LOST-BEHAVIOR GATE. Everything below passes on today's build by
// construction: each one pins something the submission path already does, so
// that a later rewrite of HOW pns reaches moshi has to keep delivering it.
//
// MOST OF THESE GUARDS ARE MECHANISM-BOUND, AND SAYING SO IS THE POINT. A
// test that reads the submission record is reading a file that exists only
// because the submission is a child process running a stub, and a transport
// change does not re-satisfy that assertion: it invalidates it. Nine of the
// guards this gate adds are in that position, in two directions that fail
// very differently.
//
// RED AT THE SWITCH, which is the safe direction, because item 25 sees the
// failure and rewrites the assertion against whatever records an endpoint
// submission:
//   one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve
//   an_approval_is_forwarded_even_when_the_moshi_channel_is_switched_off
//   an_approval_is_forwarded_even_with_the_pane_in_plain_sight
//   a_payload_at_the_cap_is_whole_and_is_still_submitted
//   the_gate_submits_one_prompt_exactly_once
//
// VACUOUS AT THE SWITCH unless re-pointed, because ABSENCE is what they
// assert and an absent file is absently true for every build, including one
// that cards the operator for every finished turn:
//   an_ordinary_stop_never_reaches_moshi
//   a_failed_turn_never_reaches_moshi
//   at_the_desk_the_gate_submits_nothing_and_exits_zero
//   the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does
//
// Those four read through `submissions`, deliberately: item 25 re-points ONE
// function at whatever the new transport records and all four keep guarding.
// Spelled as a filename they would fail open, silently, on the very switch
// this gate exists to guard.
//
// The rest assert something a transport change re-satisfies rather than
// invalidates (the card and what it says, the exit code, a submission that
// died), except the two that pin plumbing ON PURPOSE, the inherited
// environment and the inherited stdout, because those are the design
// questions a transport change has to answer deliberately.

#[test]
fn a_blocked_hook_cards_the_operator_as_blocked_and_says_what_was_asked() {
    // Every sibling arm pins its own card (`failed`, `denied`, `asked`); the
    // blocked one, the oldest, never did. The state word is asserted EXACTLY,
    // because nothing in the crate validates one and a typo would ship
    // silently.
    //
    // READ OFF THE HERMES STUB. The moshi CHANNEL (the in-process push, which
    // is a different thing from the moshi-hook submission this section is
    // about) is suppressed on this path, so the durable leg is the only one
    // still carrying the card.
    let sandbox = Sandbox::new("hook-blocked-card");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("HERDR_PANE_ID", "wY:p4");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(
        command,
        &sandbox,
        "blocked",
        r#"{"message":"may I run this","session_id":"s1","cwd":"/a/dotfiles"}"#,
    );
    assert_eq!(output.status.code(), Some(42));
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "blocked");
    assert_eq!(
        event["detail"], "may I run this",
        "what was asked is the question a blocked card has to answer"
    );
    assert_eq!(event["project"], "dotfiles");
    // The pane rides the card so a click lands on the pane that is waiting.
    assert_eq!(event["pane"], "wY:p4");
}

#[test]
fn one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve() {
    // TWO HALVES OF ONE SENTENCE: the submission really happened AND its zero
    // came back. Either alone is weaker than the pair. A count of one is the
    // operator's single-submitter rule made checkable, on the exact seam a
    // second submitter would appear at; the zero is what keeps that count from
    // being satisfied by a build that submitted nothing and defaulted.
    //
    // The exit code is a live contract for the harnesses that reach the gate
    // directly and read it. Claude Code does not honor a PermissionRequest
    // hook's exit code (the answer travels moshi's own bridge), so for THIS
    // path it is a forward-compatibility guarantee rather than today's
    // mechanism.
    //
    // MECHANISM-BOUND: the count is read off the submission record, so this
    // goes RED at the endpoint switch and item 25's duty is to rewrite it
    // against the endpoint's own record. Red is the duty being discharged,
    // not a regression.
    let sandbox = Sandbox::new("hook-blocked-single-submitter");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 0);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a zero from a submission that happened is an approve, not a default"
    );
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "one prompt, one submission: a second card is a second answer nobody gave"
    );
}

#[test]
fn an_approval_is_forwarded_even_when_the_moshi_channel_is_switched_off() {
    // The forward is independent of plugin selection AND of the push token,
    // and neither is a coincidence worth leaving unpinned: a submission built
    // on the moshi channel's own config would couple the two silently, and an
    // operator who never set a token would lose approvals while every test
    // stayed green.
    //
    // MECHANISM-BOUND: the submission is read off the record, so this goes
    // RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("hook-blocked-channel-off");
    sandbox.write_config("[plugins.moshi]\nenabled = false\n[plugins.hermes]\nenabled = true\n");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert!(
        sandbox.path("moshi.argv").exists(),
        "a channel the operator turned off is not an approval they declined"
    );
    // THE ABSENT CARD IS NOT EVIDENCE THE CONFIG WAS READ. A forward that
    // happened suppresses pns's own phone leg whatever the config says, and
    // the same absence shows up with the channel enabled and with no config
    // file at all (measured, three ways). This line pins that no second card
    // appeared, and nothing about what silenced it; the exit code and the
    // submission above are what carry the selection exemption.
    assert!(!sandbox.fired("moshi"));
    assert!(sandbox.fired("hermes"), "the paper trail is still written");
}

#[test]
fn an_approval_is_forwarded_even_with_the_pane_in_plain_sight() {
    // VISIBILITY GATES NOTIFICATIONS AND NEVER APPROVALS, and today that is
    // STRUCTURAL rather than stated: `operator_surface`'s trait bound carries
    // no session-view probe at all, so the forward cannot consult one. A
    // refactor that routed the forward through the delivery decision would
    // break it without editing a line anyone reviewed, which is what this
    // catches. The mute twin exists for the same reason and this is its
    // sibling.
    //
    // WHAT THIS DOES NOT PROVE: `stub_herdr` records nothing, so nothing here
    // distinguishes "the session view answered Visible" from "it was never
    // consulted at all". The visibility READ is exercised by some twenty
    // tests in the dispatch suite; what this pins is the forward's
    // INDIFFERENCE to it, which is the half those tests cannot see.
    //
    // MECHANISM-BOUND: the submission is read off the record, so this goes
    // RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("hook-blocked-pane-visible");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("HERDR_PANE_ID", "t1:p1");
    sandbox.stub_herdr(&mut command, true);
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert!(
        sandbox.path("moshi.argv").exists(),
        "a pane in plain sight is not an answer to the prompt on it"
    );
}

#[test]
fn the_submission_inherits_the_callers_environment() {
    // MECHANISM, PINNED ON PURPOSE. moshi-hook resolves its own host identity
    // out of the environment it inherits (HOME, and its config from there), so
    // the whole environment crossing this seam is load-bearing today. A
    // submission that carries no environment has to answer what carries the
    // host identity instead, and this is the pin that makes that question
    // unavoidable rather than implicit.
    //
    // `Sandbox::bare` clears the environment and puts back only HOME and PATH,
    // so the reading is the sandbox's own rather than the developer's.
    let sandbox = Sandbox::new("hook-blocked-env");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(
        &bin.join("moshi-hook"),
        &format!(
            "printf '%s\\n' \"$HOME\" \"$MOSHI_ENV_PROBE\" >\"{sandbox}/moshi.env\"; \
             cat >/dev/null; exit 42",
            sandbox = sandbox.display()
        ),
    );
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("MOSHI_HOOK_BIN", bin.join("moshi-hook"))
        .env("MOSHI_ENV_PROBE", "inherited");
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42));
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.env")).expect("the child's environment"),
        format!("{}\ninherited\n", sandbox.display()),
        "the child reads the caller's own environment, HOME included"
    );
}

#[test]
fn what_moshi_says_on_stdout_reaches_the_harness_unchanged() {
    // MECHANISM, PINNED ON PURPOSE, and the SECOND design question: only stdin
    // is piped, so the child's stdout IS the hook's stdout and pns is a plain
    // pipe on this seam. Claude Code reads a PermissionRequest hook's stdout,
    // so the channel from moshi back to the harness is open end to end today
    // and a submission that carries no stream has to answer what replaces it.
    //
    // THIS IS ALSO THE PROXY FOR THE UNBOUNDED WAIT. Proving the absence of a
    // deadline needs a test slower than the deadline, which this suite will
    // not carry. The likeliest way to lose it is routing the submission
    // through `run_bounded`, which pipes the child's stdout on its way to
    // attaching a deadline, so the stream this reads is the half of that
    // change an assertion can see. A deadline added WITHOUT touching the
    // stream wiring still passes here, and that residual is stated rather than
    // papered over.
    let sandbox = Sandbox::new("hook-blocked-stdout");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(
        &bin.join("moshi-hook"),
        "cat >/dev/null; printf 'moshi answered here\\n'; exit 42",
    );
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(output.status.code(), Some(42));
    let printed = String::from_utf8_lossy(&output.stdout);
    // VERBATIM, not merely present. Moshi's line has to arrive as ITS OWN
    // line, exactly once, with nothing added to either end: a `contains`
    // would pass a pns that prefixed it, wrapped it or printed it twice, and
    // each of those is a different answer by the time the harness parses it.
    assert_eq!(
        printed
            .lines()
            .filter(|line| *line == "moshi answered here")
            .count(),
        1,
        "moshi's answer did not arrive unchanged and exactly once: {printed:?}"
    );
}

#[test]
fn a_submission_that_dies_without_answering_is_not_a_decision() {
    // NO ANSWER IS NO OPINION. A child killed by a signal yields no exit code
    // at all, and reading that as anything but zero would refuse a tool call
    // the operator never refused. The analogue over any other transport is a
    // dropped connection, so the invariant outlives the pipe.
    let sandbox = Sandbox::new("hook-blocked-signalled");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    // THE MARKER IS WRITTEN BEFORE THE KILL, because a stub that never ran at
    // all produces this test's exit 0 and this test's card just as well as a
    // stub that died mid-answer, and only one of those is the behavior under
    // test.
    write_script(
        &bin.join("moshi-hook"),
        &format!(
            "printf 'ran\\n' >\"{sandbox}/moshi.started\"; cat >/dev/null; kill -TERM $$",
            sandbox = sandbox.display()
        ),
    );
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
    let output = hook_with(command, &sandbox, "blocked", r#"{"message":"may I"}"#);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a submission that died is not the operator's decision"
    );
    assert!(
        sandbox.path("moshi.started").exists(),
        "the submission has to have STARTED for its death to be what was read"
    );
    // Absence alone would also be green for a build that submitted nothing.
    assert!(
        sandbox.fired("hermes"),
        "and the operator still hears that something is blocked"
    );
}

#[test]
fn a_payload_at_the_cap_is_whole_and_is_still_submitted() {
    // THE OTHER HALF OF THE CAP. Every cap test in this file sends 1.2MB, so
    // all of them agree about what must NOT be submitted and none of them
    // says what must. A reader capped one byte lower, or a comparison that
    // turned strict, stops forwarding legitimate megabyte payloads while
    // every one of those tests stays green and approvals quietly stop
    // arriving. Exactly at the cap is the only place that edge is visible.
    //
    // MECHANISM-BOUND: the submission is read off the record, so this goes
    // RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("hook-blocked-at-cap");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let mut child = spawn_hook(command, "blocked");
    let payload = format!(r#"{{"message":"{}"}}"#, "x".repeat(999_986));
    assert_eq!(payload.len(), 1_000_000, "the test's own arithmetic");
    write_payload(&mut child, payload.as_bytes());
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(42),
        "a payload that arrived whole is the operator's to answer"
    );
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "the last byte that still fits is still a whole payload"
    );
}

// --- the tool call the harness refused --------------------------------------

#[test]
fn a_refused_tool_call_notifies_as_denied_and_says_which_tool_was_refused() {
    // Claude Code refuses a tool call on its own when the auto-mode classifier
    // rules against it, and until this arm existed the operator learned that
    // only by reading the pane. THE PAYLOAD IS THE BINARY'S OWN FIELD SET
    // (`tool_name`, `tool_input`, `tool_use_id` and `reason` over the base
    // spread every other event shares): it states no `message`, no `detail`
    // and no `error`, which is what sends the existing chain through to the
    // tool request rather than to a field this event never sends. The state
    // word is asserted EXACTLY, because nothing in the crate validates one and
    // a typo would otherwise ship silently.
    let sandbox = Sandbox::new("hook-denied");
    let mut command = sandbox.pns();
    command.env("HERDR_PANE_ID", "wY:p4");
    let output = hook_with(
        command,
        &sandbox,
        "denied",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"},"tool_use_id":"toolu_01","reason":"the auto-mode classifier refused it"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "denied");
    assert_eq!(
        event["detail"], "Bash: command=rm -rf /tmp/x",
        "which tool wanted what is the question a refused card has to answer"
    );
    assert_eq!(event["project"], "dotfiles");
    // The pane rides the card so a click lands on the pane that was refused.
    assert_eq!(event["pane"], "wY:p4");
}

#[test]
fn a_refused_tool_call_leaves_the_turn_marker_alone() {
    // THE TURN CONTINUES PAST A DENIAL: the harness hands the refusal back to
    // the model as a tool result and the turn ends later, at the Stop or the
    // StopFailure that follows. Consuming the marker here would restart the
    // clock mid-turn, so a long turn holding one denial would report itself
    // short and lose the pulse and the mobile watch card the tier raises. The
    // inverse of
    // `a_dead_turn_consumes_the_marker_so_the_next_turn_is_not_measured_from_its_start`.
    let sandbox = Sandbox::new("hook-denied-marker");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "1755000000").expect("marker");
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "denied",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","tool_name":"Bash","tool_input":{"command":"ls"},"tool_use_id":"toolu_02","reason":"refused"}"#,
    );
    assert_eq!(
        std::fs::read_to_string(marker(&sandbox, "s1")).expect("the marker survives the denial"),
        "1755000000",
        "the turn a denial interrupted is still measured by the Stop that ends it"
    );
}

#[test]
fn a_denial_never_pays_for_the_approval_round_trip_and_still_exits_zero() {
    // There is nothing left to approve: the decision has been taken, and a
    // moshi card offering Allow and Deny would be answering a closed question
    // no prompt is waiting on. THE STUB IS THE TRIPWIRE. It records its argv
    // and exits 42, so an arm routed through the blocking path would both
    // leave that file behind and hand the 42 back as an operator decision, on
    // the one path whose contract says it always returns 0. AWAY is what arms
    // it, and every sandbox is away already: `Sandbox::pns` sets the idle
    // clock. At the desk the forward declines on its own and the tripwire
    // would stay un-run for the wrong reason.
    let sandbox = Sandbox::new("hook-denied-no-round-trip");
    let mut command = sandbox.pns();
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(
        command,
        &sandbox,
        "denied",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","tool_name":"Bash","tool_input":{"command":"ls"},"tool_use_id":"toolu_03","reason":"refused"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        !sandbox.path("moshi.argv").exists(),
        "a denial is terminal news, not a question for the phone"
    );
    // Absence alone would also be green for an arm that does nothing at all.
    assert!(
        sandbox.fired("hermes"),
        "and the operator still hears about it"
    );
}

// --- the server that stopped to ask -----------------------------------------

/// An Elicitation payload, the binary's own field set: `mcp_server_name` and
/// `message` required, `mode`, `url`, `elicitation_id` and `requested_schema`
/// optional, over the base spread every other event shares. Shared by the two
/// tests below so "the same payload" is one string rather than two that drift.
const ELICITATION: &str = r#"{"hook_event_name":"Elicitation","session_id":"s1","cwd":"/a/dotfiles","mcp_server_name":"composio","message":"Please authorize Gmail access","mode":"url","url":"https://backend.composio.dev/authorize/abc123","elicitation_id":"elic_01","requested_schema":{"api_key":{"type":"string"}}}"#;

#[test]
fn an_mcp_server_waiting_on_input_notifies_as_asked_and_names_the_server() {
    // A connected MCP server can stop mid-tool-call and hold it open until
    // the operator fills a form or opens an authorization link, and until
    // this the pane stalling on a Composio authorize looked identical to a
    // pane that was thinking. The state word is asserted EXACTLY, because
    // nothing in the crate validates one and a typo would otherwise ship
    // silently.
    let sandbox = Sandbox::new("hook-elicitation");
    let mut command = sandbox.pns();
    command.env("HERDR_PANE_ID", "wY:p4");
    let output = hook_with(command, &sandbox, "asked", ELICITATION);
    assert_eq!(output.status.code(), Some(0));
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "asked");
    assert_eq!(
        event["detail"], "composio: Please authorize Gmail access",
        "which server wants what is the question a stalled card has to answer"
    );
    assert_eq!(event["project"], "dotfiles");
    // The pane rides the card so a click lands on the pane that is stalled.
    assert_eq!(event["pane"], "wY:p4");
}

#[test]
fn the_hook_writes_nothing_the_harness_could_read_as_an_answer_and_exits_zero() {
    // A GUARD RATHER THAN A RED-FIRST BEHAVIOR: it passes today, and its job
    // is to keep passing. It earns its place because the failure it prevents
    // is silent and lands in SOMEONE ELSE'S system. Claude Code awaits this
    // hook and reads a decision out of it before the dialog is ever shown:
    // stdout whose trimmed text begins with `{` is parsed as the operator's
    // answer, and exit code 2 alone declines the elicitation outright, so the
    // MCP server would report a refusal the operator never made and nothing
    // anywhere would say why. pns writes plain `pns: ...` delivery lines and
    // returns 0 on every notification path. The assertion mirrors the
    // harness's own reader, which trims before it looks at the first
    // character, so empty stdout and prose stdout are the same pass.
    let sandbox = Sandbox::new("hook-elicitation-answers-nothing");
    let output = hook(&sandbox, "asked", ELICITATION);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a non-zero exit is a decision this hook has no business taking"
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        !printed.trim().starts_with('{'),
        "stdout the harness would parse as an elicitation answer: {printed:?}"
    );
    // Absence alone would also be green for an arm that does nothing at all.
    assert!(
        sandbox.fired("hermes"),
        "and the operator still hears that a server is waiting on them"
    );
}

// --- the other harness events -----------------------------------------------

#[test]
fn a_non_blocking_event_never_pays_for_the_round_trip() {
    let sandbox = Sandbox::new("hook-asked");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let output = hook_with(command, &sandbox, "asked", r#"{"message":"which one?"}"#);
    assert_eq!(output.status.code(), Some(0));
    assert!(!sandbox.path("moshi.argv").exists());
    assert_eq!(sandbox.event("hermes")["detail"], "which one?");
    assert_eq!(sandbox.event("hermes")["state"], "asked");
}

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

#[test]
fn a_garbage_re_read_knob_still_notifies_and_still_exits_zero() {
    let sandbox = Sandbox::new("hook-garbage-knob");
    let mut command = sandbox.pns();
    command.env("PNS_REPLY_REREAD_ATTEMPTS", "not-a-number");
    let output = hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(sandbox.event("hermes")["detail"], "a turn");
}

#[test]
fn a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer() {
    // THE EXEMPTION IS STRUCTURAL: the forward runs in `blocking_event`, which
    // builds its own overrides and never constructs a delivery plan, so the
    // mute cannot reach it. Structural means it can be broken by moving code
    // rather than by editing a line this feature added, which is what this
    // pins. A muted operator who blocks on a permission prompt still gets the
    // card and still answers it; only pns's own duplicate notification about
    // that block goes quiet.
    let sandbox = Sandbox::new("hook-blocked-muted");
    let mut command = with_state_dir(&sandbox);
    // Away, so the phone is the only way to answer at all.
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
        + 600;
    let quiet_until = sandbox.path("state/quiet-until");
    std::fs::write(&quiet_until, format!("{expiry}\n")).expect("the mute");

    let payload = "{\"message\":\"may I\",\"session_id\":\"s1\"}\n";
    let output = hook_with(command, &sandbox, "blocked", payload);

    assert_eq!(
        output.status.code(),
        Some(42),
        "the exit code IS the operator's decision, muted or not"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        payload,
        "byte for byte: a consumed-but-not-forwarded stream leaves moshi with an empty parse"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("moshi argv")
            .trim(),
        "claude-hook"
    );
    // The paper trail is written. The ABSENT CARD IS NOT EVIDENCE OF THE MUTE:
    // the unmuted control produces the same two legs, because the forward's own
    // skip suppresses pns's phone leg either way. This line pins that no second
    // card appeared, and nothing about what silenced it; the pins above are
    // what carry the exemption.
    assert!(sandbox.fired("hermes"), "the durable log is never muted");
    assert!(!sandbox.fired("moshi"));
    assert!(
        std::fs::read_to_string(&quiet_until)
            .expect("the mute survives")
            .trim()
            == expiry.to_string(),
        "the mute is untouched by the event it did not suppress"
    );
}

#[test]
fn a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer() {
    // A STRUCTURAL GUARD, and it says so about itself: it passes the moment
    // the field exists, and its whole job is to keep passing. `blocking_event`
    // decides the forward through `forward_to_moshi`, which reads the presence
    // probes and never constructs a delivery plan, so nothing on `Overrides`
    // can reach it. That guarantee breaks by MOVING code rather than by
    // editing a line this feature added, which no unit test can observe.
    //
    // THE NEAR DUPLICATE OF THE MUTE'S OWN TEST IS DELIBERATE. That one would
    // keep passing on the day a Focus started suppressing approvals, and this
    // is a safety property: an operator inside a Focus who blocks on a
    // permission prompt still gets the card and still answers it.
    let sandbox = Sandbox::new("hook-blocked-focus");
    let mut command = with_state_dir(&sandbox);
    // Away, so the phone is the only way to answer at all.
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    sandbox.write_focus_store("com.apple.sleep.sleep-mode", "Sleep");
    sandbox.write_config(
        "[plugins.moshi]\nenabled = true\n[plugins.hermes]\nenabled = true\n\
         [plugins.macos-banner]\nenabled = true\n[focus]\nsilence = [\"Sleep\"]\n",
    );

    let payload = "{\"message\":\"may I\",\"session_id\":\"s1\"}\n";
    let output = hook_with(command, &sandbox, "blocked", payload);

    assert_eq!(
        output.status.code(),
        Some(42),
        "the exit code IS the operator's decision, Focus or not"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        payload,
        "byte for byte: a consumed-but-not-forwarded stream leaves moshi with an empty parse"
    );
    assert!(
        sandbox.fired("hermes"),
        "the durable log is never silenced by a Focus either"
    );
}

/// Stubs live here rather than in the shared harness: only this suite spawns
/// a condenser or an approval round trip.
trait HookStubs {
    fn stub_codex(&self, command: &mut Command, line: &str);
    fn stub_moshi(&self, command: &mut Command, exit_code: i32);
}

impl HookStubs for Sandbox {
    fn stub_codex(&self, command: &mut Command, line: &str) {
        let bin = self.path("bin");
        std::fs::create_dir_all(&bin).expect("stub bin");
        write_script(
            &bin.join("codex"),
            &format!("cat >/dev/null; printf '%s\\n' '{line}'"),
        );
        prepend_path(command, &bin);
        command.env("CODEX_BIN", bin.join("codex"));
        command.env("PNS_CODEX_HOME", self.path("codex-home"));
    }

    /// THE ARGV FILE APPENDS, one line per spawn, so a SECOND submission of the
    /// same prompt is observable at all. A truncating record answered "what was
    /// the last argv"; the single-submitter rule needs "how many were there".
    /// Of the fifteen readers, FOUR compare contents and all four trim, so one
    /// spawn still yields exactly `claude-hook`; the other eleven only ask
    /// whether anything was recorded at all, and the ones this gate adds ask
    /// through `submissions` below.
    fn stub_moshi(&self, command: &mut Command, exit_code: i32) {
        let bin = self.path("bin");
        std::fs::create_dir_all(&bin).expect("stub bin");
        write_script(
            &bin.join("moshi-hook"),
            &format!(
                "printf '%s\\n' \"$*\" >>\"{sandbox}/moshi.argv\"; cat >\"{sandbox}/moshi.stdin\"; exit {exit_code}",
                sandbox = self.display()
            ),
        );
        command.env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
    }
}

/// Every submission `stub_moshi` recorded, one per line, in the order they
/// were made, and EMPTY when nothing was recorded at all.
///
/// THE EMPTY CASE IS THE LOAD-BEARING ONE. Every "never submitted" assertion
/// in this file reads through here rather than through the record's filename,
/// so the day the submission stops being a child process there is ONE place
/// to re-point at whatever the new transport records. Spelled as a filename,
/// those guards answer "no file, so nothing was submitted" for a build that
/// submits over something else, which is the single regression this gate
/// exists to catch. `tests/dispatch.rs`'s `moshi_hook_argv` reads its own
/// record the same way, for the same reason.
///
/// NO SETTLE, AND THE RESIDUAL IS STATED RATHER THAN SLEPT ON. Every
/// submission the crate makes today is waited on by the process under test,
/// so the record has landed by the time that process exits; this counts what
/// the exiting process left behind. A duplicate that was DETACHED instead of
/// waited on could land after the read, and no sleep short enough for this
/// suite would close that (`tests/support/mod.rs` refuses fixed sleeps for
/// exactly that reason, and a 100ms one here was measured to change nothing).
fn submissions(sandbox: &Sandbox) -> Vec<String> {
    std::fs::read_to_string(sandbox.path("moshi.argv"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn prepend_path(command: &mut Command, directory: &std::path::Path) {
    let mut path = std::ffi::OsString::from(directory);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    command.env("PATH", path);
}

// --- nothing may hang -------------------------------------------------------

/// Every bound below is proved the same way: run the thing against input that
/// would block forever, with a tight injected deadline, and require an answer.
const HANG_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

fn spawn_hook(mut command: Command, event: &str) -> std::process::Child {
    command
        .args(["hook", event])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the engine runs")
}

/// Write the payload and CLOSE the pipe: the reader waits for EOF, so a
/// handle left open is the test hanging itself rather than the hook.
fn write_payload(child: &mut std::process::Child, payload: &[u8]) {
    let mut stdin = child.stdin.take().expect("stdin");
    let _ = stdin.write_all(payload);
}

fn finished_within(mut child: std::process::Child, limit: std::time::Duration) -> Option<i32> {
    let deadline = std::time::Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status.code();
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_transcript_that_never_ends_is_not_read_at_all() {
    // /dev/zero is infinite and a FIFO blocks on open: neither is a regular
    // file, and the check happens before the open for exactly that reason.
    let sandbox = Sandbox::new("hook-transcript-devzero");
    let fifo = sandbox.path("t.fifo");
    assert!(
        std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    for path in ["/dev/zero".to_string(), fifo.display().to_string()] {
        let mut child = spawn_hook(sandbox.pns(), "stop");
        let payload =
            format!(r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{path}"}}"#);
        write_payload(&mut child, payload.as_bytes());
        assert_eq!(
            finished_within(child, HANG_LIMIT),
            Some(0),
            "transcript_path {path} must not hold the hook open"
        );
    }
}

#[test]
fn a_payload_nobody_finishes_writing_still_exits_on_the_contract() {
    // The pipe stays open with nothing in it, which used to hang before any
    // of the exit-0 contract could run.
    let sandbox = Sandbox::new("hook-payload-hang");
    let mut command = sandbox.pns();
    command.env("PNS_PAYLOAD_DEADLINE_MS", "200");
    let child = spawn_hook(command, "stop");
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "no payload is no notification, and still exit 0"
    );
    assert!(!sandbox.fired("hermes"), "and nothing is sent on a guess");
}

#[test]
fn a_condenser_that_closes_stdout_and_sleeps_is_killed_at_its_deadline() {
    // The case the old bound missed entirely: stdout closes, the read
    // finishes, and the wait then blocked with no deadline on it.
    let sandbox = Sandbox::new("hook-condenser-sleeps");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    write_script(&bin.join("codex"), "cat >/dev/null; exec 1>&-; sleep 30");
    let mut command = sandbox.pns();
    command
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"))
        .env("PNS_CONDENSER_DEADLINE_MS", "300");
    let mut child = spawn_hook(command, "stop");
    write_payload(
        &mut child,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    assert_eq!(
        sandbox.event("hermes")["detail"],
        "a turn",
        "an expired condenser falls back to the reply"
    );
}

#[test]
fn a_condenser_that_never_reads_its_stdin_is_bounded_too() {
    // The write is inside the window now: this child never drains the pipe,
    // which used to block before the clock started.
    let sandbox = Sandbox::new("hook-condenser-deaf");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    write_script(&bin.join("codex"), "sleep 30");
    let mut command = sandbox.pns();
    command
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"))
        .env("PNS_CONDENSER_DEADLINE_MS", "300");
    let mut child = spawn_hook(command, "stop");
    let big = "x".repeat(200_000);
    let payload =
        format!(r#"{{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"{big}"}}"#);
    write_payload(&mut child, payload.as_bytes());
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
}

#[test]
fn a_stuck_multiplexer_leaves_the_view_unreadable_rather_than_blocking() {
    // Unknown never suppresses, so a herdr that hangs costs a spare
    // notification; a herdr that hangs the HOOK costs the notification.
    let sandbox = Sandbox::new("hook-herdr-stuck");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    write_script(&bin.join("herdr"), "sleep 30");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "0");
    let mut path = std::ffi::OsString::from(&bin);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    command.env("PATH", path);
    let mut child = spawn_hook(command, "stop");
    write_payload(
        &mut child,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x","transcript_path":""}"#,
    );
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
}

// --- the gate, as a real process --------------------------------------------

/// The gate is reached by the BARE harness word, because moshi's generated
/// extension holds one pathname with no room for a subcommand.
fn gate(sandbox: &Sandbox, word: &str, payload: &str) -> std::process::Output {
    gate_argv(sandbox, &[word], payload)
}

/// The same gate, reached by whatever argv the caller spells: the bare word
/// moshi's extension uses, or the `gate <word>` form the documentation gives
/// an operator.
fn gate_argv(sandbox: &Sandbox, argv: &[&str], payload: &str) -> std::process::Output {
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 7);
    let mut child = command
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the engine runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload");
    child.wait_with_output().expect("output")
}

#[test]
fn the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision() {
    let sandbox = Sandbox::new("gate-forwards");
    let output = gate(&sandbox, "pi-hook", "{\"ask\":1}\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "the decision is the exit code"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read it"),
        "{\"ask\":1}\n"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("argv")
            .trim(),
        "pi-hook"
    );
}

#[test]
fn a_zero_decision_passes_through_as_zero_and_is_not_a_default() {
    let sandbox = Sandbox::new("gate-approves");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 0);
    let mut child = command
        .arg("pi-hook")
        .stdin(Stdio::piped())
        .spawn()
        .expect("runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"{}")
        .expect("payload");
    assert_eq!(child.wait().expect("wait").code(), Some(0));
    assert!(
        sandbox.path("moshi.argv").exists(),
        "an approval reaches moshi; a zero exit is its answer, not a skip"
    );
}

#[test]
fn the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word() {
    // CLAUDE.md gives `pns gate <harness>-hook` as the operator-facing form,
    // and only the bare word was ever implemented: the documented one fell
    // through to EVENT mode, which forwarded nothing and fired a notification
    // about an empty event nobody asked for.
    let sandbox = Sandbox::new("gate-subcommand");
    let output = gate_argv(&sandbox, &["gate", "pi-hook"], "{\"ask\":1}\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "the decision is still the exit code"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.argv"))
            .expect("argv")
            .trim(),
        "pi-hook"
    );
    assert!(
        !sandbox.fired("hermes"),
        "a gate forwards; it never raises an event of its own"
    );
}

#[test]
fn the_gate_subcommand_refuses_a_word_it_will_not_vouch_for_without_notifying() {
    // The refusal has to be a refusal on BOTH forms. Falling through to event
    // mode here is how the bogus notification got out.
    let sandbox = Sandbox::new("gate-subcommand-refuses");
    for word in ["", "nonsense", "../../etc/passwd", "pi-hook; rm -rf /"] {
        let output = gate_argv(&sandbox, &["gate", word], "{}");
        assert_eq!(output.status.code(), Some(0), "word {word:?}");
        assert!(
            !sandbox.path("moshi.argv").exists(),
            "word {word:?} reached moshi"
        );
        assert!(!sandbox.fired("hermes"), "word {word:?} raised an event");
    }
}

#[test]
fn a_shape_the_gate_will_not_vouch_for_is_never_handed_to_moshi() {
    let sandbox = Sandbox::new("gate-refuses");
    for word in ["../../etc/passwd", "pi-hook; rm -rf /", "Pi-hook", "-hook"] {
        let output = gate(&sandbox, word, "{}");
        assert_eq!(output.status.code(), Some(0), "word {word:?}");
        assert!(
            !sandbox.path("moshi.argv").exists(),
            "word {word:?} reached moshi"
        );
    }
}

#[test]
fn at_the_desk_the_gate_submits_nothing_and_exits_zero() {
    // THE GATE IS PRESENCE-GATED TOO, off the same reading the hook path and
    // the delivery plan take. Every other gate test states the away clock, so
    // the gate's own reading has never been exercised at all: a build that
    // dropped it would card a phone for a prompt the operator is sitting in
    // front of, and every gate test would stay green. The Command is built
    // here rather than through `gate_argv`, which hard-codes away, so no
    // existing test moves.
    //
    // MECHANISM-BOUND, IN THE DANGEROUS DIRECTION: the absence reads through
    // `submissions`, so item 25 re-points one function rather than leaving a
    // desk-side submission unguarded behind a filename that no longer exists.
    let sandbox = Sandbox::new("gate-desk");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_PHONE_INPUT_AGE", "99999");
    sandbox.stub_moshi(&mut command, 7);
    let mut child = spawn_gate(command, "pi-hook");
    // The pipe is closed rather than written through: a gate that declines
    // never reads its stdin, so a write is allowed to go nowhere.
    write_payload(&mut child, b"{\"ask\":1}\n");
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "no opinion: the harness prompts as usual"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "the operator is right here; the card would be noise"
    );
    assert!(
        !sandbox.fired("hermes"),
        "a gate that declines raises no event of its own either"
    );
}

#[test]
fn the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does() {
    // The reader caps stdin, so an over-cap payload arrives CUT MID-OBJECT,
    // and handing that on is the empty parse the byte-for-byte contract exists
    // to prevent. The check runs at BOTH entry points and either call site can
    // lose it independently; only the hook's was pinned. Truncated JSON is the
    // same empty parse over any transport, so the invariant outlives the pipe.
    //
    // MECHANISM-BOUND, IN THE DANGEROUS DIRECTION: the absence reads through
    // `submissions` for the reason the desk twin above states.
    let sandbox = Sandbox::new("gate-oversized");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999");
    sandbox.stub_moshi(&mut command, 42);
    let mut child = spawn_gate(command, "pi-hook");
    let payload = format!(r#"{{"ask":"{}"}}"#, "x".repeat(1_200_000));
    write_payload(&mut child, payload.as_bytes());
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "an over-cap payload is not the operator's decision"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "half an object must never reach moshi"
    );
}

#[test]
fn the_gate_submits_one_prompt_exactly_once() {
    // THE OTHER SUBMITTER. Single-submitter is a rule about the PROMPT rather
    // than about one entry point, and the gate is the half pi and omp reach
    // directly with no pns hook in front of it. A second spawn here is a
    // second card and a second answer to one question, and until this counted
    // them nothing in the crate would have said so.
    //
    // MECHANISM-BOUND: the count is read off the submission record, so this
    // goes RED at the endpoint switch for item 25 to rewrite.
    let sandbox = Sandbox::new("gate-single-submitter");
    let output = gate(&sandbox, "pi-hook", "{\"ask\":1}\n");
    assert_eq!(
        output.status.code(),
        Some(7),
        "the decision is still the exit code"
    );
    assert_eq!(
        submissions(&sandbox),
        ["pi-hook"],
        "one prompt, one submission: a second card is a second answer nobody gave"
    );
}

/// The gate as a real process, reached by the bare harness word, with the
/// payload still to be written. The twin of `spawn_hook` for the other entry
/// point.
fn spawn_gate(mut command: Command, word: &str) -> std::process::Child {
    command
        .arg(word)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the engine runs")
}

#[test]
fn the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started() {
    // THE TIMING CONTRACT, made observable. The operator taps their phone and
    // the turn then spends seconds in the condenser; by the time anything is
    // delivered the tap is the older signal and the desk is where they are.
    //
    // The marker is touched as this hook starts and the desk is stated at one
    // second, so the two swap places DURING the condense: a reading taken at
    // process start says mobile and cards the phone, and a reading taken at
    // dispatch says desk and raises the banner. The banner is therefore the
    // whole assertion.
    let sandbox = Sandbox::new("hook-snapshot-timing");
    let marker = sandbox.path("phone.marker");
    std::fs::write(&marker, "").expect("marker");
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    // Long enough for the marker to age past the stated desk reading, which
    // is whole seconds, and no longer.
    write_script(&bin.join("codex"), "sleep 2");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "1")
        .env("PNS_DESK_IDLE_SECS", "120")
        .env("PNS_PHONE_MARKER_FILE", &marker)
        .env("CODEX_BIN", bin.join("codex"))
        .env("PNS_CODEX_HOME", sandbox.path("codex-home"));
    hook_with(
        command,
        &sandbox,
        "stop",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"a turn"}"#,
    );
    assert!(
        sandbox.fired("macos-banner"),
        "the banner belongs to the desk the operator went back to"
    );
    assert!(
        !sandbox.fired("moshi"),
        "and the tap that started this turn is no longer where they are"
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
fn a_turn_whose_transcript_lands_late_is_re_read_until_it_does() {
    // The harness has not always flushed when Stop runs. The transcript is
    // EMPTY at spawn and gains its reply while the hook is between reads, so
    // collapsing the loop to a single read loses it.
    let sandbox = Sandbox::new("hook-late-flush");
    let transcript = sandbox.path("t.jsonl");
    std::fs::write(&transcript, "").expect("empty transcript");
    let mut command = sandbox.pns();
    command
        .env("PNS_REPLY_REREAD_ATTEMPTS", "8")
        .env("PNS_REPLY_REREAD_INTERVAL", "0.05");
    let mut child = spawn_hook(command, "stop");
    write_payload(
        &mut child,
        format!(
            r#"{{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"{}"}}"#,
            transcript.display()
        )
        .as_bytes(),
    );
    std::thread::sleep(std::time::Duration::from_millis(120));
    std::fs::write(
        &transcript,
        "{\"type\":\"user\",\"message\":{\"content\":\"ask\"}}\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"landed late\"}]}}\n",
    )
    .expect("late write");
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    assert_eq!(sandbox.event("hermes")["detail"], "landed late");
}

#[test]
fn a_malformed_reread_interval_falls_back_instead_of_panicking() {
    // Duration::from_secs_f64 panics on these, on a path whose contract is
    // exiting 0. The last two are FINITE and non-negative, so they passed the
    // filter that guarded the other four and panicked in the constructor
    // anyway; sol reproduced exit 101 from 1e300 on 2026-08-19.
    let sandbox = Sandbox::new("hook-bad-interval");
    for value in ["NaN", "inf", "-1", "not-a-number", "1e30", "1e300"] {
        let mut command = sandbox.pns();
        command
            .env("PNS_REPLY_REREAD_INTERVAL", value)
            .env("PNS_REPLY_REREAD_ATTEMPTS", "1");
        let output = hook_with(
            command,
            &sandbox,
            "stop",
            r#"{"session_id":"s1","cwd":"/a/dotfiles","transcript_path":"/dev/null"}"#,
        );
        assert_eq!(output.status.code(), Some(0), "interval {value:?}");
    }
}

// --- the tier the marker decides --------------------------------------------

/// A hue config pointed at a listener nobody should reach unless the turn
/// earned a pulse. The signal is silent, so the CONNECTION is the
/// observation; the socket is closed the instant it arrives, because a
/// listener that accepts and says nothing makes the client wait out its own
/// deadline instead.
fn hue_listener(sandbox: &Sandbox) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    let reached = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&reached);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            drop(stream);
        }
    });
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        format!(
            "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
             [plugins.hermes]\nenabled = true\n"
        ),
    )
    .expect("config");
    reached
}

/// How many times the bridge was reached, after a settle for the connection
/// to land.
fn bridge_calls(reached: &std::sync::atomic::AtomicUsize) -> usize {
    std::thread::sleep(std::time::Duration::from_millis(100));
    reached.load(std::sync::atomic::Ordering::SeqCst)
}

#[test]
fn a_turn_long_enough_pulses_and_a_short_one_does_not() {
    // The marker's elapsed time is the ONLY thing that differs between these
    // two runs, which is what makes it the wiring under test.
    for (label, started_secs_ago, expected) in
        [("a long turn", 9_000, true), ("a short turn", 5, false)]
    {
        let sandbox = Sandbox::new(&format!("hook-tier-{}", started_secs_ago));
        let reached = hue_listener(&sandbox);
        std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs()
            - started_secs_ago;
        std::fs::write(marker(&sandbox, "s1"), started.to_string()).expect("marker");
        let mut child = spawn_hook(with_state_dir(&sandbox), "stop");
        write_payload(
            &mut child,
            br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#,
        );
        assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
        assert_eq!(bridge_calls(&reached) > 0, expected, "{label}");
    }
}

#[test]
fn a_long_turn_that_died_still_earns_its_pulse() {
    // The tier does not care HOW the turn ended: the operator who walked away
    // from a long run is exactly the one the lights are for, and this is the
    // first time a hook can reach the red half of the pulse at all.
    //
    // THE LISTENER COUNTS CONNECTIONS AND NEVER READS THE BODY (it closes the
    // socket the instant it arrives), so this pins that the pulse fired, not
    // that it was red. The colour is decided by `event.state`, which the
    // failed-turn test above pins as `failed`.
    let sandbox = Sandbox::new("hook-stop-failure-tier");
    let reached = hue_listener(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        - 9_000;
    std::fs::write(marker(&sandbox, "s1"), started.to_string()).expect("marker");
    let mut child = spawn_hook(with_state_dir(&sandbox), "stop-failure");
    write_payload(
        &mut child,
        br#"{"session_id":"s1","cwd":"/a/dotfiles","error":"API Error: 500"}"#,
    );
    assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    assert!(
        bridge_calls(&reached) > 0,
        "a turn that earned the tier still earns it when it dies"
    );
}

#[test]
fn two_stops_racing_one_turn_cannot_both_report_it_long() {
    // The claim is a rename, so exactly one of them can win it.
    let sandbox = Sandbox::new("hook-stop-race");
    let reached = hue_listener(&sandbox);
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        - 9_000;
    std::fs::write(marker(&sandbox, "s1"), started.to_string()).expect("marker");

    let payload = br#"{"session_id":"s1","cwd":"/a/dotfiles","last_assistant_message":"x"}"#;
    // BOTH start before either is fed, so they are genuinely in flight
    // together rather than one finishing while the next is still spawning.
    let mut children: Vec<_> = (0..2)
        .map(|_| spawn_hook(with_state_dir(&sandbox), "stop"))
        .collect();
    for child in &mut children {
        write_payload(child, payload);
    }
    for child in children.drain(..) {
        assert_eq!(finished_within(child, HANG_LIMIT), Some(0));
    }
    assert_eq!(
        bridge_calls(&reached),
        1,
        "exactly one Stop can claim the turn, so exactly one pulses"
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
    write_script(
        &bin.join("codex"),
        "cat >/dev/null; sleep 1; printf 'done|late\\n'",
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
    // The next prompt lands mid-condense.
    std::thread::sleep(std::time::Duration::from_millis(200));
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );
    assert_eq!(finished_within(stop, HANG_LIMIT), Some(0));
    assert!(
        marker(&sandbox, "s1").exists(),
        "the new turn's clock must survive the previous Stop finishing"
    );
}
