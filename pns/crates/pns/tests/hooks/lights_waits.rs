use super::*;

// --- the lamps' needs markers -----------------------------------------------

/// The lamps switched on: a map, and the transport enabled. BOTH, because a
/// `[lights]` table with hue disabled lights nothing and runs no tick, so
/// there would be nothing to sweep the markers it wrote.
pub(crate) const LAMPS_ON: &str = "[plugins.hue]\nenabled = true\n\
     [lights]\nrefresh_secs = 20\n\
     [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n";

/// Every session the lamps currently believe is waiting on the operator.
pub(crate) fn waiting_sessions(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sandbox.path("state/lights-blocked"))
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
    let sandbox = Sandbox::new("lights-blocked-marker");
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
fn a_prompt_from_a_waiting_session_ends_its_wait() {
    // THE OPERATOR ANSWERED BY TYPING, which `resolved` cannot see: the
    // PostToolBatch clearing signal never fires for a PermissionRequest wait
    // (Claude Code decides that off the hook's stdout, not off a later tool
    // batch), so the lamp used to stay blocked until the turn's Stop hook, one
    // whole tool call after the operator already answered.
    let sandbox = Sandbox::new("lights-blocked-prompt-clears");
    sandbox.write_config(LAMPS_ON);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"s1","message":"may I"}"#,
    );
    assert_eq!(waiting_sessions(&sandbox), vec!["s1".to_string()]);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "a prompt from the waiting session is the operator, so the wait is over"
    );
}

#[test]
fn a_resolved_batch_with_no_agent_id_ends_its_sessions_wait() {
    let sandbox = Sandbox::new("lights-blocked-resolved-clears");
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
        "resolved",
        r#"{"session_id":"s1"}"#,
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "the batch this session was blocked on resolved, so the wait is over"
    );
}

#[test]
fn a_resolved_batch_carrying_an_agent_id_leaves_the_parents_wait_lit() {
    // A SUBAGENT'S BATCH SAYS NOTHING ABOUT THE OPERATOR. `agent_id` is
    // present only when the hook fires inside a subagent call, and the
    // parent's own wait is still exactly as answered as it was before this
    // batch resolved. RESIDUAL, STATED HONESTLY: the parent's marker now
    // stays lit until its Stop, one call later than it needs to.
    let sandbox = Sandbox::new("lights-blocked-resolved-subagent");
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
        "resolved",
        r#"{"session_id":"s1","agent_id":"agent_01"}"#,
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "a subagent's batch must not clear the parent session's wait"
    );
}

#[test]
fn a_resolved_batch_with_a_malformed_agent_id_still_leaves_the_parents_wait_lit() {
    // PRESENCE IS THE SIGNAL, NOT SHAPE: the reference promises only that
    // the key is ABSENT on the main thread, so null, a number or an empty
    // string is not proof the operator answered, and the guard fails closed.
    let sandbox = Sandbox::new("lights-blocked-resolved-malformed-agent");
    sandbox.write_config(LAMPS_ON);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"s1","message":"may I"}"#,
    );
    for shape in ["null", "7", "\"\""] {
        hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "resolved",
            &format!(r#"{{"session_id":"s1","agent_id":{shape}}}"#),
        );
        assert_eq!(
            waiting_sessions(&sandbox),
            vec!["s1".to_string()],
            "agent_id:{shape} must not clear the parent's wait"
        );
    }
}

#[test]
fn a_prompt_ends_only_its_own_sessions_wait() {
    // ONE FILE PER SESSION IS THE WHOLE POINT: the operator typing in s1 says
    // nothing about s2, which is still waiting on them.
    let sandbox = Sandbox::new("lights-blocked-prompt-other-session");
    sandbox.write_config(LAMPS_ON);
    for session in ["s1", "s2"] {
        hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "blocked",
            &format!(r#"{{"session_id":"{session}","message":"may I"}}"#),
        );
    }
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"s1"}"#,
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s2".to_string()],
        "s1 answered; s2 is still waiting"
    );
}

#[test]
fn a_prompt_naming_a_traversal_removes_nothing() {
    // THE END ACTION GOES THROUGH THE SAME FILENAME PREDICATE AS THE START:
    // a session id that cannot be a marker name is refused before the
    // unlink, so a payload cannot aim the removal outside the marker dir.
    let sandbox = Sandbox::new("lights-blocked-prompt-traversal");
    sandbox.write_config(LAMPS_ON);
    // A real marker first, so `lights-blocked/` exists for `..` to walk out of.
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"s1","message":"may I"}"#,
    );
    let victim = sandbox.path("victim");
    std::fs::write(&victim, "x").expect("the victim file");
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "prompt",
        r#"{"session_id":"../../victim"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(victim.exists(), "a traversal id must never reach an unlink");
    assert_eq!(waiting_sessions(&sandbox), vec!["s1".to_string()]);
}

#[test]
fn an_event_with_no_session_id_behind_it_holds_no_lamp() {
    // THE HONEST LIMIT, pinned so a later build cannot quietly invent an
    // identity: an event that arrives on argv rather than through a harness
    // hook has nothing that could later say the wait ended, so it gets the
    // flash and cannot hold the lamp.
    let sandbox = Sandbox::new("lights-blocked-no-session");
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

// --- the answer that ends a wait ---------------------------------------------

/// A `PostToolUse` payload for a tool that owns its own dialog: the answer
/// itself, which is what makes the tool return.
pub(crate) fn answered_dialog(session: &str, tool: &str) -> String {
    format!(
        r#"{{"hook_event_name":"PostToolUse","session_id":"{session}","cwd":"/a/dotfiles","tool_name":"{tool}","tool_input":{{"questions":[{{"question":"which one?"}}]}},"tool_response":{{"answer":"the second"}}}}"#
    )
}

/// An `ElicitationResult` payload, Claude Code 2.1.272's own field set:
/// `mcp_server_name`, `elicitation_id`, `mode`, `action` and `content`.
pub(crate) fn elicitation_result(session: &str, action: &str) -> String {
    format!(
        r#"{{"hook_event_name":"ElicitationResult","session_id":"{session}","cwd":"/a/dotfiles","mcp_server_name":"composio","elicitation_id":"elic_01","mode":"url","action":"{action}","content":{{}}}}"#
    )
}

#[test]
fn every_shape_of_answer_ends_the_wait_it_answered() {
    // THE ANSWERED-WAIT RACE, CLOSED AT THE ANSWER. `PermissionRequest` arms
    // the wait before the question card is drawn, because `AskUserQuestion`
    // and `ExitPlanMode` both declare they require user interaction. What was
    // missing is the other end: the answer arrived as `asked` or
    // `plan-ready`, both of which ARM a wait, so one answered question left
    // the lamp lit until the session's next event. Each payload below is
    // routed to `pns hook resolved` by its own declaration, and each is the
    // operator having answered.
    for (name, payload) in [
        ("a question", answered_dialog("s1", "AskUserQuestion")),
        ("a plan", answered_dialog("s1", "ExitPlanMode")),
        ("an elicitation", elicitation_result("s1", "accept")),
        (
            "a declined elicitation",
            elicitation_result("s1", "decline"),
        ),
    ] {
        let sandbox = Sandbox::new(&format!(
            "lights-blocked-answered-{}",
            name.replace(' ', "-")
        ));
        sandbox.write_config(LAMPS_ON);
        hook_with(
            with_state_dir(&sandbox),
            &sandbox,
            "blocked",
            r#"{"session_id":"s1","message":"may I"}"#,
        );
        assert_eq!(
            waiting_sessions(&sandbox),
            vec!["s1".to_string()],
            "{name}: the precondition, a wait armed before the dialog"
        );
        hook_with(with_state_dir(&sandbox), &sandbox, "resolved", &payload);
        assert!(
            waiting_sessions(&sandbox).is_empty(),
            "{name} was answered, so the wait is over"
        );
    }
}

#[test]
fn a_subagents_own_end_ends_the_wait_it_is_holding() {
    // THE SUBAGENT RESIDUAL, BOUNDED AT THE SUBAGENT'S OWN END. A subagent's
    // approval arms the PARENT session's marker, because the marker is keyed
    // by session and a subagent shares its parent's, and `resolved` skips a
    // batch carrying `agent_id` precisely because that batch says nothing
    // about the parent's wait. `SubagentStop` says something else: the
    // subagent that was waiting has finished. It carries `agent_id` too
    // (measured in the 2.1.272 bundle), so it is the one payload on this arm
    // whose subagent key must not silence the clear, or the fifth declaration
    // would be a no-op.
    let sandbox = Sandbox::new("lights-blocked-subagent-stop");
    sandbox.write_config(LAMPS_ON);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"s1","message":"may I","agent_id":"agent_01"}"#,
    );
    assert_eq!(waiting_sessions(&sandbox), vec!["s1".to_string()]);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "resolved",
        r#"{"hook_event_name":"SubagentStop","session_id":"s1","cwd":"/a/dotfiles","agent_id":"agent_01","agent_type":"Explore"}"#,
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "the subagent holding this wait has ended, so the wait ends with it"
    );
}

#[test]
fn a_refused_tool_call_holds_no_wait_at_all() {
    // NOBODY IS WAITING ON A DENIAL. `PermissionDenied` fires after the
    // auto-mode classifier refused a call on its own, which is a decision
    // already taken rather than a question, so it is an observation: it
    // neither arms a wait nor takes one, and the wait a real question armed
    // beside it stays exactly as unanswered as it was.
    let sandbox = Sandbox::new("lights-blocked-denied-observes");
    sandbox.write_config(LAMPS_ON);
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "denied",
        r#"{"session_id":"s2","cwd":"/a/dotfiles","tool_name":"Bash","tool_input":{"command":"ls"},"reason":"refused"}"#,
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "a denial arms no wait, because nobody is answering one"
    );
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "blocked",
        r#"{"session_id":"s1","message":"may I"}"#,
    );
    hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "denied",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","tool_name":"Bash","tool_input":{"command":"ls"},"reason":"refused"}"#,
    );
    assert_eq!(
        waiting_sessions(&sandbox),
        vec!["s1".to_string()],
        "and it takes no wait either: the question beside it is still unanswered"
    );
}

#[test]
fn plan_ready_is_no_longer_a_state_word_this_binary_serves() {
    // THE ARM IS GONE, NOT REPOINTED. Nothing declares `plan-ready` after the
    // `ExitPlanMode` matcher was routed to `resolved`, so it is dead code, and
    // dead code is deleted rather than left in case a future declaration
    // wants it. An invocation carrying it is refused the way any unknown event
    // is: a line on stderr, exit 0, and nothing written.
    let sandbox = Sandbox::new("lights-blocked-plan-ready-unknown");
    sandbox.write_config(LAMPS_ON);
    let output = hook_with(
        with_state_dir(&sandbox),
        &sandbox,
        "plan-ready",
        r#"{"session_id":"s1","cwd":"/a/dotfiles","tool_name":"ExitPlanMode"}"#,
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown hook event `plan-ready`"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        waiting_sessions(&sandbox).is_empty(),
        "an event this binary no longer serves writes nothing"
    );
}
