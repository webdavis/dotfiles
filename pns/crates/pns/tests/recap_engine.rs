//! `pns recap`'s window dispatch, its refusals, and its two output forms.
//!
//! THE REAL BINARY IN A SANDBOX, because what these pin is argv reaching a
//! window and a window reaching a page: the domain tests already hold the
//! arithmetic and the layout against facts handed to them, and the engine
//! tests hold the assembly. What no unit test reaches is a word an operator
//! typed becoming the page they read.

mod support;

use support::{Sandbox, run, run_expecting, stderr, stdout};

/// One event in the store, `ago` seconds back, so a window that covers now
/// has something in it.
fn planted(sandbox: &Sandbox, ago: u64, state: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock")
        .as_secs();
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .record_activity_event(&pns_domain::recap::activity::Event {
            at: now - ago,
            agent: "claude".into(),
            state: state.into(),
            project: "dotfiles".into(),
            branch: "feat/x".into(),
            session: "one".into(),
            session_title: "the recap engine".into(),
            detail: "a decision is waiting".into(),
            ..pns_domain::recap::activity::Event::default()
        })
        .expect("the planted row");
}

fn sandbox_with_store(name: &str) -> Sandbox {
    let sandbox = Sandbox::new(name);
    sandbox.write_config("[recap]\nminimum_events = 1\n");
    std::fs::create_dir_all(sandbox.state()).expect("the state dir");
    sandbox
}

#[test]
fn a_bare_recap_names_the_window_that_most_recently_ended() {
    // WHICHEVER ONE ENDED MOST RECENTLY, so sitting down at 08:00 gives
    // nightshift and coming back at 13:30 gives morning. The test cannot
    // choose the hour it runs at, so what it pins is that the header names
    // ONE of the four periods and never a span with no name.
    let sandbox = sandbox_with_store("recap-bare-window");
    planted(&sandbox, 60, "done");
    let output = run(sandbox.pns_stateful().arg("recap"));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let first = stdout(&output)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(
        ["nightshift", "morning", "afternoon", "evening"]
            .iter()
            .any(|window| first.starts_with(window)),
        "a bare recap named no window at all: {first}"
    );
}

#[test]
fn a_named_window_heads_the_page_with_its_own_name() {
    let sandbox = sandbox_with_store("recap-named-window");
    planted(&sandbox, 60, "done");
    let output = run(sandbox.pns_stateful().args(["recap", "today"]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("today "), "{}", stdout(&output));
}

#[test]
fn open_prints_only_the_open_section_and_accepts_no_window_flag() {
    // THE "WHERE WAS I" ANSWER, and it has no window: `pns recap open` at
    // the keyboard and `pns recap open --to banner` from an unlock
    // automation both want the same one section.
    let sandbox = sandbox_with_store("recap-open");
    planted(&sandbox, 60, "blocked");
    let output = run(sandbox.pns_stateful().args(["recap", "open"]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let page = stdout(&output);
    assert!(page.contains("◆ OPEN"), "{page}");
    assert!(
        !page.contains("AGENTS"),
        "open printed a section it has no window for: {page}"
    );
    assert!(
        page.contains("claude/blocked dotfiles: a decision is waiting"),
        "{page}"
    );
}

#[test]
fn open_is_never_omitted_even_when_nothing_is_waiting() {
    // AN EMPTY OPEN IS THE NEWS the page exists to carry, so it is the one
    // section never omitted and never shed.
    let sandbox = sandbox_with_store("recap-open-empty");
    planted(&sandbox, 60, "done");
    let output = run(sandbox.pns_stateful().args(["recap", "open"]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("OPEN") && stdout(&output).contains("- nothing is waiting on you"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn two_span_flags_together_and_a_previous_with_no_window_are_refused_with_one_sentence() {
    let sandbox = sandbox_with_store("recap-span-refusals");
    for argv in [
        ["recap", "morning", "--since", "2h"].as_slice(),
        ["recap", "--since", "2h", "--duration", "2h"].as_slice(),
        ["recap", "open", "--previous"].as_slice(),
        ["recap", "--previous"].as_slice(),
        ["recap", "yesterdya"].as_slice(),
    ] {
        let output = run_expecting(2, sandbox.pns_stateful().args(argv));
        assert!(
            stderr(&output).starts_with("pns: usage: pns recap"),
            "{argv:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn a_destination_nothing_registers_is_refused_with_the_configured_ones_listed() {
    let sandbox = sandbox_with_store("recap-destination-refusal");
    planted(&sandbox, 60, "done");
    let output = run_expecting(
        2,
        sandbox
            .pns_stateful()
            .args(["recap", "today", "--to", "carrier-pigeon"]),
    );
    assert!(
        stderr(&output).contains("`carrier-pigeon` is no configured destination"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_section_this_machine_does_not_serve_is_refused_with_the_served_ones_listed() {
    let sandbox = sandbox_with_store("recap-section-refusal");
    let output = run_expecting(
        2,
        sandbox
            .pns_stateful()
            .args(["recap", "today", "--section", "tasks"]),
    );
    assert!(
        stderr(&output).contains("`tasks` is no recap section on this machine"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_source_command_that_fails_renders_one_line_naming_its_exit_code() {
    // NEVER AN EMPTY SECTION. A command that exited 7 and a window with no
    // tasks in it are different news, and the code is what tells them apart.
    let sandbox = Sandbox::new("recap-source-exit-code");
    sandbox.write_config(
        "[recap]\nminimum_events = 1\n[recap.sources]\ntasks = [\"recap-task-stub\"]\n",
    );
    std::fs::create_dir_all(sandbox.state()).expect("the state dir");
    planted(&sandbox, 60, "done");
    let mut command = sandbox.pns_stateful();
    sandbox.stub_on_path(&mut command, "recap-task-stub", "exit 7");
    let output = run(command.args(["recap", "today"]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("TASKS: the command exited 7."),
        "{}",
        stdout(&output)
    );
}

#[test]
fn the_document_flags_and_the_mask_decide_the_form_and_the_format() {
    // `--schema` ALONE SELECTS DOCUMENT OUTPUT in the mask's own format, and
    // `--json` or `--toon` beside it overrides only the format.
    let sandbox = sandbox_with_store("recap-document-forms");
    planted(&sandbox, 60, "done");
    let json = run(sandbox.pns_stateful().args(["recap", "today", "--json"]));
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    assert!(
        stdout(&json).starts_with("{\"schema\":1,"),
        "{}",
        stdout(&json)
    );

    let toon = run(sandbox.pns_stateful().args(["recap", "today", "--toon"]));
    assert_eq!(toon.status.code(), Some(0), "{}", stderr(&toon));
    assert!(
        stdout(&toon).starts_with("schema: 1\n"),
        "{}",
        stdout(&toon)
    );

    let mask = sandbox.path("mask.toon");
    std::fs::write(&mask, "schema: true\nwindow:\n  name: true\n").expect("the mask");
    let masked = run(sandbox.pns_stateful().args([
        "recap",
        "today",
        "--schema",
        mask.to_str().expect("a path"),
    ]));
    assert_eq!(masked.status.code(), Some(0), "{}", stderr(&masked));
    assert_eq!(
        stdout(&masked).trim_end(),
        "schema: 1\nwindow:\n  name: today",
        "a TOON mask did not select TOON out"
    );

    let overridden = run(sandbox.pns_stateful().args([
        "recap",
        "today",
        "--json",
        "--schema",
        mask.to_str().expect("a path"),
    ]));
    assert_eq!(overridden.status.code(), Some(0), "{}", stderr(&overridden));
    assert_eq!(
        stdout(&overridden).trim_end(),
        "{\"schema\":1,\"window\":{\"name\":\"today\"}}",
        "`--json` did not override the mask's own format"
    );
}

#[test]
fn a_mask_key_that_names_nothing_is_refused_rather_than_silently_emptying_a_field() {
    let sandbox = sandbox_with_store("recap-mask-typo");
    planted(&sandbox, 60, "done");
    let mask = sandbox.path("typo.json");
    std::fs::write(&mask, "{\"section\": true}").expect("the mask");
    let output = run_expecting(
        2,
        sandbox
            .pns_stateful()
            .args(["recap", "today", "--schema", mask.to_str().expect("a path")]),
    );
    assert!(
        stderr(&output).contains("the field mask names `section`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_printed_page_is_the_house_style_and_a_delivered_one_is_plain() {
    // THE TERMINAL GETS THE HOUSE STYLE (`◆ Title ──` headings, the accent
    // colour on the window) and the durable route gets the plain body fitted
    // to one message. A page posted with escape codes in it renders them
    // verbatim in the channel.
    let sandbox = sandbox_with_store("recap-house-style");
    planted(&sandbox, 60, "done");
    let printed = run(sandbox.pns_stateful().args(["recap", "today"]));
    assert_eq!(printed.status.code(), Some(0), "{}", stderr(&printed));
    let page = stdout(&printed);
    assert!(page.contains("◆ AGENTS"), "{page}");
    assert!(page.contains("◆ OPEN"), "{page}");
    assert!(
        !page.lines().any(|line| line.ends_with(' ')),
        "a heading was written with nothing after its separator: {page}"
    );
}
