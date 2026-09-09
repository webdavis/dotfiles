use crate::legacy::{USAGE, is_producer_argv};
use crate::*;

/// What every command reads instead of the environment: argv from the
/// subcommand on, with the tool-wide flags already taken out.
///
/// PUBLISHED ONCE rather than threaded, for the same reason the color answer
/// is. Fifteen commands used to reach for `std::env::args_os()` themselves,
/// which is what made a flag typed BEFORE the subcommand shift every position
/// after it: `pns --no-color daemon start` handed `daemon` to the daemon as its
/// verb, and `pns --no-color doctor` looked to the doctor like a stray word to
/// refuse. Reading one filtered answer is what makes the flag mean the same
/// thing wherever it is typed.
static TOOL_ARGV: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// argv from the subcommand on. Index 0 is the subcommand itself.
pub(crate) fn tool_argv() -> &'static [String] {
    TOOL_ARGV.get().map(Vec::as_slice).unwrap_or_default()
}

/// The words after the subcommand, which is what a one-word command validates.
pub(crate) fn arguments_after_subcommand() -> Vec<String> {
    tool_argv().iter().skip(1).cloned().collect()
}

/// The words after the subcommand AND its verb, for the commands that take one
/// (`pns lights tick`, `pns loop begin`, `pns presence poll`).
pub(crate) fn arguments_after_verb() -> Vec<String> {
    tool_argv().iter().skip(2).cloned().collect()
}

/// The word after the subcommand, or empty when there is none.
fn second_argument(flagless: &[String]) -> String {
    flagless.get(1).cloned().unwrap_or_default()
}

/// The tool-wide flags, taken out of argv wherever they appeared.
///
/// `--no-color` ANSWERS IN EVERY POSITION. `pns --no-color doctor` and
/// `pns doctor --no-color` mean the same thing, because an operator who has
/// decided about color has decided about the whole command rather than about
/// one subcommand's report. Taking it out here is what lets each subcommand go
/// on treating an argument it does not know as a refusal.
fn take_tool_wide_flags(argv: &[String]) -> (Vec<String>, bool) {
    let mut forced_plain = false;
    let kept = argv
        .iter()
        .filter(|argument| {
            if *argument == NO_COLOR_FLAG {
                forced_plain = true;
                return false;
            }
            true
        })
        .cloned()
        .collect();
    (kept, forced_plain)
}

/// The one flag every printing command answers to.
const NO_COLOR_FLAG: &str = "--no-color";
/// What a producer gets when its own page did not reach the durable log.
///
/// ONE, NOT TWO. Two is what this mode already returns for argv it will not
/// accept, and a producer that could not tell a lost page from a mistyped
/// command would have to guess which of the two it was looking at.
const EVENT_NOT_DELIVERED: i32 = 1;

/// The event mode's exit code, which is the ONE thing a synchronous producer
/// can read.
///
/// THE ALWAYS-EXIT-0 CONTRACT IS ABOUT ARGV, not about delivery: a stray token
/// degrades into an empty event rather than a failure, because this sits on a
/// notification path that must not break its caller. A gateway that refused the
/// page is a different fact, and one a producer such as posture has no other way
/// to learn.
pub(crate) fn event_mode(argv: &[String]) -> i32 {
    crate::legacy::run(argv, |event| {
        // Legacy argv carries no harness payload.
        match run_event(
            &event,
            &system_probes(),
            &HookPayload::default(),
            Attempt::First,
        ) {
            event_flow::Landed::Yes => 0,
            event_flow::Landed::No => EVENT_NOT_DELIVERED,
        }
    })
}

pub(crate) fn run() {
    // ONE READ OF ARGV, lossy rather than validating: `std::env::args()`
    // panics on non-UTF-8, and a stray byte degrading into an unknown token
    // (which the lenient parser already skips) is the honest failure mode
    // for an always-exit-0 notification path. `first`, the producer check
    // and the event parse each used to read `std::env::args_os()` on their
    // own; this is the one collection they share now.
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    // THE EVENT PATH KEEPS THE ORIGINAL ARGV, and every subcommand below reads
    // the filtered one. The filter is position-blind, so a producer sending
    // `--detail --no-color` would lose its value to it; the event path prints
    // nothing but an exit code, so the flag means nothing there anyway and
    // passing argv through unchanged keeps that contract exact.
    let (flagless, forced_plain) = take_tool_wide_flags(&argv);
    style::remember_forced_plain(forced_plain);
    let _ = TOOL_ARGV.set(flagless.clone());
    let first = flagless.first().cloned().unwrap_or_default();
    if matches!(first.as_str(), "--version" | "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if first == "shell" {
        std::process::exit(crate::shell_mode(&flagless[1..]));
    }
    if first == "submit" {
        std::process::exit(event_flow::submit_mode(&flagless[1..]));
    }
    // The pulse is a MODE, not a leg: it fires on a long command's exit code
    // rather than on an event, so it leaves before any of the event wiring.
    if first == "pulse" {
        std::process::exit(pulse_mode());
    }
    // The home diagnostic: one reading of the router, said out loud. The
    // doctor mode (P3) will absorb it; until then this is how the probe is
    // drilled and how a wrong config is diagnosed.
    if first == "home" {
        home_mode();
        return;
    }
    // The operator's mute, typed and timed. Also a MODE: it writes the state
    // the event path reads, and delivers nothing itself.
    if first == "quiet" {
        std::process::exit(quiet_mode());
    }
    // One test send through every configured channel, and one line per
    // registered plugin about it. A MODE for the same reason the others are:
    // it takes no decision, so nothing about an event's plan reaches it.
    if first == "doctor" {
        std::process::exit(doctor_mode());
    }
    // What a click on a failure banner runs. A MODE for the reason the others
    // are: it opens a view and delivers nothing. It is NEVER TYPED by the
    // operator, so every path here ends in something they can see.
    if first == "click" {
        std::process::exit(click_mode());
    }
    // The detail view over what is not arriving. A MODE beside the doctor's
    // for the same reason: it reads the ledger, prints, and delivers nothing,
    // so no event's plan reaches it. The doctor reports the count and names
    // this, which is what makes one command to discover and one to use.
    if first == "failures" {
        std::process::exit(failures_mode());
    }
    // The return recap, rendered from the activity ring and posted to Discord.
    // A MODE for the reason the others are: it takes no decision, so no event's
    // plan reaches it. The event path starts it detached; an operator can also
    // run it by hand, which is how it is drilled.
    if first == "recap" {
        std::process::exit(recap_mode());
    }
    // The clock. A MODE for the reason the others are: `run` takes no event
    // and delivers nothing itself, and the two typed verbs beside it only move
    // a file. Nothing on the event path below reaches it, and nothing here
    // reaches the event path except by re-executing this binary.
    if first == "daemon" {
        std::process::exit(daemon_mode(&second_argument(&flagless)));
    }
    // The lamps' upkeep. A MODE beside the daemon's for the same reason: it
    // takes no decision and delivers nothing, and the daemon is what runs it.
    // It reaches the event path through nothing at all.
    if first == "lights" {
        std::process::exit(lights_mode(&second_argument(&flagless)));
    }
    // The room sensor's own upkeep. A MODE beside the lamps' for the same
    // reason: it reads the bridge, publishes one state line and delivers
    // nothing, and the daemon is what runs it.
    if first == "presence" {
        std::process::exit(presence_mode(&second_argument(&flagless)));
    }
    // The loop lease, taken and given back by hand. A MODE beside the lamps'
    // for the same reason: it moves one file and delivers nothing.
    if first == "loop" {
        std::process::exit(loop_mode(&second_argument(&flagless)));
    }
    // The nudge about an approval nobody answered. A MODE for the reason the
    // others are: it takes no decision from an event and reads no stdin. It
    // takes NO SESSION ARGUMENT either, because coalescing means it looks at
    // every outstanding record rather than at the one whose timer woke it, so
    // an argument would be a value it had to ignore.
    if first == "nag" {
        std::process::exit(nag_mode());
    }
    // The first-run walk. A MODE that has to be reachable with NO CONFIG AT
    // ALL, which is the state it exists to end, and that is why it sits above
    // everything that loads one. Nothing on the event path reaches it and it
    // reaches nothing there: it asks questions, composes text and publishes a
    // file, and delivers nothing.
    if first == "setup" {
        std::process::exit(setup_mode());
    }
    // The gate moshi's OWN extension calls. pi and omp spawn
    // `helperBinary pi-hook`, and that field holds one PATHNAME with no room
    // for a subcommand, so the binary answers the bare harness word itself.
    if pns_adapters::is_harness_subcommand(&first) {
        std::process::exit(gate_mode(&first));
    }
    // The same gate, spelled the way an operator reads it. Both forms end in
    // gate_mode, which REFUSES a word it will not vouch for: falling through
    // to the event path instead is how the documented spelling used to fire a
    // notification about an empty event.
    if first == "gate" {
        std::process::exit(gate_mode(&second_argument(&flagless)));
    }
    if first == "hook" {
        std::process::exit(hook_mode(&second_argument(&flagless)));
    }
    // A WORD THAT NAMES NO COMMAND IS A TYPO, never an event. It is the house
    // rule `pns nag` and `pns lights` already keep, moved up to where argv[1]
    // is decided: the producer parser is deliberately lenient about a token it
    // does not know, so `pns stpo` used to skip the word, render an empty event
    // and deliver it. The always-exit-0 contract governs EVENT deliveries, and
    // a word naming no command never becomes one, so refusing it here
    // contradicts nothing. `--help`/`-h` still reaches `event_mode` from here
    // (see `is_producer_argv`): that parser holds the one help arm now, so
    // there is no second copy of it up here to answer help before anything
    // else runs.
    if !is_producer_argv(&argv) {
        eprint!("{USAGE}");
        std::process::exit(2);
    }
    std::process::exit(event_mode(&argv));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn the_color_flag_answers_before_the_subcommand_and_after_it() {
        for argv in [
            strings(&["--no-color", "doctor"]),
            strings(&["doctor", "--no-color"]),
        ] {
            let (flagless, forced_plain) = take_tool_wide_flags(&argv);
            assert!(forced_plain, "{argv:?}");
            assert_eq!(flagless, strings(&["doctor"]), "{argv:?}");
        }
    }

    #[test]
    fn argv_without_the_flag_is_passed_through_unchanged() {
        let argv = strings(&["daemon", "schedule", "--id", "x"]);
        let (flagless, forced_plain) = take_tool_wide_flags(&argv);
        assert!(!forced_plain);
        assert_eq!(flagless, argv);
    }

    #[test]
    fn the_flag_typed_twice_is_still_one_answer_and_leaves_nothing_behind() {
        let argv = strings(&["--no-color", "lights", "--no-color", "tick"]);
        let (flagless, forced_plain) = take_tool_wide_flags(&argv);
        assert!(forced_plain);
        assert_eq!(flagless, strings(&["lights", "tick"]));
    }

    #[test]
    fn a_verb_keeps_its_position_when_the_flag_was_typed_before_the_subcommand() {
        // The bug this exists to prevent: reading the verb off the environment
        // handed `daemon` to the daemon as its own verb.
        let (flagless, _) = take_tool_wide_flags(&strings(&["--no-color", "daemon", "start"]));
        assert_eq!(second_argument(&flagless), "start");
    }

    #[test]
    fn a_word_that_merely_contains_the_flag_is_not_the_flag() {
        let argv = strings(&["recap", "--since=--no-color"]);
        let (flagless, forced_plain) = take_tool_wide_flags(&argv);
        assert!(!forced_plain);
        assert_eq!(flagless, argv);
    }
}
