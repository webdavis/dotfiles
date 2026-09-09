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
