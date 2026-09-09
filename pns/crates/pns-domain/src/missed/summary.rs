use super::Entry;

/// The one card a replay delivers, whatever the count: the true count, then
/// as many entries as fit, NEWEST FIRST.
///
/// ONE SHAPE AND NO SPECIAL CASE. A summary for many plus the real card for
/// exactly one would be two code paths, two sets of tests and a seam where
/// the two can disagree about what a replayed card looks like; and a
/// one-entry summary carries the same content the real card would, because
/// an entry holds exactly the values `render::title` and `render::message`
/// consume.
///
/// `waiting` ARRIVES IN THE FILE'S OWN ORDER, oldest first, and is rendered
/// newest first here, because `render::preview` cuts from the START: what
/// survives a cut has to be what matters most.
///
/// THE COUNT IS ALWAYS THE REAL COUNT, even when the body stopped early, so
/// the card never claims a number it did not show and never shows a number it
/// cannot back. The body stops at `render::PREVIEW_MAX_CHARS` rather than
/// leaving the cut to `preview`, so the operator is told how many are behind
/// the ones they can read; the full text of every entry already reached the
/// durable log when it happened.
///
/// THE NEWEST ENTRY GOES IN WHATEVER ITS LENGTH, and only the ones behind it
/// have to fit. MEASURED: a single missed notification with a 209-character
/// detail took the body one character past the cap, so the loop stopped
/// before appending anything and the card read "1 missed notification" with
/// no content at all, which is precisely the notification it exists to
/// deliver. The cut for that one entry is `render::preview`'s, on the way
/// out, which is where every other over-long body is already cut.
pub fn summary(waiting: &[Entry]) -> String {
    let mut body = match waiting.len() {
        1 => "1 missed notification".to_string(),
        many => format!("{many} missed notifications"),
    };
    for (shown, entry) in waiting.iter().rev().enumerate() {
        let separator = if shown == 0 { ". " } else { "; " };
        let extended = format!("{body}{separator}{}", rendered(entry));
        // STOPPED RATHER THAN SKIPPED, which is also what lets the index above
        // stand in for how many were shown: the entries left out are the
        // oldest, and a body that skipped a long one to reach an older short
        // one would read as though the newest were missing.
        //
        // AND NEVER BEFORE THE FIRST ONE. `shown == 0` is the newest entry
        // with nothing appended yet, and stopping there leaves the count
        // standing alone as the whole card.
        if shown > 0 && extended.chars().count() > crate::render::PREVIEW_MAX_CHARS {
            break;
        }
        body = extended;
    }
    body
}

/// One entry as a line of the summary: the card's own title, and its text
/// where there is any.
///
/// THE TITLE ALONE FOR AN EMPTY DETAIL, because the title already carries the
/// state a bare `done` turn would otherwise repeat after a colon.
fn rendered(entry: &Entry) -> String {
    let title = crate::render::title(&entry.agent, &entry.state, &entry.project);
    if entry.detail.is_empty() {
        title
    } else {
        format!("{title}: {}", entry.detail)
    }
}
