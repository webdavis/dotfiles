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

// --- the approval contract ---------------------------------------------------
//
// THE GATE THAT BOUNDS THE SECOND APPROVAL SURFACE. Everything below is green
// on today's build by construction and each row was proved killable by a named
// mutation of the engine, because a characterization test nobody proved can
// fail is a line of green that guards nothing.
//
// WHAT IS DELIBERATELY NOT HERE, so nobody adds it back, and the test that
// covers each. Every one was written for this gate, measured against its
// mutation, found already killed by a test that exists, and dropped: a second
// copy of a guard is not a second guard.
//
//   the single-submitter rule, hook entry point
//     `one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`
//   the single-submitter rule, gate entry point
//     `the_gate_submits_one_prompt_exactly_once`
//   a submission that died without answering
//     `a_submission_that_dies_without_answering_is_not_a_decision`
//   the gate declining at the desk
//     `at_the_desk_the_gate_submits_nothing_and_exits_zero`
//   the gate refusing an over-cap payload
//     `the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does`
//   a watched pane is still forwarded
//     `an_approval_is_forwarded_even_with_the_pane_in_plain_sight`
//
// THREE MORE WERE STRUCK AND ARE BACK, at the end of this section, because in
// each case the test said to cover them drives a DIFFERENT arm and a
// blocked-only regression walks past it: the desk banner and the watched pane
// (`plan`'s matrix is a unit on the plan, and nothing composed a blocked hook
// at the desk), a payload nobody finishes writing (the deadline test beside it
// drives `stop`), and a presence reading nobody can parse (a unit on
// `operator_surface`, never across the forward).
//
// ONE BEHAVIOR IS DROPPED ON SCOPE AND IS PINNED NOWHERE END TO END: the
// locked screen. `screen_locked` spawns `/usr/sbin/ioreg` by absolute path, so
// no PATH stub reaches it, and it is read only where `PNS_IDLE_SECS` is
// unstated while every sandbox here states it. It has a unit pin on
// `operator_surface` and buying the composition would need a production
// override that exists for no other reason.
//
// THE EXIT CODE IS NOT HOW CLAUDE CODE ANSWERS, and the rows that pin one say
// so themselves. Claude Code 2.1.241 decides a PermissionRequest from the
// hook's STDOUT alone, off `hookSpecificOutput.decision`, and reads the exit
// code on that event nowhere; the answer to a phone tap travels moshi's own
// bridge, which screen-reads the pane and sends keys. What the exit code IS is
// a pns-side contract the gate's direct callers read, and whose reading by
// Codex is unverified. The corollary is the load-bearing one and
// `the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision` is
// its guard: stdout is a live channel on this event, and pns writes NOTHING to
// it on this path (measured, a real blocked run prints zero bytes), which is
// exactly why anything starting with `{` there is an object nobody meant to
// print.

/// A `PermissionRequest` payload, the binary's own field set (2.1.241):
/// `tool_name` and `tool_input` required and `permission_suggestions`
/// optional, spread over the base every event carries, in the emitter's own
/// key order.
///
/// IT STATES NO `message`, WHICH IS THE WHOLE POINT. The card's detail
/// resolves through `parse_payload`'s fallthrough to the tool request, exactly
/// as a Codex approval does, and every approval test written before this one
/// used a `{"message":...}` shape the harness has never sent.
///
/// ALL EIGHT BASE FIELDS ARE HERE (`session_id`, `transcript_path`, `cwd`,
/// `prompt_id`, `permission_mode`, `agent_id`, `agent_type`, `effort`), and so
/// is `permission_suggestions`, though pns reads three of the eight and none
/// of the suggestions. They are carried because the harness carries them: the
/// point of this fixture is that a future reader sees the real thing rather
/// than a reduction of it, and four base fields would have been a reduction.
/// The NAMES are the emitter's; the values of the four pns never reads are the
/// suite's own, because nothing measured what the harness puts in them.
const CLAUDE_APPROVAL: &str = r#"{"session_id":"s1","transcript_path":"/dev/null","cwd":"/a/dotfiles","prompt_id":"prompt_01","permission_mode":"default","agent_id":"agent_01","agent_type":"main","effort":"medium","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"},"permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"rm:*"}],"behavior":"allow","destination":"localSettings"}]}"#;

/// A Codex `PermissionRequest`, the shape measured off 0.147: `tool_name` and
/// `tool_input` and neither `message` nor `detail`, which is the same two keys
/// Claude Code sends.
const CODEX_APPROVAL: &str = r#"{"hook_event_name":"PermissionRequest","session_id":"s1","cwd":"/a/dotfiles","tool_name":"shell","tool_input":{"command":["bash","-lc","rm -rf build"]}}"#;

#[test]
fn a_two_from_moshi_comes_back_as_two_and_is_never_normalized() {
    // TWO IS THE TEMPTING VALUE. Across the hook family 2 is the code that
    // means "block", so a future normalizer written to keep pns from ever
    // blocking would special-case exactly this one, and every other
    // exit-code assertion in the crate uses 0, 7 or 42 and would stay green
    // straight through it. Measured: mapping 2 to 0 inside `moshi_decision`
    // passes the whole suite as it stands. The zero and the 42 are pinned by
    // `one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`
    // and by `a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision`,
    // so this row is the only one of the three that was missing.
    //
    // THE CODE IS MOSHI'S, AND IT IS NOT THE HARNESS'S ANSWER: see the
    // section header. It is a pns-side contract the gate's direct callers
    // read, and it is not the operator's decision, which arrives by moshi's
    // own bridge typing into the prompt.
    let sandbox = Sandbox::new("hook-blocked-two");
    let output = hook_with(approval(&sandbox, 2), &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(
        output.status.code(),
        Some(2),
        "the one code a normalizer would reach for still arrives as moshi said it"
    );
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "a 2 nobody was asked for is a refusal pns invented, not an answer"
    );
}

#[test]
fn a_payload_that_is_not_utf8_drops_the_approval_and_tells_the_operator_nothing() {
    // A KNOWN LIMIT, PINNED SO THAT CHANGING IT IS A DECISION. `read_payload`
    // reads a STRING, so invalid UTF-8 fails the read before any arm runs and
    // the hook returns 0 from `hook_mode` having done nothing at all. The
    // operator gets NOTHING: no submission, and not even a card saying
    // something is blocked, which every other refusal on this path still
    // sends. A lossy read would forward the mangled bytes instead and hand
    // back moshi's answer to them; both are defensible and neither is what
    // ships, so the choice belongs in front of whoever changes it.
    let sandbox = Sandbox::new("hook-blocked-not-utf8");
    let mut child = spawn_hook(approval(&sandbox, 42), "blocked");
    // A lone 0xff is invalid UTF-8 in any position, inside an otherwise
    // well-formed object so nothing but the encoding is wrong.
    write_payload(&mut child, b"{\"tool_name\":\"\xff\"}");
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "a payload that could not be read is not the operator's decision"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "bytes pns could not read are not bytes it may hand on"
    );
    assert!(
        !sandbox.fired("hermes"),
        "and the silence is total: this is the limit the comment above states"
    );
}

#[test]
fn a_codex_approval_is_submitted_as_codex_hook_and_names_the_tool_that_wants_to_run() {
    // THE ONLY END-TO-END COVERAGE OF THE SECOND HARNESS. Codex reaches this
    // exact path (`PNS_AGENT=codex $agent hook blocked`, from the Codex hook
    // installer), and until now the crate proved only the negative half:
    // `a_harness_pns_does_not_register_for_is_never_handed_to_moshi` shows an
    // unknown word is refused, and `moshi_subcommand`'s own unit test shows
    // the mapping is right, but nothing ran the WIRING between them. A spawn
    // that hard-coded `claude-hook` satisfies every other test in the crate
    // (measured), and Codex approvals would arrive at the wrong extension
    // while the suite stayed green.
    //
    // AND THE CARD IS THE SAME QUESTION FROM THE OTHER SIDE: a Codex payload
    // states no `message` either, so its detail comes through the same
    // fallthrough the Claude fixture exercises above.
    let sandbox = Sandbox::new("hook-blocked-codex");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_AGENT", "codex");
    let output = hook_with(command, &sandbox, "blocked", CODEX_APPROVAL);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert_eq!(
        submissions(&sandbox),
        ["codex-hook"],
        "a Codex prompt handed to the Claude extension is a card about the wrong session"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        CODEX_APPROVAL,
        "byte for byte on this harness too"
    );
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "blocked");
    assert_eq!(
        event["detail"], "shell: command=bash -lc rm -rf build",
        "which tool wants what is the question a blocked card has to answer"
    );
}

#[test]
fn a_payload_pns_cannot_parse_is_still_submitted_verbatim() {
    // PIPE, NOT INTERPRETER. moshi does the parsing, and pns forwarding only
    // what it could parse itself would silently swallow approvals the day a
    // harness changes its payload shape: the operator would sit in front of a
    // prompt whose card never came, with nothing anywhere saying why. The
    // notification still goes out carrying no detail, because something IS
    // blocked either way.
    let sandbox = Sandbox::new("hook-blocked-unparseable");
    let output = hook_with(
        approval(&sandbox, 42),
        &sandbox,
        "blocked",
        "not json at all",
    );
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        "not json at all",
        "what pns could not read is exactly what moshi has to be given"
    );
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "blocked");
    assert_eq!(
        event["detail"], "",
        "an unreadable payload names no tool, and inventing one would be worse"
    );
}

#[test]
fn the_forward_reads_the_surface_and_never_the_card_overrides() {
    // TWO OVERRIDES THAT LOOK LIKE THE SAME QUESTION AND ARE NOT. Both are
    // applied to the delivery plan's `phone_card` and NEITHER is read by
    // `forward_to_moshi`, which compares the surface and nothing else. The
    // distinction is the one a second approval surface is most likely to
    // collapse, and each direction fails differently:
    //
    //   FORCE buys a PUSH, not a ROUND TRIP. At the desk it cards the phone
    //   and submits nothing, so there is no card to answer. Wiring the
    //   override into the forward would open a round trip nobody asked for,
    //   on the one surface where the prompt is already on screen.
    //
    //   SKIP suppresses PNS'S CARD, not the SUBMISSION. It is set by the
    //   blocked path itself once a submission succeeds, so reading it in the
    //   forward would be the forward gating on its own output; away, an
    //   operator with the variable set would lose approvals entirely.
    //
    // Both mutations were measured to pass the rest of the suite.
    let forced = Sandbox::new("hook-blocked-force-phone");
    let mut command = approval(&forced, 42);
    command
        .env("PNS_IDLE_SECS", "0")
        .env("PNS_FORCE_PHONE", "1");
    let output = hook_with(command, &forced, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(0), "no round trip, no decision");
    assert!(
        submissions(&forced).is_empty(),
        "the override buys a card, and a card is not a question anyone can answer"
    );
    assert!(
        forced.fired("moshi"),
        "the push it does buy still has to arrive"
    );

    let skipped = Sandbox::new("hook-blocked-skip-phone");
    let mut command = approval(&skipped, 42);
    command.env("PNS_SKIP_PHONE", "1");
    let output = hook_with(command, &skipped, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert_eq!(
        submissions(&skipped),
        ["claude-hook"],
        "a suppressed card is not a suppressed prompt"
    );
    // THE ABSENT CARD IS NOT EVIDENCE THE VARIABLE WAS READ, the same trap
    // the channel-off sibling documents. The forward succeeded, so the blocked
    // path sets `PNS_SKIP_PHONE` itself and pns's phone leg is suppressed
    // whether or not the caller's variable ever reached anything. This line
    // pins that no second card appeared and nothing about what silenced it;
    // the submissions line above is this row's real pin, and the one the
    // `&& !overrides.skip_phone` mutation kills.
    assert!(!skipped.fired("moshi"));
}

#[test]
fn a_real_claude_approval_cards_the_tool_that_wants_to_run() {
    // KEPT AS THE HOME OF THE FIXTURE RATHER THAN AS A NEW GUARD, and saying
    // so is the point. `CLAUDE_APPROVAL` is the payload the harness actually
    // sends and no approval test in the crate had ever used one: every one of
    // them states a `message`, and a real PermissionRequest states none, so
    // the detail resolves through the fallthrough to the tool request instead.
    // A blocked card that names no tool asks the operator to approve something
    // it cannot describe, and that is what shipped for Codex until the
    // fallthrough was added.
    //
    // ITS NAMED MUTATION KILLS THREE TESTS, measured: dropping the
    // `tool_request` fallthrough from `parse_payload`'s `unwrap_or_else`
    // fails this row, and
    // `a_codex_approval_is_submitted_as_codex_hook_and_names_the_tool_that_wants_to_run`,
    // and the pre-existing
    // `a_refused_tool_call_notifies_as_denied_and_says_which_tool_was_refused`.
    // So this row is not what stands between that fallthrough and a card that
    // names no tool, and by the section header's own rule it qualifies for the
    // strike the drops above got. What it is instead is the one place the real
    // field set is driven end to end.
    //
    // ITS ASSERTIONS OVERLAP TWO SIBLINGS, deliberately rather than by
    // oversight: `a_blocked_hook_cards_the_operator_as_blocked_and_says_what_was_asked`
    // already pins the state word and the project on this arm, on a
    // stated-message payload, and the denied row already pins a detail built
    // by the same fallthrough. What is new here is the payload, not the
    // question asked of it.
    let sandbox = Sandbox::new("hook-blocked-real-payload");
    let output = hook_with(approval(&sandbox, 42), &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "blocked");
    assert_eq!(
        event["detail"], "Bash: command=rm -rf /tmp/x",
        "the tool and what it wants to do with it is the whole decision"
    );
    assert_eq!(
        event["project"], "dotfiles",
        "read out of the payload's cwd"
    );
}

#[test]
fn an_approval_that_was_submitted_is_recorded_and_is_never_journaled_as_missed() {
    // `skip_phone=yes` IS THE ONLY TRACE OF A FORWARD ANYWHERE IN PNS'S
    // RECORDS. moshi mints the actionId inside itself and answers with an
    // exit code, so nothing else about the round trip is written down: drop
    // the record for this state and `pns doctor` can no longer say why a card
    // did not fire on the one path where the answer is "because something
    // else raised it". Measured: skipping the record for `blocked` alone
    // passes the whole rest of the suite.
    //
    // AND THE JOURNAL MUST STAY EMPTY. A forwarded approval is not a missed
    // notification; replaying it later would put Allow and Deny in front of an
    // operator for a prompt that was answered hours ago. THAT HALF HAS ITS OWN
    // MUTATION, because the record mutation above fails on the missing ring
    // file before the journal assertion is ever reached: dropping
    // `!overrides.skip_phone` from `was_missed` journals this forwarded
    // approval as missed, and kills this test alone (measured).
    let sandbox = Sandbox::new("hook-blocked-records");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    let output = hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42));
    let recorded =
        std::fs::read_to_string(sandbox.path("state/decisions")).expect("the decision ring");
    let lines: Vec<&str> = recorded.lines().collect();
    assert_eq!(lines.len(), 1, "one event, one line: {recorded:?}");
    assert!(
        lines[0].contains(" claude/blocked "),
        "the record names the harness and the state: {recorded:?}"
    );
    assert!(
        lines[0].contains(" skip_phone=yes "),
        "the forward's only trace: {recorded:?}"
    );
    assert!(
        lines[0].contains("legs=hermes:"),
        "and what the durable leg had to say: {recorded:?}"
    );
    assert!(
        !sandbox.path("state/missed-notifications").exists(),
        "an approval the operator was handed is not one they missed"
    );
}

#[test]
fn an_approval_leaves_the_turn_marker_alone() {
    // THE TURN CONTINUES PAST AN APPROVAL. The harness resumes the tool call
    // and the turn ends later, at the Stop that follows, so consuming the
    // clock here would restart it mid-turn: a long turn that paused once for
    // a permission prompt would report itself short and lose the pulse and
    // the mobile watch card its tier earns. The twin of
    // `a_refused_tool_call_leaves_the_turn_marker_alone`, on the arm that
    // also spawns a submission, which is the arm where a claim is easiest to
    // add by accident.
    let sandbox = Sandbox::new("hook-blocked-marker");
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(marker(&sandbox, "s1"), "1755000000").expect("marker");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(
        std::fs::read_to_string(marker(&sandbox, "s1")).expect("the marker survives the approval"),
        "1755000000",
        "the turn an approval interrupted is still measured by the Stop that ends it"
    );
    // Absence alone would also be green for an arm that does nothing at all,
    // the correction the elicitation guard carries too. Measured: a `blocked`
    // arm short-circuited to `return 0` leaves the marker assertion above
    // green and is killed by the line below.
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "the arm that left the marker alone is the arm that forwarded"
    );
}

#[test]
fn the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision() {
    // THE LOAD-BEARING GUARD, and the reason the section header spells out
    // what the exit code is not. STDOUT is the live channel on this event:
    // Claude Code parses a PermissionRequest hook's stdout and
    // `hookSpecificOutput.decision` is the only thing that decides it.
    //
    // AND PNS WRITES NOTHING TO IT HERE, which is the premise this row rests
    // on. `Delivery::line_for` yields a line only under
    // `ReportMode::ReportOutcome`; `channel_plan` selects that mode only for
    // `--remote-only`; no hook path sets it. So every leg on this path is
    // Silent and the `pns: ` print is never reached, and a real `hook blocked`
    // run prints zero bytes (measured). The `pns: ` delivery lines are real,
    // they just do not happen here.
    //
    // WHICH IS WHY THE ASSERTION IS EXACTLY-EMPTY rather than a
    // first-character test. Nothing legitimate prints on this path today, so
    // the strongest available guard is the honest one, and a build that starts
    // printing here has to edit this test out loud instead of slipping an
    // object in behind a prose line. It also has no trim in it to be wrong
    // about: the harness reads through JavaScript `trim()`, which strips
    // U+FEFF, while Rust's `trim_start` does not, so a first-character test
    // spelled in Rust would pass a byte-order-mark in front of a valid `allow`
    // object that Claude Code accepts.
    //
    // A pns that answered the prompt itself would be a SECOND SUBMITTER by
    // another name, deciding a question moshi has already put in front of the
    // operator, and it would be invisible: the card still arrives, the
    // submission still happens, and the harness acts on pns's answer instead
    // of theirs. MUTATION, measured: printing
    // `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"pns declined"}}}`
    // at the end of the blocked path kills this row and passes every other
    // test in the crate. That object is one the harness's own schema accepts:
    // a `deny` states a `message`, and without one it is rejected before any
    // decision is read, so a mutation without it would prove nothing.
    //
    // The twin of
    // `the_hook_writes_nothing_the_harness_could_read_as_an_answer_and_exits_zero`,
    // and more load bearing than its twin, because there the channel is not
    // even open.
    let sandbox = Sandbox::new("hook-blocked-stdout-guard");
    let output = hook_with(approval(&sandbox, 42), &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42));
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        printed, "",
        "pns prints nothing at all on this path, so anything here is unmeant: {printed:?}"
    );
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "and the submission that DID happen is what answers the prompt"
    );
}

#[test]
fn a_blocked_payload_nobody_finishes_writing_forwards_nothing_and_exits_zero() {
    // THE DEADLINE ON THE ARM THAT SPAWNS. Its sibling
    // `a_payload_nobody_finishes_writing_still_exits_on_the_contract` drives
    // `stop`, where nothing is ever forwarded, so a blocked-only regression
    // walks straight past it: a timeout that fell back to an empty payload
    // rather than returning would hand moshi an empty stdin, mint a card whose
    // actionId answers a prompt nobody can read, and notify the operator about
    // an approval nobody can answer.
    //
    // THE PIPE IS HELD OPEN, which is what a harness that opens the hook and
    // then stalls does. The child's stdin handle lives as long as the `Child`
    // here, so nothing ever sends EOF and only the deadline ends it.
    //
    // TWO MUTATIONS, both measured. Dropping the deadline entirely
    // (`recv()` in place of `recv_timeout(payload_deadline())`) hangs this
    // test out to `HANG_LIMIT` and kills it, and kills the `stop` sibling
    // with it. The one that isolates this row is the blocked-only fallback
    // sol named: `hook_mode` answering a timed-out read with `String::new()`
    // for `blocked` alone leaves the `stop` sibling GREEN and kills this test
    // (with `a_payload_that_is_not_utf8_drops_the_approval...`, which reaches
    // the same arm through the same empty read).
    let sandbox = Sandbox::new("hook-blocked-payload-hang");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_PAYLOAD_DEADLINE_MS", "200");
    let child = spawn_hook(command, "blocked");
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "no payload is no approval, and still exit 0"
    );
    assert!(
        submissions(&sandbox).is_empty(),
        "an empty payload forwarded is a card answering a prompt nobody read"
    );
    assert!(
        !sandbox.fired("hermes"),
        "and nobody is told about a block that described nothing"
    );
}

#[test]
fn a_presence_reading_nobody_can_parse_still_forwards_the_approval() {
    // FAIL TOWARD THE PHONE, ACROSS THE FORWARD. `surface_reading` refuses a
    // garbled `PNS_IDLE_SECS` rather than falling back to a probe or to a
    // default, so the surface is not Desk and `forward_to_moshi` says yes.
    // The engine unit
    // `the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading`
    // pins that refusal on `operator_surface` itself and nothing crossed the
    // forward: a `forward_to_moshi` that returned false on an unreadable
    // reading would leave every unit green while an operator whose presence
    // could not be read lost approvals entirely. That is the failure on this
    // path that looks exactly like being at your desk, which is why the whole
    // section exists.
    //
    // MUTATION, measured: `surface_reading` reading an invalid idle override
    // as a desk just touched (`(Some(0), None)` in place of `(None, None)`)
    // kills this test.
    let sandbox = Sandbox::new("hook-blocked-idle-garbled");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_IDLE_SECS", "soup");
    let output = hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
    assert_eq!(output.status.code(), Some(42), "the operator's own answer");
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "a presence nobody could read is not an operator sitting at the desk"
    );
}

#[test]
fn at_the_desk_a_blocked_approval_banners_a_hidden_pane_and_leaves_a_watched_one_alone() {
    // THE SURFACE SLICE 27 PUTS BUTTONS ON, composed end to end so that
    // changing it has to be said out loud. `plan`'s own matrix pins these two
    // rows as a unit and `tests/dispatch.rs` pins them through `--state
    // blocked`, but nothing anywhere composed `hook blocked` with a desk
    // reading and asked what the operator actually receives, which is the one
    // question an approve button on the banner changes the answer to.
    //
    // NEITHER ROW FORWARDS, the same rule
    // `at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual`
    // states from the other side: the harness prompt is already in front of
    // them, so a card is noise and a round trip asks one question twice.
    //
    // MUTATION, measured: `plan`'s banner condition losing `!watching`
    // (`banner: surface == Surface::Desk`) kills the watched row here. Four
    // unit tests die on it too (`surface`'s own confirmed matrix and three in
    // `engine`), and that is the point rather than a duplication: those die on
    // the PLAN, and this dies on what a blocked hook puts in front of the
    // operator, which is the half the strike left unguarded.
    for (slug, label, pane_watched, banner_expected) in [
        ("hidden", "the pane is on another tab", false, true),
        (
            "watched",
            "the pane is the one being looked at",
            true,
            false,
        ),
    ] {
        let sandbox = Sandbox::new(&format!("hook-blocked-desk-{slug}"));
        let mut command = approval(&sandbox, 42);
        command
            .env("PNS_IDLE_SECS", "0")
            .env("HERDR_PANE_ID", "t1:p1");
        sandbox.stub_herdr(&mut command, pane_watched);
        let output = hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
        assert_eq!(output.status.code(), Some(0), "no round trip: {label}");
        assert!(
            submissions(&sandbox).is_empty(),
            "the prompt is already on their screen: {label}"
        );
        assert_eq!(
            sandbox.fired("macos-banner"),
            banner_expected,
            "the desk banner: {label}"
        );
        assert!(
            sandbox.fired("hermes"),
            "and the durable leg carries every approval: {label}"
        );
    }
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
    // anywhere would say why. pns returns 0 on every notification path and
    // writes NOTHING to stdout on one: the `pns: ` delivery lines exist, but
    // `Delivery::line_for` emits one only under `ReportMode::ReportOutcome`,
    // which only `--remote-only` selects and no hook path does. The assertion
    // mirrors the harness's own reader, which trims before it looks at the
    // first character, so empty stdout and prose stdout are the same pass.
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

/// The engine with a moshi stub ALWAYS installed, whatever else the caller
/// overrides afterwards.
///
/// Every test in the approval section spawns the blocked path, and
/// `Sandbox::pns` points `MOSHI_HOOK_BIN` nowhere, so a test that forgets to
/// stub reaches the OPERATOR'S OWN moshi-hook and can raise a real card on
/// their phone. That is not hypothetical: it happened during slice 11, seven
/// tests deep. One helper is cheaper than remembering.
fn approval(sandbox: &Sandbox, exit_code: i32) -> Command {
    let mut command = sandbox.pns();
    sandbox.stub_moshi(&mut command, exit_code);
    command
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

/// How long until the hook's STDOUT REACHES EOF, and `None` when it never does
/// inside the limit.
///
/// PROCESS EXIT IS THE WRONG CLOCK FOR A SUBMISSION. Claude Code decides a
/// `PermissionRequest` by reading the hook's stdout to end, and only stdin is
/// piped to moshi-hook, so a submission that outlives the hook still holds
/// that write end and the prompt stays hidden for as long as it does. Timing
/// the process alone reports a tenth of a second for a run that leaves the
/// harness waiting ten, which is the whole reason the bound below is measured
/// here instead.
///
/// THE READER IS ITS OWN THREAD because the read is the thing being bounded:
/// a caller that blocked on it could not report the case it exists to catch.
fn stdout_eof_within(
    stdout: std::process::ChildStdout,
    limit: std::time::Duration,
) -> Option<std::time::Duration> {
    let started = std::time::Instant::now();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut stdout = stdout;
        let mut drained = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut drained);
        let _ = sender.send(());
    });
    receiver
        .recv_timeout(limit)
        .ok()
        .map(|()| started.elapsed())
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

/// A moshi-hook that registers the submission and then never answers.
///
/// THE WEDGED DAEMON, which is the case an unbounded wait cannot survive: a
/// listener that accepts the connection and never replies held the real
/// `moshi-hook claude-hook` for 90 seconds with no self-timeout, no output and
/// no error (measured 2026-08-29). The argv record is written BEFORE the
/// sleep, so a submission that happened is still countable while the child is
/// still hanging.
///
/// `exec` IS LOAD BEARING. moshi-hook is a single binary, so the process pns
/// spawns is the process holding the inherited stdout, and the bound's kill
/// reaches exactly that one. Without `exec` this stub's shell would fork the
/// sleep and leave a GRANDCHILD holding the pipe open for its full ten
/// seconds, which is a submission shape the real one does not have and which
/// no kill short of a process group could release. Measured both ways: 0.001s
/// to EOF after the kill with `exec`, 9.9s without.
fn stub_silent_moshi(sandbox: &Sandbox, command: &mut Command) {
    let bin = sandbox.path("bin");
    std::fs::create_dir_all(&bin).expect("stub bin");
    write_script(
        &bin.join("moshi-hook"),
        &format!(
            "printf '%s\\n' \"$*\" >>\"{sandbox}/moshi.argv\"; cat >/dev/null; exec sleep 10",
            sandbox = sandbox.display()
        ),
    );
    command.env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
}

/// The deadline each silent-moshi run injects, and the window the harness's
/// own read of the hook must end inside: FOUR TIMES that deadline.
///
/// FOUR TIMES IS CHOSEN AGAINST A NAMED MUTANT. A deadline hard-coded to one
/// second, ignoring the injected value, puts EOF just past a second, which the
/// hook run's 600ms bound refuses. Measured green runs of that one sit at
/// 0.180-0.198s idle and 0.217s with every core busy, so the bound keeps a
/// 2.7x margin over the worst honest run and better than 3x over a quiet
/// machine. The gate run injects 400ms rather than 150 because its stub has a
/// `/bin/sh` spawn inside the window; its bound scales with it, and it
/// measures 0.408-0.420s.
const SILENT_MOSHI_DEADLINE_MS: &str = "150";
const SILENT_MOSHI_EOF_BOUND: std::time::Duration = std::time::Duration::from_millis(600);
const GATE_SILENT_MOSHI_DEADLINE_MS: &str = "400";
const GATE_SILENT_MOSHI_EOF_BOUND: std::time::Duration = std::time::Duration::from_millis(1_600);

#[test]
fn a_moshi_that_never_answers_stops_holding_the_operators_prompt() {
    // THE DEFECT THIS BOUND EXISTS FOR, on the most safety-critical path pns
    // has. PermissionRequest runs BEFORE the prompt is drawn and is
    // deliberately not async, so the harness awaits this hook: every second
    // spent waiting on a wedged daemon is a second the operator is looking at
    // a terminal that is not showing them the question, and the only other
    // bound in the system is the harness's own ten-minute ceiling.
    //
    // THE CLOCK IS THE HARNESS'S OWN, and that is the point of this row.
    // Claude Code decides this event by reading the hook's stdout to end, the
    // submission inherited that write end, and a survivor holds it open: a
    // deadline that returns without killing the child measures 0.18s on the
    // process and 10.03s on the stream the harness is actually reading. So
    // this assertion pins the KILL as much as the deadline, and reverting the
    // kill hangs it past its bound.
    //
    // EXIT 0 IS NO OPINION. Claude Code reads no exit code on this event at
    // all (measured); a non-zero would put pns's own word into a channel that
    // carries moshi's, which the gate's direct callers do read.
    //
    // THE SLEEP IS NOT A SLOW TEST. With the bound in place this run costs the
    // injected deadline and the teardown; it is the RED run, with no bound,
    // that pays the stub's ten seconds.
    let sandbox = Sandbox::new("hook-blocked-silent-moshi");
    let mut command = sandbox.pns();
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("PNS_MOSHI_SUBMIT_DEADLINE_MS", SILENT_MOSHI_DEADLINE_MS);
    stub_silent_moshi(&sandbox, &mut command);
    let mut child = spawn_hook(command, "blocked");
    let stdout = child.stdout.take().expect("stdout");
    write_payload(
        &mut child,
        br#"{"message":"may I run this","session_id":"s1","cwd":"/a/dotfiles"}"#,
    );
    let hidden_for = stdout_eof_within(stdout, HANG_LIMIT)
        .expect("the harness's own read of this hook has to end at all");
    assert!(
        hidden_for < SILENT_MOSHI_EOF_BOUND,
        "the prompt stayed hidden for {hidden_for:?}, past the {SILENT_MOSHI_EOF_BOUND:?} bound"
    );
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "a daemon that never answers must not hold the permission prompt off the screen"
    );
    // THE BOUND COST THE WAIT AND NOT THE NOTIFICATION. The card is raised
    // before the wait starts, so an expiry that also lost the card would mean
    // the operator learned nothing at all about a prompt they cannot see.
    assert_eq!(
        sandbox.event("hermes")["state"],
        "blocked",
        "the blocked card still went out"
    );
    // AND THE TIMEOUT SUBMITTED NOTHING FURTHER. One prompt is one submission
    // however the wait ended: a retry after an expiry is a second card and a
    // second answer to one question.
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "one prompt, one submission, expiry included"
    );
}

#[test]
fn the_gate_is_bounded_by_the_same_clock_as_the_hook() {
    // THE SECOND CALLER, which is the whole reason the bound sits at the
    // function both of them route through rather than at the one the defect
    // was found on. `pns gate <harness>-hook` is what pi and omp reach
    // directly, with no pns hook in front of it, and it waited on exactly the
    // same unbounded `child.wait()`.
    //
    // SAME CLOCK, SAME STREAM: timed to stdout EOF for the reason its twin
    // above states, and with a longer deadline because this path spawns the
    // stub's shell inside the window.
    let sandbox = Sandbox::new("gate-silent-moshi");
    let mut command = sandbox.pns();
    command.env("PNS_IDLE_SECS", "99999").env(
        "PNS_MOSHI_SUBMIT_DEADLINE_MS",
        GATE_SILENT_MOSHI_DEADLINE_MS,
    );
    stub_silent_moshi(&sandbox, &mut command);
    let mut child = command
        .args(["gate", "claude-hook"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the engine runs");
    let stdout = child.stdout.take().expect("stdout");
    write_payload(&mut child, b"{\"ask\":1}\n");
    let hidden_for = stdout_eof_within(stdout, HANG_LIMIT)
        .expect("the harness's own read of the gate has to end at all");
    assert!(
        hidden_for < GATE_SILENT_MOSHI_EOF_BOUND,
        "the gate held its stream for {hidden_for:?}, past the {GATE_SILENT_MOSHI_EOF_BOUND:?} bound"
    );
    assert_eq!(
        finished_within(child, HANG_LIMIT),
        Some(0),
        "the gate waits on the same clock: no opinion, and the harness prompts as usual"
    );
    assert_eq!(
        submissions(&sandbox),
        ["claude-hook"],
        "one prompt, one submission, expiry included"
    );
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

// --- the lamps' needs markers -----------------------------------------------

/// The lamps switched on: a map, and the transport enabled. BOTH, because a
/// `[lights]` table with hue disabled lights nothing and runs no tick, so
/// there would be nothing to sweep the markers it wrote.
const LAMPS_ON: &str = "[plugins.hue]\nenabled = true\n\
     [lights]\nrefresh_secs = 20\n[lights.families.local]\nrooms = [\"3F - Studio\"]\n";

/// Every session the lamps currently believe is waiting on the operator.
fn waiting_sessions(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sandbox.path("state/lights-needs"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it() {
    let sandbox = Sandbox::new("lights-needs-marker");
    sandbox.write_config(LAMPS_ON);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"s1","message":"may I"}"#,
    );
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "asked",
        r#"{"session_id":"s2","message":"and I"}"#,
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string(), "s2".to_string()],
        "two waiting sessions, two markers"
    );
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "stop",
        r#"{"session_id":"s1"}"#,
    );
    // COMPLETENESS OVER COUNTS: the survivor is named, so a clear that took
    // the wrong session's marker cannot pass by leaving the right number of
    // files behind.
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s2".to_string()],
        "the answered session's marker is gone and the other one is untouched"
    );
}

#[test]
fn an_event_with_no_session_id_behind_it_holds_no_lamp() {
    // THE HONEST LIMIT, pinned so a later build cannot quietly invent an
    // identity: an event that arrives on argv rather than through a harness
    // hook has nothing that could later say the wait ended, so it gets the
    // flash and cannot hold the lamp.
    let sandbox = Sandbox::new("lights-needs-no-session");
    sandbox.write_config(LAMPS_ON);
    let mut command = with_state_dir(&sandbox);
    sandbox.stub_herdr(&mut command, false);
    let output = command
        .args(["--agent", "claude", "--state", "blocked", "--detail", "x"])
        .output()
        .expect("the engine runs");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "no identity, no marker"
    );
    // AND A HOOK PAYLOAD CARRYING AN ID THAT CANNOT BE A FILENAME is the same
    // answer through the other door.
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"../../etc/passwd","message":"may I"}"#,
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "a traversal names no marker at all"
    );
}
