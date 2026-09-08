use super::*;

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
