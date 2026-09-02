//! The harness hooks: what a Claude Code or Codex event carries, and how a
//! turn becomes the event the engine already knows how to route.
//!
//! Everything here is PURE. The payload arrives as text, the transcript
//! arrives as text, the condenser's answer arrives as text, and each is turned
//! into a decision without touching the world. The spawns and the files live
//! at the composition root, which is what lets the whole turn-to-notification
//! path be tested without a harness, a transcript or a network.

/// The fields any harness hook payload may carry. Everything is optional
/// because every harness sends a different subset and a missing field is a
/// state, never an error: this runs on a path that must exit 0.
#[derive(Debug, Default, PartialEq)]
pub struct HookPayload {
    pub session_id: String,
    pub cwd: String,
    pub transcript_path: String,
    /// The harness's own copy of the final assistant text. Claude Code
    /// documents that a Stop hook can fire before the transcript write
    /// completes and recommends this field instead.
    pub last_assistant_message: String,
    /// Unique identifier for the subagent. The Claude Code hooks reference
    /// (2.1.257) states it is "present only when the hook fires inside a
    /// subagent call", so an empty value here is the ordinary main-thread
    /// case, never a parse failure.
    pub agent_id: String,
    /// The agent name, for example "Explore" or "security-reviewer". The same
    /// reference states it is present "when the session uses `--agent` or the
    /// hook fires inside a subagent", so it arrives together with `agent_id`
    /// more often than alone.
    pub agent_type: String,
    /// The current permission mode: `default`, `plan`, `acceptEdits`, `auto`,
    /// `dontAsk` or `bypassPermissions`. The reference states "not all events
    /// receive this field", so empty is the ordinary case for most of them.
    pub permission_mode: String,
    /// Which tool a `PermissionRequest` is about, RAW and unflattened, unlike
    /// the composed `message` below. A connected Model Context Protocol
    /// server names its own tools, so this is remote text; it is safe to
    /// record on its own because, like `agent_id` and `state`, it is a NAME
    /// rather than free text, filtered the same way before it is ever
    /// printed.
    pub tool_name: String,
    /// Whether the payload CARRIED an `agent_id` key at all, whatever its
    /// value. The reference promises only ABSENCE on the main thread, so a
    /// key that is present but null, numeric or empty is still a subagent
    /// signal: `resolved` reads this, never the string, to decide whose wait
    /// a batch answered, so a malformed field fails closed (clears nothing).
    pub in_subagent: bool,
    /// What a non-turn event (a permission prompt, a plan) is about.
    pub message: String,
}

/// Read a payload, treating anything unparseable as an empty one.
pub fn parse_payload(payload_json: &str) -> HookPayload {
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_json) else {
        return HookPayload::default();
    };
    let text = |key: &str| {
        payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    HookPayload {
        session_id: text("session_id"),
        cwd: text("cwd"),
        transcript_path: text("transcript_path"),
        last_assistant_message: text("last_assistant_message"),
        agent_id: text("agent_id"),
        agent_type: text("agent_type"),
        permission_mode: text("permission_mode"),
        tool_name: text("tool_name"),
        in_subagent: payload.get("agent_id").is_some(),
        // The asking MCP server in front of its own prompt, then `.message //
        // .detail` as the bash read it, then the error a dead turn reports,
        // and then the tool the request is about for the harnesses that send
        // none of the three.
        //
        // THE TWO PLAIN READS ARE FLATTENED like the other three composers.
        // Every entry in this chain is a candidate for the SAME rendered line,
        // so a control byte or a newline scrubbed out of three of them and left
        // in the other two reaches the same banner by whichever road the
        // harness happened to use; `message` and `detail` are the common roads.
        // The fields above are not: a path or a session id is matched and
        // opened rather than rendered, and flattening one would rewrite a name
        // the filesystem gave.
        message: [
            elicitation_request(&payload),
            flattened(&text("message")),
            flattened(&text("detail")),
            reported_error(&payload),
        ]
        .into_iter()
        .find(|stated| !stated.is_empty())
        .unwrap_or_else(|| tool_request(&payload)),
    }
}

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
fn elicitation_request(payload: &serde_json::Value) -> String {
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
fn tool_request(payload: &serde_json::Value) -> String {
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
fn reported_error(payload: &serde_json::Value) -> String {
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
fn one_line(value: &serde_json::Value) -> String {
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
fn flattened(text: &str) -> String {
    text.split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Enough to name a tool and the start of what it was handed, and no more: the
/// rest is a card nobody reads.
const TOOL_REQUEST_MAX_CHARS: usize = 320;

/// The assistant text of the transcript's LAST turn.
///
/// The transcript is one JSON object per line. The last USER line marks where
/// the turn began, and every assistant text block after it is the turn's
/// answer, joined the way the harness renders it. A line that will not parse
/// is skipped rather than fatal: the tail is cut mid-line by design, so the
/// first line is routinely half an object.
pub fn transcript_reply(transcript_tail: &str) -> String {
    let entries: Vec<serde_json::Value> = transcript_tail
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .filter(|entry: &serde_json::Value| entry.is_object())
        .collect();
    let last_user = entries.iter().rposition(|entry| {
        entry.get("type").and_then(serde_json::Value::as_str) == Some("user")
            && matches!(
                entry.pointer("/message/content"),
                Some(serde_json::Value::String(_))
            ) | (entry
                .pointer("/message/content/0/type")
                .and_then(serde_json::Value::as_str)
                == Some("text"))
    });
    entries
        .iter()
        .skip(last_user.map_or(0, |index| index + 1))
        .filter(|entry| entry.get("type").and_then(serde_json::Value::as_str) == Some("assistant"))
        .filter_map(|entry| entry.pointer("/message/content")?.as_array())
        .flatten()
        .filter(|block| block.get("type").and_then(serde_json::Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text")?.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The condenser's verdict: the last `STATE|SUMMARY` line it printed.
///
/// THE SUMMARY HALF IS WHAT MAKES IT USABLE. A matched state with nothing
/// after the pipe used to count as a hit, which shipped a title-only
/// notification over a turn that had text (live 2026-08-12). A summary of
/// spaces renders as blank as no summary, so it must carry one non-blank
/// character or the whole line is a miss and the caller falls back.
pub fn condenser_verdict(codex_output: &str) -> Option<(String, String)> {
    codex_output
        .lines()
        .filter_map(|line| {
            let (state, summary) = line.split_once('|')?;
            matches!(state, "done" | "asking" | "blocked")
                .then(|| (state.to_string(), summary.to_string()))
        })
        .rfind(|(_, summary)| summary.chars().any(|character| !character.is_whitespace()))
}

/// The prompt the condenser answers. One line out, so the caller can parse it
/// without a model-shaped grammar.
///
/// `asking` IS NARROWED TO A QUESTION FOR THE HUMAN, not a turn that merely
/// mentions waiting. A live status line reading "waiting on the remaining
/// reviews, then I bring you the one checkpoint" was classified `asking` under
/// the looser wording (OBS-3), which lit the blue lamp and carded the operator
/// over a turn asking them nothing: the word "waiting" was enough to match,
/// whoever the turn was waiting on. There is no keyword rule to fix; the
/// condenser is a model call, and this sentence is the whole rule it reads.
pub fn condenser_prompt(reply: &str) -> String {
    format!(
        "Summarize this AI coding agent's last turn for a brief phone notification, then classify it.
Output EXACTLY one line and nothing else: STATE|SUMMARY
STATE is one of: done (finished its work, or is only reporting status while it waits on OTHER AGENTS OR TOOLS), asking (has a question or choice that needs YOU, the human operator, to answer before it can continue), blocked (needs your permission or input to proceed).
SUMMARY is two or three sentences, up to 320 characters, plain text, no newlines, covering what was done plus any decision or question raised.

Turn:
{reply}"
    )
}

/// Whether a blocking event is handed to moshi for a round trip.
///
/// Only the harnesses pns registers itself for: the name arrives from a config
/// file, so it is MATCHED rather than pasted into a subcommand handed to a
/// third-party binary.
pub fn moshi_subcommand(agent: &str) -> Option<String> {
    matches!(agent, "claude" | "codex").then(|| format!("{agent}-hook"))
}

/// Whether a subcommand handed to us by moshi's OWN generated extension may be
/// passed through to moshi-hook.
///
/// pi and omp reach the gate directly (`helperBinary pi-hook`), so the word
/// arrives from a file moshi generates while moshi-hook's positional is a
/// PATH. Shape only, not a roster: the harness list is moshi's and grows. An
/// unvetted word here is this repo handing a third-party binary a filesystem
/// argument nobody chose.
pub fn is_harness_subcommand(subcommand: &str) -> bool {
    let (name, suffix) = match subcommand.split_once('-') {
        Some(parts) => parts,
        None => return false,
    };
    suffix == "hook" && !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{
        HookPayload, condenser_prompt, condenser_verdict, moshi_subcommand, one_line,
        parse_payload, transcript_reply,
    };

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

    #[test]
    fn every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel() {
        // A LINE FROM THIS FUNCTION IS RENDERED SOMEWHERE THAT OBEYS IT: a
        // terminal banner, a herdr pane, a Discord post. C0 is not text, and
        // the whitespace this already flattens is the only part of it that
        // was ever handled, so ESC, BEL and NUL rode through verbatim.
        //
        // THE MOTIVATING FEEDER IS PROVIDER-CONTROLLED. `error` is whatever the
        // API said, and an escape sequence in it is the one string here nobody
        // on this machine wrote.
        // EVERY CODEPOINT IN THE SET, not a representative of each run. The
        // scrub is written as one category test, so the only way it can be
        // wrong is per codepoint, and a matrix of samples cannot see a single
        // exemption: measured, a `flattened` that let U+0002 through passed
        // every test in this crate while leaking that byte to a banner. The set
        // is `char::is_control` itself, which is exactly Cc: C0, DEL and C1.
        for codepoint in (0x00..=0x1f_u32).chain([0x7f]).chain(0x80..=0x9f) {
            let control = char::from_u32(codepoint).expect("a Cc codepoint");
            assert!(control.is_control(), "U+{codepoint:04X} is the Cc set");
            assert_eq!(
                one_line(&serde_json::Value::String(format!("a{control}b"))),
                "a b",
                "U+{codepoint:04X} reached a channel verbatim"
            );
        }

        // AND THE SEQUENCES THOSE BYTES ARRIVE IN, which the loop above cannot
        // state: a scrub that removed the escape and left `[31m` or an OSC
        // title behind would still pass every assertion up there, and what
        // reaches the operator is the whole sequence rather than one byte.
        for (raw, scrubbed, class) in [
            ("a\u{1b}[31mb", "a [31mb", "a colour sequence"),
            (
                "a\u{1b}]0;title\u{7}b",
                "a ]0;title b",
                "an OSC title sequence",
            ),
            // AND THE WHITESPACE IT ALREADY FLATTENED still flattens the same
            // way, which is what makes this a widening rather than a rewrite.
            ("a\nb\tc  d", "a b c d", "the whitespace it already handled"),
        ] {
            assert_eq!(
                one_line(&serde_json::Value::String(raw.to_string())),
                scrubbed,
                "{class} reached a channel verbatim"
            );
        }

        // ORDINARY TEXT IS UNTOUCHED, which is the control the sweep needs:
        // scrubbing by codepoint RANGE rather than by category would take
        // multibyte characters with it, and an operator's own prose is full of
        // them.
        for kept in ["café", "日本語", "→ ✓ ×", "naïve résumé ½ ±"] {
            assert_eq!(
                one_line(&serde_json::Value::String(kept.to_string())),
                kept,
                "text that is not a control byte must pass through"
            );
        }

        // EVERY SHAPE THE FUNCTION WALKS, because a scrub on the string arm
        // alone leaves the same byte reaching a channel one nesting level down,
        // and an object's KEY is provider-controlled exactly like its value.
        assert_eq!(
            one_line(&serde_json::json!(["a\u{1b}b", "c"])),
            "a b c",
            "an array member is scrubbed like a bare string"
        );
        assert_eq!(
            one_line(&serde_json::json!({"k\u{7}k": "v\u{1b}v"})),
            "k k=v v",
            "an object's key is scrubbed beside its value"
        );
    }

    #[test]
    fn every_payload_string_a_card_is_built_from_is_scrubbed_and_not_the_arguments_alone() {
        // A CARD IS COMPOSED FROM FOUR PAYLOAD STRINGS and the scrub reached
        // one of them. `tool_input` went through `one_line` while the
        // `tool_name` formatted in front of it on the same line did not, and
        // `message` and `detail`, which are the first two of the chain and the
        // ones the common harnesses actually send, went through nothing at all.
        // The rule stated at `flattened` (a line from here is rendered
        // somewhere that OBEYS it) holds for every string on the card or it
        // holds for none of them: the same ESC reaches the same banner by
        // whichever road is left open.

        // ALL THREE IN ONE ASSERT, so a run names every field still riding
        // through rather than the first one only.
        //
        // `tool_name` is remote text: a connected Model Context Protocol server
        // names its own tools, and a Codex permission payload carries neither
        // `message` nor `detail`, so that name IS the whole card, shown at the
        // moment the operator is being asked to decide. `message` and `detail`
        // are the first two of the chain and carry the NEWLINE half of the
        // guarantee too, which is older than the control scrub: a second line
        // in either breaks the single rendered line every channel expects.
        let cards = [
            "{\"tool_name\":\"Bash\\u001b[2J\\u0007\",\"tool_input\":{\"c\":\"ls\"}}",
            "{\"message\":\"plan\\u001b[2J\\u0007 ready\\nsecond line\"}",
            "{\"detail\":\"a\\u0000b\\nc\"}",
        ]
        .map(|payload| parse_payload(payload).message);
        assert_eq!(
            cards,
            ["Bash [2J: c=ls", "plan [2J ready second line", "a b c"]
        );

        // AND A STRING THAT IS NOTHING BUT CONTROL BYTES SAYS NOTHING, so the
        // chain moves on to the next thing that was actually stated rather than
        // carding a blank where a message appeared to be.
        assert_eq!(
            parse_payload("{\"message\":\"\\u0007\",\"detail\":\"the real one\"}").message,
            "the real one"
        );
    }

    #[test]
    fn a_provider_error_carrying_an_escape_sequence_cannot_dress_up_a_card() {
        // THROUGH THE PAYLOAD, so this pins the feeder and not only the
        // helper: the error field is the one string on this path that a remote
        // provider writes end to end.
        let payload = parse_payload("{\"error\":\"API Error: 500\\u001b[2J\\u0007 cleared\"}");
        assert_eq!(payload.message, "API Error: 500 [2J cleared");
    }

    #[test]
    fn an_error_is_kept_to_one_line_and_cut_from_the_head_like_a_tool_request() {
        // A provider error arrives with a stack trace behind it often enough
        // that the raw string is a wall. A newline in it would break the
        // single rendered line every channel expects, and the cut keeps the
        // HEAD because an API error states its kind first.
        let payload = parse_payload(r#"{"error":"API Error: 500\n  at fetch\n  at main"}"#);
        assert_eq!(payload.message, "API Error: 500 at fetch at main");

        let long = "x".repeat(5_000);
        let payload = parse_payload(&format!(r#"{{"error":"API Error: {long}"}}"#));
        assert!(payload.message.starts_with("API Error: xxx"));
        // THE CAP ITSELF, spelled out rather than read back off the constant
        // the cut uses: an assertion phrased against `TOOL_REQUEST_MAX_CHARS`
        // agrees with whatever that constant is moved to, which is the one
        // thing this is here to catch.
        assert_eq!(
            payload.message.chars().count(),
            320,
            "an error is cut at the shared cap, not merely somewhere under it"
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

    #[test]
    fn the_reply_is_the_assistant_text_of_the_last_turn_only() {
        let transcript = r#"{"type":"user","message":{"content":"first ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"an older answer"}]}}
{"type":"user","message":{"content":"second ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"the newest answer"}]}}"#;
        assert_eq!(transcript_reply(transcript), "the newest answer");
    }

    #[test]
    fn several_text_blocks_in_one_turn_join_the_way_the_harness_renders_them() {
        let transcript = r#"{"type":"user","message":{"content":"ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"one"},{"type":"text","text":"two"}]}}"#;
        assert_eq!(transcript_reply(transcript), "one\n\ntwo");
    }

    #[test]
    fn a_first_line_cut_in_half_by_the_tail_is_skipped_not_fatal() {
        // The reader takes the last few megabytes, so the first line is
        // routinely half an object. Refusing the whole transcript over it
        // would lose the reply on every long session.
        let transcript = r#"ge":{"content":[{"type":"text","text":"cut"}]}}
{"type":"user","message":{"content":"ask"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"kept"}]}}"#;
        assert_eq!(transcript_reply(transcript), "kept");
    }

    #[test]
    fn a_transcript_with_no_readable_turn_yields_nothing_rather_than_guessing() {
        assert_eq!(transcript_reply(""), "");
        assert_eq!(transcript_reply("not json at all"), "");
        assert_eq!(
            transcript_reply(r#"{"type":"assistant","message":{"content":[]}}"#),
            ""
        );
    }

    #[test]
    fn tool_blocks_are_not_the_reply() {
        let transcript = r#"{"type":"user","message":{"content":"ask"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"},{"type":"text","text":"said"}]}}"#;
        assert_eq!(transcript_reply(transcript), "said");
    }

    #[test]
    fn the_condensers_last_usable_line_wins() {
        assert_eq!(
            condenser_verdict("noise\ndone|first\nasking|second"),
            Some(("asking".to_string(), "second".to_string()))
        );
    }

    #[test]
    fn a_state_with_a_blank_summary_is_a_miss_not_a_hit() {
        // It used to count, which shipped a title-only notification over a
        // turn that had text.
        assert_eq!(condenser_verdict("done|"), None);
        assert_eq!(condenser_verdict("done|   "), None);
        assert_eq!(condenser_verdict(""), None);
        assert_eq!(condenser_verdict("just some prose"), None);
    }

    #[test]
    fn a_state_the_prompt_never_offered_is_not_a_verdict() {
        assert_eq!(condenser_verdict("finished|all good"), None);
    }

    #[test]
    fn the_prompt_carries_the_turn_and_asks_for_one_line() {
        let prompt = condenser_prompt("what happened");
        assert!(prompt.contains("EXACTLY one line"));
        assert!(prompt.ends_with("Turn:\nwhat happened"));
    }

    #[test]
    fn the_gate_vouches_for_the_shape_of_a_subcommand_it_did_not_choose() {
        assert!(super::is_harness_subcommand("pi-hook"));
        assert!(super::is_harness_subcommand("claude-hook"));
        assert!(!super::is_harness_subcommand("hook"));
        assert!(!super::is_harness_subcommand("-hook"));
        assert!(!super::is_harness_subcommand("Pi-hook"));
        assert!(!super::is_harness_subcommand("pi-hook; rm -rf /"));
        assert!(!super::is_harness_subcommand("../../etc/passwd"));
        assert!(!super::is_harness_subcommand(""));
    }

    #[test]
    fn only_the_harnesses_pns_registers_for_are_forwarded_to_moshi() {
        assert_eq!(moshi_subcommand("claude").as_deref(), Some("claude-hook"));
        assert_eq!(moshi_subcommand("codex").as_deref(), Some("codex-hook"));
        assert_eq!(moshi_subcommand("pi"), None);
        assert_eq!(moshi_subcommand(""), None);
        assert_eq!(moshi_subcommand("claude; rm -rf /"), None);
    }
}
