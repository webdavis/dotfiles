use crate::*;

/// Everything this binary answers to, and the flags a producer states an event
/// with. Printed on request and on a refusal, which is why it is one text: an
/// operator who mistyped and an operator who asked have the same question.
pub(crate) const USAGE: &str = "\
pns: usage:
  pns [<producer flags>]           one notification, stated in argv
  pns hook <event>                 a harness hook: prompt, stop, stop-failure,
                                   blocked, asked, plan-ready, denied, resolved,
                                   model-switch, quota, config-change
  pns gate <harness>-hook          presence-gated pass-through to moshi-hook
  pns <harness>-hook               the same gate, spelled the way moshi calls it
  pns pulse <exit-code>            signal the lamps by hand
  pns quiet [<duration>|off]       the operator's mute
  pns daemon run|schedule|cancel   the clock
  pns lights tick|quiet            the lamps' upkeep
  pns presence poll                one bridge read, published for the sensor
  pns loop begin|end               take the loop lamp by hand, and give it back
  pns nag                          card every outstanding approval
  pns recap --since <epoch> --until <epoch>
  pns setup [--force]              write a first config, one question at a time
  pns doctor                       one test send through every channel
  pns home                         one reading of the router, said out loud
  pns --help, -h                   this text
  pns --version, -V                the package version

producer flags: --agent <name> --state <word> --project <name> --branch <name>
                --detail <text> --pane <id> --channel <route> --elapsed <secs>
                --local-only --remote-only --long-running
";
/// Whether argv is a PRODUCER invocation rather than a mistyped subcommand.
///
/// IT READS THE WHOLE OF ARGV, not just the leading word, and that is the
/// point. The parser deliberately accepts a stray token in front of the real
/// flags, so a leading word alone does not make an invocation a typo: what does
/// is argv carrying no producer flag, and no `--help`/`-h`, anywhere. Refusing
/// on the first word alone would drop real notifications, which is the exact
/// mirror of the bug this refusal exists to fix.
///
/// AN EMPTY ARGV is the bare invocation `args` calls a valid empty event.
/// A DASH-LED FIRST WORD IS NO LONGER A FREE PASS: that used to make ANY
/// dash-led argv[1] a producer invocation, so a mistyped flag (`--wat`,
/// `-help`, `--agent=claude`) delivered an empty event in silence, the `pns
/// stpo` bug reopened for a typo that happens to start with a dash.
/// `--help`/`-h` ARE COUNTED, so a producer invocation that only adds
/// `--help` still reaches the parser below, which is where the help arm
/// actually prints the usage and returns.
pub(crate) fn is_producer_argv(argv: &[String]) -> bool {
    argv.is_empty()
        || argv
            .iter()
            .any(|token| pns::args::is_producer_flag(token) || pns::args::is_help_flag(token))
}
/// The word after the subcommand, or empty when there is none.
pub(crate) fn second_argument() -> String {
    std::env::args_os()
        .nth(2)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
/// One notification from argv, or a usage print when `--help`/`-h` reached
/// the parse in FLAG position.
pub(crate) fn event_mode(argv: &[String]) -> i32 {
    let parsed = parse_args(argv.iter().cloned());
    // HELP WINS BEFORE ANYTHING ELSE ON THIS PATH: no config load, no probe.
    // It used to reach EVERYTHING when it fell through this same parser as an
    // unknown token, which notified about an empty event and raised a banner
    // titled "pns · done". Nothing about printing the commands needs the
    // machine read.
    if parsed.event.help {
        print!("{USAGE}");
        return 0;
    }
    for warning in &parsed.warnings {
        eprintln!("pns: {warning}");
    }
    let event = match parsed.into_event() {
        Ok(Some(event)) => event,
        Ok(None) => return 0,
        Err(error) => {
            eprintln!("pns: {error}");
            return 2;
        }
    };
    // ARGV CARRIES NO PAYLOAD, which is the honest no-identity case.
    run_event(
        &event,
        &system_probes(),
        &HookPayload::default(),
        Attempt::First,
    );
    0
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
    let first = argv.first().cloned().unwrap_or_default();
    if matches!(first.as_str(), "--version" | "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if first == "submit" {
        std::process::exit(event_flow::submit_mode(&argv[1..]));
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
        std::process::exit(daemon_mode(&second_argument()));
    }
    // The lamps' upkeep. A MODE beside the daemon's for the same reason: it
    // takes no decision and delivers nothing, and the daemon is what runs it.
    // It reaches the event path through nothing at all.
    if first == "lights" {
        std::process::exit(lights_mode(&second_argument()));
    }
    // The room sensor's own upkeep. A MODE beside the lamps' for the same
    // reason: it reads the bridge, publishes one state line and delivers
    // nothing, and the daemon is what runs it.
    if first == "presence" {
        std::process::exit(presence_mode(&second_argument()));
    }
    // The loop lease, taken and given back by hand. A MODE beside the lamps'
    // for the same reason: it moves one file and delivers nothing.
    if first == "loop" {
        std::process::exit(loop_mode(&second_argument()));
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
    if pns::hooks::is_harness_subcommand(&first) {
        std::process::exit(gate_mode(&first));
    }
    // The same gate, spelled the way an operator reads it. Both forms end in
    // gate_mode, which REFUSES a word it will not vouch for: falling through
    // to the event path instead is how the documented spelling used to fire a
    // notification about an empty event.
    if first == "gate" {
        std::process::exit(gate_mode(&second_argument()));
    }
    if first == "hook" {
        std::process::exit(hook_mode(&second_argument()));
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
