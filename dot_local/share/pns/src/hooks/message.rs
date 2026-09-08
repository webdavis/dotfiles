/// Which Model Context Protocol server is asking, in front of what it asked.
///
/// An elicitation payload states its own `message`, so without this the card
/// carries the prompt with no attribution: "Please provide your API key" on a
/// phone, from nobody. The operator cannot tell which of the connected servers
/// wants the credential, which is the one thing that decides whether to answer.
///
/// IN FRONT OF THE CHAIN, where `reported_error` was deliberately put behind
/// it, and the difference is the gate. `mcp_server_name` appears in exactly
/// two hook input schemas in the whole 2.1.241 vocabulary, `Elicitation` and
/// `ElicitationResult`, and Codex 0.149.1 sends it on nothing, so this returns
/// the empty string for every payload pns handles today. It also PREFIXES
/// rather than rewrites: the message the harness stated is preserved ahead of
/// the cap, with the asker in front of it.
pub(super) fn elicitation_request(payload: &serde_json::Value) -> String {
    // BOTH halves through the same flatten: a newline in the name a server
    // registered under would break the rendered line exactly as one in the
    // prompt would, and a name that flattens to nothing names nobody, so
    // there is no attribution to put in front of the prompt.
    let stated = |key: &str| {
        payload
            .get(key)
            // A JSON null flattens to the WORD "null", which is neither a
            // server anyone registered nor a prompt anyone sent.
            .filter(|value| !value.is_null())
            .map(one_line)
            .unwrap_or_default()
    };
    let server = stated("mcp_server_name");
    if server.is_empty() {
        return String::new();
    }
    let asked = stated("message");
    let request = if asked.is_empty() {
        server
    } else {
        format!("{server}: {asked}")
    };
    // The HEAD, like a tool request: the server plus the start of what it
    // wants identifies the ask, and an elicitation describing a form runs long.
    request.chars().take(TOOL_REQUEST_MAX_CHARS).collect()
}

/// What a permission request is asking for, when the payload says only which
/// tool wants to run.
///
/// Codex 0.147 PermissionRequest payloads carry `tool_name` and `tool_input`
/// and NEITHER `message` nor `detail` (measured 2026-08-19), so every Codex
/// approval reached the banner and the durable log carrying nothing but the
/// state word `blocked`. An operator deciding from a phone needs the tool and
/// what it wants to do with it.
pub(super) fn tool_request(payload: &serde_json::Value) -> String {
    // FLATTENED LIKE THE ARGUMENTS IT IS FORMATTED IN FRONT OF. A connected
    // Model Context Protocol server names its own tools, so this is remote
    // text on the same rendered line as the `tool_input` beside it, and
    // scrubbing one half of a composed string is scrubbing neither.
    let tool = flattened(
        payload
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default(),
    );
    let arguments = payload.get("tool_input").map(one_line).unwrap_or_default();
    let request = match (tool.as_str(), arguments.as_str()) {
        ("", arguments) => arguments.to_string(),
        (tool, "") => tool.to_string(),
        (tool, arguments) => format!("{tool}: {arguments}"),
    };
    // THE HEAD, not the tail: a write carries the whole file in its input, and
    // it is the tool plus the start of its arguments that identifies the
    // request. The reply's own cap keeps the end instead, because there the
    // last thing said is the summary.
    request.chars().take(TOOL_REQUEST_MAX_CHARS).collect()
}

/// The failure a dead turn reports, normalized the way a tool request is.
///
/// Claude Code's StopFailure payload carries the whole provider error in
/// `error`, and it arrives with a stack trace behind it often enough that the
/// raw string is a wall. Flattened through `one_line`, because a newline would
/// break the single rendered line every channel expects, and cut from the same
/// HEAD at the same cap, because an API error states its kind first.
pub(super) fn reported_error(payload: &serde_json::Value) -> String {
    payload
        .get("error")
        // A JSON null flattens to the WORD "null", which is a guess, not a
        // reported failure.
        .filter(|error| !error.is_null())
        .map(one_line)
        .unwrap_or_default()
        .chars()
        .take(TOOL_REQUEST_MAX_CHARS)
        .collect()
}

/// A JSON value as one line of plain text: a string bare, an array's members
/// joined, an object's as `key=value`. Nested JSON on a phone card is
/// punctuation an operator has to read past to find the command.
///
/// Recursion is bounded by the parse that produced the value: serde_json
/// refuses a document nested deeper than its own limit, so there is no depth
/// here that was not already accepted as a payload.
///
/// EVERY STRING IT WALKS IS SCRUBBED, keys included. An object's key is written
/// by whoever wrote its value, so scrubbing one and not the other leaves the
/// same byte on the same card by a different road.
pub(super) fn one_line(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => flattened(text),
        serde_json::Value::Array(members) => {
            members.iter().map(one_line).collect::<Vec<_>>().join(" ")
        }
        serde_json::Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| format!("{}={}", flattened(key), one_line(value)))
            .collect::<Vec<_>>()
            .join(" "),
        scalar => scalar.to_string(),
    }
}

/// One string as one line: runs of whitespace AND of control characters become
/// single spaces, and the ends are trimmed.
///
/// FLATTENED, because a newline inside a command would otherwise break the
/// single rendered line every channel expects. That much this always did.
///
/// AND CONTROL CHARACTERS GO THE SAME WAY, which it did not. A line from here
/// is rendered somewhere that OBEYS what it is handed: a terminal banner, a
/// herdr pane, a Discord post. `split_whitespace` handled the six control
/// characters that happen to be whitespace and passed the rest of C0 through
/// untouched, so an ESC, a BEL or a NUL reached a channel verbatim. The feeder
/// that makes this more than theory is `reported_error`: the provider's own
/// error string, the one value on this path that nothing on this machine
/// wrote.
///
/// BY CATEGORY AND NEVER BY CODEPOINT RANGE. `char::is_control` is exactly the
/// Cc set (C0, DEL and C1), so multibyte text an operator actually wrote passes
/// through whole; a range test written in bytes would cut a character in half
/// and a range written in codepoints would have to restate the same set worse.
pub fn flattened(text: &str) -> String {
    text.split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Enough to name a tool and the start of what it was handed, and no more: the
/// rest is a card nobody reads.
const TOOL_REQUEST_MAX_CHARS: usize = 320;
