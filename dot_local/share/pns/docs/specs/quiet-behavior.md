# Quiet behavior

## Scope

Every way `pns` is silenced, and exactly what each one silences. Five mechanisms are covered: the
operator's own typed mute (`pns quiet`, state file `quiet-until`), macOS Focus read through the Do Not
Disturb store and filtered by `[focus] silence`, the quiet window (the config key is
`[plugins.hue] quiet_hours`, and the parsed value is `hue::QuietWindow`), the dim window (per lamp, room
or zone `dim_window` plus `dim_behaviours`), and the lamps' own by-hand mute (`pns lights quiet`, state
file `lights-quiet`). Two of those names turn out to be one mechanism and the evidence is in behavior 14.
Everything below is derived from the crate at `dot_local/share/pns` and its tests only. Where the code
does not settle a question, the line begins `NOT ESTABLISHED:` and names what was looked for and where.
Approvals get their own behavior (9) because the exemption is structural rather than conditional.

## The mechanisms

| Mechanism                                             | Where its state lives                                                                                                                                                                      | What it silences                                                                                                                                                            | What it does NOT silence                                                                                                                                                                        | How it expires                                                                                                               | Tests that pin it                                                                                                                                                                                                                                                                       |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Operator mute (`pns quiet <duration>`)                | `<state>/quiet-until`, one line holding an absolute epoch second, mode `0600` (`src/main.rs:QUIET_UNTIL`, `src/main.rs:STATE_FILE_MODE`)                                                   | The banner, the phone card and the pulse for one event, plus the blocked lamp's one flash (`src/engine.rs:decide`, `src/main.rs:blocked_lamp` gate at the composition root) | The durable log leg, the moshi approval forward, the decision ring, the journal, the activity ring, the news record, the blocked marker, the tick's sustained breath, `pns pulse`, `pns doctor` | Half open against the run's own clock: `now < expiry` (`src/quiet.rs:is_muted`). `pns quiet off` unlinks the file            | `src/quiet.rs` unit tests; `tests/dispatch.rs:a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge`; `tests/dispatch.rs:the_operators_own_mute_takes_the_blocked_lamp_with_everything_else`                                                                            |
| macOS Focus (`[focus] silence`)                       | Apple's own store at `$HOME/Library/DoNotDisturb/DB/{Assertions.json,ModeConfigurations.json}` (`src/main.rs:FOCUS_DB`); the policy list is config (`src/config.rs:Config::focus_silence`) | Exactly what the operator mute silences: the same `Overrides::silenced()` predicate (`src/engine.rs:Overrides::silenced`)                                                   | The same list as above, approvals included                                                                                                                                                      | Nothing pns owns. It ends when macOS moves the assertion record out of `storeAssertionRecords` (`src/focus.rs:active_modes`) | `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`; `tests/hooks.rs:a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`                                                                                     |
| Quiet window (config key `[plugins.hue] quiet_hours`) | The config file (`src/channels/hue.rs:quiet_window`)                                                                                                                                       | On a machine with NO `[lights]` table: the whole room pulse (`src/main.rs:fire_pulse_unless_quiet`). Nothing else, ever                                                     | Cards, banners, the durable log, and every routed lamp on a machine that DOES have a `[lights]` table                                                                                           | Minute of the local day, start inclusive and end exclusive, may wrap midnight (`src/channels/hue.rs:quiet_now`)              | `tests/dispatch.rs:a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`; `tests/dispatch.rs:a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`; `src/channels/hue.rs:a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end` |
| Dim window (`dim_window`, `dim_behaviours`)           | The config file, per lamp, room or zone, arbitrated most specific first (`src/channels/hue.rs:DimWindow`)                                                                                  | Per lamp and per behavior: a behavior inside the window either runs its dim form or is taken away entirely (`src/channels/hue.rs:dim_showing`)                              | Cards, banners, the durable log, the pulse's decision, and any lamp that states no window                                                                                                       | The same minute-of-day rule, reusing `quiet_now` over its own `QuietWindow`                                                  | `src/channels/hue.rs:inside_a_window_an_enabled_behaviour_runs_dim_and_one_that_is_not_is_suppressed`; `tests/dispatch.rs:an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg`                                                                                     |
| Lamps' by-hand mute (`pns lights quiet <place>`)      | `<state>/lights-quiet`, one line per place as `<epoch> <place>`, at most 32 lines, mode `0600` (`src/main.rs:LIGHTS_QUIET`, `src/lights.rs:MAX_MUTED_PLACES`)                              | Every behavior on every lamp that answers to the named lamp, room or zone, on both the event flash and the tick's sustained breath (`src/channels/hue.rs:muted_now`)        | Cards, banners, the durable log, `pns quiet`'s file, the pulse's plan, lamps outside the named place                                                                                            | Per entry, half open on `quiet::is_muted`; expired entries are dropped on the next write (`src/lights.rs:muted_after`)       | `tests/dispatch.rs:an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`; `tests/dispatch.rs:a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch`                                                                                               |

Two families, two opposite fail directions. The engine mutes (operator mute, Focus) FAIL OPEN: anything
unreadable delivers. The lamp windows and the lamp mute FAIL CLOSED (dark): anything unreadable stays
quiet. Both directions are stated in the source (`src/quiet.rs:is_muted`, `src/focus.rs` module comment,
`src/channels/hue.rs:quiet_now`, `src/main.rs:ad_hoc_quiet`).

## Behaviors

### 1. A typed duration publishes one absolute expiry

Given the operator types `pns quiet 30m`

When the run can read a clock and write the state directory

Then `<state>/quiet-until` holds `now + 1800` followed by one newline, and stdout reads `pns: quiet for another 30 minutes`

- Success: `src/main.rs:quiet_mode` parses through `src/quiet.rs:parse_duration`, adds the seconds to
  `now_secs()` with `saturating_add`, and publishes through `src/main.rs:publish_state_line`. The
  reported line is read BACK off the file, never rendered from what the run intended
  (`src/main.rs:quiet_mode`), so the report cannot claim a mute that did not land. Pinned by
  `tests/dispatch.rs:a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it`.
- Failure sources: an unreadable clock; an unwritable state directory; a rename that fails.
- Fail direction: loud and non-zero. An unreadable clock prints
  `pns: state error (the clock cannot be read); the mute was not set`; a failed write prints
  `pns: state error (quiet-until could not be written: <error>); the mute was not set`. Neither arm
  claims "nothing is muted", because a mute set an hour ago may still stand behind the failure
  (`src/main.rs:quiet_mode`).
- Thresholds: `src/quiet.rs:MIN_SECONDS` is 1 and `src/quiet.rs:MAX_SECONDS` is 86400. `1s` and `24h` are
  accepted; `0s`, `0m`, `0h`, `25h`, `1441m`, `86401s` and `9223372036854775807h` are refused rather than
  clamped (`src/quiet.rs:a_duration_outside_the_permitted_range_is_refused_rather_than_clamped`). The
  multiply saturates so the ceiling refuses rather than an overflow deciding.
- Required side effects: exactly one state file, published by rename into the same directory, created
  with mode `0600` and re-narrowed on the open handle after creation (`src/main.rs:publish_state_line`).
- Forbidden side effects: no untimed form exists. There is no toggle and no indefinite mute
  (`src/main.rs:quiet_mode`, `src/quiet.rs:MAX_SECONDS`).
- Timeout and cancellation: Not applicable. No network, no subprocess, no wait.
- Idempotency and duplicates: re-typing a duration replaces the expiry outright. The pending file carries
  the process id so two concurrent runs cannot share one (`src/main.rs:publish_state_line`).
- Privacy: the file holds one integer. No wall-clock time and no local zone is ever rendered
  (`src/quiet.rs:the_report_counts_whole_minutes_up_so_a_live_mute_never_reads_as_zero`).
- Process ownership and cleanup: a failed rename unlinks the pending file before returning the error
  (`src/main.rs:publish_state_line`), pinned by
  `tests/dispatch.rs:a_publish_whose_rename_fails_leaves_no_pending_file_behind`.
- Compatibility contract: the state is ONE absolute epoch second, not a flag and not a start plus a
  duration, so every reader compares it against its own clock and a file left behind after the window is
  inert (`src/main.rs:QUIET_UNTIL`).

### 2. The bare command reports and mutes nothing

Given the operator types `pns quiet` with no argument

When the run completes

Then it prints the standing verdict and does not write the state file

- Success: the empty-argument arm of `src/main.rs:quiet_mode` does nothing, then prints
  `src/quiet.rs:status_line` over `read_quiet_expiry()` and `now_secs()`. The report's verdict IS
  `is_muted`'s, never re-derived (`src/quiet.rs:status_line`).
- Failure sources: a corrupt or unreadable state file, which is behavior 6.
- Fail direction: open. Every state `is_muted` answers false to reports `pns: not quiet`
  (`src/quiet.rs:the_report_says_not_quiet_for_every_state_the_predicate_calls_quiet`).
- Thresholds: minutes are rounded UP (`src/quiet.rs:minutes_left` uses `div_ceil(60)`), so 40 seconds
  left reads `pns: quiet for another 1 minute` and 61 seconds reads `pns: quiet for another 2 minutes`.
  The singular is used at exactly 1.
- Required side effects: none.
- Forbidden side effects: the report must not rewrite the mute it reports. Pinned on the modification
  time rather than on content, because a republish writes the same bytes in the same second
  (`tests/dispatch.rs:a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure read. Any number of runs answer the same.
- Privacy: the line quotes a minute count and nothing else.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: making the no-argument form the REPORT is what stops any invocation muting by
  accident (`src/main.rs:quiet_mode`).

### 3. `off` unlinks the state file

Given a mute is standing

When the operator types `pns quiet off`

Then the file is removed, stdout reads `pns: not quiet`, and the next event decorates again

- Success: `src/main.rs:quiet_mode` calls `std::fs::remove_file` on the state path and ignores the
  result. Pinned end to end by
  `tests/dispatch.rs:off_removes_the_state_file_and_the_next_event_decorates_again`, which asserts the
  banner fires on the very next event.
- Failure sources: the removal failing is deliberately swallowed; the report below then still reads the
  file and tells the truth.
- Fail direction: open. An absent file is the state every reader already treats as not muted
  (`src/main.rs:QUIET_UNTIL`).
- Thresholds: Not applicable.
- Required side effects: unlinking is ALSO the documented remedy for a file nothing can parse, and the
  complaint names it: `clear it with pns quiet off` (`src/main.rs:quiet_mode`,
  `src/quiet.rs:expiry_from_state`).
- Forbidden side effects: nothing is overwritten with a past expiry or an "off" flag. An absent file is
  the only spelling of not muted
  (`tests/dispatch.rs:off_removes_the_state_file_and_the_next_event_decorates_again`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: `off` over an already-absent file is silent and exits 0.
- Privacy: Not applicable.
- Process ownership and cleanup: this is the cleanup path.
- Compatibility contract: `off` is a single word and no other spelling is accepted;
  `pns quiet off please` is refused (behavior 4).

### 4. A mistyped mute is refused with exit 2 and writes nothing

Given the operator types a word the command does not serve

When `pns quiet` parses argv

Then it prints a refusal quoting what was typed, then the usage line, exits 2, and writes no state

- Success: `src/main.rs:quiet_mode` returns 2 from two arms: a duration `parse_duration` refused, and any
  argument list of two or more words. The usage is verbatim
  `pns: usage: pns quiet [<duration>|off]; duration is <count><s|m|h>, from 1s to 24h`
  (`src/main.rs:QUIET_USAGE`). Pinned over `tomorrow`, `30`, `off please` and `30m extra` by
  `tests/dispatch.rs:a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state`,
  which also asserts stdout is empty and no state file exists.
- Failure sources: none beyond the refusal itself.
- Fail direction: refuse. A typo an operator does not see is a mute they believe is on
  (`src/main.rs:quiet_mode`).
- Thresholds: a UNIT IS REQUIRED. `30` is refused because a bare number means minutes to one reader and
  seconds to the next. The refused shapes pinned in
  `src/quiet.rs:a_duration_that_is_not_a_count_and_a_unit_is_refused_by_what_was_typed` are `30`, the
  empty string, `1d`, `-5m`, ` 5m`, `05m`, `m` and `2 h`. The two refusal texts are
  `pns: quiet duration <typed> is not <count><s|m|h>` and
  `pns: quiet duration <typed> is outside 1s to 24h`, each quoting the typed word with Rust debug
  formatting.
- Required side effects: both lines go to stderr, never stdout.
- Forbidden side effects: no silent fallthrough to the report.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: the operator's own typed word is echoed back, which is their own input.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `pns quiet` is hand typed, so it is outside the always-exit-0 contract that
  covers the hook and notification paths
  (`tests/dispatch.rs:a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state`).
  NOT ESTABLISHED: what `pns quiet --help` does. Reading `src/main.rs:main`, the `quiet` dispatch at the
  top of `main` precedes the `is_help_flag` check in `is_producer_argv`, and `src/main.rs:quiet_mode` has
  no help arm, so `--help` falls into the single-argument duration arm and is refused with exit 2. No
  test in `tests/dispatch.rs` or `tests/hooks.rs` covers it, so the behavior is code-derived and
  unpinned.

### 5. A mute that could not be written reports the mute that still stands

Given a live mute is on disk and the state directory is not writable

When the operator types `pns quiet 30m`

Then stderr says the write failed, stdout reports the OLD mute, the file is untouched, and the exit code is 1

- Success: `src/main.rs:quiet_mode` sets `set_failed`, falls through to the report, reads the file back,
  and returns 1. Pinned by
  `tests/dispatch.rs:a_mute_that_could_not_be_written_reports_the_mute_that_still_stands`, which asserts
  stdout is `pns: quiet for another 60 minutes` after a failed `30m`.
- Failure sources: a read-only state directory; a directory standing at the file's path.
- Fail direction: loud, and truthful about the disk rather than about the intent. The measured defect
  behind this is quoted in the test: the failed write once said "nothing is muted" while a bare
  `pns quiet` a second later reported sixty minutes left.
- Thresholds: Not applicable.
- Required side effects: exit code 1 and one stderr line beginning
  `pns: state error (quiet-until could not be written: `. Pinned separately by
  `tests/dispatch.rs:a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind`, which
  exists because `return 1` mutated to `return 0` survived the whole suite.
- Forbidden side effects: no half-set mute is left on disk.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: the operating system's error text is included but never pinned verbatim in the tests, because
  a kernel may reword it
  (`tests/dispatch.rs:a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event`).
- Process ownership and cleanup: as behavior 1.
- Compatibility contract: exit 1 is a mute that did not land, exit 2 is a mistyped invocation, exit 0 is
  a report or a mute that landed.

### 6. Reading `quiet-until`: one epoch line, and everything else complains and delivers

Given a `quiet-until` file

When any reader takes the mute

Then only `<digits>` optionally followed by one trailing newline is an expiry, and every other shape complains once and reads as NOT muted

- Success: `src/quiet.rs:expiry_from_state` strips exactly the one trailing newline the publish writes
  and hands the rest to the crate's one numeric gate. `src/main.rs:read_quiet_expiry` separates the three
  cases: absent is silent, a read error complains, a parse failure complains.
- Failure sources: contents that are not one epoch second; a file that cannot be read at all; a directory
  at the path; bytes that are not UTF-8.
- Fail direction: OPEN, and deliberately the opposite of the lamp path. `src/quiet.rs:is_muted` answers
  false for a missing expiry, a missing clock, or both. The stated reason: a window failing closed costs
  one flash of a lamp, and a mute failing closed costs every notification including the card for a tool
  call the operator is blocked on, with no expiry and no way to discover it. Pinned by
  `src/quiet.rs:nothing_readable_is_not_muted_which_is_the_opposite_of_the_lights_window`,
  `tests/dispatch.rs:a_corrupt_state_file_delivers_everything_and_complains_once_per_event` and
  `tests/dispatch.rs:a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event`.
  Both binary tests run at the desk with `PNS_FORCE_PHONE=1`, which is the one row that earns BOTH
  decorations, so a mute reading true would be unmissable.
- Thresholds: half open. `is_muted(Some(1000), Some(999))` is true, `Some(1000)` against `Some(1000)` is
  false, and `Some(1001)` is false. The boundary second itself is the assertion, because both neighbours
  agree under either spelling (`src/quiet.rs:the_mute_ends_at_the_second_it_says_and_not_one_later`).
  Padding is refused rather than trimmed: `" 9223372036854775807\n"` once read as a live mute with
  153722867251113165 minutes left on it, which is why there is no `trim()`
  (`src/quiet.rs:a_state_file_holding_anything_else_is_a_complaint_naming_what_it_holds`).
- Required side effects: exactly one complaint per event, not one per reader. The parse complaint is
  `pns: state error (quiet-until is <contents>, not an expiry time); nothing is muted, clear it with pns quiet off`
  and the read complaint is
  `pns: state error (quiet-until could not be read: <error>); nothing is muted, clear it with pns quiet off`.
  Both name the file's own content or the error, because the operator has to find it to fix it.
- Forbidden side effects: an ABSENT file says nothing at all, which is the ordinary state
  (`tests/dispatch.rs:an_absent_state_file_is_the_ordinary_state_and_says_nothing` asserts stderr is
  empty).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the complaint repeats on the next event and stays repeating until the file
  is fixed, which is proportional: it IS broken until someone fixes it (`src/main.rs:read_quiet_expiry`).
  This is the OPPOSITE of the lights-quiet complaint, which is said once (behavior 19).
- Privacy: the complaint quotes the file's own bytes with debug formatting, and the file is a state file
  pns wrote.
- Process ownership and cleanup: no reader repairs the file. `pns quiet off` is the stated remedy.
- Compatibility contract: the verdict is `is_muted`'s and the report is derived from the same call, so a
  report and a behavior cannot disagree about whether a mute is on (`src/quiet.rs:status_line`).

### 7. The two engine mutes zero the plan and beat every producer override

Given an event whose plan called for a banner, a phone card and a pulse

When either the operator mute is live or a Focus named in `[focus] silence` is asserted

Then the delivery plan becomes `banner: false, phone_card: false, pulse: false`

- Success: `src/engine.rs:decide` applies the two mutes LAST, after `PNS_SKIP_PHONE` and
  `PNS_FORCE_PHONE` have already been arbitrated, through the single predicate
  `src/engine.rs:Overrides::silenced`, which is `self.muted || self.focus_active`. The struct is written
  as a FULL literal with no `..delivery`, so a future field of `DeliveryPlan` must state its own answer
  rather than inherit an unmuted one. Pinned by
  `src/engine.rs:a_muted_decision_keeps_the_durable_log_and_drops_every_decorative_leg`,
  `src/engine.rs:a_muted_decision_plans_no_pulse_even_for_a_long_running_event`, and end to end by
  `tests/dispatch.rs:a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge` and
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`.
- Failure sources: none. Both fields are booleans already decided at the composition root.
- Fail direction: whichever direction the READ took (behaviors 6 and 12). A mute that could not be read
  arrives here as `false`.
- Thresholds: Not applicable.
- Required side effects: the decision ring records `muted=` and `focus=` as TWO separate fields, not one
  (`src/decision_log.rs:line`), because "you have a `pns quiet` running" sends the operator somewhere
  completely different from "your Mac is in a Focus you told pns to respect". The Focus test asserts
  `muted=no focus=yes`, `force_phone=yes` and `plan=banner:no,card:no,pulse:no` in the same line.
- Forbidden side effects: NEITHER FIELD MAY EVER BE SET FROM THE ENVIRONMENT.
  `src/engine.rs:Overrides::from_env` writes `muted: false` and `focus_active: false` unconditionally, so
  no variable can mute the operator and none can end a mute they are still inside
  (`src/engine.rs:Overrides`). The mute is also never applied as a filter over `decision.legs`
  afterwards: which legs are decorative is routing's policy, and re-deriving it would be a second copy
  that drifts (`src/main.rs` composition root comment above `let overrides`).
- Timeout and cancellation: the decision runs on one clock reading taken once at the edge
  (`src/main.rs:run_event` uses `probes.now_secs()`), so an expiry crossed mid-run costs one event either
  way and never splits one decision across two readings.
- Idempotency and duplicates: one reading per event.
- Privacy: the ring records yes or no. The Focus MODE NAME is never written to the decision ring: no such
  field exists in `src/decision_log.rs:line`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: force is a producer's per-event opinion set in the environment; the mute is the
  operator's own typed instruction and a Focus is the same instruction with the operating system as its
  author. A mute any producer can override is not a mute (`src/engine.rs:decide`). Pinned by
  `src/engine.rs` (the forced-and-muted case) and by the `PNS_FORCE_PHONE=1` setup in
  `tests/dispatch.rs:focus_event`.

### 8. The durable log is never silenced

Given any event, muted or inside a named Focus

When the plan is arbitrated

Then the durable channel still receives it

- Success: `src/routing.rs:channel_plan` gates a presence-gated plugin on `delivery.phone_card` and a
  local plugin on `delivery.banner`, and everything else is `true` unconditionally. The durable log is
  not a field of `DeliveryPlan`, so it is exempt STRUCTURALLY rather than by a rule. Pinned by
  `tests/dispatch.rs:a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge` and
  `tests/hooks.rs:a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`.
- Failure sources: the channel's own transport, which is out of scope here.
- Fail direction: not applicable to silencing.
- Thresholds: Not applicable.
- Required side effects: the same holds for a NUDGE.
  `tests/hooks.rs:a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer` asserts one
  hermes delivery for the muted nudge while both decorations are zero.
- Forbidden side effects: nothing may add a durable-log check keyed on the mute.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: the mute changes nothing about what the log carries.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: this is what makes the mute LOSSLESS. The catch-up reads the durable stream to
  say what was missed
  (`tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`).

### 9. An approval is never suppressed, by any mechanism

Given an agent blocked on a permission prompt, with the operator away

When a `pns quiet` mute is live, or a Focus named in `[focus] silence` is asserted

Then the moshi forward still happens, byte for byte, and moshi's own exit code is still passed through

- Success: `src/main.rs:blocking_event` decides the forward through `src/main.rs:forward_to_moshi`, which
  reads ONLY the surface (`operator_surface(...) != Surface::Desk`) and never constructs a delivery plan.
  Nothing on `Overrides` can reach it. `src/main.rs:gate_mode`, the `pns gate <harness>-hook`
  pass-through, is the same: it calls `forward_to_moshi` with a throwaway probe set and runs no delivery
  plan at all. Pinned by two deliberately near-duplicate tests,
  `tests/hooks.rs:a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer` and
  `tests/hooks.rs:a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`, each
  asserting exit code 42 from the stubbed moshi and the payload arriving byte for byte at `moshi.stdin`.
  The Focus test states its own reason for existing: the mute test would keep passing on the day a Focus
  started suppressing approvals.
- Failure sources: moshi not installed (exit 0, "no opinion"); a payload too large to have arrived whole
  (`payload_is_whole`); the operator being at the desk, where the harness prompt in front of them already
  is the prompt.
- Fail direction: not forwarded means exit 0, which the harness reads as "no opinion, prompt as usual"
  (`src/main.rs:gate_mode`). Silence is never inferred from a mute.
- Thresholds: Not applicable.
- Required side effects: what IS silenced is pns's own duplicate notification about the block. The
  blocked event still runs `run_event`, so its banner, card and pulse are zeroed by behavior 7, and its
  durable log entry survives by behavior 8. The mute test states the distinction: "A muted operator who
  blocks on a permission prompt still gets the card and still answers it; only pns's own duplicate
  notification about that block goes quiet."
- Forbidden side effects: the exemption is structural and the tests say so about themselves. It breaks by
  MOVING code (building the forward from a delivery plan) rather than by editing a line, which is exactly
  what those two tests exist to catch.
- Timeout and cancellation: the wait on moshi is bounded at the shared seam by
  `src/main.rs:answer_within(child, submit_deadline())`, not at either caller, because `pi` and `omp`
  reach `gate_mode` with no pns hook in front of them (`src/main.rs:gate_mode`).
- Idempotency and duplicates: when the forward starts, `PNS_SKIP_PHONE=1` is set so pns's own phone leg
  is suppressed and the operator is not carded twice (`src/main.rs:blocking_event`). That suppression is
  applied to the forward that really STARTED, never to the intent to forward.
- Privacy: the payload is passed through unmodified.
- Process ownership and cleanup: the payload is written to the child off a spawned thread, which outlives
  a caller that stops waiting; it holds a pipe and a copy of the payload and the process is on its way
  out (`src/main.rs:spawn_moshi_hook`).
- Compatibility contract: the config template states it to the operator in the `[focus]` prose:
  "approvals never are, and neither is the durable log" (`src/config_text.rs`, the `focus` table). The
  NUDGE about an approval is a different thing and IS suppressible: it is informational, goes through the
  same `decide` call as any other event, and a suppressed nudge is LOST rather than queued
  (`src/main.rs:run_event`, the `attempt != Attempt::First` early return; pinned in the second half of
  `tests/hooks.rs:a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`).

### 10. A silenced event is journaled as a miss and cannot replay

Given an event silenced by the operator mute or a named Focus

When the decision is recorded

Then the event joins the missed-notification journal, and the same run flushes nothing

- Success: `src/missed_notifications.rs:was_missed` reads the ARBITRATED plan
  (`!plan.banner && !plan.phone_card`), so a zeroed plan is a miss by construction, and
  `src/missed_notifications.rs:should_replay` reads the same two fields, so a silenced run can never
  deliver the entry it just wrote. Neither function reads `overrides.muted` or `overrides.focus_active`.
  Pinned by `tests/dispatch.rs:a_muted_event_queues_its_own_miss_and_replays_nothing` (journal grows from
  2 to 3, no banner, exactly one durable delivery) and by
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`
  (exactly one miss queued).
- Failure sources: a journal path that cannot be read or is not a regular file, which costs the event
  nothing
  (`tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay`).
- Fail direction: the journal is best effort and fail quiet; it never changes a verdict.
- Thresholds: `plan.pulse` is DELIBERATELY NOT READ by `was_missed`: the lights are decoration and the
  quiet window suppresses only them (`src/missed_notifications.rs:was_missed`).
- Required side effects: three records are written REGARDLESS of the silencing. The activity ring records
  unconditionally, because the recap's window is every event, delivered or not. The news record is
  written whatever the delivery did, because a card that was suppressed or muted is exactly the news the
  unread lamp exists to carry. The blocked marker is written when the lamps are live
  (`src/main.rs:run_event`, the tail after `record_decision`).
- Forbidden side effects: a nudge and an observation write NO journal entry and NO activity-ring line,
  never claim the return moment, never trigger the replay and never pulse (`src/main.rs:run_event`,
  `attempt != Attempt::First`).
- Timeout and cancellation: the records are written after every channel and before the pulse, so a
  channel hanging to its deadline can cost the decision record if the process is killed. Stated as an
  accepted price at the record site (`src/main.rs:run_event`).
- Idempotency and duplicates: a miss and a replay are mutually exclusive by construction, because a run
  whose plan decorated nothing is exactly a run that journals
  (`src/missed_notifications.rs:should_replay`).
- Privacy: the journal holds the event's own fields.
- Process ownership and cleanup: the journal is bounded state that prunes itself, read back under
  `src/main.rs:RING_READ_MAX` (256 KiB).
- Compatibility contract: suppressing is strictly MORE informative than not suppressing. macOS was going
  to withhold the banner inside a Focus anyway, and posting it regardless would make pns believe it
  delivered, so the event would never be journaled: no banner AND no recap entry
  (`tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`).

### 11. macOS Focus is read per mode, never as "a Focus is on"

Given `[focus] silence = ["Sleep"]` and a Focus asserted in the Do Not Disturb store

When the composition root takes the reading

Then the event is silenced only if an ASSERTED mode matches a listed name or identifier

- Success: `src/main.rs:focus_now` reads `$HOME/Library/DoNotDisturb/DB/Assertions.json` and, beside it,
  `ModeConfigurations.json`. `src/focus.rs:active_modes` collects
  `data[0].storeAssertionRecords[].assertionDetails.assertionDetailsModeIdentifier` into a SET;
  `src/focus.rs:mode_names` builds identifier to display name off `mode.modeIdentifier` and `mode.name`;
  `src/focus.rs:silenced` matches each listed entry against either spelling. Pinned by
  `src/focus.rs:a_mode_the_config_names_by_its_display_name_is_silenced`,
  `src/focus.rs:a_raw_mode_identifier_is_accepted_for_a_mode_the_catalog_does_not_name` and end to end by
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`
  (which uses a CUSTOM mode whose identifier says nothing about its name, so a test using `Sleep` for
  both would pass with the catalog read deleted).
- Failure sources: the store's read; the catalog's read (behavior 13); a schema Apple changes.
- Fail direction: OPEN on every one. See behavior 12.
- Thresholds: an EMPTY `[focus] silence` list silences nothing and, more than that, opens no file at all:
  `src/main.rs:focus_now` returns early, so the default machine pays no input or output for a feature it
  did not ask for (`src/focus.rs:an_empty_list_is_the_feature_switched_off`). Matching is case
  insensitive and folded BOTH ways, because neither direction alone is enough: "Straße" lowercases to
  "straße" while "STRASSE" lowercases to "strasse", and upper-casing both agrees (`src/focus.rs:same`,
  `src/focus.rs:a_name_whose_lowercase_disagrees_with_itself_is_still_the_same_name`). It is case MAPPING
  and not full Unicode case folding or normalization: "İstanbul" against "istanbul" and a decomposed
  "Cafe\\u{301}" against a composed "café" both stay false, stated in `src/focus.rs:same`.
- Required side effects: an ENDED Focus is never an active one. macOS MOVES the record into
  `storeInvalidationRecords`, which nothing here reads
  (`src/focus.rs:an_ended_focus_in_the_invalidation_history_is_never_an_active_one`). Both documented
  spellings of "no Focus", the key absent and the key present as an empty array, answer an empty set
  (`src/focus.rs:both_documented_spellings_of_no_focus_name_no_mode`).
- Forbidden side effects: no timestamp is read. `header.timestamp` moves for writes that are not Focus
  transitions (cloud sync and record pruning, both measured), so a freshness gate on it would be a guess
  dressed as a check (`src/focus.rs:active_modes`). The store path is HOME-relative with NO environment
  hatch, so no producer can force the answer in either direction; the test seam is the sandbox's own
  `HOME` (`src/main.rs:focus_now`).
- Timeout and cancellation: the two files are read through `src/system.rs:readable_state_file` with the 256 KiB
  `RING_READ_MAX` ceiling and a regular-file check, so a FIFO at either path is refused rather than
  parking the event. The live store measures 6 KiB against that ceiling (`src/main.rs:focus_now`).
- Idempotency and duplicates: the reading is a SET, because the live store on this machine carries the
  SAME assertion record twice; uniqueness is not a property macOS maintains, so nothing downstream may
  count these (`src/focus.rs:active_modes`,
  `src/focus.rs:the_same_record_written_twice_is_one_mode_and_not_two`). Two modes asserted at once are
  both named (`src/focus.rs:every_mode_asserted_at_once_is_named_and_not_just_the_first`).
- Privacy: the mode name reaches no record. The decision ring writes `focus=yes` or `focus=no` and
  nothing more (`src/decision_log.rs:line`).
- Process ownership and cleanup: pns writes nothing to the store and never asserts or ends a Focus.
- Compatibility contract: per-mode policy is the whole point. MEASURED on this operator's machine, a
  Focus was asserted for 95% of one day, so a gate that fired on ANY active Focus would be a mute with no
  expiry (`src/focus.rs` module comment, restated in
  `tests/dispatch.rs:an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual`, which
  is also the positive control proving all three decorations really were on the plan).

### 12. An unreadable Focus store silences nothing, and the doctor is where it is said

Given `[focus] silence` names a mode

When the assertion store is absent, gated, not a regular file, or past the read ceiling

Then no notification is silenced, and `pns doctor` prints which of the states the machine is in

- Success: `src/main.rs` composition root reads
  `focus_now(&home, &focus_silence).is_ok_and(|reading| reading.silenced)`, so an `Err` is `false`.
  Pinned by `tests/dispatch.rs:a_focus_store_that_cannot_be_read_costs_no_notification_at_all` over two
  vehicles: no store at all, and a directory where `Assertions.json` should be. Both assert the banner
  fires and the journal stays empty.
- Failure sources: a macOS update moving the store or changing its schema; a Full Disk Access gate.
- Fail direction: OPEN, and it is the direction a reviewer should attack first. Failing closed would
  silence every banner, card and pulse from the morning after an upgrade with nothing on screen to say
  why; failing open costs one interruption the operator asked not to have (`src/focus.rs` module comment,
  `src/main.rs:focus_now`).
- Thresholds: 256 KiB (`src/main.rs:RING_READ_MAX`); the live store is 6 KiB.
- Required side effects: five distinct doctor sentences, from `src/main.rs:focus_line`. Verbatim:
  `pns doctor: focus awareness is off (no [focus] table names a mode to silence)`;
  `pns doctor: a macOS Focus you named is ON, so banners, cards and pulses are suppressed`;
  `pns doctor: no macOS Focus you named is active`;
  `pns doctor: no Focus database was found on this machine, so no Focus is being respected`; and
  `pns doctor: the Focus database could not be read, so Focus is being ignored (<kind>).`
  (`src/main.rs:FOCUS_UNREADABLE` plus the error kind). Four of the five are pinned by
  `tests/dispatch.rs:the_doctor_tells_the_truth_about_a_named_focus_in_every_state`, which exists because
  a review probe showed the ON sentence could lie without anything going red.
- Forbidden side effects: the Focus line must NOT move the doctor's exit code. A Focus being on is not a
  fault (`src/main.rs:doctor_mode`, the comment above the `focus_line` print).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the catalog's failure rides out on the SAME reading rather than being read
  a second time by the doctor, because a second read is a second moment and the doctor would then report
  on a file the decision never saw (`src/main.rs:FocusReading`).
- Privacy: only the `std::io::ErrorKind` is printed, never a path or the store's contents.
- Process ownership and cleanup: pns never repairs the store.
- Compatibility contract: the parser is TOTAL, and the accepted limit is stated rather than designed
  around. Bytes that are not JSON at all, and a schema change that leaves the file valid JSON, both read
  as "no Focus" rather than as an error, so only a failed READ reaches the unreadable sentence
  (`src/focus.rs:active_modes`, `src/main.rs:focus_line`). The doctor test builds a chmod-000 file
  precisely because `b"{}"` alone would take the fail-open path.
- ESTABLISHED GAP, not a NOT ESTABLISHED: `pns doctor` reports the Focus state but has NO line about the
  operator's own `pns quiet` mute. `src/main.rs:muted_now` and `src/main.rs:read_quiet_expiry` have
  exactly two call sites in the whole crate, the composition root (`src/main.rs`, building `Overrides`)
  and `src/main.rs:quiet_mode`'s own report; `pns quiet` with no argument is the only way to ask.

### 13. A mode catalog that cannot be read leaves NAME matching inert, and says so

Given `[focus] silence = ["Coding"]` and a `ModeConfigurations.json` that cannot be read

When the reading is taken

Then no display name resolves, only a raw `modeIdentifier` entry can still match, and the doctor adds a clause of its own

- Success: `src/focus.rs:mode_names` answers an EMPTY MAP for an unreadable or unparseable catalog, never
  an error, and `src/focus.rs:silenced` still matches identifier entries with no catalog at all.
  `src/main.rs:FocusReading::catalog` carries the `ErrorKind` out on the answer. Pinned by
  `src/focus.rs:a_catalog_nothing_can_read_resolves_no_names_at_all` and
  `tests/dispatch.rs:a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health`.
- Failure sources: permissions, a schema change, a missing file.
- Fail direction: open, and it silences LESS rather than more, which is the module's direction
  (`src/focus.rs:mode_names`).
- Thresholds: Not applicable.
- Required side effects: the doctor appends, verbatim,
  `; the mode catalog could not be read (<kind>), so no Focus NAME can match and only a raw modeIdentifier still would`
  to whichever state sentence it printed (`src/main.rs:focus_line`). It is said whenever the catalog
  failed and the feature is on, because WHICH entries are names is not decidable without the very file
  that failed.
- Forbidden side effects: the catalog's failure must NOT become an `Err` from `focus_now`. It is reported
  rather than errored, so an unreadable catalog never reads as an unreadable store
  (`src/main.rs:focus_now`).
- Timeout and cancellation: as behavior 11.
- Idempotency and duplicates: one read, one answer.
- Privacy: only the error kind is printed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the identifier escape hatch is the DESIGN and not an oversight. The raw
  `modeIdentifier` is the only handle a mode the catalog does not name has, so it cannot be made to
  depend on the catalog without deleting the escape hatch that exists for exactly that case
  (`src/focus.rs:silenced`). The consequence, stated in the source: a catalog that is absent, gated or
  garbled leaves identifier entries working and NAME entries inert.

### 14. Quiet hours and the quiet window are ONE mechanism, under two names

Given the config key `[plugins.hue] quiet_hours = "22:00-07:00"`

When the pulse gate runs

Then that string is parsed once into a `QuietWindow` and judged by one predicate

- Success: this is not two mechanisms. `src/channels/hue.rs:quiet_window` reads the key literally named
  `quiet_hours` from the `[plugins.hue]` settings table and returns
  `Result<Option<QuietWindow>, String>`; `src/channels/hue.rs:quiet_now` is the only predicate over it;
  `src/channels/hue.rs:QuietWindow::ends_at` is the one field a bare `pns lights quiet` reads. There is
  no separate "quiet hours" state, file, or code path anywhere in the crate. The two names are the CONFIG
  KEY (`quiet_hours`) and the parsed VALUE and its type (`QuietWindow`, "the window" throughout the
  source prose).
- Failure sources: a value that is not `HH:MM-HH:MM`; a value of the wrong TOML type.
- Fail direction: a refusal, never a silent no-window, because an operator who asked for quiet hours and
  mistyped them would otherwise be flashed at 3am and told nothing (`src/channels/hue.rs:quiet_window`).
  The refusal text is verbatim
  `pns: config error (hue.quiet_hours is <offender>, not a HH:MM-HH:MM window); no pulse`, where the
  offender is the debug-quoted string or the TOML type name (`src/channels/hue.rs:quiet_hours_refusal`,
  pinned by `src/channels/hue.rs:a_quiet_hours_that_is_not_two_clock_readings_is_refused_by_name` and
  `src/channels/hue.rs:a_quiet_hours_of_the_wrong_type_is_refused_by_name_and_by_type`). An EMPTY string
  is absent rather than a refusal, the rule the bridge and key beside it already follow
  (`src/channels/hue.rs:a_blanked_quiet_hours_is_no_window_rather_than_a_refusal`).
- Thresholds: `src/channels/hue.rs:minute_of_day` requires exactly two ASCII digits per field, hours
  below 24 and minutes below 60, so `2:00-07:00`, `24:00-07:00`, `22:60-07:00`, `10pm-7am` and a trailing
  space are all refused.
- Required side effects: the refusal is printed once and only where a pulse was actually due, inside the
  `if` that already earned one. An event that earned no pulse says nothing about the window
  (`src/main.rs:fire_pulse_unless_quiet`, pinned by
  `tests/dispatch.rs:a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`, which asserts
  exactly one occurrence of `hue.quiet_hours` and that the room stayed dark).
- Forbidden side effects: on a machine WITH a `[lights]` table, `quiet_hours` is no longer a rung of the
  routed chain at all. A typo in the house key cannot darken a routed lamp
  (`tests/dispatch.rs:a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`). Its only two
  remaining jobs are the no-map pulse gate and the schedule a bare `pns lights quiet` reads.
- Timeout and cancellation: Not applicable to the gate. The pulse behind it dials under
  `src/channels/hue.rs:BRIDGE_DEADLINE` (10 seconds).
- Idempotency and duplicates: the clock is read FRESH at the gate rather than at the run's start, because
  the legs above dial the network under their own deadlines and a run can cross into a window between
  starting and reaching the moment a lamp would light. HONEST LIMIT, stated in the source: no suite pins
  the freshness, because a test's clock does not advance mid-run (`src/main.rs:fire_pulse_unless_quiet`).
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the window mutes the LIGHTS and nothing else. The card and the durable log are
  how a long command reports at any hour
  (`tests/dispatch.rs:a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`
  asserts both other legs still dispatch while the room stays dark).

### 15. Window boundaries: start inclusive, end exclusive, wrapping, and an unreadable clock

Given a window and a minute of the local day

When `quiet_now` judges it

Then the start minute is inside, the end minute is outside, a wrapping window is the two ends of the day joined, and a start equal to its end is never quiet

- Success: `src/channels/hue.rs:quiet_now`. The minute of the local day comes from
  `src/system.rs:local_minutes_since_midnight`, which calls `localtime_r` and range-checks the result
  below 1440.
- Failure sources: `localtime_r` returning null; an epoch past `time_t`; arithmetic overflow. All answer
  `None`.
- Fail direction: CLOSED, and deliberately the opposite of the operator mute. A CONFIGURED window with no
  clock reading is QUIET: a missed pulse costs nothing and a flash at 3am is what the window was set to
  prevent. NO WINDOW is never quiet whatever the clock says, so an operator who configured no quiet hours
  keeps the pulse an unreadable clock would otherwise cost them (`src/channels/hue.rs:quiet_now`,
  `src/channels/hue.rs:a_clock_this_machine_cannot_read_is_treated_as_inside_the_window`).
- Thresholds: for 22:00 to 23:00 (minutes 1320 to 1380): 1319 is loud, 1320 is quiet, 1379 is quiet, 1380
  is loud on the dot, so two adjacent windows cannot overlap
  (`src/channels/hue.rs:a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end`). For a
  wrapping 22:00 to 07:00 (1320 to 420): 1319 loud, 1320 quiet, 1439 quiet, 0 quiet, 419 quiet, 420 loud,
  720 loud
  (`src/channels/hue.rs:a_window_whose_start_is_after_its_end_is_quiet_on_both_sides_of_midnight`). A
  window whose start equals its end is loud at every one of the 1440 minutes, and that is deliberately
  not a special case: the all-day mute already exists as `enabled = false`
  (`src/channels/hue.rs:a_window_whose_start_equals_its_end_is_never_quiet`).
- Required side effects: none.
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure function of two values.
- Privacy: no wall-clock time is rendered to the operator anywhere on the mute paths.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: local wall clock, start inclusive, end exclusive, may wrap midnight. This is
  what the shipped config template promises for both `quiet_hours` and `dim_window`
  (`src/config_text.rs`, the `quiet_hours` key prose and `src/config_text.rs:ROUTING`).

### 16. The hand-run pulse and the doctor are exempt from the quiet window

Given a live quiet window

When the operator runs `pns pulse <exit-code>` or `pns doctor`

Then the bridge is dialled anyway

- Success: the gate lives at the EVENT PATH's call site in `src/main.rs:fire_pulse_unless_quiet`, not
  inside `src/main.rs:fire_pulse`, which both the hand-run pulse and the doctor reach directly. Pinned by
  `tests/dispatch.rs:the_hand_run_pulse_reaches_the_bridge_inside_the_quiet_window` and
  `tests/dispatch.rs:the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`.
- Failure sources: no bridge or key in the config, which is a silent exit 0 for `pulse`.
- Fail direction: not applicable.
- Thresholds: Not applicable.
- Required side effects: the exemption is STRUCTURAL, which is what keeps the window checkable by hand
  exactly while it is on (`src/main.rs:pulse_mode` doc comment).
- Forbidden side effects: nothing in this repo calls `pns pulse`. The tiers that used to are part of the
  event plan now, which is what stopped the tier being decided twice (`src/main.rs:pulse_mode`).
- Timeout and cancellation: `BRIDGE_DEADLINE` is 10 seconds per call. The doctor test asserts the run
  finished rather than parking on the bridge.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `pns doctor` also bypasses the OPERATOR mute, the desk surface, and both phone
  overrides: every channel receives, and the run leaves nothing behind in the state directory
  (`tests/dispatch.rs:the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`,
  which arms a real `quiet-until` and asserts `quiet-until` is still the only file there afterwards).

### 17. The dim window is per lamp and per behavior, with three answers

Given a lamp whose declaration (or its room's or zone's) states `dim_window` and `dim_behaviours`

When a behavior is about to be shown at some minute of the local day

Then the answer is `Full` outside the window, `Dimmed` inside it for a listed behavior, and `Dark` inside it for one that is not listed

- Success: `src/channels/hue.rs:dim_showing` over `src/channels/hue.rs:DimWindow`. THREE ANSWERS RATHER
  THAN A BOOLEAN, because the caller has to know which body to write. Pinned by
  `src/channels/hue.rs:inside_a_window_an_enabled_behaviour_runs_dim_and_one_that_is_not_is_suppressed`.
- Failure sources: a `dim_window` value that is not `HH:MM-HH:MM`; a lamp two zone declarations both
  answer for.
- Fail direction: CLOSED FOR THAT LAMP ALONE. A window nobody can parse leaves the lamp DARK and says
  which lamp, because an operator who asked for a dim window and mistyped it would otherwise be flashed
  at 3am and told nothing. The refusal is verbatim
  `lights: `<lamp>` has dim_window <stated>, which is not a HH:MM-HH:MM window; that lamp stays dark`,
  prefixed `pns ` by `src/main.rs:routing_complaints` (`src/channels/hue.rs:window_refusal`,
  `src/channels/hue.rs:a_dim_window_nobody_can_parse_leaves_that_lamp_dark_and_says_which_lamp`). An
  unreadable clock is INSIDE the window, through `quiet_now`'s own rule
  (`src/channels/hue.rs:a_clock_this_machine_cannot_read_is_treated_as_inside_the_window`).
- Thresholds: the same minute rules as behavior 15, since `DimWindow` holds a `QuietWindow`. A window
  with an EMPTY `dim_behaviours` list suppresses EVERY behavior for its duration, which is the bedroom
  rule and needs no mode of its own
  (`src/channels/hue.rs:a_window_with_nothing_enabled_suppresses_every_behaviour_and_needs_no_mode`). A
  lamp with NO window is untouched at every hour, which is what makes the whole feature opt in.
- Required side effects: the enables RIDE the window, so a declaration either states when the lamp is
  quiet and what it does then, or says nothing about quiet hours at all. Two separately inherited halves
  would let a lamp take its room's window and a zone's enables (`src/channels/hue.rs:DimWindow`). A lamp
  that states behaviours and no window keeps inheriting its room's window (`src/config_text.rs:ROUTING`).
- Forbidden side effects: the dim window costs no LEG. An event fired at an hour every lamp is asleep
  still resolves the map, still cards and still logs
  (`tests/dispatch.rs:an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg`). That
  test also states honestly what it can prove: its spy is a plain TCP listener, so the "no lamp is
  WRITTEN to" half is pinned in the unit tests over `dim_showing` and `pulse_render`, not there.
- Timeout and cancellation: the map is resolved on the bridge under `BRIDGE_DEADLINE` on the event path
  and under `src/main.rs:tick_bridge_deadline` (refresh interval divided by 5, at least 1 second) on the
  tick.
- Idempotency and duplicates: the minute is read once per pass and handed down as
  `src/channels/hue.rs:Reading::minutes_now`.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: this is a per-lamp RENDERING rule, and it is separate from the quiet window
  even though it reuses the `QuietWindow` type and the `quiet_now` predicate. It has its own key at its
  own scope, it answers three states instead of two, and the routed path never consults `quiet_hours`
  (`tests/dispatch.rs:a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`). A bare
  `pns lights quiet` reads `quiet_hours` and NEVER any room's `dim_window`, because a mute typed at
  bedtime is about the operator's night and a room's window is a rendering rule that has nothing to say
  about how long a by-hand silence should last (`src/lights.rs:bare_mute_secs`).

### 18. `pns lights quiet <place> [<duration>|off]` mutes one place's lamps and nothing else

Given the operator types `pns lights quiet "3F - Studio" 2h`

When the place is a name a mute can enforce

Then `lights-quiet` gains a line, the report prints what is quiet, and no other channel is affected

- Success: `src/main.rs:lights_quiet` resolves the vocabulary, parses through
  `src/lights.rs:quiet_command`, rebuilds through `src/lights.rs:muted_after`, publishes through
  `src/main.rs:publish_muted`, and prints `src/lights.rs:muted_report`. Pinned end to end by
  `tests/dispatch.rs:an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`, which
  asserts the CARD still reaches the phone when away and the BANNER still runs at the desk, each against
  an unmuted control in the same world.
- Failure sources: a place no declaration and no bridge listing names; a bare mute with no `quiet_hours`
  set; a clock that cannot be read; an unwritable state directory; the 32-line cap.
- Fail direction: FAIL OPEN at every turn for the COMMAND, which is `quiet.rs`'s direction rather than
  the window's: a state file nobody can parse mutes NOTHING at this command and says so, because a lights
  mute the operator cannot see is worse than a lamp that flashed (`src/main.rs:lights_quiet`). The LAMP
  PATH takes the opposite direction, which is behavior 19.
- Thresholds: durations are `src/quiet.rs:parse_duration`'s, refusal and all, so 1s to 24h with no second
  set of bounds (`src/lights.rs:quiet_command`). The cap is `src/lights.rs:MAX_MUTED_PLACES` = 32, and a
  mute past it is REFUSED rather than written, because publishing one more line would have
  `muted_entries` reject the whole file at the next event and cancel every mute on the machine silently:
  `pns: lights quiet: 32 places are already quiet, which is every line lights-quiet keeps; the mute was not set, and `pns
  lights quiet <place> off` ends one` (`src/lights.rs:muted_after`). A bare mute runs from now to the
  next end of `quiet_hours`, and NOW AT THE END MINUTE IS A WHOLE DAY rather than nothing, since a mute
  of zero seconds is not a mute (`src/lights.rs:bare_mute_secs`,
  `src/lights.rs:how_long_a_bare_mute_runs_is_the_minutes_from_now_to_the_windows_end`).
- Required side effects: a place NO claim names is REFUSED rather than stored, because a mute would
  otherwise be a line in a file nothing will ever match while the lamp goes on flashing:
  `pns: lights quiet: <place> is no lamp, room or zone this can quiet; a mute reaches <names>`, or
  `...; this config claims no lamp at all, so there is nothing a mute could reach` when the vocabulary is
  empty (`src/lights.rs:unmutable`). The vocabulary is the config's declarations PLUS the bridge's own
  lamps, rooms and zones (`src/channels/hue.rs:mutable_names`), and the bridge is dialled only on the
  MISS path (`src/main.rs:asks_the_bridge`), so muting a room the config routes costs no network at all.
  A bare mute with no schedule is refused rather than guessed:
  `pns: lights quiet: a bare mute lasts until your quiet hours end, and `[plugins.hue]
  quiet_hours` states none; give a duration instead, or set that key` (`src/lights.rs:NO_SCHEDULE`).
- Forbidden side effects: `off` is allowed over ANY name, because it can only remove; a place muted
  yesterday and dropped from the config today would otherwise be a mute nothing could clear
  (`src/lights.rs:quiet_command`, `src/main.rs:asks_the_bridge`). A failed publish reports NOTHING on
  stdout and exits 1, because `kept` is what the file WOULD have held and printing it would describe a
  house that does not exist
  (`tests/dispatch.rs:a_lights_quiet_write_that_failed_reports_the_disk_and_not_the_list_it_built`, which
  asserts empty stdout). Any other arity is a refusal, never a silent fallthrough to the report:
  `pns: lights quiet takes a place, optionally with a duration or off, or nothing at all`, followed by
  `pns: usage: pns lights tick | pns lights quiet [<place> [<duration>|off]]`
  (`src/lights.rs:quiet_command`, `src/main.rs:LIGHTS_USAGE`), exit 2.
- Timeout and cancellation: the bridge inventory read uses `src/channels/hue.rs:TYPED_COMMAND_DEADLINE`
  (1 second per call, three calls), which is the HUMAN'S deadline rather than the transport's: three
  calls at the transport's ten seconds is half a minute before a mute typed at bedtime says anything. A
  bridge that answers nothing is not a refusal; the vocabulary narrows to the declarations
  (`src/main.rs:bridge_inventory`).
- Idempotency and duplicates: re-muting a place replaces its entry (`muted_after` filters the place out
  before pushing), and expired entries are dropped as it goes past, which is what keeps a machine that
  mutes a different room every night off the cap. A clock nobody can read KEEPS every other entry, so one
  broken reading cannot erase mutes the operator can still see (`src/lights.rs:muted_after`). The
  read-modify-write race is real and ACCEPTED: this is hand typed, so racing means an operator typing two
  commands in the same second, and a lock between two interactive commands would be a mechanism with no
  reader (`src/main.rs:lights_quiet`).
- Privacy: the file holds the operator's own typed place names at mode `0600`. It is ONE file with one
  line per place rather than a file per place, and that is a path-safety decision: a place is a room name
  the operator typed, spaces and all, and nothing in this crate turns typed text into a filename unless a
  predicate already vouches for it (`src/main.rs:LIGHTS_QUIET`, `src/lights.rs:Muted`).
- Process ownership and cleanup: an EMPTY list removes the file rather than writing an empty one, which
  is what keeps the reader's refusal of an empty file honest (`src/main.rs:publish_muted`).
- Compatibility contract: LIGHTS ONLY, and stated in three places. The two mutes share a duration parser
  and NOTHING ELSE, and neither reads the other's file (`src/main.rs:lights_quiet`,
  `src/config_text.rs:TRAILER`). The scope test says why it matters: a mute that quietly took the card
  with it would be an approval the operator is blocked on, silenced by a command about a bedroom lamp
  (`tests/dispatch.rs:an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`). The
  expiry is written from THIS RUN'S clock, pinned on both sides, because a run measuring from a fixed
  epoch would publish an entry already expired in 1970 while printing "quiet for 60m"
  (`tests/dispatch.rs:a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch`).

### 19. The lamp path reads `lights-quiet` fail-dark, and says so once

Given a `lights-quiet` file that cannot be read or cannot be parsed, or a run with no clock

When a lamp is about to be written

Then EVERY lamp is treated as muted, and the reason is said once rather than on every event

- Success: `src/main.rs:muted_state` is the ONE reader for both consumers, and `src/main.rs:ad_hoc_quiet`
  turns any complaint, and a missing clock, into `src/channels/hue.rs:Muting::Everything`.
  `src/channels/hue.rs:muted_now` answers true for every lamp under `Everything`, and otherwise matches
  the lamp's own name, its room and each of its zones against the muted places.
- Failure sources: a file that is unreadable, not UTF-8, a directory in its place, holding a line that is
  not `<epoch> <place>`, or holding more than 32 lines.
- Fail direction: DARK, and it is the OPPOSITE of the same file's reading in the COMMAND. `ad_hoc_quiet`
  mutes everything because a house with every lamp loud is the 3am the mute was armed to prevent;
  `lights_quiet`, with an operator standing in front of it, prints the complaint and rebuilds from an
  empty list because they are losing what the file held and get to see that rather than a silent repair
  (`src/lights.rs:muted_entries` doc comment names both callers).
- Thresholds: a line is `<epoch> <place>` and nothing else, with the only leniency the ONE trailing
  newline the publish writes. The place is the rest of the line VERBATIM, spaces and all, because a room
  is called `3F - Master Bedroom`; what it may not be is empty or padded at either end
  (`src/lights.rs:muted_entry`). Over 32 lines the whole file is refused (`src/lights.rs:muted_entries`).
  The report rounds minutes up through `src/quiet.rs:minutes_left`, the same rule the operator mute uses,
  so a room quiet for 40 more seconds never reads as zero (`src/lights.rs:muted_report`).
- Required side effects: exactly two operator-facing sentences.
  `pns: state error (lights-quiet holds <what>); nothing is quiet, and the next pns lights quiet write replaces the file`
  for anything malformed (`src/lights.rs:quiet_state_error`), and
  `pns: state error (lights-quiet could not be read: <error>); nothing is quiet` for a failed read
  (`src/main.rs:muted_state`). On no clock the reason is
  `pns lights: the clock cannot be read, so no mute can be judged live; every lamp is quiet until it can`
  (`src/lights.rs:NO_CLOCK_FOR_THE_MUTE`), and the REPORT prints the same sentence rather than "nothing
  is quiet", which would tell the operator the opposite of what every lamp is about to do
  (`src/lights.rs:a_clock_that_will_not_answer_reports_the_reason_never_nothing_is_quiet`).
- Forbidden side effects: the complaint must not repeat per event. `src/main.rs:say_lights_once` keeps a
  memory in `<state>/lights-quiet-said` for the event path and `<state>/lights-said` for the tick, two
  files rather than one so neither forgets the other's line and repeats it. Pinned by
  `tests/dispatch.rs:a_corrupt_lights_quiet_is_complained_about_once_rather_than_on_every_event`, which
  asserts the first event says it and the second does not. This is the OPPOSITE of the operator mute's
  once-per-event complaint (behavior 6).
- Timeout and cancellation: the mute is a RENDER FILTER at the per-lamp decision, so the map is still
  resolved and every lamp under the muted name is then skipped. That costs three bridge GETs for the
  length of the mute, and it is what keeps ONE answer to "is this lamp muted"; a second, config-only copy
  of the question upstream of the listing is how a report and a lamp come to disagree
  (`tests/dispatch.rs:an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`).
- Idempotency and duplicates: the state repairs itself on the next `pns lights quiet` write, which
  republishes the whole file (`src/main.rs:ad_hoc_quiet`).
- Privacy: as behavior 18.
- Process ownership and cleanup: the event path and the tick both read; only the command writes.
- Compatibility contract: `Muting::Everything` is a FAIL DIRECTION and not a command anybody types. There
  is no way to mute every lamp on purpose through this command (`src/channels/hue.rs:Muting`).

### 20. The tick's sustained breath answers to the lamp mutes only

Given a lamp holding a state (a wait, a streak, unread news) written by `pns lights tick`

When the operator's own `pns quiet` mute is live, or a named Focus is asserted

Then the breath is UNAFFECTED, and only `pns lights quiet` and the dim window can quiet it

- Success: `src/main.rs:lights_tick` reads `ad_hoc_quiet` and `dim_showing` and nothing else. It never
  calls `src/main.rs:muted_now` and never calls `src/main.rs:focus_now`: those two functions have exactly
  three call sites between them in the whole crate, the composition root building `Overrides`
  (`src/main.rs`, both), `src/main.rs:quiet_mode`'s report, and `src/main.rs:focus_line` for the doctor.
  The composition root states the contract in prose: "That reading takes `pns lights quiet` and each
  room's own dim window, and never this event's own silence or a macOS Focus: those gate the flash and
  the cards, not the sustained breath" (`src/main.rs:run_event`, above the `blocked_lamp` gate).
- Failure sources: a held record that cannot be read; an unreachable bridge; a lock another tick holds.
- Fail direction: a held record of `None` is EVERY LAMP HELD on the event path's pulse gate, which is
  fail dark on the one gate that decides whether a blink writes over a breath
  (`src/main.rs:run_pulse_writes`). On the tick, an unreadable record names nothing to clear and the pass
  goes on, because the tick is the record's only writer and publishing a fresh record is what repairs it;
  the residual is stated: a lamp held under a name this run could not read stays lit until the repaired
  record names it again or the operator's next return clears it (`src/main.rs:lights_tick`).
- Thresholds: `src/main.rs:tick_bridge_deadline` is the refresh interval divided by 5, at least 1 second.
- Required side effects: a SILENCED event still arms the breath. `update_blocked_marker` and
  `record_news` are written in the tail of `src/main.rs:run_event` gated on the lamp switches
  (`lamps_live`) and on `attempt == Attempt::First`, never on `overrides.silenced()`; the news record's
  own comment says a card that was suppressed or muted is exactly the news the unread lamp exists to
  carry. `src/main.rs:register_lights_tick` also refreshes the tick's lease for a silenced event, and
  takes the LONGER journalled lease because `was_missed` is true.
- Forbidden side effects: the event path's own blocked FLASH does respect the silence, through the same
  predicate arbitration uses rather than a second copy of it:
  `let blocked_lamp = behaviour == Behaviour::Blocked && !overrides.silenced();`
  (`src/main.rs:run_event`), pinned by
  `tests/dispatch.rs:the_operators_own_mute_takes_the_blocked_lamp_with_everything_else` against an
  unmuted control. That flash is not what holds the lamp blue: `pulse_render` answers `None` for every
  held behaviour, so the flash fires once at the moment the wait begins and does nothing after.
- Timeout and cancellation: one tick drives the house at a time, through a lock claimed before the
  resolve (`src/main.rs:run_tick_writes`, `LIGHTS_TICK_LOCK`).
- Idempotency and duplicates: every state is re-derived from scratch on each tick; nothing is carried
  between runs except what is on disk (`src/main.rs:lights_tick`).
- Privacy: Not applicable beyond behaviors 18 and 19.
- Process ownership and cleanup: the tick runs under the daemon and exits 0 on every path, silent on
  every happy one, because a line per tick would be a log the rotation job then rotates a real log out of
  (`src/main.rs:lights_tick`).
- Compatibility contract: NOT ESTABLISHED: no test in `tests/dispatch.rs`, `tests/hooks.rs`,
  `tests/daemon.rs` or `tests/native.rs` runs `pns lights tick` with a live `quiet-until` or a written
  Focus store to pin that the breath survives them. Searched by test name for `tick` combined with
  `quiet`, `mute` and `focus`, and by reading every reader of `muted_now` and `focus_now`. The property
  holds by construction (the tick never calls either function) and is asserted in prose at
  `src/main.rs:run_event`, but nothing goes red if a future edit wires the operator mute into
  `lights_tick`.

## Gaps

- NOT ESTABLISHED: `pns quiet --help`. Code-derived exit 2 (behavior 4); no test covers it.
- NOT ESTABLISHED: the tick's independence from the operator mute and from Focus is unpinned by any test
  (behavior 20).
- NOT ESTABLISHED, and named in the source itself: the freshness of the clock read at
  `src/main.rs:fire_pulse_unless_quiet` ("HONEST LIMIT: no suite pins the freshness, because a test's
  clock does not advance mid-run"), behavior 14.
- Established rather than a gap, but worth naming: `pns doctor` reports the Focus state in five sentences
  and reports NOTHING about the operator's own `pns quiet` mute. Bare `pns quiet` is the only way to ask
  (behavior 12).
