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
        // `.message // .detail`, as the bash read it, and then the tool the
        // request is about for the harnesses that send neither.
        message: [text("message"), text("detail")]
            .into_iter()
            .find(|stated| !stated.is_empty())
            .unwrap_or_else(|| tool_request(&payload)),
    }
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
    let tool = payload
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let arguments = payload.get("tool_input").map(one_line).unwrap_or_default();
    let request = match (tool, arguments.as_str()) {
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

/// A JSON value as one line of plain text: a string bare, an array's members
/// joined, an object's as `key=value`. Nested JSON on a phone card is
/// punctuation an operator has to read past to find the command.
///
/// Recursion is bounded by the parse that produced the value: serde_json
/// refuses a document nested deeper than its own limit, so there is no depth
/// here that was not already accepted as a payload.
fn one_line(value: &serde_json::Value) -> String {
    match value {
        // Flattened, because a newline inside a command would otherwise break
        // the single rendered line every channel expects.
        serde_json::Value::String(text) => text.split_whitespace().collect::<Vec<_>>().join(" "),
        serde_json::Value::Array(members) => {
            members.iter().map(one_line).collect::<Vec<_>>().join(" ")
        }
        serde_json::Value::Object(fields) => fields
            .iter()
            .map(|(key, value)| format!("{key}={}", one_line(value)))
            .collect::<Vec<_>>()
            .join(" "),
        scalar => scalar.to_string(),
    }
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
        .filter(|(_, summary)| summary.chars().any(|character| !character.is_whitespace()))
        .next_back()
}

/// The prompt the condenser answers. One line out, so the caller can parse it
/// without a model-shaped grammar.
pub fn condenser_prompt(reply: &str) -> String {
    format!(
        "Summarize this AI coding agent's last turn for a brief phone notification, then classify it.
Output EXACTLY one line and nothing else: STATE|SUMMARY
STATE is one of: done (finished its work), asking (wants you to answer or choose), blocked (needs permission/input to continue).
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
        HookPayload, condenser_prompt, condenser_verdict, moshi_subcommand, parse_payload,
        transcript_reply,
    };

    #[test]
    fn a_payload_yields_every_field_the_hooks_read() {
        let payload = parse_payload(
            r#"{"session_id":"s1","cwd":"/a/b","transcript_path":"/t.jsonl","last_assistant_message":"the reply"}"#,
        );
        assert_eq!(payload.session_id, "s1");
        assert_eq!(payload.cwd, "/a/b");
        assert_eq!(payload.transcript_path, "/t.jsonl");
        assert_eq!(payload.last_assistant_message, "the reply");
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
    fn a_payload_naming_no_tool_and_no_message_still_says_nothing_rather_than_guessing() {
        assert_eq!(parse_payload(r#"{"session_id":"s1"}"#).message, "");
        assert_eq!(parse_payload(r#"{"tool_input":{}}"#).message, "");
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
