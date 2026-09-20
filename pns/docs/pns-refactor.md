# pns refactor

Agreed changes to pns, one per line. Add a line for each new agreed change.

## Background

- The flag path names the sender with `--agent` (`crates/pns/src/legacy/argv.rs:166`). The name is shown
  in the notification title as `agent · state · project` (`crates/pns-domain/src/render.rs:15`) and is
  stored in the ledger and journal.
- The approval reminder ("nag") is armed only when that name is exactly `"claude"`
  (`crates/pns-application/src/arm_nag.rs:52`, constant at `arm_nag.rs:144`). It runs only on the
  approval hook path (`crates/pns/src/moshi_submission.rs:112`) and only when `[nag] after_secs` is
  non-zero.
- On the hook path the name comes from `PNS_AGENT`, which defaults to `claude` when unset
  (`crates/pns/src/hook_dispatch.rs:23`). A missing variable therefore turns on Claude-only behavior.
- The gate exists because Claude Code sends an "answered" signal that clears the reminder, and Codex does
  not, so a Codex reminder could not tell when to stop.
- The JSON producer API (`pns submit --json`) already calls the sender `producer` and already has an
  explicit switch for approvals (`interaction: {kind: "await_decision"}`).

## Changes

1. Stop using the sender's name as a feature switch: delete the `event.agent != "claude"` check in
   `crates/pns-application/src/arm_nag.rs:52` and the `CLAUDE_AGENT` constant, and arm the reminder only
   when the resolved reminder setting (see changes 2 to 4) is on. The name goes back to being a label
   only.
1. Add an explicit per-call switch on the approval hook path: `--remind` turns the reminder on for that
   call and `--no-remind` turns it off, overriding config.
1. Add a config setting per producer: `[producer.<name>] remind = true|false` in
   `~/.config/pns/config.toml`. This is still a match on a name, but one the operator wrote on purpose
   and can see. The timing stays in the `[remind]` table.
1. Resolve the setting in this order, most specific first: the per-call flag, then the producer's config
   entry, then the built-in default.
1. The built-in default is off. An unset `PNS_AGENT` (which still defaults to `claude` in
   `hook_dispatch.rs:23`) must no longer turn on any reminder by itself.
1. Each harness's own hook wiring passes `--remind`, rather than config naming harnesses: only the
   harness knows whether it sends an "answered" signal. Claude Code's declarations in
   `private_dot_claude/modify_settings.json` pass it today.
1. Warn when a reminder is turned on for a producer that sends no "answered" signal (Codex today, see
   `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh`). The warning goes to stderr, never
   stdout, because Claude Code parses the hook's stdout. pns must never keep reminding forever: the
   `[remind]` staleness cap stays the hard stop.
1. Rename the `--agent` flag to `--producer`, so the flag path and the JSON `producer` field use the same
   word.
1. Remove `--agent` entirely, with no alias: pns is pre-1.0 and has no users besides the operator. Move
   every caller to `--producer` in the same change:
   `lights/crates/lights-adapters/src/notification.rs:49` (and its tests and
   `lights/crates/lights/tests/argument_surface/expectations.py:176`),
   `crates/pns-adapters/src/process/shell_event.rs:13`, the failed-command text built in
   `crates/pns/src/command_failures.rs:198`, and the examples in
   `docs/superpowers/specs/2026-09-06-lights-design.md` and
   `docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`.
1. Update the tests that pin the old name match (`crates/pns-application/src/arm_nag/tests`) to pin the
   new precedence instead: flag over config, config over default, default off, and the stderr warning.
1. Update the specs that describe the old behavior: `docs/specs/blocking-approval.md` (the `PNS_AGENT`
   section near line 152) and `docs/superpowers/specs/2026-09-05-pns-behavioral-specification.md` (S073).
1. Merge `kind` and `class` into one field named `delivery_class`. Today `--kind agent|health` (flag
   only) picks the route, with `health` taking `[routes] urgent` (`crates/pns-domain/src/routes.rs:105`),
   and `class` (JSON only) lets a message through mute when listed in `[delivery] bypass_silence_classes`
   (`crates/pns/src/event_flow/execution.rs:17`). Both answer "how should this message be delivered", so
   one field replaces them. `class` alone did not say what the label controls.
1. Pass the delivery class the same way on both paths: `--delivery-class <name>` on the command line and
   `"delivery_class": "<name>"` in the JSON. Remove `--kind` and the JSON `class` field.
1. Define what each delivery class does in config, one table per class, singular table name, with no
   class words hardcoded in pns: `[delivery_class.<name>]` with `route = "<route>"` and
   `bypass_mute = true|false`, e.g. `[delivery_class.security]` and `[delivery_class.health]`. This
   replaces the hardcoded `agent`/`health` words in `crates/pns-domain/src/routes.rs` and the
   `[delivery] bypass_silence_classes` list.
1. Move today's callers to `delivery_class`: `uu/crates/uu-adapters/src/alert.rs` (passes
   `--kind health`) and `posture/crates/posture-adapters/src/producer/request.rs:60` (sets
   `class = "security"`), plus posture's copy of the wire contract in
   `posture/crates/posture-producer-wire/` and both `request-v1.json` fixtures.
1. The command line (`pns send --<field>`) and the JSON API (`pns send --json`) share one field list.
   Each field has one name and one set of values; the flag is the JSON name with dashes (`delivery_class`
   becomes `--delivery-class`). The list: `producer` (required), `state`, `detail`, `project`, `branch`,
   `pane`, `route`, `delivery_class`, `elapsed`, `scope`, `request_id`, `session`, `remind`,
   `require_delivery`.
1. `state` replaces the JSON `signal: {kind: ...}` wrapper and the free-text `--state`. Both take the
   same closed set: `done`, `failed`, `blocked`, `resolved`, `observation`, `progress`. JSON `succeeded`
   becomes `done`, and `needs_attention` and `approval_requested` become `blocked`, because pns already
   maps both to `blocked` (`crates/pns/src/event_flow/submit/mapping.rs:7`) and nothing tells them apart.
1. `observation` and `progress` behave the same on both paths. Today JSON sends them as quiet updates
   (`Attempt::Observation`, `mapping.rs:13`), while `--state observation` on the command line is sent as
   an ordinary message (`crates/pns/src/invocation.rs`, `event_mode` always uses `Attempt::First`).
1. Flatten `context`: the JSON takes `project`, `branch` and `pane` at the top level, like the flags.
1. Rename `--channel` to `--route`, matching the JSON `route`.
1. Rename the JSON `elapsed_secs` to `elapsed`, matching `--elapsed`. Both take a duration (`90s`).
1. Replace `--local-only` and `--remote-only` with `--scope automatic|local_only|remote_only`, matching
   the JSON `scope`. This also removes today's "both flags given" refusal.
1. Add `--request-id` to the command line. It stays optional on both paths; pns makes one when it is
   missing. Today the JSON requires it and the flags have no way to pass it.
1. Add `--session` to the command line, and flatten the JSON `session: {id, turn}` object to a plain
   `session` id.
1. Add `remind` to the JSON, matching the command line: `"remind": true` uses the configured delay,
   `"remind": "<duration>"` (e.g. `"5m"`) sets the delay for that request, and `"remind": false` matches
   `--no-remind`.
1. Remove `require_delivery` from both paths. With the exit code always reporting delivery (see the
   exit-code change), an opt-in for that answer is a second switch for a thing that is now always on. The
   always-exit-0 contract stays where it belongs, on the harness hook paths (`pns hook <event>`), which
   must never fail the turn they report on.
1. Remove `--long-running`. pns already derives it from the elapsed time (`mapping.rs`,
   `elapsed_secs >= DEFAULT_LONG_SESSION_SECS`), so the flag is a second way to say the same thing, and
   today it is refused when combined with `--elapsed`.
1. Remove the JSON fields that change nothing: `event` and `occurred_at` (stored, never read, apart from
   the `github` extension's own `occurred_at`), and `session.turn`.
1. Remove the JSON `interaction: {kind: "await_decision"}` field until it does something: `pns submit`
   always answers it with "no opinion" (`crates/pns/src/event_flow/submit.rs:86`).
1. Keep two things JSON-only on purpose: `schema` (the version tag) and `extensions` (free-form data,
   which has no clean flag form; `extensions.github` drives the GitHub lamps).
1. Use the same exit codes on both paths, and a partial delivery is a failure: `0` only when every
   destination delivered, `1` when any destination failed (today's `degraded` included) or nothing
   delivered at all, `2` bad input. A `0` for a partial delivery hides a broken destination from the tool
   that sent the message. Today the command line returns `1` only with `--require-delivery`
   (`crates/pns/src/legacy.rs:46`) and JSON never returns `1` (`submit.rs:50`).
1. Handle bad input the same way on both paths, strictly: an unknown flag or field, a missing value, or a
   value outside a closed set is refused and named, with exit `2`. Today the command line skips unknown
   flags silently and only warns about missing values (`crates/pns/src/legacy/argv.rs`), while the JSON
   refuses bad values and ignores unknown fields with a diagnostic.
1. Move every caller and copy of the contract to the new names in the same change: `uu`, `posture` and
   `posture/crates/posture-producer-wire/`, `lights`, the shell notifier
   (`crates/pns-adapters/src/process/shell_event.rs`), the harness hook installers, and both
   `request-v1.json` fixtures.
1. A per-call `--remind` beats config in every case, `[nag] after_secs = 0` included: the calling tool is
   closer to the operator, so its explicit choice wins.
1. Rename the reminder wait `[nag] after_secs` to `[remind] delay`, which takes a duration
   (`delay = "5m"`).
1. `--remind` takes an optional value joined with `=`, the convention of `git log --color[=<when>]`,
   `git commit --gpg-sign[=<key-id>]` and GNU `ls --color[=WHEN]`. `--remind` alone uses the configured
   `delay`; `--remind=<duration>` sets the delay for that call and beats config. A value separated by a
   space (`--remind 5m`) is never read as the delay, so the next token is never swallowed. No second
   delay flag exists.
1. `--remind` alone fails when config sets no `delay` (unset): pns refuses the request as bad input (exit
   `2`) and says to set `delay` or pass `--remind=<duration>`, rather than guessing a delay.
1. Every duration in pns is written as a number plus a unit: `250ms`, `30s`, `5m`, `2h`. This holds on
   the command line, in the JSON and in config. A bare number is refused, because one reader takes `300`
   as seconds and the next as minutes.
1. Names of duration fields drop their unit suffix, since the value now carries it: `elapsed`, `delay`,
   and in `daemon schedule`, `--in` and `--every` take durations and `--until +<duration>` takes one for
   its relative form. Config keys follow: `after_secs`, `desk_stale_after_secs`, `duration_ms`,
   `flare_ms`, `give_up_after_secs`, `lease_timeout_secs`, `max_age_secs`, `poll_secs`, `refresh_secs`,
   `retry_base_secs`, `stale_after_secs`, `submit_deadline_secs`, `summarizer_deadline_secs` and
   `threshold_secs` (listed in `crates/pns-adapters/src/config/schema.rs`) lose the suffix and take
   duration strings.
1. One duration parser serves every field. Promote `parse_duration` out of
   `crates/pns-domain/src/quiet.rs:9`: add the `ms` unit, make its error text name the field instead of
   saying "quiet duration", and give each field its own allowed range instead of the shared 1s to 24h.
1. Points in time are not durations and keep a timestamp: `recap --since`/`--until` and the absolute form
   of `daemon schedule --until`.
1. Rename the reminder everywhere from "nag" to "remind", matching `--remind`: the `pns nag` subcommand
   becomes `pns remind`, the `[nag]` config table becomes `[remind]`, and the code follows
   (`crates/pns/src/command_nag.rs`, `crates/pns-application/src/arm_nag.rs`,
   `crates/pns/src/nag_schedule_runtime.rs`, `crates/pns-adapters/src/config/nag.rs`).
1. Use one word for muting by hand, "mute", and keep "quiet hours" only for the schedule:
   `pns quiet [<duration>|off]` becomes `pns mute [<duration>|off]`,
   `pns lights quiet [<place> [<duration>|off]]` becomes `pns lights mute [<place> [<duration>|off]]`,
   and the delivery class key is `bypass_mute` rather than `bypass_silence`. Code and comments that say
   "quiet" or "silence" for a hand-set mute follow (`crates/pns/src/command_quiet.rs`,
   `crates/pns/src/command_lights.rs`, `crates/pns-domain/src/quiet.rs`).
1. Rename `pns shell end --exit` to `--exit-code` (`crates/pns/src/shell/parse.rs:28`) and update the
   caller in `dot_bashrc.tmpl`.
1. Flags that take a point in time say so: `pns recap --since`/`--until` become
   `--since-epoch`/`--until-epoch` (`crates/pns-application/src/build_return_recap/window.rs`, caller
   `crates/pns-adapters/src/recap_child.rs:47`), and the absolute form of `pns daemon schedule --until`
   becomes `--until-epoch`, leaving `--until +<duration>` for the relative form
   (`crates/pns/src/command_daemon.rs:73`).
1. Rename the `PNS_AGENT` environment variable to `PNS_PRODUCER`, matching `--producer` and the
   `PNS_PRODUCER` pns already passes to executable plugins
   (`crates/pns-adapters/src/destinations/executable.rs:112`). Callers:
   `crates/pns/src/hook_dispatch.rs:23` and
   `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh`.
1. Sending gets one subcommand with two input forms: `pns send [flags]` and `pns send --json` (JSON on
   standard input, one JSON result on standard output). This replaces the bare `pns --state ...` event
   path and `pns submit --json`, and deletes the typo guard the bare form needs (`is_producer_argv` in
   `crates/pns/src/legacy/argv.rs`). The jobs `pns daemon schedule` runs and every caller (`lights`,
   `uu`, `posture`, the shell notifier, the failed-command text) move to `pns send`.
1. `pns tap` uses words for its operations, like every other subcommand: `pns tap --info` becomes
   `pns tap info` and `pns tap --install` becomes `pns tap install`; `--json` stays a flag
   (`crates/pns/src/command_tap.rs:60`). Update the `pns tap --install` hint in
   `dot_config/pns/private_config.toml.tmpl`.
1. Move `pns click <id>` under failures as `pns failures open <id>`, beside `pns failures`,
   `pns failures <id>` and `pns failures serve`. The failure banner's click command changes with it
   (`crates/pns/src/command_click.rs`).
1. Move `pns pulse <exit-code>` under lights as `pns lights pulse <exit-code>`, since it only touches the
   lamps (`crates/pns/src/command_pulse.rs`).
1. Remove `pns gate <harness>-hook` and keep only `pns <harness>-hook`, the one spelling moshi's pi and
   omp extensions can call (their `helperBinary` holds a single pathname). Two spellings of one gate is
   one too many (`crates/pns/src/invocation.rs`, the `gate` branch).
1. Fold `pns home` (one reading of the router) into `pns doctor`, as the code already plans
   (`crates/pns/src/invocation.rs`, the `home` branch), and remove the `home` subcommand.
1. `pns --help` lists every subcommand, split into commands the operator types and commands machines
   call. Today it hides `submit`, `click`, `failures`, `daemon retry`, `recap agent`, `recap git` and
   `presence poll --daemon`.
1. Every subcommand answers `--help` and `-h` with its own usage and exit `0`. Today only `pulse` and the
   event path do (`crates/pns/src/command_pulse.rs:24`, `crates/pns/src/legacy.rs`); everywhere else
   `--help` is refused as bad input.
1. Fix `pns gate <word>` exiting `0` silently for a word it does not accept, although its comment says it
   refuses (`crates/pns/src/moshi_submission.rs:14`). Removing `pns gate` removes the bug; the surviving
   `pns <harness>-hook` path must not inherit the silent exit.
1. Support reminders on every harness, not Claude Code alone. Each harness wires a "waiting" event and an
   "answered" event to pns, and passes `--remind` on the waiting one.
1. Claude Code is already wired: `PermissionRequest` to `pns hook blocked`, `PostToolBatch` to
   `pns hook resolved`, plus `Stop` and `StopFailure`
   (`private_dot_claude/modify_settings.json:385-426`). It only gains `--remind` on the blocked
   declaration.
1. Codex gains its answered event: today its installer wires `PermissionRequest` and `Stop` only
   (`dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh:12`), so pns learns of an answer only
   at turn end. Codex supports `PostToolUse`, which fires after a tool produces output, including a
   non-zero exit (learn.chatgpt.com/docs/hooks), so wire it to `pns hook resolved`. It also supports
   `Interrupt`, which ends a wait the same way.
1. hermes gains reminders: its hook events include `pre_approval_request` and `post_approval_response`
   (`hermes hooks test <event>` lists every valid event), which is the waiting/answered pair pns needs.
   Wire them to `pns hook blocked --remind` and `pns hook resolved` through the hooks block in
   `~/.hermes/config.yaml`, whose source is `private_dot_hermes/modify_private_config.yaml`.
1. pi and omp gain reminders through a pns extension of their own. pi has no approval prompt built in,
   and its extension API has no permission event; it does have `ui_prompt_start` and `ui_prompt_end`,
   which open and close a question to the operator (pi-mono `packages/coding-agent/docs/extensions.md`).
   omp is a pi fork and is expected to match; the decision if it does not is below.
1. Fix the dead pns gate in the pi and omp moshi extensions: `helperBinary` is declared and never called
   (`~/.pi/agent/extensions/moshi-hooks.ts:16`, same in `~/.omp/`), because the extension writes straight
   to the moshi daemon socket. So the repoint in
   `.chezmoiscripts/run_after_62-bounce-moshi-hook-on-upgrade.sh.tmpl` changes nothing and pi and omp
   pushes are unfiltered by presence, although that script reports the gate as wired.
1. Do NOT bump the envelope version. These changes break `pns.request/1` and `pns.result/1` in place: pns
   is pre-1.0 with one operator and three in-tree callers, so a version bump would buy compatibility
   nobody needs. Every copy moves in the same change (`posture/crates/posture-producer-wire/`, both
   fixtures).
1. Ship `[delivery_class.default]` in the config, so the rule for a message that names no class is
   written down rather than hidden in code: the default route and `bypass_mute = false`.
1. A `delivery_class` naming no configured class is refused as bad input (exit `2`) and named, rather
   than silently delivered as the default. A class the operator deleted is a message routed somewhere
   they did not intend.
1. `status` reports DELIVERY, not storage. Today it is computed only from the ledger
   (`crates/pns/src/event_flow/submit/receipt.rs:54-60`): a request whose every destination failed still
   answers `"accepted"` as long as the ledger row committed (pinned by `receipt/tests.rs:29-55`), and
   `degraded` means "the ledger did not commit" rather than the partial delivery its own doc comment
   claims (`crates/pns-protocol/src/result.rs:24`). The new values are `delivered` (every destination
   delivered), `partial` (at least one delivered, at least one did not), `undelivered` (none did) and
   `rejected` (bad input), and they match the exit codes: `0` for `delivered`, `1` for `partial` and
   `undelivered`, `2` for `rejected`.
1. Keep the ledger fact, but as a fact of its own rather than as the status: the `ledger_committed` and
   `ledger_unavailable` diagnostics stay (`receipt.rs:69-73`), and a committed ledger with nothing
   delivered reads as `undelivered` plus `ledger_committed`.
1. Rename `decision_id` to `ledger_sequence`. It holds the stringified ledger row number
   (`receipt.rs:64`, `crates/pns-application/src/submission_delivery.rs:11`), while pns has a real and
   unrelated decision record keyed by producer and request id
   (`crates/pns-adapters/src/persistence/sqlite/decisions.rs:11`). Two things wearing one word.
1. Remove `interaction` from the result, with the request's `await_decision` it answers. It is hardcoded:
   `receipt.rs:65` sets none and `event_flow/submit.rs:85-86` sets `{"kind":"no_opinion"}` whenever the
   request asked, so `answered` is never constructed outside tests.
1. Every closed set on both envelopes is a bare string, never a one-key wrapper object. `Status` and
   `DeliveryOutcome` already are (`result.rs:26,46`), while `Signal`, `Interaction` and
   `InteractionResult` are `{"kind": ...}` objects (`request.rs:52,77`, `result.rs:37`). The wrapper buys
   room for data none of them carries.
1. Populate `destinations[].note` with the destination's own sentence, which pns already has and throws
   away (`receipt.rs:79-89` drops `Delivery::Failed(String)`, `Rejected{detail}` and `Unlaunched(String)`
   from `crates/pns-domain/src/routing.rs:145-168`). The rule that the EVENT's own text never comes back
   stays, pinned by `receipt/tests.rs:68`. Today the field is dead: `named()` hardcodes `note: None`
   (`receipt.rs:96`), so "hermes failed" reaches a producer with no reason attached.
1. Split the overloaded `silent` outcome. It means "the channel ran and said nothing"
   (`routing.rs:146,157-159`) and ALSO "the ledger never learned the answer" on the replay path
   (`receipt.rs:32-35`). The second becomes its own `unknown` outcome, so a producer can tell a quiet
   success from a missing answer. Note that `ReportMode::Silent` serializes to the channel contract as
   the word `async` (`routing.rs:24-34`), a third meaning of the same word, which the mute rename does
   not cover.
1. Carry the retry hint the ledger already has: `LedgerCompletion::Retry{retry_at}`
   (`crates/pns-application/src/ports/ledger.rs:43-47`) is dropped at `receipt.rs:24-36`, so a producer
   cannot tell a leg pns will retry from one it has given up on. Add `retry_at` to the destination
   outcome.
1. Name each destination's route in the result: a producer that submitted `route: "priority"` gets back
   `destination: "hermes"` with no stated relationship (`result.rs:60`). Add `route` beside it.
1. Rename `destinations[].destination` to `destinations[].name`, so the field does not stutter with its
   own array.
1. Move ignored field names out of `diagnostics` into their own `ignored_fields` list. Today the marker
   string `ignored_fields` is pushed into `diagnostics` and the names follow it as bare entries
   (`event_flow/submit.rs:89-91`), so a reader has to know that one diagnostic changes the meaning of
   every entry after it.
1. Fix the golden fixture, which contradicts the code: `crates/pns-protocol/fixtures/result-v1.json:11`
   spells one ignored field as `["ignored_field:detial"]` (singular, colon-joined) and is pinned that way
   by `result/tests.rs:36`, while pns emits `["ignored_fields", "detial"]`. Its `"note"` line is also a
   shape pns can never produce today.
1. One rule for absent optional fields on both envelopes: omit them. Today five fields serialize as
   `null` and only `note` is skipped (`result.rs:62,70-76`).
1. Refuse rather than panic on a destination name pns cannot encode:
   `Name::new(...).expect("a registered destination name")` (`receipt.rs:93-94`) crashes the process if a
   registered name ever exceeds 64 characters or carries a control character. Check it where destinations
   are registered.
1. Rename the domain-side `retry::DeliveryOutcome` (`crates/pns-domain/src/retry.rs:27-34`, values
   `Status(u16)`, `NoResponse`, `NoStatus`), which collides by name with the wire
   `pns_protocol::DeliveryOutcome` while meaning something else. The wire type keeps the name.
1. Give the two envelopes symmetric Rust type names: the request is `Request` with `Decoded`, re-exported
   as `DecodedRequest` (`request.rs:108`, `lib.rs:50`), and the result is `ResultEnvelope`
   (`result.rs:67`). Three conventions for two symmetric things.
1. Every environment variable pns itself owns starts with `PNS_`. Four do not: `HUE_PULSE_ROOMS`
   (`crates/pns/src/lamp_pulse.rs:56`, consumed by pns's own
   `crates/pns-adapters/src/hue/settings.rs:25`), `MOSHI_HOOK_BIN`
   (`crates/pns-adapters/src/process/settings.rs:20`) and `CODEX_BIN`
   (`crates/pns-adapters/src/codex.rs:21`), which borrow another tool's prefix for names pns invented,
   and `REPORT_LIB_PLAIN` (`crates/pns-adapters/src/style.rs:109`), which is shared with this
   repository's shell style library and keeps its name for that reason. `NO_COLOR` stays as the
   cross-tool convention.
1. Config is the single source for a setting that has a config key: delete the environment variables that
   duplicate one, rather than documenting a precedence rule per pair. `PNS_PHONE_MARKER_FILE` duplicates
   `[phone] marker_file`, `HUE_PULSE_ROOMS` duplicates `plugins.hue.rooms`,
   `PNS_MOSHI_SUBMIT_DEADLINE_MS` duplicates `plugins.mobile.submit_deadline_secs` (and disagrees on
   unit), `PNS_PULSE_THRESHOLD_SECS` duplicates `lights.loop.threshold_secs`, and
   `PNS_CONDENSER_DEADLINE_MS` duplicates `recap.summarizer_deadline_secs` (and disagrees on both unit
   and word). Keys are at `crates/pns-adapters/src/config/schema.rs:48-153`.
1. Give a config key to every durable setting that has only an environment variable today, since an
   install-wide value belongs in the file the operator can read: `PNS_STATE_DIR`
   (`crates/pns-adapters/src/persistence/state_dir.rs:6`), `PNS_CHANNELS_DIR`
   (`crates/pns/src/channel_dispatch.rs:53`), `PNS_HERMES_URL` (`channel_dispatch.rs:74`),
   `PNS_MOSHI_URL` (`channel_dispatch.rs:151`), `PNS_TERMINAL_BUNDLE_ID` (`channel_dispatch.rs:134`) and
   `PNS_REMOTE_TIMEOUT` (`channel_dispatch.rs:168`), which is a delivery bound and belongs beside
   `[delivery] max_attempts`. Discord has no such override at all, which is the inconsistency this
   removes.
1. Every duration environment variable takes the same duration string as flags and config (`500ms`,
   `30s`, `5m`) and drops its unit suffix: `PNS_PAYLOAD_DEADLINE_MS`, `PNS_MOSHI_JSON_DEADLINE_MS`,
   `PNS_MOSHI_STATUS_DEADLINE_MS`, `PNS_DB_BUSY_TIMEOUT_MS`, `PNS_DAEMON_TICK_MS`,
   `PNS_RING_LOCK_TEST_DELAY_MS`, `PNS_IDLE_SECS`, `PNS_DESK_IDLE_SECS`, `PNS_REPLY_REREAD_INTERVAL`
   (parsed as float seconds today, `crates/pns/src/turn_text.rs:82-85`) and `PNS_PHONE_INPUT_AGE` (no
   unit at all, `crates/pns-domain/src/decision/overrides.rs:90`).
1. Use one word per kind of time knob, rather than six for one idea: `deadline` bounds a single
   operation, `interval` repeats, `delay` waits before acting, and `max_age` bounds how stale a value
   may be. The first three point forward at work this process is about to do or repeat; `max_age` points
   backward at a value already in hand and says how old it may be and still be trusted, which is why it
   earns a word rather than being a fourth synonym (operator ruling 2026-09-17). Today `timeout`,
   `deadline`, `threshold`, `interval` and `tick` all name the same kind of value (`PNS_REMOTE_TIMEOUT`,
   `PNS_DB_BUSY_TIMEOUT_MS`, the four `*_DEADLINE_MS`, `PNS_PULSE_THRESHOLD_SECS`,
   `PNS_REPLY_REREAD_INTERVAL`, `PNS_DAEMON_TICK_MS`), and `PNS_PHONE_INPUT_AGE` is the one `age` that
   stays, as `PNS_PHONE_INPUT_MAX_AGE`, because it measures exactly that: how long ago the phone was
   tapped and whether that still counts. Collapsing it into `deadline` would make it read as a timeout
   on TAKING the reading, which is a different knob pns could also want, so the collapse would recreate
   the ambiguity this item exists to remove.
1. Spell words out in environment names, as the repository's naming rule requires:
   `PNS_DB_BUSY_TIMEOUT_MS` abbreviates "database" where every other adapter name says `sqlite`
   (`crates/pns-adapters/src/persistence/sqlite/store.rs:40`), and `PNS_IDLE_SECS` beside
   `PNS_DESK_IDLE_SECS` does not say which idleness it means
   (`crates/pns-domain/src/decision/overrides.rs:88-89`).
1. One word for one thing across names: the recap's text shortener is `condenser` in the environment and
   in `crates/pns-adapters/src/codex.rs`, and `summarizer` in config (`recap.summarizer_deadline_secs`)
   and in `crates/pns/src/tests`. The word is summarizer, everywhere.
1. Test-only knobs stop shipping in production builds. `PNS_RING_LOCK_TEST_DELAY_MS` sleeps inside a
   locked section (`crates/pns-adapters/src/persistence/ring.rs:158-162`) and `PNS_DB_BUSY_TIMEOUT_MS`
   calls itself "A TEST-ONLY OVERRIDE" (`sqlite/store.rs:35`), yet both are read by unguarded production
   code, so a stray variable in a real environment changes real behavior. Put them behind `cfg(test)` or
   a dev feature (see the decisions below, which settle each one). The six deadline knobs whose doc
   comments say only a test has ever set them (`crates/pns-application/src/daemon.rs:123` for the tick)
   get the same decision, one way or the other.
1. Name the variables pns exports to a child in the same vocabulary as the request fields it carries:
   `PNS_REQUEST_ID` and `PNS_PRODUCER` (`crates/pns-adapters/src/destinations/executable.rs:108-112`)
   already match `request_id` and `producer`, so a plugin sees one word per idea. Any field added to the
   envelope that a plugin needs follows the same rule.
1. A plugin table is named for the FUNCTION it serves, with `type` naming the vendor, which is the shape
   three tables already use (`[plugins.mobile] type = "moshi"`, `[plugins.presence] type = "hue"`,
   `[plugins.router] type = "unifi"`). Rename the rest to match
   (`crates/pns-adapters/src/config/schema.rs:117-170`): `[plugins.hue]` becomes
   `[plugins.lights] type = "hue"`, `[plugins.macos-banner]` becomes `[plugins.banner] type = "macos"`
   (also the only table name spelled with a hyphen), and `[plugins.router]` becomes
   `[plugins.home_presence] type = "unifi"`, since every line of prose around it calls it the home probe.
1. Fold the two durable-log vendors into one function table. `[plugins.discord]` and `[plugins.hermes]`
   serve one function and are already refused together (`refuse_two_durable_logs` in
   `crates/pns-adapters/src/config/plugins.rs`), so they become `[plugins.log]` with
   `type = "hermes"|"discord"`, and the mutual exclusion stops needing a rule of its own.
1. Merge the top-level `[phone]` table into the phone plugin: `[phone] marker_file` (`schema.rs:70`) is
   the only key in it and belongs beside `[plugins.mobile]`, which is renamed `[plugins.phone]` for the
   same function-not-device-class reason (its `type = "moshi"` already names the vendor).
1. Rename `[lights.github]` (`schema.rs:96`), the one vendor word among behaviours that are otherwise
   states (`done`, `failed`, `blocked`, `unread`, `loop`, `dim`), to the state it lights: the pass and
   fail colors of a checks result.
1. Name each credential for the kind of secret its own tool issues, spelled out, and take the wording
   from the KeePassXC entry that already states it (operator ruling 2026-09-17). A config key that
   disagrees with the vendor's own word and with the vault entry makes a reader guess whether they are
   the same secret.

   | Table              | Key today | Key after               |
   | ------------------ | --------- | ----------------------- |
   | `plugins.mobile`   | `token`   | `device_token`          |
   | `plugins.discord`  | `token`   | `bot_token`             |
   | `plugins.github`   | `token`   | `personal_access_token` |
   | `plugins.router`   | `api_key` | `api_key`               |
   | `plugins.hue`      | `key`     | `api_key`               |

   `plugins.hermes.keys.<route>` stays a map of route keys. `plugins.hue.bridge` is a host address
   rather than a credential and is out of scope. If a single internal type helps the Rust, it lives in
   the code, not in the file a human reads.
1. One word for a route. `[routes] default`/`urgent` and `plugins.hermes.keys.<route>` say route;
   `plugins.router.stale_alert_channel` (`schema.rs:167`) says channel for the same thing, while its own
   code calls the variable `alert_route` (`crates/pns/src/command_home.rs:52`) and its error text says
   "route name". Rename it to a route, and keep "channel" only for a Discord channel id.
1. One word for a dim window. `plugins.hue.quiet_hours` and `lights.<level>.dim_window`
   (`schema.rs:116,129`) take the same `HH:MM-HH:MM`, wrap midnight the same way, and the second already
   supersedes the first. The name is `dim_window`, as a `[lights]`-level default that a place may
   override.
1. One word for a held lamp's expiry: `lights.blocked.give_up_after_secs` and
   `lights.loop.lease_timeout_secs` (`schema.rs:93,111`) are both "how long a held lamp survives with
   nothing renewing it", in one table, sharing a floor constant (`MIN_LEASE_TIMEOUT_SECS`,
   `crates/pns-adapters/src/config/lights_bounds.rs:44`).
1. Rename `shows` (`schema.rs:116`) so it pairs with its sibling `dim_behaviours`: the two take the same
   type and the same validator (`lights_targets.rs:34,37`) and read as unrelated words.
1. Rename the three unrelated `stale_after_secs` keys to what each one means: `nag.stale_after_secs`
   escalates a blocked session to the urgent route, `plugins.presence.stale_after_secs` discards an old
   bridge reading, and `plugins.presence.desk_stale_after_secs` stops trusting an old keystroke
   (`schema.rs:71,139,143`). Same for the two `after_secs`: the reminder delay (already renamed `delay`)
   and `lights.unread.after_secs`, which is how long a finished run waits before its lamp arms.
1. Split `[nag]`, which holds two unrelated features: the local nudge about an unanswered approval
   becomes `[remind]`, and the page about a session stuck past its window gets its own table
   (`schema.rs:71`).
1. No key doubles as its own on/off switch. Today `nag.after_secs = 0` means off (`config/nag.rs:122`),
   `nag.stale_after_secs = 0` means off (`nag.rs:62`), `focus.silence` is both roster and switch
   (`config/focus.rs:5-9`), `recap.summarizer` is both command and switch (`config/recap/options.rs:23`),
   and `recap.repos`/`recap.review_notes` each gate whether a subprocess or directory is touched at all.
   Durations refuse `0` anyway, so unset means off and an explicit `enabled` carries the switch where one
   is needed.
1. Make the `enabled` defaults visible rather than opposite and unstated: `[daemon] enabled` defaults
   true (`config/daemon.rs:4`) while every `[plugins.*] enabled` defaults false (`config/load.rs:65`).
   The shipped config writes each one out at its default.
1. Name the percent-valued keys for what they are. `high`, `low`, `brightness` and `flare`
   (`schema.rs:89-114`) are 1 to 100 percentages carrying no unit, in tables where every other number
   does, and `flare` sits beside `flare_ms` as though the two were one quantity in two units when one is
   a brightness and the other a duration (`config/lights_tables.rs:172,176`).
1. Fix the delivery keys whose names disagree with the code: `delivery.max_attempts` is compared against
   the RETRY count (`if retries >= self.max_attempts`, `crates/pns-domain/src/retry.rs:100`), so it is a
   retry ceiling; `delivery.retry_base_secs` is not a base but a per-retry increment
   (`wait = base * retries`, `retry.rs:143`); and `delivery.max_age_secs` measures the ORIGINAL EVENT's
   age, which its name does not say (`schema.rs:64-66`).
1. Rename `lights.refresh_secs` (`schema.rs:74`) to `arm_interval`, matching the duration vocabulary.
   It is one value with one meaning: how often the daemon re-arms the lamps
   (`lamp_registration.rs:90`), from which `tick_bridge_deadline` takes a fifth as the budget for one
   bridge call (`lamps/deadline.rs:17`), deliberately, so three calls cannot outlive the interval that
   spawned them. A fade is `duration_ms` on each state's own table (4000 for `done`, `failed` and
   `github`, 2000 for the `blocked` breath).
1. Remove `plugins.hue.rooms` (`schema.rs:129`). It names which places the plain pulse flashes and is
   dead whenever a `[lights]` lamp/room/zone map exists, which is the shipped state, so two keys say one
   thing and one silently wins.
1. Rename `plugins.presence.exclude` (`schema.rs:141`), a verb among noun siblings, and say what it
   excludes: rooms, subtracted from `rooms`, with `desk_room` required to be in one and not the other
   (`config/presence.rs:93-108`).
1. Remove the table-name stutters: `plugins.router.router_url` and `plugins.mobile.mobile_watch_card`
   (`schema.rs:152,166`). While there, `plugins.hue.bridge` holds a hostname or address
   (`crates/pns-adapters/src/hue/settings.rs:32`) and should say so, and it and `router_url` are the same
   kind of setting in two shapes.
1. Rename the keys that do not say what they control: `recap.digest` (its value is whether the
   whole-window recap is posted, inside a table already called recap), `recap.review_notes` (a glob path,
   not notes), `recap.repos` and `recap.min_events` (abbreviations; the parser is already called
   `repositories`, `config/recap_sources.rs:19`), `plugins.mobile.submit_deadline_secs` (submit what),
   `delivery.bypass_silence_classes` (classes of what, and "silence" becomes "mute"),
   `lights.loop.threshold_secs` (threshold of what), and `[failures] serve`/`port`, which configure an
   HTTP listener inside a table named for the record.
1. Keep the plural rule one way round: a table holding a set is plural (`[routes]`), a table keyed by one
   name is singular (`[delivery_class.<name>]`, `[lights.lamp."<name>"]`), and a list key is plural
   (`rooms`). Today `[routes]`, `[plugins]`, `[lights]` and `[failures]` are plural beside singular
   `[daemon]`, `[delivery]`, `[focus]`, `[nag]`, `[phone]` and `[recap]`, and `plugins.hue.key` sits
   beside `plugins.hermes.keys` for the same kind of value (`schema.rs:42-43,126,129`).
1. Ship an example for `lights.zone`. It is declared (`schema.rs:86`) and parsed
   (`config/lights_tables.rs:35`), but the template carries no block and no commented example, unlike
   `lamp` and `room`, so the only documented level is prose.
1. Give the values file's `note` its own namespace. `[plugins.hermes.keys] note`
   (`dot_config/pns/config-values.toml:29-32`) is stripped by the renderer
   (`crates/pns-adapters/src/config/render/write.rs:120-146`), but it occupies the same open table as
   real route names, so a gateway route actually named `note` could never be given a key.
1. Stop using a table's PRESENCE as a setting: `dot_config/pns/config-values.toml:90` ships an empty
   `[nag]` whose only effect is to keep `after_secs` uncommented in the render.
1. The `_secs` versus `_seconds` question does not arise: a duration value carries its own unit, so a
   duration name carries none. Names that are counts, not durations, stay plain (`minimum_events`,
   `max_retries`).
1. Wire the omp extension exactly as pi's, and confirm `ui_prompt_start`/`ui_prompt_end` against omp's
   own extension docs while doing it. If omp does not have that pair, omp ships without reminders and
   says so in its setup output rather than arming one it cannot clear.
1. Keep the two test knobs that are read by production code out of production builds: put
   `PNS_RING_LOCK_TEST_DELAY_MS` behind `cfg(test)` (it sleeps inside a locked section,
   `crates/pns-adapters/src/persistence/ring.rs:158-162`), and make SQLite's busy bound a real setting,
   `[storage] busy_deadline`, rather than an environment variable that calls itself test-only
   (`crates/pns-adapters/src/persistence/sqlite/store.rs:35`). The remaining deadline knobs stay
   environment variables, documented, taking durations.
1. Give the two envelopes symmetric Rust names without touching the wire: `Request` becomes
   `RequestEnvelope` beside the existing `ResultEnvelope`, and the decoded forms are `DecodedRequest` and
   `DecodedResult` (`crates/pns-protocol/src/request.rs:108`, `result.rs:67`, `lib.rs:50`). The schema
   strings stay `pns.request/1` and `pns.result/1`.
1. Rename the colliding domain enum `retry::DeliveryOutcome` (`crates/pns-domain/src/retry.rs:27-34`) to
   `retry::TransportOutcome`. The wire `pns_protocol::DeliveryOutcome` keeps its name.
1. The recap's text shortener is called the summarizer everywhere: `PNS_CONDENSER_DEADLINE_MS` becomes
   `[recap] summarizer_deadline`, and `crates/pns-adapters/src/codex.rs` follows.

## Names

Every rename above, old to new. Durations take a duration string; nothing else changes type.

| Old                                                                                                                                 | New                                                                            |
| ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `--agent`                                                                                                                           | `--producer`                                                                   |
| `--state <free text>`                                                                                                               | `--state done\|failed\|blocked\|resolved\|observation\|progress`               |
| `--channel`                                                                                                                         | `--route`                                                                      |
| `--elapsed`                                                                                                                         | `--elapsed` (duration)                                                         |
| `--kind agent\|health`                                                                                                              | `--delivery-class <name>`                                                      |
| `--local-only`, `--remote-only`                                                                                                     | `--scope automatic\|local_only\|remote_only`                                   |
| `--long-running`, `--require-delivery`                                                                                              | removed                                                                        |
| `pns <flags>`, `pns submit --json`                                                                                                  | `pns send <flags>`, `pns send --json`                                          |
| `pns nag`                                                                                                                           | `pns remind`                                                                   |
| `pns quiet`, `pns lights quiet`                                                                                                     | `pns mute`, `pns lights mute`                                                  |
| `pns pulse <code>`                                                                                                                  | `pns lights pulse <code>`                                                      |
| `pns click <id>`                                                                                                                    | `pns failures open <id>`                                                       |
| `pns home`                                                                                                                          | `pns doctor`                                                                   |
| `pns gate <harness>-hook`                                                                                                           | removed, `pns <harness>-hook` stays                                            |
| `pns tap --info`, `pns tap --install`                                                                                               | `pns tap info`, `pns tap install`                                              |
| `pns shell end --exit`                                                                                                              | `pns shell end --exit-code`                                                    |
| `pns recap --since`, `--until`                                                                                                      | `--since-epoch`, `--until-epoch`                                               |
| `pns daemon schedule --in`, `--every`                                                                                               | unchanged, durations                                                           |
| `pns daemon schedule --until <epoch>`                                                                                               | `--until-epoch <epoch>`, `--until +<duration>` stays                           |
| JSON `signal: {kind: "succeeded"}`                                                                                                  | `state: "done"`                                                                |
| JSON `context: {project, branch, pane}`                                                                                             | `project`, `branch`, `pane` at top level                                       |
| JSON `session: {id, turn}`                                                                                                          | `session: "<id>"`                                                              |
| JSON `elapsed_secs`                                                                                                                 | `elapsed`                                                                      |
| JSON `class`, `interaction`, `event`, `occurred_at`, `require_delivery`                                                             | `delivery_class`; the rest removed                                             |
| result `status: accepted\|degraded\|rejected`                                                                                       | `delivered\|partial\|undelivered\|rejected`                                    |
| result `decision_id`                                                                                                                | `ledger_sequence`                                                              |
| result `destinations[].destination`                                                                                                 | `destinations[].name`                                                          |
| result `interaction`                                                                                                                | removed                                                                        |
| result `diagnostics: ["ignored_fields", ...]`                                                                                       | `ignored_fields: [...]`                                                        |
| `[nag] after_secs`                                                                                                                  | `[remind] delay`                                                               |
| `[nag] stale_after_secs`                                                                                                            | `[stale] escalate_after`, plus an explicit `[stale] route`                     |
| `[focus] silence`                                                                                                                   | `[focus] modes`, plus `[focus] enabled`                                        |
| `[delivery] bypass_silence_classes`                                                                                                 | `[delivery_class.<name>] bypass_mute`                                          |
| `[delivery] max_attempts`                                                                                                           | `[delivery] max_retries`                                                       |
| `[delivery] retry_base_secs`                                                                                                        | `[delivery] retry_step`                                                        |
| `[delivery] max_age_secs`                                                                                                           | `[delivery] event_max_age`                                                     |
| `PNS_REMOTE_TIMEOUT`                                                                                                                | `[delivery] remote_deadline`                                                   |
| `[recap] digest`                                                                                                                    | `[recap] post_window_recap`                                                    |
| `[recap] min_events`                                                                                                                | `[recap] minimum_events`                                                       |
| `[recap] repos`                                                                                                                     | `[recap] repositories`                                                         |
| `[recap] review_notes`                                                                                                              | `[recap] review_notes_glob`                                                    |
| `[recap] summarizer_deadline_secs`                                                                                                  | `[recap] summarizer_deadline`                                                  |
| `[failures] serve`, `port`                                                                                                          | `[failures] page_enabled`, `page_port`                                         |
| `[lights] refresh_secs`                                                                                                             | `[lights] arm_interval`                                                        |
| `[lights.<behaviour>] duration_ms`                                                                                                  | `duration`                                                                     |
| `[lights.<behaviour>] brightness`, `high`, `low`                                                                                    | `brightness_percent`, `high_percent`, `low_percent`                            |
| `[lights.loop] flare`, `flare_ms`                                                                                                   | `flare_percent`, `flare_duration`                                              |
| `[lights.loop] threshold_secs`                                                                                                      | `[lights.loop] arm_after`                                                      |
| `[lights.loop] lease_timeout_secs`, `[lights.blocked] give_up_after_secs`                                                           | `lease_expiry` in both                                                         |
| `[lights.unread] after_secs`                                                                                                        | `[lights.unseen] arm_after`                                                    |
| `[lights.unread]`                                                                                                                   | `[lights.unseen]`                                                              |
| `[lights.github]`                                                                                                                   | `[lights.checks]`, with `pass_color` and `fail_color`                          |
| `[lights.<level>."<name>"] shows`                                                                                                   | `behaviours`                                                                   |
| `[plugins.hue]`                                                                                                                     | `[plugins.lights] type = "hue"`                                                |
| `[plugins.hue] bridge`, `key`, `quiet_hours`, `rooms`                                                                               | `bridge_host`, `key`, `[lights] dim_window`, removed                           |
| `[plugins.discord]`, `[plugins.hermes]`                                                                                             | `[plugins.log] type = "discord"\|"hermes"`                                     |
| `[plugins.discord] token`, `[plugins.router] api_key`                                                                               | `key`                                                                          |
| `[plugins.macos-banner]`                                                                                                            | `[plugins.banner] type = "macos"`                                              |
| `[plugins.mobile]`, `[phone]`                                                                                                       | `[plugins.phone] type = "moshi"`                                               |
| `[plugins.mobile] mobile_watch_card`                                                                                                | `[plugins.phone] card_while_watching`                                          |
| `[plugins.mobile] submit_deadline_secs`                                                                                             | `[plugins.phone] ack_deadline`                                                 |
| `[plugins.router]`                                                                                                                  | `[plugins.home_presence] type = "unifi"`                                       |
| `[plugins.router] router_url`, `stale_alert_channel`                                                                                | `url`, `alert_route`                                                           |
| `[plugins.presence] poll_secs`                                                                                                      | `poll_interval`                                                                |
| `[plugins.presence] stale_after_secs`                                                                                               | `reading_max_age`                                                              |
| `[plugins.presence] desk_stale_after_secs`                                                                                          | `desk_input_max_age`                                                           |
| `[plugins.presence] exclude`                                                                                                        | `excluded_rooms`                                                               |
| `PNS_AGENT`                                                                                                                         | `PNS_PRODUCER`                                                                 |
| `HUE_PULSE_ROOMS`, `PNS_PHONE_MARKER_FILE`, `PNS_PULSE_THRESHOLD_SECS`, `PNS_MOSHI_SUBMIT_DEADLINE_MS`, `PNS_CONDENSER_DEADLINE_MS` | removed, config only                                                           |
| `PNS_STATE_DIR`, `PNS_CHANNELS_DIR`, `PNS_HERMES_URL`, `PNS_MOSHI_URL`, `PNS_TERMINAL_BUNDLE_ID`                                    | keep, plus a config key each                                                   |
| `MOSHI_HOOK_BIN`, `CODEX_BIN`                                                                                                       | `PNS_MOSHI_HOOK_BIN`, `PNS_CODEX_BIN`                                          |
| `PNS_IDLE_SECS`, `PNS_DESK_IDLE_SECS`                                                                                               | `PNS_SCREEN_IDLE`, `PNS_DESK_IDLE`                                             |
| `PNS_PHONE_INPUT_AGE`                                                                                                               | `PNS_PHONE_INPUT_MAX_AGE`, duration value                                      |
| `PNS_REPLY_REREAD_INTERVAL`                                                                                                         | unchanged name, duration value                                                 |
| `PNS_PAYLOAD_DEADLINE_MS`, `PNS_MOSHI_JSON_DEADLINE_MS`, `PNS_MOSHI_STATUS_DEADLINE_MS`                                             | `PNS_PAYLOAD_DEADLINE`, `PNS_MOSHI_JSON_DEADLINE`, `PNS_MOSHI_STATUS_DEADLINE` |
| `PNS_DAEMON_TICK_MS`                                                                                                                | `PNS_DAEMON_TICK_INTERVAL`                                                     |
| `PNS_DB_BUSY_TIMEOUT_MS`                                                                                                            | `[storage] busy_deadline`                                                      |
| `PNS_RING_LOCK_TEST_DELAY_MS`                                                                                                       | `cfg(test)` only                                                               |
| `retry::DeliveryOutcome`                                                                                                            | `retry::TransportOutcome`                                                      |
| `Request`, `Decoded`                                                                                                                | `RequestEnvelope`, `DecodedRequest` and `DecodedResult`                        |
