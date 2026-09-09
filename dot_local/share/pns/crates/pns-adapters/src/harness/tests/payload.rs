use super::*;
#[test]
fn a_payload_yields_every_field_the_hooks_read() {
    let payload = parse_payload(
        r#"{"session_id":"s1","cwd":"/a/b","transcript_path":"/t.jsonl","last_assistant_message":"the reply","agent_id":"agent_01"}"#,
    );
    assert_eq!(payload.session_id, "s1");
    assert_eq!(payload.cwd, "/a/b");
    assert_eq!(payload.transcript_path, "/t.jsonl");
    assert_eq!(payload.last_assistant_message, "the reply");
    assert_eq!(payload.agent_id, "agent_01");
}

#[test]
fn an_agent_id_is_absent_rather_than_a_parse_failure_on_the_main_thread() {
    // THE HOOKS REFERENCE STATES IT PLAINLY: `agent_id` is "present only
    // when the hook fires inside a subagent call", so a main-thread
    // payload naming none is the ordinary case, never something to guess
    // at or report on.
    assert_eq!(parse_payload(r#"{"session_id":"s1"}"#).agent_id, "");
}

#[test]
fn a_present_agent_id_of_any_shape_marks_a_subagent_and_absence_does_not() {
    // THE REFERENCE PROMISES ONLY ABSENCE ON THE MAIN THREAD, so a key
    // that is there but null, numeric or empty is not proof of the main
    // thread; only a missing key is.
    for shape in ["null", "7", "\"\"", "\"agent_01\""] {
        let payload = parse_payload(&format!(r#"{{"session_id":"s1","agent_id":{shape}}}"#));
        assert!(payload.in_subagent, "agent_id:{shape} is a present key");
    }
    assert!(!parse_payload(r#"{"session_id":"s1"}"#).in_subagent);
    assert!(!parse_payload("not json").in_subagent);
}

#[test]
fn a_permission_request_yields_its_mode_agent_and_raw_tool_name() {
    // THE BINARY'S OWN FIELD SET (2.1.241, `CLAUDE_APPROVAL` in
    // tests/hooks.rs), so this is the real shape rather than a reduction
    // of it.
    let payload = parse_payload(
        r#"{"session_id":"s1","transcript_path":"/dev/null","cwd":"/a/dotfiles",
                "prompt_id":"prompt_01","permission_mode":"default","agent_id":"agent_01",
                "agent_type":"main","effort":"medium","hook_event_name":"PermissionRequest",
                "tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"}}"#,
    );
    assert_eq!(payload.permission_mode, "default");
    assert_eq!(payload.agent_id, "agent_01");
    assert_eq!(payload.agent_type, "main");
    assert_eq!(payload.tool_name, "Bash");
}

#[test]
fn permission_mode_agent_type_and_tool_name_are_absent_rather_than_guessed() {
    // "NOT ALL EVENTS RECEIVE THIS FIELD," the reference says of
    // `permission_mode`, and `agent_type` arrives only with `--agent` or a
    // subagent: absent is the ordinary case for most events, never a
    // parse failure.
    let payload = parse_payload(r#"{"session_id":"s1"}"#);
    assert_eq!(payload.permission_mode, "");
    assert_eq!(payload.agent_type, "");
    assert_eq!(payload.tool_name, "");
}

#[test]
fn a_payload_that_will_not_parse_is_empty_rather_than_fatal() {
    // The hook exits 0 whatever arrives: a harness sending garbage costs a
    // notification, never the turn it was reporting on.
    assert_eq!(parse_payload("not json"), HookPayload::default());
    assert_eq!(parse_payload(""), HookPayload::default());
}

#[test]
fn detail_stands_in_for_message_because_the_harnesses_disagree() {
    assert_eq!(parse_payload(r#"{"message":"m"}"#).message, "m");
    assert_eq!(parse_payload(r#"{"detail":"d"}"#).message, "d");
    assert_eq!(parse_payload(r#"{"message":"","detail":"d"}"#).message, "d");
}

#[test]
fn a_dead_turns_error_becomes_the_message_when_the_payload_states_nothing_else() {
    // Claude Code's StopFailure payload carries the failure in `error` and
    // states neither a message nor a detail (its emitter builds the input
    // as `{...base, error, error_details, last_assistant_message}`), so
    // without this element the card reporting a dead turn cannot say why
    // it died.
    let payload = parse_payload(
        r#"{"hook_event_name":"StopFailure","session_id":"s1","cwd":"/a/b",
                "error":"API Error: 500 internal server error"}"#,
    );
    assert_eq!(payload.message, "API Error: 500 internal server error");
}

#[test]
fn a_stated_message_or_detail_still_outranks_an_error() {
    // The error was APPENDED to the chain, not put in front of it. Every
    // event pns handles today states a message, a detail or a tool, so an
    // error placed higher would rewrite what an already-working event
    // says the moment a harness starts sending both.
    assert_eq!(parse_payload(r#"{"message":"m","error":"e"}"#).message, "m");
    assert_eq!(parse_payload(r#"{"detail":"d","error":"e"}"#).message, "d");
}

#[test]
fn a_notification_yields_its_type_and_message() {
    // THE BINARY'S OWN FIELD SET (hooks reference matcher table): a
    // `Notification` event carries `notification_type` beside the same
    // `message` every other event states, so the quota arm reads both
    // off fields this parser already exposes.
    let payload = parse_payload(
        r#"{"session_id":"s1","cwd":"/a/dotfiles",
                "hook_event_name":"Notification","notification_type":"quota_auto_resume_stale",
                "message":"Your usage limit has reset"}"#,
    );
    assert_eq!(payload.notification_type, "quota_auto_resume_stale");
    assert_eq!(payload.message, "Your usage limit has reset");
}

#[test]
fn notification_type_is_absent_rather_than_guessed_off_a_non_notification_event() {
    assert_eq!(
        parse_payload(r#"{"session_id":"s1"}"#).notification_type,
        ""
    );
}

#[test]
fn a_payload_naming_no_tool_and_no_message_still_says_nothing_rather_than_guessing() {
    assert_eq!(parse_payload(r#"{"session_id":"s1"}"#).message, "");
    assert_eq!(parse_payload(r#"{"tool_input":{}}"#).message, "");
    // A stated-but-empty error and a null one are both nothing said. The
    // flattener renders a JSON null as the WORD "null", which would put
    // that word on the card as though a harness had reported it.
    assert_eq!(parse_payload(r#"{"error":""}"#).message, "");
    assert_eq!(parse_payload(r#"{"error":null}"#).message, "");
}
