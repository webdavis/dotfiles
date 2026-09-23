/// The states that mean an agent is WAITING ON THE OPERATOR rather than
/// reporting to them.
///
/// ONE LIST FOR EVERY READER that asks whether an event needs the operator:
/// the recap's own NEEDS YOU section and open list, and the delivery classes'
/// severity rule. Only `asked` is the mid-turn arm's own word now; `blocked`
/// arrives through the separate approval path (`blocking_event`), `denied`
/// through the classifier's own refusal, and `failed` through `failed_turn`
/// for a turn that died, which needs the operator every bit as much as one
/// that asked.
pub const NEEDS_YOU: [&str; 4] = ["asked", "blocked", "denied", "failed"];

/// One session still waiting on the operator at the return moment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenWait {
    pub agent: String,
    /// The newest wait's own state word.
    pub state: String,
    pub project: String,
    /// What the newest wait asks.
    pub asks: String,
    /// How many of the session's waits while the operator was away nothing
    /// has answered, at least one.
    pub count: usize,
}

/// The phone layer of the return recap: the waits still open, one item per
/// session, then the true counts, then where the rest is.
///
/// OPEN WAITS FIRST AND NEVER SUMMARIZED AWAY, which is why they are composed
/// here and not by any model: the urgent line is the one a hallucination would
/// cost the most, and it is the one thing on this card that cannot wait for
/// the recap to be read. A wait the operator already answered is not on it,
/// and waits the card has no room for are counted on its last item
/// (`+3 more waiting`) rather than dropped in silence.
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
/// `recap in #<route>` IS ONLY SAID WHEN THERE IS ONE. `recap_route` is the
/// route a started child posts to, and it is `None` when no child was really
/// started, so the card never points at a recap that was never going to
/// arrive.
pub fn recap_card(
    open: &[OpenWait],
    counted: usize,
    missed: usize,
    recap_route: Option<&str>,
) -> String {
    let mut counts = event_count(counted);
    if missed > 0 {
        counts.push_str(&format!(", {missed} missed"));
    }
    if let Some(route) = recap_route {
        counts.push_str(&format!(". recap in #{route}"));
    }
    // THE ROOM THE COUNTS LEFT, separator included, which is what every urgent
    // item is fitted into. A count so long that nothing is left is an empty
    // room, and the card is then the counts alone.
    let room = crate::render::PREVIEW_MAX_CHARS.saturating_sub(counts.chars().count() + SEPARATOR);
    with_counts(&fitted(open, room), &counts)
}

/// The open waits that fit the room, newest first, closed by a count of the
/// ones that did not.
///
/// STOPPED RATHER THAN SKIPPED, and never before the first: `summary`'s own
/// two rules, for its own two reasons. The newest wait is what the card is
/// for, so when even it will not fit beside the count of the rest, it is cut
/// to the room that count leaves.
fn fitted(open: &[OpenWait], room: usize) -> Vec<String> {
    let items: Vec<String> = open.iter().rev().map(waiting).collect();
    let mut shown = 0;
    while shown < items.len() && joined(&with_rest(&items, shown + 1)).chars().count() <= room {
        shown += 1;
    }
    if shown > 0 || items.is_empty() {
        return with_rest(&items, shown);
    }
    let rest = rest_line(items.len() - 1);
    let spent = rest
        .as_ref()
        .map_or(0, |line| line.chars().count() + "; ".len());
    let mut kept = vec![crate::render::clipped(
        &items[0],
        room.saturating_sub(spent),
    )];
    kept.extend(rest);
    kept
}

/// The first `shown` items, then the count of the ones after them.
fn with_rest(items: &[String], shown: usize) -> Vec<String> {
    let mut kept = items[..shown].to_vec();
    kept.extend(rest_line(items.len() - shown));
    kept
}

/// How many waits the card had no room for, or nothing when it had room for
/// all of them.
fn rest_line(hidden: usize) -> Option<String> {
    (hidden > 0).then(|| format!("+{hidden} more waiting"))
}

/// One open wait as the card says it: whose, how many are unanswered when
/// more than one is, and what the newest one asks.
fn waiting(wait: &OpenWait) -> String {
    let mut said = crate::render::title(&wait.agent, &wait.state, &wait.project);
    if wait.count > 1 {
        said.push_str(&format!(" ×{}", wait.count));
    }
    if !wait.asks.is_empty() {
        said.push_str(&format!(": {}", wait.asks));
    }
    said
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
