use super::*;

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
