use super::*;

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
