//! What a session's own transcript adds to the summarizer's prompt.
//!
//! THE MODEL'S TURNS AND NOTHING ELSE. User prompts and tool results carry
//! paths, secrets and whatever was pasted into a session, so only the
//! assistant's own text crosses, which is the design's own ruling.
//!
//! IT REACHES THE MODEL AND NOTHING ELSE. The excerpt is appended to the
//! prompt and never enters the document other tools read, so a consumer of a
//! recap cannot be handed a transcript by asking for one.

use pns_domain::recap::activity::Project;

/// Each session's assistant turns, newest session first, under both ceilings.
///
/// TWO CEILINGS, BOTH HONOURED. `per_session` bounds one session's excerpt and
/// `total` the whole appendix, so one very long session cannot spend the
/// window's whole budget and a window of many sessions cannot outgrow it
/// either.
pub fn excerpt(projects: &[Project], per_session: usize, total: usize) -> String {
    let mut appended = String::new();
    for project in projects {
        for session in &project.sessions {
            if session.transcript_path.is_empty() || appended.len() >= total {
                continue;
            }
            let text = assistant_turns(&read_tail(&session.transcript_path, per_session));
            if text.is_empty() {
                continue;
            }
            let room = total.saturating_sub(appended.len());
            appended.push_str(&format!(
                "\n--- {} ({}) ---\n",
                session.title, session.harness
            ));
            appended.push_str(cut(&text, room.min(per_session)));
            appended.push('\n');
        }
    }
    appended
}

/// The assistant text of every turn in one transcript tail, oldest first.
///
/// A LINE THAT WILL NOT PARSE IS SKIPPED rather than fatal, for
/// `transcript_reply`'s reason: the tail is cut mid-line by design, so the
/// first line is routinely half an object.
fn assistant_turns(tail: &str) -> String {
    tail.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|entry| entry.get("type").and_then(serde_json::Value::as_str) == Some("assistant"))
        .filter_map(|entry| Some(entry.pointer("/message/content")?.as_array()?.clone()))
        .flatten()
        .filter(|block| block.get("type").and_then(serde_json::Value::as_str) == Some("text"))
        .filter_map(|block| Some(block.get("text")?.as_str()?.to_string()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The last `bytes` of a transcript, or nothing at all.
///
/// CHECKED BEFORE OPENING, AND ON THE LINK ITSELF, the same refusal the hook
/// path takes: opening a FIFO blocks until a writer appears and /dev/zero
/// never ends, and a transcript is a regular file.
fn read_tail(path: &str, bytes: usize) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return String::new();
    };
    if !metadata.is_file() {
        return String::new();
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let bytes = bytes as u64;
    let _ = file.seek(SeekFrom::Start(metadata.len().saturating_sub(bytes)));
    let mut tail = Vec::new();
    // Capped as well as sought: the file can grow between the two calls, and
    // a seek that failed would otherwise read all of it.
    let _ = file.take(bytes).read_to_end(&mut tail);
    String::from_utf8_lossy(&tail).into_owned()
}

/// `text` cut to at most `bytes`, on a character boundary.
fn cut(text: &str, bytes: usize) -> &str {
    match text.len() <= bytes {
        true => text,
        false => {
            let mut end = bytes;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            &text[..end]
        }
    }
}

#[cfg(test)]
#[path = "transcripts/tests.rs"]
mod tests;
