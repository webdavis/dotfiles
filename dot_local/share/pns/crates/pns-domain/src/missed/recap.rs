use super::Entry;

/// The states that mean an agent is WAITING ON THE OPERATOR rather than
/// reporting to them.
///
/// ONE LIST, TWO READERS. The phone card's needs-you line and the recap's own
/// NEEDS YOU section are the same question asked at two sizes, and two copies
/// of this list would drift the day a sixth state joins. The first four are the
/// mid-turn arm's own words in the composition root; `failed` is a turn that
/// died, which needs the operator every bit as much as one that asked.
pub const NEEDS_YOU: [&str; 5] = ["asked", "blocked", "denied", "failed", "plan-ready"];

/// The entries in a window that still need the operator, in the order they
/// arrived.
pub fn needing_you(entries: &[Entry]) -> Vec<Entry> {
    entries
        .iter()
        .filter(|entry| NEEDS_YOU.contains(&entry.state.as_str()))
        .cloned()
        .collect()
}

/// The phone layer of the return recap: what still needs the operator, then
/// the true counts, then where the rest is.
///
/// NEEDS YOU FIRST AND NEVER SUMMARIZED AWAY, which is why it is composed here
/// and not by any model: the urgent line is the one a hallucination would cost
/// the most, and it is the one thing on this card that cannot wait for the
/// Discord recap to be read.
///
/// EVERY NUMBER IS A LENGTH, never a claim. `counted` is the window's own
/// length and `missed` is the claimed journal's, so a card that ran out of room
/// still names totals it can back. That is `summary`'s count-never-lies rule
/// applied to a second card.
///
/// THE COUNTS AND THE POINTER ARE RESERVED, and the urgent items are fitted
/// into whatever room is left. MEASURED as the reason this is not a stop rule
/// alone: a 120-character agent and a 120-character project compose a
/// 253-character title, the first urgent item used to go in whatever its
/// length, and the card reached 289 characters. `render::preview` is what the
/// phone is actually handed, and it cuts at the last SENTENCE END that fits,
/// which is the full stop before the counts: the delivered preview was
/// 254 characters of title with the event count, the missed count and the
/// pointer all gone. So the newest urgent item is CUT to the room rather than
/// dropped (a card without the one thing waiting on the operator is the
/// notification it exists to deliver) and the card never exceeds the cap at
/// all, which is what makes the preview a no-op.
///
/// TWO INDEPENDENT READS OF ONE RING, STATED. `counted` is this process's read
/// of the window and the Discord header is the child's, so the two can differ
/// by an event written between them. Each is honest about what it read; see
/// `spawn_recap`'s own comment for why nothing reconciles them.
///
/// "recap in #pns" IS ONLY SAID WHEN THERE IS ONE. `digest_posted` is whether a
/// child was really started, not whether one was wanted, so the card never
/// points at a recap that was never going to arrive.
pub fn recap_card(
    needs_you: &[Entry],
    counted: usize,
    missed: usize,
    digest_posted: bool,
) -> String {
    let mut counts = event_count(counted);
    if missed > 0 {
        counts.push_str(&format!(", {missed} missed"));
    }
    if digest_posted {
        counts.push_str(". recap in #pns");
    }
    // THE ROOM THE COUNTS LEFT, separator included, which is what every urgent
    // item is fitted into. A count so long that nothing is left is an empty
    // room, and the card is then the counts alone.
    let room = crate::render::PREVIEW_MAX_CHARS.saturating_sub(counts.chars().count() + SEPARATOR);
    let mut urgent: Vec<String> = Vec::new();
    for entry in needs_you.iter().rev() {
        let mut extended = urgent.clone();
        extended.push(crate::render::clipped(
            &crate::render::title(&entry.agent, &entry.state, &entry.project),
            room,
        ));
        // STOPPED RATHER THAN SKIPPED, and never before the first: `summary`'s
        // own two rules, for its own two reasons. The first item is already
        // inside the room by the clip above, so "never before the first" costs
        // the cap nothing here.
        if !urgent.is_empty() && joined(&extended).chars().count() > room {
            break;
        }
        urgent = extended;
    }
    with_counts(&urgent, &counts)
}

/// The window's own count, said ONCE so the phone card and the Discord header
/// cannot disagree about it. A one-event window read "1 events" on the phone
/// and "1 event" in Discord while this was two sentences.
pub fn event_count(counted: usize) -> String {
    if counted == 1 {
        "1 event".to_string()
    } else {
        format!("{counted} events")
    }
}

/// What separates the urgent items from the counts, counted rather than
/// guessed at, so the reservation above and the composition below cannot
/// disagree about its width.
const SEPARATOR: usize = ". ".len();

/// The urgent items in front of the counts, or the counts alone.
fn with_counts(urgent: &[String], counts: &str) -> String {
    if urgent.is_empty() {
        counts.to_string()
    } else {
        format!("{}. {counts}", joined(urgent))
    }
}

/// The urgent items as one run of text, which is the thing the room is
/// measured against.
fn joined(urgent: &[String]) -> String {
    urgent.join("; ")
}
