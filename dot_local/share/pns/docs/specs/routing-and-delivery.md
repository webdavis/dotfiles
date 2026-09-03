# Routing and delivery

## Scope

This specification covers how one pns notification becomes a set of delivery legs, how each leg reaches a
destination, what a delivery outcome is, and what happens when a destination fails. It covers the leg
model (`src/routing.rs`), the plugin roster and selection (`src/registry.rs`), the rendering that
produces the one event every destination is handed (`src/render.rs`), the three built-in destinations
(banner, mobile, hermes), executable channel discovery and invocation, the route concept behind
`--channel`, delivery outcomes and partial failure, and the rule that a notification must never fail the
work it reports on. It does NOT cover the Hue lamps: the pulse, the lamp map, the `dim window`, the
`quiet window` and `quiet hours` are deferred to the sibling specification
`docs/specs/lighting-policy.md`. It also does not cover how the surface and presence model DECIDES what
the operator can see; this specification takes `surface::DeliveryPlan` as an input. Whether the operator
is home is read by the `home probe` from the `router` sensor, which is covered here only insofar as a
sensor can never become a delivery leg.

**Hue is not a delivery destination.** It is registered as `PluginKind::Channel` with
`Routing { local: true, presence_gated: false, durable: false, event_dispatched: false }`
(`src/registry.rs:ROSTER`), and `channel_plan` filters on `routing.event_dispatched` before anything
else, so no notification ever routes to it (`src/routing.rs:channel_plan`, pinned by
`src/routing.rs:tests::a_plugin_that_is_not_event_dispatched_is_never_a_leg_however_it_is_selected`). The
binary drives it in its own `pulse` mode instead (`src/main.rs:pulse_mode`,
`src/main.rs:fire_pulse_unless_quiet`), and the hand-run check classifies it as `CheckKind::Pulse` rather
than `CheckKind::Send` for exactly that reason (`src/doctor.rs:kind_of`). It registers only so the config
can select it and so a typo in its table name is still refused.

______________________________________________________________________

## Destination table

| Destination                        | How it is selected                                                                                                                                                     | Transport                                                                                                                                                                | Configuration keys                                                                                                                                                                             | Failure behavior                                                                                                                                                                                                                                                                  | Tests that pin it                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mobile` (moshi backend)           | `[plugins.mobile] enabled = true`, or the core fallback; then `channel_plan` keeps it only when `delivery.phone_card` is true, because it is `presence_gated`          | One HTTPS POST, `ureq`, `content-type: application/json`, `max_redirects(0)`, 10 s global deadline                                                                       | `[plugins.mobile] type` (must be `"moshi"`), `[plugins.mobile] token`; env `PNS_MOSHI_URL` overrides `https://api.getmoshi.app/api/webhook`                                                    | `Delivery::Failed`; a refused `type` fails before either seam; no token fails naming the key; any non 2xx or unreachable endpoint fails. The sentence is unreachable from an event's stdout because the leg is never `ReportOutcome`                                              | `src/channels/moshi.rs:tests::a_push_the_endpoint_took_is_delivered_and_one_it_did_not_is_failed_without_the_token`, `src/channels/moshi.rs:tests::a_missing_token_posts_nothing_and_fails_by_naming_the_config_key_to_write`, `tests/native.rs:native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output`, `tests/native.rs:a_dead_moshi_endpoint_is_silent_because_the_only_report_would_carry_the_token`                                                     |
| `macos-banner`                     | `[plugins.macos-banner] enabled = true`, or the core fallback; then `channel_plan` keeps it only when `delivery.banner` is true, because it is `local`                 | Spawn of `terminal-notifier` by NAME through PATH, under `SystemCommandRunner` (5 s deadline, 1 MiB stdout ceiling)                                                      | env `PNS_TERMINAL_BUNDLE_ID`, else inherited `__CFBundleIdentifier`, else `com.mitchellh.ghostty`; `herdr` resolved on PATH at construction                                                    | `Delivery::Failed("banner FAILED (terminal-notifier did not run)")` whenever the runner answers nothing, which covers not installed, non-zero exit and killed at the deadline alike                                                                                               | `src/channels/banner.rs:tests::a_spawn_that_answered_is_delivered_and_one_that_never_ran_names_the_notifier`, `src/channels/banner.rs:tests::nothing_but_the_notifier_is_ever_spawned`, `tests/native.rs:the_banner_leg_delivers_natively_and_the_executable_channel_stays_silent`                                                                                                                                                                                                 |
| `hermes`                           | `[plugins.hermes] enabled = true`; NOT in the core, so a machine with no readable config has no durable route. Kept under `--remote-only` because it is `durable`      | One signed POST, `ureq`, `content-type: application/json`, header `X-Webhook-Signature`, `max_redirects(0)`; 10 s deadline when silent, the sync deadline when reporting | `[plugins.hermes] key`; env `PNS_HERMES_URL` overrides `http://127.0.0.1:8644/webhooks/pns`; env `PNS_REMOTE_TIMEOUT` sets the sync deadline; `--channel <route>` swaps the final path segment | `Delivery::Failed` with the outcome sentence: `post FAILED HTTP <code>`, `post FAILED HTTP 000 (no response; is the hermes gateway up?)`, `post FAILED (curl reported no HTTP status at all)`, or the no-key `post SKIPPED` line. On a `ReportOutcome` leg the failure IS printed | `src/channels/hermes.rs:tests::sync_outcomes_are_spelled_exactly_as_the_bash_spells_them`, `src/channels/hermes.rs:tests::no_key_means_no_post_in_either_mode_and_the_verdict_is_a_failure`, `tests/dispatch.rs:every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`, `tests/native.rs:sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent`, `tests/native.rs:a_gateway_that_answers_401_is_named_rather_than_read_as_a_downed_gateway` |
| Any executable channel `<name>.sh` | Reached for a planned leg when the native plugin does not win: always when `PNS_CHANNELS_DIR` is set non-empty, and for any leg name with no compiled-in arm otherwise | `Command::new(<dir>/<name>.sh)` with the event JSON plus a newline on stdin; stdout and stderr are INHERITED                                                             | env `PNS_CHANNELS_DIR`, default `$HOME/.local/libexec/pns/channels`                                                                                                                            | Never an error for the caller. A spawn that failed is `Delivery::Unlaunched`; a channel that ran is `Delivery::Silent` whatever its exit status                                                                                                                                   | `tests/dispatch.rs:an_absent_channel_is_simply_not_installed`, `tests/dispatch.rs:a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings`, `tests/dispatch.rs:a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`                                                                                                                                                                                                                 |
| `hue`                              | Registered and selectable, NEVER a leg                                                                                                                                 | Not a delivery path. Driven by `pulse` mode                                                                                                                              | Deferred to `docs/specs/lighting-policy.md`                                                                                                                                                    | Not applicable                                                                                                                                                                                                                                                                    | `src/routing.rs:tests::a_plugin_that_is_not_event_dispatched_is_never_a_leg_however_it_is_selected`                                                                                                                                                                                                                                                                                                                                                                                |
| `router`                           | Registered as `PluginKind::Sensor`, NEVER a leg                                                                                                                        | Not a delivery path. It is an input the `home probe` reads                                                                                                               | Deferred to the presence specification                                                                                                                                                         | Not applicable                                                                                                                                                                                                                                                                    | `src/routing.rs:tests::a_selected_sensor_is_never_a_leg_on_the_alert_path` and the two flag variants beside it                                                                                                                                                                                                                                                                                                                                                                     |

______________________________________________________________________

## Central dispatch on a destination NAME

A later refactor is required to remove these. Each is a place where the open registration model
(`Routing` declarations) is closed again by a hard-coded name.

1. **`src/main.rs:deliver_leg`** is the primary one. It performs
   `match leg.name { "macos-banner" => ..., "mobile" => ..., "hermes" => ..., _ => {} }` inside an
   `if native_wins` block, and falls through to the executable channel at
   `channels_dir.join(format!("{}.sh", leg.name))`. Adding a native destination means editing this match,
   which is exactly what `src/registry.rs`'s module comment claims registration removed.
1. **`src/main.rs:dispatch_legs`** gates the backend refusal on `if leg.name == "mobile"`, returning
   `Delivery::Failed(refused_backend_line(reason))` before any seam is chosen.
1. **`src/main.rs:disabled_backend_warnings`** switches on the literal table names `"router"` and
   `"mobile"` to decide which two `type` refusals earn a diagnostic line.
1. **`src/main.rs:run_event`** computes `durable_route` as
   `selection.iter().any(|plugin| plugin.name == "hermes")` rather than from the `durable` declaration
   that already exists on the registration.
1. **`src/main.rs:deliver_recap`** constructs its single leg with the literal `name: "hermes"`.
1. **`src/main.rs:enabled_hue_table`** and **`src/main.rs:plugin_settings(config, "hermes")`** read
   settings tables by literal name at the composition root.
1. **`src/main.rs:doctor_mode`** pairs outcomes to checks with
   `delivered.iter().find(|(leg, _)| leg.name == check.plugin)`. This one is a name PAIRING rather than a
   switch, and its own comment states it is deliberately by name rather than by position.
1. In tests,
   `src/routing.rs:tests::no_plan_over_the_real_roster_hands_the_phone_or_the_banner_a_reporting_leg`
   asserts over `matches!(planned.name, "mobile" | "macos-banner")`, so the structural safety argument is
   itself keyed on two names.

______________________________________________________________________

## Behaviors

### 1. A plan is legs, computed from declarations and never from names

Given a vetted `Selection` of registered plugins and a `surface::DeliveryPlan`

When `channel_plan` runs

Then it emits one `Leg { name, mode, decorative }` per surviving registration, in REGISTRATION order,
filtering only on `PluginKind`, `routing.event_dispatched`, `routing.local`, `routing.presence_gated` and
`routing.durable`. Presence-gated survives only when `delivery.phone_card`; local survives only when
`delivery.banner`; anything else survives unconditionally, because the durable log is what every event
reaches.

- **Success:** `src/routing.rs:tests::the_alert_path_plans_phone_then_banner_then_log` pins the full
  alert plan as `mobile`, then `macos-banner`, then `hermes`, all `ReportMode::Silent`.
- **Failure sources:** A registration that mislaid or mis-stated its `Routing` declaration. A registry
  whose registration order changed.
- **Fail direction:** Toward planning nothing rather than planning a wrong destination. An empty plan is
  a legitimate verdict the caller must report rather than pass over (`src/routing.rs:channel_plan` doc
  comment), and `src/routing.rs:tests::no_enabled_plugins_plan_nothing_under_every_flag` pins that an
  unconfigured machine gets an empty plan rather than a crash or a built-in fallback.
- **Thresholds:** Not applicable. No numeric threshold participates.
- **Required side effects:** None. `channel_plan` is pure over its four arguments.
- **Forbidden side effects:** No IO, no clock read, no printing. It cannot name a plugin, so it cannot
  special-case one.
- **Timeout and cancellation:** Not applicable, pure function.
- **Idempotency and duplicates:** Pure and total; the same inputs give the same `Vec<Leg>`. Each
  registered name appears at most once because `Registry::register_plugin` refuses a duplicate name
  (`src/registry.rs:register_plugin`).
- **Privacy:** Not applicable. No event content reaches this function.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** Registration order IS delivery order, stated once in
  `src/registry.rs:ROSTER`. The `PluginKind` match is exhaustive on purpose so a third kind must state
  its answer rather than inherit delivery from a catch-all.

### 2. A sensor can never become a delivery leg

Given a `Selection` that includes the enabled `router` sensor alongside the three channels

When any plan is computed, under any combination of `local_only`, `remote_only` and phone verdict

Then the sensor is absent from every plan, because `PluginKind::Sensor` carries no `Routing` for the plan
to read at all.

- **Success:** `src/routing.rs:tests::a_selected_sensor_is_never_a_leg_on_the_alert_path`,
  `:a_selected_sensor_is_never_a_leg_under_local_only_either`,
  `:a_selected_sensor_is_never_a_leg_under_remote_only_either`. Each carries the three channels as a
  positive control, so a plan that dropped everything cannot pass by looking like a suppressed sensor.
- **Failure sources:** A future filter that treated "reads this machine" as "local surface" would admit a
  sensor under `--local-only`, which the test comment names as the most likely mistake.
- **Fail direction:** Absent. The consequence of the alternative is stated in code: the engine would try
  to exec `channels/router.sh` on every notification, and on the `--remote-only` path that attempt would
  be printed to the operator by name.
- **Thresholds:** Not applicable.
- **Required side effects:** None.
- **Forbidden side effects:** No exec attempt named after a sensor.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** A sensor still occupies the one `[plugins.<name>]` namespace so a typo in
  its name is still refused (`src/registry.rs:register_plugin` doc comment). The fallback roster
  `Registry::all()` must keep knowing sensor names for that refusal to work, pinned by
  `src/routing.rs:tests::the_unconfigured_machine_knows_every_sensor_and_still_plans_channels_only`.

### 3. The narrowing flags select a surface set, and `--remote-only` alone selects the report mode

Given `--local-only` and `--remote-only` as two independent booleans

When `channel_plan` runs

Then both together return an empty plan; `--local-only` alone keeps the `local` plugins; `--remote-only`
alone keeps the `durable` plugins AND sets every leg's mode to `ReportMode::ReportOutcome`; neither flag
leaves every event-dispatched plugin eligible at `ReportMode::Silent`.

- **Success:**
  `src/routing.rs:tests::local_only_plans_the_local_surfaces_alone_whatever_the_phone_verdict_was`,
  `:remote_only_plans_the_durable_legs_alone_and_sync_which_keeps_a_lost_entry_visible`,
  `:both_narrowing_flags_plan_nothing_at_all`. End to end:
  `tests/dispatch.rs:local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`,
  `tests/dispatch.rs:remote_only_delivers_through_hermes_alone`,
  `tests/dispatch.rs:hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible`.
- **Failure sources:** A narrowing that also consulted the phone verdict when setting the mode. Both
  `--remote-only` tests assert the suppressed-phone form for exactly that reason.
- **Fail direction:** The mode is set from the flag alone, so an undelivered durable entry is always
  visible. An undelivered log entry is invisible in a way an undelivered alert is not
  (`src/registry.rs:Routing::durable`).
- **Thresholds:** Not applicable.
- **Required side effects:** With both flags given and the plan therefore empty, the event path prints
  exactly:
  `pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent`
  on stdout (`src/main.rs:run_event`). Pinned by
  `tests/dispatch.rs:both_narrowing_flags_together_deliver_nothing_and_say_so`.
- **Forbidden side effects:** No channel is spawned and no HTTP request is made when the plan is empty.
  The pane scrub warning is also withheld, because a scrub nobody was going to receive is not news
  (`tests/dispatch.rs:a_scrub_warning_is_not_printed_when_no_channel_will_run`).
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** `ReportMode::as_str` emits the wire words `async` and `sync`, NOT the
  variant names, because executable channels read those words out of the event's `mode` field
  (`src/routing.rs:ReportMode::as_str`, pinned by
  `src/routing.rs:tests::a_mode_names_what_the_channel_contract_spells_in_the_event` and end to end by
  `tests/dispatch.rs:the_alert_path_labels_the_hermes_leg_silent_on_the_wire`).

### 4. The presence gate means one thing under every flag

Given a presence-gated plugin, whatever else it declares

When any plan is computed

Then it is dropped whenever `delivery.phone_card` is false, under every combination of the narrowing
flags.

- **Success:** `src/routing.rs:tests::the_presence_gate_means_one_thing_under_every_flag` registers a
  hypothetical presence-gated LOCAL plugin and a hypothetical presence-gated DURABLE plugin and drives
  both flags across both phone verdicts.
  `src/routing.rs:tests::a_suppressed_phone_drops_only_the_presence_gated_leg` pins the ordinary path.
- **Failure sources:** An implementation that skipped the gate inside either flag's branch. The test
  comment states it keeps one of the two hypotheticals wrongly.
- **Fail direction:** Toward suppressing the gated leg. The gate composes as a filter rather than a
  branch.
- **Thresholds:** Not applicable.
- **Required side effects:** None.
- **Forbidden side effects:** None.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** `presence_gated` is the declaration; the ROSTER attaches it to `mobile`
  today and to nothing else.

### 5. A leg carries whether the operator is SHOWN something

Given a surviving leg

When `channel_plan` decides it survives

Then it sets `decorative = routing.presence_gated || routing.local` in the same reading, and carries it
on the `Leg` rather than letting a caller re-derive it.

- **Success:** `src/routing.rs:tests` states the flag at every call site through the `decorative` and
  `logged` helpers, so a plan that mislabelled one fails the test that names it. The consumer is
  `src/main.rs:replay_missed`, which returns early when
  `!decision.legs.iter().any(|leg| leg.decorative)`.
- **Failure sources:** A caller naming plugins or re-reading declarations to answer the same question,
  which `src/routing.rs:Leg` names as two copies of one policy, drifting.
- **Fail direction:** Toward not replaying. Nowhere the operator would see it is not a replay, and the
  empty plan is refused by the same line (`src/main.rs:replay_missed`).
- **Thresholds:** Not applicable.
- **Required side effects:** None from routing itself.
- **Forbidden side effects:** The mute is never applied as a filter over `decision.legs` afterwards
  (`src/main.rs:run_event` comment); it is an input to the decision.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** The hand-run check and the recap both build legs BY HAND with
  `decorative: false`, because no plan chose them (`src/main.rs:doctor_mode`,
  `src/main.rs:deliver_recap`).

### 6. Selection is vetted once, and only a vetted selection can be planned over

Given an operator config with `[plugins.<name>]` tables

When `Registry::enabled` runs

Then a name nothing registered is refused as `RegistryError::UnknownPlugin(name)` whether or not it is
switched on, a plugin switched on without the one it borrows a credential from is refused as
`RegistryError::Unsatisfied { plugin, needs }` naming both, and the result is a `Selection` in
REGISTRATION order whatever order the config listed.

- **Success:** `Selection`'s inner list is private with no public constructor, so a `Selection` can only
  come out of `Registry::enabled`, `Registry::all` or `Registry::core` (`src/registry.rs:Selection`).
  Fabricated registrations cannot reach routing.
- **Failure sources:** A config typo. Two plugins claiming one name, refused at registration as
  `RegistryError::Duplicate(name)`. A borrowed credential the config did not switch on: `REQUIRES` pairs
  `presence` with `hue`, because the room sensor reads the bridge through `[plugins.hue]`'s own address
  and key rather than declaring its own (`src/registry.rs:REQUIRES`).
- **Fail direction:** Loud. A typo'd plugin name that silently no-ops is a notification quietly turned
  off (`src/registry.rs` module comment). `build_registry` PANICS on a refused registration, which is
  safe on an always-exit-0 path because the only reachable refusal is a duplicate name in a compiled-in
  const.
- **Thresholds:** `ROSTER` has exactly 6 entries, `REQUIRES` exactly 1 pair and `CORE` exactly 2, all
  typed as fixed-size arrays (`src/registry.rs:ROSTER`, `src/registry.rs:REQUIRES`,
  `src/registry.rs:CORE`).
- **Required side effects:** None from the registry itself.
- **Forbidden side effects:** No IO.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Duplicate names are structurally impossible in a `Registry`.
- **Privacy:** Not applicable. Settings tables are not read here.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** One namespace serves both kinds, because both are selected by one
  `[plugins.<name>]` table.

### 7. What runs when the config could not be read

Given the outcome of loading the config

When `select_plugins` runs

Then a LOADED config is authoritative and selects what it enabled; a loaded config naming an unregistered
plugin selects the WHOLE ROSTER with a warning; a MISSING config selects the CORE silently; a config that
could not be read selects the CORE with a warning.

- **Success:** `src/registry.rs:select_plugins`. The two warning strings are exactly
  `pns: config error ({detail}); running every built-in plugin` and
  `pns: config error ({detail}); running the core plugins (mobile, macos-banner)`
  (`src/registry.rs:every_plugin_warning`, `src/registry.rs:core_warning`, the latter joining `CORE` with
  `", "`).
- **Failure sources:** An unreadable, malformed or invalid config file. A mistyped table name in an
  otherwise good file.
- **Fail direction:** Toward still delivering. A config error that silently turned every notification off
  would be the exact failure the config layer exists to refuse. Narrowing on one typo would cost a fully
  configured machine its durable paper trail and its lights.
- **Thresholds:** `CORE` is exactly `["mobile", "macos-banner"]`. hermes, hue and router are outside it
  because each needs a credential stood up (operator ruling 2026-08-31, recorded at
  `src/registry.rs:Registry::core`).
- **Required side effects:** The warning is RETURNED, not printed. The composition root prints it
  (`src/main.rs:run_event`, `src/main.rs:doctor_mode`).
- **Forbidden side effects:** No third answer that selects only the known names out of a config with one
  typo; `src/registry.rs:select_plugins` states it is deliberately not built.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** The detail carried into the warning comes from `ConfigError::detail()`, which the event
  path also prints. NOT ESTABLISHED: whether `detail()` can ever quote a secret value. I looked at
  `src/registry.rs:core_warning` and its one caller, and did not read `src/config.rs:ConfigError` in
  full; the config specification owns that question.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** A machine with no config has no durable route, which
  `src/main.rs:run_event` reads directly to decide whether a recap can be promised.

### 8. Every leg is handed one rendered event, produced once

Given parsed producer arguments and the sanitized pane

When `rendered_event` runs

Then it builds one `channels::Event` carrying `agent`, `state`, `project`, `branch`, `detail`, `title`,
`message`, `preview` and `pane`, and every leg in the dispatch is handed that same value.

- **Success:** `src/main.rs:rendered_event` is called once at the top of `src/main.rs:dispatch_legs`,
  before the loop. `tests/dispatch.rs:a_channel_is_handed_the_rendered_event_not_the_raw_arguments`
  asserts `title`, `message` and `preview` all arrive as non-empty rendered strings.
- **Failure sources:** Rendering per channel would let two destinations describe one event differently.
- **Fail direction:** `render::title` falls back to `pns` for an empty agent and `done` for an empty
  state; `render::message` falls back to the state, then to `done` (`src/render.rs:title`,
  `src/render.rs:message`, pinned by
  `src/render.rs:tests::title_falls_back_to_relay_and_done_when_the_caller_gave_neither` and
  `src/render.rs:tests::message_falls_back_to_done_when_it_was_given_nothing_at_all`). Nothing here can
  produce an error.
- **Thresholds:** `render::PREVIEW_MAX_CHARS` is **260** characters, counted in characters and not bytes.
  At exactly 260 the body passes through untouched
  (`src/render.rs:tests::a_body_at_the_cap_passes_through_untouched`); at 261 with no sentence end it is
  hard cut to 259 characters plus the ellipsis, total exactly 260
  (`src/render.rs:tests::one_character_over_the_cap_with_no_sentence_end_is_hard_cut_and_marked`). A
  sentence end at exactly 260 is where the cut lands; one at 261 is not used
  (`src/render.rs:tests::a_sentence_ending_exactly_at_the_cap_is_where_the_cut_lands`,
  `:a_sentence_ending_one_past_the_cap_is_not_used`). `render::DEFAULT_REPLY_MAX_CHARS` is **8000**
  characters, and `flatten_reply` keeps the TAIL: at the cap the text is whole, one character past it the
  first character is dropped (`src/render.rs:tests::a_reply_exactly_at_the_cap_is_left_whole`,
  `:one_character_past_the_cap_is_already_a_cut`).
- **Required side effects:** None. `render` is pure.
- **Forbidden side effects:** `flatten_reply` collapses EXACTLY four whitespace characters (space, tab,
  carriage return, newline). A form feed and a non-breaking space are content the turn wrote and survive
  verbatim (`src/render.rs:FLATTEN_WHITESPACE`, pinned by
  `src/render.rs:tests::whitespace_outside_the_four_is_content_the_turn_wrote_rather_than_a_separator`).
  A glob in a reply is never expanded
  (`src/render.rs:tests::a_reply_that_mentions_a_glob_keeps_it_verbatim`).
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Pure and deterministic.
- **Privacy:** The `detail` is operator-supplied text and travels to every destination. No secret is
  composed into any rendered field: the moshi token and the hermes key are read separately at the
  composition root and never reach `Event` (`src/main.rs:read_mobile`,
  `src/main.rs:plugin_settings(config, "hermes")`).
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** `Event::to_json` emits the channel contract's object with the per-leg
  `mode` added as an argument rather than stored on the struct, so one event serializes both ways
  (`src/channels/mod.rs:Event::to_json`, pinned by
  `src/channels/mod.rs:tests::the_mode_is_the_only_per_leg_field_so_one_event_serializes_both_ways`). The
  branch prefix is `branch: body`, never `(branch) body`, and that format is one across every channel
  (`src/render.rs:message`).

### 9. A pane with shell metacharacters is scrubbed once, before any leg runs

Given a producer-supplied `--pane` value

When the decision judges it unsafe (`!pane.is_empty() && !safety::pane_is_safe(pane)`, `src/engine.rs`)

Then `dispatch_legs` substitutes the empty string for every leg and prints one line to stderr:
`pns: dropped a pane id with shell metacharacters; no channel will focus a pane`.

- **Success:**
  `tests/dispatch.rs:a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event` asserts the
  delivered event's `pane` is `""` and that the warning reached stderr.
- **Failure sources:** A channel written in another language cannot be expected to share the guard, which
  is why the scrub is central (`src/main.rs:dispatch_legs`).
- **Fail direction:** Toward dropping the pane and delivering anyway. The notification still fires; only
  click-to-focus is lost.
- **Thresholds:** `safety::pane_is_safe` admits non-empty strings of ASCII alphanumerics plus `.`, `_`,
  `:` and `-` and nothing else (`src/safety.rs:pane_is_safe`). The colon is deliberately admitted because
  herdr's real ids carry one
  (`src/safety.rs:tests::a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane`).
- **Required side effects:** Exactly one stderr line, and only when at least one leg will run
  (`tests/dispatch.rs:a_scrub_warning_is_not_printed_when_no_channel_will_run`).
- **Forbidden side effects:** No unsafe pane value reaches any destination, argv, click string or deep
  link.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** One warning per dispatch, not one per leg.
- **Privacy:** The rejected pane value is NOT quoted in the warning.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** The same predicate guards the moshi deep link
  (`src/channels/moshi.rs:herdr_link`) and the banner click string consumes the already-sanitized value.

### 10. Native plugins win unless the channels directory was explicitly overridden

Given the dispatch site

When `native_first(channels_dir_overridden)` is asked

Then it answers `!channels_dir_overridden`: with `PNS_CHANNELS_DIR` set to a non-empty value, EXECUTABLES
win for every name; with it unset or empty, a native plugin wins and the executable fallback serves only
names with no compiled-in arm.

- **Success:** `src/channels/mod.rs:native_first`, pinned by
  `src/channels/banner.rs:tests::an_explicit_channels_dir_means_executables_win` and end to end by
  `tests/native.rs:the_banner_leg_delivers_natively_and_the_executable_channel_stays_silent`, which
  plants a decoy executable at the default path and asserts it never fires.
- **Failure sources:** An exported-but-blank `PNS_CHANNELS_DIR`. It is filtered out by
  `.filter(|dir| !dir.is_empty())` in `src/main.rs:dispatch_legs`, so a blank variable does NOT count as
  an override.
- **Fail direction:** Toward native. An empty or unset variable resolves to
  `$HOME/.local/libexec/pns/channels` via `src/main.rs:resolve_path`, which defaults like bash's
  `${VAR:-default}` because joining a filename to an empty path resolves into the current directory and
  quietly delivers nothing.
- **Thresholds:** Not applicable.
- **Required side effects:** None.
- **Forbidden side effects:** The precedence rule is decided once, in `src/channels/mod.rs`, and read
  once, in `src/main.rs:dispatch_legs`.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** The environment is read once per dispatch, not per leg.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** This rule is what lets the integration suite point the engine at stub
  directories and keep passing as each channel goes native (`src/channels/mod.rs` module comment).

### 11. An executable channel is handed the event on stdin and never reports

Given a planned leg whose native arm did not win

When `deliver` runs

Then it spawns `<channels_dir>/<leg.name>.sh` with stdin piped, writes the event JSON followed by a
newline, waits for the child, and answers `Delivery::Silent`. A spawn that failed answers
`Delivery::Unlaunched("could not launch the channel at {path} ({error}); nothing was sent")`.

- **Success:** `src/main.rs:deliver`. The newline is required because a channel reading one line with
  `read -r` gets nothing without it.
  `tests/dispatch.rs:a_channel_is_handed_the_rendered_event_not_the_raw_arguments` reads the recorded
  line back as JSON.
- **Failure sources:** A missing file, a file that is not executable, a directory in its place, an
  interpreter that is absent. All of them arrive as a spawn error.
- **Fail direction:** Never an error for the caller. A channel that is missing is simply not installed; a
  channel that ran and exited non-zero declined, and its exit status is DROPPED deliberately
  (`src/main.rs:deliver` doc comment). Pinned by
  `tests/dispatch.rs:an_absent_channel_is_simply_not_installed` and
  `tests/dispatch.rs:a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings`.
- **Thresholds:** **There is no deadline and no output ceiling on an executable channel.**
  `src/main.rs:deliver` calls `Command::spawn` and `child.wait()` directly, not `system::run_bounded`. A
  wedged channel blocks the dispatch indefinitely. This is the one delivery path in the crate with no
  bound; the banner's spawn is bounded at 5 s and 1 MiB and both HTTP legs carry their own deadlines.
- **Required side effects:** Exactly one write of the event JSON plus `\n` to the child's stdin.
- **Forbidden side effects:** The exit status must not become a caller-visible failure.
- **Timeout and cancellation:** None. See Thresholds. NOT ESTABLISHED: no test in `tests/dispatch.rs` or
  `tests/native.rs` pins the absence of a deadline here; I grepped both files for `sleep`, `deadline` and
  `timeout` against the channel-stub tests and found nothing exercising a hanging channel.
- **Idempotency and duplicates:** One spawn per leg per dispatch.
- **Privacy:** The event JSON reaches the child's stdin, which is the process's own pipe rather than argv
  or the environment. It carries `agent`, `state`, `project`, `branch`, `detail`, `title`, `message`,
  `preview`, `pane` and `mode`, and NO secret: neither the moshi token nor the hermes key is a field of
  `channels::Event` (`src/channels/mod.rs:Event`).
- **Process ownership and cleanup:** The engine owns the child and reaps it with `child.wait()`. Its
  stdin pipe is dropped after the write, which is what lets a child waiting on end of file exit. **stdout
  and stderr are INHERITED**: `src/main.rs:deliver` sets neither, so an executable channel can write onto
  the event's own stdout, which a harness hook reads. NOT ESTABLISHED: no test observes an executable
  channel writing to stdout; the stub channels all redirect into a file
  (`tests/support/mod.rs:Sandbox::without_config`), and
  `tests/dispatch.rs:an_absent_channel_is_simply_not_installed` asserts empty stdout only for a channel
  that never ran.
- **Compatibility contract:** The filename is always `<leg.name>.sh`. The JSON object shape and the
  `async`/`sync` wire words are the channel contract and must not be renamed
  (`src/routing.rs:ReportMode`).

### 12. The banner spawns `terminal-notifier` and nothing else

Given a `macos-banner` leg and a rendered event

When `BannerChannel::deliver` runs

Then it builds the argv
`-title <encoded title> -message <encoded preview> -sound default -activate <bundle id> -execute <click command>`,
in that pinned order, and spawns `terminal-notifier` by NAME through PATH.

- **Success:** `src/channels/banner.rs:notifier_args`, `src/channels/banner.rs:BannerChannel::deliver`.
  `src/channels/banner.rs:tests::a_delivered_leg_posts_the_banner_with_the_click_baked_in` asserts the
  title, the activate target and the click string.
  `src/channels/banner.rs:tests::nothing_but_the_notifier_is_ever_spawned` asserts exactly one spawn.
- **Failure sources:** `terminal-notifier` not on PATH, exiting non-zero, or outliving its deadline. All
  three arrive at the channel as `None` from the runner.
- **Fail direction:** `Delivery::Failed("banner FAILED (terminal-notifier did not run)")`, naming the
  binary because a line that only said "failed" would send the operator to the notification settings
  instead (`src/channels/banner.rs:BannerChannel::deliver`). Pinned by
  `src/channels/banner.rs:tests::a_spawn_that_answered_is_delivered_and_one_that_never_ran_names_the_notifier`.
- **Thresholds:** The spawn runs under `SystemCommandRunner`, which is
  `run_bounded(command, None, PROBE_DEADLINE, PROBE_READ_MAX)`: a **5 second** deadline and a **1 MiB**
  (1024 * 1024 bytes) stdout ceiling (`src/system.rs:SystemCommandRunner`,
  `src/system.rs:PROBE_DEADLINE`, `src/system.rs:PROBE_READ_MAX`). The reader asks for one byte PAST the
  ceiling and refuses the answer when the total exceeds it, so exactly at the cap succeeds and one byte
  over is no answer (`src/system.rs:run_bounded`). Past the deadline the child is killed and reaped and
  the answer is `None`.
- **Required side effects:** One `terminal-notifier` process. The click string is
  `{herdr} workspace focus {workspace}; {herdr} agent focus {pane}`, where the workspace is the pane id's
  prefix before the first colon, and the herdr path is absolute because the click runs in a bare launchd
  context (`src/channels/banner.rs:click_command`).
- **Forbidden side effects:** The banner must not read the frontmost application to judge suppression for
  itself. It used to, which meant two places could disagree about one event (`src/channels/banner.rs`
  module comment).
- **Timeout and cancellation:** 5 seconds, then `kill` followed by `wait` (`src/system.rs:run_bounded`).
  The poll loop uses `try_wait` with a backing-off interval rather than a blocking wait, because a child
  can close stdout and sleep (`src/system.rs:wait_until`).
- **Idempotency and duplicates:** One spawn per leg.
- **Privacy:** The title and the preview go on ARGV, which is world-readable in the process table. Both
  are operator-facing rendered text. No secret reaches this channel: `BannerChannel` holds only a runner,
  a terminal bundle id and a herdr path.
- **Process ownership and cleanup:** `run_bounded` owns the child, kills it at the deadline and always
  reaps it. stderr is `Stdio::null()`; stdout is a pipe read on a helper thread that closes under the
  child at the ceiling.
- **Compatibility contract:** `verbatim_argument` prefixes ONE unconditional backslash to every
  operator-facing value. terminal-notifier reads its options through NSUserDefaults' argument domain,
  which yields no string for a value whose first character is `(`, `[`, `{`, `-`, `<`, a double quote or
  a zero-width space, and strips one leading backslash from what survives (measured live 2026-08-12,
  probes P4 to P8). The encoding is unconditional rather than keyed to a character set, so the set can
  grow without a code change (`src/channels/banner.rs:verbatim_argument`, pinned by
  `src/channels/banner.rs:tests::every_case_in_the_matrix_encodes_to_its_exact_argv_value` and
  `:no_case_in_the_matrix_can_encode_to_a_value_the_parser_eats` over a 12-case matrix). An unknown
  terminal activates `com.mitchellh.ghostty` (`src/channels/banner.rs:DEFAULT_TERMINAL_BUNDLE_ID`, pinned
  by `src/channels/banner.rs:tests::an_unknown_terminal_activates_the_default`).

### 13. The mobile leg is refused before either seam when the table names no compiled-in backend

Given `[plugins.mobile]` switched on with a `type` that is absent, empty, or not `"moshi"`

When `dispatch_legs` reaches the `mobile` leg

Then it returns `Delivery::Failed(refused_backend_line(reason))` without choosing a seam at all, so the
refusal holds whether the native plugin or an executable channel would have won.

- **Success:** `src/main.rs:dispatch_legs` gates on `leg.name == "mobile" && mobile.refusal.is_some()`.
  The reason comes from `src/channels/moshi.rs:mobile_backend` via `src/config.rs:armed_mobile` and
  `src/main.rs:read_mobile`. The two reason strings are verbatim:
  `no type in [plugins.mobile]; the only type is "moshi"` and
  `[plugins.mobile] has type "<named>", which no compiled-in backend answers; the only type is "moshi"`
  (`src/channels/moshi.rs:mobile_backend`, pinned by
  `src/channels/moshi.rs:tests::the_table_has_to_name_a_backend_and_the_refusal_names_the_key` and
  `:a_type_no_compiled_in_backend_answers_is_refused_quoting_it`). The wrapper is
  `push SKIPPED -- {reason}; nothing was sent` (`src/channels/moshi.rs:refused_backend_line`).
- **Failure sources:** A `type` key left blank reads the same as absent, deliberately, matching the
  reading `home::router_settings` gives the `router` table's own `type`.
- **Fail direction:** Nothing is sent, and the operator is told once on stderr by
  `src/main.rs:read_mobile`: `pns: config error ({reason}); no card is pushed`.
- **Thresholds:** Not applicable.
- **Required side effects:** Exactly one stderr line from `read_mobile`, because the table is read once
  and the token, the toggle and the refusal come out of one verdict.
- **Forbidden side effects:** The gate must NOT sit on the token. It used to, and with an executable
  channel of the same name installed the card went out under a backend nobody named while stderr said "no
  card is pushed" (`src/main.rs:dispatch_legs` comment).
- **Timeout and cancellation:** Not applicable. A refused leg runs nothing, so there is no panic to catch
  and nothing to unwind.
- **Idempotency and duplicates:** One read of the table per process.
- **Privacy:** The refusal quotes the offending `type` value, never the `token`.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** A SWITCHED-OFF `[plugins.mobile]` table is inert: nothing at load and
  nothing on the event path refuses its `type` (operator ruling 2026-08-31). The hand-run check says it
  once instead, on stderr, in the words of `src/main.rs:disabled_backend_warning`:
  `pns: [plugins.<table>] is switched off and names no backend this binary answers (the only type is "<type>"); nothing refuses it until it is enabled`
  (`src/main.rs:disabled_backend_warnings`, pinned end to end by
  `tests/dispatch.rs:the_doctor_says_a_switched_off_table_names_no_backend_and_an_event_never_does`,
  which asserts the line reaches the hand-run check's stderr AND that the event path stays silent about
  it).

### 14. The mobile card is one HTTPS POST carrying the token in the body

Given an armed `[plugins.mobile]` table with a non-empty `token`

When `MoshiChannel::deliver` runs

Then it POSTs `{"token": ..., "title": ..., "message": <preview>}` plus an optional
`{"data": {"type": "url", "url": <deep link>}}` to the moshi URL, and reads a 2xx as delivered.

- **Success:** `src/channels/moshi.rs:webhook_body`, `src/channels/moshi.rs:MoshiChannel::deliver`,
  `src/channels/moshi.rs:UreqPost`. The card carries the PREVIEW, never the full message, because the
  phone card has a length ceiling (pinned by
  `src/channels/moshi.rs:tests::a_token_posts_once_to_the_url_with_the_preview_never_the_message`). The
  verdict on success is `Delivery::Delivered("pushed the card")`.
- **Failure sources:** No token; a refusal; an unreachable endpoint; a redirect.
- **Fail direction:** No token is `Delivery::Failed` with
  `push SKIPPED -- no moshi token in the config ([plugins.mobile] token); nothing was sent`
  (`src/channels/moshi.rs:NO_TOKEN_LINE`). Anything else is `Delivery::Failed` with
  `push FAILED (the moshi endpoint refused it or could not be reached)`, which deliberately does not pick
  a reason because the seam answers a bool. Neither sentence can reach an event's stdout: the leg is
  never `ReportOutcome`, because `ReportOutcome` is produced only under `--remote-only`, which keeps
  durable plugins, and `mobile` is not durable
  (`src/routing.rs:tests::no_plan_over_the_real_roster_hands_the_phone_or_the_banner_a_reporting_leg`).
- **Thresholds:** `POST_DEADLINE` is **10 seconds**, applied as ureq's `timeout_global`
  (`src/channels/moshi.rs:POST_DEADLINE`). `DELIVERED_STATUS` is the range **200..300**, so 200 through
  299 are delivered, 199 and 300 are not (`src/channels/moshi.rs:DELIVERED_STATUS`). `max_redirects(0)`,
  so a 3xx comes back as a RESPONSE rather than an error and
  `is_ok_and(|r| DELIVERED_STATUS.contains(...))` reads it as a failure; `is_ok` alone answered true for
  a card the endpoint bounced elsewhere (`src/channels/moshi.rs:UreqPost::post_json`, pinned by
  `src/channels/moshi.rs:tests::a_redirect_is_not_a_delivery_however_the_endpoint_dresses_it_up` and
  `:a_redirecting_endpoint_is_never_followed`). The deadline itself is pinned by
  `src/channels/moshi.rs:tests::the_deadline_fires_instead_of_parking_the_notification_path`.
- **Required side effects:** Exactly one POST per leg
  (`src/channels/moshi.rs:tests::a_token_posts_once_to_the_url_with_the_preview_never_the_message`).
- **Forbidden side effects:** No retry. No redirect followed, because following one would send the token
  to whatever host the endpoint names. No logging of the request.
- **Timeout and cancellation:** 10 seconds, whole-request. Nobody waits on the answer, so the deadline
  only bounds how long the process lingers.
- **Idempotency and duplicates:** No retry, so at most one card per leg per dispatch. The replay path can
  post a SECOND, synthetic, card about the same window; that is a separate event with title
  `pns · missed` (`src/main.rs:replay_missed`).
- **Privacy:** The token belongs in the request BODY and nowhere else: never argv, never a child's
  environment, never an error string. Pinned end to end by
  `tests/native.rs:native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output`, which
  asserts the token appears in the captured body and in neither stdout nor stderr, and by
  `tests/native.rs:a_dead_moshi_endpoint_is_silent_because_the_only_report_would_carry_the_token`. The
  deep link carries the sanitized pane id and nothing else.
- **Process ownership and cleanup:** No child process. One `ureq` agent per delivery.
- **Compatibility contract:** The deep link is `moshi://herdr?pane=<pane>`, built only when
  `safety::pane_is_safe` accepts the pane, whose charset is legal unencoded in a query value so nothing
  needs escaping (`src/channels/moshi.rs:herdr_link`, pinned by
  `:a_safe_pane_becomes_a_pane_precise_herdr_link`,
  `:a_pane_the_safety_guard_refuses_gets_no_link_rather_than_an_escaped_one` and
  `:the_link_needs_no_escaping_because_the_guard_already_bounded_its_charset`). The link is a DECORATION:
  no pane means no `data` object and the card ships exactly as it would without it
  (`:a_link_rides_as_the_one_url_action_and_no_link_leaves_the_slot_absent`,
  `:the_posted_card_links_to_the_origin_pane_and_a_paneless_one_ships_plain`). One `data` object carrying
  one `type` is a structural limit of the field, which is what makes a url action and an image action
  mutually exclusive. The plugin is named `mobile` for the DESTINATION and `type = "moshi"` names the
  backend, so a second backend is a value the operator writes rather than a second table
  (`src/registry.rs:ROSTER`).

### 15. The hermes record is one signed POST, and it says how it went

Given an enabled `[plugins.hermes]` table with a non-empty `key`

When `HermesChannel::deliver` runs

Then it builds `{"agent", "state", "project", "detail": <full message>}`, signs the exact body bytes with
a hash-based message authentication code (HMAC) over SHA-256 under the key, sends it as lowercase hex in
the `X-Webhook-Signature` header, and converts the outcome into a `Delivery` whose sentence names what
happened.

- **Success:** `src/channels/hermes.rs:hermes_body`, `:sign`, `:HermesChannel::deliver`,
  `:UreqSignedPost`. The body carries the FULL message rather than the preview, because Discord has no
  length ceiling
  (`src/channels/hermes.rs:tests::the_body_carries_the_full_message_because_discord_has_no_ceiling`). The
  signature covers the bytes actually sent, pinned on the wire by
  `tests/native.rs:sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent`, and the
  computation is checked against openssl's own HMAC for empty and unicode bodies
  (`src/channels/hermes.rs:tests::the_empty_and_unicode_bodies_match_openssls_own_hmac`).
- **Failure sources:** No key; a non-2xx status; no response at all; a request that was never put on the
  wire.
- **Fail direction:** Every one of them is `Delivery::Failed`, INCLUDING the no-key case, because from
  the record's point of view a missing entry reads the same as a refusal
  (`src/channels/hermes.rs:HermesChannel::deliver`). The four sentences are verbatim:
  `posted HTTP {code}`, `post FAILED HTTP {code}`, `post FAILED (curl reported no HTTP status at all)`,
  `post FAILED HTTP 000 (no response; is the hermes gateway up?)`, plus
  `post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent`
  (`src/channels/hermes.rs:outcome_line`, `:skipped_line`). Three of them are pinned end to end through
  the real binary by
  `tests/dispatch.rs:every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before` and
  the other two by `tests/native.rs` against the capture server.
- **Thresholds:** `DELIVERED_STATUS` is **200..300**, written once so the sentence and the verdict cannot
  disagree (`src/channels/hermes.rs:DELIVERED_STATUS`, `:delivered`). `ASYNC_DEADLINE` is **10 seconds**
  and is not configurable. `DEFAULT_SYNC_DEADLINE_SECS` is **5**; `MAX_SYNC_DEADLINE_SECS` is **86400**
  (one day). `PNS_REMOTE_TIMEOUT` is validated as a count: a garbled value falls back to 5, a value of
  **0** means NO deadline at all (curl's `-m 0`, explicit caller intent), and 86401 clamps to 86400
  rather than panicking ureq's deadline arithmetic (`src/channels/hermes.rs:remote_deadline`, pinned by
  `:the_sync_deadline_validates_and_defaults_to_five`,
  `:an_explicit_zero_deadline_is_no_deadline_like_curls_dash_m_zero`,
  `:an_absurd_deadline_clamps_to_a_day_instead_of_panicking_the_edge`). Which deadline applies is the ONE
  thing `ReportMode` still changes on this channel: `ReportOutcome` uses the sync deadline, `Silent` uses
  `ASYNC_DEADLINE` (pinned by `:sync_carries_the_validated_sync_deadline`).
- **Required side effects:** Exactly one POST, `content-type: application/json`, one
  `X-Webhook-Signature` header (`:a_key_posts_once_with_the_signature_of_the_exact_body_bytes`).
- **Forbidden side effects:** No redirect followed; following one would send the signed body to whatever
  host the gateway names (`:a_redirecting_gateway_is_the_final_answer_and_the_signed_body_stays_home`).
  No post at all when there is no key, in either mode
  (`:no_key_means_no_post_in_either_mode_and_the_verdict_is_a_failure`).
- **Timeout and cancellation:** As above. A `None` deadline is passed straight through rather than
  defaulted back into one.
- **Idempotency and duplicates:** No retry inside the channel. The recap path retries ONCE at a higher
  level, on a different route (behavior 16).
- **Privacy:** The signing key never reaches argv, a child's environment, or any printed line; the
  signature is computed in process over the exact body bytes (`src/channels/hermes.rs` module comment,
  pinned by `:the_key_never_rides_in_the_body_the_url_or_the_signature`). An empty key signs nothing,
  which is the not-set-up case (`:an_empty_key_signs_nothing_which_is_the_not_set_up_case`).
- **Process ownership and cleanup:** No child process.
- **Compatibility contract:** `PostOutcome::NoStatus` and `PostOutcome::NoResponse` are two different
  reports and must stay so: a malformed URL was never attempted, a closed port was
  (`:a_malformed_url_is_never_attempted_which_is_its_own_outcome`, `:a_closed_port_is_no_response`). An
  HTTP error status is unwrapped from `ureq::Error::StatusCode` rather than collapsed, matching the
  bash's missing `-f`. The default gateway is `http://127.0.0.1:8644/webhooks/pns`
  (`:the_default_url_is_the_local_gateway_route`).

### 16. A route is a NAME, and it swaps the gateway's final path segment

Given `--channel <route>` on an event, or a route named in config for the stale alert, or the `pns-recap`
route for a recap

When `hermes_url_for` runs

Then `PNS_HERMES_URL` wins outright if set non-empty; else an empty route posts to the default; else
`channel_url` swaps the default URL's final path segment for the route name; else the engine warns and
posts to the default.

- **Success:** `src/main.rs:hermes_url_for`, `src/channels/hermes.rs:channel_url`. Pinned on the wire by
  `tests/native.rs:the_stale_alert_posts_to_the_hermes_route_the_config_named`, which asserts both
  `POST /webhooks/priority HTTP/1.1` AND that the `Host` header is still `127.0.0.1:8644`, so a swap that
  took the base with it would be a different defect passing the test.
- **Failure sources:** A route name that could not safely become a path segment. A base URL with no `/`
  at all (`:a_base_without_a_path_yields_nothing_rather_than_a_bogus_url`).
- **Fail direction:** LOUD-WARD. An unusable name prints
  `pns: --channel "<name>" is not a usable route name; posting to the default route` to stderr and posts
  to the default anyway, because a misrouted notification on the loud route beats a silently dropped one
  (`src/main.rs:hermes_url_for`).
- **Thresholds:** `safety::route_name_is_usable` admits non-empty strings of ASCII alphanumerics plus `-`
  and `_` and nothing else (`src/safety.rs:route_name_is_usable`). The empty string, `a/b`, `../x`,
  `a b`, `a?x=1`, `a#f`, `.`, `a\nb`, `%2e%2e` and `café` are all refused, and refused through
  `channel_url` too
  (`src/channels/hermes.rs:channel_url_tests::one_rule_judges_a_route_name_wherever_it_is_read`,
  `:a_name_that_could_not_be_a_path_segment_is_refused_not_glued`).
- **Required side effects:** The one stderr warning on an unusable name. NOT ESTABLISHED: no test pins
  that warning line from `hermes_url_for`. I grepped `tests/` for `usable route name` and the only hit is
  `tests/dispatch.rs` line 1385, which pins the analogous sentence from `src/home.rs` for
  `stale_alert_channel`, not the `--channel` one.
- **Forbidden side effects:** Names, not URLs, cross the command line interface. The gateway and its
  route table stay the single source of truth in the hermes config
  (`src/channels/hermes.rs:channel_url`).
- **Timeout and cancellation:** Inherited from behavior 15.
- **Idempotency and duplicates:** A recap posts to `pns-recap` first and, if that is refused, ONCE more
  to the default with an appended line saying why it landed there:
  `(the pns-recap route did not take this, so it landed on the default route instead)`
  (`src/main.rs:THREAD_UNAVAILABLE`, `src/main.rs:refused`). Pinned by
  `tests/native.rs:a_recap_the_thread_route_will_not_take_falls_back_to_the_default_and_says_so`, which
  asserts exactly two POSTs, to `/webhooks/pns-recap` then `/webhooks/pns`, and that the second body
  carries the explanation.
- **Privacy:** The rejected route name IS quoted in the warning. It is operator-supplied and not a
  secret.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** The gateway has no route named `alert`; the default is where an event with
  no route named goes (`src/main.rs:hermes_url_for`). `--channel` affects the hermes leg ONLY:
  `dispatch_legs` passes `hermes_url_for(&event.channel)` to the hermes constructor and nowhere else.

### 17. A delivery outcome is a variant, and the mode alone decides who hears it

Given one leg's `Delivery`

When `Delivery::line_for(mode)` is asked

Then `Delivered` and `Failed` both yield their sentence on a `ReportOutcome` leg and nothing on a
`Silent` leg; `Silent` and `Unlaunched` yield nothing in BOTH modes.

- **Success:** `src/channels/mod.rs:Delivery::line_for`, pinned exhaustively by
  `src/channels/mod.rs:tests::either_verdict_reaches_the_operator_on_a_reporting_leg_and_nothing_does_otherwise`.
  The one place a line reaches the operator is `src/main.rs:run_event`, which prints
  `println!("pns: {line}")`, and `src/main.rs:deliver_recap`, which prints the identical form.
- **Failure sources:** A caller keying on English (searching for "FAILED" in the text) rather than on the
  variant. `src/channels/mod.rs:Delivery` states one of those has already cost this repo a defect.
- **Fail direction:** Toward silence on the notification path. `Unlaunched` is swallowed in both modes
  because the common case is a channel nobody installed
  (`tests/dispatch.rs:an_absent_channel_is_simply_not_installed` removes `channels/hermes.sh` and asserts
  stdout is exactly empty). NOT ESTABLISHED: that test's own comment claims "hermes runs sync on this
  path", but the command it builds passes no `--remote-only`, so by `src/routing.rs:channel_plan` the leg
  is `Silent` and the swallowing it demonstrates is the Silent arm rather than the `ReportOutcome` arm.
  The `ReportOutcome` arm is pinned in the unit test above and by
  `tests/dispatch.rs:a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`
  through the hand-run check, which reads the VARIANT rather than a printed line.
- **Thresholds:** Not applicable.
- **Required side effects:** The `pns: ` prefix is added at the print site, never carried inside a
  channel's sentence, so a caller that labels a line with the plugin's name does not have to unpick a
  prefix out of the middle of its own (`src/channels/mod.rs:Delivery`).
- **Forbidden side effects:** A channel decides HOW to deliver and whether it can, never WHETHER it
  should fire, and it must never fail the caller. Nothing in `Delivery` is an error path.
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** `line_for` consumes the `Delivery`; callers clone
  (`src/main.rs:run_event`).
- **Privacy:** A sentence must not quote a secret. See behaviors 14 and 15.
- **Process ownership and cleanup:** Not applicable.
- **Compatibility contract:** Four variants, and `Silent` versus `Unlaunched` must stay distinct: a
  caller that cannot tell them apart calls an empty channels directory a set of successful sends, which
  is exactly what a hand-run check did before `Unlaunched` existed
  (`src/channels/mod.rs:Delivery::Unlaunched`, pinned end to end by
  `tests/dispatch.rs:a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`,
  which asserts `0 sent, 3 failed, 2 skipped` for a directory holding no channel at all).

### 18. One leg's failure costs no other leg its turn

Given a plan of several legs where an early one fails

When `dispatch_legs` runs

Then every remaining leg is still dispatched, and the failure is one entry in the returned
`Vec<(Leg, Delivery)>`.

- **Success:** `src/main.rs:dispatch_legs` uses `.map(...).collect()` with no `?` and no early return,
  and every channel is CONSTRUCTED before the first delivery so a leg cannot be lost to a sibling's
  refusal. Pinned by
  `tests/dispatch.rs:a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings` and,
  through the whole native stack, by
  `tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`,
  which deliberately fails the FIRST channel in delivery order (`mobile`, no token), asserts the banner
  behind it both delivered AND received its payload (`notifier.args` exists), and asserts hermes at the
  tail still got its turn.
- **Failure sources:** A `?` in the loop. A constructor that ran lazily inside the loop and could refuse.
- **Fail direction:** Toward delivering the rest.
- **Thresholds:** Not applicable.
- **Required side effects:** One outcome per leg, in the plan's order.
- **Forbidden side effects:** No leg may be skipped because of a sibling.
- **Timeout and cancellation:** Legs run sequentially, each under its own bound (5 s for the banner, 10 s
  or the sync deadline for the HTTP legs, unbounded for an executable channel). There is no aggregate
  deadline across a dispatch.
- **Idempotency and duplicates:** One dispatch per leg.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Each leg cleans up its own child; see behaviors 11 and 12.
- **Compatibility contract:** `dispatch_legs` RETURNS its outcomes and prints nothing. Two callers
  spelling one report two ways is exactly what a returned value is for (`src/main.rs:dispatch_legs` doc
  comment).

### 19. A channel that panics costs one leg, never the run

Given a leg whose channel unwinds

When `dispatch_legs` dispatches it

Then `std::panic::catch_unwind` converts the panic into
`Delivery::Failed("the {name} channel PANICKED; nothing was sent")` and the loop continues.

- **Success:** `src/main.rs:dispatch_legs`. The same shape guards the pulse (`src/main.rs:pulse_outcome`,
  `the pulse PANICKED; no room was signalled`).
- **Failure sources:** Any unwinding panic inside a native channel's deliver.
- **Fail direction:** One leg fails; the remaining legs and, in a hand-run check, the rest of the census
  still run. A census that ended early is read as a report that finished.
- **Thresholds:** Not applicable.
- **Required side effects:** The default panic hook still prints its own trace to stderr; that is left
  alone deliberately, because silencing it process-wide would hide every other panic in the binary.
- **Forbidden side effects:** **The panic payload text is never quoted**, because a panic message is
  written for a developer and may quote anything the channel was holding (the token, the key).
- **Timeout and cancellation:** Not applicable.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** See Forbidden side effects. The sentence carries the plugin NAME and nothing else.
- **Process ownership and cleanup:** A panic inside a bounded spawn still leaves `run_bounded`'s own
  kill-and-reap unreached; NOT ESTABLISHED whether a child can be orphaned that way. I looked at
  `src/main.rs:dispatch_legs` and `src/system.rs:run_bounded` and found no drop guard, and no test
  exercises a panicking channel.
- **Compatibility contract:** The catch sits at the one site that dispatches any leg at all, so the
  backend refusal and the panic catch are the same fence.

### 20. A notification never fails the work it reports on

Given any event delivered through the producer path

When the process finishes, whatever every destination did

Then it exits 0.

- **Success:** `src/main.rs:main` falls through to `event_mode(&argv)`, which returns `()`; no
  `std::process::exit` is on the event path (`src/main.rs:main` lines 48 to 155). Every mode that DOES
  set a code (`pulse`, `quiet`, `doctor`, `recap`, `daemon`, `lights`, `loop`, `nag`, `setup`, `gate`,
  `hook`) is reached by argv[1] before the event path.
- **Failure sources:** A non-UTF-8 byte in argv, which would panic `std::env::args()`. It is avoided by
  one lossy read through `args_os` (`src/main.rs:main`), pinned by
  `tests/dispatch.rs:a_non_unicode_argument_never_breaks_the_exit_zero_edge`.
- **Fail direction:** Always exit 0 for an event. The exceptions are stated and narrow: a word that names
  no command is a typo, not an event, and earns usage on stderr and exit 2 (`src/main.rs:main`); the
  hand-run check exits 1 when a channel failed
  (`tests/dispatch.rs:a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`);
  and `pns recap` exits 0 even when the gateway refused, because that contract is the binary's and not
  the mode's to break
  (`tests/native.rs:a_recap_the_gateway_refused_says_so_out_loud_and_still_exits_zero`).
- **Thresholds:** Not applicable.
- **Required side effects:** None. Silence is the ordinary outcome of a successful event.
- **Forbidden side effects:** No destination's failure may propagate as a non-zero status, a panic that
  escapes, or an early return that skips a sibling. `catch_unwind` (behavior 19) and the no-`?` loop
  (behavior 18) are the two mechanisms.
- **Timeout and cancellation:** Each leg is individually bounded except an executable channel; see
  behavior 11's Thresholds for the one gap.
- **Idempotency and duplicates:** Not applicable.
- **Privacy:** Not applicable.
- **Process ownership and cleanup:** Every spawned child is either reaped by `run_bounded` or by
  `deliver`'s `child.wait()`.
- **Compatibility contract:** The always-exit-0 contract governs EVENT deliveries, and a word naming no
  command never becomes one, so refusing an unknown argv[1] contradicts nothing (`src/main.rs:main`).

### 21. Three callers dispatch legs, and each spells its own report

Given `dispatch_legs` returning `Vec<(Leg, Delivery)>`

When each caller consumes it

Then the event path prints only what a reporting leg said, prefixed `pns: `; the hand-run check labels
EVERY outcome with its plugin's name and prints the lot; the missed-notification replay prints nothing at
all.

- **Success:** `src/main.rs:run_event` (prints via `line_for`), `src/main.rs:doctor_mode` (maps every
  `Delivery` variant onto a `doctor::Outcome` by leg NAME and prints one line per registered plugin),
  `src/main.rs:replay_missed` (discards the result with `let _ =`), `src/main.rs:deliver_recap` (prints
  via `line_for` and returns the outcomes so `src/main.rs:refused` can decide the route fallback).
- **Failure sources:** A positional pairing in the hand-run check would print one channel's verdict under
  another's label. It pairs by name for exactly that reason (`src/main.rs:doctor_mode`), and an absent
  pairing reports `the leg was never dispatched` rather than claiming a send.
- **Fail direction:** The hand-run check maps `Failed` and `Unlaunched` alike to
  `doctor::Outcome::Failed` and `Silent` to `doctor::Outcome::SentUnreported`, which is an executable
  channel that RAN. It exits 1 when anything failed.
- **Thresholds:** Not applicable.
- **Required side effects:** The hand-run check builds its legs with `mode: ReportMode::ReportOutcome`
  because the operator is standing there waiting, and with `decorative: false` because no plan chose
  them. It passes NO pane, because a click target cannot be verified without a human clicking it. It
  BYPASSES every suppression gate, which is the point of a check (`src/main.rs:doctor_mode`, pinned by
  `tests/dispatch.rs:the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`).
- **Forbidden side effects:** The replay must not re-decide. A synthetic event fed back through
  `run_event` would take a second decision, write a second `decision ring` line, fire a second pulse, and
  re-`journal` itself forever (`src/main.rs:replay_missed`). It dispatches THIS decision's own legs
  verbatim instead.
- **Timeout and cancellation:** Inherited per leg.
- **Idempotency and duplicates:** The replay's accepted consequence is stated: the durable leg is among
  the legs it reuses, so the summary is posted to a log that already holds every entry in it. That is a
  duplicate in content and a new fact in kind (`src/main.rs:replay_missed`).
- **Privacy:** The replay's synthetic event carries an empty project, branch, channel and pane, so no
  stale pane id from an hour ago reaches a destination.
- **Process ownership and cleanup:** The Discord half of a recap runs in its own spawned process before
  the card, so the card can truthfully say whether there is a recap to point at
  (`src/main.rs:replay_missed`).
- **Compatibility contract:** All three go through the engine's own wiring, down to the constructors and
  `dispatch_legs`, so a hand-run check cannot report green through a path an event would not use
  (`src/main.rs:doctor_mode` doc comment).

______________________________________________________________________

## Gaps

Recorded here as well as inline, so they can be closed deliberately.

- `NOT ESTABLISHED:` no test pins that an executable channel has no deadline or output ceiling. I grepped
  `tests/dispatch.rs` and `tests/native.rs` for a hanging or high-volume stub channel and found none. The
  absence is read from `src/main.rs:deliver`, which calls `Command::spawn` and `child.wait()` directly
  rather than `system::run_bounded`.
- `NOT ESTABLISHED:` no test observes an executable channel writing to the event's stdout or stderr.
  `src/main.rs:deliver` configures neither, so both are inherited; every stub channel in
  `tests/support/mod.rs:Sandbox::without_config` redirects into a file instead.
- `NOT ESTABLISHED:` no test pins the
  `pns: --channel "<name>" is not a usable route name; posting to the default route` warning from
  `src/main.rs:hermes_url_for`. The only `usable route name` assertion in `tests/` is `tests/dispatch.rs`
  line 1385, which covers `src/home.rs`'s analogous sentence for `stale_alert_channel`.
- `NOT ESTABLISHED:` no test exercises a panicking channel, so `src/main.rs:dispatch_legs`'s
  `catch_unwind` arm and its `the {name} channel PANICKED; nothing was sent` sentence are unpinned. I
  grepped `tests/` for `PANICKED` and found no hits.
- `NOT ESTABLISHED:` whether a panic inside a bounded spawn can orphan the child. I found no drop guard
  in `src/system.rs:run_bounded` and no test covering it.
- `NOT ESTABLISHED:` whether `ConfigError::detail()`, whose text is interpolated into the two
  `select_plugins` warnings and printed, can ever quote a secret value. I read
  `src/registry.rs:core_warning` and its callers but not `src/config.rs:ConfigError` in full; the config
  specification owns that question.
- `NOT ESTABLISHED:` no test in `tests/dispatch.rs` covers a channel file that exists but is not
  executable, or a directory standing in for one. I grepped for `set_permissions` and `chmod` in
  `tests/dispatch.rs`; every hit is about the state directory, the `decision ring` or the `journal`, not
  about a channel file. From `src/main.rs:deliver` both would arrive as a spawn error and therefore as
  `Delivery::Unlaunched`.

______________________________________________________________________

## Glossary

| Term                          | Defining symbol                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------- |
| leg                           | `src/routing.rs:Leg`                                                            |
| plan                          | `src/routing.rs:channel_plan`                                                   |
| report mode                   | `src/routing.rs:ReportMode`                                                     |
| decorative                    | `src/routing.rs:Leg::decorative`                                                |
| routing declaration           | `src/registry.rs:Routing`                                                       |
| plugin kind                   | `src/registry.rs:PluginKind`                                                    |
| registration                  | `src/registry.rs:Registration`                                                  |
| roster                        | `src/registry.rs:ROSTER` (built by `src/registry.rs:roster`)                    |
| core                          | `src/registry.rs:CORE` (selected by `src/registry.rs:Registry::core`)           |
| selection                     | `src/registry.rs:Selection`                                                     |
| selection policy              | `src/registry.rs:select_plugins`                                                |
| event                         | `src/channels/mod.rs:Event`                                                     |
| delivery outcome              | `src/channels/mod.rs:Delivery`                                                  |
| dispatch precedence           | `src/channels/mod.rs:native_first`                                              |
| title                         | `src/render.rs:title`                                                           |
| message                       | `src/render.rs:message`                                                         |
| preview                       | `src/render.rs:preview`                                                         |
| preview cap                   | `src/render.rs:PREVIEW_MAX_CHARS`                                               |
| clipped                       | `src/render.rs:clipped`                                                         |
| flatten                       | `src/render.rs:flatten_reply`                                                   |
| click command                 | `src/channels/banner.rs:click_command`                                          |
| verbatim argument             | `src/channels/banner.rs:verbatim_argument`                                      |
| mobile backend                | `src/channels/moshi.rs:mobile_backend`                                          |
| deep link                     | `src/channels/moshi.rs:herdr_link`                                              |
| webhook body                  | `src/channels/moshi.rs:webhook_body`                                            |
| post outcome                  | `src/channels/hermes.rs:PostOutcome`                                            |
| signature                     | `src/channels/hermes.rs:sign`                                                   |
| route                         | `src/channels/hermes.rs:channel_url` (resolved by `src/main.rs:hermes_url_for`) |
| sync deadline                 | `src/channels/hermes.rs:remote_deadline`                                        |
| dispatch                      | `src/main.rs:dispatch_legs`                                                     |
| leg delivery                  | `src/main.rs:deliver_leg`                                                       |
| executable channel invocation | `src/main.rs:deliver`                                                           |
| executable discovery          | `src/main.rs:resolve_path`, `src/main.rs:executable_in_path`                    |
| mobile verdict                | `src/main.rs:Mobile`, `src/main.rs:read_mobile`                                 |
| rendered event                | `src/main.rs:rendered_event`                                                    |
| bounded spawn                 | `src/system.rs:run_bounded`                                                     |
| pane guard                    | `src/safety.rs:pane_is_safe`                                                    |
| route name guard              | `src/safety.rs:route_name_is_usable`                                            |
| delivery plan                 | `src/surface.rs:DeliveryPlan`                                                   |
| check kind                    | `src/doctor.rs:CheckKind`                                                       |
