//! The composition rules' own tests: what a heading and a body fall back to,
//! and where a cut lands when the text is longer than the surface.

use super::{DEFAULT_REPLY_MAX_CHARS, PREVIEW_MAX_CHARS, flatten_reply, message, preview, title};

// --- title -------------------------------------------------------------

#[test]
fn title_carries_agent_state_and_project() {
    assert_eq!(
        title("claude", "done", "dotfiles"),
        "claude · done · dotfiles"
    );
}

#[test]
fn title_omits_the_project_separator_when_there_is_no_project() {
    assert_eq!(title("claude", "done", ""), "claude · done");
}

#[test]
fn title_falls_back_to_relay_and_done_when_the_caller_gave_neither() {
    assert_eq!(title("", "", ""), "pns · done");
}

#[test]
fn title_still_carries_a_project_under_both_fallbacks() {
    assert_eq!(title("", "", "dotfiles"), "pns · done · dotfiles");
}

// --- message -----------------------------------------------------------

#[test]
fn message_prefixes_the_branch_when_there_is_one() {
    assert_eq!(
        message("main", "ran the suite", "done"),
        "main: ran the suite"
    );
}

#[test]
fn mid_text_punctuation_survives_composition_untouched() {
    // What this module can actually promise. Whether a leading character
    // survives is the BANNER's encoding to guarantee, not this one's: the
    // old assertion here only ever exercised a branch-prefixed message,
    // which cannot start with one of those characters, while a branchless
    // detail can and is covered where the encoding lives.
    assert!(
        message("main", "text with (parens) in the middle", "done")
            .contains("(parens) in the middle")
    );
}

#[test]
fn message_is_the_detail_alone_when_there_is_no_branch() {
    assert_eq!(message("", "ran the suite", "done"), "ran the suite");
}

#[test]
fn message_falls_back_to_the_state_when_there_is_no_detail() {
    assert_eq!(message("", "", "blocked"), "blocked");
}

#[test]
fn message_falls_back_to_done_when_it_was_given_nothing_at_all() {
    assert_eq!(message("", "", ""), "done");
}

// --- flatten_reply -----------------------------------------------------

#[test]
fn a_multi_line_reply_is_flattened_to_one_space_separated_line() {
    assert_eq!(
        flatten_reply("  first\nsecond\twith   runs \n", DEFAULT_REPLY_MAX_CHARS),
        "first second with runs"
    );
}

#[test]
fn a_carriage_return_is_flatten_whitespace_too() {
    assert_eq!(
        flatten_reply("first\r\nsecond", DEFAULT_REPLY_MAX_CHARS),
        "first second"
    );
}

#[test]
fn a_reply_that_mentions_a_glob_keeps_it_verbatim() {
    // The shell version splits on whitespace, and splitting is also where a
    // shell would glob. A turn that says it deleted *.jsonl must not arrive
    // at the phone as the contents of some directory.
    assert_eq!(
        flatten_reply("removed *.jsonl", DEFAULT_REPLY_MAX_CHARS),
        "removed *.jsonl"
    );
}

#[test]
fn whitespace_outside_the_four_is_content_the_turn_wrote_rather_than_a_separator() {
    // The set is exactly four characters. A unicode-aware split, the
    // obvious simplification, also eats a form feed and a non-breaking
    // space, silently rewriting text an agent chose to send.
    assert_eq!(
        flatten_reply("first\u{000c}second", DEFAULT_REPLY_MAX_CHARS),
        "first\u{000c}second"
    );
    assert_eq!(
        flatten_reply("first\u{00a0}second", DEFAULT_REPLY_MAX_CHARS),
        "first\u{00a0}second"
    );
}

#[test]
fn a_reply_within_the_cap_is_left_whole() {
    assert_eq!(
        flatten_reply("short enough", DEFAULT_REPLY_MAX_CHARS),
        "short enough"
    );
}

#[test]
fn an_over_long_reply_is_cut_to_its_tail() {
    assert_eq!(flatten_reply("abcdefghij", 4), "ghij");
}

#[test]
fn a_reply_exactly_at_the_cap_is_left_whole() {
    assert_eq!(flatten_reply("abcd", 4), "abcd");
}

#[test]
fn one_character_past_the_cap_is_already_a_cut() {
    assert_eq!(flatten_reply("abcde", 4), "bcde");
}

#[test]
fn the_tail_cut_counts_characters_rather_than_bytes() {
    assert_eq!(flatten_reply("ééééé", 2), "éé");
}

#[test]
fn a_reply_that_is_only_whitespace_flattens_to_nothing() {
    assert_eq!(flatten_reply(" \t\r\n ", DEFAULT_REPLY_MAX_CHARS), "");
}

// --- preview -----------------------------------------------------------

fn repeat(character: char, count: usize) -> String {
    character.to_string().repeat(count)
}

#[test]
fn a_body_at_the_cap_passes_through_untouched() {
    let body = repeat('a', PREVIEW_MAX_CHARS);
    assert_eq!(preview(&body), body);
}

#[test]
fn one_character_over_the_cap_with_no_sentence_end_is_hard_cut_and_marked() {
    let body = repeat('a', PREVIEW_MAX_CHARS + 1);
    let cut = preview(&body);
    assert_eq!(cut, format!("{}…", repeat('a', PREVIEW_MAX_CHARS - 1)));
    assert_eq!(cut.chars().count(), PREVIEW_MAX_CHARS);
}

#[test]
fn a_sentence_ending_exactly_at_the_cap_is_where_the_cut_lands() {
    let body = format!(
        "{}. {}",
        repeat('b', PREVIEW_MAX_CHARS - 1),
        repeat('c', 100)
    );
    let cut = preview(&body);
    assert_eq!(cut, format!("{}.", repeat('b', PREVIEW_MAX_CHARS - 1)));
    assert_eq!(cut.chars().count(), PREVIEW_MAX_CHARS);
}

#[test]
fn a_sentence_ending_one_past_the_cap_is_not_used() {
    let body = format!("{}. {}", repeat('b', PREVIEW_MAX_CHARS), repeat('c', 100));
    assert_eq!(
        preview(&body),
        format!("{}…", repeat('b', PREVIEW_MAX_CHARS - 1))
    );
}

#[test]
fn the_last_sentence_end_that_fits_wins_over_an_earlier_one() {
    let body = format!("one. two. {}", repeat('c', 400));
    assert_eq!(preview(&body), "one. two.");
}

#[test]
fn an_exclamation_or_a_question_ends_a_sentence_too() {
    assert_eq!(preview(&format!("done! {}", repeat('c', 400))), "done!");
    assert_eq!(preview(&format!("really? {}", repeat('c', 400))), "really?");
}

#[test]
fn a_colon_does_not_end_a_sentence_however_much_a_space_follows_it() {
    // Widening the set is the tempting edit, and it costs the whole
    // preview: a body that opens "Result: " and never reaches a full stop
    // would be cut to its first two words.
    let body = format!("Result: {}", repeat('c', 400));
    assert_eq!(
        preview(&body),
        format!("Result: {}…", repeat('c', PREVIEW_MAX_CHARS - 9))
    );
}

#[test]
fn punctuation_with_no_space_after_it_is_not_a_sentence_end() {
    // A version number or a file name must not be mistaken for a full stop.
    let body = format!("v1.2{}", repeat('c', 400));
    assert_eq!(
        preview(&body),
        format!("v1.2{}…", repeat('c', PREVIEW_MAX_CHARS - 5))
    );
}

#[test]
fn the_hard_cut_right_strips_before_appending_its_mark() {
    let body = format!("{}{}{}", repeat('d', 255), repeat(' ', 5), repeat('e', 50));
    assert_eq!(preview(&body), format!("{}…", repeat('d', 255)));
}

#[test]
fn the_hard_cut_strips_only_the_right_because_the_left_is_text_nobody_cut() {
    // Trailing whitespace is an artefact of where the cut landed; leading
    // whitespace is how the body opened. A plain trim takes both.
    let body = format!("   {}", repeat('d', 300));
    assert_eq!(
        preview(&body),
        format!("   {}…", repeat('d', PREVIEW_MAX_CHARS - 4))
    );
}

#[test]
fn the_preview_cap_counts_characters_rather_than_bytes() {
    let body = repeat('é', 300);
    assert_eq!(
        preview(&body),
        format!("{}…", repeat('é', PREVIEW_MAX_CHARS - 1))
    );
}

#[test]
fn a_multibyte_body_that_fits_passes_through_rather_than_being_measured_in_bytes() {
    // 200 characters and 400 bytes: a cap measured in bytes calls this
    // over the limit and sends a body that fits down the cutting path,
    // where the cut then indexes past the end of a shorter text.
    let body = repeat('é', 200);
    assert_eq!(preview(&body), body);
}
