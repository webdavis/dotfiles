/// Everything this binary answers to, and the flags a producer states an event
/// with. Printed on request and on a refusal, which is why it is one text: an
/// operator who mistyped and an operator who asked have the same question.
pub const USAGE: &str = "\
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
  pns shell begin --pid <pid> --command <line>
  pns shell end --pid <pid> --command <line> --exit <code> --elapsed <secs>
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
                --local-only --remote-only --long-running --require-delivery
";
