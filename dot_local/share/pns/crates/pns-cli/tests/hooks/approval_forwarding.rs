use super::*;

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
        !sandbox.fired("mobile"),
        "moshi is raising the card itself; pns pushing too is the same event twice"
    );
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
        sandbox.fired("mobile"),
        "a forward that never spawned suppresses nothing"
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
