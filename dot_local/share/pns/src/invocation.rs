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

producer flags: --agent <name> --state <word> --project <name> --branch <name>
                --detail <text> --pane <id> --channel <route>
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
pub(crate) fn event_mode(argv: &[String]) {
    let (event, warnings) = parse_args(argv.iter().cloned());
    // HELP WINS BEFORE ANYTHING ELSE ON THIS PATH: no config load, no probe.
    // It used to reach EVERYTHING when it fell through this same parser as an
    // unknown token, which notified about an empty event and raised a banner
    // titled "pns · done". Nothing about printing the commands needs the
    // machine read.
    if event.help {
        print!("{USAGE}");
        return;
    }
    for warning in &warnings {
        eprintln!("pns: {warning}");
    }
    // ARGV CARRIES NO PAYLOAD, which is the honest no-identity case.
    run_event(
        &event,
        &system_probes(),
        &HookPayload::default(),
        Attempt::First,
    );
}
