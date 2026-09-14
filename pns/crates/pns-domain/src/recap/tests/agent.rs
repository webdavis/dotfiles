//! The recap an agent wrote, pinned: what fitting it may and may not do.

use crate::recap::agent::fitted;
use crate::recap::budget::MAX_CHARS;

/// One recap in the locked layout, with `bullets` lines under every prose
/// section and `files` lines in the fenced block, so a test can ask for a body
/// of whatever size the behavior is about.
fn recap(bullets: usize, files: usize) -> String {
    let mut body = String::from("Recap\n==========\n\n**Git**\n");
    body.push_str("- Branch: `feat/pns-recap-agent` (open, 1 of 1)\n");
    body.push_str("- Worktree: `/Users/stephen/.herdr/worktrees/dotfiles/x` (kept)\n");
    body.push_str("- PR: #591 (open)\n");
    body.push_str("- Stack: `feat/pns-recap-agent` (1 PR, trunk main)\n\n```\n");
    body.push_str(" Stack Graph\n -----------\n  `main` (*trunk*)\n");
    body.push_str("  └─ `feat/pns-recap-agent`  #591  *open*  ×  ← *current*\n\n");
    for index in 0..files {
        let status = ["A", "M", "D"][index % 3];
        body.push_str(&format!(
            "{status}  crates/pns-domain/src/recap/file{index}.rs\n"
        ));
    }
    body.push_str("```\n");
    for section in [
        "Summary",
        "In-Progress",
        "Upcoming Agent Tasks",
        "User Tasks",
    ] {
        body.push_str(&format!("\n**{section}**\n"));
        for index in 0..bullets {
            body.push_str(&format!(
                "- line {index} of {section}, long enough to spend a real share of one Discord message\n"
            ));
        }
    }
    body
}

#[test]
fn a_recap_already_under_the_ceiling_is_posted_exactly_as_it_was_written() {
    // THE ORDINARY CASE IS UNTOUCHED. An agent that kept to the layout's own
    // 2,000-character rule wrote a message that fits, and rewriting it would
    // be this command editing prose nobody asked it to edit.
    let written = recap(1, 2);
    assert!(written.chars().count() <= MAX_CHARS, "the fixture is over");
    assert_eq!(fitted(&written), written.trim_end());
}

#[test]
fn a_recap_over_the_ceiling_comes_back_under_it() {
    let written = recap(6, 30);
    assert!(
        written.chars().count() > 2_000,
        "the fixture is not over the cap it is about: {}",
        written.chars().count()
    );
    let body = fitted(&written);
    assert!(
        body.chars().count() <= MAX_CHARS,
        "the fitted recap is still {} characters",
        body.chars().count()
    );
}

#[test]
fn a_recap_over_the_ceiling_is_never_cut_in_the_middle_of_a_line() {
    // TRUNCATION IS THE ONE THING THE LAYOUT'S OWN READABILITY RULE FORBIDS:
    // "collapse a long file list to counts per status rather than truncating
    // mid-list". A line either survives whole or is replaced by the count of
    // what went with it.
    let written = recap(6, 30);
    let body = fitted(&written);
    let kept: Vec<&str> = written.lines().map(str::trim_end).collect();
    for line in body.lines() {
        assert!(
            kept.contains(&line) || line.starts_with("...and ") || line == "A 10  M 10  D 10",
            "a line came back in a shape nobody wrote: {line}"
        );
    }
}

#[test]
fn what_a_cut_section_left_out_is_counted_rather_than_dropped_in_silence() {
    let body = fitted(&recap(6, 30));
    assert!(
        body.contains("...and "),
        "a recap was shortened with nothing saying so: {body}"
    );
}

#[test]
fn a_long_file_list_becomes_counts_per_status_inside_the_same_fence() {
    let body = fitted(&recap(6, 30));
    assert_eq!(
        body.lines().filter(|line| line.trim() == "```").count(),
        2,
        "the fenced block did not survive the fit: {body}"
    );
    assert!(
        body.contains("A 10  M 10  D 10"),
        "the file list was not collapsed to counts per status: {body}"
    );
    assert!(
        !body.contains("file29.rs"),
        "a collapsed list still carries its own rows: {body}"
    );
}

#[test]
fn the_stack_graph_keeps_the_indentation_that_makes_it_a_tree() {
    // THE FENCE EXISTS FOR THIS. `safe_line`, which the night recap runs over
    // somebody else's sentence, flattens every run of whitespace to one space;
    // run over a tree it would leave every branch at the same depth.
    let body = fitted(&recap(6, 30));
    assert!(
        body.contains("\n  └─ `feat/pns-recap-agent`  #591  *open*"),
        "the graph lost the spacing that lines it up: {body}"
    );
}

#[test]
fn what_needs_the_operator_survives_a_recap_that_had_to_be_cut() {
    // THE SAME DIRECTION THE NIGHT RECAP'S OWN BUDGET TAKES: a recap that
    // dropped what is waiting on the operator has failed at the one job it
    // had.
    let body = fitted(&recap(6, 30));
    assert!(body.contains("**User Tasks**"), "{body}");
    assert!(
        body.contains("- line 5 of User Tasks"),
        "the last thing the operator owes was cut: {body}"
    );
}

#[test]
fn the_characters_the_night_recap_drops_are_dropped_here_too() {
    // ONE FILTER, TWO READERS. A right-to-left override renders a line in an
    // order nobody wrote it in, and Discord honours it.
    let written = "Recap\n==========\n\n**Summary**\n- a\u{202e}b\u{200b}c\u{7}d\te\n";
    let body = fitted(written);
    for character in ['\u{202e}', '\u{200b}', '\u{7}'] {
        assert!(
            !body.contains(character),
            "{character:?} survived the sanitize: {body:?}"
        );
    }
    assert!(body.contains("- abcd e"), "{body:?}");
}

#[test]
fn a_recap_whose_user_tasks_alone_fill_the_message_still_carries_every_one_of_them() {
    // THE ONE SECTION ALLOWED PAST THE BUDGET, which is `budget::fit`'s own
    // rule for NEEDS YOU and the same reasoning: a recap that dropped what is
    // waiting on the operator has failed at the one job it had. Every other
    // section is shed first, and then the message runs over rather than
    // cutting this one.
    let mut written = recap(6, 30);
    for index in 6..40 {
        written.push_str(&format!(
            "\n- line {index} of User Tasks, long enough to spend a real share of one Discord message"
        ));
    }
    let body = fitted(&written);
    for index in 0..40 {
        assert!(
            body.contains(&format!("- line {index} of User Tasks,")),
            "user task {index} was shed: {body}"
        );
    }
    assert!(
        !body.contains("- line 0 of Summary,"),
        "everything else was kept instead: {body}"
    );
}

#[test]
fn a_file_list_pns_recap_git_already_collapsed_is_left_exactly_as_it_is() {
    // `pns recap git` COLLAPSES ITS OWN LONG LIST, so the body an agent pastes
    // may arrive carrying the counts rather than the rows. Tallying a tally
    // would turn "A 10  M 10  D 10" into "A 1".
    let written = recap(6, 0).replace(
        "  └─ `feat/pns-recap-agent`  #591  *open*  ×  ← *current*\n",
        "  └─ `feat/pns-recap-agent`  #591  *open*  ×  ← *current*\n\nA 10  M 10  D 10\n",
    );
    let body = fitted(&written);
    assert!(body.contains("\nA 10  M 10  D 10\n"), "{body}");
}
