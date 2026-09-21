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

/// What a transcript says about the session itself, read from the same tail
/// the reply is read from.
///
/// VERIFIED AGAINST REAL TRANSCRIPTS (Claude Code 2.1.x, this machine's own
/// session directory, 2026-09-20): the name the operator gave the session
/// arrives on its own line as `{"type":"custom-title","customTitle":...}`,
/// the harness's generated one as `{"type":"ai-title","aiTitle":...}`, and
/// every assistant line carries `message.model`. A Codex rollout file carries
/// neither title field, which is why a Codex session shows no title at all.
///
/// NEWEST WINS for each of the three, because a session can be renamed and a
/// model can be switched mid-session, and the tail holds both writes in order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SessionFacts {
    pub custom_title: String,
    pub ai_title: String,
    pub model: String,
}

pub fn session_facts(transcript_tail: &str) -> SessionFacts {
    let mut facts = SessionFacts::default();
    for entry in transcript_tail
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    {
        let text = |key: &str| {
            entry
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        match entry.get("type").and_then(serde_json::Value::as_str) {
            Some("custom-title") => facts.custom_title = text("customTitle"),
            Some("ai-title") => facts.ai_title = text("aiTitle"),
            Some("assistant") => {
                if let Some(model) = entry.pointer("/message/model").and_then(|m| m.as_str()) {
                    facts.model = model.to_string();
                }
            }
            _ => {}
        }
    }
    facts
}
