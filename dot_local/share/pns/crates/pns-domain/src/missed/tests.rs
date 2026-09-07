//! The card and the doctor line, pinned: what a summary renders, what
//! survives the cap the phone actually renders, what counts as needing the
//! operator, and what the doctor's line says with nothing waiting.
//!
//! Four more tests of these same functions stay in the legacy package, along
//! with the codec's own and the `was_missed` ones: they build their fixtures
//! by round-tripping a journal through `entries`, which this crate cannot
//! reach.

use super::{Entry, needing_you, recap_card, summary, waiting_line};

// --- the summary one card carries --------------------------------------

#[test]
fn a_bare_entry_renders_its_title_alone_without_a_dangling_colon() {
    // The common bare `done` turn carries no detail, and a card that read
    // "claude \u{b7} done \u{b7} dotfiles:" would look truncated rather
    // than complete. Pins the title-only arm of `rendered`.
    let bare = Entry {
        at: Some(1_000),
        agent: "claude".to_string(),
        state: "done".to_string(),
        project: "dotfiles".to_string(),
        branch: String::new(),
        detail: String::new(),
    };
    let line = summary(&[bare]);
    assert!(
        !line.trim_end().ends_with(':') && !line.contains(": ;") && !line.contains(":;"),
        "a bare entry must not dangle a colon: {line}"
    );
}

/// One entry of a stated length, carrying a phrase no other entry in its
/// fixture holds, so an assertion names WHICH entry reached the body.
fn sized(phrase: &str, padding: usize) -> Entry {
    Entry {
        agent: "claude".to_string(),
        state: "done".to_string(),
        detail: format!("{phrase}{}", "x".repeat(padding)),
        ..Entry::default()
    }
}

#[test]
fn a_summary_too_long_for_the_card_stops_early_and_still_names_the_true_count() {
    // THE COUNT NEVER LIES EITHER WAY: it is the real number even when the
    // body ran out of room, and the body never runs past what a card
    // renders without a cut.
    //
    // THE TWO NEWEST ARE LONG AND THE TWO OLDEST SHORT, which is what makes
    // STOPPING distinguishable from SKIPPING at all. Four entries of ONE
    // length cannot tell the two apart: what does not fit for the newest
    // does not fit for an older one either, so a build that skipped and
    // carried on would render exactly the same body. With a short entry
    // waiting behind a long one, skipping reaches it and prints the oldest
    // news as though the newest were missing.
    let cap = crate::render::PREVIEW_MAX_CHARS;
    let waiting = [
        sized("the oldest short one", 0),
        sized("the second short one", 0),
        sized("the older long one", cap / 2),
        sized("the newest long one", cap / 2),
    ];
    let body = summary(&waiting);
    assert!(
        body.starts_with("4 missed notifications. "),
        "the true count leads: {body}"
    );
    assert!(
        body.chars().count() <= cap,
        "the body is inside what a card renders whole: {} chars",
        body.chars().count()
    );
    assert!(
        body.contains("the newest long one"),
        "the newest entry is what the body spends its room on: {body}"
    );
    // AND NOTHING BEHIND THE ONE THAT DID NOT FIT, which is the assertion a
    // `continue` in place of the `break` fails: it would reach past the
    // older long entry to both short ones.
    for skipped in [
        "the older long one",
        "the second short one",
        "the oldest short one",
    ] {
        assert!(
            !body.contains(skipped),
            "{skipped} rode past the stop: {body}"
        );
    }
}

#[test]
fn a_single_entry_past_the_cap_is_still_delivered_rather_than_becoming_a_bare_count() {
    // MEASURED ON THE SHIPPED BUILD: a 209-character detail put the body
    // one character past the cap, the loop broke before appending
    // anything, and the whole card read "1 missed notification" with no
    // content at all. The one notification the operator missed is exactly
    // what a card is for, so the newest entry goes in whatever its length
    // and `render::preview` takes the cut on the way out, which is the
    // existing, tested path.
    //
    // THE TWO FIXTURES STRADDLE THE CAP BY ONE, and each asserts where it
    // landed rather than trusting a length: 208 characters of detail is the
    // last that fits whole, 209 the first that does not. A cap that moves
    // fails these two lines and names the new number.
    //
    // THE MEASURED SHAPE, card and all: a title of `claude, blocked,
    // dotfiles` is what puts the boundary at 208 characters of detail.
    let carded = |detail: usize| Entry {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        project: "dotfiles".to_string(),
        detail: "x".repeat(detail),
        ..Entry::default()
    };
    let cap = crate::render::PREVIEW_MAX_CHARS;
    let fits = summary(&[carded(208)]);
    assert_eq!(
        fits.chars().count(),
        cap,
        "the fixture has to land ON the cap: {fits}"
    );
    assert!(fits.contains(&"x".repeat(208)), "{fits}");
    let over = summary(&[carded(209)]);
    assert_eq!(
        over.chars().count(),
        cap + 1,
        "the fixture has to land one PAST the cap: {over}"
    );
    assert!(
        over.starts_with("1 missed notification. "),
        "the count still leads: {over}"
    );
    assert!(
        over.contains(&"x".repeat(209)),
        "the only entry there was never reached the card: {over}"
    );
}

// --- the recap's own card ----------------------------------------------

/// One activity entry in a state and a project, which is all the card
/// renders of it.
fn acted(state: &str, project: &str) -> Entry {
    Entry {
        agent: "claude".to_string(),
        state: state.to_string(),
        project: project.to_string(),
        ..Entry::default()
    }
}

#[test]
fn the_recap_card_puts_what_needs_the_operator_in_front_of_every_count() {
    // NEEDS YOU FIRST. The counts are the reason the card is worth reading
    // at all, but the urgent item is the reason it is worth acting on, and
    // a card that opened with a number would bury it.
    let card = recap_card(
        &[acted("done", "p"), acted("blocked", "dotfiles")],
        12,
        2,
        true,
    );
    let urgent = card
        .find("blocked")
        .expect("the urgent item is on the card");
    let count = card
        .find("12 events")
        .expect("the window's count is on the card");
    assert!(urgent < count, "the counts came first: {card}");
    assert!(card.contains("2 missed"), "{card}");
    assert!(card.ends_with("recap in #pns"), "{card}");
}

#[test]
fn a_recap_card_with_nothing_waiting_says_so_by_saying_nothing_about_it() {
    // NO ZERO CLAUSE. "0 missed" is a sentence about nothing, and the card
    // is 260 characters wide; the pointer is dropped for the mirror reason,
    // because a card must never name a recap that was never started.
    let card = recap_card(&[], 12, 0, false);
    assert_eq!(card, "12 events", "{card}");
}

#[test]
fn the_recap_cards_counts_survive_a_needs_you_list_too_long_to_fit() {
    // THE COUNTS ARE RESERVED, not fitted. They are what the card can
    // always back, so they are built first and the urgent items are fitted
    // in front of them; a build that filled the card with titles and then
    // cut would drop the numbers instead.
    let crowd: Vec<Entry> = (0..40)
        .map(|which| acted("blocked", &format!("project-{which}")))
        .collect();
    let card = recap_card(&crowd, 80, 3, true);
    assert!(
        card.contains("80 events, 3 missed. recap in #pns"),
        "the counts were cut to make room: {card}"
    );
    assert!(
        card.chars().count() <= crate::render::PREVIEW_MAX_CHARS,
        "the card ran past what a phone renders: {} chars",
        card.chars().count()
    );
    assert!(
        card.starts_with("claude · blocked · project-39"),
        "the newest urgent item leads: {card}"
    );
}

#[test]
fn one_urgent_item_too_long_for_the_card_is_cut_to_fit_rather_than_dropped() {
    // `summary`'S MEASURED RULE, applied to the second card. The one thing
    // waiting on the operator is exactly what the card is for, so the
    // newest item is never the thing that goes; what gives is its length.
    let huge = acted("blocked", &"x".repeat(crate::render::PREVIEW_MAX_CHARS));
    let card = recap_card(&[huge], 9, 0, false);
    assert!(
        card.contains(&"x".repeat(200)),
        "the item was dropped: {card}"
    );
    assert!(
        card.ends_with("9 events"),
        "and the count still stands: {card}"
    );
}

#[test]
fn every_count_survives_the_preview_the_phone_is_actually_handed() {
    // THE DELIVERED CARD IS `render::preview` OF THE CARD, never the card
    // itself, and that is where this used to fail. MEASURED: a
    // 120-character agent and a 120-character project (the activity ring's
    // own field cap, twice) compose a 253-character title; the card reached
    // 289 characters, and the preview cuts at the last SENTENCE END that
    // fits, which is the full stop in front of the counts. The phone was
    // handed 254 characters of title with the event count, the missed
    // count and the pointer all gone: every number the card exists to
    // carry, and the pointer to where the rest of it is.
    //
    // ASSERTED AGAINST THE PREVIEW, never the raw detail, which is the
    // whole point: the raw detail passed the whole time.
    let wide = Entry {
        agent: "a".repeat(120),
        state: "blocked".to_string(),
        project: "p".repeat(120),
        ..Entry::default()
    };
    let card = recap_card(&[wide], 13, 2, true);
    let delivered = crate::render::preview(&card);

    assert_eq!(
        delivered, card,
        "the card was long enough for the preview to cut it at all"
    );
    assert!(delivered.contains("13 events"), "{delivered}");
    assert!(delivered.contains("2 missed"), "{delivered}");
    assert!(delivered.contains("recap in #pns"), "{delivered}");
    assert!(
        delivered.contains("blocked"),
        "and the urgent item is still what leads: {delivered}"
    );
}

#[test]
fn only_the_states_that_wait_on_the_operator_are_needing_you() {
    // ONE LIST, TWO READERS, so this pins the list itself rather than the
    // card that spends it. `done` is a report and `stale` is a warning;
    // neither is waiting on an answer.
    let window = vec![
        acted("done", "p"),
        acted("blocked", "p"),
        acted("stale", "p"),
        acted("asked", "p"),
        acted("plan-ready", "p"),
        acted("denied", "p"),
        acted("failed", "p"),
    ];
    let waiting: Vec<String> = needing_you(&window)
        .into_iter()
        .map(|entry| entry.state)
        .collect();
    assert_eq!(
        waiting,
        ["blocked", "asked", "plan-ready", "denied", "failed"],
        "in the order they arrived"
    );
}

#[test]
fn an_absent_or_blank_journal_says_nothing_is_recorded_and_names_nothing() {
    // AN EMPTY JOURNAL IS AMBIGUOUS: either nothing was missed or a write
    // did not land. The line claims neither, and it never says a number.
    let none = "pns doctor: no missed notification is recorded.";
    assert_eq!(waiting_line(None, true), none);
    assert_eq!(waiting_line(Some(""), true), none);
    assert_eq!(waiting_line(Some("\n"), true), none);
    assert_eq!(waiting_line(Some("\n   \n\t\n"), true), none);
    // AND THE CARD'S SWITCH DOES NOT REACH THIS ARM. There is nothing
    // waiting, so there is no promise for the switch to make or unmake,
    // and a second sentence for the same empty journal would be one more
    // thing to keep true for no reading gained.
    assert_eq!(waiting_line(None, false), none);
    assert_eq!(waiting_line(Some(""), false), none);
    assert_eq!(waiting_line(Some("\n"), false), none);
    assert_eq!(waiting_line(Some("\n   \n\t\n"), false), none);
}
