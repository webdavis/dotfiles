# The doctor: one census, one test send, and the read-only reports

`pns doctor` is the hand-typed diagnostic. It runs a **census** of the plugin roster, one **check** per
registration, and prints one line per check plus a summary of the **outcomes**; below that it prints six
read-only sections (moshi pairing, macOS Focus, the daemon, the nag, the lamps, the decision ring, and
the missed-notification journal). Two properties decide everything else. First, the census walks the
whole ROSTER and never the selection, so a plugin the config left off is visibly absent by choice rather
than silently missing (`src/doctor.rs` module header, `src/doctor.rs:checks`). Second, every check runs
whatever the ones before it found: a channel that fails, a channel that panics, a bridge that hangs and a
moshi-hook that never answers each cost their own line and nothing else. The policy lives in
`src/doctor.rs`, which is a total function of its arguments with no config, clock, environment, network
or printing; the composition root `src/main.rs:doctor_mode` reads the world, sends through the engine's
own wiring, and hands what came back to those functions to shape.

## LIVE EXTERNAL SIDE EFFECTS: read this before running or automating `pns doctor`

`pns doctor` is not a dry run. It is the one diagnostic that deliberately bypasses every suppression gate
(`src/main.rs:DOCTOR_OPENING`), so nothing in the configuration can make it quiet. On 2026-09-02 a
verification harness that ran it posted two real banners and drove the operator's lamps. The complete
list of effects a single invocation can cause outside this process:

1. **A real macOS banner.** The `macos-banner` leg spawns `terminal-notifier` with the doctor's title and
   preview (`src/channels/banner.rs:BannerChannel::deliver`, reached through `src/main.rs:deliver_leg`).
   The operator sees a notification on their screen.
1. **A real card on the operator's phone.** The `mobile` leg posts the event to the moshi webhook
   (`https://api.getmoshi.app/api/webhook` by default, `src/channels/moshi.rs:DEFAULT_MOSHI_URL`,
   overridable with `PNS_MOSHI_URL`).
1. **A real signed post to the hermes gateway.** The `hermes` leg sends a signed Hypertext Transfer
   Protocol (HTTP) request to the configured route (`src/channels/hermes.rs:HermesChannel::deliver`,
   `src/main.rs:hermes_url_for`), which lands in the operator's durable log route (Discord in practice).
1. **Real lamp writes.** The `hue` check is a `Pulse`, and `src/main.rs:pulse_outcome` calls
   `src/main.rs:fire_pulse`, which issues one `PUT` per addressed room to the Hue bridge
   (`src/channels/hue.rs:HuePulse::run`, `src/channels/hue.rs:signal_fixtures`). The operator's lamps
   flash. It runs inside the lights' **quiet hours** and inside the **quiet window** deliberately, pinned
   by `tests/dispatch.rs:the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`.
1. **Executable channels are spawned.** When `PNS_CHANNELS_DIR` is set, native plugins lose and the
   doctor executes `<dir>/<plugin>.sh` with the event on standard input
   (`src/channels/mod.rs:native_first`, `src/main.rs:deliver`). Whatever those scripts do, the doctor
   does.
1. **Three read-only `GET` calls to the Hue bridge**, when and only when a `[lights]` table exists, hue
   is enabled, and a bridge and key are named (`src/main.rs:lights_report`,
   `src/channels/hue.rs:resolve_on_bridge`). These resolve the lamp map. They write nothing.
1. **Two spawns of `moshi-hook`.** `moshi-hook status --json` is local-only (measured at 77ms with the
   base URL pointed at an unroutable host, `src/main.rs:read_pairing`); plain `moshi-hook status`
   contacts the moshi API and is the only network call the doctor makes on its own behalf.
1. **File reads only, everywhere else.** The config, the macOS Focus store, the daemon heartbeat, the
   daemon spool, the decision ring and the missed-notification journal are read and never written.

The doctor writes NOTHING to its own state directory, appends NOTHING to the decision ring, and journals
NOTHING (behavior 27). Everything above except items 6, 7 and 8 is observable by the operator as a
notification or a lamp. **A test harness must never invoke `pns doctor` against a real configuration.**
The suite's own pattern is `tests/dispatch.rs:doctor_command`, which points `PNS_STATE_DIR` into a
sandbox and `MOSHI_HOOK_BIN` at a path that does not exist, and `tests/dispatch.rs:no_moshi_hook`, whose
doc comment states the reason in full: without it "the suite reads the developer's own machine".

## The checks

Every row's outcome wording is quoted exactly as `src/doctor.rs:line` renders it, with `<plugin>`
standing for the plugin's config-table name.

| Check                       | What it exercises                                                                                                                                          | Live side effect                                                                                    | Deadline                                                                                                                                                                                                                           | Possible outcomes                                                                                                                               | Exact wording                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Tests                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `router`                    | Nothing. It is a `PluginKind::Sensor`, and no leg can route to a sensor (`src/doctor.rs:kind_of`).                                                         | **None.**                                                                                           | Not applicable.                                                                                                                                                                                                                    | `Skipped` only.                                                                                                                                 | `router: skipped, a sensor and never a delivery destination`                                                                                                                                                                                                                                                                                                                                                                                                     | `src/doctor.rs:a_selected_sensor_is_a_skip_because_no_leg_can_ever_reach_one`, `src/doctor.rs:a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel`, `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`                                                                                                                                                                            |
| `mobile`                    | One real push of the doctor event to the moshi webhook.                                                                                                    | **YES: a card on the operator's phone.**                                                            | `src/channels/moshi.rs:POST_DEADLINE`, 10 seconds.                                                                                                                                                                                 | `Sent`, `SentUnreported`, `Failed`, `Skipped`.                                                                                                  | `mobile: sent, <what the channel said>` / `mobile: sent, this channel reports no outcome` / `mobile: FAILED, push SKIPPED -- no moshi token in the config ([plugins.mobile] token); nothing was sent` / `mobile: skipped, <reason>`                                                                                                                                                                                                                              | `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`, `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`, `tests/dispatch.rs:the_doctor_names_the_type_when_the_type_is_the_fault_and_never_the_token`                                                                                                                                                   |
| `macos-banner`              | One real banner through `terminal-notifier`.                                                                                                               | **YES: a notification on the operator's screen.**                                                   | `src/system.rs:PROBE_DEADLINE`, 5 seconds (the runner's bounded spawn).                                                                                                                                                            | `Sent`, `SentUnreported`, `Failed`, `Skipped`.                                                                                                  | `macos-banner: sent, posted the banner` / `macos-banner: FAILED, banner FAILED (terminal-notifier did not run)` / `macos-banner: sent, this channel reports no outcome`                                                                                                                                                                                                                                                                                          | `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one` (asserts `macos-banner: sent, posted the banner` and that the notifier really received its payload)                                                                                                                                                                                                                                                      |
| `hermes`                    | One signed HTTP post to the gateway route.                                                                                                                 | **YES: an entry in the operator's durable log route.**                                              | `src/channels/hermes.rs:remote_deadline`, 5 seconds by default (`DEFAULT_SYNC_DEADLINE_SECS`), overridable with `PNS_REMOTE_TIMEOUT`, capped at `MAX_SYNC_DEADLINE_SECS` (86,400 seconds); the value `0` means no deadline at all. | `Sent`, `SentUnreported`, `Failed`, `Skipped`.                                                                                                  | `hermes: sent, <the post's own sentence>` / `hermes: FAILED, post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent`                                                                                                                                                                                                                                                                                                               | `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`, `src/doctor.rs:a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel`                                                                                                                                                                                                                                                                |
| `hue` (pulse)               | One `PUT` per addressed room, `Behaviour::Done`. No event dispatches to hue, so it is checked as a `Pulse` rather than a `Send` (`src/doctor.rs:kind_of`). | **YES: the operator's lamps flash**, quiet hours and quiet window included.                         | `src/channels/hue.rs:BRIDGE_DEADLINE`, 10 seconds per call (one `GET` of the room listing plus one `PUT` per room).                                                                                                                | `Signalled(n)` for n above zero, `Signalled(0)` (graded a failure), `Failed` for a config that resolves to no bridge or for a panic, `Skipped`. | `hue: signalled 1 room (watch for the flash; the bridge acknowledges no write)` / `hue: signalled 2 rooms (watch for the flash; the bridge acknowledges no write)` / `hue: FAILED, signalled no rooms (no room listing from the bridge, or no configured room name matched)` / `hue: FAILED, pulse SKIPPED -- no hue bridge and key in the config ([plugins.hue] bridge, key); nothing was signalled` / `hue: FAILED, the pulse PANICKED; no room was signalled` | `src/doctor.rs:the_pulse_line_claims_neither_a_flash_nor_a_cause_it_cannot_know`, `tests/dispatch.rs:a_pulse_with_no_bridge_to_dial_names_the_settings_rather_than_the_rooms`, `tests/dispatch.rs:a_pulse_the_bridge_answered_nothing_for_still_names_both_causes_it_cannot_choose_between`, `tests/dispatch.rs:the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`                                                                               |
| moshi pairing, local leg    | `moshi-hook status --json`, which carries the pairing fact pns grades.                                                                                     | Spawns another program. No network today (measured local-only).                                     | `src/main.rs:MOSHI_JSON_DEADLINE`, 5 seconds, overridable with `PNS_MOSHI_JSON_DEADLINE_MS`.                                                                                                                                       | `Paired`, `Unpaired`, `Unreadable`, `NoAnswer`.                                                                                                 | See behavior 15 for all four sentences.                                                                                                                                                                                                                                                                                                                                                                                                                          | `tests/dispatch.rs:the_doctor_runs_moshi_hook_exactly_twice_and_never_probes`, `tests/dispatch.rs:a_moshi_hook_that_never_returns_does_not_park_the_doctor`                                                                                                                                                                                                                                                                                                 |
| moshi pairing, relay leg    | Plain `moshi-hook status`, the only shape carrying a server verdict.                                                                                       | **YES: a call to the moshi API.** This is the only network call the doctor makes for its own sake.  | `src/main.rs:MOSHI_STATUS_DEADLINE`, 8 seconds, overridable with `PNS_MOSHI_STATUS_DEADLINE_MS`. It must exceed moshi's own internal timeout, measured at about 5.1 seconds against an unroutable base URL.                        | A relayed sentence, or nothing.                                                                                                                 | `pns doctor: moshi says: <moshi's own sentence>`                                                                                                                                                                                                                                                                                                                                                                                                                 | `tests/dispatch.rs:the_doctor_runs_moshi_hook_exactly_twice_and_never_probes`, `src/doctor.rs:the_server_line_is_relayed_as_moshis_own_words_with_the_label_removed`, `src/doctor.rs:only_a_server_line_at_column_zero_is_relayed`                                                                                                                                                                                                                          |
| macOS Focus                 | Reads `~/Library/DoNotDisturb/DB/Assertions.json` and `ModeConfigurations.json` when, and only when, `[focus] silence` names something.                    | **None.** Read-only, and nothing is opened at all when the list is empty (`src/main.rs:focus_now`). | Not applicable. Bounded by size at `src/main.rs:RING_READ_MAX`, 262,144 bytes.                                                                                                                                                     | Off, on, quiet, unreadable, absent, plus a catalog clause.                                                                                      | See behavior 19.                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `tests/dispatch.rs:the_doctor_tells_the_truth_about_a_named_focus_in_every_state`, `tests/dispatch.rs:a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health`                                                                                                                                                                                                                                                                           |
| daemon                      | Reads the heartbeat file and counts regular files in the spool.                                                                                            | **None.**                                                                                           | Not applicable.                                                                                                                                                                                                                    | Five states.                                                                                                                                    | See behavior 20.                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `src/doctor.rs:the_daemons_doctor_line_tells_the_truth_in_four_states`, `src/doctor.rs:a_daemon_switched_off_but_still_beating_is_reported_as_still_beating`, `src/doctor.rs:a_heartbeat_whose_age_cannot_be_taken_reads_as_not_running`                                                                                                                                                                                                                    |
| nag                         | The config's `[nag] after_secs` alone.                                                                                                                     | **None.** No file is read for it beyond the one config read the doctor already took.                | Not applicable.                                                                                                                                                                                                                    | Two states.                                                                                                                                     | `` pns doctor: the nag is off (no `[nag] after_secs`) `` / `pns doctor: an unanswered approval is carded again after <duration>`                                                                                                                                                                                                                                                                                                                                 | `src/doctor.rs:the_nag_line_names_the_schedule_or_says_the_feature_is_off`                                                                                                                                                                                                                                                                                                                                                                                  |
| lights                      | Resolves the lamp map on the bridge: three `GET` calls (`room`, `light`, `zone`).                                                                          | Three read-only bridge calls. **No lamp changes state.**                                            | `src/channels/hue.rs:BRIDGE_DEADLINE`, 10 seconds per call, so up to 30 seconds total against a bridge that accepts and never answers.                                                                                             | Six states.                                                                                                                                     | See behavior 22.                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `src/doctor.rs:the_lights_section_says_which_of_its_six_states_the_config_is_in`, `src/doctor.rs:an_unresolved_name_and_a_refused_declaration_each_get_their_own_line`, `src/doctor.rs:every_lights_state_says_something_rather_than_printing_nothing`                                                                                                                                                                                                      |
| decision ring               | Reads `<state>/decisions` and renders the last five entries, newest first.                                                                                 | **None.** Never appended to.                                                                        | Not applicable. Bounded by size at 262,144 bytes.                                                                                                                                                                                  | Rendered, absent, unreadable.                                                                                                                   | See behavior 23.                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `tests/dispatch.rs:the_doctor_prints_the_decision_section_after_its_summary_newest_first`, `tests/dispatch.rs:the_doctors_exit_code_does_not_move_for_a_log_that_is_absent_or_unreadable`, `tests/dispatch.rs:a_ring_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`, `tests/dispatch.rs:a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind`, `tests/dispatch.rs:the_doctor_records_no_decision_of_its_own` |
| missed-notification journal | Counts the lines of `<state>/missed-notifications`.                                                                                                        | **None.** Never appended to, and nothing in it is parsed.                                           | Not applicable. Bounded by size at 262,144 bytes.                                                                                                                                                                                  | A count, absent, unreadable.                                                                                                                    | See behavior 24.                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `tests/dispatch.rs:the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it`, `tests/dispatch.rs:a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`, `tests/dispatch.rs:a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`, `tests/dispatch.rs:the_doctor_leaves_the_journal_exactly_as_it_found_it`                                                                              |

______________________________________________________________________

### 1. A doctor given any extra word is refused before anything runs

Given the doctor takes one word and no flags (`src/main.rs:DOCTOR_USAGE`)

When `std::env::args_os().nth(2)` is anything at all, the empty string included

Then usage goes to standard error, the process exits 2, and nothing is printed, sent, or spawned.

- Success: `pns doctor` with no further argument proceeds.
- Failure sources: `extra`, `send`, `--dry-run`, `""`, `send hermes`, all pinned by
  `tests/dispatch.rs:a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel`.
- Fail direction: fail-closed and loud. The house rule is that an unknown argument never falls through to
  help with exit 0, and the comment at `src/main.rs:doctor_mode` states why for this command in
  particular: "A doctor that quietly ignored an argument is a check the operator believes was narrower or
  wider than it was."
- Thresholds: Not applicable, this is an argument-count predicate.
- Required side effects: one line on standard error, exit code 2.
- Forbidden side effects: standard output is empty, no channel receives a payload, and `moshi-hook` is
  never spawned. The test stubs a RECORDING `moshi-hook` precisely so "reaches no channel" covers the
  spawn as well, and asserts the recorded argv list is empty.
- Timeout and cancellation: Not applicable, the refusal is the first statement in the function.
- Idempotency and duplicates: Not applicable, nothing is mutated.
- Privacy: the refused word is not echoed. The usage line is a fixed literal.
- Process ownership and cleanup: no child is created.
- Compatibility contract: the exact line on standard error is `pns: usage: pns doctor`
  (`src/main.rs:DOCTOR_USAGE`). Its doc comment states the reason for one bare word: "a namespace built
  for callers that do not exist makes the common case longer to type, and the report absorbs a new
  section without a new spelling."

### 2. The opening line states the bypass contract rather than measuring it

Given the doctor is worth nothing if it can be suppressed

When it starts, before the config is read

Then it prints one fixed sentence naming every gate it bypasses.

- Success: `src/main.rs:DOCTOR_OPENING` is the first line of standard output, pinned as `printed[0]` by
  `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`.
- Failure sources: none. The line is a constant and no reading can change it.
- Fail direction: Not applicable.
- Thresholds: Not applicable.
- Required side effects: one line on standard output before anything is sent.
- Forbidden side effects: it must not report LIVE gate state. The doc comment at
  `src/main.rs:DOCTOR_OPENING` says so: "Whether a gate is currently in effect is the decision log's
  question, and reporting live gate state here would be that feature built twice, in two places, from two
  readings."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: printed exactly once per run.
- Privacy: Not applicable, the sentence carries no reading.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, quoted verbatim:
  `pns doctor: sending one test to every enabled channel. Every suppression gate is bypassed (the operator mute, a macOS Focus you named, the presence gate, the viewed-pane rule, the lights' quiet hours), because a check that can be suppressed proves nothing.`

### 3. The census is the whole roster, in registration order

Given the operator asks "what will reach me", not "what is on"

When `src/doctor.rs:checks` is handed the registered roster, the selection, and the config state

Then it returns one `Check` per REGISTRATION, in registration order, whatever the selection holds.

- Success: with nothing enabled at all, the returned plugin names still equal `registry.names()`, pinned
  by `src/doctor.rs:the_check_list_holds_one_entry_per_registration_in_registration_order` with the
  message "a report cannot silently omit a plugin". End to end,
  `tests/dispatch.rs:a_config_that_enables_nothing_names_every_plugin_sends_nothing_and_exits_one`
  asserts all five lines are still printed.
- Failure sources: a census walking the selection would return an empty report on a config that enables
  nothing, losing every plugin at once. That is the defect this shape exists to prevent.
- Fail direction: toward saying more, never less. An absent line is unreadable as anything but health.
- Thresholds: the roster is exactly five registrations (`src/registry.rs:ROSTER`) in the order `router`,
  `mobile`, `macos-banner`, `hermes`, `hue`. Registration order is delivery order for the channels.
- Required side effects: one printed line per check (behavior 12).
- Forbidden side effects: none of these functions read config, clock, environment or network
  (`src/doctor.rs` module header).
- Timeout and cancellation: Not applicable, pure.
- Idempotency and duplicates: pure, so repeated calls give the same list.
- Privacy: only plugin names, which are the config's own table names.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: registration order is the printed order, and the operator reads the report
  against their config file top to bottom.

### 4. What each registration is checked as

Given a registration's kind and whether the selection chose it

When `src/doctor.rs:kind_of` decides

Then not-selected is asked FIRST, then a sensor is `Skipped`, then a channel no event dispatches to is a
`Pulse`, and every other channel is a `Send`.

- Success: `[plugins.router] enabled = true` gives
  `Skipped("a sensor and never a delivery destination")`; `[plugins.hue] enabled = true` gives `Pulse`;
  `mobile`, `macos-banner` and `hermes` each give `Send`. Pinned by
  `src/doctor.rs:a_selected_sensor_is_a_skip_because_no_leg_can_ever_reach_one`,
  `src/doctor.rs:a_selected_channel_no_event_dispatches_is_a_pulse_rather_than_a_send`, and
  `src/doctor.rs:a_selected_event_dispatched_channel_is_a_send`.
- Failure sources: asking the kind before asking the selection would report a sensor the config never
  switched on as "a sensor" rather than as absent by choice. The order in `kind_of` is what prevents it,
  and its comment says so.
- Fail direction: toward the narrower, more informative sentence.
- Thresholds: Not applicable.
- Required side effects: none, this is a pure classification.
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `hue` is registered with `event_dispatched: false` (`src/registry.rs:ROSTER`),
  which is what makes it a `Pulse`. The comment there states the intent: it registers so the config can
  select it and so a typo in its name is still refused, but no event ever routes to it.

### 5. A plugin the selection left out is skipped in words true of THIS machine

Given three different config states are three different edits for the operator

When `src/main.rs:doctor_mode` classifies the load outcome into `ConfigState` and passes it to `checks`

Then a skipped plugin's reason is one of exactly three distinct sentences.

- Success: `ConfigState::Read` gives `not enabled in the config`; `ConfigState::Absent` gives
  `no config file, so only the core runs`; `ConfigState::Unreadable` gives
  `the config could not be read, so only the core runs`. Pinned by
  `src/doctor.rs:a_plugin_the_selection_left_out_is_skipped_in_words_true_of_this_machine`, which also
  asserts the three constants are three DISTINCT strings so one accidentally pointed at another cannot
  pass. End to end, `tests/dispatch.rs:the_doctor_tells_a_machine_with_no_config_that_there_is_no_config`
  asserts the absent wording for `router`, `hermes` and `hue`.
- Failure sources: both ways a config declines a plugin (never naming it, and naming it switched off)
  land on `NOT_ENABLED`, pinned by
  `src/doctor.rs:a_registered_plugin_the_config_did_not_enable_is_a_skip_that_says_which`.
- Fail direction: toward the sentence the operator can act on. `src/doctor.rs:ConfigState`'s doc records
  the defect this fixed: "not enabled in the config" became the ORDINARY report on a fresh machine once
  the fallback narrowed to the core, pointing the operator at a file that does not exist.
- Thresholds: Not applicable.
- Required side effects: the state is taken from `loaded` BEFORE `select_plugins` consumes it
  (`src/main.rs:doctor_mode`).
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure given the state.
- Privacy: the config's contents are never quoted, only the fact of it.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, verbatim: `<plugin>: skipped, not enabled in the config`,
  `<plugin>: skipped, no config file, so only the core runs`,
  `<plugin>: skipped, the config could not be read, so only the core runs`.

### 6. Config faults are said on standard error and grade nothing

Given the doctor is where a misconfiguration is meant to become visible

When the config names an unknown or duplicate plugin, or cannot be read, or holds a switched-off table
whose `type` names no compiled-in backend

Then the complaint goes to standard ERROR, the census on standard output is unaffected, and the exit code
does not move for it.

- Success:
  `tests/dispatch.rs:the_doctor_says_a_switched_off_table_names_no_backend_and_an_event_never_does`
  asserts the doctor's standard error names `[plugins.router]` and `switched off`, and that the same
  config produces no such line on the event path.
- Failure sources: `src/main.rs:disabled_backend_warnings` emits one line per switched-off `router` or
  `mobile` table whose type is unrecognized; `src/registry.rs:select_plugins` returns
  `pns: config error (unknown plugin `<name>`); running every built-in plugin` for a parsed config with a
  bad table name, and `pns: config error (<detail>); running the core plugins (mobile, macos-banner)` for
  one that could not be read.
- Fail direction: loud but inert. `src/main.rs:disabled_backend_warning`'s doc states the rule: "It moves
  no exit code, which is the same rule the Focus and daemon lines keep: a switch nobody flipped is not a
  broken notifier."
- Thresholds: Not applicable.
- Required side effects: the warnings are printed BEFORE the census lines (`src/main.rs:doctor_mode`
  prints them right after the config read).
- Forbidden side effects: they never appear on standard output, which is one line per registered plugin
  plus the sections.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: one fault, one line.
  `tests/dispatch.rs:a_mobile_table_naming_no_compiled_in_backend_pushes_no_card_through_either_seam`
  asserts `said.matches("no card is pushed").count() == 1` on the event path.
- Privacy: the type value is quoted (`"pushover"`); the token is never quoted. Pinned by
  `tests/dispatch.rs:the_doctor_names_the_type_when_the_type_is_the_fault_and_never_the_token`, which
  asserts the mobile line contains `"pushover"` and `type` and does NOT contain `token`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, verbatim:
  `pns: [plugins.<table>] is switched off and names no backend this binary answers (the only type is <type>); nothing refuses it until it is enabled`.

### 7. The test event: one payload, no pane, reporting mode

Given the operator is standing at the terminal waiting for the answer

When the doctor builds the event it will send

Then it is `agent = "pns"`, `state = "doctor"`, the fixed detail sentence, no pane, and every leg runs in
`ReportMode::ReportOutcome` marked `decorative: false`.

- Success:
  `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`
  asserts, for each of `mobile`, `macos-banner` and `hermes`, that the delivered event carries
  `agent = "pns"`, `state = "doctor"`, `title = "pns · doctor"`, `pane = ""` and `mode = "sync"`.
- Failure sources: Not applicable, the event is constructed from constants.
- Fail direction: Not applicable.
- Thresholds: Not applicable.
- Required side effects: the detail must say at once that nothing is wrong, because it wakes whoever
  receives the card.
- Forbidden side effects: no pane is carried. `src/main.rs:doctor_mode` states the reason: the pane's
  only consumer is the banner's click target, and whether a click focuses the right pane cannot be
  verified without a human clicking it, so carrying one would add the scrub rule to a second call site to
  test nothing this can observe. The same test asserts standard error does not contain
  `dropped a pane id`.
- Timeout and cancellation: per channel, from the deadlines in the check table.
- Idempotency and duplicates: one event, one dispatch, one outcome per leg.
- Privacy: the payload carries no session identifier, no project, no branch and no transcript.
- Process ownership and cleanup: executable channels are spawned and waited on (`src/main.rs:deliver`).
- Compatibility contract, verbatim (`src/main.rs:DOCTOR_DETAIL`):
  `test send from pns doctor; nothing is wrong and nothing needs doing`. The `decorative: false` flag is
  deliberate: no plan chose these legs, so the honest answer to "is this leg here because the operator
  was to be shown something" is no.

### 8. Every gate is bypassed structurally, because no decision is taken

Given a check that can be suppressed proves nothing

When the doctor dispatches

Then it calls `dispatch_legs` directly and never `decide`, so the operator mute, the presence gate and
both phone overrides cannot reach it.

- Success:
  `tests/dispatch.rs:the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`
  plants a live mute in the state directory, sets `PNS_IDLE_SECS=0` (at the desk), `PNS_SKIP_PHONE=1` and
  `PNS_FORCE_PHONE=1`, and asserts every one of `mobile`, `macos-banner` and `hermes` still fired, with
  exit 0.
- Failure sources: none reachable. There is no code path from a gate to the doctor's leg list.
- Fail direction: toward sending. The doctor over-delivers by design.
- Thresholds: Not applicable.
- Required side effects: the live effects in the section above. This is the behavior that makes them
  unavoidable.
- Forbidden side effects: the mute file is read by nothing on this path and is left untouched.
- Timeout and cancellation: per channel.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: the same test asserts the state directory afterwards holds exactly
  `["quiet-until"]`, the file it planted.
- Compatibility contract: the viewed-pane rule is NOT among the bypasses this run can observe, and the
  test's own comment says why: `decide` is never called on this path, so no pane verdict exists to
  bypass. The opening line still names it, because the line states the CONTRACT (behavior 2).

### 9. A send's outcome is the channel's own verdict, matched by name

Given `dispatch_legs` answers one `Delivery` per leg

When the doctor turns those into outcomes

Then it looks each leg up BY NAME, and maps `Delivered` to `Sent`, `Failed` and `Unlaunched` to `Failed`,
and `Silent` to `SentUnreported`.

- Success: an executable channel that ran and said nothing gives
  `<plugin>: sent, this channel reports no outcome`, pinned end to end by
  `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`.
  A native run gives the channel's own sentence, pinned by
  `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`.
- Failure sources: `Unlaunched` is the one that matters most.
  `tests/dispatch.rs:a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`
  records the measured defect: an empty channels directory once reported "3 sent, 0 failed" and exited 0.
  The test now asserts all three channels report `<channel>: FAILED, could not launch the channel at` and
  the summary reads `pns doctor: 0 sent, 3 failed, 2 skipped`.
- Fail direction: a leg that is somehow absent from the delivery list reports
  `<plugin>: FAILED, the leg was never dispatched` rather than claiming a send. The comment at
  `src/main.rs:doctor_mode` says the case cannot happen and still reports a problem, "which is the
  direction to be wrong in".
- Thresholds: Not applicable.
- Required side effects: the send itself.
- Forbidden side effects: a POSITIONAL pairing is forbidden. The comment states the harm: it "would print
  one channel's verdict under another's label, which is a silent misreport rather than a visible one".
- Timeout and cancellation: per channel, from the check table.
- Idempotency and duplicates: one leg per `Send` check, one outcome per leg.
- Privacy: the channel's sentence is quoted verbatim. The channels themselves are responsible for not
  putting a secret in one; `src/channels/moshi.rs` names the config KEY rather than the token value.
- Process ownership and cleanup: `src/main.rs:deliver` writes the event to the child's standard input,
  closes it, and waits; the child's exit status is deliberately dropped, because a channel declining is
  its own business.
- Compatibility contract: `src/channels/mod.rs:Delivery`'s doc states the rule callers depend on: "THE
  VERDICT IS THE VARIANT, never a word inside the sentence." A caller that had to find `FAILED` in the
  text would be a predicate keyed on English.

### 10. One channel's failure or panic costs no sibling its turn

Given a census that ended early is read as a report that finished

When one leg fails, or the channel behind it panics

Then every remaining leg still runs, and the panicking one becomes a `Failed` outcome with no panic text
quoted.

- Success:
  `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`
  fails the FIRST dispatched channel (`mobile`, no token), asserts the banner behind it delivered AND was
  handed its payload, asserts `hermes` at the tail still got its turn and failed with its own sentence,
  and asserts the summary reads `pns doctor: 1 sent, 2 failed, 2 skipped`.
- Failure sources: an unwinding channel. `src/main.rs:dispatch_legs` wraps each `deliver_leg` in
  `catch_unwind`; `src/main.rs:pulse_outcome` and `src/main.rs:lights_report` each do the same for the
  bridge.
- Fail direction: one leg's failure, never the run's. The comment at `dispatch_legs` names the harm:
  without the catch, an unwinding channel takes the remaining legs and, in a hand-run check, the rest of
  the census with it.
- Thresholds: Not applicable.
- Required side effects: none beyond the sends that did happen. The default panic hook still prints its
  own trace to standard error, which is left alone deliberately: silencing it process-wide would hide
  every other panic in the binary.
- Forbidden side effects: NO PANIC TEXT is quoted, in any of the three catch sites. The reason is stated
  identically at each: a panic message is written for a developer and may quote anything the channel was
  holding.
- Timeout and cancellation: per channel.
- Idempotency and duplicates: a leg that panicked sent nothing that the doctor can know about.
- Privacy: this is the privacy rule, see forbidden side effects.
- Process ownership and cleanup: Not applicable, the panic is caught in-process.
- Compatibility contract, verbatim: `the <name> channel PANICKED; nothing was sent` (from
  `dispatch_legs`), and `the pulse PANICKED; no room was signalled` (from `pulse_outcome`).
- NOT ESTABLISHED: no test drives a real panic through `dispatch_legs`, `pulse_outcome` or
  `lights_report`. I grepped `tests/dispatch.rs` and `src/main.rs`'s test module for `PANICKED` and found
  only the source strings.

### 11. The pulse is a live write, and a config with no bridge is never dialled

Given `fire_pulse` answers zero rooms both for a bridge that listed none and for a hue table that names
no bridge at all

When the doctor reaches the `hue` check

Then it asks `src/main.rs:hue_resolves` FIRST, and only dials when the settings resolve.

- Success: a config with `bridge` and `key` is dialled and the count is reported, pinned by
  `tests/dispatch.rs:a_pulse_the_bridge_answered_nothing_for_still_names_both_causes_it_cannot_choose_between`,
  which uses a listening socket to prove the bridge really was contacted.
- Failure sources: a `[plugins.hue]` table with a `bridge` and no `key` (or vice versa) resolves to
  `None` (`src/channels/hue.rs:hue_settings` requires both as non-empty strings), so the check answers
  `Failed(NO_HUE_BRIDGE_LINE)` and nothing is dialled.
  `tests/dispatch.rs:a_pulse_with_no_bridge_to_dial_names_the_settings_rather_than_the_rooms` asserts the
  exact line and asserts the spy socket was NOT dialled.
- Fail direction: toward the sentence that names the edit. The comment at `src/main.rs:doctor_mode` says
  the zero-rooms line would blame the listing or the room names, "both wrong here, and both send the
  operator hunting through a bridge nothing contacted".
- Thresholds: `Signalled(0)` is graded a FAILURE by `src/doctor.rs:verdict`; `Signalled(1)` and above are
  `Sent`. One room either side of zero is the whole distinction. The pulse addresses rooms from
  `[plugins.hue] rooms`, or `HUE_PULSE_ROOMS` when that environment variable is set and non-empty.
- Required side effects: **the lamps flash.** `Behaviour::Done` is sent as an `on_off_color` signal for
  `src/channels/hue.rs:UNMAPPED_SIGNAL_DURATION_MS`, 3000 milliseconds, with no brightness stated.
- Forbidden side effects: no brightness is written on this path, so the lamp comes back byte-identical
  (measured on a real lamp in both directions, `src/channels/hue.rs:pulse_body` doc). The bridge
  acknowledges no write, so the count is the only observable fact.
- Timeout and cancellation: `BRIDGE_DEADLINE`, 10 seconds per call.
- Idempotency and duplicates: one pulse per doctor run. It is not idempotent in the world: two runs are
  two flashes.
- Privacy: the bridge key never reaches a printed line.
- Process ownership and cleanup: no child process; this is an in-process HTTP call.
- Compatibility contract, verbatim (`src/main.rs:NO_HUE_BRIDGE_LINE`, rendered through
  `src/doctor.rs:line`):
  `hue: FAILED, pulse SKIPPED -- no hue bridge and key in the config ([plugins.hue] bridge, key); nothing was signalled`.
  The doc comment explains the naming: it names the settings to write, the way moshi's and hermes's do,
  because "no rooms" without an address sends the operator to a bridge nothing dialled.

### 12. One line per check, in the outcome's own words

Given the operator reads the report top to bottom against their config

When `src/doctor.rs:line` renders a check and its outcome

Then it emits exactly one line, labelled with the plugin's config-table name, quoting the channel's own
sentence unchanged.

- Success: all six shapes are pinned by
  `src/doctor.rs:a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel` and
  `src/doctor.rs:the_pulse_line_claims_neither_a_flash_nor_a_cause_it_cannot_know`.
- Failure sources: Not applicable, `line` is total over the five `Outcome` variants.
- Fail direction: toward claiming less. `Signalled(0)` names BOTH possible causes rather than choosing
  one, and no positive count claims the lamps actually flashed.
- Thresholds: `Signalled(1)` is singular ("1 room"), every other non-zero count plural ("n rooms").
- Required side effects: one line on standard output per check.
- Forbidden side effects: no paraphrase. The test's own message states it: "the channel's own sentence,
  verbatim: a doctor that paraphrased would be a second wording of one answer".
- Timeout and cancellation: Not applicable, pure.
- Idempotency and duplicates: pure.
- Privacy: whatever the channel put in its sentence. See behavior 9.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, the six forms verbatim:
  - `<plugin>: sent, <said>`
  - `<plugin>: sent, this channel reports no outcome`
  - `<plugin>: FAILED, <said>`
  - `<plugin>: FAILED, signalled no rooms (no room listing from the bridge, or no configured room name matched)`
  - `<plugin>: signalled 1 room (watch for the flash; the bridge acknowledges no write)` and
    `<plugin>: signalled <n> rooms (watch for the flash; the bridge acknowledges no write)`
  - `<plugin>: skipped, <reason>`

### 13. The summary counts every check exactly once

Given the summary's counts and the exit code must never read one run differently

When `src/doctor.rs:summary` counts

Then every outcome falls into exactly one of `Sent`, `Failed`, `Skipped`, decided once by
`src/doctor.rs:verdict`.

- Success: `src/doctor.rs:the_summary_counts_every_check_exactly_once` builds seven outcomes covering
  every variant, asserts the rendered line is `pns doctor: 3 sent, 2 failed, 2 skipped`, and then parses
  the three numbers back out of the line and asserts they sum to the number of outcomes, with the message
  "a check that fell into no bucket is a plugin the summary lost".
- Failure sources: a new `Outcome` variant not handled in `verdict` would fail to compile, since the
  match is exhaustive.
- Fail direction: `Signalled(0)` counts as FAILED, not as sent, because a pulse that reached no room
  reached nothing.
- Thresholds: `Signalled(0)` versus `Signalled(1)` is the one boundary in the bucketing.
- Required side effects: one line on standard output after the per-check lines.
- Forbidden side effects: `verdict` is private and there is exactly one of it, so the summary and the
  exit code cannot disagree.
- Timeout and cancellation: Not applicable, pure.
- Idempotency and duplicates: pure.
- Privacy: numbers only.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, verbatim: `pns doctor: <n> sent, <n> failed, <n> skipped`. The summary counts
  SENDS, which is why an unpaired host can exit 1 while the summary still reads "0 failed" (behavior 25).

### 14. The pairing check spawns `moshi-hook` twice and never probes

Given `status --json` is local-only and carries the fact pns grades, while plain `status` is the only
shape carrying a server verdict

When `src/main.rs:read_pairing` runs

Then it makes exactly two bounded spawns, the local one first, and never calls `probe`.

- Success: `tests/dispatch.rs:the_doctor_runs_moshi_hook_exactly_twice_and_never_probes` asserts the
  recorded argv is exactly `[["status", "--json"], ["status"]]` and that no argument anywhere was
  `probe`. The stub records argument boundaries with a unit separator precisely so a single argument
  `status --json` could not masquerade as the two real ones.
- Failure sources: the binary is resolved through `MOSHI_HOOK_BIN`, falling back to
  `/opt/homebrew/bin/moshi-hook` (`src/main.rs:moshi_hook_bin`, `src/main.rs:DEFAULT_MOSHI_HOOK_BIN`). An
  absent binary, a hang, and a non-zero exit are indistinguishable to `src/system.rs:run_bounded`, which
  answers `None` for all three.
- Fail direction: no answer is its own state and nothing guesses past it. A machine that does not use
  moshi exits 0 (behavior 25).
- Thresholds: 5 seconds for the local leg, 8 seconds for the relay leg, so a moshi-hook wedged on both
  puts 13 seconds on a hand-typed command (measured at 13.07 seconds, `src/main.rs:read_pairing` doc).
  Ten seconds is explicitly NOT the bound.
- Required side effects: two child processes; one call to the moshi API on the relay leg.
- Forbidden side effects: `probe` is never called. `src/main.rs:read_pairing` states the reason: measured
  on 0.3.3, probe answers `running: true` and `gateway: true` against a HOME holding no pairing at all
  while its host identifier disappears, so its daemon-side provenance cannot be stated honestly. Also
  forbidden: reading the `hooks` key off the JSON answer, because on this machine it reports the claude
  and codex hooks as stale BY DESIGN under the single-submitter rule (`src/doctor.rs:PAIRED_JSON` doc).
- Timeout and cancellation: `src/system.rs:run_bounded` runs the read on a thread, polls the child with a
  backoff, and on expiry sends `kill` and reaps with `wait`. Pinned end to end by
  `tests/dispatch.rs:a_moshi_hook_that_never_returns_does_not_park_the_doctor`, which hangs each leg in
  turn (with `PNS_MOSHI_STATUS_DEADLINE_MS=200` and `PNS_MOSHI_JSON_DEADLINE_MS=200`) and asserts the
  whole command finished in under 2 seconds, that the other leg's answer still arrived, and that the
  sections printed before it survived.
- Idempotency and duplicates: two spawns per run, never more.
- Privacy: `moshi-hook`'s standard error is discarded (`run_bounded` sets it to null); only standard
  output is read.
- Process ownership and cleanup: the child is killed and waited on when the deadline blows, so no zombie
  is left behind.
- Compatibility contract: a forward risk is named rather than coded around at `src/main.rs:read_pairing`.
  Every pairing state exits 0 today, so a future moshi that exited non-zero when unpaired would come back
  as no answer and be reported as "could not check" while the approval path is really dead. A future
  moshi that renamed or dropped the `server:` line degrades the other way, silently and safely.

### 15. What the pairing report reads out of the JSON answer

Given the local answer is JavaScript Object Notation (JSON)

When `src/doctor.rs:pairing_of` reads it

Then exactly three keys are read (`paired`, `hostId`, `displayName`), and the four states each get their
own sentence.

- Success: `{"paired":true,...}` yields `Paired { host_id, display_name }` with both values verbatim,
  pinned by `src/doctor.rs:a_paired_answer_carries_back_the_host_id_and_display_name_moshi_named`;
  `{"paired":false,...}` yields `Unpaired`, pinned by
  `src/doctor.rs:an_unpaired_answer_is_unpaired_rather_than_unreadable`.

- Failure sources: `""`, `not json at all`, `{`, `{"displayName":"dresden"}` and `{"paired":"yes"}` all
  yield `Unreadable`, pinned by
  `src/doctor.rs:json_that_will_not_parse_or_names_no_paired_key_claims_neither`. `None` yields
  `NoAnswer`, pinned by
  `src/doctor.rs:a_pairing_built_from_no_answer_claims_neither_paired_nor_unpaired`. A `paired: true`
  with either identifier missing renders that field as the literal `not reported`.

- Fail direction: a shape nobody recognized is NEVER read as unpaired. The comment states the harm:
  "guessing the one state that earns an exit 1 out of a shape nobody recognized is how a doctor starts
  failing healthy machines."

- Thresholds: an answer longer than `src/doctor.rs:ANSWER_MAX` (1,048,576 bytes) is `Unreadable`; one of
  exactly 1,048,576 bytes is read normally. See behavior 18 for the reader's own, wider ceiling.

- Required side effects: none, this is pure over the two answers.

- Forbidden side effects: nothing else on the object is read.

- Timeout and cancellation: Not applicable, pure.

- Idempotency and duplicates: pure.

- Privacy: the host identifier and display name ARE printed, deliberately: the host identifier is what
  the operator compares against the phone.

- Process ownership and cleanup: Not applicable.

- Compatibility contract, the four sentences verbatim (each prefixed `pns doctor: moshi pairing: `):

  - `paired as <display name> (<host id>).`
  - `this host is NOT paired, so every approval card is dead until `moshi-hook pair` runs.`
  - `moshi-hook answered something this cannot read.`
  - `moshi-hook did not answer (not installed, or it did not answer in time), so the approval path could not be checked.`

  The paired line says who this host is paired as and STOPS.
  `src/doctor.rs:the_paired_line_names_the_host_and_claims_nothing_about_approvals` asserts the line
  contains none of `approvals work`, `working`, `will reach`, `healthy`, because a re-pair mints a new
  host identifier while the live daemon keeps serving the old one, and an approval only really round
  trips when a human taps a card. Neither is visible from here.

### 16. The server sentence is relayed as moshi's own words, off a line prefix

Given pns has no stable way to tell "Moshi Pro attached" from "host does not belong to this user token"

When `src/doctor.rs:server_said` scans the plain answer

Then it takes the ONE line beginning with `server:` at column zero, strips the label, trims, and relays
what is left without judging it.

- Success: `src/doctor.rs:the_server_line_is_relayed_as_moshis_own_words_with_the_label_removed` asserts
  the captured 0.3.3 output relays `Moshi Pro attached (usage scope: license)`.
- Failure sources: no `server:` line at all relays nothing, and a label with nothing after it relays
  nothing (the empty value is filtered). Both pinned by
  `src/doctor.rs:plain_output_with_no_server_line_relays_nothing_rather_than_an_empty_line`, which also
  asserts no line containing `moshi says` is produced.
- Fail direction: silent and safe. A future moshi that renamed or dropped the line simply relays nothing
  and nothing else about the report moves.
- Thresholds: the label is a line PREFIX and never a substring.
  `src/doctor.rs:only_a_server_line_at_column_zero_is_relayed` asserts that an indented
  `  server: an indented line` is ignored when a real one follows, and that an indented line ALONE relays
  nothing. moshi indents its detail lines, so a substring rule would quote whichever of them said the
  word first.
- Required side effects: at most one extra line.
- Forbidden side effects: nothing here matches on the sentence's content. The comment states the harm: a
  prefix or substring rule over moshi's prose "would fail in the dangerous direction the day the wording
  changes".
- Timeout and cancellation: Not applicable, pure.
- Idempotency and duplicates: exactly one relayed line, or none.
- Privacy: this is third-party text going to the operator's terminal, filtered by behavior 17.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, verbatim: `pns doctor: moshi says: <moshi's sentence>`. The label is what
  attributes the claim to moshi rather than to pns, since pns is not making it and could not check it.

### 17. Third-party text cannot forge a report line

Given every string moshi chose reaches the operator's terminal inside a line pns signs its own name to

When `src/doctor.rs:printable` filters it

Then only the space character and printable ASCII survive, capped at `RELAY_MAX` characters, and the same
filter is applied to the relayed sentence AND to both identity fields.

- Success:
  `src/doctor.rs:a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line` feeds a
  server value carrying a newline, a forged summary, a carriage return, an escape sequence and a bell,
  asserts the result is exactly two lines, and asserts the relayed line reads
  `pns doctor: moshi says: attachedpns doctor: 9 sent, 0 failed, 0 skipped[2Kok`.
  `src/doctor.rs:an_identity_moshi_named_cannot_forge_a_report_line_either` does the same through the
  JSON path with a hostile `displayName` and `hostId`, asserts the whole output is ONE line, and asserts
  no character in it is a control character. That test's comment records that the forgery was MEASURED
  before the fix: a `displayName` carrying a newline printed `pns doctor: 9 sent, 0 failed, 0 skipped` as
  its own line inside the real report.
- Failure sources: any control byte, any non-ASCII character. Non-ASCII is dropped whole, which is also
  what makes the cap safe: the count can never land inside a multi-byte sequence.
- Fail direction: drop rather than escape. A report that can be made to lie about itself is worse than no
  relay at all.
- Thresholds: `src/doctor.rs:RELAY_MAX` is 200 CHARACTERS, counted after filtering.
  `src/doctor.rs:an_over_long_relayed_value_stops_at_the_cap` asserts a 500-character value relays
  exactly 200 characters, and that a value of 300 non-ASCII characters relays NOTHING at all (one line,
  not two), because nothing printable survived. One character either side: 200 characters relay whole,
  201 lose the last one.
- Required side effects: the filter is applied at the point the value becomes a LINE
  (`src/doctor.rs:pairing_lines`, `src/doctor.rs:said_of`), never at the point it is stored. The report
  holds what moshi said; this decides what may be printed.
- Forbidden side effects: this does NOT reuse the decision ring's identity filter. The doc states the
  difference: that rule judges a short identity token that becomes a key's value and replaces the whole
  thing when it fails, while this judges a relayed English sentence full of spaces, parentheses, quotes
  and colons. One predicate for both would have to be the wider of the two.
- Timeout and cancellation: Not applicable, pure.
- Idempotency and duplicates: pure.
- Privacy: this is the privacy and integrity control for the pairing section.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the newline is the load-bearing one, because an unfiltered newline prints a
  second `pns doctor:` line the operator reads as pns's own verdict. The carriage return is the one that
  survives being split into lines and returns a terminal's cursor to column zero for whatever follows to
  overwrite the prefix with.

### 18. The pairing read is bounded in time AND in bytes, at two different ceilings

Given the deadlines bound time and not bytes, so a moshi-hook answering endlessly would stay inside its
window while pns handed the lot to a parser

When `src/main.rs:read_pairing` reads and `src/doctor.rs:within_cap` checks

Then the reader stops at `PAIRING_READ_MAX` and the check refuses anything past `ANSWER_MAX`, and the gap
between them is what keeps the two refusals distinguishable.

- Success: `tests/dispatch.rs:an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read` stubs
  a moshi-hook that answers well-formed but over-sized JSON and an over-sized plain answer, asserts the
  report says `pns doctor: moshi pairing: moshi-hook answered something this cannot read.`, asserts
  neither `dresden` nor `host_over_cap` appears anywhere (so nothing was parsed), asserts no `moshi says`
  line (so nothing was scanned), and asserts the whole run took under 5 seconds. Its comment notes the
  answers are deliberately WELL FORMED: junk bytes would land on `Unreadable` through the parser and
  prove nothing about the cap.
- Failure sources: an answer above `ANSWER_MAX` but at or below `PAIRING_READ_MAX` ARRIVES and is refused
  by the check, reported as "answered something this cannot read". An answer above `PAIRING_READ_MAX`
  never arrives at all: `run_bounded` filters it to `None`, so it is reported as "did not answer".
- Fail direction: past the reader's ceiling is no answer, which is the deadline's own direction rather
  than a second one. `src/system.rs:run_bounded` states why: a truncated answer arrives at a caller
  looking exactly like a complete short answer.
- Thresholds, exact: `src/doctor.rs:ANSWER_MAX` is 1,048,576 bytes (1024 * 1024).
  `src/main.rs:PAIRING_READ_MAX` is 2,097,152 bytes, defined as `2 * ANSWER_MAX`. One step either side:
  an answer of exactly 1,048,576 bytes is READ; 1,048,577 bytes is `Unreadable`; exactly 2,097,152 bytes
  still arrives and is `Unreadable`; 2,097,153 bytes is `NoAnswer`. The reader asks the pipe for
  `max_bytes + 1` precisely so "over the cap" and "exactly at the cap" stay two different answers.
- Required side effects: the pipe is closed under the child at the ceiling, which is also what stops it
  writing.
- Forbidden side effects: the check is made BEFORE either the parser or the line scan runs, which is the
  only point where it means anything. It is checked in `src/doctor.rs` rather than in the shared bounded
  spawn because every other caller of that spawn reads a different tool, and one of them is a condenser
  whose whole job is to answer at length.
- Timeout and cancellation: see behavior 14.
- Idempotency and duplicates: Not applicable.
- Privacy: an over-cap answer is never quoted, only refused.
- Process ownership and cleanup: the child is killed and reaped when the read is refused.
- Compatibility contract: an accepted limit is stated at `src/main.rs:PAIRING_READ_MAX`. A moshi-hook
  that answers with more than two megabytes is reported as a daemon that DID NOT ANSWER, so a wedged
  daemon streaming prose is diagnosed as a dead one, sending the operator to `brew services restart`
  rather than to the output. The trade is deliberate.

### 19. The Focus line, in five sentences

Given this feature dies OPEN and SILENT if the store is ever gated, moved, or changed

When `src/main.rs:focus_line` reports

Then it prints exactly one line, in one of five sentences, optionally extended by a catalog clause, and
it never moves the exit code.

- Success: an empty `[focus] silence` list prints the OFF sentence without opening any file (the census
  test's `FOCUS_OFF_LINE`). A named mode asserted prints the ON sentence, and a mode the config did not
  name prints the quiet sentence. Both pinned by
  `tests/dispatch.rs:the_doctor_tells_the_truth_about_a_named_focus_in_every_state`, whose comment
  records that the ON sentence "could lie without anything going red" until this test existed.

- Failure sources: `NotFound` on the assertion store prints the ABSENT sentence; any other read error
  prints the UNREADABLE sentence with its error kind named. The same test builds the unreadable case by
  chmod 000 on `Assertions.json` and asserts the line ends
  `could not be read, so Focus is being ignored (permission denied).`, and builds the absent case on a
  sandbox with no store at all and asserts the output does NOT contain `could not be read`.

- Fail direction: reporting, never grading. The comment at `src/main.rs:doctor_mode` states the rule: "a
  Focus being on is not a fault".

- Thresholds: the two store files are read through `readable_state_file` at `RING_READ_MAX`, 262,144 bytes; the
  live store is 6 KiB. A store past that ceiling reads as `FileTooLarge`, which is the unreadable
  sentence.

- Required side effects: none. Read-only, and with no `[focus] silence` list the files are never opened,
  so a default machine pays no input or output for a feature it did not ask for.

- Forbidden side effects: no environment hatch names the store path. `src/main.rs:focus_now` states why:
  a variable naming this path would let any producer force the answer in either direction. The test seam
  is the sandbox's own `HOME`.

- Timeout and cancellation: a named pipe at either path is refused by `readable_state_file`'s regular-file
  check rather than opened, so it cannot park the doctor.

- Idempotency and duplicates: one line per run.

- Privacy: no mode name and no identifier from the store is printed; only the graded state and, when it
  applies, the catalog's error kind.

- Process ownership and cleanup: Not applicable, no child.

- Compatibility contract, the five sentences verbatim (each prefixed `pns doctor: `):

  - `focus awareness is off (no [focus] table names a mode to silence)`
  - `a macOS Focus you named is ON, so banners, cards and pulses are suppressed`
  - `no macOS Focus you named is active`
  - `no Focus database was found on this machine, so no Focus is being respected`
  - `the Focus database could not be read, so Focus is being ignored (<kind>).`

  Either of the middle two may be extended with
  `; the mode catalog could not be read (<kind>), so no Focus NAME can match and only a raw modeIdentifier still would`,
  pinned by
  `tests/dispatch.rs:a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health`, which
  also asserts the state sentence is EXTENDED rather than replaced. The accepted limit is stated at
  `src/main.rs:focus_line`: the parser is TOTAL, so bytes that are not JSON at all, and a schema change
  that leaves the file valid JSON, both read as "no Focus" rather than as an error.

### 20. The daemon line, in five states

Given the config switch is not the process

When `src/doctor.rs:daemon_line` is handed the switch, the heartbeat, the clock, and the job count

Then it reports one of five states, and it never moves the exit code.

- Success: the four ordinary states are pinned by
  `src/doctor.rs:the_daemons_doctor_line_tells_the_truth_in_four_states`, including the singular
  `1 job scheduled` against the plural `2 jobs scheduled`. The fifth, a switch turned off while the
  process is still beating, is pinned by
  `src/doctor.rs:a_daemon_switched_off_but_still_beating_is_reported_as_still_beating`.

- Failure sources: a heartbeat whose age cannot be taken (no clock, or a beat stamped in the future)
  reads as NOT RUNNING with an unknown age, pinned by
  `src/doctor.rs:a_heartbeat_whose_age_cannot_be_taken_reads_as_not_running`. A heartbeat file that does
  not parse yields no beat at all, pinned by
  `src/doctor.rs:a_heartbeat_round_trips_and_anything_else_is_no_heartbeat_at_all`.

- Fail direction: toward "not running". The doc states the rule: claiming a daemon is alive on the
  strength of a timestamp nothing could compare is the identity-is-not-presence mistake with a file
  standing in for the pid.

- Thresholds, exact: `src/daemon.rs:HEARTBEAT_STALE_SECS` is 10 seconds (`10 * DEFAULT_TICK_SECS`, and
  `DEFAULT_TICK_SECS` is 1). An age of exactly 10 seconds reads as RUNNING; 11 seconds reads as not
  running. The unit test builds both boundary cases explicitly.

- Required side effects: none. Two reads that cost nothing: the heartbeat file, and a count of the spool.

- Forbidden side effects: it never signals the pid, because a pid can be reused. `enabled` comes from the
  ONE config read the doctor already took, never a second one, so the report cannot describe a switch the
  run itself never saw. Its broken-config fallback is ON, the same one `daemon_run` takes, so the report
  and the service cannot disagree.

- Timeout and cancellation: a non-regular file at the heartbeat path is NEVER OPENED
  (`src/main.rs:daemon_line` checks `symlink_metadata(...).is_file()` first). The reason is stated there:
  `open` on a named pipe blocks until a writer arrives, so a doctor that read whatever it found would
  hang with the pairing check and the exit code never reached.

- Idempotency and duplicates: one line per run.

- Privacy: it COUNTS jobs and never names them, following the missed journal's structural privacy rule.
  `src/daemon.rs:job_count` counts REGULAR FILES only, so the word "job" in the sentence is earned.

- Process ownership and cleanup: Not applicable.

- Compatibility contract, the five sentences verbatim (each prefixed `pns doctor: `):

  - `the daemon is off in the config`
  - `the daemon is off in the config, but pid <pid> is still beating; bootout (or wait) to stop it`
  - `the daemon is enabled and has not run yet`
  - `the daemon is running, pid <pid>, <n> job(s) scheduled`
  - `the daemon is enabled, its last beat was <n>s ago, so it is not running`, or
    `the daemon is enabled, its last beat was an unknown time ago, so it is not running`

  The doc at `src/doctor.rs:daemon_line` states why it returns a `String` and is never an input to
  `exit_code`: "a daemon that is down costs ambient features rather than a card. Reporting it as a broken
  notifier would be the fail-open sin's mirror: a true alarm about the wrong thing, in a place that
  already means something else."

### 21. The nag line, in two states

Given the config's `[nag] after_secs` is the whole input

When `src/doctor.rs:nag_line` renders it

Then zero prints the off sentence and anything else prints the schedule, in the same unit the card uses.

- Success: `src/doctor.rs:the_nag_line_names_the_schedule_or_says_the_feature_is_off` asserts all three:
  0 gives the off sentence, 300 gives `after 5m`, and 30 gives `after 30s`. The unit comes from
  `src/nag.rs:waited`: under 60 seconds prints seconds, under 3600 prints whole minutes, otherwise whole
  hours.
- Failure sources: none. The value is range-bound at parse time and this function is total over `u64`.
- Fail direction: Not applicable.
- Thresholds: `src/main.rs:NAG_OFF` is 0, and it is also the config default (`src/config.rs`,
  `nag_after_secs: NAG_OFF`). Two states and not three: no `[nag]` table and `after_secs = 0` are the
  SAME statement in this config, so telling them apart would be the doctor inventing a distinction the
  parser does not carry.
- Required side effects: one line on standard output.
- Forbidden side effects: it reports the config only and does NOT grade the daemon. The doc says why: a
  nag with a dead daemon never fires, but the daemon line one row above already says the daemon is not
  running, from the heartbeat, and two lines deriving one fact is how they drift apart.
- Timeout and cancellation: Not applicable, pure.
- Idempotency and duplicates: pure.
- Privacy: a duration only.
- Process ownership and cleanup: Not applicable.
- Compatibility contract, verbatim: `` pns doctor: the nag is off (no `[nag] after_secs`) `` and
  `pns doctor: an unanswered approval is carded again after <duration>`. THE PLACEMENT IS THE WHOLE
  MITIGATION for the fact it does not grade the daemon: it sits immediately below the daemon line so the
  two read as one paragraph, pinned by
  `tests/dispatch.rs:the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section`
  at `lines[summary + 5]`.

### 22. The lights section, in six states

Given a dark lamp is not a broken notifier

When `src/main.rs:lights_report` decides the state and `src/doctor.rs:lights_lines` renders it

Then one of six states is reported, per BEHAVIOUR rather than per lamp, and the exit code never moves.

- Success: all six are pinned by
  `src/doctor.rs:the_lights_section_says_which_of_its_six_states_the_config_is_in`, and a resolved map
  renders `pns doctor: lights: done 2, failed 2, blocked 1, unread 1, loop 0`. The behaviour words and
  their order come from `src/config.rs:BEHAVIOUR_WORDS`: `done`, `failed`, `blocked`, `unread`, `loop`.

- Failure sources: `HueMissing` (a `[lights]` table with no `[plugins.hue]` table at all) versus
  `HueDisabled` (a `[plugins.hue]` table with its switch off) are told apart by reading
  `config.plugins.contains_key("hue")` separately, because `enabled_hue_table` answers `None` for both.
  The test's message states the harm of merging them: "telling an operator to go flip a switch that is
  not there is the kind of wrong direction they act on." `NoBridge` (no bridge and key named) versus
  `Unreachable` (a bridge that listed nothing) are told apart for the same class of reason: one is a
  config to fix and the other is a network to fix.

- Fail direction: reporting, never grading. A bridge call that PANICS is caught and reported as
  `Unreachable`, because a call that panicked resolved no lamp.

- Thresholds: a behaviour NOTHING carries is listed at ZERO rather than omitted, because "the word I
  wrote is missing from the report" is not a state anybody should have to infer from an absence.
  `src/doctor.rs:every_lights_state_says_something_rather_than_printing_nothing` asserts every state
  including an entirely empty routing produces at least one line.

- Required side effects: three read-only `GET` calls to the bridge, and only for a config that has asked
  for the lamps AND enabled hue AND named a bridge and key. The cost is named at
  `src/main.rs:lights_report`: each `GET` is bounded by `BRIDGE_DEADLINE`, so a bridge that accepts and
  never answers adds up to thirty seconds to `pns doctor`, paid only by a machine that wrote the table.

- Forbidden side effects: no lamp is written on this path. Counts and names only, following the missed
  journal's structural privacy rule: no colours, no session identifiers, no detail text.

- Timeout and cancellation: `BRIDGE_DEADLINE`, 10 seconds per call. This section is deliberately the LAST
  thing that touches the network, so a bridge that hangs cannot delay a line above it.

- Idempotency and duplicates: one section per run.

- Privacy: lamp, room and zone NAMES appear only in the unresolved and refusal lines, which are the
  operator's own declarations echoed back. The dim window and the per-lamp arbitration are what the three
  listings are for, and neither is printed.

- Process ownership and cleanup: Not applicable, in-process HTTP.

- Compatibility contract, the six openings verbatim (each prefixed `pns doctor: `):

  - `lights: off in the config, so the pulse uses the [plugins.hue] rooms`
  - `lights: configured, but there is no [plugins.hue] table to light them through`
  - `lights: configured, but [plugins.hue] enabled is false, so nothing lights`
  - `lights: no [plugins.hue] bridge and key, so no lamp could be resolved`
  - `lights: the bridge listed nothing, so no lamp resolved`
  - `lights: done <n>, failed <n>, blocked <n>, unread <n>, loop <n>`

  A resolved state adds one line per unresolved name and one per refusal, in the channel's OWN words
  (`src/channels/hue.rs:missing_sentence`), so the tick reports an unresolved lamp in the same words and
  only the prefix differs. Pinned by
  `src/doctor.rs:an_unresolved_name_and_a_refused_declaration_each_get_their_own_line`, which asserts
  `` pns doctor: lights: `3F - Studio - HCL9` (lamp) is not on the bridge ``,
  `` pns doctor: lights: `3F - Cupboard` (room) is on the bridge, but it holds no lamp `` and
  `` pns doctor: lights: `HCL1` is covered by 2 zone declarations ``.

- Documentation discrepancy, noted rather than fixed: `src/main.rs:lights_report`'s doc comment says
  "which of its five states this machine is in" while `src/doctor.rs:LightsReport` has six variants and
  its own doc says "SIX STATES AND NO GRADE". The behavior is six; the main.rs comment is stale.

### 23. The decision ring section reports history and is never appended to

Given the operator came to read the decision the card just did or did not fire on

When `src/main.rs:decision_section` reads `<state>/decisions`

Then it renders the last five entries newest first, or one honest sentence, and the exit code never moves
for any of it.

- Success: `tests/dispatch.rs:the_doctor_prints_the_decision_section_after_its_summary_newest_first` runs
  two events and asserts the heading is `pns doctor: the last 2 decisions,<heading tail>`, that the
  NEWEST leads, and that only the journal's count follows.
- Failure sources: three states. An absent ring gives
  `pns doctor: no decision has been recorded yet (no event has run since this was installed, or none could be written).`
  (`src/decision_log.rs:NOTHING_RECORDED`); a line nobody can parse is quoted back as
  `  unreadable entry: "<line>"`; and a read that failed gives
  `pns doctor: the decision log could not be read (<kind>).` (`src/main.rs:DECISIONS_UNREADABLE`). The
  first two are pinned by
  `tests/dispatch.rs:the_doctors_exit_code_does_not_move_for_a_log_that_is_absent_or_unreadable`; the
  third by
  `tests/dispatch.rs:a_ring_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`,
  which uses a directory at the path and asserts the kind is NAMED rather than left an empty parenthesis,
  and that the absent sentence never appears for it.
- Fail direction: absent and unreadable are DIFFERENT sentences, because one is fixed by running an event
  and the other by looking at the file.
- Thresholds: `src/decision_log.rs:KEPT` is 5 entries. The heading counts what it is ABOUT TO SHOW rather
  than the cap, so one entry reads `the last decision,` and never `the last 5 decisions,`. The file is
  read through `readable_state_file` at `RING_READ_MAX`, 262,144 bytes; past that the read fails with
  `FileTooLarge` and lands on the unreadable sentence.
- Required side effects: none. Read only.
- Forbidden side effects: NEVER APPENDED TO. `src/main.rs:decision_section` states why: "A doctor that
  recorded would push the decision the operator came to read out of the ring by the act of going to look
  at it." Pinned by `tests/dispatch.rs:the_doctor_records_no_decision_of_its_own`, which byte-compares
  the ring before and after.
- Timeout and cancellation: a named pipe at the ring's path is refused rather than opened, and the file
  is left exactly as found. Pinned by
  `tests/dispatch.rs:a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind`, which
  runs the command under a deadline and afterwards asserts the path still holds a named pipe. Its comment
  records the measurement: opening one BLOCKS until the other end is opened, for reading as much as for
  writing.
- Idempotency and duplicates: reading changes nothing, so two runs print the same section.
- Privacy: the section's heading tells the truth about what is NOT recorded rather than printing an empty
  field. Entry bodies are ESCAPED and printed, never parsed (`src/decision_log.rs:render`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract, the heading tail verbatim:
  ` newest first (why a card did or did not fire). No actionId is recorded: moshi mints it inside the approval round trip and never hands it back.`

### 24. The missed-notification journal is COUNTED and never written

Given the doctor's own test send is the last event anything should ever replay

When `src/main.rs:missed_line` reads `<state>/missed-notifications`

Then it counts non-blank lines, says what will deliver them, and writes nothing.

- Success: `tests/dispatch.rs:the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it`
  plants two entries and asserts the LAST line of the whole report is
  `pns doctor: 2 missed notifications are waiting to be replayed; the next event that raises a banner or a card while the operator is not away delivers them.`,
  with exit 0.
- Failure sources: an absent journal gives `pns doctor: no missed notification is recorded.`; a failed
  read gives `pns doctor: the missed-notification journal could not be read (<kind>).`
  (`src/main.rs:MISSED_UNREADABLE`). Both pinned, the unreadable case by
  `tests/dispatch.rs:a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`,
  which also asserts the absent sentence never appears for it.
- Fail direction: the zero sentence is deliberately about what is RECORDED, not about what was missed: an
  empty journal means either nothing was missed or a write did not land, and the line claims neither.
- Thresholds: singular at one entry, plural above. Read through `readable_state_file` at 262,144 bytes.
- Required side effects: none. Read only.
- Forbidden side effects: NOTHING HERE PARSES AN ENTRY. The contents go straight to
  `src/missed_notifications.rs:waiting_line`, which counts lines and has no parse at all, so the
  operator's own text has no path from this file to a terminal. And nothing is appended: pinned by
  `tests/dispatch.rs:the_doctor_leaves_the_journal_exactly_as_it_found_it`, which byte-compares the file
  and then asserts the state directory holds exactly `["missed-notifications"]`.
- Timeout and cancellation: a named pipe at the journal's path is refused, not opened, and left in place.
  Pinned by
  `tests/dispatch.rs:a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`, whose
  comment notes this file is read on the doctor's way OUT, so a park here wedges the command after it has
  already sent to every channel.
- Idempotency and duplicates: reading changes nothing.
- Privacy: a count, never the entries.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the sentence changes when `[recap] replay_card = false`, because with the card
  switched off nothing will ever deliver what is counted and a doctor that still named "the next event"
  would be telling the operator a lie their own setting makes permanent. The off wording is
  `pns doctor: <n> missed notifications are recorded; the catch-up card is switched off (`[recap]
  replay_card = false`), so nothing delivers them until the card is switched back on.` The zero sentence
  is the same either way.

### 25. The exit code: sends, plus one pairing state, and nothing else

Given the doctor is hand typed and is never a hook, so the always-exit-0 contract does not apply here

When `src/doctor.rs:exit_code` decides from the outcomes and the pairing report

Then any failure exits 1, an unpaired host alone exits 1, and a run that sent NOTHING exits 1; only a run
that sent something and failed nothing and is not unpaired exits 0.

- Success: pinned by `src/doctor.rs:only_a_run_that_sent_something_and_failed_nothing_exits_zero`, which
  covers `Sent` plus a skip (0), `SentUnreported` alone (0), `Signalled(3)` (0), one failure among
  successes (1), `Signalled(0)` (1), skips only (1), and the empty list (1).
- Failure sources: `Pairing::Unpaired` alone moves it, pinned by
  `src/doctor.rs:an_unpaired_host_alone_earns_the_exit_code_a_one`, which asserts the identical green
  sends exit 0 with a healthy pairing and 1 with an unpaired one, "the pairing ALONE moved it, with
  nothing else changed". End to end,
  `tests/dispatch.rs:an_unpaired_host_exits_one_while_the_summary_still_reads_zero_failed` asserts exit 1
  while the summary still reads `pns doctor: 3 sent, 0 failed, 2 skipped`.
- Fail direction: a check that could not run is never a failure it found. `NoAnswer` and `Unreadable`
  leave a green run at 0, pinned by
  `src/doctor.rs:a_no_answer_or_unreadable_pairing_leaves_a_green_run_exiting_zero`, so a machine that
  does not use moshi does not fail its doctor forever. And neither reader overrides the other: a healthy
  pairing cannot mask a failed send or turn a nothing-to-check run green, pinned by
  `src/doctor.rs:a_failed_send_still_exits_one_when_the_pairing_is_healthy`.
- Thresholds: exit 2 is the argument refusal (behavior 1). Exit 1 and 0 are the only other codes.
- Required side effects: the process exit code.
- Forbidden side effects: the Focus line, the daemon line, the nag line, the lights section, the decision
  ring section and the missed journal line move it in NO state. That is enforced structurally rather than
  by convention: `exit_code` takes only outcomes and a pairing report, so those lines are not even in
  scope. The doc at `src/doctor.rs:daemon_line` states it explicitly.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the pairing is an ARGUMENT rather than a second code the caller combines,
  so the summary and the exit code are decided at one point and cannot disagree.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `src/doctor.rs:exit_code`'s doc names the rule the two hardest cases share: "A
  CHECK WITH NOTHING TO CHECK MUST NEVER REPORT GREEN, which is the same ruling the mute took: reporting
  success for something that is not in effect is the worst outcome available." And the unpaired case is a
  named judgement call: it only fires on a machine where moshi-hook is installed and answering, and there
  an unregistered host means every card is going nowhere while the census reports the mobile channel
  green over its webhook.

### 26. The section order is health, then gate state, then history

Given the exit code is what an operator's automation reads as "notifications are broken"

When the doctor prints

Then the gradeable lines come first and the ungradeable ones after, in one fixed order.

- Success: the order is the opening, one line per check, the summary, the pairing line (plus the relayed
  line when there is one), the Focus line, the daemon line, the nag line, the lights section, the
  decision ring section, and the journal count. Pinned end to end by
  `tests/dispatch.rs:the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section`,
  which anchors on the summary and asserts `summary + 1` through `summary + 7` exactly, and by
  `tests/dispatch.rs:the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`,
  which asserts the whole thirteen-line report as one array.
- Failure sources: none reachable; the order is the statement order in `src/main.rs:doctor_mode`.
- Fail direction: Not applicable.
- Thresholds: Not applicable.
- Required side effects: the lights section must be the last thing that touches the network, so a bridge
  that hangs cannot delay a line above it.
- Forbidden side effects: nothing may be inserted between the census lines and the summary. The comment
  at `src/main.rs:doctor_mode` says why the sections are APPENDED after the summary: "the census plus its
  summary is one complete thought whose line order the suite already pins, and nothing below can disturb
  it."
- Timeout and cancellation: the pairing check runs AFTER the summary is printed, so a moshi-hook that
  eats its full 13 seconds cannot delay the census the operator came for.
- Idempotency and duplicates: one of each section per run.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: warnings and refusals go to standard ERROR, everything above to standard
  OUTPUT, so a caller can capture the report without capturing the complaints.

### 27. The doctor writes nothing, anywhere

Given the doctor is a reader that would corrupt what it reads if it wrote

When a full run finishes

Then the state directory, the decision ring and the journal are byte-identical to what they were.

- Success: three independent tests assert it.
  `tests/dispatch.rs:the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`
  asserts the state directory afterwards holds exactly the one file the test planted.
  `tests/dispatch.rs:the_doctor_records_no_decision_of_its_own` byte-compares the decision ring.
  `tests/dispatch.rs:the_doctor_leaves_the_journal_exactly_as_it_found_it` byte-compares the journal AND
  re-lists the state directory. `tests/dispatch.rs:the_pairing_check_records_nothing_of_its_own` lists
  the state directory and re-reads the ring around a run that really did print the paired line.
- Failure sources: none in the current code; every state path in `doctor_mode` is a read.
- Fail direction: Not applicable.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: this behavior IS the forbidden-side-effect list, and it is the counterpart to
  the live-effects section at the top: the doctor is loud in the world and silent on disk.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: two consecutive runs leave the same disk state (they do not leave the same
  world state: each run flashes the lamps and posts a card again).
- Privacy: nothing the doctor read is persisted anywhere.
- Process ownership and cleanup: the only children are the executable channels and the two `moshi-hook`
  spawns, all waited on or killed.
- Compatibility contract: `src/main.rs:missed_line` states the strongest version of the rule: "a doctor
  that journaled would file a miss for the act of going to look for one, and its own test send is the
  last event anything should ever replay."
