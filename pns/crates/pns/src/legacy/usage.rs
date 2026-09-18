/// Every subcommand this binary answers to. Printed on request and on a
/// refusal, which is why it is one text: an operator who mistyped and an
/// operator who asked have the same question.
///
/// IT NAMES THE MACHINE-CALLED SUBCOMMANDS TOO. Hooks, launchd jobs, the
/// daemon and the shell notifier call some of these rather than a person
/// typing them, and a reader who cannot find one in the help concludes it does
/// not exist.
pub const USAGE: &str = "\
pns: usage:
  pns send [<producer flags>]      one notification, stated in argv
  pns send --json                  one notification, as a JSON request on stdin
  pns hook <event>                 a harness hook: prompt, stop, stop-failure,
                                   blocked, asked, denied, waiting, resolved,
                                   model-switch, quota, config-change
  pns <harness>-hook               presence-gated pass-through to moshi-hook,
                                   spelled the way moshi's extension calls it
  pns quiet [<duration>|off]       the operator's mute
  pns daemon run|schedule|cancel   the clock
  pns daemon retry                 one sweep of the retry queue, run by the clock
  pns lights tick                  the lamps' upkeep, run by the clock
  pns lights quiet                 the lamps' own mute, one place at a time
  pns lights pulse <exit-code>     signal the lamps by hand
  pns lights enroll                pair a bridge, once per machine
  pns presence poll [--daemon]     one bridge read, published for the sensor
  pns github poll [--daemon]       one notifications read, submitted as events
  pns github receive               the push receiver: a delivery polls now
  pns shell begin --pid <pid> --command <line>
  pns shell end --pid <pid> --command <line> --exit-code <code> --elapsed <duration>
  pns loop begin|end               take the loop lamp by hand, and give it back
  pns nag                          card every outstanding approval
  pns stale                        page about every session stuck past the window
  pns failures [<id>|open <id>]    what is not arriving, and one banner's click
  pns failures serve               the local page, run by the clock
  pns recap --since <epoch> --until <epoch>
  pns recap agent --stdin          post a recap somebody else composed
  pns recap git                    print what only git, worktrunk and gh answer
  pns setup [--force]              write a first config, one question at a time
  pns doctor [--raw]               one test send through every channel
  pns tap [info|install] [--json]  record phone attention or inspect its setup
  pns <subcommand> --help, -h      that subcommand's own usage
  pns --help, -h                   this text
  pns --version, -V                the package version

machine-called:  pns send, pns hook <event>, pns shell begin, pns shell end,
                 pns daemon retry, pns lights tick, pns nag, pns stale,
                 pns failures serve, pns recap --since, pns recap agent,
                 pns recap git, pns presence poll [--daemon] and
                 pns github poll [--daemon] are called by hooks, by launchd, by
                 the clock and by the shell notifier rather than typed.
";

/// What `pns send` takes, which is the one subcommand a producer states an
/// event with.
pub const SEND_USAGE: &str = "\
pns: usage:
  pns send [<producer flags>]      one notification, stated in argv
  pns send --json                  one notification, as a JSON request on stdin

producer flags: --producer <name> --state <word> --project <name> --branch <name>
                --detail <text> --pane <id> --route <name> --elapsed <duration>
                --request-id <id> --session <id> --kind <agent|health>
                --scope <automatic|local_only|remote_only> --long-running
                --require-delivery

durations:      a count and a unit, `30s`, `5m`, `2h`. A bare number is
                refused: one reader takes it as seconds and the next as
                minutes.

states:         done, failed, blocked, resolved, observation, progress. The
                same six words the JSON request's `state` takes; any other
                word is refused. observation and progress are quiet updates
                on both paths.

scopes:         automatic, the default, lets presence decide; local_only keeps
                the event on this machine; remote_only sends it off the machine
                alone.

kinds:          agent, the default, is a session event and takes the route
                `[routes] default` names; health is a machine's own health and
                takes `[routes] urgent` when its --state is one somebody has to
                answer, unless --route already named one.
";
