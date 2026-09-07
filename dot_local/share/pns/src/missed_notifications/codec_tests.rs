//! The JSON codec, and the card and doctor-line tests whose fixtures round-trip
//! a journal through it. The rest of those two moved to `pns-domain` with the
//! code they drive.

use super::{Entry, entries, entry, summary, waiting_line};
use crate::args::EventArgs;

// --- the entry ---------------------------------------------------------

/// One entry at the JOURNAL'S own cap, which is what every test but the
/// cap test itself is about. The cap travels with the caller now, so the
/// journal's number is stated here rather than assumed inside `entry`.
fn journaled(event: &EventArgs, at: Option<u64>) -> String {
    entry(event, at, crate::render::PREVIEW_MAX_CHARS)
}

/// The five values `render::title` and `render::message` consume, as an
/// event. Everything else on `EventArgs` defaults, because nothing else
/// reaches an entry.
fn event(detail: &str) -> EventArgs {
    EventArgs {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        project: "dotfiles".to_string(),
        branch: "main".to_string(),
        detail: detail.to_string(),
        ..EventArgs::default()
    }
}

#[test]
fn an_entry_carries_the_epoch_and_the_five_values_a_card_is_rebuilt_from() {
    // RAW FIELDS AND NOT A PRE-RENDERED STRING: the replay may need to
    // shape them differently from the live card, and a frozen string
    // cannot be reshaped. AND NO OTHER FIELD: the pane, the channel and
    // the tier are all deliberately absent, so the key set is asserted
    // whole rather than one key at a time.
    let written = journaled(&event("a summary"), Some(1_756_500_000));
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
    let object = parsed.as_object().expect("an object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["agent", "at", "branch", "detail", "project", "state"]
    );
    assert_eq!(parsed["at"], 1_756_500_000_u64);
    assert_eq!(parsed["agent"], "claude");
    assert_eq!(parsed["state"], "blocked");
    assert_eq!(parsed["project"], "dotfiles");
    assert_eq!(parsed["branch"], "main");
    assert_eq!(parsed["detail"], "a summary");
}

#[test]
fn an_entry_written_with_no_readable_clock_records_a_null_rather_than_a_zero() {
    // A ZERO IS A CLAIM about January 1970; null is the honest reading,
    // and a reader can tell it from an absent field.
    let written = journaled(&event("a summary"), None);
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
    assert!(parsed["at"].is_null(), "got {written}");
    assert!(
        parsed.as_object().expect("an object").contains_key("at"),
        "the field is still there to be read: {written}"
    );
}

#[test]
fn a_hostile_detail_still_produces_exactly_one_entry_on_one_line() {
    // A NEWLINE IN A DETAIL WOULD FORGE A SECOND ENTRY, and a quote or a
    // control byte would leave a line no reader can parse back. The
    // library escaping is what prevents both; interpolating the value
    // into a JSON-shaped string is what would not.
    let hostile = "he said \"stop\"\nthen a literal \\n and an escape \u{1b}[0m";
    let written = journaled(&event(hostile), Some(1_756_500_000));
    assert_eq!(written.lines().count(), 1, "got {written:?}");
    assert!(!written.contains('\n'), "got {written:?}");
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
    assert_eq!(
        parsed["detail"],
        serde_json::Value::from(hostile.replace('\n', " ")),
        "the detail survives the escaping unchanged but for the flatten"
    );
}

#[test]
fn every_text_field_is_flattened_and_cut_to_the_cap_a_card_renders() {
    // THE TAIL SURVIVES, following `flatten_reply`'s own reasoning: a turn
    // states its conclusion at the end. EVERY field, not the detail alone,
    // because a branch or a project is free text too.
    let cap = crate::render::PREVIEW_MAX_CHARS;
    let long = format!("{}\n\n  END", "x ".repeat(cap));
    let written = entry(
        &EventArgs {
            project: long.clone(),
            ..event(&long)
        },
        Some(1_756_500_000),
        cap,
    );
    let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
    for field in ["detail", "project"] {
        let held = parsed[field].as_str().expect("a string");
        assert_eq!(held.chars().count(), cap, "{field}: {held:?}");
        assert!(held.ends_with("END"), "{field} kept its tail: {held:?}");
        assert!(!held.contains('\n'), "{field} was flattened: {held:?}");
    }
}

// --- reading the entries back ------------------------------------------

#[test]
fn an_entry_reads_back_into_the_six_values_the_writer_put_there() {
    // FED FROM `entry`'S OWN OUTPUT rather than a hand-written literal, so
    // the writer and the reader can never drift apart in a fixture. BY
    // KEY and never by position, which is what makes the writer's key
    // order (`serde_json`'s business, not this module's) invisible here.
    let written = journaled(&event("a private summary"), Some(1_756_500_000));
    let read = entries(&format!("{written}\n"));
    assert_eq!(
        read,
        vec![Entry {
            at: Some(1_756_500_000),
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            project: "dotfiles".to_string(),
            branch: "main".to_string(),
            detail: "a private summary".to_string(),
        }]
    );
    // AND THE CLOCK'S OTHER STATE, which is the one field that is not a
    // string: a null reads back as an unknown epoch, not as 1970.
    assert_eq!(entries(&journaled(&event("x"), None))[0].at, None);
}

#[test]
fn a_short_entry_reads_its_absent_fields_as_empty_and_a_junk_line_costs_the_batch_nothing() {
    // THE FILE IS A PLAIN FILE anything can reach, and the append's own
    // heal can republish a single line over it. One line nobody can parse
    // must not throw away the notifications around it, and a short object
    // must degrade to a thinner card rather than to no card.
    let read = entries(&format!(
        "{}\nnot JSON at all\n{{\"agent\":\"codex\"}}\n\"a bare string\"\n[1,2]\n\n{}\n",
        journaled(&event("the first"), Some(1_756_500_000)),
        journaled(&event("the last"), Some(1_756_500_001)),
    ));
    assert_eq!(
        read.len(),
        3,
        "the two whole entries and the short one survived: {read:?}"
    );
    assert_eq!(read[0].detail, "the first");
    assert_eq!(read[2].detail, "the last", "{read:?}");
    assert_eq!(
        read[1],
        Entry {
            agent: "codex".to_string(),
            ..Entry::default()
        },
        "every absent field read as empty, the epoch included"
    );
}

#[test]
fn an_entry_whose_keys_arrive_in_another_order_reads_back_the_same() {
    // HAND-BUILT AND DELIBERATELY NOT FROM `entry`, which is the only way
    // this says anything at all: `serde_json` writes the keys in one order,
    // and a reader taking fields by POSITION would agree with every fixture
    // the writer produced. The file is a plain file another hand can
    // rewrite, so the reader has to be keyed.
    let read = entries(
        "{\"detail\":\"a private summary\",\"branch\":\"main\",\"at\":1756500000,\
         \"project\":\"dotfiles\",\"state\":\"blocked\",\"agent\":\"claude\"}\n",
    );
    assert_eq!(
        read,
        vec![Entry {
            at: Some(1_756_500_000),
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            project: "dotfiles".to_string(),
            branch: "main".to_string(),
            detail: "a private summary".to_string(),
        }]
    );
}

// --- the summary one card carries --------------------------------------

/// The journal as the replay receives it: oldest first, each entry naming
/// its own place, which is what makes an order assertion unambiguous.
fn waiting(count: usize) -> Vec<Entry> {
    entries(&journal(count))
}
#[test]
fn a_summary_of_three_names_three_and_puts_the_newest_first() {
    // NEWEST FIRST because `render::preview` cuts from the START, so what
    // survives a cut has to be what matters most. The count leads, so a
    // card that stopped early still says how many are behind it.
    let body = summary(&waiting(3));
    assert_eq!(
        body,
        "3 missed notifications. claude · blocked · dotfiles: summary 2; \
         claude · blocked · dotfiles: summary 1; \
         claude · blocked · dotfiles: summary 0"
    );
}

#[test]
fn a_summary_of_one_reads_as_a_single_notification_in_the_singular() {
    // ONE SHAPE FOR EVERY COUNT: a single entry gets the same card the
    // batch does, carrying the same values the live card would have, and
    // only the wording follows the count the way `waiting_line`'s does.
    assert_eq!(
        summary(&waiting(1)),
        "1 missed notification. claude · blocked · dotfiles: summary 0"
    );
}

// --- the doctor's one line ---------------------------------------------

/// A journal holding `count` real entries, written the way the append
/// leaves it: one per line, oldest first, trailing newline present.
fn journal(count: usize) -> String {
    (0..count)
        .map(|which| {
            journaled(
                &event(&format!("summary {which}")),
                Some(1_756_500_000 + which as u64),
            )
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

#[test]
fn the_waiting_line_counts_the_journal_and_says_the_entries_wait_to_be_replayed() {
    // IT SAYS WHAT IS WAITING, never "you missed N": the prune drops the
    // oldest, so a count of what was truly missed over a long absence is a
    // number this file cannot back.
    //
    // AND IT NAMES WHAT DELIVERS THEM, because this is a promise the
    // binary now keeps: the old sentence ended "nothing replays them yet",
    // which the replay made false the moment it shipped.
    assert_eq!(
        waiting_line(Some(&journal(3)), true),
        "pns doctor: 3 missed notifications are waiting to be replayed; \
         the next event that raises a banner or a card while the operator \
         is not away delivers them."
    );
    assert_eq!(
        waiting_line(Some(&journal(1)), true),
        "pns doctor: 1 missed notification is waiting to be replayed; \
         the next event that raises a banner or a card while the operator \
         is not away delivers it."
    );
}

#[test]
fn a_switched_off_card_says_the_misses_are_recorded_and_that_nothing_delivers_them() {
    // THE PROMISE BELONGS TO THE SWITCH. `[recap] replay_card = false`
    // means no event will ever deliver these, so a line that still named
    // "the next event" would be a lie the operator's own setting makes
    // permanent, and the doctor would be the thing telling it. It says
    // what is true instead: the misses are recorded, the card is off, and
    // nothing moves them until the card is back on.
    assert_eq!(
        waiting_line(Some(&journal(3)), false),
        "pns doctor: 3 missed notifications are recorded; the catch-up card \
         is switched off (`[recap] replay_card = false`), so nothing delivers \
         them until the card is switched back on."
    );
    assert_eq!(
        waiting_line(Some(&journal(1)), false),
        "pns doctor: 1 missed notification is recorded; the catch-up card \
         is switched off (`[recap] replay_card = false`), so nothing delivers \
         it until the card is switched back on."
    );
}

#[test]
fn the_waiting_line_cannot_emit_an_entrys_content() {
    // THE PRIVACY RULE, pinned. Every value in this fixture is
    // unmistakable, so a line that leaked any part of an entry, in any
    // arm, cannot pass by coincidence.
    let secret = EventArgs {
        agent: "zzagentzz".to_string(),
        state: "zzstatezz".to_string(),
        project: "zzprojectzz".to_string(),
        branch: "zzbranchzz".to_string(),
        detail: "zzthe-operators-own-private-summaryzz".to_string(),
        ..EventArgs::default()
    };
    for (count, replay_card) in [(1, true), (3, true), (1, false), (3, false)] {
        let contents: String = (0..count)
            .map(|_| format!("{}\n", journaled(&secret, Some(1_756_500_000))))
            .collect();
        let line = waiting_line(Some(&contents), replay_card);
        for leaked in [
            &secret.agent,
            &secret.state,
            &secret.project,
            &secret.branch,
            &secret.detail,
        ] {
            assert!(
                !line.contains(leaked.as_str()),
                "{leaked:?} reached the doctor's line: {line}"
            );
        }
        // AND NOT THE EPOCH EITHER, which is the one field that would look
        // harmless enough to print.
        assert!(!line.contains("1756500000"), "{line}");
    }
}
