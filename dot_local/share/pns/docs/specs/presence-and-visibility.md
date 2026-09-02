# Presence and visibility arbitration

## Scope

This document specifies how `pns` decides where the operator's attention is and what that means for one
event's delivery. It covers the `surface` model (Desk, Mobile, Away), the `visibility` model (Visible,
Hidden, Unknown), the readings behind each of them (the desk idle clock, the console lock, the phone's
pty clock, the Back Tap marker, the herdr session view, the wall clock, and the macOS Focus store), the
fail direction of every unreadable reading, the exact freshness threshold and its override, how the
readings are memoized so one submission decides on one coherent snapshot, and how the slow
subprocess-backed readings are started ahead and bounded. It does not cover channel delivery mechanics
(`src/routing.rs`, `src/channels/`), the `quiet window` and `quiet hours` rules (`src/quiet.rs`,
`src/lights.rs`), the `unread` lamp, the `decision ring` and `journal` file formats beyond the fields
this area writes into them, or the `home probe` and `router` (`src/home.rs`). Everything here is derived
from the crate's own source and tests; gaps are marked `NOT ESTABLISHED:`.

## Glossary

| Term                 | Defining symbol                                                                                                                                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `surface`            | `src/surface.rs:Surface` (`Desk`, `Mobile`, `Away`): "Where the operator's eyes are. Picks the notifier: Desk = banner, Mobile = phone card, Away = phone card. A banner NEVER fires while the surface is Mobile." |
| `visibility`         | `src/surface.rs:Visibility` (`Visible`, `Hidden`, `Unknown`): "Whether the origin pane can be seen on whatever client shows the session. Unknown never suppresses."                                                |
| `presence`           | `src/presence.rs` module doc: "Where the operator is: the raw readings turned into the units the arbitration compares."                                                                                            |
| session view         | `src/surface.rs:SessionView`, built by `src/system.rs:SystemProbes::session_view`                                                                                                                                  |
| delivery plan        | `src/surface.rs:DeliveryPlan` / `src/surface.rs:plan`                                                                                                                                                              |
| effective visibility | `src/surface.rs:effective_visibility`                                                                                                                                                                              |
| freshness window     | `src/engine.rs:DEFAULT_DESK_IDLE_SECS`, applied by `src/surface.rs:is_fresh`                                                                                                                                       |
| probe                | `src/probes.rs` (five narrow traits), implemented in `src/system.rs:SystemProbes`                                                                                                                                  |
| `decision ring`      | `src/decision_log.rs:line`, written by `src/main.rs:record_decision`                                                                                                                                               |
| `journal`            | `src/missed_notifications.rs:entry`                                                                                                                                                                                |

## Probe table

Every mechanism below is spawned through `src/system.rs:CommandRunner::run`, implemented for production
by `src/system.rs:SystemCommandRunner`, which calls `src/system.rs:run_bounded` with `PROBE_DEADLINE` (5
seconds, `src/system.rs:PROBE_DEADLINE`) and `PROBE_READ_MAX` (1 MiB inclusive,
`src/system.rs:PROBE_READ_MAX`). Commands are built with `Command::new(program).args(args)`, so no
argument passes through a shell.

| Reading                        | Mechanism (exact)                                                                                                                                                                                                                                                                  | Deadline             | Unreadable means                                                                                                                     | Fail direction pinned by                                                                                                                                                                                                                                                                                                                      |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Desk idle age                  | `/usr/sbin/ioreg -c IOHIDSystem`; first line containing `HIDIdleTime`, last whitespace field, divided by 1_000_000_000 (`src/system.rs:parse_idle_nanoseconds`, `src/presence.rs:idle_secs_from_ns`)                                                                               | 5 s                  | `None`, never 0 seconds idle                                                                                                         | `src/presence.rs:an_empty_reading_is_unknown_rather_than_zero_seconds_idle`, `src/presence.rs:a_garbled_reading_is_unknown`, `src/system.rs:contaminated_idle_output_reads_as_unknown_rather_than_a_reading`, `src/system.rs:an_idle_command_that_fails_reports_unknown_which_fails_open_into_a_push`                                         |
| Console lock                   | `/usr/sbin/ioreg -n Root -d1`; line containing `"IOConsoleLocked"` (matched with its quotes), last field `Yes` or `No` (`src/system.rs:parse_screen_locked`)                                                                                                                       | 5 s                  | `None`, and only `Some(true)` disqualifies the desk clock                                                                            | `src/surface.rs:an_unlocked_or_unreadable_console_leaves_every_verdict_exactly_as_it_was`, `src/system.rs:a_console_key_that_is_missing_or_says_something_else_reads_as_no_reading`                                                                                                                                                           |
| Back Tap marker mtime          | `std::fs::symlink_metadata(<marker>).modified()` on `$PNS_PHONE_MARKER_FILE`, defaulting to `$HOME/.local/state/pns/phone-attention.marker` (`src/system.rs` `PhoneMarkerProbe for SystemProbes`, `src/main.rs:system_probes`)                                                     | none (no subprocess) | `None`, which is never fresh, so the tap does not speak for the phone                                                                | `src/system.rs:an_absent_marker_reports_unknown_which_the_marker_rule_fails_closed_on`, `src/system.rs:the_marker_probe_reads_the_link_itself_never_its_target`                                                                                                                                                                               |
| Phone input atime              | `/usr/bin/pgrep -x mosh-server`, then `/usr/bin/pgrep -P <server ids joined by comma>`, then `/bin/ps -o tty= -p <client ids joined by comma>`, then the newest `std::fs::metadata(/dev/<name>).accessed()` (`src/system.rs:phone_reading`, `src/system.rs:newest_terminal_atime`) | 5 s per command      | `None` at any step, which is never fresh                                                                                             | `src/system.rs:a_failure_at_any_step_of_the_chain_reads_as_no_phone_rather_than_a_fresh_one`, `src/system.rs:no_mosh_server_at_all_never_asks_for_children_of_nothing`, `src/surface.rs:a_phone_reading_that_could_not_be_taken_never_counts_as_fresh`                                                                                        |
| Session view                   | `herdr workspace list` (resolved through PATH), then `herdr pane layout --pane <origin pane>` (`src/system.rs` `SessionViewProbe for SystemProbes`, `src/system.rs:parse_focused_tab`, `src/system.rs:parse_layout`)                                                               | 5 s per command      | `None`, which becomes `Visibility::Unknown` and never suppresses                                                                     | `src/surface.rs:a_session_view_that_cannot_be_read_is_unknown_never_visible`, `src/system.rs:any_herdr_call_failing_leaves_the_view_unreadable_rather_than_guessing`, `src/system.rs:a_session_with_no_focused_workspace_is_unreadable_rather_than_a_guess`, `tests/dispatch.rs:an_unreadable_view_delivers_rather_than_suppressing_on_doubt` |
| Wall clock                     | `SystemTime::now().duration_since(UNIX_EPOCH).as_secs()` (`src/system.rs:SystemProbes::now_secs`)                                                                                                                                                                                  | none                 | `None`, which ages nothing, so the phone atime and the marker mtime both drop out of the arbitration                                 | `src/engine.rs:an_unreadable_clock_ages_no_phone_signal_rather_than_treating_it_as_fresh`, `src/engine.rs:an_unreadable_clock_ages_no_marker_rather_than_treating_it_as_fresh`                                                                                                                                                                |
| macOS Focus assertions         | `$HOME/Library/DoNotDisturb/DB/Assertions.json`, read through `src/main.rs:readable_ring` at `RING_READ_MAX` (256 KiB), parsed by `src/focus.rs:active_modes` (`src/main.rs:FOCUS_DB`, `src/main.rs:focus_now`)                                                                    | none                 | `Err`, which the event path reads as not silenced (fail open)                                                                        | `tests/dispatch.rs:a_focus_store_that_cannot_be_read_costs_no_notification_at_all`, `src/focus.rs:nothing_readable_names_no_mode_one_row_per_failure_shape`                                                                                                                                                                                   |
| macOS Focus mode catalog       | `$HOME/Library/DoNotDisturb/DB/ModeConfigurations.json`, same ceiling, parsed by `src/focus.rs:mode_names`                                                                                                                                                                         | none                 | an empty map, so display-name entries in `[focus] silence` go inert and raw `modeIdentifier` entries still match                     | `src/focus.rs:a_catalog_nothing_can_read_resolves_no_names_at_all`, `src/focus.rs:a_raw_mode_identifier_is_accepted_for_a_mode_the_catalog_does_not_name`                                                                                                                                                                                     |
| Operator mute (`quiet window`) | `src/main.rs:read_quiet_expiry` over the quiet-until state file, judged by `src/quiet.rs:is_muted` against this run's clock (`src/main.rs:muted_now`)                                                                                                                              | none                 | not muted, with `pns: state error (quiet-until could not be read: {error}); nothing is muted, clear it with pns quiet off` on stderr | Specified in the quiet-window document; recorded here only because it is an `Overrides` field the arbitration reads.                                                                                                                                                                                                                          |

The marker and the wall clock are deliberately absent from `src/probes.rs:Wants` "because neither spawns
a subprocess", and the session view is absent "because it has exactly one production reader already, with
nothing to overlap it against" (`src/probes.rs:Wants`).

## Decision table

`src/surface.rs:plan` maps `surface`, effective `visibility`, the `long_running` tier and the
`mobile_watch_card` config toggle onto a `DeliveryPlan`. Every row below is a case in
`src/surface.rs:every_delivery_row_in_the_confirmed_matrix_plans_correctly`.

| Surface | Visibility | long_running | mobile_watch_card | banner | phone_card | pulse |
| ------- | ---------- | ------------ | ----------------- | ------ | ---------- | ----- |
| Desk    | Visible    | no           | any               | no     | no         | no    |
| Desk    | Visible    | yes          | any               | no     | no         | yes   |
| Desk    | Hidden     | no           | any               | yes    | no         | no    |
| Desk    | Hidden     | yes          | any               | yes    | no         | yes   |
| Desk    | Unknown    | no           | any               | yes    | no         | no    |
| Mobile  | Visible    | no           | any               | no     | no         | no    |
| Mobile  | Visible    | yes          | off               | no     | no         | yes   |
| Mobile  | Visible    | yes          | on                | no     | yes        | yes   |
| Mobile  | Hidden     | no           | any               | no     | yes        | no    |
| Mobile  | Unknown    | no           | any               | no     | yes        | no    |
| Away    | Visible    | no           | any               | no     | yes        | no    |
| Away    | Hidden     | yes          | any               | no     | yes        | yes   |

Two properties hold over the whole input space rather than row by row:
`src/surface.rs:no_plan_row_can_ever_banner_on_the_mobile_surface` and
`src/surface.rs:every_long_running_row_pulses_whatever_else_it_decides`.

After `plan` returns, `src/engine.rs:decide` applies three more rules in this order:

| Rule                                                         | Effect                                                | Evidence                                                                                                                                                                                                     |
| ------------------------------------------------------------ | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `PNS_FORCE_PHONE`                                            | forces `phone_card` on whatever the surface said      | `src/engine.rs:force_phone_sends_the_card_from_the_desk_with_the_pane_in_plain_sight`, `tests/dispatch.rs:force_phone_is_caller_intent_and_beats_the_whole_surface_model`                                    |
| `PNS_SKIP_PHONE`                                             | clears `phone_card`, beating force                    | `src/engine.rs:skip_phone_beats_force_phone_because_already_sent_is_more_specific`, `tests/dispatch.rs:relay_skip_phone_beats_relay_force_phone`                                                             |
| `Overrides::silenced()` (operator mute or named macOS Focus) | clears banner, card and pulse together, beating force | `src/engine.rs:the_mute_beats_a_forced_phone_card_because_a_producer_cannot_overrule_the_operator`, `src/engine.rs:a_focus_the_config_named_suppresses_the_mutes_three_decorations_and_beats_a_forced_phone` |

______________________________________________________________________

## Behaviors

### 1. The surface names one of three places and picks the notifier

Given a set of presence readings for one event When `src/surface.rs:surface` is asked where the operator
is Then it answers exactly one of `Surface::Desk`, `Surface::Mobile` or `Surface::Away`, and that answer
alone selects the notifier: Desk means the banner, Mobile and Away both mean the phone card, and a banner
never fires on Mobile.

- Success: `src/surface.rs:Surface` has three variants and no fourth; `src/surface.rs:plan` reads it in
  exactly two places (`banner: surface == Surface::Desk && !watching` and the `phone_card` match).
- Failure sources: a reading that cannot be taken (each covered by its own behavior below).
- Fail direction: toward `Away`, which always cards. `src/surface.rs:surface` states it: "A missing
  reading is never fresh, so every unknown falls toward Away rather than Desk: getting a card while at
  the desk costs a glance, missing one while away costs the event." Pinned by
  `src/surface.rs:every_surface_case_in_the_matrix_arbitrates_correctly` row "no readings at all fails
  toward away, never desk".
- Thresholds: Not applicable at this level; the freshness window is behavior 5.
- Required side effects: none. `src/surface.rs:surface` is a pure function of its five arguments.
- Forbidden side effects: no IO, no clock read, no environment read inside `src/surface.rs`. The module
  holds only value functions.
- Timeout and cancellation: Not applicable; no IO.
- Idempotency and duplicates: total and deterministic, so calling it twice with the same arguments
  answers the same.
- Privacy: the surface is a three-valued enum. It is written to the `decision ring` as
  `surface=Desk|Mobile|Away` (`src/decision_log.rs:line`) and carries nothing about the operator beyond
  that.
- Process ownership and cleanup: Not applicable; nothing is spawned here.
- Compatibility contract: `Surface` is `pub` and read by `src/engine.rs`, `src/main.rs`
  (`forward_to_moshi`) and `src/missed_notifications.rs:is_present`. Adding a variant breaks all three;
  the `Debug` variant name is the `decision ring`'s wire spelling
  (`src/decision_log.rs:a_line_names_the_event_and_every_gate_input_behind_one_epoch_second` asserts
  `surface=Mobile`).

### 2. Visibility names one of three states, and Unknown never suppresses

Given an origin pane and whatever the session reported about it When `src/surface.rs:visibility` judges
it Then it answers `Visible`, `Hidden` or `Unknown`, where Hidden needs proof (a different tab, or a zoom
covering this pane) and anything unreadable is Unknown.

- Success: `src/surface.rs:visibility` returns `Hidden` only for `view.origin_tab != view.focused_tab` or
  `view.zoomed && view.focused_pane != origin`.
- Failure sources: an empty origin pane, an empty `origin_tab`, an empty `focused_tab`.
- Fail direction: `Unknown`, which routes like Hidden at plan time because `plan` computes
  `watching = visibility == Visibility::Visible`. The type comment states why they stay distinct:
  "Unknown never suppresses: a notification wrongly delivered costs a glance, a notification wrongly
  suppressed is the product failing silently." Pinned by
  `src/surface.rs:a_session_view_that_cannot_be_read_is_unknown_never_visible` and
  `src/surface.rs:an_empty_origin_pane_reads_unknown`.
- Thresholds: Not applicable; visibility is structural, with no time in it.
- Required side effects: none.
- Forbidden side effects: none possible; it is a pure function.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic over its two arguments.
- Privacy: it consumes pane and tab identifiers and returns an enum. The `decision ring` records
  `visibility=` and `session_visibility=` as enum names and records the pane only as `pane=present|none`
  (`src/decision_log.rs:line`: "THE PANE AS THE DECISION USED IT and no further").
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the six-row matrix in
  `src/surface.rs:every_visibility_case_in_the_matrix_reads_correctly` is the contract, including
  "unzoomed sibling in the focused tab" reading `Visible`.

### 3. The session view is one session-global reading, never a caller-relative one

Given an event carrying an origin pane id When `src/system.rs` `SessionViewProbe for SystemProbes` builds
a `SessionView` Then it runs `herdr workspace list` for the focused workspace's `active_tab_id` and
`herdr pane layout --pane <origin>` for that pane's tab id, focused pane and zoom flag, and it never asks
`herdr pane current`.

- Success: two calls, both addressed explicitly. `src/system.rs:parse_focused_tab` takes the one
  workspace whose `focused` is `true`; `src/system.rs:parse_layout` reads `tab_id`, `focused_pane_id` and
  `zoomed` out of `/result/layout`.
- Failure sources: no `herdr` on PATH, either call exiting non-zero, no workspace flagged focused, a JSON
  shape the parser does not recognize, a missing field in the layout.
- Fail direction: `None` from either step, which `src/engine.rs:operator_visibility` turns into
  `Visibility::Unknown`. `src/system.rs:parse_layout` states the reason a missing field refuses the whole
  reading: "assuming a tab is unzoomed suppresses a notification the operator cannot see." Pinned by
  `src/system.rs:any_herdr_call_failing_leaves_the_view_unreadable_rather_than_guessing`,
  `src/system.rs:an_answer_this_parser_does_not_recognise_is_unreadable_too` and
  `src/system.rs:a_session_with_no_focused_workspace_is_unreadable_rather_than_a_guess`.
- Thresholds: 5 seconds per herdr call (`src/system.rs:PROBE_DEADLINE`), 1 MiB per answer
  (`src/system.rs:PROBE_READ_MAX`).
- Required side effects: two `herdr` child processes per event that carries a pane.
- Forbidden side effects: `herdr pane current` must never be consulted. The doc on
  `src/surface.rs:SessionView` records the live failure: "Measured live on 2026-08-13 (drill D4): with
  the session zoomed onto wW:p3R, a hook in wW:p3K was answered wW:p3K." Pinned by
  `src/system.rs:the_view_asks_the_session_what_is_focused_and_never_asks_for_the_current_pane` and by
  the test-support stub, which exits non-zero for `pane current` on purpose
  (`tests/support/mod.rs:stub_herdr`).
- Timeout and cancellation: each call is bounded by `run_bounded`; on a blown deadline or an over-cap
  answer the child is killed and reaped and the reading is `None`.
- Idempotency and duplicates: exactly one production reader, so the reading is taken once per event by
  call site alone. The impl states the consequence: "NO CELL, UNLIKE THE OTHER FOUR PROBES ON THIS STRUCT
  ... A second production reader would need the same `OnceCell` the other four carry." Bounded by
  `src/engine.rs:one_decision_reads_each_probe_at_most_once_and_never_twice`, which asserts
  `view_reads == 1`.
- Privacy: pane and tab identifiers only; nothing about pane contents is read.
- Process ownership and cleanup: `run_bounded` owns the children; a child past its deadline gets `kill()`
  then `wait()`, so nothing is left as a zombie.
- Compatibility contract: `herdr` is resolved through PATH, unlike the absolute system binaries, and a
  PATH without it reads as Unknown, which "fails OPEN into a notification"
  (`src/system.rs:SystemProbes::herdr`). The recorded live fixtures (`src/system.rs:WORKSPACE_LIST`,
  `LAYOUT_ZOOMED_ON_SIBLING` and siblings) are what fail if herdr's JSON shape moves.

### 4. The origin pane's tab and zoom decide whether it is on screen

Given a `SessionView` When the origin pane's tab is not the focused tab, or the tab is zoomed onto a
different pane Then visibility is `Hidden`; otherwise it is `Visible`.

- Success: the six-row matrix in `src/surface.rs:every_visibility_case_in_the_matrix_reads_correctly`,
  including that a zoom onto the origin itself leaves it Visible and a zoom onto a sibling hides it.
- Failure sources: none beyond the empty-field cases in behavior 2.
- Fail direction: Unknown (behavior 2).
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: the pane list inside the layout answer must not be consulted;
  `src/system.rs:parse_layout` reads the focused pane and the zoom flag alone, and the doc says so: "The
  pane list is not read: visibility turns on the focused pane and the zoom flag alone."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: identifiers only.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: zoom is tab-level (`src/surface.rs:SessionView::zoomed`: "true means the
  focused pane fills the window and every sibling is hidden (operator-confirmed herdr semantics)").

### 5. A signal speaks for its place only inside the freshness window

Given an age in whole seconds and a window When `src/surface.rs:is_fresh` (through
`src/surface.rs:fresh_age`) judges it Then the age counts only if it is strictly less than the window;
anything else, including `None`, counts for nothing.

- Success: `fresh_age` is `age.filter(|seconds| *seconds < fresh_secs)`.
- Failure sources: a reading that could not be taken (`None`); a threshold that could not be parsed
  (behavior 23).
- Fail direction: not fresh, so the signal drops out of the arbitration rather than holding its surface.
- Thresholds: the window defaults to **120 seconds** (`src/engine.rs:DEFAULT_DESK_IDLE_SECS`) and is
  overridden by `PNS_DESK_IDLE_SECS`. The comparison is strict: an age of **119** is fresh, an age of
  **120** is not, and an age of **121** is not. `src/surface.rs`'s matrix uses 120 throughout, with
  "scrolling moshi now beats desk touched 90s ago" fresh at 90 and "nothing fresh anywhere is away" stale
  at 600. `tests/hooks.rs:the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started` drives
  the same 120 through the real binary.
- Required side effects: none.
- Forbidden side effects: there must be exactly one definition of fresh. `src/surface.rs:is_fresh` is
  exported for that reason: "ONE definition of fresh, exported so there is only ever one: the arbitration
  below and the mobile-visibility rule beside it must not be able to disagree about whether the phone was
  just used."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: an age in seconds; the `decision ring` writes `fresh_window=` and the three ages as counts, or
  `none` (`src/decision_log.rs:count`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `PNS_DESK_IDLE_SECS` is the only knob. There is no config key for the freshness
  window: `NOT ESTABLISHED:` grepped `PNS_DESK_IDLE_SECS` and `desk_idle` across `src/`, `tests/` and the
  repository's TOML and Markdown; the only definitions are `src/engine.rs:Overrides::from_env` and
  `src/engine.rs:DEFAULT_DESK_IDLE_SECS`, and `src/config.rs` has no matching key.

### 6. Newest signal wins between the desk clock and the phone's two signals

Given a desk input age, a phone input age and a Back Tap marker age, all measured against one clock When
`src/surface.rs:surface` arbitrates Then the fresher of the desk clock and the phone (whose age is the
smaller of its own two fresh signals) names the surface, and nothing fresh anywhere is `Away`.

- Success: the eleven-row matrix in
  `src/surface.rs:every_surface_case_in_the_matrix_arbitrates_correctly`, plus
  `src/surface.rs:a_phone_signal_needs_no_expiry_window_while_it_stays_the_newest_one` and
  `src/surface.rs:a_stale_phone_reading_loses_to_the_desk_rather_than_holding_mobile`. End to end:
  `tests/dispatch.rs:a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile` and
  `tests/dispatch.rs:desk_input_after_the_tap_cancels_it`.
- Failure sources: any of the three readings coming back `None`; an unreadable clock, which ages neither
  timestamp-based reading.
- Fail direction: a `None` never competes, so unknowns fall toward `Away`
  (`src/surface.rs:a_phone_reading_that_could_not_be_taken_never_counts_as_fresh`,
  `src/engine.rs:a_phone_probe_that_read_nothing_leaves_the_operator_at_their_desk`).
- Thresholds: the freshness window from behavior 5. There is no separate marker time-to-live: the doc
  records that newest-signal-wins "retired the marker's fixed TTL", and
  `src/surface.rs:a_phone_signal_needs_no_expiry_window_while_it_stays_the_newest_one` shows a
  3600-second marker still yielding Mobile when the pty clock is at 30.
- Required side effects: none.
- Forbidden side effects: the tap must not outrank the pty clock or be outranked by it. "THE TAP AND THE
  PTY ARE ONE CLASS ... the fresher of the two speaks for the phone", pinned by the matrix rows "the tap
  speaks for the phone when it is the fresher of the two" and "and the pty speaks for it when IT is the
  fresher of the two".
- Timeout and cancellation: Not applicable at this level.
- Idempotency and duplicates: pure over five arguments.
- Privacy: three ages and one boolean in, one enum out.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the argument order is
  `(desk_input_age, phone_input_age, marker_age, desk_fresh_secs, screen_locked)`. `surface` is `pub` and
  called from `src/engine.rs:surface_reading` only.

### 7. The tie goes to the desk

Given a desk age and a phone age that are equal and both fresh When `src/surface.rs:surface` compares
them Then the answer is `Desk`.

- Success: the comparison is `if desk <= phone { Surface::Desk } else { Surface::Mobile }`, matrix row
  "the tie goes to the desk, where the operator has to be sitting for the reading to exist at all".
- Failure sources: Not applicable; this is the tie-break itself.
- Fail direction: toward the desk, on the argument that the desk reading exists only if somebody is
  sitting there.
- Thresholds: exact equality. At desk 30 and phone 30 the answer is `Desk`; at desk 31 and phone 30 it is
  `Mobile`; at desk 30 and phone 31 it is `Desk`. The tie is load-bearing enough to have cost a flaky
  test: `tests/hooks.rs:the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started` records
  "ages are whole seconds and a tie goes to the desk, so a desk stated at one second read Desk whenever
  the fresh marker's own age had just rolled over to one, and a hook reading the world at start passed
  this test about one run in twenty (measured 2026-09-01)", which is why that test states the desk at two
  seconds.
- Required side effects: none.
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: ages are whole seconds throughout, so sub-second differences are invisible to
  the tie-break by construction.

### 8. A locked screen disqualifies the desk clock and nothing else

Given a fresh desk input age and a console lock reading of `Some(true)` When `src/surface.rs:surface`
arbitrates Then the desk drops out of the running entirely, and the phone's own signals still answer for
the phone.

- Success: `let desk = fresh(desk_input_age).filter(|_| screen_locked != Some(true));`. Pinned by
  `src/surface.rs:a_locked_screen_takes_the_desk_out_of_the_running_however_fresh_its_clock_is` (desk at
  2 seconds, locked, answer `Away`),
  `src/surface.rs:a_locked_screen_with_a_fresh_pty_clock_is_still_the_phone_and_never_away` and
  `src/surface.rs:a_locked_screen_with_a_fresh_back_tap_is_still_the_phone_and_never_away`. End to end:
  `src/engine.rs:a_locked_screen_cards_the_phone_and_leaves_the_desk_banner_unraised` and
  `src/engine.rs:a_locked_screen_sends_a_blocked_approval_to_the_phone_rather_than_the_lock_screen`.
- Failure sources: the `ioreg -n Root -d1` spawn failing, the `"IOConsoleLocked"` key being absent or
  carrying a word other than `Yes` or `No`.
- Fail direction: only `Some(true)` locks. `Some(false)` and `None` leave every verdict exactly as it
  was, which the doc justifies as costing "one freshness window of the behavior that shipped before this,
  where inventing a lock would kill the desk banner permanently wherever the reading stops working".
  Pinned by `src/surface.rs:an_unlocked_or_unreadable_console_leaves_every_verdict_exactly_as_it_was`,
  which runs three surface cases against both `None` and `Some(false)`.
- Thresholds: none of its own. The lock is deliberately not a freshness question: "locking necessarily
  postdates the last desk input, because typing again means unlocking first, so the lock is the newest
  fact about the desk."
- Required side effects: one extra `ioreg` spawn, and only under the condition in behavior 15.
- Forbidden side effects: a lock must not be read as a blanket `Away`. The doc states the canonical case
  it would get wrong: "locking the laptop and picking it up ... Away always cards while Mobile lets a
  watched pane suppress."
- Timeout and cancellation: 5 seconds, as every probe.
- Idempotency and duplicates: memoized (behavior 20);
  `src/system.rs:the_lock_probe_reads_the_root_dictionary_by_exact_argv_and_only_once`.
- Privacy: a boolean. The `decision ring` writes `locked=yes|no|none`, and `src/decision_log.rs:tri`
  keeps the third state distinct: "an unread lock is not an unlocked one", pinned byte for byte by the
  second assertion in
  `src/decision_log.rs:a_line_names_the_event_and_every_gate_input_behind_one_epoch_second`.
- Process ownership and cleanup: the `ioreg` child is owned and reaped by `run_bounded`.
- Compatibility contract: the key is matched with its quotes (`src/system.rs:IOREG_LOCK_KEY`), which is
  what keeps the neighbouring `"IOConsoleUsers"` array and its per-session `CGSSessionScreenIsLocked` out
  of the reading.

### 9. A Mobile surface the Back Tap alone reached is watching nothing

Given a `Mobile` surface whose phone pty clock is not fresh (the tap is what put the operator there) When
`src/surface.rs:effective_visibility` adjusts what the session reported Then the delivery decision runs
on `Visibility::Hidden` whatever any client's display shows.

- Success: `if surface == Surface::Mobile && !phone_input_fresh { return Visibility::Hidden; }`. Pinned
  by `src/surface.rs:every_effective_visibility_case_adjusts_or_passes_through_correctly` and composed in
  `src/surface.rs:a_back_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_on_screen`. End to end:
  `tests/dispatch.rs:a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight` and
  `src/engine.rs:a_tap_with_moshi_closed_cards_even_when_the_session_view_cannot_be_read`.
- Failure sources: Not applicable; the rule consumes decisions already taken.
- Fail direction: toward delivering, since Hidden is what cards the phone on Mobile.
- Thresholds: the same freshness window as behavior 5, applied to the phone input age alone. The
  freshness flag is computed once, in `src/engine.rs:surface_reading`, as
  `crate::surface::is_fresh(phone_input_age, desk_fresh_secs)` and carried on
  `src/engine.rs:SurfaceReading::phone_input_fresh`, "because deriving the phone's freshness a second
  time somewhere else is how the arbitration and the visibility rule beside it would come to disagree".
- Required side effects: none.
- Forbidden side effects: the rule must not reach any other surface or a Mobile surface with a fresh pty
  clock. `src/surface.rs:the_rule_rewrites_nothing_but_a_mobile_surface_the_phone_never_earned` measures
  the whole eighteen-combination input space and asserts that only the three Mobile rows with a cold pty
  clock may differ from what the session reported.
  `src/surface.rs:moshi_open_on_the_origin_pane_still_suppresses_the_card` is the guard for the case that
  already passed drill D5.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure over three arguments.
- Privacy: enums and a boolean.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `src/engine.rs:GateInputs` carries both `session_visibility` (what the session
  reported) and `visibility` (what the plan ran on), so the rewrite is auditable from the `decision ring`
  rather than inferred
  (`src/engine.rs:a_decision_reports_both_the_sessions_visibility_and_the_one_the_plan_ran_on`).

### 10. The plan is the surface, the visibility, the tier and one toggle

Given a surface, an effective visibility, a `long_running` tier and the `mobile_watch_card` config toggle
When `src/surface.rs:plan` decides Then the banner belongs to the desk with the pane out of sight, the
card belongs to the phone (always when away, and on mobile unless the operator is watching the pane), and
the pulse rides on top of every long-running event.

- Success: the twelve-row matrix in
  `src/surface.rs:every_delivery_row_in_the_confirmed_matrix_plans_correctly`, reproduced in the decision
  table above; through the process in
  `tests/dispatch.rs:away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner`,
  `tests/dispatch.rs:at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery`,
  `tests/dispatch.rs:at_the_desk_watching_the_pane_only_the_log_fires`,
  `tests/dispatch.rs:a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log` and
  `tests/dispatch.rs:a_phone_in_hand_showing_another_tab_still_cards`.
- Failure sources: none of its own; every input is already a decided value.
- Fail direction: `Unknown` visibility routes as not-watching, so it delivers
  (`tests/dispatch.rs:an_unreadable_view_delivers_rather_than_suppressing_on_doubt`).
- Thresholds: `long_running` is a caller-stated tier, not a threshold this function computes. It arrives
  either from the `--long-running` flag (`src/args.rs`) or, on the hook path, from
  `pns::pulse::session_was_long(elapsed, Some(pulse_threshold_secs()))`, whose default is **300 seconds**
  inclusive (`src/pulse.rs:DEFAULT_LONG_SESSION_SECS`, overridable with `PNS_PULSE_THRESHOLD_SECS` at
  `src/main.rs:pulse_threshold_secs`): 300 is long, 299 is not (`src/pulse.rs` asserts both).
  `mobile_watch_card` defaults to false (`src/main.rs:watch_card`).
- Required side effects: none. `plan` returns a value; `src/routing.rs:channel_plan` turns it into legs.
- Forbidden side effects: no banner on Mobile, ever
  (`src/surface.rs:no_plan_row_can_ever_banner_on_the_mobile_surface`); no long-running event without a
  pulse (`src/surface.rs:every_long_running_row_pulses_whatever_else_it_decides`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: three booleans out. The `decision ring` writes
  `plan=banner:{yes|no},card:{yes|no},pulse:{yes|no}` (`src/decision_log.rs:line`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: a value of the wrong type under `[plugins.mobile] mobile_watch_card` is refused
  out loud rather than read as false, with
  `pns: config error ([plugins.mobile] mobile_watch_card is {type}, not a boolean); the mobile watching card stays off`
  (`src/main.rs:watch_card`,
  `tests/dispatch.rs:a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud`).

### 11. Caller intent survives the arbitration, and skip beats force

Given `PNS_SKIP_PHONE` or `PNS_FORCE_PHONE` in the environment When `src/engine.rs:decide` has a plan
Then the card is `!skip_phone && (force_phone || planned)`, so force overrides the surface entirely and
skip overrides force.

- Success: `src/engine.rs:skip_phone_beats_force_phone_because_already_sent_is_more_specific`,
  `src/engine.rs:force_phone_sends_the_card_from_the_desk_with_the_pane_in_plain_sight`, and through the
  process `tests/dispatch.rs:relay_skip_phone_drops_the_phone_and_only_the_phone`,
  `tests/dispatch.rs:relay_skip_phone_beats_relay_force_phone`,
  `tests/dispatch.rs:relay_force_phone_overrides_presence`,
  `tests/dispatch.rs:force_phone_is_caller_intent_and_beats_the_whole_surface_model`.
- Failure sources: none; both are presence checks (`set(key)` in `src/engine.rs:Overrides::from_env` is
  `is_some_and(|raw| !raw.is_empty())`), so there is no parse to fail.
- Fail direction: an unset or empty variable is off.
- Thresholds: Not applicable.
- Required side effects: on the blocking path, a forward that really spawned sets `PNS_SKIP_PHONE=1` in
  the process environment so the card moshi is about to raise is not sent twice
  (`src/main.rs:blocking_event`). The suppression is applied to the spawn, not to the intent: "an away
  operator whose moshi-hook could not spawn lost the one notification still able to reach them", pinned
  by `tests/hooks.rs:moshi_not_being_installed_leaves_the_hook_a_silent_exit_zero`.
- Forbidden side effects: the narrowing flags `--local-only` and `--remote-only` are not overridden by a
  fresh tap (`tests/dispatch.rs:a_narrowing_flag_still_beats_a_fresh_tap`,
  `tests/dispatch.rs:skip_phone_still_beats_a_fresh_tap`). Giving both prints
  `pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent`
  (`src/main.rs`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the environment is read once per invocation
  (`src/main.rs:overrides_from_env`).
- Privacy: booleans; the `decision ring` writes `skip_phone=` and `force_phone=`.
- Process ownership and cleanup: the `set_var` on the blocking path is `unsafe` and is documented as safe
  only because every probe thread started for that event has already been joined by the time
  `surface_reading` returns (`src/system.rs` `ProbeStart::start` doc: "EVERY THREAD STARTED HERE IS
  JOINED BY A READ ON THE SAME PATH before anything calls `std::env::set_var`").
- Compatibility contract: the two variables are the relay's own spelling
  (`src/engine.rs:skip_and_force_parse_from_their_relay_variables`).

### 12. The two mutes are applied last and beat a forced card

Given the operator's own `quiet window` mute or a macOS Focus the config named When
`src/engine.rs:decide` finishes arbitrating Then banner, card and pulse are all cleared, and that
clearing beats `PNS_FORCE_PHONE`.

- Success: `Overrides::silenced()` is `self.muted || self.focus_active`, and the muted branch is a full
  struct literal with all three fields set false. Pinned by
  `src/engine.rs:a_muted_decision_keeps_the_durable_log_and_drops_every_decorative_leg`,
  `src/engine.rs:a_muted_decision_plans_no_pulse_even_for_a_long_running_event`,
  `src/engine.rs:the_mute_beats_a_forced_phone_card_because_a_producer_cannot_overrule_the_operator`, and
  end to end
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`.
- Failure sources: an unreadable quiet-until file, an unreadable Focus store.
- Fail direction: not muted, in both cases (behaviors 13 and the quiet-window document).
- Thresholds: Not applicable here; the mute's expiry is judged on this run's own clock reading
  (`src/main.rs:muted_now`), which is the same clock every reading in this decision used.
- Required side effects: the durable log leg still fires and the `journal` still records the miss, so a
  suppressed event becomes a catch-up rather than a hole
  (`tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`
  asserts one hermes event and one journal entry).
- Forbidden side effects: the mute must never be applied as a filter over `decision.legs` afterwards.
  `src/main.rs` states it: "THE MUTE IS AN INPUT TO THE DECISION, stated here and nowhere else." And no
  environment variable may set either field: `src/engine.rs:Overrides::from_env` leaves `muted` and
  `focus_active` false unconditionally, because "a variable able to set it would let any producer mute
  the operator, and one able to clear it would silently end a mute they are still inside".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: both are read once per event, before `decide` (`src/main.rs`, the
  `Overrides { muted: ..., focus_active: ..., ..overrides_from_env() }` literal).
- Privacy: two booleans, recorded as two separate `decision ring` fields (`muted=` and `focus=`) "because
  `pns quiet` and a macOS Focus send the operator to completely different places"
  (`src/decision_log.rs:line`), asserted as `muted=no focus=yes` in
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the muted branch is written as a full struct literal with no `..delivery`,
  deliberately, so a future `DeliveryPlan` field must state its own answer rather than inherit an unmuted
  one (`src/engine.rs:decide`).

### 13. A macOS Focus is judged per mode and fails open

Given a `[focus] silence` list naming zero or more Focus modes When `src/main.rs:focus_now` reads the Do
Not Disturb store Then it is silenced only if a mode the list named is asserted right now, an empty list
reads nothing at all, and an unreadable store is not silenced.

- Success: `src/focus.rs:active_modes` takes `data[0].storeAssertionRecords` and collects each record's
  `assertionDetails.assertionDetailsModeIdentifier` into a `BTreeSet`; `src/focus.rs:mode_names` keys the
  catalog on each entry's own `mode.modeIdentifier`; `src/focus.rs:silenced` matches a config entry
  against either the identifier or the display name, case-insensitively. Pinned by
  `src/focus.rs:a_store_holding_a_live_assertion_names_the_mode_that_is_on`,
  `src/focus.rs:every_mode_asserted_at_once_is_named_and_not_just_the_first`,
  `src/focus.rs:a_mode_is_named_by_its_own_identifier_field_and_never_by_the_map_key`,
  `src/focus.rs:a_focus_nobody_named_silences_nothing`, and through the process
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual`.
- Failure sources: no store on the machine, a store behind Full Disk Access, something at the path that
  is not a regular file, a store past the 256 KiB `RING_READ_MAX` ceiling, bytes that are not JSON, a
  schema Apple moved.
- Fail direction: **open**. Bytes that are not JSON, a missing key, records that are not objects and two
  concatenated JSON documents all read as no Focus
  (`src/focus.rs:nothing_readable_names_no_mode_one_row_per_failure_shape`), and a failed read of the
  file itself reads as not silenced on the event path
  (`tests/dispatch.rs:a_focus_store_that_cannot_be_read_costs_no_notification_at_all`, which runs both
  "no store at all" and "something at the path that is not a file"). The module doc states the cost:
  failing closed "would silence every banner, card and pulse on the morning after an upgrade with nothing
  on screen to say why".
- Thresholds: `RING_READ_MAX` is 256 KiB (`src/main.rs:RING_READ_MAX`); the doc records the live store at
  6 KiB. An empty `[focus] silence` list is the feature off
  (`src/focus.rs:an_empty_list_is_the_feature_switched_off`).
- Required side effects: with a non-empty list, two file reads per event under
  `$HOME/Library/DoNotDisturb/DB`.
- Forbidden side effects: with an empty list the two files are never opened ("NOTHING NAMED MEANS NOTHING
  READ", `src/main.rs:focus_now`). No environment variable may name the store path, for the reason
  `Overrides::muted` states. `header.timestamp` must not be used as a freshness gate
  (`src/focus.rs:active_modes`: it "moves for writes that are not Focus transitions"). The
  `storeInvalidationRecords` array must never be read as active
  (`src/focus.rs:an_ended_focus_in_the_invalidation_history_is_never_an_active_one`).
- Timeout and cancellation: no subprocess. `readable_ring` refuses a non-regular file rather than opening
  it, because "opening a FIFO BLOCKS until the other end is opened, for READING as much as for writing"
  (`src/main.rs:readable_ring`, and the same refusal is exercised on the sibling ring files by
  `tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay`).
- Idempotency and duplicates: one reading per event; the catalog's failure rides out on
  `src/main.rs:FocusReading::catalog` rather than being re-read by the doctor, "because a second read is
  a second moment".
- Privacy: mode identifiers and display names only. The doctor prints the error kind, never the store's
  contents.
- Process ownership and cleanup: Not applicable; no children.
- Compatibility contract: the doctor's five sentences are
  `pns doctor: focus awareness is off (no [focus] table names a mode to silence)`,
  `pns doctor: a macOS Focus you named is ON, so banners, cards and pulses are suppressed`,
  `pns doctor: no macOS Focus you named is active`,
  `pns doctor: no Focus database was found on this machine, so no Focus is being respected`, and
  `pns doctor: the Focus database could not be read, so Focus is being ignored ({kind}).`; an unreadable
  catalog appends
  `; the mode catalog could not be read ({kind}), so no Focus NAME can match and only a raw modeIdentifier still would`
  (`src/main.rs:focus_line`, `src/main.rs:FOCUS_UNREADABLE`,
  `tests/dispatch.rs:the_doctor_tells_the_truth_about_a_named_focus_in_every_state`). Name matching is
  case mapping folded both ways and is explicitly not full Unicode case folding or normalization
  (`src/focus.rs:same`,
  `src/focus.rs:a_name_whose_lowercase_disagrees_with_itself_is_still_the_same_name`).

### 14. The desk idle probe reports whole seconds since the last physical input

Given a machine with a HID subsystem When `src/system.rs:idle_reading` runs Then it spawns
`/usr/sbin/ioreg -c IOHIDSystem`, takes the last whitespace field of the first line carrying
`HIDIdleTime`, and divides by 1_000_000_000 to whole seconds.

- Success: `src/system.rs:the_idle_probe_argv_matches_the_bash_original` pins the exact argv;
  `src/system.rs:the_idle_probe_reports_whole_seconds_from_the_nanosecond_count` and
  `src/presence.rs:a_nanosecond_counter_becomes_whole_seconds` pin the conversion.
- Failure sources: `ioreg` missing or exiting non-zero, output with no `HIDIdleTime` line, a line with no
  numeric last field, output containing a NUL or a replacement character.
- Fail direction: `None`, never zero. `src/presence.rs:idle_secs_from_ns` states it: "a garbled probe
  line must never coerce to 0, which reads as 'actively typing' and silently drops the push." Pinned by
  `src/presence.rs:an_empty_reading_is_unknown_rather_than_zero_seconds_idle`,
  `src/system.rs:output_without_the_idle_key_reads_as_unknown_rather_than_zero`,
  `src/system.rs:contaminated_idle_output_reads_as_unknown_rather_than_a_reading` and
  `src/system.rs:a_garbled_idle_count_is_unknown_rather_than_zero_seconds_idle`.
- Thresholds: division truncates rather than rounding up: 1_999_999_999 nanoseconds is 1 second and 0 is
  0 (`src/presence.rs:a_partial_second_truncates_rather_than_rounding_up`). The parse rejects padding,
  signs and leading zeros (`src/lib.rs:parse_count`), so `"5000000000 "` is unknown.
- Required side effects: one `ioreg` child per event, unless the caller stated the answer.
- Forbidden side effects: the reading must not be taken from a second device's line: the first line
  carrying the key wins (`src/system.rs:the_first_idle_line_wins_so_a_second_device_cannot_override_it`).
- Timeout and cancellation: 5 seconds; over-deadline or over-cap yields `None` after `kill()` and
  `wait()`.
- Idempotency and duplicates: memoized (behavior 20).
- Privacy: an age in seconds. No key codes, no window titles, no application names.
- Process ownership and cleanup: `run_bounded` owns the child.
- Compatibility contract: `/usr/sbin/ioreg` is absolute, "because a probe must not resolve a system
  binary through a PATH it does not control" (`src/system.rs:IOREG_PATH`), which is also why
  `tests/support/mod.rs:spy_path` cannot stand in front of it.

### 15. The console lock is read only where the idle clock answered

Given the idle reading for this event When it was stated by the caller, was garbled, or came back `None`
Then the lock probe is never spawned; only an idle reading that really arrived earns the second `ioreg`.

- Success: the guard is
  `let (desk_input_age, screen_locked) = if overrides.reads_desk() { let idle = probes.idle_secs(); (idle, idle.is_some().then(|| probes.screen_locked()).flatten()) } ...`
  (`src/engine.rs:surface_reading`). Pinned at the trait level by
  `src/engine.rs:the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading`, which runs four
  cases (nothing stated, a stated clock, a garbled clock, an unreadable clock) asserting idle and lock
  read counts of (1,1), (0,0), (0,0) and (1,0); and at the spawn level by
  `src/system.rs:the_lock_is_not_spawned_where_idle_failed`, which asserts exactly one `ioreg` spawn
  total.
- Failure sources: as behavior 8.
- Fail direction: an unattempted lock and an unreadable lock are both `None`, and `None` never locks.
  `src/system.rs:join_desk` deliberately fills both cells from one join "even when the lock was never
  attempted (idle failed to parse) ... the cell holds `None` either way".
- Thresholds: Not applicable.
- Required side effects: at most one extra `ioreg` child per event.
- Forbidden side effects: no second `ioreg -n Root -d1` for an answer already held. `start` captures
  `lock_already_known` before spawning so a lock read inline before `start` is not retaken
  (`src/system.rs:a_lock_probe_already_answered_inline_is_not_retaken_by_a_later_start`).
- Timeout and cancellation: 5 seconds.
- Idempotency and duplicates: memoized in `SystemProbes::screen_locked`.
- Privacy: a boolean.
- Process ownership and cleanup: as behavior 14. The measured cost difference is recorded: the Root read
  is "92KB against 294KB, measured on dresden 2026-08-28" (`src/system.rs:lock_reading`).
- Compatibility contract: the doc records that nothing in this repository sets `PNS_IDLE_SECS` in
  production (measured repository-wide 2026-08-28) and warns that "a future setter would silently disable
  the override with it" (`src/engine.rs:surface_reading`).

### 16. The Back Tap marker is read as the link's own modification time

Given a marker path from `PNS_PHONE_MARKER_FILE` or the default
`$HOME/.local/state/pns/phone-attention.marker` When `PhoneMarkerProbe::marker_mtime_secs` reads it Then
it uses `symlink_metadata`, so a dangling symlink still carries a reading.

- Success: `src/system.rs:the_marker_probe_reports_the_files_modification_time_in_whole_seconds` and
  `src/system.rs:the_marker_probe_reads_the_link_itself_never_its_target`. Through the process:
  `tests/dispatch.rs:a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile`.
- Failure sources: nothing at the path, a stat that fails, a modification time before the Unix epoch.
- Fail direction: `None`, which is never fresh, so the tap does not speak for the phone
  (`src/system.rs:an_absent_marker_reports_unknown_which_the_marker_rule_fails_closed_on`).
- Thresholds: the marker has no time-to-live of its own; it competes on age like every other signal
  (behavior 6).
- Required side effects: one `symlink_metadata` call. No subprocess.
- Forbidden side effects: the link must not be followed, "matching BSD `stat -f %m`: the Back Tap touch
  lands on this path, so a dangling link still carries the reading and following it would erase one".
- Timeout and cancellation: Not applicable; there is no deadline on the filesystem call.
  `NOT ESTABLISHED:` no guard against a marker path on a hung filesystem; grepped `src/system.rs` for a
  bound around `symlink_metadata` and found none.
- Idempotency and duplicates: memoized in `SystemProbes::marker_mtime`.
- Privacy: an mtime. The marker's contents are never read.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the marker is never started ahead, because it spawns nothing
  (`src/probes.rs:Wants`).

### 17. The phone's input clock is the mosh client pty's access time

Given one or more attached mosh sessions When `src/system.rs:phone_reading` runs Then it walks
`/usr/bin/pgrep -x mosh-server`, then `/usr/bin/pgrep -P <ids>`, then `/bin/ps -o tty= -p <ids>`, and
takes the newest access time among `/dev/<name>` for the terminals named.

- Success: `src/system.rs:the_discovery_argv_is_pinned_to_the_chain_that_was_measured_live` pins the
  three calls in order; `src/system.rs:every_server_and_every_client_is_asked_for_in_one_call_each` pins
  that the id lists are batched so the spawn count stays three however many sessions are open;
  `src/system.rs:the_freshest_terminal_wins_across_every_session_found` pins the maximum.
- Failure sources: any of the three commands missing or failing, no `mosh-server` at all, a client with
  no controlling terminal (`ps` prints `??`), a terminal that cannot be stat-ed.
- Fail direction: `None` at any step, which is never fresh.
  `src/system.rs:a_failure_at_any_step_of_the_chain_reads_as_no_phone_rather_than_a_fresh_one` drops each
  scripted answer in turn; `src/system.rs:a_server_whose_client_has_no_terminal_reads_as_no_phone` and
  `src/system.rs:a_terminal_that_cannot_be_stat_ed_drops_out_without_taking_the_others_with_it` cover the
  rest. The whole-chain consequence is pinned in
  `src/surface.rs:a_phone_reading_that_could_not_be_taken_never_counts_as_fresh`: "The discovery chain
  has four steps and any of them can come back with nothing."
- Thresholds: the freshness window from behavior 5. Note that an attached-but-untouched mosh session
  decides nothing: "Presence is the pty's CLOCK, never the session's existence"
  (`src/surface.rs:a_stale_phone_reading_loses_to_the_desk_rather_than_holding_mobile`).
- Required side effects: up to three children per event, unless the caller stated the answer.
- Forbidden side effects: `pgrep -P` must not be called with an empty parent list, because that is a
  usage error rather than a query answering "none" (`src/system.rs:pgrep_children`,
  `src/system.rs:no_mosh_server_at_all_never_asks_for_children_of_nothing`). The reading is `atime`,
  never `mtime`: "atime is input and mtime is the agent talking back", "Proven live on 2026-08-15 in both
  directions". Reading the terminal must not itself disturb it: `src/system.rs:atime_secs` is "A plain
  `stat`, which does not itself count as an access".
- Timeout and cancellation: 5 seconds per command, so the chain is bounded at roughly 15 seconds in the
  worst case.
- Idempotency and duplicates: memoized in `SystemProbes::phone_atime`.
- Privacy: process ids and terminal names, then a file access time. No pty contents are read.
- Process ownership and cleanup: each of the three children is owned and reaped by `run_bounded`.
- Compatibility contract: `src/system.rs:parse_tty_names` is a trust boundary, because the name is joined
  onto `/dev`. Only plain ASCII alphanumerics survive, so no reading can carry a slash or a `..` into the
  join (`src/system.rs:a_name_that_could_escape_the_device_directory_is_refused_outright`, which runs
  `../../etc/passwd`, `..`, `tty/../../root`, `tty s000`, `tty;rm` and `tty.0`). Padding is trimmed here,
  unlike in `parse_pids`, "because `ps -o tty=` pads its column by design"
  (`src/system.rs:a_padded_terminal_name_is_trimmed_because_the_padding_is_the_format`,
  `src/system.rs:a_padded_pid_line_is_rejected_like_any_other_malformed_line`).

### 18. Every subprocess reading is bounded in time and in bytes

Given any probe command When `src/system.rs:run_bounded` runs it Then the answer is discarded and the
child killed if the deadline passes, and discarded if the output exceeds the ceiling.

- Success: `src/system.rs:the_production_runner_captures_stdout_on_success`;
  `src/system.rs:a_short_answer_is_told_apart_from_the_cap_because_it_is_still_an_answer` shows the
  ceiling itself is a working answer.
- Failure sources: a missing binary, a non-zero exit, a wedged child, a child that closes stdout and
  sleeps, a child that streams past the cap.
- Fail direction: `None` in every case, which every caller reads as unknown, and unknown never
  suppresses. A truncated answer is refused rather than returned, because "a process list cut at the
  ceiling has lost its last rows and a JSON listing has stopped mid-object, and both arrive at a caller
  looking exactly like a complete short answer"
  (`src/system.rs:a_command_that_talks_past_the_cap_is_no_answer_rather_than_a_truncated_one`,
  `src/system.rs:the_production_runner_yields_no_reading_from_a_failing_command`,
  `src/system.rs:the_production_runner_yields_no_reading_for_a_missing_binary`).
- Thresholds: `PROBE_DEADLINE` is **5 seconds** and `PROBE_READ_MAX` is **1 MiB inclusive**: an answer of
  exactly 4096 bytes under a 4096-byte cap is returned in full, and 100_000 bytes under that cap is
  `None`. The wait polls with an exponential backoff starting at **200 microseconds**
  (`FIRST_POLL_INTERVAL`) and doubling to a ceiling of **10 milliseconds** (`POLL_INTERVAL`), pinned by
  `src/system.rs:the_wait_between_checks_doubles_and_stops_at_the_ceiling` and
  `src/system.rs:the_wait_sleeps_the_schedule_it_computes_rather_than_a_flat_ceiling`. The ceiling
  matters because the wait "begins after the child's stdout has already hit EOF, which is a child on its
  way out".
- Required side effects: one child process, one reader thread per call.
- Forbidden side effects: stderr is `Stdio::null()`, so a probe never writes to the operator's terminal.
  Stdin is `Stdio::null()` when no stdin text is given.
- Timeout and cancellation: on a blown deadline, an over-cap answer or a wait that does not finish by
  `expires_at`, the child gets `kill()` then `wait()`. The bytes are capped by
  `Read::take(max_bytes + 1)`, "which is also what stops it writing", so a runaway child does not grow
  the buffer without bound.
- Idempotency and duplicates: each call is a fresh spawn; the memoization is one level up (behavior 20).
- Privacy: only stdout is captured, and only up to the cap; the conversion is lossy
  (`String::from_utf8_lossy`) and only after the size has been judged, so "one invalid byte must cost its
  own line rather than the whole answer"
  (`src/system.rs:the_production_runner_keeps_a_reading_with_stray_invalid_bytes`).
- Process ownership and cleanup: the child is always killed and reaped on the failure path. The reader
  thread is **not** joined; it ends when the pipe closes.
- Compatibility contract: macOS ships no `timeout(1)` and the standard library has no wait-with-timeout,
  which is why the wait is a polling loop with an injectable sleeper (`src/system.rs:wait_until`).

### 19. Every timestamp is aged against one clock read

Given a phone atime and a marker mtime, both absolute epoch seconds When `src/engine.rs:surface_reading`
ages them Then both use the single `now_secs` value taken at the edge, and an unreadable clock ages
neither.

- Success:
  `let age_of = |taken_at: Option<u64>| now_secs.and_then(|now| Some(now.saturating_sub(taken_at?)));`
  (`src/engine.rs:surface_reading`). `src/system.rs:SystemProbes::now_secs` memoizes the clock: "THE
  FIFTH MEMOIZED READING", pinned by `src/system.rs:the_clock_is_the_fifth_memoized_reading`.
- Failure sources: `SystemTime::now().duration_since(UNIX_EPOCH)` failing, which is a clock before the
  epoch.
- Fail direction: `None`, which makes both timestamp-based signals absent rather than infinitely fresh
  (`src/engine.rs:an_unreadable_clock_ages_no_phone_signal_rather_than_treating_it_as_fresh`,
  `src/engine.rs:an_unreadable_clock_ages_no_marker_rather_than_treating_it_as_fresh`). An unreadable
  clock is memoized too, so "the first reader's `None` is the second reader's `None`".
- Thresholds: subtraction is saturating, so a timestamp in the future ages to 0 rather than wrapping.
- Required side effects: none beyond one clock read per invocation.
- Forbidden side effects: no second wall-clock read for one event. `src/main.rs` states the reason at
  both call sites: "Two reads of the wall clock for one event is the boundary that drifted a phone
  reading and a desk reading apart" (R4-1). `src/main.rs:mark_present` and
  `src/missed_notifications.rs:entry` both take their epoch from `decision.inputs.now_secs` rather than
  calling `SystemTime` again.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: `OnceCell`-backed; every reader of one probe set sees the same second.
- Privacy: an epoch second, written into the `decision ring` as the line's leading field, or `-` when
  there was no clock
  (`src/decision_log.rs:a_line_with_no_readable_clock_leads_with_a_dash_rather_than_epoch_zero`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `with_clock` is `cfg(test)`-only and reads no environment variable, "so the
  wall clock has no operator-facing knob to seed it by accident"
  (`src/system.rs:SystemProbes::with_clock`).

### 20. One probe set is one reading, including the reading that came back empty

Given one `SystemProbes` built for one invocation When two consumers ask for the same reading Then the
command runs once and both get the same answer, and that holds for `None` as much as for a value.

- Success: `src/system.rs:a_reading_asked_for_twice_is_still_taken_once` and
  `src/system.rs:a_reading_that_came_back_empty_is_not_retaken_either`, both asserting exactly one runner
  call; `src/system.rs:the_lock_probe_reads_the_root_dictionary_by_exact_argv_and_only_once`; and at the
  engine level `src/engine.rs:one_decision_reads_each_probe_at_most_once_and_never_twice`, which bounds
  idle, marker, phone input and session view at one read each for one `decide`.
- Failure sources: a second `SystemProbes` being built for one event, which would defeat the whole
  property.
- Fail direction: not applicable as a reading; the design failure is a freshness boundary falling between
  two measurements of one moment.
- Thresholds: Not applicable.
- Required side effects: `src/main.rs:system_probes` builds exactly one probe set per invocation, and
  `src/main.rs:blocking_event` hands the same one to `forward_to_moshi` and to `run_event`. The struct
  doc names the case that makes it load-bearing: "The blocked path is what makes this load-bearing: it
  asks where the operator is twice by design ... Taking the measurement twice lets a freshness boundary
  fall between them, which cards a phone with no round trip behind it." Verified end to end by
  `tests/hooks.rs:a_phone_used_more_recently_than_the_desk_gets_the_approval_forwarded_to_it`.
- Forbidden side effects: no consumer may take its own reading of a fact the decision already read.
  `src/engine.rs:writing_the_record_consults_no_probe_the_decision_had_not_already_read` pins that for
  the record site.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: this behavior is the idempotency guarantee.
- Privacy: Not applicable.
- Process ownership and cleanup: `OnceCell` per reading, plus a `Cell` holding each in-flight thread
  handle. The struct stays single-threaded; only the probe bodies run elsewhere
  (`src/system.rs:desk_handle_absent`).
- Compatibility contract: `src/probes.rs` implements every trait for `&T`, "which is what lets them share
  without moving ownership", so a caller can hand `&probes` to two consumers and still get one reading.
  `SystemProbes::session_view` is the one probe with no cell, safe only while it has one production
  reader.

### 21. Independent subprocess probes are started ahead and run concurrently, and the session view is not one of them

Given a caller that has not stated the desk clock or the phone age When `src/engine.rs:surface_reading`
calls `probes.start(Wants { desk, phone })` Then `SystemProbes` spawns one thread for the desk pair and
one for the phone chain, and the reads below join them.

- Success: `src/system.rs:a_slow_probe_does_not_hold_up_a_fast_one` proves the overlap by order rather
  than by time: the phone thread's `pgrep` releases a blocked `ioreg`, so a sequential or desk-only
  mutant never releases it and the idle assertion goes red.
  `src/system.rs:a_desk_only_start_spawns_no_phone_thread_and_the_phone_still_reads_inline` and
  `src/system.rs:a_phone_only_start_spawns_no_desk_thread_and_the_desk_still_reads_inline` cover the
  narrowed cases.
- Failure sources: the operating system refusing a thread.
- Fail direction: a refused spawn falls back to the inline read: `.ok()` "drops a spawn failure into
  'nothing started', which is exactly the state `join_desk` and the trait impls already treat as 'compute
  it when asked'" (`src/system.rs` `ProbeStart::start`). `src/probes.rs:ProbeStart` states the general
  rule: "STARTING IS NEVER READING. A caller that never calls this still gets an answer."
- Thresholds: Not applicable. **What is concurrent and what is not:** the desk pair (idle, then the lock
  it qualifies) and the phone chain overlap each other. The Back Tap marker, the wall clock and the
  session view do not overlap anything. `src/engine.rs:decide` calls `surface_reading` first and
  `operator_visibility` second, and `surface_reading` joins both threads before returning, so the two
  `herdr` calls are strictly serial after the desk and phone readings. Derived worst case for one
  `decide`: the desk thread is two commands at 5 seconds each and the phone thread is three, so the
  started pair is bounded near 15 seconds, plus two serial `herdr` calls bounded near 10 seconds, for
  roughly 25 seconds; in practice every command answers in milliseconds (`src/system.rs:PROBE_DEADLINE`:
  "All of them answer in milliseconds, so this is generous and still far short of a hang").
- Required side effects: at most two threads, carrying at most five child processes between them (two on
  the desk thread, three on the phone thread). The two `herdr` calls in behavior 3 are on top of that and
  are not started ahead.
- Forbidden side effects: a probe must never be started for an answer the caller already gave.
  `Overrides::reads_desk` and `Overrides::reads_phone` are the single spelling of that rule, read by
  `start` and by the read guards alike, pinned by
  `src/engine.rs:reads_desk_is_true_only_when_the_idle_guard_below_would_run_the_probe`,
  `src/engine.rs:reads_phone_is_true_only_when_the_phone_guard_below_would_run_the_chain` and
  `src/engine.rs:start_is_asked_for_exactly_what_the_read_guards_below_it_would_consult`, which covers
  valid and garbled overrides on both sides and asserts `start_calls == 1` on each. `start` is called
  exactly once per `surface_reading` call, which is once per `decide`; on the blocking path
  `surface_reading` runs twice on one probe set (once through `operator_surface`, once through `decide`),
  so `start` is called twice there and still spawns each probe once
  (`src/system.rs:starting_twice_and_reading_twice_spawns_each_probe_once`).
- Timeout and cancellation: the threads are not cancellable. Each is bounded only by the deadlines of the
  commands it runs; `join` waits for the thread to finish, and a panicking probe thread is re-raised in
  the joiner (`resume_unwind` in `src/system.rs:join_desk` and `src/system.rs:join_phone`).
- Idempotency and duplicates: `start` is a no-op if the cell is already filled or a handle is already in
  flight, so starting twice and reading twice still spawns each probe once
  (`src/system.rs:starting_twice_and_reading_twice_spawns_each_probe_once`, which also asserts the joined
  values, not just the call counts).
- Privacy: Not applicable.
- Process ownership and cleanup: every thread started is joined by a read on the same path before
  `surface_reading` returns, which is what makes the one `unsafe { set_var }` in this crate sound
  (`src/system.rs` `ProbeStart::start`: "`set_var` is `unsafe` because libc readers such as `localtime_r`
  do not take std's environment lock ... Keep it when adding a thread or a `set_var`.").
- Compatibility contract: `src/probes.rs:ProbeStart::start` is a no-op by default, so every test double
  answers identically started or unstarted; only `SystemProbes` overrides it.

### 22. A stated reading is trusted and its probe never runs

Given `PNS_IDLE_SECS` or `PNS_PHONE_INPUT_AGE` set to a valid count When `src/engine.rs:surface_reading`
needs that reading Then it uses the stated value and neither starts nor reads the probe underneath it.

- Success: `src/engine.rs:an_overridden_idle_reading_spares_the_idle_probe` (idle reads 0) and
  `src/engine.rs:a_stated_phone_input_age_spares_the_process_walk_behind_it` (phone reads 0). The module
  doc states the reason: "Every reading is a spawn on a path that must never stall, so a caller who
  already stated an answer never pays for the probe underneath it."
- Failure sources: a variable present but not a count (behavior 23).
- Fail direction: not applicable for a valid value; the caller's word is taken as given.
- Thresholds: `src/lib.rs:parse_count` accepts plain ASCII digits only, rejects the empty string, rejects
  leading zeros beyond a single `0`, rejects signs and padding, and caps at `i64::MAX`.
- Required side effects: none.
- Forbidden side effects: stating the desk clock also suppresses the lock probe, because the lock exists
  only to qualify the idle reading and "stating the desk clock states the desk's whole story, garbled
  value included" (`src/engine.rs:the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the environment is snapshotted once per invocation
  (`src/main.rs:overrides_from_env` collects `std::env::vars_os()` into one `BTreeMap`).
- Privacy: Not applicable.
- Process ownership and cleanup: fewer children, by design.
- Compatibility contract: `PNS_IDLE_SECS`, `PNS_DESK_IDLE_SECS`, `PNS_PHONE_INPUT_AGE`, `PNS_SKIP_PHONE`,
  `PNS_FORCE_PHONE` and `PNS_PHONE_MARKER_FILE` are the six presence-facing variables. `muted` and
  `focus_active` are unreachable from any of them (`src/engine.rs:Overrides::from_env`). The overrides
  steer the delivery decision only: `src/main.rs:last_interaction` states that "`PNS_IDLE_SECS` and
  `PNS_PHONE_INPUT_AGE` steer the delivery decision in `engine::decide`, not this reading: the `unread`
  lamp always sees the machine's own probes."

### 23. A garbled override answers unknown outright, never a fallback

Given a presence variable that is present, non-empty and not a count When
`src/engine.rs:Overrides::from_env` parses it Then the value is `None` and the matching `*_invalid` flag
is set, and the reading is treated as unknown rather than falling back to a probe or a default.

- Success: `src/engine.rs:a_garbage_idle_override_is_unknown_without_a_probe_read` and
  `src/engine.rs:a_garbage_phone_override_is_unknown_without_a_probe_read`, both asserting the probe read
  count stays 0. Through the process:
  `tests/hooks.rs:a_presence_reading_nobody_can_parse_still_forwards_the_approval`, with the mutation
  measured in its comment ("`surface_reading` reading an invalid idle override as a desk just touched
  (`(Some(0), None)` in place of `(None, None)`) kills this test").
- Failure sources: this behavior is the failure handling.
- Fail direction: unknown, which for the desk clock and the phone means "does not compete", so the
  arbitration falls toward `Away`. For the freshness window it is stronger: a garbled
  `PNS_DESK_IDLE_SECS` returns a `SurfaceReading` of `Away` with every field `None` and no probe read at
  all, because "substituting 120 would read a stale desk as fresh and hold the operator at their desk"
  (`src/engine.rs:surface_reading`,
  `src/engine.rs:a_garbage_desk_threshold_fails_toward_away_never_into_the_default`, which uses `"0600"`,
  a value `parse_count` rejects for its leading zero).
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: no fallback to the live probe and no fallback to the default. The field doc
  states the rule: "the fallback is what would turn an unknown into a confident number: a probe reading
  where the caller overrode it, or a default threshold where the caller's was garbled."
- Timeout and cancellation: with a garbled desk threshold, `surface_reading` returns before
  `probes.start` is ever called, so no thread and no child is created.
- Idempotency and duplicates: one parse per invocation.
- Privacy: the raw value is never echoed. The `decision ring` records only `idle_invalid=`,
  `desk_invalid=` and `phone_invalid=` as yes or no (`src/decision_log.rs:line`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: an empty variable is treated as absent, not as garbled
  (`vars.get(key).filter(|raw| !raw.is_empty())`).

### 24. One submission decides on one coherent snapshot, read at the last moment before delivery

Given one event When `src/engine.rs:decide` runs Then every reading it needs is taken at dispatch, once,
into `src/engine.rs:GateInputs`, and nothing below that point touches a probe.

- Success: `src/engine.rs:GateInputs` carries all thirteen inputs and its doc states the timing contract:
  "operator ruling 2026-08-13: the decision evaluates the world at the LAST MOMENT BEFORE DELIVERY, and
  never earlier than the return of the work being reported on ... So the reading is taken here, at
  dispatch, and NOTHING BELOW THIS POINT touches a probe: one decision cannot be split across two
  readings that disagree about where the operator is." Pinned end to end by
  `tests/hooks.rs:the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started`, which backdates
  the marker inside the condenser stub so a start-time reading would card the phone and a dispatch-time
  reading banners the desk.
- Failure sources: a probe answering differently between two reads, which the memoization prevents; a
  second `SystemProbes`, which the composition root prevents.
- Fail direction: an unreadable reading is carried as `None` all the way into the record, never
  substituted (`src/engine.rs:a_reading_nobody_could_take_is_reported_as_absent_and_never_as_a_number`).
- Thresholds: Not applicable.
- Required side effects: `GateInputs` is assembled in exactly one place, with every field stated,
  "because a struct assembled in two places is one a later edit can leave holding a default nobody meant"
  (`src/engine.rs:decide`).
- Forbidden side effects: nothing downstream may re-read a probe. `src/engine.rs:SurfaceReading` states
  it: "Nothing downstream may re-read them: a second reading is a second moment." Pinned by
  `src/engine.rs:writing_the_record_consults_no_probe_the_decision_had_not_already_read`.
- Timeout and cancellation: bounded by behavior 18 and behavior 21.
- Idempotency and duplicates: a single `decide` per event on the alert path. On the blocking path
  `operator_surface` runs first and `decide` second, on the same probe set, so both see the same snapshot
  (`src/main.rs:forward_to_moshi`,
  `src/system.rs:starting_twice_and_reading_twice_spawns_each_probe_once`).
- Privacy: `GateInputs` deliberately does not carry the pane's value, only `pane_present`: "Its VALUE is
  never carried: the decision used it for exactly this and for the safety check beside it."
- Process ownership and cleanup: covered by behaviors 18 and 21.
- Compatibility contract: `GateInputs` is `Copy` and is carried out on `Decision::inputs`, so callers
  read the decision's own readings rather than taking new ones
  (`src/engine.rs:a_decision_reports_the_readings_its_surface_was_decided_from`).

### 25. The decision carries out the readings it ran on, and the record is written from them

Given a completed `Decision` When `src/decision_log.rs:line` writes the `decision ring` entry Then the
surface, both visibilities, the three ages, the lock, the freshness window and every override flag are
written from `decision.inputs`, with no second reading.

- Success: the byte-for-byte line assertion in `src/decision_log.rs` covers
  `surface=Mobile visibility=Hidden session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=no fresh_window=120 ... plan=banner:no,card:no,pulse:no legs=none`,
  and a second assertion covers `locked=none` for an unread lock.
- Failure sources: an unreadable clock, which writes `-` as the leading field
  (`src/decision_log.rs:NO_CLOCK`).
- Fail direction: absent readings are written as `none`, never as `0` (`src/decision_log.rs:count`,
  `src/decision_log.rs:tri`).
- Thresholds: Not applicable.
- Required side effects: one `decision ring` line per event, written after every channel and before the
  pulse, and written on both branches: "'Nothing fired' is exactly what an operator opens the report to
  ask about" (`src/main.rs`).
- Forbidden side effects: the ring must not carry the pane's value or a channel's own sentence
  (`src/decision_log.rs:verdicts`: "THE VARIANT NAME AND NEVER THE SENTENCE").
- Timeout and cancellation: the record is written before the pulse because the pulse "talks to a bridge
  under a ten-second deadline and would take the record with it"; the accepted price is stated: "a
  decision is lost if a channel hangs to its deadline and the process is killed before this runs"
  (`src/main.rs`).
- Idempotency and duplicates: one line per dispatch attempt; a nudge is flagged `nag=yes`.
- Privacy: agent, state, permission mode, payload agent id and tool name pass through
  `src/decision_log.rs:printable`; the pane is reduced to `present` or `none`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the field order and spelling are asserted byte for byte, so any change to
  `GateInputs` that reaches the line must be made deliberately.

### 26. An unsafe pane still decides visibility and is dropped only from delivery

Given an origin pane containing characters outside the allowlist When `src/engine.rs:decide` runs Then
the raw pane is still used for the session-view read and `pane_dropped` is set, and the pane is blanked
once at dispatch with a single warning.

- Success: `decide` computes `pane_dropped: !pane.is_empty() && !crate::safety::pane_is_safe(pane)` after
  calling `operator_visibility(probes, pane)` with the raw value.
  `src/engine.rs:an_unsafe_pane_is_dropped_once_for_every_channel` and
  `src/engine.rs:a_safe_pane_is_not_dropped` pin the flag;
  `tests/dispatch.rs:a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event` asserts the
  delivered event carries an empty pane.
- Failure sources: a multiplexer id this crate does not own carrying shell metacharacters.
- Fail direction: the pane is dropped from delivery, so the banner loses its click-to-focus rather than
  gaining an injection.
- Thresholds: the allowlist is ASCII alphanumerics plus `.`, `_`, `:` and `-`
  (`src/safety.rs:pane_is_safe`). The colon "earns its place by being herdr's own separator (`wW:p21`)
  and by being no operator at all".
- Required side effects: exactly one stderr line at dispatch:
  `pns: dropped a pane id with shell metacharacters; no channel will focus a pane`
  (`src/main.rs:dispatch_legs`), sanitized once rather than per channel.
- Forbidden side effects: the raw pane must never reach a channel. It does reach the session-view probe,
  and that is safe because `SystemCommandRunner::run` builds the child with
  `Command::new(program).args(args)`, so `--pane <raw>` is one argv element and no shell parses it
  (`src/system.rs` `CommandRunner for SystemCommandRunner`). `NOT ESTABLISHED:` no test asserts that an
  unsafe pane still reaches the session-view read; grepped `tests/dispatch.rs` and `src/engine.rs` for a
  case pairing `pane_dropped` with a view read count, and found none. The property is read off the source
  ordering in `src/engine.rs:decide`.
- Timeout and cancellation: as behavior 3.
- Idempotency and duplicates: the warning is printed once per dispatch, not once per leg ("Warned about
  only now, because a scrub nobody was going to receive is not news").
- Privacy: the pane value never reaches the `decision ring`, only `pane=present` and `pane_dropped=yes`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: an empty pane is not "unsafe", it is "absent", and
  `src/engine.rs:operator_visibility` short-circuits it to `Unknown` before any probe runs.

### 27. Presence decides whether an event counts as seen

Given a completed `Decision` When `src/missed_notifications.rs:is_present` asks whether the operator was
there Then the answer is `decision.inputs.surface != Surface::Away`, and a present event moves the
last-present marker while an away event does not.

- Success: `tests/dispatch.rs:a_present_event_moves_the_last_present_marker_and_an_away_event_does_not`;
  `tests/dispatch.rs:a_present_event_delivers_one_extra_notification_carrying_the_whole_journal` and
  `tests/dispatch.rs:an_away_event_delivers_no_replay_and_leaves_the_journal_byte_identical`.
- Failure sources: an unreadable clock, which leaves `now_secs` absent.
- Fail direction: `src/main.rs:mark_present` returns without writing when the clock is absent, and the
  marker only ever moves forward.
- Thresholds: Not applicable; presence here is the surface enum, with no time in it.
- Required side effects: the marker write happens inside `claim_moment`, because a measured race ("one
  run in sixty with eight racers") let a second owner republish the marker and put two cards on the phone
  at one moment (`src/main.rs:mark_present`).
- Forbidden side effects: the epoch written must be the decision's own clock read, "taken off the
  readings it decided from rather than by a second `SystemTime` call".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the read in front of the claim means an edge already at or past this event
  takes no claim at all;
  `tests/dispatch.rs:racing_present_events_deliver_exactly_one_replay_between_them` and
  `tests/dispatch.rs:the_marker_advances_so_a_second_present_event_recaps_nothing` pin the duplicate
  suppression.
- Privacy: an epoch second in a state file.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `Mobile` counts as present, so a phone in hand suppresses a `journal` replay
  exactly as a desk does.
