//! `--help` and `-h` on every subcommand, through the real binary.
//!
//! THE LIST COMES FROM THE TOOL'S OWN HELP rather than from a copy here, so a
//! subcommand added to the listing without a usage string of its own fails
//! this suite instead of answering with the whole catalog. The unit test
//! beside the catalog closes the other direction: every word it holds is named
//! in the listing.

mod support;

use support::{Sandbox, run, stderr, stdout};

/// Every subcommand the tool-wide listing names, taken off `pns --help`.
///
/// A LINE NAMES A SUBCOMMAND when its second word is a bare lowercase word:
/// that skips the `<harness>-hook` gate, the `<subcommand>` help line and the
/// two tool-wide flags, which are spellings rather than subcommands.
fn listed_subcommands(listing: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for line in listing.lines() {
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some("pns") {
            continue;
        }
        let Some(word) = tokens.next() else { continue };
        if !word.chars().all(|letter| letter.is_ascii_lowercase()) {
            continue;
        }
        if !words.iter().any(|kept| kept == word) {
            words.push(word.to_string());
        }
    }
    words
}

#[test]
fn the_tool_wide_help_lists_every_subcommand_and_exits_zero() {
    let sandbox = Sandbox::new("help-listing");
    for spelling in ["--help", "-h"] {
        let output = run(sandbox.pns().arg(spelling));
        assert_eq!(output.status.code(), Some(0), "{spelling}: {output:?}");
        let listed = listed_subcommands(&stdout(&output));
        // The machine-called subcommands are in here as much as the typed
        // ones: a reader who cannot find `failures` or `daemon retry` in the
        // help concludes they do not exist.
        for expected in [
            "send", "hook", "daemon", "failures", "recap", "presence", "github", "shell", "lights",
            "remind", "stale",
        ] {
            assert!(
                listed.iter().any(|word| word == expected),
                "{spelling} must name `{expected}`: {listed:?}"
            );
        }
        for machine_called in [
            "pns daemon retry",
            "pns recap agent",
            "pns recap git",
            "pns presence poll [--daemon]",
            "pns failures serve",
        ] {
            assert!(
                stdout(&output).contains(machine_called),
                "{spelling} must name `{machine_called}`"
            );
        }
    }
}

#[test]
fn every_subcommand_answers_both_help_flags_with_its_own_usage_and_exits_zero() {
    // HELP ASKED FOR IS A SUCCESS, which is the whole difference from the
    // refusals beside it: those exit 2 because the caller typed something
    // wrong. And a subcommand's help is ITS OWN text, so `pns send --help`
    // answers about `send` rather than reprinting the catalog.
    let sandbox = Sandbox::new("help-per-subcommand");
    let listing = stdout(&run(sandbox.pns().arg("--help")));
    let subcommands = listed_subcommands(&listing);
    assert!(subcommands.len() > 10, "{subcommands:?}");
    for word in &subcommands {
        for spelling in ["--help", "-h"] {
            let output = run(sandbox.pns().args([word.as_str(), spelling]));
            let printed = stdout(&output);
            assert_eq!(
                output.status.code(),
                Some(0),
                "pns {word} {spelling}: {output:?}"
            );
            assert!(
                printed.contains(&format!("pns {word}")),
                "pns {word} {spelling} must print its own usage: {printed}"
            );
            assert_eq!(
                stderr(&output),
                "",
                "pns {word} {spelling} is not a complaint: {output:?}"
            );
        }
    }
}

#[test]
fn a_subcommands_help_reaches_no_channel_and_writes_no_state() {
    // The reason the tool-wide help is answered before anything loads: a help
    // print that reached the event path notified about an empty event.
    let sandbox = Sandbox::new("help-reaches-nothing");
    for argv in [
        ["send", "--help"],
        ["hook", "--help"],
        ["lights", "--help"],
        ["failures", "--help"],
    ] {
        let output = run(sandbox.pns().args(argv));
        assert_eq!(output.status.code(), Some(0), "{argv:?}: {output:?}");
        assert!(!sandbox.fired("mobile"), "{argv:?}: {output:?}");
        assert!(!sandbox.fired("hermes"), "{argv:?}: {output:?}");
        assert!(!sandbox.fired("banner"), "{argv:?}: {output:?}");
        assert!(!sandbox.state().exists(), "{argv:?}: {output:?}");
    }
}

#[test]
fn a_verb_asks_for_its_own_subcommands_help() {
    let sandbox = Sandbox::new("help-behind-a-verb");
    for argv in [
        ["daemon", "schedule", "--help"],
        ["lights", "pulse", "-h"],
        ["recap", "agent", "--help"],
        ["presence", "poll", "--help"],
    ] {
        let output = run(sandbox.pns().args(argv));
        assert_eq!(output.status.code(), Some(0), "{argv:?}: {output:?}");
        assert!(stdout(&output).contains("usage"), "{argv:?}: {output:?}");
        assert_eq!(stderr(&output), "", "{argv:?}: {output:?}");
    }
}

#[test]
fn a_genuine_argument_error_still_refuses_with_exit_two() {
    // The two halves kept apart: asking for help is exit 0 on stdout, and
    // typing something wrong is exit 2 on stderr, even though both print the
    // same usage text.
    let sandbox = Sandbox::new("help-versus-refusal");
    for argv in [
        ["daemon", "stpo"].as_slice(),
        ["lights", "stpo"].as_slice(),
        ["presence", "stpo"].as_slice(),
        ["github", "stpo"].as_slice(),
        ["doctor", "--stpo"].as_slice(),
        ["setup", "--stpo"].as_slice(),
        ["tap", "stpo"].as_slice(),
        ["shell", "stpo"].as_slice(),
        ["stpo"].as_slice(),
    ] {
        let output = sandbox.pns().args(argv).output().expect("the engine runs");
        assert_eq!(output.status.code(), Some(2), "{argv:?}: {output:?}");
    }
}
