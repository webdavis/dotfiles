//! The recap, pinned: composition.

use super::fixtures::*;
use crate::recap::activity::Event;
use crate::recap::budget::{MAX_CHARS, MAX_LINES, Trim, fit};
use crate::recap::external::{External, Found, Sourced, merged, noted};
use crate::recap::sections::Section;
use crate::recap::sections::{Open, Timeline, body, sections};

/// A window's lines, fitted to the delivery budget.
fn fitted(events: &[Event], open: &Open) -> Vec<String> {
    let projects = grouped(events);
    fit(
        &sections(&page(
            &projects,
            events.len(),
            &[],
            open,
            Timeline::Mechanical,
        )),
        MAX_LINES,
    )
}

/// One thing waiting on a person.
fn waiting(text: &str) -> Open {
    Open {
        sessions: vec![text.to_string()],
        ..Open::default()
    }
}

#[test]
fn the_body_opens_with_the_window_and_its_count_and_closes_with_what_is_open() {
    // THE FIRST LINE IS THE THREAD'S TITLE, because hermes names a forum
    // thread after it, so the header has to be first and has to read as a
    // title on its own. `open` IS LAST AND NEVER OMITTED, which is the
    // design's own order.
    let events = window(4);
    let lines = fitted(
        &events,
        &waiting("claude/blocked dotfiles: a decision is waiting"),
    );
    assert_eq!(
        lines[0], "While you were away, 23:04-06:15 · 4 events",
        "{lines:?}"
    );
    let agents = lines
        .iter()
        .position(|line| line == "AGENTS")
        .expect("an agents section");
    let open = lines
        .iter()
        .position(|line| line == "OPEN")
        .expect("an open section");
    assert!(agents < open, "open came first: {lines:?}");
    assert_eq!(
        lines[open + 1],
        "- claude/blocked dotfiles: a decision is waiting",
        "{lines:?}"
    );
}

#[test]
fn the_agents_section_is_one_line_per_session_grouped_under_its_project() {
    let events = vec![
        acted(1_756_500_000, "done", "the first"),
        acted(1_756_500_060, "failed", "the second"),
    ];
    let lines = fitted(&events, &Open::default());
    let agents = lines
        .iter()
        .position(|line| line == "AGENTS")
        .expect("an agents section");
    assert_eq!(
        &lines[agents + 1..agents + 4],
        [
            "dotfiles",
            "- the first  claude  0m  done",
            "- the second  claude  0m  failed",
        ],
        "{lines:?}"
    );
}

#[test]
fn a_session_shows_how_long_it_ran_inside_the_window_and_where_it_got_to() {
    let mut first = acted(1_756_500_000, "prompt", "the recap engine");
    first.session = "one".to_string();
    let mut last = acted(1_756_508_040, "blocked", "the recap engine");
    last.session = "one".to_string();
    let lines = fitted(&[first, last], &Open::default());
    assert!(
        lines
            .iter()
            .any(|line| line == "- the recap engine  claude  2h 14m  blocked"),
        "{lines:?}"
    );
}

#[test]
fn an_empty_open_section_still_prints_because_it_is_the_news() {
    let rendered = rendered_body(&window(2), &Open::default());
    assert!(
        rendered.contains("OPEN\n- nothing is waiting on you"),
        "{rendered}"
    );
}

#[test]
fn a_source_nobody_configured_is_absent_from_the_page_entirely() {
    // NOT A STATE OF THE SECTION: it is the section not existing, which is
    // what keeps `--section`'s accepted names and the page in step.
    let projects = grouped(&window(2));
    let open = Open::default();
    let sources = [(
        "commits",
        External {
            found: Found::Unconfigured,
            ..External::default()
        },
    )];
    let rendered = body(&page(&projects, 2, &sources, &open, Timeline::Mechanical));
    assert!(!rendered.contains("COMMITS"), "{rendered}");
}

#[test]
fn a_source_command_that_fails_renders_one_line_naming_its_exit_code() {
    // NEVER AN EMPTY SECTION. A command that exited 127 and a window with no
    // tasks in it are different news, and the exit code is what tells the
    // operator which they are looking at.
    let projects = grouped(&window(2));
    let open = Open::default();
    let sources = [(
        "tasks",
        External {
            found: Found::Failed(127),
            ..External::default()
        },
    )];
    let rendered = body(&page(&projects, 2, &sources, &open, Timeline::Mechanical));
    assert!(
        rendered.contains("TASKS: the command exited 127."),
        "{rendered}"
    );
}

/// A rendered body over one window with nothing sourced.
fn rendered_body(events: &[Event], open: &Open) -> String {
    let projects = grouped(events);
    body(&page(
        &projects,
        events.len(),
        &[],
        open,
        Timeline::Mechanical,
    ))
}

#[test]
fn a_window_too_long_for_the_budget_cuts_lines_and_never_a_count_or_an_open_item() {
    // THE BUDGET IS ENFORCED, NOT HOPED. Eighty sessions is a real overnight
    // window and a naive body is eighty lines long; what has to survive is
    // every open line, the true header count, and a remainder that is the
    // real number left out rather than what happened to fit.
    let events = window(80);
    let open = Open {
        sessions: (0..3).map(|which| format!("urgent {which}")).collect(),
        ..Open::default()
    };
    let lines = fitted(&events, &open);

    assert!(
        lines.len() <= MAX_LINES,
        "the budget was exceeded: {} lines",
        lines.len()
    );
    assert!(
        lines[0].ends_with("· 80 events"),
        "the header counts the window, not the survivors: {}",
        lines[0]
    );
    for urgent in ["urgent 0", "urgent 1", "urgent 2"] {
        assert!(
            lines.iter().any(|line| line.contains(urgent)),
            "{urgent} was cut: {lines:?}"
        );
    }
    let remainder = lines
        .iter()
        .find(|line| line.starts_with("...and "))
        .expect("a remainder line");
    // The section's own content is the project heading plus one line per
    // session, and the remainder counts what was cut off that.
    let shown = lines
        .iter()
        .filter(|line| line.contains("turn ") || *line == "dotfiles")
        .count();
    assert_eq!(
        remainder,
        &format!("...and {} more", 81 - shown),
        "the remainder disagreed with what was shown: {lines:?}"
    );
}

#[test]
fn a_worst_case_window_stays_inside_one_discord_message() {
    // ONE MESSAGE, AS LOCKED, and the line budget alone never bought it.
    // MEASURED on the real binary before the character ceiling existed:
    // forty entries at the ring's own field cap rendered as 2,859
    // characters inside 25 lines, and the operator's own hermes adapter
    // splits a Discord message at 1,900.
    //
    // WORST CASE MEANS EVERY FIELD FULL: forty sessions, each titled with
    // the longest text the writer will store.
    let events: Vec<Event> = (0..40)
        .map(|which| {
            acted(
                1_756_500_000 + which as u64 * 60,
                "done",
                &"d".repeat(ACTIVITY_MAX_CHARS),
            )
        })
        .collect();
    let open = Open::default();
    let projects = grouped(&events);
    let rendered = body(&page(&projects, 40, &[], &open, Timeline::Mechanical));

    assert!(
        rendered.chars().count() <= MAX_CHARS,
        "the recap would be split into two Discord messages: {} chars",
        rendered.chars().count()
    );
    // AND THE COUNTS ARE STILL TRUE, which is what the ceiling may never
    // buy itself: the header names the whole window and the remainder
    // names the whole tail, however short each surviving line was cut.
    assert!(
        rendered.starts_with("While you were away, 23:04-06:15 · 40 events"),
        "the header's count paid for the ceiling: {rendered}"
    );
    let shown = rendered
        .lines()
        .filter(|line| line.contains("ddd") || *line == "dotfiles")
        .count();
    assert!(
        rendered.contains(&format!("...and {} more", 41 - shown)),
        "the remainder disagrees with the {shown} lines that survived: {rendered}"
    );
    // AND LINES WERE CUT RATHER THAN DROPPED, which is the direction the
    // ceiling is supposed to fail in.
    assert!(
        shown > 10,
        "the ceiling was paid for by dropping the agents instead: {shown} lines"
    );

    // AND THE SAME WINDOW WITH TWO LIST SECTIONS AT THEIR OWN WORST CASE,
    // because those are PROTECTED: every character they spend is reserved
    // before the agents section is given a share, so their width and their
    // line count are what decide whether one message is still one message.
    let merges: Vec<Sourced> = (0..10)
        .map(|which| merged(200 + which, &"m".repeat(200), ""))
        .collect();
    let notes: Vec<Sourced> = (0..10)
        .map(|which| {
            noted(
                &format!("note-{which}.md"),
                &format!("# {}", "n".repeat(200)),
            )
        })
        .collect();
    let sources = [
        (
            "pull_requests",
            External {
                found: Found::Read(&merges),
                answered: None,
                truncated: false,
            },
        ),
        (
            "review_notes",
            External {
                found: Found::Read(&notes),
                answered: None,
                truncated: false,
            },
        ),
    ];
    let sourced = body(&page(&projects, 40, &sources, &open, Timeline::Mechanical));
    assert!(
        sourced.chars().count() <= MAX_CHARS,
        "two full list sections split the message in two: {} chars",
        sourced.chars().count()
    );
    assert!(
        sourced.starts_with("While you were away, 23:04-06:15 · 40 events"),
        "the header's count paid for the ceiling: {sourced}"
    );
    // AND THE AGENTS SECTION SURVIVED THEM, which is the direction that
    // matters: the protected sections take their reservation off the LENGTH
    // of a line, never off the list itself.
    assert!(
        sourced.lines().filter(|line| line.contains("ddd")).count() >= 5,
        "the list sections were paid for by dropping the agents: {sourced}"
    );
}

#[test]
fn an_open_list_longer_than_the_whole_budget_is_still_never_cut() {
    // THE ONE THING ALLOWED PAST THE BUDGET, stated as a test so nobody
    // "fixes" it. A recap that dropped what is waiting on the operator has
    // failed at the one job the phone card could not do for it.
    let open = Open {
        sessions: (0..40).map(|which| format!("urgent {which}")).collect(),
        dead_lettered: 7,
        ..Open::default()
    };
    let lines = fitted(&window(40), &open);
    for which in 0..40 {
        assert!(
            lines
                .iter()
                .any(|line| line.contains(&format!("urgent {which}"))),
            "urgent {which} was cut"
        );
    }
    assert!(
        lines
            .iter()
            .any(|line| line == "- 7 legs dead-lettered, run pns failures"),
        "the dead-letter line was cut: {lines:?}"
    );
    assert!(lines[0].ends_with("· 40 events"), "{}", lines[0]);
}

#[test]
fn no_dead_lettered_legs_says_nothing_about_them() {
    // A ZERO SAID OUT LOUD IS A LINE THE OPERATOR READS PAST every day, which
    // is how a standing count stops being read at all.
    let rendered = rendered_body(&window(2), &Open::default());
    assert!(!rendered.contains("dead-lettered"), "{rendered}");
    assert!(
        rendered.contains("OPEN\n- nothing is waiting on you"),
        "{rendered}"
    );
}

#[test]
fn one_dead_lettered_leg_is_singular_and_several_are_plural() {
    let one = rendered_body(
        &window(2),
        &Open {
            dead_lettered: 1,
            ..Open::default()
        },
    );
    assert!(
        one.contains("- 1 leg dead-lettered, run pns failures"),
        "{one}"
    );
    // The section is no longer empty, so its all-clear must be gone with it.
    assert!(!one.contains("nothing is waiting on you"), "{one}");
    let many = rendered_body(
        &window(2),
        &Open {
            dead_lettered: 12,
            ..Open::default()
        },
    );
    assert!(
        many.contains("- 12 legs dead-lettered, run pns failures"),
        "{many}"
    );
}

#[test]
fn a_trimmable_section_with_no_room_left_says_nothing_rather_than_half_a_line() {
    // THE FLOOR IS TWO LINES, its own heading and the remainder. Below that
    // the section is dropped whole and the header's count is still the
    // window's, which is the only number that has to survive.
    let cut = fit(
        &[
            Section {
                lines: vec!["a header".to_string()],
                trim: Trim::Never,
                omitted: 0,
                at_least: false,
            },
            Section {
                lines: vec!["AGENTS".to_string(), "one".to_string()],
                trim: Trim::Always,
                omitted: 0,
                at_least: false,
            },
        ],
        1,
    );
    assert_eq!(cut, ["a header"], "{cut:?}");
}
