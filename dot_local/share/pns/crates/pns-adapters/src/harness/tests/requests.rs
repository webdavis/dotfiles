use super::*;
#[test]
fn a_codex_permission_request_says_which_tool_wants_what() {
    // Codex 0.147 sends tool_name and tool_input and NEITHER message nor
    // detail, so every Codex approval reached the banner and the durable
    // log carrying nothing but the state word: an operator was asked to
    // approve something the card could not name.
    let payload = parse_payload(
        r#"{"hook_event_name":"PermissionRequest","session_id":"s1","cwd":"/a/b",
                "tool_name":"shell","tool_input":{"command":["bash","-lc","rm -rf build"]}}"#,
    );
    assert_eq!(payload.message, "shell: command=bash -lc rm -rf build");
}

#[test]
fn an_elicitation_says_which_server_is_asking_in_front_of_what_it_asked() {
    // An MCP server that stops mid-tool-call to ask the operator for
    // input states its own `message`, so the chain resolves at step one
    // and the card carries the prompt with NO attribution: "Please
    // authorize Gmail access" on a phone, from nobody. Which of the
    // connected servers wants the credential is the one thing that
    // decides whether to answer it. THE PAYLOAD IS THE BINARY'S OWN FIELD
    // SET (`mcp_server_name` and `message` required, `mode`, `url`,
    // `elicitation_id` and `requested_schema` optional, over the base
    // spread every other event shares), carrying every optional so the
    // assertion also says which of them reach the card: none.
    let payload = parse_payload(
        r#"{"hook_event_name":"Elicitation","session_id":"s1","cwd":"/a/dotfiles",
                "mcp_server_name":"composio","message":"Please authorize Gmail access",
                "mode":"url","url":"https://backend.composio.dev/authorize/abc123",
                "elicitation_id":"elic_01","requested_schema":{"api_key":{"type":"string"}}}"#,
    );
    assert_eq!(payload.message, "composio: Please authorize Gmail access");

    // The harness requires `message` but its schema allows the EMPTY
    // string, and a server that asks with one still deserves a name on
    // the card rather than a dangling "composio: " or nothing at all.
    let payload = parse_payload(r#"{"mcp_server_name":"composio","message":""}"#);
    assert_eq!(payload.message, "composio");

    // A JSON null is that same absence, not a prompt: flattened it would
    // card the literal WORD "null" as what the server asked for.
    let payload = parse_payload(r#"{"mcp_server_name":"composio","message":null}"#);
    assert_eq!(payload.message, "composio");

    // A name made of whitespace names nobody, so there is no attribution
    // to put in front and the stated message stands alone rather than
    // arriving behind a blank prefix and a colon.
    let payload = parse_payload(r#"{"mcp_server_name":"   ","message":"authorize Gmail"}"#);
    assert_eq!(payload.message, "authorize Gmail");
}

#[test]
fn a_payload_that_states_its_own_message_is_never_second_guessed() {
    // The composed line is a LAST resort: a harness that says what it
    // wants keeps saying it, whatever else the payload carries.
    assert_eq!(
        parse_payload(r#"{"message":"may I","tool_name":"shell"}"#).message,
        "may I"
    );
    assert_eq!(
        parse_payload(r#"{"detail":"may I","tool_name":"shell"}"#).message,
        "may I"
    );
}

#[test]
fn a_tool_request_is_cut_from_the_head_and_kept_to_one_line() {
    // A write carries the whole file in its input. The tool and the start
    // of its arguments identify the request; the rest is a phone card
    // nobody can read, and a newline in it would break the rendered line.
    let payload = parse_payload(
        r#"{"tool_name":"write","tool_input":{"path":"/a/b","contents":"line one\nline two"}}"#,
    );
    assert!(
        payload
            .message
            .starts_with("write: contents=line one line two"),
        "got {:?}",
        payload.message
    );
    assert!(!payload.message.contains('\n'));

    let long = "x".repeat(5_000);
    let payload = parse_payload(&format!(r#"{{"tool_name":"write","tool_input":"{long}"}}"#));
    assert!(payload.message.starts_with("write: xxx"));
    assert!(payload.message.chars().count() < 400, "an uncapped request");
}

#[test]
fn an_elicitation_prompt_is_kept_to_one_line_and_cut_from_the_head_too() {
    // An elicitation prompt describes a FORM, so it is multi-line often
    // enough that the raw string would break the single rendered line
    // every channel expects, and long enough that a phone card would be
    // all schema. The same flatten and the same cap the two sibling
    // composers use, cutting the HEAD because the server and the start of
    // what it wants are what identify the ask.
    let payload = parse_payload(
        r#"{"hook_event_name":"Elicitation","session_id":"s1","cwd":"/a/dotfiles",
                "mcp_server_name":"composio","message":"Fill this form:\n  name\n  email"}"#,
    );
    assert_eq!(payload.message, "composio: Fill this form: name email");
    assert!(!payload.message.contains('\n'));

    // The SERVER half goes through that same flatten, so a name carrying
    // a newline cannot break the line the prompt half was flattened to
    // protect.
    let payload = parse_payload(r#"{"mcp_server_name":"corp\nprod","message":"authorize"}"#);
    assert_eq!(payload.message, "corp prod: authorize");
    assert!(!payload.message.contains('\n'));

    let long = "x".repeat(5_000);
    let payload = parse_payload(&format!(
        r#"{{"mcp_server_name":"composio","message":"{long}"}}"#
    ));
    assert!(
        payload.message.starts_with("composio: xxx"),
        "got {:?}",
        payload.message
    );
    assert!(payload.message.chars().count() < 400, "an uncapped prompt");
}
