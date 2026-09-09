//! What a channel is handed to display: the heading, the body, and the
//! shortened body a surface with a small preview shows.

/// The reply cap a caller uses when it names none.
pub const DEFAULT_REPLY_MAX_CHARS: usize = 8000;

/// The longest preview a phone card or banner renders without a cut.
pub const PREVIEW_MAX_CHARS: usize = 260;

/// The whitespace a flatten collapses. Exactly these four, so a turn keeps
/// every other character it wrote.
const FLATTEN_WHITESPACE: [char; 4] = [' ', '\t', '\r', '\n'];

/// The one-line heading a channel with a title field uses.
pub fn title(agent: &str, state: &str, project: &str) -> String {
    let agent = if agent.is_empty() { "pns" } else { agent };
    let state = if state.is_empty() { "done" } else { state };
    if project.is_empty() {
        format!("{agent} · {state}")
    } else {
        format!("{agent} · {state} · {project}")
    }
}

/// The body: the summary itself, branch-prefixed. Deliberately NOT a repeat of
/// the state and project the title already carries, so a channel with a short
/// preview spends it on content rather than boilerplate.
///
/// The prefix is `branch: body`, never `(branch) body`: macOS argument parsing
/// eats a terminal-notifier `-message` whose FIRST CHARACTER is "(", "[", "-"
/// (read as an option) or presumably "{", and the banner then renders
/// title-only. Only position one matters: mid-text punctuation and a leading
/// digit both render fine, and neither a leading space nor a zero-width space
/// escapes the rule (live probes P3-P7, 2026-08-12). One format across every
/// channel, so Discord reads the same way rather than the banner getting a
/// special case.
///
/// A detail that begins with one of those characters is no longer a limit: the
/// banner spawn armors the first character of every value it passes (see
/// `channels::banner::notifier_args`), so composition here is free to produce
/// anything.
pub fn message(branch: &str, detail: &str, state: &str) -> String {
    let body = match (detail.is_empty(), state.is_empty()) {
        (false, _) => detail,
        (true, false) => state,
        (true, true) => "done",
    };
    if branch.is_empty() {
        body.to_string()
    } else {
        format!("{branch}: {body}")
    }
}

/// An agent turn reduced to the one line a summary prompt and a notification
/// can carry: every run of the flatten whitespace becomes ONE space, both ends
/// are trimmed, and at most `max_chars` survive.
///
/// THE TAIL IS WHAT SURVIVES, not the head. A turn states its conclusion at the
/// end, and the beginning is setup whoever gets the notification already
/// watched.
pub fn flatten_reply(text: &str, max_chars: usize) -> String {
    let flattened: String = text
        .split(FLATTEN_WHITESPACE)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let length = flattened.chars().count();
    if length <= max_chars {
        return flattened;
    }
    flattened.chars().skip(length - max_chars).collect()
}

/// The body cut to the last full sentence that fits a small preview. The phone
/// and banner clip mid-sentence otherwise; the channels that carry the full
/// text keep it.
pub fn preview(message: &str) -> String {
    let characters: Vec<char> = message.chars().collect();
    if characters.len() <= PREVIEW_MAX_CHARS {
        return message.to_string();
    }

    // The LAST sentence end that still fits, so the cut keeps as much as it can.
    let mut cut = 0;
    for (index, character) in characters.iter().enumerate() {
        if !matches!(character, '.' | '!' | '?') {
            continue;
        }
        let end = index + 1;
        if end > PREVIEW_MAX_CHARS {
            break;
        }
        // Punctuation ends a sentence only when a space follows, so a version
        // number is not mistaken for a full stop. A sentence end at the very
        // end of the text would count too, but it cannot fit: the text is
        // longer than the cap, so its end is past the cap.
        if characters.get(end) == Some(&' ') {
            cut = end;
        }
    }
    if cut > 0 {
        return characters[..cut].iter().collect();
    }

    // No sentence end to cut at, so the text is cut short and SAID to be.
    clipped(message, PREVIEW_MAX_CHARS)
}

/// `text` inside `max_chars`, and SAID to have been cut when it was.
///
/// THE HEAD IS WHAT SURVIVES, which is the opposite of `flatten_reply` and for
/// the opposite reason: that one keeps the tail of a turn's own reply, because
/// a turn states its conclusion at the end, and this one cuts a line somebody
/// COMPOSED, whose beginning names what it is about.
///
/// THE ANSWER IS NEVER LONGER THAN THE ROOM IT WAS GIVEN, mark included, which
/// is what lets a caller reserve space and rely on the reservation. A room of
/// zero is an empty answer rather than a bare mark.
pub fn clipped(text: &str, max_chars: usize) -> String {
    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= max_chars {
        return text.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let head: String = characters[..max_chars - 1].iter().collect();
    format!("{}…", head.trim_end())
}

#[cfg(test)]
mod tests;
