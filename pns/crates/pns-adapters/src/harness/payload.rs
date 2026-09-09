use super::flattened;
use super::message::{elicitation_request, reported_error, tool_request};

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
    /// A `PostModelSwitch` event's prior model, empty when the event is not a
    /// model switch.
    pub from_model: String,
    /// A `PostModelSwitch` event's new model, empty when the event is not a
    /// model switch.
    pub to_model: String,
    /// A `PostModelSwitch` event's cause (`auto`, `command`, `picker`, `sdk`
    /// or `resume`), OR a `ConfigChange` event's own kind (`user_settings`,
    /// `project_settings`, `local_settings`, `policy_settings`, `skills`).
    /// ONE FIELD FOR BOTH, in `message`'s own style: the two hooks never fire
    /// together, both name their field `source` in the payload Claude Code
    /// sends, and only one caller ever reads it for a given invocation. Only
    /// `auto` and the five documented config sources are routed anywhere;
    /// every other value, including `resume` and one this binary's own
    /// declaration never asked for, is silence rather than a guess.
    pub source: String,
    /// A `Notification` event's own kind, for example
    /// `quota_auto_resume_fired`. The hooks reference documents the whole
    /// matcher vocabulary; only the three `quota_auto_resume_*` values are
    /// routed anywhere today, and every other value is silence.
    pub notification_type: String,
    /// A `ConfigChange` event's own file, when the harness names one. RAW and
    /// UNFLATTENED, unlike `message`: it identifies a path rather than
    /// composing a rendered line, so the config-change arm sanitises it
    /// itself before it ever reaches a card or a durable record.
    pub file_path: String,
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
        from_model: text("from_model"),
        to_model: text("to_model"),
        source: text("source"),
        notification_type: text("notification_type"),
        file_path: text("file_path"),
    }
}
