use crate::legacy::{USAGE, is_help_flag};
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
    if first == SEND {
        std::process::exit(send_mode(&arguments_after_send(&argv)));
    }
    if first == "tap" {
        std::process::exit(crate::command_tap::tap_mode());
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
    // The GitHub notification source's own poll. A MODE beside the room
    // sensor's for the same reason: it reads one remote listing, submits what
    // it finds through the ordinary producer path, and takes no decision of
    // its own. The daemon is what runs it.
    if first == pns_adapters::GITHUB {
        std::process::exit(github_mode(&second_argument(&flagless)));
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
    // The page about a session nobody came back to. A MODE beside the nag's
    // for the same reasons: it reads no stdin, takes no decision from an
    // event, and takes NO SESSION ARGUMENT, because one fire covers every
    // session stuck past the window rather than the one whose timer woke it.
    if first == pns_domain::stale::FIRE_WORD {
        std::process::exit(stale_mode());
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
    // ARGV THAT NAMES NO SUBCOMMAND ENDS HERE, and only `--help` ends well.
    // Sending is `pns send` and nothing else, so a bare `pns --state done` is
    // refused instead of reaching the lenient producer parser, which skipped
    // the tokens it did not know and delivered an empty event about them.
    // Refusing costs the always-exit-0 contract nothing: that contract governs
    // EVENT deliveries, and argv naming no command never becomes one.
    let usage = Usage::of(&first);
    if usage.refused() {
        eprint!("{USAGE}");
    } else {
        print!("{USAGE}");
    }
    std::process::exit(usage.exit_code());
}

/// Why the usage text is being printed, which is what decides the stream and
/// the exit code.
#[derive(Debug, PartialEq, Eq)]
enum Usage {
    /// `--help`/`-h`, with no subcommand behind it: what the operator asked
    /// for, so it goes to stdout and exits 0.
    Requested,
    /// Any other word, the empty one included: a typo, a retired spelling, or
    /// the bare event path that `send` replaced.
    Refused,
}

impl Usage {
    fn of(first: &str) -> Self {
        if is_help_flag(first) {
            Self::Requested
        } else {
            Self::Refused
        }
    }
    fn refused(&self) -> bool {
        *self == Self::Refused
    }
    fn exit_code(&self) -> i32 {
        match self {
            Self::Requested => 0,
            Self::Refused => 2,
        }
    }
}

/// The one sending subcommand.
const SEND: &str = "send";

/// `pns send`: one subcommand, two input forms.
///
/// THE FORM IS CHOSEN BY `--json` IN LEADING POSITION, once `--no-color` is
/// taken out of the way: that is the one flag that answers wherever it is
/// typed, so `pns send --no-color --json` and `pns send --json --no-color`
/// both name the envelope. The envelope branch then hands `submit::run` that
/// SAME no-color-filtered tail, since its exact-argv check demands `--json`
/// alone. The flags branch keeps the raw tail, so a value that merely spells
/// `--no-color` (`--detail --no-color`) is not mistaken for the flag.
fn send_mode(args: &[String]) -> i32 {
    let (filtered, _) = take_tool_wide_flags(args);
    match SendForm::of(&filtered) {
        SendForm::Envelope => event_flow::submit_mode(&filtered),
        SendForm::Flags => event_mode(args),
    }
}

/// Which of `pns send`'s two input forms this call is using.
#[derive(Debug, PartialEq, Eq)]
enum SendForm {
    /// One JSON request on standard input, selected by `--json`.
    Envelope,
    /// The request spelled as flags.
    Flags,
}

impl SendForm {
    /// LEADING TOKEN ONLY: a flag whose value spells `--json`
    /// (`--detail --json`) leaves it in a later position, where it is just a
    /// value rather than the form selector.
    fn of(args: &[String]) -> Self {
        if args.first().is_some_and(|argument| argument == "--json") {
            Self::Envelope
        } else {
            Self::Flags
        }
    }
}

/// What `pns send` was handed, taken from the ORIGINAL argv.
///
/// UNFILTERED, for the reason `run` keeps the original: a producer sending
/// `--detail --no-color` would otherwise lose that value to the tool-wide
/// filter. The subcommand is the first word that is not the color flag, so the
/// flag still answers in either position without a value that merely spells it
/// being mistaken for it.
fn arguments_after_send(argv: &[String]) -> Vec<String> {
    let subcommand = argv
        .iter()
        .position(|token| token != NO_COLOR_FLAG)
        .map_or(0, |index| index + 1);
    argv.iter().skip(subcommand).cloned().collect()
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

    #[test]
    fn both_input_forms_are_read_off_the_one_send_subcommand() {
        assert_eq!(
            SendForm::of(&strings(&["--producer", "lights", "--state", "done"])),
            SendForm::Flags
        );
        assert_eq!(SendForm::of(&strings(&["--json"])), SendForm::Envelope);
    }

    #[test]
    fn json_in_a_value_position_is_still_just_a_value() {
        // `--detail`'s own value could legitimately spell `--json`; only the
        // leading token names the form.
        assert_eq!(
            SendForm::of(&strings(&[
                "--producer",
                "x",
                "--state",
                "done",
                "--detail",
                "--json"
            ])),
            SendForm::Flags
        );
    }

    #[test]
    fn the_color_flag_answers_in_either_position_for_the_envelope_form_too() {
        for args in [
            strings(&["--no-color", "--json"]),
            strings(&["--json", "--no-color"]),
        ] {
            let (filtered, _) = take_tool_wide_flags(&args);
            assert_eq!(SendForm::of(&filtered), SendForm::Envelope, "{args:?}");
            assert_eq!(filtered, strings(&["--json"]), "{args:?}");
        }
    }

    #[test]
    fn the_bare_event_path_is_refused_rather_than_delivered_empty() {
        // The whole point of the subcommand: argv that used to render an empty
        // event and notify about it now earns the usage text and exit 2.
        for first in ["--state", "--producer", "stpo", ""] {
            let usage = Usage::of(first);
            assert_eq!(usage, Usage::Refused, "{first:?}");
            assert_eq!(usage.exit_code(), 2, "{first:?}");
        }
    }

    #[test]
    fn help_with_no_subcommand_still_prints_and_exits_zero() {
        for first in ["--help", "-h"] {
            let usage = Usage::of(first);
            assert_eq!(usage, Usage::Requested, "{first}");
            assert!(!usage.refused(), "{first}");
            assert_eq!(usage.exit_code(), 0, "{first}");
        }
    }

    #[test]
    fn send_hands_on_what_followed_it_and_keeps_a_value_spelling_the_color_flag() {
        for argv in [
            strings(&["send", "--detail", "--no-color"]),
            strings(&["--no-color", "send", "--detail", "--no-color"]),
        ] {
            assert_eq!(
                arguments_after_send(&argv),
                strings(&["--detail", "--no-color"]),
                "{argv:?}"
            );
        }
    }
}
