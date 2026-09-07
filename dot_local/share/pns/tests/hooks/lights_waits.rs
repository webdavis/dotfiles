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
