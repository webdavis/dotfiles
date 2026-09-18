/// Everything this binary answers to, and the flags a producer states an event
/// with. Printed on request and on a refusal, which is why it is one text: an
/// operator who mistyped and an operator who asked have the same question.
pub const USAGE: &str = "\
pns: usage:
  pns send [<producer flags>]      one notification, stated in argv
  pns send --json                  one notification, as a JSON request on stdin
  pns hook <event>                 a harness hook: prompt, stop, stop-failure,
                                   blocked, asked, denied, waiting, resolved,
                                   model-switch, quota, config-change
  pns gate <harness>-hook          presence-gated pass-through to moshi-hook
  pns <harness>-hook               the same gate, spelled the way moshi calls it
  pns pulse <exit-code>            signal the lamps by hand
  pns quiet [<duration>|off]       the operator's mute
  pns daemon run|schedule|cancel   the clock
  pns lights tick|quiet            the lamps' upkeep
  pns presence poll                one bridge read, published for the sensor
  pns github poll                  one notifications read, submitted as events
  pns github receive               the push receiver: a delivery polls now
  pns shell begin --pid <pid> --command <line>
  pns shell end --pid <pid> --command <line> --exit <code> --elapsed <secs>
  pns loop begin|end               take the loop lamp by hand, and give it back
  pns nag                          card every outstanding approval
  pns stale                        page about every session stuck past the window
  pns recap --since <epoch> --until <epoch>
  pns setup [--force]              write a first config, one question at a time
  pns doctor                       one test send through every channel
  pns tap [--info|--install] [--json]  record phone attention or inspect its setup
  pns home                         one reading of the router, said out loud
  pns --help, -h                   this text
  pns --version, -V                the package version

producer flags: --producer <name> --state <word> --project <name> --branch <name>
                --detail <text> --pane <id> --channel <route> --elapsed <secs>
                --kind <agent|health> --local-only --remote-only --long-running
                --require-delivery

kinds:          agent, the default, is a session event and takes the route
                `[routes] default` names; health is a machine's own health and
                takes `[routes] urgent` when its --state is one somebody has to
                answer, unless --channel already named one.
";
