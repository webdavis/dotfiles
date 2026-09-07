use super::*;

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
fn the_decision_log_carries_the_payloads_mode_agent_and_tool() {
    // WHY: three `claude/blocked` events lined up with subagent hand-offs,
    // not with any prompt the operator saw (OBS-4), and the decision log had
    // no field that could ever tell those apart from an ordinary approval.
    // `CLAUDE_APPROVAL` states `permission_mode: "default"`,
    // `agent_id: "agent_01"` and `tool_name: "Bash"`.
    let sandbox = Sandbox::new("hook-blocked-payload-fields");
    let mut command = approval(&sandbox, 42);
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    hook_with(command, &sandbox, "blocked", CLAUDE_APPROVAL);
    let recorded =
        std::fs::read_to_string(sandbox.path("state/decisions")).expect("the decision ring");
    assert!(
        recorded.contains(" mode=default agent=agent_01 tool=Bash "),
        "got {recorded:?}"
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
