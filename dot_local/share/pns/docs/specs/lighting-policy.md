# Lighting policy

## Scope

The Hue lamps are not a notification destination. They are a stateful attention indicator: a small set of
named states (`blocked`, `loop`, `unread` in its failure and success flavours) that the house HOLDS
between events, plus a transient `pulse` that blinks once for a finished or a dead turn. A separate
repeating job (`pns lights tick`) re-derives every held state from the machine, resolves the operator's
name-to-lamp map against the bridge, writes the lamps that should be showing something, puts out by name
whatever it was holding and is not any more, and breathes each lit lamp for the rest of its interval.
This document covers the pulse, the `unread` state, the leases, the breath phases, the quiet window and
the dim window, the precedence between lamp states, the tick's write order, held lamps, streaks, and the
legacy `lights-glow` migration. Every claim below is derived from the code and its tests in this crate;
gaps are marked `NOT ESTABLISHED:`. The lamp state is spelled `unread`
(`src/lights.rs:Held::UnreadFailure`, `src/lights.rs:Held::UnreadSuccess`,
`src/config.rs:Behaviour::Unread`, whose config word is `"unread"` in `src/config.rs:BEHAVIOUR_WORDS`);
`glow` survives only as the legacy state directory name `lights-glow`, read once by
`src/main.rs:sweep_legacy_state` and never written.

______________________________________________________________________

## Table 1: lamp states

The four held states and the one transient one. A lamp shows at most one held state at a time; the house
may hold all of them at once and different lamps may show different ones (`src/lights.rs:active_held`,
`src/lights.rs:shown`).

| State                         | What arms it                                                                                                                                                                                                                                   | What clears it                                                                                                                                                                                                                                                                     | Rank                 | State file                                                                                                                                      | Tests                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Blocked` (`"blocked"`)       | An event whose state is in `src/pulse.rs:LAMP_BLOCKED` (`blocked`, `asked`, `plan-ready`, `denied`, `asking`) writes one marker per session, but only when both switches are live (`src/main.rs:update_blocked_marker`, gated on `lamps_live`) | Any other event from that session (`src/lights.rs:blocked_marker_action` returns `Action::End`), `src/main.rs:end_blocked_wait` from the `prompt` and `resolved` hooks, or the backstop sweeping a marker past `[lights.blocked] give_up_after_secs` (`src/main.rs:sweep_blocked`) | 1, highest           | `lights-blocked/<session-id>`, one epoch per file (`src/lights.rs:blocked_dir`, `src/lights.rs:blocked_marker`)                                 | `src/lights.rs:a_blocked_event_starts_a_wait_and_every_other_event_ends_one`, `src/lights.rs:a_live_wait_holds_the_blocked_lamp_and_an_abandoned_one_stops_holding_it`, `src/main.rs:a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live`, `src/main.rs:a_wait_nobody_has_answered_still_holds_its_lamp_until_the_configured_backstop`, `tests/dispatch.rs:a_blocked_turn_lights_the_lamps_once_the_map_exists` |
| `Looping` (`"loop"`)          | Any of three: an agent streak past `[lights.loop] threshold_secs`, a shell marker whose command started that long ago, or a live lease (`src/lights.rs:loop_running`)                                                                          | The streak clearing behind its grace, the shell marker being removed or its shell dying, `pns loop end`, or the lease timing out (`src/main.rs:sweep_leases`)                                                                                                                      | 2                    | `lights-streak` (one line, `since last_seen`), `lights-shell/<shell-pid>` (one epoch), `lights-loop/<pane>` (one epoch)                         | `src/lights.rs:work_past_the_threshold_arms_the_loop_lamp_and_both_edges_are_closed`, `src/lights.rs:a_live_lease_arms_the_loop_lamp_with_nothing_working_and_an_expired_one_does_not`, `src/lights.rs:a_shell_command_is_measured_from_its_own_start_and_not_from_an_agents_streak`, `src/main.rs:the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding`                                                               |
| `UnreadFailure` (`"failure"`) | A `failed_at` epoch newer than the last interaction and not in the future, with nothing working. No delay at all (`src/lights.rs:unread_arming`)                                                                                               | Any interaction (desk, phone input, phone marker) later than that epoch; anything working; the operator's return clearing the held record (`src/main.rs:clear_held_lamps`)                                                                                                         | 3                    | `lights-news`, one line of two epochs `done_at failed_at`, `0` for "not yet" (`src/lights.rs:render_news`)                                      | `src/lights.rs:unread_arms_on_news_the_operator_has_not_been_back_for_and_on_nothing_else`, `src/lights.rs:success_news_waits_out_its_delay_and_failure_news_does_not`                                                                                                                                                                                                                                                           |
| `UnreadSuccess` (`"success"`) | A `done_at` epoch newer than the last interaction, at least `[lights.unread] after_secs` old, not in the future, with nothing working                                                                                                          | Same as `UnreadFailure`, and it is outranked by `UnreadFailure` whenever both are pending                                                                                                                                                                                          | 4, lowest            | `lights-news` (same file)                                                                                                                       | `src/lights.rs:success_news_waits_out_its_delay_and_failure_news_does_not`                                                                                                                                                                                                                                                                                                                                                       |
| pulse (transient, not held)   | One event whose plan earned a pulse, or a `blocked` behaviour on a mapped machine that is not silenced (`src/main.rs`, the \`decision.plan.pulse                                                                                               |                                                                                                                                                                                                                                                                                    | blocked_lamp\` gate) | Nothing: the bridge runs the signal for its own duration and puts the lamp back itself (`src/channels/hue.rs`, module doc, measured 2026-09-01) | Not ranked. It is refused on any lamp currently holding a state (`src/lights.rs:pulse_fires`)                                                                                                                                                                                                                                                                                                                                    |

The rank is the declaration order of `src/lights.rs:Held`, pushed in that fixed order by
`src/lights.rs:active_held` with no runtime sort. It is pinned by
`src/lights.rs:every_held_state_is_active_at_once_and_they_rank_blocked_loop_then_unread`.

The paths a held write is holding are recorded in `lights-held`, one line, space separated, each token
either a bare fixture path or `<path>@<end-unix-ms>:<h|l>:<state>` (`src/lights.rs:render_held_token`,
`src/main.rs:remember_held`).

______________________________________________________________________

## Table 2: leases and locks

| Name                                                           | Duration constant                                                                                                                                                                        | Who takes it                                                                                                                                            | Who renews it                                                                                                       | How it expires                                                                                        | A stranded one                                                                                                                                                                                                                 |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Loop lease (`lights-loop/<pane>`)                              | `[lights.loop] lease_timeout_secs`, default `src/config.rs:DEFAULT_LEASE_TIMEOUT_SECS` = 3900 seconds (65 minutes); bounds 60 (`MIN_LEASE_TIMEOUT_SECS`) to 86400 (`MAX_THRESHOLD_SECS`) | `pns loop begin` (`src/main.rs:loop_mode`), writing one epoch keyed to `HERDR_PANE_ID` or `--pane`                                                      | That pane's own ordinary event traffic, through `src/main.rs:renew_loop_lease`. Nothing else in the crate renews it | `src/lights.rs:marker_is_live`, both edges closed: exactly `lease_timeout_secs` old is still live     | The tick's `src/main.rs:sweep_leases` removes it on the pass after it expires. Until then it holds the loop lamp with nothing behind it, which is why `pns loop end` reports a failed removal loudly (`src/main.rs:end_lease`) |
| Tick job lease, ordinary (`daemon/lights`, the `until=` field) | `src/main.rs:ORDINARY_LEASE_SECS` = 300 seconds                                                                                                                                          | Every event, through `src/main.rs:register_lights_tick`                                                                                                 | Every subsequent event, and the tick itself while `standing.in_flight` (`src/main.rs:lights_tick`)                  | The daemon drops the job once `now` passes `until`                                                    | The tick simply stops running: no lamp is re-armed, and whatever the last tick wrote stays lit until an event's `clear_held_lamps` puts it out                                                                                 |
| Tick job lease, journalled                                     | `src/main.rs:JOURNALLED_LEASE_SECS` = 12 hours                                                                                                                                           | The same call, when `missed_notifications::was_missed` says the event was journalled (the operator is away or muted)                                    | Same                                                                                                                | Same                                                                                                  | Same                                                                                                                                                                                                                           |
| Tick job lease, hand-taken loop                                | `[lights.loop] lease_timeout_secs`                                                                                                                                                       | `pns loop begin` calls `src/main.rs:schedule_lights_tick` with the loop lease length, because event traffic will not refresh a pane that has gone quiet | Same                                                                                                                | Same                                                                                                  | Same                                                                                                                                                                                                                           |
| Lights tick lock (`lights-tick.lock`)                          | `src/main.rs:lights_tick_stale_secs` = `MAX_REFRESH_SECS` (30) + `tick_bridge_deadline(30)` (6) + 1 = 37 seconds                                                                         | The tick, before it resolves anything (`src/main.rs:run_tick_writes`, via `claim_lock`)                                                                 | Nobody. It is held for one tick and released by `src/main.rs:HeldLock`'s `Drop`                                     | Age: a lock older than the stale window is taken by rename and republished (`src/main.rs:claim_lock`) | A later tick stands down for one interval, then steals it. A lock whose own mtime cannot be read counts as live (`src/main.rs:lock_aged_out`)                                                                                  |
| News claim (`lights-news.claim.<pid>`)                         | Not a duration. `src/main.rs:NEWS_CLAIM_ATTEMPTS` = 2 tries, `src/main.rs:NEWS_CLAIM_WAIT` = 2 milliseconds between them                                                                 | `src/main.rs:record_news`, by renaming the record aside for the merge                                                                                   | Nobody                                                                                                              | Removed unconditionally by the claiming run after it publishes                                        | A run whose second attempt also misses merges blind, against whatever it can read at the published path. Cost is one lamp colour (stated in `src/main.rs:record_news`)                                                         |

Also relevant, though not a lease: the blocked backstop `[lights.blocked] give_up_after_secs`, default
`src/config.rs:DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS` = 16 hours, bounds 60 seconds to 7 days
(`MAX_GIVE_UP_AFTER_SECS`). Configuration refuses a `give_up_after_secs` below `[nag] after_secs` because
that is a config that gives up on a wait before it ever nudges about it (`src/config.rs`, the
`give_up`/`nag_after_secs` comparison).

______________________________________________________________________

## Table 3: windows

| Name         | What it is                                                                                                                                                                                                                                                                                                                     | Boundaries                                                                                                                                                                                                                                                         | What it changes about a lamp                                                                                                                                                                                                                     | Tests                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| quiet hours  | The `[plugins.hue] quiet_hours` key, `"HH:MM-HH:MM"`, parsed by `src/channels/hue.rs:quiet_window` into a `QuietWindow` of minutes since local midnight. It is the OPERATOR'S OWN schedule: it gates the no-map pulse, and it is the only source for how long a bare `pns lights quiet` lasts (`src/lights.rs:bare_mute_secs`) | Two-digit hours under 24 and minutes under 60 (`src/channels/hue.rs:minute_of_day`, `two_digits`). Absent or empty is no window; anything else is a refusal                                                                                                        | On a machine with NO `[lights]` table: inside it, no pulse fires at all. On a machine WITH a `[lights]` table it reaches no routed lamp: `src/main.rs:fire_pulse_unless_quiet` takes the routed branch before it ever calls `quiet_window`       | `src/channels/hue.rs:a_table_that_names_no_quiet_hours_has_no_window`, `a_quiet_hours_that_is_not_two_clock_readings_is_refused_by_name`, `a_blanked_quiet_hours_is_no_window_rather_than_a_refusal`, `tests/dispatch.rs:a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`, `tests/dispatch.rs:a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`, `tests/dispatch.rs:a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`         |
| quiet window | The evaluated form of a `QuietWindow` at one minute of the local day, `src/channels/hue.rs:quiet_now`. One predicate, read by the house gate and by every per-lamp dim decision                                                                                                                                                | Half open, start inclusive and end exclusive: minute 1319 is loud, 1320 is quiet, 1379 is quiet, 1380 is loud. A window whose start is after its end wraps midnight and is an OR of the two halves. A window whose start equals its end is never quiet             | Decides whether the dim rendering applies at all                                                                                                                                                                                                 | `src/channels/hue.rs:a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end`, `a_window_whose_start_is_after_its_end_is_quiet_on_both_sides_of_midnight`, `a_window_whose_start_equals_its_end_is_never_quiet`, `a_clock_this_machine_cannot_read_is_treated_as_inside_the_window`, `tests/dispatch.rs:the_window_is_read_in_the_zone_the_child_was_given`                                                                                                                             |
| dim window   | Per declaration: `dim_window = "HH:MM-HH:MM"` plus `dim_behaviours = [...]` on a `[lights.lamp/room/zone.<name>]` target, resolved to `src/channels/hue.rs:DimWindow`. The two keys travel together as ONE question so a lamp cannot take its room's window and a zone's enables                                               | Same `quiet_now` boundaries. Inside it a listed behaviour renders `Showing::Dimmed`, an unlisted one renders `Showing::Dark`; outside it everything renders `Showing::Full`. An EMPTY `dim_behaviours` suppresses every behaviour, with no second mode to spell it | Dimmed held state: same colour, the one shared `[lights.dim]` shape (default 3000 ms fades, high 7, low 1). Dimmed pulse: same colour and duration at `lights.dim.low`, since a blink has no low end to fade to. Dark: nothing is written at all | `src/channels/hue.rs:inside_a_window_an_enabled_behaviour_runs_dim_and_one_that_is_not_is_suppressed`, `a_window_with_nothing_enabled_suppresses_every_behaviour_and_needs_no_mode`, `a_dim_window_nobody_can_parse_leaves_that_lamp_dark_and_says_which_lamp`, `a_dimmed_pulse_fires_at_the_dim_floor_and_a_suppressed_one_does_not_fire`, `each_held_state_renders_its_own_locked_colour_and_shape`, `tests/dispatch.rs:an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg` |

A fourth silence exists and is NOT a window: the ad-hoc mute,
`pns lights quiet <place> [<duration>|off]`, one line per place in `lights-quiet`, each
`<expiry-epoch> <place>`. It is judged by `src/quiet.rs:is_muted`, half open, so a mute ends on the
second it names. See behaviours 30 to 32.

______________________________________________________________________

## Table 4: the legacy `lights-glow` migration

| Question             | Answer                                                                                                                                                                                                                                                                                                                         |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Where                | `src/main.rs:sweep_legacy_state`, called once per tick from `src/main.rs:lights_tick`, immediately after the state directory is resolved and before the house is read                                                                                                                                                          |
| What is READ         | Nothing. It is a deletion, not a migration. `src/main.rs:sweep_legacy_state`'s own doc: "Delete the state the lamps kept under their OLD names, and never read it"                                                                                                                                                             |
| What is REMOVED      | `lights-glow` (the old held record) and `lights-working-since` (the old streak) by `remove_file`, and the whole `lights-needs` directory by `remove_dir_all`                                                                                                                                                                   |
| What is WRITTEN      | Nothing at any legacy path. The new record is written under the new names by the same tick's ordinary work (`lights-held`, `lights-streak`, `lights-blocked/`)                                                                                                                                                                 |
| Why deletion is safe | Every one of those files is re-derived from the machine on the next tick: a wait re-arrives with its session's next event, and a streak restarts the moment work is seen. Keeping the old held record would have the NEW tick clear lamps it never wrote, by names it never chose                                              |
| The cost, stated     | The dark direction. At most one lamp stays lit under a name only the retired binary knew, until the operator's next event runs `clear_held_lamps`, which reads `lights-held` and not `lights-glow`                                                                                                                             |
| A second run         | Three failed `remove_file`/`remove_dir_all` syscalls and nothing else. There is deliberately no marker file recording that the sweep happened: "A removal of a name that is not there is one failed syscall, so the deletion happens exactly once and every tick after it pays three of those rather than a fourth state file" |
| Test                 | `src/main.rs:the_first_tick_sweeps_the_state_the_old_names_held` plants all three legacy paths and asserts all three are gone, "contents and all"                                                                                                                                                                              |

______________________________________________________________________

# Behaviours

## The pulse

### 1. One event yields one behaviour word

Given a harness event carrying a state word,

When the composition root decides what the lamps say about it,

Then `src/pulse.rs:state_behaviour` answers exactly once: `failed` is `Behaviour::Failed`; any word in `src/pulse.rs:LAMP_BLOCKED` (`"blocked"`, `"asked"`, `"plan-ready"`, `"denied"`, `"asking"`) is `Behaviour::Blocked` but only when a `[lights]` table exists; every other word, the empty string included, is `Behaviour::Done`.

- Success: the colour a lamp flashes, the record that arms the `unread` lamp, and the gate that lets a
  pulse fire at all are all read off that ONE answer, so they cannot disagree about one event
  (`src/main.rs`, the `state_behaviour(&event.state, lights.is_some())` call).
- Failure sources: an unrecognised state word. It reads as `Done`, deliberately
  (`src/pulse.rs:a_state_the_lamps_have_no_word_for_reports_done`).
- Fail direction: not on the delivery path. This is a pure classification; the notification legs are
  dispatched before it.
- Thresholds: none. `asking` is on `LAMP_BLOCKED` and `failed` is not, which is the one-word trade with
  `missed_notifications::NEEDS_YOU` in each direction (`src/pulse.rs:LAMP_BLOCKED` doc).
- Required side effects: none. Pure function.
- Forbidden side effects: no second list of waiting words anywhere. `src/lights.rs:blocked_marker_action`
  reads `LAMP_BLOCKED` rather than keeping its own.
- Timeout and cancellation: not applicable, pure.
- Idempotency and duplicates: pure and total.
- Privacy: reads only the state word, never the detail.
- Process ownership and cleanup: not applicable.
- Compatibility contract: without a `[lights]` table every `LAMP_BLOCKED` word still answers `Done`,
  which is the green flash a long-running blocked turn has produced since the shell version
  (`src/pulse.rs:without_a_lamp_map_a_waiting_agent_reports_done_exactly_as_it_did_before`).

### 2. An exit code is a success, a failure, or a refusal

Given `pns pulse [<exit-code>]` or the long-command notifier,

When `src/pulse.rs:exit_behaviour` reads the word,

Then empty is `Done`, all ASCII zeroes is `Done`, any other run of ASCII digits is `Failed`, and anything else is `None`.

- Success: `pns pulse` exits 0 having signalled the configured rooms.
- Failure sources: a non-digit code (`"oops"`, `"-0"`, `" 0"`, `"0\n"`, the Arabic-Indic digit `"١"`)
  answers `None`, and `src/main.rs:pulse_mode` prints `PULSE_USAGE` to stderr and exits 2.
- Fail direction: an unreachable bridge does not fail the caller. `src/channels/hue.rs:UreqBridge::put`
  discards every outcome, and `HuePulse::run` answers 0 rooms rather than an error. On the event path the
  pulse is the LAST thing dispatched, after every channel the operator might be waiting on, so a slow or
  dead bridge cannot delay a notification.
- Thresholds: `src/pulse.rs:DEFAULT_LONG_SESSION_SECS` = 300. `src/pulse.rs:session_was_long` is closed
  at the threshold: 299 is not long, 300 is long, 400 is long. An unreadable elapsed time or threshold is
  NOT long (fails closed, because a missed pulse costs nothing).
- Required side effects: `pns pulse` reads the config only after the argument word, so `--help` and a bad
  code both answer with no machine read at all.
- Forbidden side effects: `pns pulse` never consults `hue.quiet_hours`. The gate lives at the event
  path's call site so the window stays checkable by hand while it is on
  (`tests/dispatch.rs:the_hand_run_pulse_reaches_the_bridge_inside_the_quiet_window`).
- Timeout and cancellation: each bridge call is bounded by `src/channels/hue.rs:BRIDGE_DEADLINE` = 10
  seconds.
- Idempotency and duplicates: two pulses are two independent signals; the bridge ends each by itself.
- Privacy: the exit code and nothing else reaches the bridge.
- Process ownership and cleanup: no state file, no child, nothing to clean up.
- Compatibility contract: `pulse_mode` fails CLOSED on a broken config (no roster fallback), because
  applying the event-mode fallback would let an unrelated typo switch a deliberately disabled pulse back
  on (`tests/dispatch.rs:an_unknown_plugin_never_resurrects_a_disabled_pulse`,
  `tests/dispatch.rs:a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`).

### 3. A machine with no map pulses whole rooms and states no brightness

Given `[plugins.hue]` with a bridge, a key and `rooms`, and no `[lights]` table,

When a pulse fires,

Then `src/channels/hue.rs:HuePulse::run` fetches the `room` listing, maps each wanted room name to its `grouped_light` id in WANTED order, and `src/channels/hue.rs:signal_fixtures` writes one `on_off_color` signal per group for `src/channels/hue.rs:UNMAPPED_SIGNAL_DURATION_MS` = 3000 milliseconds, stating no `dimming` at all.

- Success: the count of rooms signalled, which is the only observable fact on this path because `put` is
  fire and forget.
- Failure sources: a bridge that answered no `room` listing (0 rooms), a renamed room, a room with no
  `grouped_light` service, unparseable listing content. Each drops out silently.
- Fail direction: the notification still goes out. The pulse is dispatched after every leg and its
  failures are discarded.
- Thresholds: 3000 milliseconds, deliberately NOT the locked 4000 the routed path uses. Moving it would
  change what an unconfigured machine does without anybody asking
  (`src/channels/hue.rs:UNMAPPED_SIGNAL_DURATION_MS` doc).
- Required side effects: exactly one PUT per resolved room.
- Forbidden side effects: this path must never write a `dimming` field. A `dimming` written beside a
  signal PERSISTS after the signal ends (drill D4, 2026-08-30), and nothing on this path could ever clear
  a floor it left (`src/channels/hue.rs:signal_fixtures` doc,
  `src/channels/hue.rs:the_no_map_body_states_no_brightness_and_keeps_its_own_duration`).
- Timeout and cancellation: `BRIDGE_DEADLINE`, 10 seconds per call.
- Idempotency and duplicates: independent per fixture; one refused write does not cost another its
  signal.
- Privacy: room names come from the config or from `HUE_PULSE_ROOMS`; no event text is sent.
- Process ownership and cleanup: the bridge owns the whole effect and puts the lamp back byte for byte
  when the signal ends. Measured on a real lamp on 2026-09-01, with the lamp on and again with it off
  (`src/channels/hue.rs`, module doc). Nothing here snapshots or restores.
- Compatibility contract: `src/channels/hue.rs:DEFAULT_ROOMS` = `["3F - Studio", "2F - Kitchen"]` when
  neither `HUE_PULSE_ROOMS` nor a settings `rooms` array names any. The environment override wins and
  splits on newlines, because room names carry spaces
  (`src/channels/hue.rs:the_environment_override_wins_and_splits_on_newlines`).

### 4. A machine with a map pulses per lamp, and skips muted and held lamps

Given a `[lights]` table and an event that earned a pulse,

When `src/main.rs:run_pulse_writes` runs,

Then it resolves the map on the bridge once, and for each routed lamp skips it if the ad-hoc mute covers it or if `src/lights.rs:pulse_fires` says no, then writes `src/channels/hue.rs:pulse_body` at the brightness `src/channels/hue.rs:pulse_render` chose.

- Success: one PUT per eligible lamp, addressed as `light/<id>` (never as a group, because arbitration,
  the dim window and the mute are each per lamp).
- Failure sources: a bridge that answered any of the three listings with nothing resolves nothing and
  says nothing here (the doctor is where an unreachable bridge is reported; a warning on every
  notification for the life of a machine is noise).
- Fail direction: the notification still goes out. This runs last, after every channel leg.
- Thresholds: the locked shapes, 4000 milliseconds at brightness 100 for both `done` and `failed`
  (`src/config.rs:DEFAULT_DONE`, `DEFAULT_FAILED`), pinned byte for byte in
  `src/channels/hue.rs:the_pulse_body_carries_the_locked_colour_duration_and_brightness`.
- Required side effects: the routing complaints are RETURNED for the caller to say once, never printed
  here (`src/main.rs:routing_complaints`, `src/main.rs:say_lights_once` with `LIGHTS_QUIET_SAID`).
- Forbidden side effects: a held state is never re-derived on this path. The TICK's `lights-held` record
  is the gate, one writer and one reader, at the cost of up to one refresh interval of staleness.
- Timeout and cancellation: `BRIDGE_DEADLINE` per call, three GETs plus one PUT per lamp.
- Idempotency and duplicates: a repeat event writes a repeat signal; the bridge ends each by itself.
- Privacy: no event text reaches the bridge, only a colour, a duration and a brightness.
- Process ownership and cleanup: nothing to clean up; a pulse leaves no record.
- Compatibility contract: an unreadable `lights-held` record reads as EVERY lamp held
  (`held.is_none_or(...)` in `src/main.rs:run_pulse_writes`), so a corrupt record suppresses pulses
  rather than letting a blink write straight over a breathing lamp
  (`src/main.rs:a_held_record_that_is_absent_holds_nothing_and_one_that_will_not_read_holds_everything`).

### 5. A held lamp preempts a pulse on that lamp alone

Given a lamp routed for both `done` and `blocked`, currently holding `blocked`,

When a `done` pulse fires,

Then `src/lights.rs:pulse_fires` answers false for THAT lamp and true for every other lamp routed for `done`.

- Success: the held state is not interrupted by a four-second blink it would have to be re-armed after,
  and nothing is lost on the other lamps.
- Failure sources: none in the predicate; it is `shows.contains(&behaviour) && !lamp_is_held`.
- Fail direction: not applicable, pure function.
- Thresholds: none.
- Required side effects: none.
- Forbidden side effects: it must not fall through to a lamp that was not routed for the behaviour.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pure.
- Privacy: not applicable.
- Process ownership and cleanup: not applicable.
- Compatibility contract: this is the operator's "dedicated, but it helps out when free" ruling
  generalised: a lamp dedicated to the held states joins the pulse lamps whenever none of them is active
  (`src/lights.rs:pulse_fires` doc,
  `src/lights.rs:a_pulse_fires_on_a_lamp_it_is_routed_for_unless_a_held_state_has_that_lamp`).

## The `unread` state

### 6. The news record is written whatever the delivery did

Given any event whose behaviour is `Done` or `Failed`,

When `src/main.rs:record_news` runs on the event path,

Then it merges that epoch into `lights-news` regardless of whether any card, banner or log line was delivered, and regardless of whether the machine has a `[lights]` table or has hue enabled.

- Success: `lights-news` holds `<done_at> <failed_at>`, `0` for a kind that has not happened.
- Failure sources: no clock (returns without writing, never an epoch of zero); a publish that failed
  (dropped, fail-quiet in `record_missed`'s style); a garbled existing record (read as `News::default()`
  through `src/lights.rs:parse_news`, so the other field is lost).
- Fail direction: not on the delivery path. A record that did not land costs one lamp its colour and
  never a card.
- Thresholds: `src/lights.rs:news_after` only ever moves an epoch FORWARD (`at.max(Some(now))`), so a run
  that was slow to publish cannot put an older second back over a newer one.
- Required side effects: exactly one line rewritten in place. A wait (`Blocked`, `Unread`, `Looping`)
  writes nothing and does not even claim the file.
- Forbidden side effects: it must NOT be gated on the lamp switches. Written only while a map and a
  transport were both live, an operator who switched hue off for an evening came back to a lamp with
  nothing to say about the evening
  (`tests/dispatch.rs:the_news_record_is_written_whatever_the_lamps_are_doing`).
- Timeout and cancellation: two claim attempts, 2 milliseconds apart, then the merge goes ahead blind.
- Idempotency and duplicates: the record is owned by RENAME for the merge. Two events landing together
  (an agent that finished beside one that died) would otherwise each publish the whole line and lose the
  other's field. The claim is removed whether or not the publish landed.
- Privacy: two integers. No detail, no session id, no pane.
- Process ownership and cleanup: the claim path carries this process's pid and is removed by the same
  run.
- Compatibility contract: `src/lights.rs:parse_news` refuses anything that is not two parseable counts
  separated by one space, and the fail direction is DARK: no news, so no lamp
  (`src/lights.rs:the_news_record_survives_as_one_line_and_anything_else_is_no_news`,
  `src/main.rs:the_news_record_is_written_for_a_finished_or_a_dead_turn_and_read_back_as_it_was`,
  `tests/dispatch.rs:a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`).

### 7. The `unread` state arms off news the operator has not been back for

Given a news record, a last-interaction epoch, and whether anything is working,

When `src/lights.rs:unread_arming` is asked,

Then it answers `None` while anything is working, `None` with no interaction at all, and otherwise the freshest UNSEEN news, failure first.

- Success: `Some(Unread::Failure)` or `Some(Unread::Success)`.
- Failure sources: no readable interaction on any of the three roads; news from the future; a `working`
  reading of true.
- Fail direction: dark. A machine that cannot prove the operator was ever here cannot prove this news is
  unseen either.
- Thresholds: the age test is CLOSED and the edge test is NOT. News exactly `after_secs` old HAS waited
  that long and arms; one second under does not. News exactly AT the interaction edge is not newer than
  it and arms nothing; one second past the edge arms. Default `after_secs` is
  `src/config.rs:DEFAULT_UNREAD_AFTER_SECS` = 300 seconds; the config permits 0 (which means "at once")
  up to 86400.
- Required side effects: none. Pure.
- Forbidden side effects: never an edge at epoch zero; `None` means "nothing of that kind yet" and is
  never an epoch of 0.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pure and total.
- Privacy: reads epochs only.
- Process ownership and cleanup: not applicable.
- Compatibility contract: RED WINS when both are pending, whichever is fresher, because showing the
  calmer of the two would hide the one that needs answering (operator ruling, stated in
  `src/lights.rs:unread_arming`). News with an epoch AHEAD of `now` arms nothing of either flavour,
  including through an `after_secs` of zero
  (`src/lights.rs:success_news_waits_out_its_delay_and_failure_news_does_not`).

### 8. The interaction edge is the freshest of three roads

Given a desk idle age, a phone input epoch and a deliberate phone marker epoch,

When `src/lights.rs:last_interaction` combines them,

Then it answers the MAXIMUM of `now - desk_idle`, `phone_input_at` and `phone_marker_at`, or `None` when none of the three can be read.

- Success: one epoch, the operator's most recent touch by any road.
- Failure sources: all three probes unreadable.
- Fail direction: dark. `None` leaves the `unread` state unarmed.
- Thresholds: the desk reading is an AGE and the other two are EPOCHS, which is why it is subtracted
  rather than compared. The subtraction saturates: an idle age longer than the clock reads as an
  interaction at the epoch, never a wrapped one in the far future.
- Required side effects: `src/main.rs:last_interaction` reads the clock LAST, after the three samples.
  Hoisting the clock read above them would put `t_now` BEFORE the sample, the desk edge would land
  earlier than the true touch, and news the operator had already seen could arm the lamp. The order is
  load-bearing and is documented as not provable by a diff alone.
- Forbidden side effects: `PNS_IDLE_SECS` and `PNS_PHONE_INPUT_AGE` are NOT consulted here. They steer
  the delivery decision in `engine::decide`; the `unread` state always sees the machine's own probes.
- Timeout and cancellation: four bounded spawns (one `ioreg`, then `pgrep`, `pgrep -P`, `ps`), each
  capped at `PROBE_DEADLINE` (5 seconds, `src/system.rs`). The residual makes the desk touch read YOUNGER
  than it was, never older, which is the dark direction.
- Idempotency and duplicates: read-only.
- Privacy: ages and epochs only.
- Process ownership and cleanup: the spawned probes are bounded and reaped by the probe layer.
- Compatibility contract: taking the STALEST of the three would arm the state about news the operator had
  already seen through whichever road they were actually using
  (`src/lights.rs:the_interaction_edge_is_the_freshest_of_the_three_roads`).

## The blocked state

### 9. A waiting state starts a wait, every other event ends one

Given an event carrying a session id,

When `src/main.rs:update_blocked_marker` runs,

Then `src/lights.rs:blocked_marker_action` reads the state: a word in `LAMP_BLOCKED` is `Action::Start` and publishes one epoch to `lights-blocked/<session-id>`; anything else is `Action::End` and removes that file.

- Success: one marker per waiting session.
- Failure sources: a session id `src/safety.rs:session_id_is_safe` refuses (no marker at all, rather than
  a path escape); no clock (no marker, never a marker at epoch zero); a failed write (dropped,
  fail-quiet).
- Fail direction: not on the delivery path. This is written after the plan is dispatched, and a marker
  that did not land costs one lamp its colour and never a card.
- Thresholds: none here. The bound lives in the sweep, behaviour 10.
- Required side effects: STARTING one is gated on both lamp switches (`lamps_live` = a `[lights]` table
  AND `[plugins.hue]` enabled); a machine that never asked for the lamps must not accumulate files
  nothing will sweep. ENDING one is UNCONDITIONAL, because gating it too meant a wait that ended while
  the lamps were off kept its marker, and switching hue back on inside the backstop put `blocked` on a
  lamp for a session nobody was waiting on
  (`src/main.rs:a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live`).
- Forbidden side effects: a CLOSED set of starters and everything else ends. An unknown word treated as a
  start would hold blue on a session nobody is waiting for.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: one file per session, no generation. Stated limit: an older Stop can remove
  a newer wait's marker, because concurrent unlink does not arbitrate on this filesystem. The damage is
  bounded by the backstop and closed by the session's next event (`src/main.rs:update_blocked_marker`
  doc).
- Privacy: one epoch. The session id is a filename, already vouched for by `session_id_is_safe`.
- Process ownership and cleanup: the TICK is the only sweeper of this directory
  (`src/main.rs:sweep_blocked`).
- Compatibility contract: it runs BEFORE the delivery, so an Enter inside the arming window cannot clear
  a marker that does not exist yet; ordering shrinks the race from a plan of network legs to one file
  write. The prompt hook is the fast path and the session's Stop is the guarantee
  (`src/main.rs:arm_quota_stale_wait` doc,
  `src/lights.rs:a_blocked_event_starts_a_wait_and_every_other_event_ends_one`).

### 10. The blocked lamp reads live markers against the configured backstop

Given the `lights-blocked` directory at tick time,

When `src/main.rs:blocked_lamp` runs,

Then `src/main.rs:sweep_blocked` removes every marker past `[lights.blocked] give_up_after_secs` on the way through and returns the live epochs, and `src/lights.rs:any_blocked` lights the lamp if any survive.

- Success: `House.blocked` is true while any session is genuinely waiting.
- Failure sources: an epoch nobody can read (swept, for the same reason as an expired one: nothing could
  ever age it out otherwise).
- Fail direction: not on the delivery path. The tick is unattended.
- Thresholds: `src/lights.rs:marker_is_live` has BOTH edges closed: exactly at the bound is still live,
  one second past it is swept. A marker from the FUTURE is live too, because a clock that stepped
  backwards is not a wait that ended (saturating subtraction reads it as zero seconds old). Default bound
  16 hours; bounds 60 seconds to 7 days.
- Required side effects: the sweep and the aggregate take the SAME `give_up_after_secs`, both handed one
  value (`src/main.rs:blocked_lamp`), so a marker the aggregate ignored cannot be one the sweep kept
  (`src/main.rs:the_ticks_blocked_reading_takes_its_backstop_from_the_config_on_both_halves`).
- Forbidden side effects: no second spelling of "expired" anywhere in the module
  (`src/lights.rs:marker_is_live` doc).
- Timeout and cancellation: one directory read per tick.
- Idempotency and duplicates: a removal is owned by RENAME and never by read-then-unlink, and the epoch
  is READ AGAIN off the claim, so a marker that turned out to be live is put back rather than destroyed
  (`src/main.rs:sweep_markers`,
  `src/main.rs:a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind`,
  `src/main.rs:the_sweep_leaves_a_marker_that_is_mid_publish_alone`).
- Privacy: epochs and session-id filenames.
- Process ownership and cleanup: working files (`<name>.new.<pid>`, `<name>.sweep.<pid>`) are told from
  markers by `src/lights.rs:working_owner`, which takes the RIGHTMOST of the two suffixes and requires a
  positive process id after it. One whose owner is gone is collected; one whose owner is alive is never
  swept, because unlinking a publish caught between its open and its rename loses a wait with the agent
  still waiting (`src/lights.rs:a_working_file_is_told_from_a_marker_by_the_process_id_that_owns_it`,
  `src/lights.rs:a_working_file_is_told_by_its_rightmost_suffix_not_its_first`).
- Compatibility contract: a residual is stated rather than fixed. A marker written under a legacy id that
  spells the working grammar is judged by `owner_is_gone` rather than by `marker_is_live`, so it neither
  lights a lamp nor ages out; the operator removes it by hand. No id this crate's own callers produce can
  spell that shape (`src/main.rs:sweep_markers` doc).

## The loop state

### 11. Work past its threshold arms the loop lamp, each source timed against its own start

Given agent statuses from `herdr workspace list`, a shell marker epoch, and the live leases,

When `src/lights.rs:loop_running` is asked,

Then it is an OR of three: an agent run whose STREAK started at least `threshold_secs` ago AND is still working, a shell command whose OWN published start is at least `threshold_secs` ago, or any live lease.

- Success: `House.looping` is true.
- Failure sources: a herdr that is missing, wedged, or answering something unparseable yields no working
  workspace (`src/lights.rs:workspace_agent_statuses` returns an empty vector on any parse failure, and a
  workspace with no `agent_status` field answers the empty string, which is not `working`).
- Fail direction: dark, and not on the delivery path.
- Thresholds: `elapsed >= threshold_secs`, so exactly at the threshold arms and one second under does
  not. Default `src/config.rs:DEFAULT_LOOP_THRESHOLD_SECS` = 300 seconds; bounds 1 to 86400. A `now`
  BEHIND a start has no elapsed time in it (`checked_sub`), so a clock that stepped backwards cannot wrap
  into a number that passes every threshold.
- Required side effects: none. Pure, over a `Loop` struct rather than six positional values, four of them
  `u64`-shaped.
- Forbidden side effects: the two sources must NOT share one clock. Pooled, a fresh five-second command
  starting inside the agent grace inherited a finished agent's run and armed the lamp at once, while a
  build already ten minutes in was clocked from `now` and had to wait out the whole threshold again
  (`src/lights.rs:a_shell_command_is_measured_from_its_own_start_and_not_from_an_agents_streak`).
- Timeout and cancellation: the `herdr workspace list` call is bounded the same way the visibility model
  bounds it (`src/main.rs:lights_house`).
- Idempotency and duplicates: pure.
- Privacy: status words and epochs.
- Process ownership and cleanup: not applicable to the predicate.
- Compatibility contract: the agent arm needs BOTH halves (`agents_working` AND a long-enough streak),
  because the streak deliberately outlives the work by the grace, and the threshold alone would keep the
  lamp claiming a run in progress after everything went idle. The shell arm needs no second half, because
  its marker exists for exactly as long as its command runs
  (`src/lights.rs:work_past_the_threshold_arms_the_loop_lamp_and_both_edges_are_closed`,
  `src/lights.rs:a_live_lease_arms_the_loop_lamp_with_nothing_working_and_an_expired_one_does_not`).

### 12. The streak carries an agent run across the gap between its turns

Given the previous streak and this tick's `working` reading,

When `src/lights.rs:next_streak` runs,

Then working carries `since` forward (or starts it at `now`) and moves `last_seen` to `now`; not working keeps the streak while `now - last_seen <= grace_secs` and otherwise clears it entirely.

- Success: `lights-streak` holds `<since> <last_seen>` (`src/lights.rs:render_streak`), published by
  `src/main.rs:advance_streak`, or is removed when the streak clears.
- Failure sources: a file another hand rewrote. `src/lights.rs:parse_streak` REFUSES it rather than
  guessing, because reading a garbled half as zero would report a run as having worked since 1970, which
  passes every threshold there is.
- Fail direction: dark, and not on the delivery path. A streak that did not publish costs one lamp its
  breathing; the failure is dropped (fail-quiet).
- Thresholds: `src/main.rs:WORKING_GRACE_SECS` = 120 seconds, closed at its far edge. Exactly 120 seconds
  since the last confirmed working second still carries the streak; 121 clears it
  (`src/lights.rs:the_streak_starts_survives_a_gap_between_turns_and_clears_behind_the_grace`).
- Required side effects: the streak is advanced from `src/main.rs:lights_house`, which is the one reading
  in the house that WRITES.
- Forbidden side effects: a cleared streak is GONE rather than remembered. The next working reading
  starts a fresh one at that second. The streak is the AGENTS' alone: `agents_working` is
  `any_working(&statuses, None)`, deliberately excluding the shell marker.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: the tick is the only writer.
- Privacy: two epochs.
- Process ownership and cleanup: the file is removed, not truncated, when the streak clears.
- Compatibility contract: two numbers and not one, because `since` must never move while a run is alive
  (it is what the threshold measures against) and `last_seen` moves on every working tick (it is what the
  grace measures against).

### 13. The shell is the second producer, and its markers are swept by process id

Given `lights-shell/<shell-pid>` files, each holding the second a tracked command started,

When `src/main.rs:sweep_shell_markers` runs at tick time,

Then it removes any file whose name is not a positive process id or whose process is gone, and returns the OLDEST epoch a live shell is holding.

- Success: one epoch, or `None`.
- Failure sources: none that stop the tick. Nothing in this crate writes these files; the interactive
  shell does (`src/main.rs:LIGHTS_SHELL_DIR` doc).
- Fail direction: dark, and not on the delivery path.
- Thresholds: none in the sweep itself. Liveness is by process id, not by age.
- Required side effects: the sweep lives WITH the read, because the tick is the only process that ever
  looks in this directory and a shell killed mid-command leaves a file its own `precmd` will never run to
  remove.
- Forbidden side effects: an epoch that cannot be read is NOT swept while its shell is alive. The shell
  publishes with a truncating redirect, so a tick landing between that open and the write sees an empty
  file for a command that is genuinely starting; unlinking it there wins the race and the build then runs
  to completion with no marker at all. This is the one place it differs from `sweep_blocked`
  (`src/main.rs:a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone`).
- Timeout and cancellation: one directory read per tick.
- Idempotency and duplicates: one file per interactive shell, never one shared file, because every shell
  runs the same two bash-preexec functions and a shared path is a marker any other pane erases.
- Privacy: an epoch and a process id. No command text.
- Process ownership and cleanup: the pid in the NAME is what collects a killed terminal's file
  (`src/main.rs:a_marker_whose_shell_is_gone_is_swept_and_never_read`,
  `src/main.rs:a_name_that_is_not_a_shell_pid_is_swept`).
- Compatibility contract: the OLDEST and not the freshest. The freshest would restart the clock every
  time any pane ran anything, so a build running for an hour beside a prompt somebody keeps typing at
  would never reach a threshold measured in minutes
  (`src/main.rs:the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding`).

### 14. `pns loop begin` takes a lease and registers the tick that reads it

Given `pns loop begin` typed in a herdr pane, or with `--pane <id>`,

When `src/main.rs:loop_mode` runs,

Then it writes `now` to `lights-loop/<pane>` and registers the lights tick for the WHOLE lease length (`[lights.loop] lease_timeout_secs`, not the ordinary 300 seconds).

- Success: exit 0, a lease file, and a spool record whose `until` outlasts the lease.
- Failure sources: no `HERDR_PANE_ID` and no `--pane` (refusal
  `"pns: loop: no HERDR_PANE_ID in this environment, so there is no pane to key the lease to; run it inside the pane, or name one with --pane"`,
  exit 2); a pane id `src/safety.rs:pane_file_is_safe` refuses
  (`"pns: loop: {pane:?} is not a pane id this can key a lease to"`, exit 2); an unknown verb or arity
  (`src/lights.rs:LOOP_USAGE`, exit 2); no clock
  (`"pns: loop: the clock cannot be read; the lease was not taken"`, exit 1); an unwritable lease
  (`"pns: loop: the lease could not be written: {error}"`, exit 1).
- Fail direction: not on the delivery path. This is a typed command with a human waiting, so every
  failure is LOUD and exits non-zero: a lease that was not taken is a lamp that never lights, and
  reporting success for one is the worst outcome available.
- Thresholds: none at the command. The lease's own timeout is behaviour 15.
- Required side effects: the tick registration, because nothing else will register it in time. The tick's
  own lease is refreshed by EVENT traffic, so a lease taken by hand in a pane that then goes quiet, which
  is exactly the overnight run this verb exists for, would be read by a tick that expired minutes into it
  (`tests/dispatch.rs:a_lease_taken_by_hand_schedules_the_tick_that_reads_it`).
- Forbidden side effects: no guessed pane. Picking one would key the lease to a pane whose ordinary
  traffic will never renew it, and the lamp would breathe for the whole timeout with nothing behind it.
- Timeout and cancellation: the registration cannot block: it is one file written by rename into a
  directory, so a daemon that is dead, wedged or mid-restart changes nothing about the call.
- Idempotency and duplicates: re-taking a lease rewrites one epoch. Re-registering the tick KEEPS a due
  second already pending, so an event storm cannot keep pushing the tick away from itself
  (`src/main.rs:schedule_lights_tick`).
- Privacy: one epoch, keyed by pane id.
- Process ownership and cleanup: the lease outlives the process that took it, by design.
- Compatibility contract: `pns loop end` on a machine that never began is a removal of a file that is not
  there and is NOT a failure (`src/main.rs:end_lease`). A removal that genuinely failed is reported:
  `"pns: loop: the lease could not be given back ({error}); the loop lamp keeps breathing until it times out"`,
  exit 1 (`src/main.rs:a_lease_that_could_not_be_given_back_is_reported_rather_than_called_a_success`,
  `src/lights.rs:a_lease_is_keyed_to_the_pane_it_was_typed_in_and_refused_when_there_is_none`).

### 15. A lease is renewed by its pane's own event traffic, and never created by one

Given an event carrying a pane,

When `src/main.rs:renew_loop_lease` runs,

Then it writes `<now>\n` THROUGH an existing handle opened without `create`, then `set_len`s the file; if no lease file exists, nothing happens.

- Success: the lease's epoch moves to now
  (`tests/dispatch.rs:a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`).
- Failure sources: no lease file, an unsafe pane id, no clock, an unwritable file. All silent: a lease
  that did not renew costs the lamp one timeout and this process has no reader for a complaint.
- Fail direction: not on the delivery path.
- Thresholds: expiry is `marker_is_live` against `lease_timeout_secs`, both edges closed. Default 3900
  seconds, chosen because the harness's own wakeup scheduler clamps a sleep to 3600 seconds, so the
  longest legitimate gap between two events from a live loop is an hour and a timeout AT the hour would
  drop a lease that was about to be renewed.
- Required side effects: the open must state NO `create`. A `pns loop end` that lands after the open
  sends these bytes to an inode nobody can reach any more; a look-then-publish would have written the
  lease back into existence and left the lamp breathing for a whole timeout over work that had finished
  (`src/main.rs:a_lease_is_renewed_only_while_it_exists_and_swept_once_it_times_out`).
- Forbidden side effects: it must NOT truncate first. A tick reading the file mid-renewal would see an
  empty one and sweep the lease. Both epochs are ten digits, so a read caught between them sees a mix of
  two same-length numbers, seconds out rather than unparseable; the trailing `set_len` is for the day
  that stops being true
  (`src/main.rs:a_renewal_writes_through_the_lease_it_found_rather_than_publishing_a_new_one`).
- Timeout and cancellation: one open, one write, one `set_len`.
- Idempotency and duplicates: every event from that pane rewrites one epoch in place.
- Privacy: one epoch.
- Process ownership and cleanup: a stranded lease is swept by the tick's `sweep_leases` once it times
  out.
- Compatibility contract: a machine with no lamps pays one failed open and keeps no state.

## Precedence and resolution

### 16. The house holds every state at once, and each lamp resolves its own

Given a `House { blocked, looping, unread }`,

When `src/lights.rs:active_held` runs,

Then it returns every active state, most urgent first, and `src/lights.rs:shown` filters that list by one lamp's OWN `shows` routing and takes the first survivor.

- Success: one blue lamp and one violet lamp can be lit at the same moment, because they are routed for
  different words.
- Failure sources: an empty `shows` list leaves the lamp out of the walk entirely rather than costing a
  write that does nothing (`src/channels/hue.rs:Routing.lamps` doc).
- Fail direction: not on the delivery path.
- Thresholds: none. The rank is `Blocked` > `Looping` > `UnreadFailure` > `UnreadSuccess`, the
  declaration order of `src/lights.rs:Held`, with no runtime sort behind it.
- Required side effects: none. Pure.
- Forbidden side effects: a state nothing routes to a lamp leaves it dark rather than falling through to
  a lamp that was not asked.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pure.
- Privacy: not applicable.
- Process ownership and cleanup: not applicable.
- Compatibility contract: the operator's own ruling, applied literally: a question waiting on them
  outranks work in progress, and work in progress outranks news about work that has already finished
  (`src/lights.rs:Held` doc,
  `src/lights.rs:every_held_state_is_active_at_once_and_they_rank_blocked_loop_then_unread`,
  `src/lights.rs:one_lamp_shows_the_most_urgent_state_it_is_routed_for_and_nothing_it_is_not`).

### 17. Names resolve lamp, then room, then zone, and each question on its own

Given a bridge inventory and the config's `[lights.lamp/room/zone.<name>]` declarations,

When `src/channels/hue.rs:resolve` runs,

Then for each lamp it walks `src/channels/hue.rs:LEVELS` (`["lamp", "room", "zone"]`) INDEPENDENTLY for each question (`shows`, `dim_window`), and the winning level supplies the WHOLE answer to its question.

- Success: a `Routing` of lamps that carry something, plus `unresolved` names and `refusals`.
- Failure sources: a name no lamp answers is reported as `Missing::NotOnBridge`
  (`"lights: `<name>` (<level>) is not on the bridge"`) or `Missing::AddressedNothing`
  (`"lights: `<name>` (<level>) is on the bridge, but it holds no lamp"`), which are different problems
  an operator fixes in different places. A bridge that refused ANY of the three listings resolves nothing
  at all (`src/channels/hue.rs:resolve_on_bridge` returns `None`), because a config resolved against an
  empty inventory would report every name it holds as a typo.
- Fail direction: an unreachable bridge on the delivery path costs the lamps and NOTHING else. The
  notification legs have already fired; `run_pulse_writes` returns an empty complaint list and says
  nothing, because the doctor is where an unreachable bridge is reported
  (`tests/dispatch.rs:a_lights_table_changes_nothing_about_an_ordinary_notification`).
- Thresholds: none.
- Required side effects: levels never MERGE. A union would re-add exactly what a lamp-level declaration
  deliberately left out, and the operator's own routing needs one lamp in a room to carry the held states
  while the rest carry the pulses
  (`src/channels/hue.rs:the_most_specific_declaration_that_names_a_lamp_supplies_its_whole_behaviour_set`,
  `src/channels/hue.rs:each_question_resolves_on_its_own_so_a_lamp_can_state_one_and_inherit_the_other`).
- Forbidden side effects: two ZONES answering one question for one lamp is a REFUSAL naming both, never a
  guess:
  `"lights: `<lamp>`is covered by <n> zone declarations that each state`<question>` (<names>); there is nothing more specific to break the tie, so that lamp answers none of them"`.
  A contested `shows` is an empty set (dark lamp); a contested `dim_window` skips the lamp entirely,
  which is a THIRD answer distinct from silence, because collapsed into one `None` the refusal took the
  no-window path and ran the lamp at full brightness all night
  (`src/channels/hue.rs:a_lamp_two_zones_both_answer_for_is_refused_with_both_named`,
  `src/channels/hue.rs:a_dim_question_two_zones_both_answer_leaves_that_lamp_dark_rather_than_bright`).
- Timeout and cancellation: three GETs, each bounded (10 seconds on the event path and the doctor, a
  fifth of the refresh interval on the tick, 1 second for the typed mute command).
- Idempotency and duplicates: `src/channels/hue.rs:remember` deduplicates refusals, one per problem
  rather than one per lamp that met it.
- Privacy: only lamp, room and zone names cross the wire, and only in the direction of reading.
- Process ownership and cleanup: no state written.
- Compatibility contract: THE BRIDGE'S CURRENT MEMBERSHIP IS THE TRUTH. A lamp named by room A's
  declaration and physically moved to room B answers room B's, because the join is taken from the listing
  this call was handed
  (`src/channels/hue.rs:a_lamp_moved_to_another_room_answers_the_room_it_is_in_now`). Names are matched
  exactly, so a case-folded name is a typo rather than a name to forgive
  (`src/channels/hue.rs:a_case_folded_name_is_a_typo_rather_than_a_name_to_forgive`). The two joins
  differ in shape: a room's `children` are DEVICE ids reached through a light's `owner.rid`, a zone's
  `children` are LIGHT ids reached directly (`src/channels/hue.rs:inventory`).

## The tick

### 18. The tick re-derives everything, says nothing, and exits 0

Given `pns lights tick`,

When it runs,

Then it holds nothing in memory between runs, exits 0 on every path, and prints nothing on every happy one.

- Success: exit 0, empty stdout, empty stderr, three times a minute forever, however many times it runs
  (`tests/dispatch.rs:the_tick_says_nothing_at_all_however_many_times_it_runs`).
- Failure sources: an unreadable config (returns 0, having asked for nothing); hue absent or disabled
  (returns 0, KEEPING the held record); no `[lights]` table or no clock (clears held lamps and returns
  0); credentials gone (returns 0, keeping the record); an unreachable bridge (returns 0, changing
  nothing)
  (`tests/dispatch.rs:the_tick_exits_zero_with_no_config_no_table_hue_off_and_an_unreachable_bridge`).
- Fail direction: this is not the delivery path. A tick is not an event and reaches no channel; the tests
  assert `!sandbox.fired("hermes") && !sandbox.fired("mobile")`.
- Thresholds: `[lights] refresh_secs`, default `src/config.rs:DEFAULT_REFRESH_SECS` = 12, bounds 10
  (`MIN_REFRESH_SECS`, the transport deadline) to 30 (`MAX_REFRESH_SECS`).
- Required side effects: `src/main.rs:sweep_legacy_state` runs first, then the house is derived, then the
  writes. NOTHING TO LIGHT AND NOTHING TO PUT OUT IS NO BRIDGE CALL AT ALL, which keeps an idle machine
  off the network several times a minute.
- Forbidden side effects: the journal is READ and never CLAIMED. `claim_journal` is how the replay
  CONSUMES a queue; a tick that claimed it would delete the misses the operator has not seen yet
  (`src/main.rs:lights_tick` doc).
- Timeout and cancellation: each bridge call is bounded by `src/main.rs:tick_bridge_deadline` =
  `refresh_secs / 5`, at least 1 second. The daemon bounds the whole child by `src/main.rs:child_bound`,
  which for `LIGHTS_JOB` is at least `MAX_REFRESH_SECS` + the per-call deadline at that interval + one
  reap tick (37 seconds at the production clock).
- Idempotency and duplicates: every state is re-derived from scratch. A divergence between what a process
  believes and what the disk says is the class this crate keeps paying for (`src/lights.rs` module doc).
- Privacy: no event text is ever read by the tick.
- Process ownership and cleanup: the tick is a fresh process each time, re-executed by the daemon.
- Compatibility contract: THE FEATURE BEING OFF STILL PUTS A HELD LAMP OUT, but only where a bridge can
  still be NAMED. `[lights]` removed with hue enabled clears and forgets; hue switched off keeps the
  record, so the tick after the switch goes back on still has a name to write the clear to
  (`tests/dispatch.rs:switching_the_lamps_off_puts_out_a_held_glow_and_switching_hue_off_keeps_the_record`).

### 19. The tick's write order is the behaviour

Given a resolved routing and the states the house is holding,

When `src/main.rs:run_tick_writes` runs,

Then the order is: claim the lock, resolve, compute what breathes, re-read the record and stand down if it moved, clear the DIFFERENCE (what was held and is not held now), write the BARE held record, breathe, and only then write the PHASE record.

- Success: complaints returned for the caller to say once; every lit lamp accounted for by name.
- Failure sources: a lock somebody live holds (return, having done nothing); a record that moved under
  this run (return); a bridge that answered no listing (return, changing nothing at all); a held record
  that would not publish (complaint
  `"pns lights: the held record could not be written ({error}); no lamp was armed, because nothing would have been able to put one out"`,
  and NOTHING is armed).
- Fail direction: not the delivery path.
- Thresholds: the breath budget is `refresh_secs * 1000` LESS what the resolve already spent.
- Required side effects: a clear computed before the arm, or a record written before the clear, is a lamp
  left lit with nothing that knows its name. Every held body is a plain state write that does NOT expire.
- Forbidden side effects: the pre-arm record is BARE, deliberately dropping any phase this tick read: a
  killed child cannot finish a fade, so a bare token is a lamp this run cannot promise landed anywhere in
  particular. The phase write is guarded by a re-read of the same bare list this tick wrote, so a return
  that cleared the record mid-breath is left cleared rather than resurrected
  (`src/main.rs:a_record_cleared_during_the_breath_is_left_cleared_rather_than_resurrected`,
  `src/main.rs:the_phase_reaches_disk_only_after_the_breath_that_earned_it_has_run`).
- Timeout and cancellation: the child holds itself open until the last fade has been ISSUED, one
  `FADE_LEAD_MS` before the budget ends. A driver killed mid-breath costs a lamp frozen at its last
  brightness and NEVER a lamp nothing can put out, because the record and the clear are already on disk
  before the first sleep.
- Idempotency and duplicates: ONE TICK DRIVES THE HOUSE AT A TIME. The lock is taken BEFORE the resolve,
  because two ticks that both got past a record comparison would still spend a whole interval issuing
  fades at each other (`src/main.rs:a_second_tick_stands_down_while_a_first_still_holds_the_lamps`). The
  lock deliberately does NOT lock out the event path: the operator's return clears the record from a
  process that holds no lock and must never wait on one, and the re-read is that case's guard
  (`src/main.rs:a_tick_whose_record_moved_under_it_stands_down_rather_than_re_arming_the_lamps`).
- Privacy: fixture paths only.
- Process ownership and cleanup: `src/main.rs:HeldLock`'s `Drop` gives the lock back on every exit path.
  A failed release prints
  `"pns: the lock <path> could not be given up ({error}); the next claimant waits it out"`.
- Compatibility contract: STALE lamps are put out as a DIFFERENCE, so a lamp dropped by a dim window, a
  mute, a config edit or the condition simply ending is covered by one line rather than four
  (`src/main.rs:a_tick_arms_a_held_lamp_records_it_and_a_dark_house_puts_it_out_by_name`,
  `src/main.rs:a_lamp_this_arm_wrote_to_stays_held_rather_than_being_put_out_behind_the_arm`,
  `src/main.rs:a_phased_record_clears_by_its_bare_path_never_by_the_suffix`,
  `src/main.rs:a_tick_whose_bridge_answered_nothing_keeps_the_record_it_was_holding`,
  `tests/dispatch.rs:a_tick_with_nothing_left_to_show_puts_out_the_glow_it_was_holding`).

### 20. The tick keeps itself scheduled while anything is in flight

Given a tick that has just run,

When `standing.in_flight` (a streak, a shell marker, or a live lease) or anything is active,

Then it calls `src/main.rs:schedule_lights_tick` with `ORDINARY_LEASE_SECS`.

- Success: the lease outlasts the threshold the run is climbing toward
  (`tests/dispatch.rs:a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold`).
- Failure sources: a broken spool. The registration failure is DROPPED: a lamp that did not re-arm must
  never cost a card, a line of stdout or an exit code
  (`tests/dispatch.rs:a_registration_that_cannot_be_written_costs_the_event_nothing`).
- Fail direction: not the delivery path.
- Thresholds: `until = due.max(now + lease_secs)`, at least as far as the due second, because a lease
  that ended before its own job's first run is a record `validate_shape` refuses. `MAX_REFRESH_SECS` is
  under the ordinary lease precisely so a long refresh cannot EXTEND that lease
  (`tests/dispatch.rs:an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer` asserts 300
  and 43200 EXACTLY).
- Required side effects: the tick renews itself because its lease was refreshed by EVENTS alone, and a
  shell command produces no events at all while the loop threshold (300 default, 360 on the operator's
  machine) is PAST the 300-second event lease. Without this the one lamp whose whole job is a long run
  could never arm itself.
- Forbidden side effects: it is bounded by the CONDITION, not self-perpetuating. A house holding nothing
  with no run and no lease renews nothing
  (`tests/dispatch.rs:a_tick_with_nothing_in_flight_lets_its_own_lease_lapse`).
- Timeout and cancellation: the registration is one rename and cannot block.
- Idempotency and duplicates: re-registering replaces the job by name and KEEPS a pending due second, so
  an event storm cannot push the tick away from itself.
- Privacy: a job record with the args `["lights", "tick"]`.
- Process ownership and cleanup: the daemon reaps the child; the spool record expires with its `until`.
- Compatibility contract: ONE JOB FOR THE WHOLE HOUSE (`src/main.rs:LIGHTS_JOB` = `"lights"`), not one
  per lamp, because a second job would be a second writer of the same bulbs.

## Breathing and phases

### 21. A breath fills the interval, each fade leading the one before it

Given a budget in milliseconds, a cycle of `src/lights.rs:Leg`s and a `Resume`,

When `src/lights.rs:breath_fades` runs,

Then it walks the cycle from `resume.next_leg`, issuing the first fade at `resume.first_due_ms` and each one after it a step later, where the step is THAT LEG'S OWN `src/lights.rs:step_ms` (`duration_ms - FADE_LEAD_MS`, floored at 1).

- Success: a schedule where every fade is issued strictly INSIDE the budget and the LAST one ends after
  it, so the lamp is still moving when the child exits.
- Failure sources: a `first_due_ms` at or past the budget yields an EMPTY schedule. The lamp keeps what
  it was last told and the next tick picks the breath back up
  (`src/lights.rs:a_budget_that_cannot_fit_even_one_fade_is_empty`).
- Fail direction: not the delivery path.
- Thresholds: `src/lights.rs:FADE_LEAD_MS` = 50 milliseconds, operator-locked on a real lamp. The doc
  states plainly that nothing measured what a lead of zero looks like. Fade duration bounds are
  `MIN_FADE_MS` = 200 to `MAX_FADE_MS` = 5000 (`src/config.rs`). There is no fade COUNT: the walk stops
  at the first start that would fall at or past the budget, which is what lets a cycle whose legs differ
  in duration contribute a shorter step at the short leg
  (`src/lights.rs:the_accent_leads_the_fades_around_it_by_the_same_lead_every_other_fade_gets`).
- Required side effects: `start_ms` is measured from the TICK's own start, never from the fade before it,
  because a per-fade delay accumulates every sleep's overshoot and the breath drifts past its interval.
- Forbidden side effects: `src/main.rs:drive_breaths` checks the budget IMMEDIATELY BEFORE each write
  rather than once from the schedule, because writes are synchronous and sequential, so the schedule is
  only ever NOMINAL. A dropped fade costs one turn-around; an issued one costs two children writing to
  one lamp.
- Timeout and cancellation: the sleep is saturating, so a write that ran long issues the next fade at
  once rather than sleeping a wrapped duration.
- Idempotency and duplicates: one landing per lamp is kept, overwritten as later fades are issued.
- Privacy: brightness numbers only.
- Process ownership and cleanup: the last fade keeps running on the bridge with no child left to
  interrupt it. The residual pause is bounded by one step and is zero on most ticks, since the two
  resolves are of the same order and cancel on average (`src/lights.rs:breath_fades` doc).
- Compatibility contract: the FIRST fade of every tick carries the colour and the `on`
  (`src/channels/hue.rs:breath_arm_body`); every fade after it states brightness and duration alone
  (`src/channels/hue.rs:fade_body`), so the bridge has nothing else to reconcile mid-transition. This
  holds on a resumed tick too, so an externally switched-off lamp comes back on
  (`src/lights.rs:each_fade_leads_the_one_before_it_so_the_lamp_never_pauses_at_an_end`,
  `src/lights.rs:every_last_fade_is_issued_inside_the_budget_and_lands_after_it`,
  `src/channels/hue.rs:the_arm_states_the_colour_and_the_first_fade_and_every_fade_after_it_states_neither`).

### 22. A phase resumes the breath across two ticks

Given a `lights-held` entry `<path>@<end-unix-ms>:<brightness>:<state>`,

When `src/lights.rs:resume_from` is asked for a lamp now showing state S,

Then `first_due_ms` is `end_unix_ms - now_ms - FADE_LEAD_MS`, saturating at zero, and `next_leg` is the leg after the one whose brightness the record names.

- Success: the schedule this tick issues is the next leg of the breath the previous tick was already
  running, not a fresh one restarted at the interval's zero
  (`src/main.rs:a_resumed_breath_composes_across_two_ticks_on_a_fake_clock`).
- Failure sources: no entry, no phase, a phase belonging to ANOTHER state, a phase naming a brightness
  the cycle about to run has no leg for, or a phase more than one step ahead. All five return
  `Resume::default()` (due at once, taking leg zero, which every cycle builds as its low end).
- Fail direction: not the delivery path.
- Thresholds: STALENESS is one `step_ms`, of the LEG THE RECORD NAMES rather than of the cycle at large.
  A `first_due_ms` of exactly that step resumes; one millisecond past it starts over. The bound is a LAW
  and not a tolerance: the tick that wrote the phase issued that leg's fade strictly inside its own
  budget, that fade lands one leg-duration later, and the next tick begins at most the daemon's slop
  after that budget ended (`src/lights.rs:a_phase_sitting_further_ahead_than_one_step_reads_as_stale`,
  `src/lights.rs:a_tick_that_ended_on_the_accent_falls_next_and_inherits_only_the_accents_own_step`).
- Required side effects: the phase is written with `now_ms + spent_ms + end_relative_ms`, because a
  landing is reported from the DRIVER's start, which is `spent_ms` after the tick's. Dropping that term
  would put every end a whole resolve early and the next tick would take the breath over before this one
  finished it.
- Forbidden side effects: a phase another STATE left is never resumed from. The slow shapes land their
  last fade almost four seconds past the interval that issued them, so a lamp that was looping and is now
  blocked would wait that fade out before its first blue body reached the bridge: the locked precedence,
  arriving up to a whole fade late
  (`src/main.rs:a_lamp_that_changed_state_starts_its_new_colour_at_once_rather_than_resuming`,
  `src/lights.rs:a_phase_another_state_left_behind_is_started_over_rather_than_resumed`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: the state word in the token is `src/lights.rs:Held::word` (`"blocked"`,
  `"loop"`, `"failure"`, `"success"`), four words for four states, deliberately NOT the routable
  behaviour word, because the two unread flavours share a behaviour and do not share a colour.
- Privacy: a fixture path, a millisecond epoch, and a state word.
- Process ownership and cleanup: a malformed suffix is NO PHASE, never an unreadable record: losing the
  phase costs one fade of resume, while inventing an unreadable path would cost the lamp
  (`src/lights.rs:parse_held_token`,
  `src/lights.rs:a_bare_token_reads_as_no_phase_and_a_malformed_one_falls_back_to_bare`).
- Compatibility contract: `@` and `:` appear in neither a fixture path (`light/<id>` or
  `grouped_light/<id>`, the id a bridge-issued universally unique identifier) nor a state word, so the
  token round trips through the same whitespace-separated line the bare record always used, with nothing
  to escape (`src/lights.rs:a_held_entrys_phase_round_trips_through_its_rendered_token`,
  `src/main.rs:a_held_records_phase_round_trips_through_remember_held_and_read_held`,
  `src/main.rs:a_bare_token_on_disk_still_reads_as_a_held_lamp_with_no_phase`).

### 23. Two breathing lamps share one schedule

Given two lamps with different shapes (blocked at 2000 ms, loop at 4000 ms),

When `src/main.rs:drive_breaths` runs,

Then all fades are pooled into ONE schedule sorted by `(due_ms, path)` and issued against ONE clock, and a lamp whose fades are already done simply stops.

- Success: a lamp whose write took a moment does not push every later fade of every lamp out by that
  moment; the overshoot is absorbed rather than accumulated
  (`src/main.rs:two_breathing_lamps_share_one_schedule_rather_than_running_back_to_back`).
- Failure sources: a `put` the bridge refused is invisible (fire and forget).
- Fail direction: not the delivery path.
- Thresholds: `at_ms >= budget_ms` breaks the loop, read AGAIN after the sleep because the sleep is the
  one thing here allowed to overshoot.
- Required side effects: every LANDING is derived from a write that actually happened, at the moment it
  actually started, never from the nominal schedule; otherwise the next tick would take the breath over
  early on every interval the bridge ran slow in.
- Forbidden side effects: nothing is issued at or past the budget.
- Timeout and cancellation: the clock and the sleeper are PARAMETERS, so a test drives the same cadence
  without living the interval.
- Idempotency and duplicates: one landing per lamp, replaced as later fades are issued.
- Privacy: brightness and duration.
- Process ownership and cleanup: the child exits inside its budget with its last fade still running.
- Compatibility contract: `src/config.rs:DEFAULT_REFRESH_SECS` = 12 is a BREATH BUDGET rather than a
  round number: twelve seconds carries seven of the locked two-second shape, and three or four of the
  four-second one depending on what that tick's resolve took off the budget first.

## Windows and rendering

### 24. The dim window answers per lamp and per behaviour

Given a lamp with a resolved `DimWindow` and the minute of the local day,

When `src/channels/hue.rs:dim_showing` runs,

Then a lamp with NO window is `Showing::Full`; outside the window it is `Full`; inside it a behaviour on `dim_behaviours` is `Dimmed` and one that is not is `Dark`.

- Success: a room can breathe faintly about a wait all night while refusing to strobe green about a
  build.
- Failure sources: an unparseable `dim_window` string is a refusal for THAT LAMP ALONE:
  `"lights: `<lamp>` has dim_window "<stated>", which is not a HH:MM-HH:MM window; that lamp stays dark"`,
  and the lamp is skipped (`src/channels/hue.rs:window_refusal`,
  `src/main.rs:the_tick_says_what_could_not_be_resolved_and_what_was_refused`).
- Fail direction: an unreachable bridge means no routing at all, so no lamp is written; the notification
  legs already fired.
- Thresholds: `quiet_now`'s half-open boundaries (see Table 3). An UNREADABLE CLOCK is treated as INSIDE
  the window, because a flash at 3am is what the window was set to prevent and a missed signal costs
  nothing (`src/channels/hue.rs:a_clock_this_machine_cannot_read_is_treated_as_inside_the_window`).
- Required side effects: `Dark` writes nothing at all: `pulse_render` returns `None`, and the tick
  `continue`s past the lamp so it joins the stale set and is put out by name.
- Forbidden side effects: an EMPTY `dim_behaviours` needs no second mode. A window with nothing enabled
  suppresses everything, which is the bedroom rule with no special case in the code
  (`src/channels/hue.rs:a_window_with_nothing_enabled_suppresses_every_behaviour_and_needs_no_mode`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pure.
- Privacy: a minute of the day.
- Process ownership and cleanup: not applicable.
- Compatibility contract: a lamp with no window pays NOTHING and behaves exactly as it did, which is what
  makes the whole feature opt-in. `[plugins.hue] quiet_hours` is NOT a rung of the routed chain: a typo
  there cannot darken a routed lamp
  (`tests/dispatch.rs:a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`).

### 25. The dim form is one shape, and the colour still says which state it is

Given a held state and `Showing::Dimmed`,

When `src/channels/hue.rs:held_render` runs,

Then it returns that state's own locked colour with the ONE shared `[lights.dim]` shape.

- Success: the locked figures. Blocked: `x 0.1532, y 0.0475`, 2000 ms, high 100, low 30. Loop:
  `x 0.213, y 0.0766`, 4000 ms, high 80, low 10, with a 200 ms flash to 100 at the peak
  (`breathe_then_flare`, the one shape carrying an accent). Unread failure: `x 0.675, y 0.322`, 4000 ms,
  high 60, low 10. Unread success: `x 0.50, y 0.40`, same shape. Dim: 3000 ms, high 7, low 1
  (`src/config.rs:DEFAULT_DIM`; drill D4 measured a lamp asked for one percent reporting 1.19, which is
  its own floor rather than a rounding).
- Failure sources: none. `held_render` is total over the four states.
- Fail direction: not the delivery path.
- Thresholds: brightness is a percent and ZERO is refused at load rather than read as off; `low` above
  `high` is refused too, because with the ends reversed a fade to `high` would move the lamp DOWN
  (`src/config.rs:Breath` doc, `ends_agree`).
- Required side effects: `src/channels/hue.rs:pulse_render` returns `None` for every held behaviour,
  because a lamp asked to flash a state it holds would be armed with something nobody measured.
- Forbidden side effects: a dimmed PULSE is the same blink at `lights.dim.low`, not the dim breath; there
  is no low end for a blink to fade to.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pure.
- Privacy: not applicable.
- Process ownership and cleanup: `src/channels/hue.rs:clear_body` is `{"on":{"on":false}}`, OFF and not a
  restore: nothing snapshotted what the lamp was doing before the breath took it, and a `grouped_light`
  read carries no colour at all, so there is nothing honest to put back
  (`src/channels/hue.rs:what_puts_a_held_lamp_out_is_off_and_not_a_restore`).
- Compatibility contract: every colour and shape here was set on a real lamp under the operator's
  observe-adjust-lock protocol (2026-08-31 and 2026-09-01), so a change to one is a change to something
  that was looked at (`src/config.rs`, the "five locked shapes" comment;
  `src/channels/hue.rs:each_held_state_renders_its_own_locked_colour_and_shape`).

### 26. `FAILURE_COLOR` carries two jobs and is one constant

Given the failure pulse and the `unread` failure flavour,

When either renders,

Then both read `src/pulse.rs:FAILURE_COLOR`.

- Success: the two look like one statement, "something died", said once as a blink and once as a breath.
- Failure sources: none.
- Fail direction: not applicable.
- Thresholds: none.
- Required side effects: none.
- Forbidden side effects: a second red constant, which would let the two drift into looking like
  different events (`src/pulse.rs:FAILURE_COLOR` doc).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: not applicable.
- Privacy: not applicable.
- Process ownership and cleanup: not applicable.
- Compatibility contract: `src/pulse.rs:UNREAD_SUCCESS_COLOR` (daylight) is deliberately a colour nobody
  reads as an alarm, so a success that has merely gone unseen does not compete with the red beside it.

## The ad-hoc mute

### 27. `pns lights quiet` mutes one place, lights only, for a bounded while

Given `pns lights quiet [<place> [<duration>|off]]`,

When `src/main.rs:lights_quiet` runs,

Then a bare command REPORTS and mutes nothing; `<place>` mutes until the operator's quiet hours end; `<place> <duration>` mutes for that duration; `<place> off` unmutes.

- Success: exit 0 and one line per live place:
  `` pns lights: `<place>` is quiet for another <n> minute(s) ``, or `"pns lights: nothing is quiet"`.
- Failure sources: a place no lamp, room or zone name reaches
  (`` pns: lights quiet: `<place>` is no lamp, room or zone this can quiet; a mute reaches <names> ``, or
  `"this config claims no lamp at all, so there is nothing a mute could reach"`, exit 2); a duration
  outside `src/quiet.rs:parse_duration`'s bounds (exit 2); any other arity
  (`"pns: lights quiet takes a place, optionally with a duration or off, or nothing at all"`, exit 2); no
  clock (`"pns: state error (the clock cannot be read); the mute was not set"`, exit 1); an unwritable
  file (`"pns: state error (lights-quiet could not be written: {error}); the mute was not set"`, exit 1).
- Fail direction: an unreachable bridge does NOT refuse the command. The declared names alone are still a
  vocabulary a mute can enforce once the transport is back (`src/channels/hue.rs:mutable_names`,
  `src/main.rs:bridge_inventory`).
- Thresholds: `src/lights.rs:MAX_MUTED_PLACES` = 32 lines. A file past it is refused WHOLE
  (`"pns: state error (lights-quiet holds <n> lines, more than the 32 places it keeps); nothing is quiet, and the next pns lights quiet write replaces the file"`),
  and a command that would write a 33rd line is REFUSED rather than truncating
  (`"pns: lights quiet: 32 places are already quiet, which is every line lights-quiet keeps; the mute was not set, and `pns
  lights quiet <place> off` ends one"`). Expiry is half open through `src/quiet.rs:is_muted`
  (`now < expiry`), so a mute ends ON the second it names.
- Required side effects: complaints are printed BEFORE anything is written, because the write republishes
  the whole file and an operator whose file was unreadable is losing what it held. Expired entries are
  dropped on the way past, so a machine that mutes a different room every night cannot reach the line cap
  and have the whole file refused (`src/lights.rs:muted_after`,
  `src/lights.rs:off_clears_one_place_and_leaves_the_others_where_they_were`).
- Forbidden side effects: NO REPORT after a failed write. `kept` is what the file WOULD have held: for a
  failed mute it would say the place is quiet when it is not, and for a failed `off` it would say nothing
  is quiet while the old mute is still on disk
  (`tests/dispatch.rs:a_lights_quiet_write_that_failed_reports_the_disk_and_not_the_list_it_built`). The
  command must NOT touch `pns quiet`, cards, banners or the durable log: LIGHTS ONLY is the operator's
  own scope, and the two mutes share a duration parser and nothing else
  (`tests/dispatch.rs:an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`).
- Timeout and cancellation: the bridge dial for a wider vocabulary uses
  `src/channels/hue.rs:TYPED_COMMAND_DEADLINE` = 1 second per call, not the transport's 10, because three
  calls at ten seconds is half a minute before a mute typed at bedtime says anything at all. The dial is
  on the MISS path alone: muting a room the config already declares costs no network
  (`src/main.rs:asks_the_bridge`).
- Idempotency and duplicates: re-muting a place replaces its line. `off` never refuses, because it can
  only shrink the file, and is allowed over ANY name, so a place muted yesterday and dropped from the
  config today is still clearable. The read-modify-write race is real and ACCEPTED: this is hand-typed,
  so two racers means an operator typing two commands in the same second, and the loser is one mute they
  can see is missing and retype.
- Privacy: the place name is the operator's own text and never becomes a filename. ONE FILE rather than
  one per place, because nothing in this crate turns typed text into a path unless a predicate already
  vouches for it (`src/main.rs:LIGHTS_QUIET` doc).
- Process ownership and cleanup: `src/main.rs:publish_muted` REMOVES the file when nothing is muted; an
  empty file is never written, which is what keeps the reader's refusal of an empty one honest.
- Compatibility contract: the report reads the same entries the lamps read, entry for entry, because a
  report that decided liveness for itself is how a command and a lamp come to disagree
  (`src/lights.rs:muted_report`,
  `src/lights.rs:the_report_names_every_live_place_and_says_so_when_there_are_none`,
  `tests/dispatch.rs:a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch`).

### 28. A bare mute lasts until the operator's quiet hours end

Given `pns lights quiet "<place>"` with no duration,

When `src/lights.rs:bare_mute_secs` computes the length,

Then it is the minutes from now to `[plugins.hue] quiet_hours`' END minute, times 60.

- Success: a mute that ends when the operator's night does.
- Failure sources: no quiet hours configured, or a window nobody can parse. Both refuse:
  `` pns: lights quiet: a bare mute lasts until your quiet hours end, and `[plugins.hue] quiet_hours` states none; give a duration instead, or set that key ``
  (`src/lights.rs:NO_SCHEDULE`). No clock also refuses.
- Fail direction: refusal, never a guessed duration: picking a length would be a mute the operator did
  not ask for, ending at an hour they cannot predict.
- Thresholds: NOW AT THE END MINUTE IS A WHOLE DAY, not nothing. `(end + 1440 - now) % 1440`, and a
  result of 0 becomes 1440. A mute of zero seconds is not a mute, and the operator asked for one
  (`src/lights.rs:how_long_a_bare_mute_runs_is_the_minutes_from_now_to_the_windows_end`,
  `src/lights.rs:a_bare_mute_lasts_until_the_operators_quiet_hours_end`).
- Required side effects: none beyond the ordinary mute write.
- Forbidden side effects: it must NOT read a room's own dim window. A mute typed at bedtime is about the
  operator's night; a room's window is a rendering rule with nothing to say about how long a by-hand
  silence should last.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: not applicable.
- Privacy: not applicable.
- Process ownership and cleanup: not applicable.
- Compatibility contract: there is deliberately no UNTIMED form, for `pns quiet`'s reason: a mute the
  operator forgets is a lamp that has silently stopped working (`src/lights.rs:QuietCommand` doc).

### 29. An unreadable mute record mutes everything, and says so once

Given a `lights-quiet` file that will not read or will not parse,

When `src/main.rs:ad_hoc_quiet` runs on the event path or the tick,

Then it answers `src/channels/hue.rs:Muting::Everything` and one complaint.

- Success: every lamp quiet until the file is repaired, and the operator told.
- Failure sources: a file that is unreadable, not valid text, a directory standing where the file should
  be, a line that is not `<epoch> <place>`, padding around the place, an empty place, or more than 32
  lines.
- Fail direction: DARK. Read as an empty list, an unreadable record was a house with every lamp loud,
  which is exactly the 3am the mute was armed to prevent, on the one night the machine could not tell
  anybody why. A missing file is the ORDINARY case and says nothing.
- Thresholds: no clock also yields `Everything`, with the line
  `"pns lights: the clock cannot be read, so no mute can be judged live; every lamp is quiet until it can"`
  (`src/lights.rs:NO_CLOCK_FOR_THE_MUTE`), which is the SAME sentence `src/lights.rs:muted_report`
  prints, so an operator reading either sees one wording.
- Required side effects: `src/main.rs:say_lights_once` remembers the joined line, so the complaint is
  said once and again only when it CHANGES. The event path and the tick keep SEPARATE memories
  (`LIGHTS_QUIET_SAID` and `LIGHTS_SAID`), because sharing one would have each forgetting the other's
  line and repeating it.
- Forbidden side effects: `src/lights.rs:muted_entries` must not `trim()` a line. Padding is not
  something this ever wrote, so a padded file was edited by something else, and the leniency once read
  `" 9223372036854775807\n"` as a live mute.
- Timeout and cancellation: one file read.
- Idempotency and duplicates: `src/lights.rs:say` has three answers: `Nothing` (unchanged), `Aloud`
  (print and remember), `Forget` (the complaint cleared: print nothing and DELETE the memory, so the same
  complaint returning is news again). The `Forget` arm is why `say_lights_once` sits OUTSIDE the tick's
  activity gate (`src/main.rs:a_complaint_that_cleared_is_forgotten_so_its_return_is_news_again`,
  `src/lights.rs:a_tick_says_a_complaint_once_and_says_it_again_only_when_it_changes`).
- Privacy: the complaint quotes the offending line back, which is the operator's own place name.
- Process ownership and cleanup: the memory file is removed on `Forget`; the state repairs itself on the
  next `pns lights quiet` write, which republishes the whole file.
- Compatibility contract: the two readers of one complaint take OPPOSITE directions on purpose. The lamp
  path mutes everything; the typed command prints it and rebuilds from an empty list, because an operator
  standing in front of it is losing what the file held and gets to see that rather than a silent repair
  (`src/lights.rs:muted_entries` doc,
  `tests/dispatch.rs:a_corrupt_lights_quiet_is_complained_about_once_rather_than_on_every_event`,
  `src/main.rs:an_unreadable_lights_quiet_complains_and_an_absent_one_says_nothing`,
  `src/main.rs:a_mute_reading_nobody_could_take_leaves_every_lamp_quiet_rather_than_loud`).

### 30. A mute reaches a lamp by every name it answers to

Given a lamp with a name, a room and any number of zones,

When `src/channels/hue.rs:muted_now` is asked,

Then any muted place matching the lamp's own name, its room, or any of its zones covers it.

- Success: `pns lights quiet "3F - Studio"` reaches every lamp in the studio and
  `pns lights quiet "3F - Studio - HCL3"` reaches one.
- Failure sources: none. `Muting::Everything` covers every lamp unconditionally.
- Fail direction: not the delivery path. The map is still resolved and the muted lamps are simply not
  written to, which costs three reads for the length of the mute and keeps ONE answer to "is this lamp
  muted"; a second, config-only copy of the question upstream is how a report and a lamp come to
  disagree.
- Thresholds: none.
- Required side effects: none.
- Forbidden side effects: no second mute predicate anywhere.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pure.
- Privacy: name comparison only.
- Process ownership and cleanup: not applicable.
- Compatibility contract: the vocabulary `pns lights quiet` accepts is BOTH the declarations and the
  bridge's own lamps, rooms and zones. Off the config alone it accepted a misspelled declaration (a mute
  that can never match a lamp) and refused a real inherited lamp the operator was reading off the
  bridge's own app (`src/channels/hue.rs:mutable_names`,
  `src/channels/hue.rs:a_mute_reaches_a_lamp_by_its_own_name_by_its_room_and_by_any_zone_holding_it`,
  `src/channels/hue.rs:the_names_a_mute_takes_are_the_declarations_and_the_bridges_own_three_levels`).

## Clearing

### 31. The operator's return puts out every held lamp, with no daemon involved

Given an event whose surface is not `Away` (`missed_notifications::is_present`) on a machine where both lamp switches are live,

When `src/main.rs:clear_held_lamps` runs,

Then it reads `lights-held`, writes `{"on":{"on":false}}` to each recorded path, and forgets the file.

- Success: the lamps go out and the next return costs no write at all
  (`tests/dispatch.rs:the_operators_return_puts_out_a_glow_without_any_daemon_running`).
- Failure sources: an unreadable record (returns, KEEPING the file, because the clear works off names
  alone and forgetting the file would take the tick's only chance of repairing it); no bridge named
  (returns, keeping the record).
- Fail direction: an unreachable bridge means the writes are refused invisibly and the record is
  FORGOTTEN anyway. The cost is stated rather than coded around: the lamp stays lit with nothing recorded
  to put it out. The alternative, keeping the record until somebody proved the write landed, would have
  every later event re-clearing an already-dark lamp forever on a machine whose daemon is down.
- Thresholds: none.
- Required side effects: THE FILE IS THE FENCE. An ordinary event reads only whether it exists, so every
  event that is not a return costs one failed open and no network at all
  (`tests/dispatch.rs:an_event_holding_no_glow_reaches_the_bridge_for_nothing`).
- Forbidden side effects: no listing is resolved. The paths were recorded when they were written, so a
  clear cannot be defeated by a bridge that has stopped answering its listings
  (`src/channels/hue.rs:clear_held`).
- Timeout and cancellation: `BRIDGE_DEADLINE` per PUT.
- Idempotency and duplicates: a second return finds no file and does nothing.
- Privacy: fixture paths only.
- Process ownership and cleanup: `is_present` is the SAME predicate that advances the return edge the
  `unread` state is derived from, so the lamp and the marker cannot disagree about whether the operator
  came back.
- Compatibility contract: STATED LIMIT, a tick can republish a state the return just cleared. The tick
  reads its condition before it reaches the bridge, so a present event that advances the edge and clears
  the held paths while an older tick is still resolving loses the race, and that tick writes and records
  the state again. Nothing arbitrates, because there is no lock between two deliberately independent
  processes. The exposure is one refresh interval, unbounded only for a tick that was its lease's LAST
  run, where the state waits for the operator's return, which is the event that clears it
  (`src/main.rs:remember_held` doc).

### 32. A held record that would not publish stops the arm

Given a tick that computed which lamps should breathe,

When `src/main.rs:remember_held` fails,

Then NOTHING is armed and the complaint is returned.

- Success: not applicable; this is the failure path.
- Failure sources: an unwritable state directory, a directory standing where the file goes.
- Fail direction: not the delivery path.
- Thresholds: none.
- Required side effects: the complaint
  `"pns lights: the held record could not be written ({error}); no lamp was armed, because nothing would have been able to put one out"`.
- Forbidden side effects: arming a lamp the record does not name is a lamp NOTHING in the system can ever
  put out: not the next tick (which computes its clear by name off this file), not the return from an
  absence, and not the operator's own mute. Nothing armed is one interval of a dark lamp, which the next
  tick fixes by itself
  (`src/main.rs:a_held_record_that_will_not_publish_stops_the_arm_rather_than_lighting_a_lamp`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: `remember_held` REMOVES the file when the held list is empty; an empty file
  is never written.
- Privacy: fixture paths only.
- Process ownership and cleanup: the tick lock is still released by `HeldLock`'s `Drop` on this exit.
- Compatibility contract: a separate complaint covers a record that could not be READ:
  `src/main.rs:HELD_RECORD_UNREADABLE` =
  `"pns lights: the held record could not be read, so no lamp can be put out by name"`. The tick GOES ON
  in that case, because it is the file's only writer and the record it publishes is what repairs the
  file. The residue is stated: a lamp held under a name this run could not read stays lit until the
  repaired record names it again or the operator's next return clears it.

### 33. The lamps narrow to the room the operator is physically in

Given a resolved `Routing` and an armed `[plugins.presence]` table,

When `src/main.rs:narrow_to_presence` runs it through `src/presence_policy.rs:narrow` on the event
path's pulse (`src/main.rs:run_pulse_writes`) and on the tick (`src/main.rs:run_tick_writes`),

Then `src/presence_room.rs:chosen` weighs the desk's idle clock against the bridge's motion edge, and
`src/presence_policy.rs:narrow` takes the room it answers to the lamp map. A WARM DESK IS A CLAIM ON THE
OPERATOR'S OWN BODY, and the arbitration falls out of that: inside `desk_stale_after_secs` the keyboard
says they are at the desk, while motion says A BODY moved in a room and never whose. So while the desk
still speaks,

- motion in the desk's own room AGREES with it, and that room is kept;
- motion NO NEWER than the desk loses to it, the tie included, where a hand is what made the reading;
- NEWER motion in ANOTHER room is AMBIGUOUS and narrows nothing. The bridge reports a room that is still
  occupied as age zero rather than as the age of the edge that began it, so "newer" here is routinely
  three seconds after a keystroke and means only that somebody is in that other room. Read as the
  operator having walked out, it handed every lamp to whoever else was moving in the house.

Past the bound, with the screen locked, or with no readable idle clock, the desk has no claim at all and
motion answers alone. With no `desk_room` named, the desk can name no room, so usable motion answers
alone as well and `NoDeskRoom` is recorded only when nothing else could answer either. A lamp belongs to
a room by the bridge's own membership (`src/channels/hue.rs:Lamp.room`), which `resolve` already joined
off the room listing.

- Success: `Routing.lamps` holds only the lamps that room holds; `unresolved` and `refusals` are
  untouched, because a name the bridge could not answer is a typo whether or not the operator is
  standing in that room.
- Failure sources: every one of them narrows NOTHING and says which (`src/presence_room.rs:Full`):
  `Nowhere` (a fresh poll that found motion in no watched room, and the room they are in may have no
  sensor), each `src/presence.rs:Unreadable` variant, a router answering `home::HomePresence::NotHome`,
  `Ambiguous` (a desk still inside its bound in one room and newer motion in another), a desk that would
  have won with no `desk_room` named and no usable motion to answer instead, and a room that holds no
  lamp this event would light.
- Fail direction: PRESENCE ONLY EVER NARROWS. Not knowing costs the narrowing and nothing else, and a
  narrowing that would leave ZERO targets falls back to the whole routing rather than going silent
  (`src/presence_policy.rs:a_room_holding_no_routed_lamp_falls_back_to_the_whole_routing`).
- Thresholds: the motion reading's freshness is `src/presence.rs:classify`'s, against
  `[plugins.presence] stale_after_secs`; the desk's is `[plugins.presence] desk_stale_after_secs`
  (default 120, `src/config.rs:DEFAULT_DESK_STALE_AFTER_SECS`, bounded 1 to
  `src/config.rs:MAX_DESK_STALE_AFTER_SECS` so a mistyped digit cannot park the lamps in `desk_room` for
  good), past which a keyboard nobody has touched speaks for nothing. No dwell rule and no hysteresis of
  its own.
- Required side effects: one JSON object per decision appended to the `presence-decisions` ring
  (`src/presence_journal.rs:entry`), carrying the reading, the desk clock, the router verdict and the
  room chosen or the reason none was. `pns doctor`'s presence leg reads the last one back
  (`src/main.rs:last_narrowing`).
- Forbidden side effects: reading `src/surface.rs:Surface`. It answers where the operator's EYES are and
  is what picks a notifier; read as location it is `Desk` for two minutes after the last keystroke,
  which ignores fresh motion in the kitchen for that whole window, and `Away` whenever neither the
  keyboard nor the phone has been touched lately, which reads a phone in a pocket at home as an empty
  house. Also forbidden: taking any reading twice. One `SystemProbes` supplies the clock, the idle
  counter, the screen lock and the presence line, all memoized, so the reading and the decision cannot
  straddle a boundary. The presence line is taken EAGERLY, where the set is pointed at the file
  (`src/system.rs:with_presence_path`), which is before that set can hold a clock: read after one, a
  line the daemon republished in the meantime carried an epoch newer than the frozen clock and
  classified as `Future`.
- Timeout and cancellation: none; `narrow` is a total function of its arguments and touches no bridge.
  The router's own verdict is NOT dialled here (`src/main.rs:home_presence` answers `Unknown`), because
  two `home::ROUTER_DEADLINE` calls behind a lamp would outlive a tick's whole interval.
- Idempotency and duplicates: pure, so the same snapshot always gives the same routing.
- Privacy: the room is the bridge's own text, escaped by `serde_json` on the way into the ring so a name
  carrying a newline cannot forge a second entry, and filtered by `src/doctor.rs:shown_room` on the way
  out. The router verdict is recorded as one word, never with the matched key's value.
- Process ownership and cleanup: the ring prunes itself to `decision_log::KEPT`; the write is fail-quiet,
  in `src/main.rs:record_decision`'s style.
- Compatibility contract: a machine with no `[plugins.presence]` table passes `None` and reaches none of
  this, so its lamp map behaves exactly as it did before the feature existed.

### 34. Presence narrows over the lamps THIS event would light, never over all of them

Given a room whose only lamp is routed for some other behaviour, and a fresh reading naming that room,

When either lamp path narrows,

Then the routing is filtered to the lamps this event would actually write to BEFORE `narrow` is called,
so the room holds nothing and the fallback in behaviour 33 leaves the whole routing standing.

- Success: the lamps that were going to light still light, in whatever room the operator is not.
- Failure sources: none of its own; it is a reordering of two filters that already existed.
- Fail direction: full routing, which is the same fail-open the rest of this feature takes.
- Thresholds: none.
- Required side effects: the eligibility question is asked ONCE and used twice, as the set presence
  narrows over and as the write itself (`src/main.rs:run_pulse_writes`'s `write_for` and
  `src/main.rs:run_tick_writes`'s `breath_for`). "Eligible" has to mean "would light", so it answers the
  mute, the routing, the held record and the dim window together: a lamp the dim window darkens writes
  nothing either.
- Forbidden side effects: narrowing first and filtering second. That order kept a kitchen lamp routed
  for `blocked` alone through a `done` event, then dropped it at the per-lamp gate, and wrote nothing at
  all (`src/main.rs:a_pulse_narrows_over_the_lamps_this_behaviour_would_reach_and_not_the_rest`,
  `src/main.rs:a_tick_narrows_over_the_lamps_this_state_would_reach_and_not_the_rest`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: the predicate is pure, so asking it twice per lamp costs microseconds and
  cannot disagree with itself.
- Privacy: no new text.
- Process ownership and cleanup: no state written.

______________________________________________________________________

## Gaps

- `NOT ESTABLISHED:` what a `FADE_LEAD_MS` of zero looks like on a real lamp. The 50 millisecond figure
  was set and looked at; the doc on `src/lights.rs:FADE_LEAD_MS` says plainly that nothing measured the
  alternative.
- `NOT ESTABLISHED:` the freshness of the clock read in `src/main.rs:fire_pulse_unless_quiet`. The
  function reads `now` fresh so a run that crossed into a dim window mid-run does not flash, and the
  comment states the honest limit itself: "no suite pins the freshness, because a test's clock does not
  advance mid-run".
- `NOT ESTABLISHED:` what the bridge is actually WRITTEN on the integration path. The transport is HTTPS
  with verification disabled and the test spy is a plain listener that hangs up, so a binary test can
  show THAT the bridge was reached and never WHAT was sent. Every body, colour and path assertion is a
  unit test through the `src/channels/hue.rs:Bridge` trait
  (`tests/dispatch.rs:a_blocked_turn_lights_the_lamps_once_the_map_exists`).
- `NOT ESTABLISHED:` any test of the real `UreqBridge` transport (certificate handling, redirect refusal,
  the global timeout). `src/channels/hue.rs:UreqBridge` is exercised only through the trait in tests.
- `NOT ESTABLISHED:` the behaviour of `src/main.rs:lights_tick_stale_secs` against a live holder in an
  end-to-end run. `src/main.rs:a_second_tick_stands_down_while_a_first_still_holds_the_lamps` covers the
  stand-down through the function seam; the age-based steal is covered only by `claim_lock`'s own general
  contract.
- `NOT ESTABLISHED:` whether the harness's continuation prompt reaches the `UserPromptSubmit` hook. The
  code says explicitly that no capture in this repository settles it, which is why every event except a
  `LAMP_BLOCKED` one ends a wait (`src/main.rs:arm_quota_stale_wait` doc).
- `NOT ESTABLISHED:` a subagent's own `resolved` batch. It is skipped by design, so a subagent wait holds
  blue until the parent's own Stop (`src/main.rs:update_blocked_marker` doc).
