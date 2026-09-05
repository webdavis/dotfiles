# pns behavioral specification

Recorded 2026-09-05 against `origin/main` at `7ee33504`, from the crate at `dot_local/share/pns` and
its callers in this repository. This is the repository-level statement inventory the refactor plan at
`docs/superpowers/plans/2026-09-05-pns-refactor-plan.md` moves code against. It states what pns does
today, not what it should do. Where the code and this document disagree, the code is right and the
document is the defect.

## How to read it

Every statement is one line of observable behavior with two citations underneath it:

- `Source:` the file and line range that implements it, with the symbol name beside the number so the
  statement survives a line drift. Every range was read on 2026-09-05; `main.rs` is under active
  refactor, so grep for the symbol when a number has moved.
- `Pin:` the test whose failure would announce a change, named as the leaf test name with its file and
  line. A second pin is written `also`. A statement no test pins says `UNPINNED` and names what was
  looked for. A move of the code behind an UNPINNED statement writes the missing test first, against
  the code where it lives today, per `dot_local/share/pns/docs/specs/unpinned-behaviors.md`.

Scenario prose, thresholds one step either side, and the reasoning behind each rule live in the
crate's own specifications under `dot_local/share/pns/docs/specs/` (seventeen areas, written
2026-09-02, cited by file and symbol). This document does not repeat them. It is the flat inventory
those specifications lack: one numbered statement per behavior, organized by the charter's
vocabulary, each pinned or marked unpinned, so the plan can name exactly which statements a step
moves and which tests carry them.

Vocabulary is the code's own, checked against `src/` (see `docs/specs/glossary.md` in the crate):
producer, event, attempt (`First`, `Nudge`, `Observation`), decision, overrides, surface (`Desk`,
`Mobile`, `Away`), visibility (`Visible`, `Hidden`, `Unknown`), delivery plan, leg, report mode (wire
words `async` and `sync`), decorative, roster and core, route, delivery (`Silent`, `Delivered`,
`Failed`, `Unlaunched`), decision ring, journal, activity ring, claim, lease, marker, ring lock, job,
spool, tick, nag, recap, doctor, unread, held, phase, streak, house, quiet window, dim window, muting.
`signal` names nothing in the code today and is reserved for the protocol crate.

Counts, computed over this document by `grep`, are at the end.

## 1. Entry points and exit codes

### 1.1 Dispatch

S001. Argv is read once, lossily (`args_os` plus `to_string_lossy`), and every later reader works off
      that one vector; a non-UTF-8 argument becomes a replacement-character token and never a panic.
      Source: `src/main.rs:48-60 main`.
      Pin: `a_non_unicode_argument_never_breaks_the_exit_zero_edge`
           at tests/dispatch.rs:413

S002. The first argv word is matched in this fixed order before the producer check runs: `pulse`,
      `home`, `quiet`, `doctor`, `recap`, `daemon`, `lights`, `presence`, `loop`, `nag`, `setup`, a
      `<name>-hook` word, `gate`, `hook`. `setup` therefore sits above every config load.
      Source: `src/main.rs:63-144 main`.
      Pin: `the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision`
           at tests/hooks.rs:2329
      also `the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone`
           at tests/dispatch.rs:624

S003. A first word that names no command and carries no producer flag and no help spelling prints the
      whole `USAGE` text to stderr and exits 2, having loaded no config, spawned nothing and created
      no state directory. `--wat`, `-`, `--`, `--help=x`, `--HELP`, `-help`, `--agent=claude`, `stpo`
      and the literal empty word are all refused this way.
      Source: `src/main.rs:156-159 main`, `src/main.rs:209-214 is_producer_argv`.
      Pin: `a_word_that_names_no_command_is_refused_and_delivers_nothing`
           at tests/dispatch.rs:470
      also `a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`
           at tests/dispatch.rs:510
      also `a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it`
           at tests/dispatch.rs:538

S004. An empty argv is the valid empty event: it is decided, rendered and delivered, with `pns`,
      `done` and `done` filled in for the missing agent, state and body.
      Source: `src/main.rs:209-214 is_producer_argv`, `src/render.rs:15-23 title`,
      `src/render.rs:42-53 message`.
      Pin: `a_bare_invocation_is_still_the_empty_event_the_contract_calls_valid`
           at tests/dispatch.rs:685

S005. `USAGE` is one text, printed on stdout for help and on stderr for a refusal, and it lists
      sixteen entry lines plus the ten producer flags.
      Source: `src/main.rs:166-190 USAGE`.
      Pin: `the_help_flag_prints_the_usage_and_reaches_nothing_at_all`
           at tests/dispatch.rs:445

S006. Every producer event path exits 0, whatever a channel, a config, a probe or a state write did.
      The shared test runner asserts `status.success()` on every call, so every dispatch test pins it.
      Source: `src/main.rs:160 main` (the `event_mode` return), `tests/support/mod.rs run`.
      Pin: `a_state_directory_that_cannot_be_written_costs_the_event_nothing`
           at tests/dispatch.rs:3704
      also `a_non_unicode_argument_never_breaks_the_exit_zero_edge`
           at tests/dispatch.rs:413

S007. A subcommand word wins over producer flags that follow it (`pns pulse --agent x` is a pulse
      invocation refused with exit 2, not an event).
      Source: `src/main.rs:63-144 main` (order), `src/main.rs:3952-3993 pulse_mode`.
      Pin: UNPINNED. No test drives a subcommand word carrying producer flags; the nearest is the
      doctor's extra-word refusal, which uses a bare word.

### 1.2 The producer flags

S008. Seven flags take a value (`--agent`, `--state`, `--project`, `--branch`, `--detail`, `--pane`,
      `--channel`) and three are bare (`--long-running`, `--local-only`, `--remote-only`); `--help`
      and `-h` are in neither list on purpose.
      Source: `src/args.rs:44-52 VALUE_FLAGS`, `src/args.rs:58 BARE_FLAGS`, `src/args.rs:77
      is_help_flag`.
      Pin: `every_value_flag_lands_in_its_field`
           at src/args.rs:134
      also `help_in_flag_position_is_recognized_wherever_it_sits`
           at src/args.rs:228

S009. A value flag as the last token, or followed by a recognized flag, warns
      `<flag> given without a value; ignoring`, leaves its field empty, and consumes nothing; the
      recognized flag that followed still applies.
      Source: `src/args.rs:83-123 parse_args` (the warning at 105).
      Pin: `a_trailing_value_flag_is_warned_and_ignored`
           at src/args.rs:203
      also `a_recognized_flag_is_never_consumed_as_a_value`
           at src/args.rs:175
      also `the_long_running_flag_is_protected_from_being_eaten_like_every_other_one`
           at src/args.rs:187
      also `the_channel_flag_names_a_route_and_is_protected_like_every_value_flag`
           at src/args.rs:162

S010. The exact warning sentence is printed as `pns: <flag> given without a value; ignoring`.
      Source: `src/args.rs:105 parse_args`, `src/main.rs:2738-2759 event_mode`.
      Pin: UNPINNED. The unit tests assert only that the warning names the flag; no test asserts the
      sentence.

S011. An unrecognized token in value position is taken as the value verbatim, with no warning
      (`--agent --bogus` names an agent `--bogus`).
      Source: `src/args.rs:83-123 parse_args`.
      Pin: `an_unrecognized_token_is_still_taken_as_a_value`
           at src/args.rs:212

S012. An unknown token in flag position is skipped in silence; a stray leading word does not stop a
      delivery.
      Source: `src/args.rs:83-123 parse_args`.
      Pin: `unknown_arguments_are_skipped_in_silence`
           at src/args.rs:221
      also `a_producer_invocation_led_by_a_stray_word_still_delivers`
           at tests/dispatch.rs:669

S013. `--help` or `-h` in flag position prints `USAGE` to stdout and returns before any config load,
      probe, warning or delivery, with stderr exactly empty and no state directory created.
      Source: `src/main.rs:2738-2759 event_mode`.
      Pin: `the_help_flag_prints_the_usage_and_reaches_nothing_at_all`
           at tests/dispatch.rs:445
      also `help_in_flag_position_wins_wherever_it_reaches_the_event_parser`
           at tests/dispatch.rs:553

S014. `--help` in value position is the field's literal value and the event is delivered.
      Source: `src/args.rs:83-123 parse_args`.
      Pin: `help_in_value_position_is_still_just_a_value`
           at tests/dispatch.rs:582, src/args.rs:242

S015. A repeated value flag overwrites its field; the last occurrence wins.
      Source: `src/args.rs:83-123 parse_args` (each arm assigns).
      Pin: UNPINNED. Read off the assignment; no test repeats a flag.

S016. `--local-only` keeps only plugins declaring `local`; `--remote-only` keeps only plugins declaring
      `durable` and makes every leg report its outcome (`sync` on the wire).
      Source: `src/routing.rs:72-130 channel_plan`.
      Pin: `local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`
           at tests/dispatch.rs:104
      also `remote_only_delivers_through_hermes_alone`
           at tests/dispatch.rs:117
      also `hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible`
           at tests/dispatch.rs:129

S017. Both narrowing flags together plan nothing, and the event path prints exactly one line to stdout:
      `pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every
      channel; nothing was sent`. No other empty plan prints anything.
      Source: `src/routing.rs:72-130 channel_plan`, `src/main.rs:2913 run_event`.
      Pin: `both_narrowing_flags_together_deliver_nothing_and_say_so`
           at tests/dispatch.rs:139
      also `a_scrub_warning_is_not_printed_when_no_channel_will_run`
           at tests/dispatch.rs:402

S018. `--long-running` is the tier the lamps ride on: the plan's `pulse` is that flag, and a watched
      pane still earns the mobile card only when both the flag and `mobile_watch_card` are true.
      Source: `src/surface.rs:211-227 plan`.
      Pin: `every_long_running_row_pulses_whatever_else_it_decides`
           at src/surface.rs:844
      also `every_delivery_row_in_the_confirmed_matrix_plans_correctly`
           at src/surface.rs:671

S019. `--channel <route>` names a hermes route, never a URL: `PNS_HERMES_URL` wins if set, an empty
      route posts to the default, a usable route swaps the default URL's final path segment, and an
      unusable name prints `pns: --channel "<name>" is not a usable route name; posting to the default
      route` and posts to the default.
      Source: `src/main.rs:3826-3853 hermes_url_for`, `src/channels/hermes.rs:91 channel_url`,
      `src/safety.rs:70 route_name_is_usable`.
      Pin: `a_route_name_swaps_the_default_urls_final_segment`
           at src/channels/hermes.rs:129
      also `one_rule_judges_a_route_name_wherever_it_is_read`
           at src/channels/hermes.rs:105

S020. No test drives `--channel` from argv to the wire, and no test pins the refusal line above.
      Source: `src/main.rs:3826-3853 hermes_url_for`.
      Pin: UNPINNED. The route-on-the-wire test covers the config-named stale-alert route only
      (`the_stale_alert_posts_to_the_hermes_route_the_config_named`).

S021. `--pane <id>` failing the safety allowlist is replaced by the empty string for every leg, and one
      stderr line `pns: dropped a pane id with shell metacharacters; no channel will focus a pane` is
      printed, only when a leg will run.
      Source: `src/engine.rs:244 decide` (`pane_dropped`), `src/main.rs:3245-3325 dispatch_legs`,
      `src/safety.rs:17 pane_is_safe`.
      Pin: `a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event`
           at tests/dispatch.rs:386
      also `a_scrub_warning_is_not_printed_when_no_channel_will_run`
           at tests/dispatch.rs:402

### 1.3 The other entry points, one line each

S022. `pns gate <harness>-hook` and the bare `pns <harness>-hook` reach one `gate_mode`, which exits 0
      unless the payload was forwarded, and then returns moshi's own exit code.
      Source: `src/main.rs:134-143 main`, `src/main.rs:235-247 gate_mode`.
      Pin: `the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word`
           at tests/hooks.rs:2374
      also `a_zero_decision_passes_through_as_zero_and_is_not_a_default`
           at tests/hooks.rs:2350

S023. `pns gate <bad word>` exits 0 silently with nothing handed to moshi and no event raised; a bare
      `pns <bad word>` falls through to the typo refusal and exits 2.
      Source: `src/main.rs:235-247 gate_mode`, `src/hooks.rs:374-380 is_harness_subcommand`.
      Pin: `the_gate_subcommand_refuses_a_word_it_will_not_vouch_for_without_notifying`
           at tests/hooks.rs:2399
      also `a_shape_the_gate_will_not_vouch_for_is_never_handed_to_moshi`
           at tests/hooks.rs:2415

S024. `pns hook <event>` serves eleven words; an unserved word reads stdin, prints
      `pns: unknown hook event \`<word>\`` on stderr, notifies nobody and exits 0.
      Source: `src/main.rs:488-679 hook_mode` (the catch-all arm).
      Pin: `a_hook_word_this_binary_does_not_serve_says_so_and_notifies_nobody`
           at tests/hooks.rs:1692

S025. `pns pulse [<exit-code>]`: help anywhere in the tail prints `PULSE_USAGE` to stdout and exits 0
      with no config read; a tail longer than one word, or a word that is not all ASCII digits,
      prints `PULSE_USAGE` to stderr and exits 2; an all-zero code is a success pulse and any other
      digit run a failure pulse.
      Source: `src/main.rs:3952-3993 pulse_mode`, `src/main.rs:3995 PULSE_USAGE`, `src/pulse.rs:95-112
      exit_behaviour`.
      Pin: `pulse_help_prints_its_own_usage_before_any_config_load`
           at tests/dispatch.rs:822
      also `pulse_refuses_a_code_it_cannot_read_instead_of_guessing_it_failed`
           at tests/dispatch.rs:859

S026. `pns pulse` fails CLOSED on a broken config: it prints `pns: config error (<detail>); no pulse`,
      dials no bridge and exits 0; an absent config is silent and dials nothing.
      Source: `src/main.rs:3952-3993 pulse_mode`.
      Pin: `a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`
           at tests/dispatch.rs:781
      also `an_unknown_plugin_never_resurrects_a_disabled_pulse`
           at tests/dispatch.rs:913
      also `an_absent_config_stays_silent_in_pulse_mode`
           at tests/dispatch.rs:807

S027. The global usage still says `pns pulse <exit-code>` while `PULSE_USAGE` makes the code optional.
      Source: `src/main.rs:166-190 USAGE`, `src/main.rs:3995 PULSE_USAGE`.
      Pin: UNPINNED. A recorded defect (backlog B30), not a tested contract.

S028. `pns home` always exits 0, ignores every further argv word, and prints exactly one report.
      Source: `src/main.rs:69 main`, `src/main.rs:4012-4126 home_mode`.
      Pin: `every_way_the_home_probe_is_not_set_up_says_which_one_it_is`
           at tests/dispatch.rs:1400

S029. `pns home <extra>` still takes a reading rather than refusing.
      Source: `src/main.rs:69 main` (no argv read past the word).
      Pin: UNPINNED. Backlog B12 remainder; no test.

S030. `pns quiet` with no argument reports and mutes nothing; `pns quiet <duration>` publishes an
      absolute expiry and reports it back off the file; `pns quiet off` unlinks the file; any other
      shape prints `QUIET_USAGE` on stderr and exits 2; a write that failed exits 1 and reports the
      mute that still stands.
      Source: `src/main.rs:9104-9175 quiet_mode`, `src/main.rs:9179 QUIET_USAGE`.
      Pin: `a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it`
           at tests/dispatch.rs:2439
      also `off_removes_the_state_file_and_the_next_event_decorates_again`
           at tests/dispatch.rs:2482
      also `a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state`
           at tests/dispatch.rs:2629
      also `a_mute_that_could_not_be_written_reports_the_mute_that_still_stands`
           at tests/dispatch.rs:2719
      also `a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind`
           at tests/dispatch.rs:2761

S031. `pns quiet --help` is refused with exit 2 like any other unparseable duration.
      Source: `src/main.rs:9104-9175 quiet_mode` (no help arm).
      Pin: UNPINNED. Code-derived.

S032. `pns doctor` with any extra word, the empty string included, prints `pns: usage: pns doctor` on
      stderr and exits 2 before anything is printed, sent or spawned.
      Source: `src/main.rs:4147-4376 doctor_mode`, `src/main.rs:7718 DOCTOR_USAGE`.
      Pin: `a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel`
           at tests/dispatch.rs:3538

S033. `pns doctor` exits 1 when any send failed, when the host is unpaired, or when nothing at all was
      sent; every other state exits 0, and no read-only section moves the code.
      Source: `src/doctor.rs:232-256 exit_code`, `src/main.rs:4147-4376 doctor_mode`.
      Pin: `only_a_run_that_sent_something_and_failed_nothing_exits_zero`
           at src/doctor.rs:1215
      also `an_unpaired_host_alone_earns_the_exit_code_a_one`
           at src/doctor.rs:1597
      also `the_doctor_reports_a_dead_daemon_without_moving_its_exit_code`
           at tests/daemon.rs:389

S034. `pns recap --since <epoch> --until <epoch>` requires both flags exactly once, each a plain count,
      with `since <= until`; any other word, a repeated flag or a backwards window prints
      `pns: usage: pns recap --since <epoch> --until <epoch>` and exits 2 having posted nothing.
      Source: `src/main.rs:7752-7857 recap_mode`, `src/main.rs:8227 recap_bounds`.
      Pin: `a_recap_told_a_window_it_cannot_read_prints_usage_exits_two_and_posts_nothing`
           at tests/dispatch.rs:6442

S035. `pns daemon <word>` serves `run`, `schedule` and `cancel`; every other word, `--help` included,
      prints `DAEMON_USAGE` on stderr and exits 2.
      Source: `src/main.rs:4982-4992 daemon_mode`, `src/main.rs:4994 DAEMON_USAGE`.
      Pin: UNPINNED. The three verbs are pinned end to end; no test drives the unknown-verb arm.

S036. `pns daemon run <anything>` is refused with `DAEMON_USAGE` and exit 2 before the clock starts.
      Source: `src/main.rs:6773-6829 daemon_run`.
      Pin: UNPINNED. Code-derived; the plist passes exactly `daemon run`.

S037. `pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>|<epoch>]
      [--unless-marker <name>] -- <args>` validates and publishes one spool record by rename, waits
      on nothing, and exits 0; an unparseable argv is usage plus exit 2, no clock is exit 1, a refused
      registration is `pns daemon: <refusal>` plus exit 1.
      Source: `src/main.rs:7343-7379 daemon_schedule`, `src/main.rs:7408 parse_schedule`.
      Pin: `a_registration_succeeds_with_no_daemon_anywhere_and_blocks_on_nothing`
           at tests/daemon.rs:191
      also `a_scheduled_job_runs_once_and_its_effect_is_observable`
           at tests/daemon.rs:98

S038. `pns daemon cancel --id <id>` exits 0 whether or not the job was there (`cancelled` or
      `no job named`), exits 1 for an id that is not a job id or a removal error, and exits 2 for any
      other argv shape.
      Source: `src/main.rs:7448-7478 daemon_cancel`, `src/daemon.rs cancel`.
      Pin: UNPINNED. No integration test drives `daemon cancel`; `cancel`'s library half is exercised
      only through the unit module.

S039. `pns lights tick` exits 0 on every path and prints nothing on a healthy one; `pns lights quiet`
      is the lamps' by-hand mute; any other lights verb prints `LIGHTS_USAGE` and exits 2. A word
      trailing `lights tick` is dropped rather than refused.
      Source: `src/main.rs:4999-5012 lights_mode`, `src/main.rs:5014 LIGHTS_USAGE`, `src/main.rs:5742
      lights_tick`.
      Pin: `the_tick_says_nothing_at_all_however_many_times_it_runs`
           at tests/dispatch.rs:8340
      also `the_tick_exits_zero_with_no_config_no_table_hue_off_and_an_unreachable_bridge`
           at tests/dispatch.rs:8367

S040. `pns lights quiet [<place> [<duration>|off]]`: bare reports; `<place>` mutes until the quiet hours
      end; `<place> <duration>` mutes for that long; `<place> off` unmutes; a place no lamp, room or
      zone answers to, a bad duration or any other arity exits 2; no clock or an unwritable file exits
      1; a bare mute with no `quiet_hours` configured is refused.
      Source: `src/main.rs:5479-5575 lights_quiet`, `src/lights.rs:1183 quiet_command`,
      `src/lights.rs:1249 bare_mute_secs`.
      Pin: `an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`
           at tests/dispatch.rs:2139
      also `a_lights_quiet_write_that_failed_reports_the_disk_and_not_the_list_it_built`
           at tests/dispatch.rs:2361
      also `a_bare_mute_lasts_until_the_operators_quiet_hours_end`
           at src/lights.rs:3391

S041. `pns loop begin [--pane <id>]` writes the lease marker for the pane and registers the lights tick
      for the whole lease length; `pns loop end` removes it; no pane, an unsafe pane, or an unknown
      verb or shape exits 2; no clock or an unwritable marker exits 1; `end` with nothing to remove
      is a success.
      Source: `src/main.rs:5237-5291 loop_mode`, `src/main.rs:5303 end_lease`, `src/lights.rs:417
      loop_command`, `src/lights.rs:446 LOOP_USAGE`.
      Pin: `a_lease_is_keyed_to_the_pane_it_was_typed_in_and_refused_when_there_is_none`
           at src/lights.rs:2066
      also `a_pane_that_cannot_name_a_file_and_an_argument_this_does_not_know_are_refused`
           at src/lights.rs:2105
      also `a_lease_taken_by_hand_schedules_the_tick_that_reads_it`
           at tests/dispatch.rs:8293
      also `a_lease_that_could_not_be_given_back_is_reported_rather_than_called_a_success`
           at src/main.rs:11492

S042. `pns nag` takes no argument; any extra word prints `NAG_USAGE` to stderr, exits 2, delivers
      nothing and consumes nothing.
      Source: `src/main.rs:4401-4568 nag_mode`, `src/main.rs:4752-4765 NAG_USAGE`.
      Pin: `pns_nag_refuses_an_argument_rather_than_falling_through_to_a_fire`
           at tests/hooks.rs:3528

S043. `pns setup [--force]` accepts zero words or exactly `--force`; every other shape prints
      `SETUP_USAGE` and exits 2 first, before `HOME`, the config path or the terminal is looked at.
      Source: `src/main.rs:8434-8542 setup_mode`, `src/main.rs:9088 SETUP_USAGE`.
      Pin: `a_setup_typed_wrong_is_refused_with_what_it_takes_rather_than_walked_anyway`
           at tests/dispatch.rs:646

S044. `pns setup` refuses, each with exit 2 and nothing written: an unset or empty `HOME`; a config
      already at the name without `--force`; a path that does not resolve or cannot be checked, with
      or without `--force`; a stdin that is not a terminal. A publication failure after the walk is
      the only exit 1.
      Source: `src/main.rs:8434-8542 setup_mode`, `src/main.rs:8396 unresolvable_ancestor`.
      Pin: `an_empty_home_is_refused_by_name_before_anything_is_written`
           at tests/setup.rs:617
      also `the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone`
           at tests/dispatch.rs:624
      also `a_dangling_symlink_at_the_config_path_is_refused_before_the_first_question`
           at tests/setup.rs:496
      also `a_dangling_link_above_the_config_is_refused_before_the_first_question`
           at tests/setup.rs:526
      also `an_unreadable_config_directory_is_refused_by_path_and_cause`
           at tests/setup.rs:565
      also `the_first_run_walk_refuses_a_terminal_nobody_is_at_and_writes_nothing`
           at tests/dispatch.rs:599

S045. `pns presence poll [--daemon]` takes one reading of the bridge's room motion, publishes it to
      the presence state file, and stands down when another poll holds the lock; the hand-typed form
      says so and exits 1, the daemon's form stays silent and exits 0.
      Source: `src/main.rs:5017-5031 presence_mode`, `src/main.rs:5038-5062 presence_launch`,
      `src/main.rs:5072-5110 presence_poll`, `src/main.rs:5189-5223 Polled`.
      Pin: `a_poll_publishes_the_room_it_read_as_the_line_the_sensor_parses`
           at src/main.rs:9389
      also `two_live_contenders_and_exactly_one_is_inside_the_poll`
           at src/presence_lock.rs:119

S046. Exit codes across the surface fall into four classes: 0 for every event, hook, help, declining
      gate, `home`, pulse and successful subcommand; 1 for a typed command whose write or read failed;
      2 for a mistyped invocation; and moshi's own code for a forwarded gate or blocked hook.
      Source: `src/main.rs:1-47` (module doc), `src/main.rs:48-161 main`.
      Pin: `a_word_that_names_no_command_is_refused_and_delivers_nothing`
           at tests/dispatch.rs:470
      also `a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision`
           at tests/hooks.rs:367

## 2. Hook events

Eleven event words reach `pns hook <event>` (`src/main.rs:488-679 hook_mode`). Three contracts hold
across all of them and are stated once.

S047. The payload is read from stdin on a thread, at most `MAX_PAYLOAD_BYTES + 1` = 1,000,001 bytes,
      inside `PNS_PAYLOAD_DEADLINE_MS` (default 5,000 ms); a payload nobody finishes writing yields no
      notification and exit 0; a payload over the cap is not whole and is never forwarded, but still
      notifies.
      Source: `src/main.rs:2608-2679 read_payload`, `src/main.rs:2680 MAX_PAYLOAD_BYTES`,
      `src/main.rs:2690 payload_is_whole`.
      Pin: `a_payload_at_the_cap_is_whole_and_is_still_submitted`
           at tests/hooks.rs:827
      also `a_payload_too_large_to_be_whole_is_never_forwarded_as_though_it_were`
           at tests/hooks.rs:459
      also `a_payload_nobody_finishes_writing_still_exits_on_the_contract`
           at tests/hooks.rs:2058

S048. A payload that is not UTF-8 fails the string read, and the hook returns 0 having done nothing at
      all: no forward, no card. A pinned known limit.
      Source: `src/main.rs:2608-2679 read_payload`.
      Pin: `a_payload_that_is_not_utf8_drops_the_approval_and_tells_the_operator_nothing`
           at tests/hooks.rs:966

S049. Every payload field is optional and a document that will not parse is `HookPayload::default()`;
      `in_subagent` records whether the `agent_id` KEY was present, whatever its value.
      Source: `src/hooks.rs:14-77 HookPayload`, `src/hooks.rs:80-129 parse_payload`.
      Pin: `a_payload_yields_every_field_the_hooks_read`
           at src/hooks.rs:390
      also `a_payload_that_will_not_parse_is_empty_rather_than_fatal`
           at src/hooks.rs:453
      also `a_present_agent_id_of_any_shape_marks_a_subagent_and_absence_does_not`
           at src/hooks.rs:411

S050. The card's message is the first non-empty of `elicitation_request`, flattened `message`,
      flattened `detail`, `reported_error`, with `tool_request` as the fallback; three of those are cut
      from the head at 320 characters.
      Source: `src/hooks.rs:80-129 parse_payload`, `src/hooks.rs TOOL_REQUEST_MAX_CHARS`.
      Pin: `detail_stands_in_for_message_because_the_harnesses_disagree`
           at src/hooks.rs:461
      also `a_codex_permission_request_says_which_tool_wants_what`
           at src/hooks.rs:492
      also `a_dead_turns_error_becomes_the_message_when_the_payload_states_nothing_else`
           at src/hooks.rs:468
      also `a_stated_message_or_detail_still_outranks_an_error`
           at src/hooks.rs:482
      also `an_error_is_kept_to_one_line_and_cut_from_the_head_like_a_tool_request`
           at src/hooks.rs:740

S051. Every payload string a card is built from goes through `flattened`, which turns every run of
      whitespace or control characters (the whole Cc set, by category) into one space; paths and
      session ids are deliberately not flattened.
      Source: `src/hooks.rs:271-276 flattened`, `src/hooks.rs:80-129 parse_payload`.
      Pin: `every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel`
           at src/hooks.rs:614
      also `every_payload_string_a_card_is_built_from_is_scrubbed_and_not_the_arguments_alone`
           at src/hooks.rs:689

S052. Every ordinary hook exits 0 whatever went wrong building the notification, including an
      unparseable payload, an unknown word and a garbage re-read interval such as `1e300`.
      Source: `src/main.rs:488-679 hook_mode`.
      Pin: `nothing_that_goes_wrong_building_a_notification_fails_the_harness_turn`
           at tests/hooks.rs:1681
      also `a_malformed_reread_interval_falls_back_instead_of_panicking`
           at tests/hooks.rs:2646

S053. A hook writes nothing to stdout; on `blocked` that is asserted as exactly the empty string,
      because Claude Code decides a `PermissionRequest` off the hook's stdout.
      Source: `src/main.rs:488-679 hook_mode`, `src/channels/mod.rs:97-107 line_for` (only a
      `ReportOutcome` leg prints, and no hook arm sets `remote_only`).
      Pin: `the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision`
           at tests/hooks.rs:1256
      also `the_hook_writes_nothing_the_harness_could_read_as_an_answer_and_exits_zero`
           at tests/hooks.rs:1557
      also `a_payload_with_no_session_id_is_a_silent_no_op`
           at tests/hooks.rs:91

S054. Nothing asserts that stderr is EMPTY on a healthy hook run, and the plugin-selection warning on a
      hook path is printed with no `pns: ` prefix.
      Source: `src/main.rs:2798-3104 run_event` (the `eprintln!("{warning}")`).
      Pin: UNPINNED. Searched `tests/hooks.rs` for stderr assertions; only the unknown-event line and
      two nag lines are asserted.

### 2.1 Per event

S055. `prompt`: writes `session-<id>.start` holding the epoch only when none exists, removes this
      session's blocked wait marker, delivers nothing. No clock is no marker. A traversal-shaped
      session id writes and removes nothing.
      Source: `src/main.rs:683-703 start_of_turn`, `src/main.rs:724 turn_marker`,
      `src/main.rs:950 end_blocked_wait`.
      Pin: `the_first_prompt_of_a_turn_writes_a_marker_and_a_later_one_does_not_reset_it`
           at tests/hooks.rs:51
      also `a_prompt_from_a_waiting_session_ends_its_wait`
           at tests/hooks.rs:2931
      also `a_prompt_ends_only_its_own_sessions_wait`
           at tests/hooks.rs:3037
      also `a_session_id_carrying_a_path_traversal_never_becomes_a_filename`
           at tests/hooks.rs:74
      also `a_prompt_naming_a_traversal_removes_nothing`
           at tests/hooks.rs:3064
      also `the_prompt_hook_clears_a_stale_quota_marker`
           at tests/hooks.rs:5276

S056. `stop`: claims the turn marker by rename first, before the reply and the condenser; the elapsed
      seconds against `PNS_PULSE_THRESHOLD_SECS` (default 300, inclusive) decide `long_running`; two
      Stops racing one turn cannot both report it long.
      Source: `src/main.rs:2087-2135 end_of_turn`, `src/main.rs:2075 consume_turn_marker`,
      `src/pulse.rs:73-78 session_was_long`.
      Pin: `a_turn_long_enough_pulses_and_a_short_one_does_not`
           at tests/hooks.rs:2719
      also `stopping_consumes_the_marker_so_a_second_stop_cannot_re_fire_the_tier`
           at tests/hooks.rs:99
      also `two_stops_racing_one_turn_cannot_both_report_it_long`
           at tests/hooks.rs:2776
      also `a_second_stop_cannot_re_fire_the_tier_because_the_marker_is_claimed_once`
           at tests/hooks.rs:2594
      also `a_prompt_arriving_while_the_previous_stop_condenses_keeps_its_own_marker`
           at tests/hooks.rs:2808
      also `a_corrupt_marker_declines_rather_than_crashing_and_is_still_consumed`
           at tests/hooks.rs:116

S057. `stop`: the reply is the payload's `last_assistant_message`, else the transcript tail re-read up
      to `1 + attempts` times (default 4, max 10) with `interval` between (default 150 ms, max 5 s);
      the transcript is opened only when `symlink_metadata` says regular file, and only its last
      4,000,000 bytes are read.
      Source: `src/main.rs:2171-2193 turn_reply`, `src/main.rs:2194-2219 transcript_tail`,
      `src/hooks.rs:289-315 transcript_reply`.
      Pin: `the_payloads_own_final_text_becomes_the_detail_without_reading_a_transcript`
           at tests/hooks.rs:139
      also `the_transcript_tail_is_the_fallback_when_the_harness_carried_no_text`
           at tests/hooks.rs:152
      also `a_turn_whose_transcript_lands_late_is_re_read_until_it_does`
           at tests/hooks.rs:2615
      also `a_transcript_that_never_ends_is_not_read_at_all`
           at tests/hooks.rs:2025
      also `a_garbage_re_read_knob_still_notifies_and_still_exits_zero`
           at tests/hooks.rs:1718

S058. `stop`: a non-empty reply is condensed by `codex exec --ephemeral --skip-git-repo-check -C
      <home> -s read-only -` against a private 0700 home with `PNS_SUMMARIZING=1`, bounded by
      `CONDENSER_DEADLINE` (30 s, `PNS_CONDENSER_DEADLINE_MS`); the last usable `STATE|SUMMARY` line
      wins, only `done`, `asking` and `blocked` are verdicts, and anything else falls back to
      `("done", preview(reply))`.
      Source: `src/main.rs:2220-2258 condense`, `src/main.rs:2259-2293 condenser_home`,
      `src/hooks.rs:324-333 condenser_verdict`, `src/hooks.rs:345-355 condenser_prompt`.
      Pin: `a_condenser_line_is_used_state_and_all_and_a_blank_summary_falls_back`
           at tests/hooks.rs:186
      also `the_re_entry_guard_keeps_a_condenser_run_from_condensing_itself`
           at tests/hooks.rs:220
      also `a_condenser_that_closes_stdout_and_sleeps_is_killed_at_its_deadline`
           at tests/hooks.rs:2074
      also `a_condenser_that_never_reads_its_stdin_is_bounded_too`
           at tests/hooks.rs:2100
      also `a_state_the_prompt_never_offered_is_not_a_verdict`
           at src/hooks.rs:859
      also `the_condensers_last_usable_line_wins`
           at src/hooks.rs:841

S059. `stop`: `branch` comes from `git rev-parse --abbrev-ref HEAD` in the payload's `cwd` under a 5 s
      bound, `project` is the last segment of `cwd`, `pane` is `HERDR_PANE_ID` verbatim, and the event
      never reaches moshi.
      Source: `src/main.rs:2294-2319 git_branch`, `src/main.rs:2087-2135 end_of_turn`.
      Pin: `the_herdr_pane_reaches_the_event_verbatim_and_a_hostile_one_is_scrubbed_downstream`
           at tests/hooks.rs:235
      also `an_ordinary_stop_never_reaches_moshi`
           at tests/hooks.rs:1606

S060. `stop` and `stop-failure` both clear the nag record for the session (answered marker first, then
      the record).
      Source: `src/main.rs:4603-4649 clear_nag`, `src/main.rs:2087 end_of_turn`, `src/main.rs:2136
      failed_turn`.
      Pin: `an_answered_approval_is_never_nudged_by_either_clearing_signal`
           at tests/hooks.rs:3605

S061. The `stop-failure` call into `clear_nag` has no test of its own.
      Source: `src/main.rs:2136-2170 failed_turn`.
      Pin: UNPINNED. The clearing sweep drives `resolved` and `stop` only.

S062. `stop-failure`: claims the turn marker, delivers state `failed` with the payload's error as the
      detail, spawns no condenser and reads no transcript, still earns its pulse when the turn was
      long, and never reaches moshi.
      Source: `src/main.rs:2136-2170 failed_turn`.
      Pin: `a_turn_that_died_notifies_as_failed_and_says_what_killed_it`
           at tests/hooks.rs:251
      also `a_dead_turn_consumes_the_marker_so_the_next_turn_is_not_measured_from_its_start`
           at tests/hooks.rs:282
      also `a_dead_turn_spawns_no_condenser_and_reads_no_transcript`
           at tests/hooks.rs:305
      also `a_long_turn_that_died_still_earns_its_pulse`
           at tests/hooks.rs:2745
      also `a_failed_turn_never_reaches_moshi`
           at tests/hooks.rs:1648

S063. `blocked`: runs `blocking_event` (section 3) and the full `First` tail; the turn marker is left
      alone; the decision ring line carries `mode=`, `agent=` and `tool=` off the payload.
      Source: `src/main.rs:488-679 hook_mode`, `src/main.rs:2320-2364 blocking_event`.
      Pin: `an_approval_leaves_the_turn_marker_alone`
           at tests/hooks.rs:1224
      also `the_decision_log_carries_the_payloads_mode_agent_and_tool`
           at tests/hooks.rs:1205

S064. `asked`, `plan-ready` and `denied` share one arm: a `First` delivery with that state word, the
      session's wait marker started for `asked` and `plan-ready` (both are `LAMP_BLOCKED` words),
      never a moshi round trip.
      Source: `src/main.rs:488-679 hook_mode`, `src/pulse.rs:127 LAMP_BLOCKED`.
      Pin: `an_mcp_server_waiting_on_input_notifies_as_asked_and_names_the_server`
           at tests/hooks.rs:1533
      also `a_non_blocking_event_never_pays_for_the_round_trip`
           at tests/hooks.rs:1593
      also `a_refused_tool_call_notifies_as_denied_and_says_which_tool_was_refused`
           at tests/hooks.rs:1436
      also `a_denial_never_pays_for_the_approval_round_trip_and_still_exits_zero`
           at tests/hooks.rs:1493
      also `a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it`
           at tests/hooks.rs:2894

S065. `plan-ready` specifically has no test; its behavior is inferred from the shared arm.
      Source: `src/main.rs:488-679 hook_mode`.
      Pin: UNPINNED. The word appears in `tests/hooks.rs` only inside usage-text expectations.

S066. `resolved`: writes the session's answered nag marker, removes the record, ends the session's wait
      marker only when the payload carries no `agent_id` key, loads no config and delivers nothing.
      Source: `src/main.rs:488-679 hook_mode`, `src/main.rs:4603-4649 clear_nag`.
      Pin: `a_resolved_batch_with_no_agent_id_ends_its_sessions_wait`
           at tests/hooks.rs:2959
      also `a_resolved_batch_carrying_an_agent_id_leaves_the_parents_wait_lit`
           at tests/hooks.rs:2981
      also `a_resolved_batch_with_a_malformed_agent_id_still_leaves_the_parents_wait_lit`
           at tests/hooks.rs:3009

S067. `model-switch`: delivers an `Observation` only when `source` is `auto` and the two names differ
      once rendered plainly (flattened, then every Unicode format character dropped); an equal pair, a
      missing name or another source delivers nothing and writes nothing.
      Source: `src/main.rs:488-679 hook_mode`, `src/main.rs:263 rendered_plainly`, `src/main.rs:273
      model_switch_detail`.
      Pin: `an_observation_still_delivers_and_is_logged`
           at tests/hooks.rs:4436
      also `an_auto_switch_between_equal_names_delivers_nothing`
           at tests/hooks.rs:4480
      also `an_auto_switch_missing_a_model_name_delivers_nothing`
           at tests/hooks.rs:4504
      also `an_auto_switch_strips_a_unicode_format_character_from_the_name`
           at tests/hooks.rs:4527
      also `a_non_auto_model_switch_source_delivers_nothing_and_writes_nothing`
           at tests/hooks.rs:4552

S068. `quota`: `quota_auto_resume_fired`, `_stale` and `_disabled` each deliver one card naming
      themselves as an `Observation`; `_stale` additionally starts the session's wait marker before
      the card; an unrecognized `notification_type` delivers nothing.
      Source: `src/main.rs:488-679 hook_mode`, `src/main.rs:458 arm_quota_stale_wait`.
      Pin: `quota_auto_resume_fired_delivers_one_card_naming_itself`
           at tests/hooks.rs:4660
      also `quota_auto_resume_stale_delivers_one_card_naming_itself`
           at tests/hooks.rs:4683
      also `quota_auto_resume_disabled_delivers_one_card_naming_itself`
           at tests/hooks.rs:4706
      also `an_unrecognised_notification_type_delivers_nothing`
           at tests/hooks.rs:4755
      also `quota_auto_resume_stale_arms_the_needs_marker_for_its_own_session`
           at tests/hooks.rs:5176
      also `a_stale_wait_arms_the_needs_marker_before_the_card_is_delivered`
           at tests/hooks.rs:5202
      also `quota_auto_resume_fired_and_disabled_arm_no_needs_marker`
           at tests/hooks.rs:5246
      also `every_quota_type_is_logged_as_an_observation_with_no_nag`
           at tests/hooks.rs:5142

S069. A stale quota wait clears at the turn's own Stop without any prompt hook.
      Source: `src/main.rs:908-937 update_blocked_marker` (every non-waiting state ends the wait).
      Pin: `a_stale_quota_marker_clears_at_the_turns_stop_without_any_prompt_hook`
           at tests/hooks.rs:5313

S070. `config-change`: one of five exact sources (`user_settings`, `project_settings`,
      `local_settings`, `policy_settings`, `skills`) delivers one `Observation` card naming the source
      and the sanitized `file_path`; anything else delivers nothing and writes nothing; every card is
      delivered with no once-ever guarantee.
      Source: `src/main.rs:488-679 hook_mode`, `src/main.rs:318 config_source_label`, `src/main.rs:273
      config_change_detail`.
      Pin: `each_config_change_source_delivers_one_card_naming_itself_and_its_file`
           at tests/hooks.rs:5385
      also `a_config_change_with_no_file_names_only_the_source`
           at tests/hooks.rs:5412
      also `config_change_events_each_deliver_their_own_card_with_no_once_ever_guarantee`
           at tests/hooks.rs:5437
      also `a_hostile_file_path_is_sanitised_before_it_reaches_the_card`
           at tests/hooks.rs:5465
      also `an_unrecognised_config_source_delivers_nothing_and_writes_nothing`
           at tests/hooks.rs:5490

S071. `config-change` with `source = policy_settings` appends one line `<epoch> session=<id>
      file=<path>` to `policy-settings-audit`, bounded at 20 entries, path clipped at 1,024 and session
      at 64 characters, with no newline able to forge a second entry.
      Source: `src/main.rs:377 record_policy_settings_change`,
      `src/main.rs:360 POLICY_SETTINGS_AUDIT_KEPT`, `src/main.rs:293 config_field`.
      Pin: `a_policy_settings_change_is_recorded_to_a_bounded_audit_trail`
           at tests/hooks.rs:5863
      also `a_non_policy_config_change_writes_no_policy_audit_entry`
           at tests/hooks.rs:5896
      also `the_policy_settings_audit_trail_is_bounded_and_drops_the_oldest_entry`
           at tests/hooks.rs:5949
      also `an_enormous_file_path_cannot_wipe_the_policy_audit_trail`
           at tests/hooks.rs:6089
      also `a_newline_in_a_file_path_cannot_forge_a_policy_audit_entry`
           at tests/hooks.rs:6154
      also `an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail`
           at tests/hooks.rs:6185

S072. An `Observation` or a `Nudge` writes its decision ring line and then returns before the journal,
      the activity ring, the return-moment claim, the replay, the marker advance and the pulse.
      Source: `src/main.rs:2782 Attempt`, `src/main.rs:2798-3104 run_event` (the
      `attempt != Attempt::First` return).
      Pin: `an_observation_still_delivers_and_is_logged`
           at tests/hooks.rs:4436
      also `a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`
           at tests/hooks.rs:1733

S073. The default agent is `claude`; `PNS_AGENT=codex` is the Codex hook installer's spelling and
      changes the moshi subcommand (`codex-hook`) and switches the nag off.
      Source: `src/main.rs:488-679 hook_mode`, `src/hooks.rs:362-364 moshi_subcommand`,
      `src/main.rs:4650-4749 arm_nag`, `src/main.rs:4752 CLAUDE_AGENT`.
      Pin: `a_codex_approval_is_submitted_as_codex_hook_and_names_the_tool_that_wants_to_run`
           at tests/hooks.rs:996
      also `nothing_is_armed_when_nothing_should_be`
           at tests/hooks.rs:3824

## 3. Blocking approval and the gate

S074. `blocking_event` runs in this order: start the moshi forward, set `PNS_SKIP_PHONE=1` in this
      process only if the spawn really began, arm the nag, run the notification, then wait on the
      submission for the deadline. Only the surface decides the forward: it happens for every surface
      but `Desk`, and visibility, Focus, the mute and the phone overrides cannot reach it.
      Source: `src/main.rs:2320-2364 blocking_event`, `src/main.rs:2377-2388 forward_to_moshi`.
      Pin: `a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision`
           at tests/hooks.rs:367
      also `the_notification_still_goes_out_while_moshi_holds_the_card_but_not_to_the_phone`
           at tests/hooks.rs:395
      also `moshi_not_being_installed_leaves_the_hook_a_silent_exit_zero`
           at tests/hooks.rs:440
      also `at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual`
           at tests/hooks.rs:409
      also `the_forward_reads_the_surface_and_never_the_card_overrides`
           at tests/hooks.rs:1063
      also `an_approval_is_forwarded_even_with_the_pane_in_plain_sight`
           at tests/hooks.rs:673
      also `a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`
           at tests/hooks.rs:1733
      also `a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`
           at tests/hooks.rs:1824
      also `a_presence_reading_nobody_can_parse_still_forwards_the_approval`
           at tests/hooks.rs:1352
      also `a_phone_used_more_recently_than_the_desk_gets_the_approval_forwarded_to_it`
           at tests/hooks.rs:423

S075. Only `claude` and `codex` map to a moshi subcommand on the hook path; any other `PNS_AGENT`
      forwards nothing and exits 0 while the notification still goes out.
      Source: `src/hooks.rs:362-364 moshi_subcommand`, `src/main.rs:2320-2364 blocking_event`.
      Pin: `a_harness_pns_does_not_register_for_is_never_handed_to_moshi`
           at tests/hooks.rs:520

S076. The payload crosses to `moshi-hook <sub>` byte for byte on the child's stdin, written from a
      separate thread, whether or not pns could parse it; the child inherits the whole environment;
      the binary is `MOSHI_HOOK_BIN` else `/opt/homebrew/bin/moshi-hook`.
      Source: `src/main.rs:2445-2476 spawn_moshi_hook`, `src/main.rs:2477 DEFAULT_MOSHI_HOOK_BIN`.
      Pin: `a_payload_pns_cannot_parse_is_still_submitted_verbatim`
           at tests/hooks.rs:1034
      also `a_moshi_that_never_reads_its_stdin_cannot_hold_the_notification`
           at tests/hooks.rs:487
      also `the_submission_inherits_the_callers_environment`
           at tests/hooks.rs:706

S077. `answer_within` polls every 10 ms up to the submit deadline, returns the child's exit code if it
      finished, and on expiry kills and reaps the child and returns 0; a child killed by a signal is 0
      too; moshi's 2 comes back as 2; the answered path keeps stdout inherited so moshi's line is the
      hook's line.
      Source: `src/main.rs:2539-2561 answer_within`, `src/main.rs:2491 moshi_decision`.
      Pin: `a_moshi_that_never_answers_stops_holding_the_operators_prompt`
           at tests/hooks.rs:2188
      also `the_gate_is_bounded_by_the_same_clock_as_the_hook`
           at tests/hooks.rs:2253
      also `a_submission_that_dies_without_answering_is_not_a_decision`
           at tests/hooks.rs:786
      also `a_two_from_moshi_comes_back_as_two_and_is_never_normalized`
           at tests/hooks.rs:936
      also `what_moshi_says_on_stdout_reaches_the_harness_unchanged`
           at tests/hooks.rs:742

S078. The submit deadline is `PNS_MOSHI_SUBMIT_DEADLINE_MS` (a literal 0 falls through), else
      `[plugins.mobile] submit_deadline_secs` off the armed table (1 to 3600, 0 refused by name), else
      5 s; a refused value prints `pns: config error (<detail>); the moshi submission keeps its
      <n>-second bound`.
      Source: `src/main.rs:2570-2607 submit_deadline`, `src/main.rs:2587 configured_submit_deadline`,
      `src/config.rs:1902 submit_deadline`.
      Pin: `the_mobile_submission_deadline_is_a_count_of_seconds_defaulted_to_five`
           at src/config.rs:2850
      also `a_submission_deadline_that_is_not_a_count_of_seconds_is_refused_by_name`
           at src/config.rs:2940

S079. Exactly one submission per prompt, however the wait ends, on both entry points; no non-blocking
      event ever spawns one.
      Source: `src/main.rs:2320-2364 blocking_event`, `src/main.rs:235-247 gate_mode`.
      Pin: `one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`
           at tests/hooks.rs:607
      also `the_gate_submits_one_prompt_exactly_once`
           at tests/hooks.rs:2503

S080. A forwarded approval is recorded with `skip_phone=yes` and is never journaled as missed.
      Source: `src/main.rs:2798-3104 run_event`, `src/missed_notifications.rs:79-83 was_missed`.
      Pin: `an_approval_that_was_submitted_is_recorded_and_is_never_journaled_as_missed`
           at tests/hooks.rs:1161

S081. The gate builds a throwaway probe set, runs no delivery plan, writes no marker and raises no
      event; the over-cap payload refusal holds on the gate too.
      Source: `src/main.rs:235-247 gate_mode`.
      Pin: `the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does`
           at tests/hooks.rs:2475
      also `at_the_desk_the_gate_submits_nothing_and_exits_zero`
           at tests/hooks.rs:2437

S082. No test asserts that a gate run leaves no marker behind, and no test measures what Codex does
      with a `PermissionRequest` hook's exit code.
      Source: `src/main.rs:235-247 gate_mode`.
      Pin: UNPINNED. Both recorded in `docs/specs/blocking-approval.md`.

S083. The blocked hook's card is state `blocked`, project from the payload's `cwd`, detail from the
      message chain (`Bash: command=rm -rf /tmp/x` for Claude Code, `shell: command=bash -lc rm -rf
      build` for Codex), pane from `HERDR_PANE_ID`.
      Source: `src/main.rs:2320-2364 blocking_event`, `src/main.rs:2626 project_of`.
      Pin: `a_blocked_hook_cards_the_operator_as_blocked_and_says_what_was_asked`
           at tests/hooks.rs:572
      also `a_real_claude_approval_cards_the_tool_that_wants_to_run`
           at tests/hooks.rs:1118

## 4. The decision policy

The decision is `decide(GateInputs)` (`src/engine.rs:159-247 decide`), a total function of readings
taken once at the composition root. Nothing below `GateInputs` reads a probe.

### 4.1 Readings and their fail directions

S084. Desk idle age is `/usr/sbin/ioreg -c IOHIDSystem`, the last field of the first `HIDIdleTime`
      line, in whole seconds; a garbled, empty or failed reading is `None`, never 0 seconds idle.
      Source: `src/system.rs idle_reading`, `src/presence.rs:30 idle_secs_from_ns`.
      Pin: `the_idle_probe_argv_matches_the_bash_original`
           at src/system.rs:1940
      also `the_idle_probe_reports_whole_seconds_from_the_nanosecond_count`
           at src/system.rs:1577
      also `an_empty_reading_is_unknown_rather_than_zero_seconds_idle`
           at src/presence.rs:133
      also `contaminated_idle_output_reads_as_unknown_rather_than_a_reading`
           at src/system.rs:1407
      also `an_idle_command_that_fails_reports_unknown_which_fails_open_into_a_push`
           at src/system.rs:1583

S085. The console lock is `/usr/sbin/ioreg -n Root -d1`, the `"IOConsoleLocked"` key matched with its
      quotes, `Yes` or `No`; anything else is `None`, and only `Some(true)` disqualifies the desk.
      The lock is read only where the idle probe returned a reading.
      Source: `src/system.rs parse_screen_locked`, `src/engine.rs:341-420 surface_reading`.
      Pin: `a_console_key_that_is_missing_or_says_something_else_reads_as_no_reading`
           at src/system.rs:1474
      also `the_lock_probe_is_read_only_where_the_idle_probe_returned_a_reading`
           at src/engine.rs:1414
      also `the_lock_is_not_spawned_where_idle_failed`
           at src/system.rs:1981
      also `an_unlocked_or_unreadable_console_leaves_every_verdict_exactly_as_it_was`
           at src/surface.rs:517

S086. The Back Tap marker is the modification time of `PNS_PHONE_MARKER_FILE` else
      `$HOME/.local/state/pns/phone-attention.marker`, read off the link itself; absent is `None` and
      never fresh. Nothing in the crate writes it.
      Source: `src/system.rs PhoneMarkerProbe for SystemProbes`, `src/main.rs:2389-2417 system_probes`.
      Pin: `the_marker_probe_reads_the_link_itself_never_its_target`
           at src/system.rs:1622
      also `an_absent_marker_reports_unknown_which_the_marker_rule_fails_closed_on`
           at src/system.rs:1613

S087. The phone's input clock is the newest access time of a mosh client pty, found by
      `/usr/bin/pgrep -x mosh-server`, `/usr/bin/pgrep -P <ids>`, `/bin/ps -o tty= -p <ids>`, then a
      stat of `/dev/<tty>`; any step failing is `None`; `pgrep -P` is never called with no parents.
      Source: `src/system.rs phone_reading`, `src/system.rs newest_terminal_atime`.
      Pin: `the_discovery_argv_is_pinned_to_the_chain_that_was_measured_live`
           at src/system.rs:1763
      also `a_failure_at_any_step_of_the_chain_reads_as_no_phone_rather_than_a_fresh_one`
           at src/system.rs:1796
      also `no_mosh_server_at_all_never_asks_for_children_of_nothing`
           at src/system.rs:1820
      also `the_freshest_terminal_wins_across_every_session_found`
           at src/system.rs:1906

S088. The session view is `herdr workspace list` (the focused workspace's `active_tab_id`) then `herdr
      pane layout --pane <origin>` (tab, focused pane, zoom), never `herdr pane current`; any failure
      leaves the view unreadable, which is `Visibility::Unknown`.
      Source: `src/system.rs SessionViewProbe for SystemProbes`, `src/system.rs parse_focused_tab`,
      `src/system.rs parse_layout`.
      Pin: `the_view_asks_the_session_what_is_focused_and_never_asks_for_the_current_pane`
           at src/system.rs:2190
      also `any_herdr_call_failing_leaves_the_view_unreadable_rather_than_guessing`
           at src/system.rs:2260
      also `a_session_with_no_focused_workspace_is_unreadable_rather_than_a_guess`
           at src/system.rs:2247
      also `an_unreadable_view_delivers_rather_than_suppressing_on_doubt`
           at tests/dispatch.rs:359

S089. Every subprocess reading runs under `run_bounded`: 5 s (`PROBE_DEADLINE`), 1 MiB
      (`PROBE_READ_MAX`), the reader asks for one byte past the cap and refuses the lot, a blown
      deadline or a non-zero exit is no answer, and the child is killed and reaped.
      Source: `src/system.rs:76-148 run_bounded`, `src/system.rs PROBE_DEADLINE`,
      `src/system.rs PROBE_READ_MAX`.
      Pin: `a_stuck_multiplexer_leaves_the_view_unreadable_rather_than_blocking`
           at tests/hooks.rs:2121
      also `starting_twice_and_reading_twice_spawns_each_probe_once`
           at src/system.rs:1144

S090. Every reading on one probe set is memoized, the empty answer included, and the wall clock is one
      `now_secs()` read shared by every age; an unreadable clock ages nothing, so the phone and marker
      timestamps drop out.
      Source: `src/system.rs:404-500 SystemProbes`.
      Pin: `one_decision_reads_each_probe_at_most_once_and_never_twice`
           at src/engine.rs:1289
      also `an_unreadable_clock_ages_no_phone_signal_rather_than_treating_it_as_fresh`
           at src/engine.rs:1498
      also `an_unreadable_clock_ages_no_marker_rather_than_treating_it_as_fresh`
           at src/engine.rs:1526

S091. A stated override (`PNS_IDLE_SECS`, `PNS_PHONE_INPUT_AGE`) is trusted and its probe never runs;
      a garbled one sets an `_invalid` flag and answers unknown outright rather than a fallback.
      Source: `src/engine.rs:101-129 Overrides::from_env`, `src/engine.rs:341-420 surface_reading`.
      Pin: `a_stated_phone_input_age_spares_the_process_walk_behind_it`
           at src/engine.rs:1332
      also `a_presence_reading_nobody_can_parse_still_forwards_the_approval`
           at tests/hooks.rs:1352

S092. The world is read at dispatch, not at the moment the hook started.
      Source: `src/main.rs:2320-2364 blocking_event` (one probe set built there).
      Pin: `the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started`
           at tests/hooks.rs:2540

### 4.2 Surface

S093. `surface(desk_age, phone_age, marker_age, fresh_secs, locked)` answers `Desk`, `Mobile` or
      `Away`: an age counts only when strictly under the window; the phone's age is the smaller of its
      pty and marker ages; the fresher of desk and phone wins; a tie goes to the desk; a locked
      screen drops the desk out; nothing fresh anywhere is `Away`.
      Source: `src/surface.rs:141-170 surface`, `src/surface.rs:89 fresh_age`, `src/surface.rs:98
      is_fresh`.
      Pin: `every_surface_case_in_the_matrix_arbitrates_correctly`
           at src/surface.rs:320
      also `a_phone_signal_needs_no_expiry_window_while_it_stays_the_newest_one`
           at src/surface.rs:438
      also `a_stale_phone_reading_loses_to_the_desk_rather_than_holding_mobile`
           at src/surface.rs:461
      also `a_locked_screen_takes_the_desk_out_of_the_running_however_fresh_its_clock_is`
           at src/surface.rs:480
      also `a_locked_screen_with_a_fresh_pty_clock_is_still_the_phone_and_never_away`
           at src/surface.rs:492
      also `a_locked_screen_with_a_fresh_back_tap_is_still_the_phone_and_never_away`
           at src/surface.rs:505

      Worked examples, from the matrix test: desk 90 s and phone 5 s reads `Mobile`; desk 30 s and
      phone 30 s reads `Desk`; desk 2 s locked with no phone reads `Away`; desk 600 s and phone 600 s
      reads `Away`; a 3600 s marker beside a 30 s pty still reads `Mobile`.

S094. The freshness window is `DEFAULT_DESK_IDLE_SECS` = 120, overridable only by `PNS_DESK_IDLE_SECS`;
      119 is fresh, 120 is not. There is no config key for it.
      Source: `src/engine.rs:29 DEFAULT_DESK_IDLE_SECS`, `src/surface.rs:89 fresh_age`.
      Pin: `every_surface_case_in_the_matrix_arbitrates_correctly`
           at src/surface.rs:320
      also `the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started`
           at tests/hooks.rs:2540

S095. End to end, a Back Tap newer than the last desk input moves the operator to `Mobile`, and desk
      input after the tap cancels it.
      Source: `src/surface.rs:141-170 surface`.
      Pin: `a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile`
           at tests/dispatch.rs:242
      also `desk_input_after_the_tap_cancels_it`
           at tests/dispatch.rs:258

### 4.3 Visibility

S096. `visibility(origin, view)` is `Hidden` only with proof (a different focused tab, or a zoom onto a
      different pane), `Visible` otherwise, and `Unknown` for an empty origin or an unreadable view;
      `Unknown` never suppresses.
      Source: `src/surface.rs:74-86 visibility`, `src/surface.rs:44 SessionView`.
      Pin: `every_visibility_case_in_the_matrix_reads_correctly`
           at src/surface.rs:248
      also `a_session_view_that_cannot_be_read_is_unknown_never_visible`
           at src/surface.rs:294
      also `an_empty_origin_pane_reads_unknown`
           at src/surface.rs:305

S097. A `Mobile` surface the Back Tap alone reached (pty not fresh) runs the plan on `Hidden` whatever
      the session showed; every other case passes through.
      Source: `src/surface.rs:192-202 effective_visibility`.
      Pin: `every_effective_visibility_case_adjusts_or_passes_through_correctly`
           at src/surface.rs:559
      also `the_rule_rewrites_nothing_but_a_mobile_surface_the_phone_never_earned`
           at src/surface.rs:616
      also `a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight`
           at tests/dispatch.rs:277
      also `moshi_open_on_the_origin_pane_still_suppresses_the_card`
           at src/surface.rs:653

S098. The decision carries both the session's visibility and the one the plan ran on.
      Source: `src/engine.rs:268-301 GateInputs`.
      Pin: `a_decision_reports_both_the_sessions_visibility_and_the_one_the_plan_ran_on`
           at src/engine.rs:604

### 4.4 The plan

S099. `plan(surface, visibility, long_running, mobile_watch_card)`: `banner = Desk && !watching`;
      `phone_card` is false at the desk, `!watching || (long_running && mobile_watch_card)` on
      mobile, and true when away; `pulse = long_running`. No row banners on `Mobile`.
      Source: `src/surface.rs:211-227 plan`, `src/surface.rs:61 DeliveryPlan`.
      Pin: `every_delivery_row_in_the_confirmed_matrix_plans_correctly`
           at src/surface.rs:671
      also `no_plan_row_can_ever_banner_on_the_mobile_surface`
           at src/surface.rs:827

      Worked rows: `Desk/Visible/!long` plans nothing; `Desk/Hidden` plans the banner; `Mobile/Visible`
      plans nothing unless `long_running && mobile_watch_card`; `Mobile/Hidden` and every `Away` row
      plan the card; every `long_running` row pulses.

S100. End to end: away cards the phone and logs but raises no banner; at the desk with the pane out
      of sight the banner is the whole delivery; watching the pane only the log fires; a phone in
      hand watching the pane gets the log alone and showing another tab still cards.
      Source: `src/surface.rs:211-227 plan`, `src/routing.rs:72-130 channel_plan`.
      Pin: `away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner`
           at tests/dispatch.rs:20
      also `at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery`
           at tests/dispatch.rs:35
      also `at_the_desk_watching_the_pane_only_the_log_fires`
           at tests/dispatch.rs:51
      also `a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log`
           at tests/dispatch.rs:329
      also `a_phone_in_hand_showing_another_tab_still_cards`
           at tests/dispatch.rs:345

S101. `mobile_watch_card` defaults to false; a value of the wrong type is refused out loud with `pns:
      config error ([plugins.mobile] mobile_watch_card is <type>, not a boolean); the mobile watching
      card stays off`.
      Source: `src/main.rs:3381 watch_card`.
      Pin: `a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud`
           at tests/dispatch.rs:715

### 4.5 Overrides and the two mutes

S102. `PNS_FORCE_PHONE` forces the card on; `PNS_SKIP_PHONE` clears it and beats force; both are
      presence checks (set and non-empty), and a narrowing flag still beats a fresh tap.
      Source: `src/engine.rs:159-247 decide` (the arbitration at 214), `src/engine.rs:101-129
      Overrides::from_env`.
      Pin: `skip_phone_beats_force_phone_because_already_sent_is_more_specific`
           at src/engine.rs:1278
      also `force_phone_sends_the_card_from_the_desk_with_the_pane_in_plain_sight`
           at src/engine.rs:1316
      also `relay_skip_phone_drops_the_phone_and_only_the_phone`
           at tests/dispatch.rs:166
      also `relay_skip_phone_beats_relay_force_phone`
           at tests/dispatch.rs:182
      also `relay_force_phone_overrides_presence`
           at tests/dispatch.rs:196
      also `force_phone_is_caller_intent_and_beats_the_whole_surface_model`
           at tests/dispatch.rs:372
      also `skip_phone_still_beats_a_fresh_tap`
           at tests/dispatch.rs:313
      also `a_narrowing_flag_still_beats_a_fresh_tap`
           at tests/dispatch.rs:299
      also `skip_and_force_parse_from_their_relay_variables`
           at src/engine.rs:1770

S103. `muted` and `focus_active` are never set from the environment; the composition root reads them
      itself and applies them LAST, zeroing banner, card and pulse together and beating a forced card.
      The mute is an input to the decision and never a filter over the legs afterwards.
      Source: `src/engine.rs:34-69 Overrides`, `src/engine.rs:79 silenced`, `src/engine.rs:232-240
      decide`.
      Pin: `a_muted_decision_keeps_the_durable_log_and_drops_every_decorative_leg`
           at src/engine.rs:977
      also `a_muted_decision_plans_no_pulse_even_for_a_long_running_event`
           at src/engine.rs:1010
      also `the_mute_beats_a_forced_phone_card_because_a_producer_cannot_overrule_the_operator`
           at src/engine.rs:1042
      also `a_focus_the_config_named_suppresses_the_mutes_three_decorations_and_beats_a_forced_phone`
           at src/engine.rs:1073
      also `a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge`
           at tests/dispatch.rs:2510

S104. The operator mute is `quiet-until` holding one epoch; `is_muted(expiry, now)` is `now < expiry`
      and false for a missing expiry or a missing clock; an absent file says nothing, a corrupt or
      unreadable one complains once per event with `pns: state error (quiet-until ...); nothing is
      muted, clear it with pns quiet off` and delivers everything.
      Source: `src/main.rs:9203 read_quiet_expiry`, `src/main.rs:9224 muted_now`, `src/quiet.rs:57-62
      is_muted`, `src/quiet.rs:36-44 expiry_from_state`.
      Pin: `nothing_readable_is_not_muted_which_is_the_opposite_of_the_lights_window`
           at src/quiet.rs:206
      also `the_mute_ends_at_the_second_it_says_and_not_one_later`
           at src/quiet.rs:196
      also `a_state_file_holding_anything_else_is_a_complaint_naming_what_it_holds`
           at src/quiet.rs:165
      also `a_corrupt_state_file_delivers_everything_and_complains_once_per_event`
           at tests/dispatch.rs:2569
      also `a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event`
           at tests/dispatch.rs:2667
      also `an_absent_state_file_is_the_ordinary_state_and_says_nothing`
           at tests/dispatch.rs:2613

S105. A macOS Focus silences only when `[focus] silence` names an asserted mode, by identifier or
      display name, case-mapped both ways; an empty list opens no file; an unreadable store or catalog
      silences nothing; the two files are read under the 256 KiB ceiling through the regular-file
      guard.
      Source: `src/main.rs:9278-9298 focus_now`, `src/main.rs:9229 FOCUS_DB`, `src/focus.rs:45
      active_modes`, `src/focus.rs:79 mode_names`, `src/focus.rs:124 silenced`.
      Pin: `a_store_holding_a_live_assertion_names_the_mode_that_is_on`
           at src/focus.rs:418
      also `a_mode_the_config_names_by_its_display_name_is_silenced`
           at src/focus.rs:574
      also `a_raw_mode_identifier_is_accepted_for_a_mode_the_catalog_does_not_name`
           at src/focus.rs:607
      also `a_focus_nobody_named_silences_nothing`
           at src/focus.rs:618
      also `an_empty_list_is_the_feature_switched_off`
           at src/focus.rs:630
      also `nothing_readable_names_no_mode_one_row_per_failure_shape`
           at src/focus.rs:488
      also `a_catalog_nothing_can_read_resolves_no_names_at_all`
           at src/focus.rs:559
      also `an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`
           at tests/dispatch.rs:4915
      also `an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual`
           at tests/dispatch.rs:4979
      also `a_focus_store_that_cannot_be_read_costs_no_notification_at_all`
           at tests/dispatch.rs:5022

S106. A silenced event still reaches the durable log, is journaled as a miss, and cannot replay in the
      same run.
      Source: `src/routing.rs:72-130 channel_plan`, `src/missed_notifications.rs:79-83 was_missed`,
      `src/missed_notifications.rs:113-115 should_replay`.
      Pin: `a_muted_event_queues_its_own_miss_and_replays_nothing`
           at tests/dispatch.rs:4841
      also `an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`
           at tests/dispatch.rs:4915

### 4.6 Quiet hours, the dim window and the lamp mute

S107. `[plugins.hue] quiet_hours = "HH:MM-HH:MM"` parses into one `QuietWindow`; a value that is not
      two clock readings, or of the wrong type, is refused by name with `pns: config error
      (hue.quiet_hours is <offender>, not a HH:MM-HH:MM window); no pulse`, said once and only where
      a pulse was due; an empty string is no window.
      Source: `src/channels/hue.rs quiet_window`, `src/channels/hue.rs quiet_hours_refusal`,
      `src/main.rs:3510-3584 fire_pulse_unless_quiet`.
      Pin: `a_quiet_hours_that_is_not_two_clock_readings_is_refused_by_name`
           at src/channels/hue.rs:2201
      also `a_quiet_hours_of_the_wrong_type_is_refused_by_name_and_by_type`
           at src/channels/hue.rs:2221
      also `a_blanked_quiet_hours_is_no_window_rather_than_a_refusal`
           at src/channels/hue.rs:2233
      also `a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`
           at tests/dispatch.rs:1676

S108. `quiet_now(window, minute)`: start inclusive, end exclusive, wrapping midnight, a start equal to
      its end never quiet, and an unreadable clock INSIDE a configured window (fail dark) while no
      window is never quiet.
      Source: `src/channels/hue.rs quiet_now`, `src/system.rs local_minutes_since_midnight`.
      Pin: `a_same_day_window_is_quiet_from_its_start_and_loud_again_at_its_end`
           at src/channels/hue.rs:2248
      also `a_window_whose_start_is_after_its_end_is_quiet_on_both_sides_of_midnight`
           at src/channels/hue.rs:2268
      also `a_window_whose_start_equals_its_end_is_never_quiet`
           at src/channels/hue.rs:2292
      also `a_clock_this_machine_cannot_read_is_treated_as_inside_the_window`
           at src/channels/hue.rs:1823
      also `the_window_is_read_in_the_zone_the_child_was_given`
           at tests/dispatch.rs:1761

S109. On a machine with no `[lights]` table the quiet window gates the whole room pulse and nothing
      else; on a machine with one it reaches no routed lamp, so a typo in the house key cannot darken
      a routed lamp. The hand-run pulse and the doctor are exempt.
      Source: `src/main.rs:3510-3584 fire_pulse_unless_quiet`, `src/main.rs:3952-3993 pulse_mode`.
      Pin: `a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`
           at tests/dispatch.rs:1648
      also `a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`
           at tests/dispatch.rs:2076
      also `the_hand_run_pulse_reaches_the_bridge_inside_the_quiet_window`
           at tests/dispatch.rs:1719
      also `the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`
           at tests/dispatch.rs:3353

S110. The clock for the quiet gate is read fresh at the gate, not at the run's start.
      Source: `src/main.rs:3510-3584 fire_pulse_unless_quiet`.
      Pin: UNPINNED. Stated in the source as an honest limit: a test's clock does not advance mid-run.

S111. The dim window answers per lamp and per behaviour: `Full` outside, `Dimmed` for a listed
      behaviour inside, `Dark` for an unlisted one; an empty `dim_behaviours` darkens everything; an
      unparseable window darkens that lamp alone and names it.
      Source: `src/channels/hue.rs dim_showing`, `src/channels/hue.rs DimWindow`, `src/channels/hue.rs
      window_refusal`.
      Pin: `inside_a_window_an_enabled_behaviour_runs_dim_and_one_that_is_not_is_suppressed`
           at src/channels/hue.rs:1769
      also `a_window_with_nothing_enabled_suppresses_every_behaviour_and_needs_no_mode`
           at src/channels/hue.rs:1796
      also `a_dim_window_nobody_can_parse_leaves_that_lamp_dark_and_says_which_lamp`
           at src/channels/hue.rs:1833
      also `an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg`
           at tests/dispatch.rs:2034

S112. The lamps' by-hand mute (`lights-quiet`, `<epoch> <place>` per line, at most 32 lines) reaches a
      lamp by its own name, its room or any zone holding it, on the event flash and the tick alike; an
      unreadable record mutes everything and says so once; a missing file is ordinary.
      Source: `src/channels/hue.rs muted_now`, `src/main.rs:5710 ad_hoc_quiet`, `src/lights.rs:1107
      muted_entries`, `src/lights.rs:1149 MAX_MUTED_PLACES`.
      Pin: `a_mute_reaches_a_lamp_by_its_own_name_by_its_room_and_by_any_zone_holding_it`
           at src/channels/hue.rs:1874
      also `a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch`
           at tests/dispatch.rs:2326
      also `a_corrupt_lights_quiet_is_complained_about_once_rather_than_on_every_event`
           at tests/dispatch.rs:2207
      also `an_unreadable_lights_quiet_complains_and_an_absent_one_says_nothing`
           at src/main.rs:9839
      also `a_mute_reading_nobody_could_take_leaves_every_lamp_quiet_rather_than_loud`
           at src/main.rs:10343

S113. The tick's sustained breath answers to the lamp mutes only, never to `pns quiet` or a Focus.
      Source: `src/main.rs:5742-5861 lights_tick` (reads no `quiet-until` and no Focus store).
      Pin: UNPINNED. No test runs the tick with a live `quiet-until` or a Focus store.

### 4.7 The blocked backstop

S114. A `LAMP_BLOCKED` state (`blocked`, `asked`, `plan-ready`, `denied`, `asking`) starts a wait
      marker `lights-blocked/<session>` holding the decision's clock, only when both lamp switches are
      live; every other state ends it unconditionally; an unsafe session id or no clock writes nothing.
      Source: `src/main.rs:908-937 update_blocked_marker`, `src/lights.rs:1012
      blocked_marker_action`, `src/lights.rs:1033 blocked_marker`, `src/pulse.rs:127 LAMP_BLOCKED`.
      Pin: `a_blocked_event_starts_a_wait_and_every_other_event_ends_one`
           at src/lights.rs:3049
      also `a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live`
           at src/main.rs:11578
      also `a_blocked_turn_lights_the_lamps_once_the_map_exists`
           at tests/dispatch.rs:2008

S115. The tick sweeps a wait past `[lights.blocked] give_up_after_secs` (default 57,600, bounds 60 to
      604,800), both edges closed, taking the marker by rename and re-reading it before removal; a
      marker from the future is live.
      Source: `src/main.rs:6449 sweep_blocked`, `src/main.rs:5416-5461 sweep_markers`,
      `src/lights.rs:519 marker_is_live`, `src/config.rs DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS`.
      Pin: `a_wait_nobody_has_answered_still_holds_its_lamp_until_the_configured_backstop`
           at src/main.rs:11825
      also `the_ticks_blocked_reading_takes_its_backstop_from_the_config_on_both_halves`
           at src/main.rs:11789
      also `a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind`
           at src/main.rs:11932
      also `the_sweep_leaves_a_marker_that_is_mid_publish_alone`
           at src/main.rs:11862
      also `a_live_wait_holds_the_blocked_lamp_and_an_abandoned_one_stops_holding_it`
           at src/lights.rs:2042

S116. Configuration refuses a `give_up_after_secs` strictly below `[nag] after_secs`, naming both keys
      and both values; equal is accepted.
      Source: `src/config.rs backstop_outlasts_the_nag`.
      Pin: `a_backstop_that_gives_up_before_the_nag_nudges_is_refused_naming_both_keys`
           at src/config.rs:3276

S117. The blocked lamp's one flash is muted with everything else by the operator mute; the tick's
      sustained blue breath is not.
      Source: `src/main.rs:2798-3104 run_event` (the `blocked_lamp` gate), `src/main.rs:6459
      blocked_lamp`.
      Pin: `the_operators_own_mute_takes_the_blocked_lamp_with_everything_else`
           at tests/dispatch.rs:2102

S118. The `pane` value never reaches `GateInputs`; only `pane_present` and `pane_dropped` do, and an
      empty pane is not a drop.
      Source: `src/engine.rs:268-301 GateInputs`, `src/engine.rs:244 decide`.
      Pin: `no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans`
           at src/decision_log.rs:565
      also `a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane`
           at src/safety.rs:89

## 5. Routing and legs

S119. `channel_plan(selection, plan, local_only, remote_only)` emits one `Leg { name, mode,
      decorative }` per surviving registration in registration order, filtering only on `PluginKind`
      and the `Routing` declaration: presence-gated survives only with `phone_card`, local only with
      `banner`, everything else always; `decorative = presence_gated || local`.
      Source: `src/routing.rs:72-130 channel_plan`, `src/routing.rs:53-58 Leg`.
      Pin: `the_alert_path_plans_phone_then_banner_then_log`
           at src/routing.rs:204
      also `a_suppressed_phone_drops_only_the_presence_gated_leg`
           at src/routing.rs:241
      also `the_presence_gate_means_one_thing_under_every_flag`
           at src/routing.rs:407
      also `no_enabled_plugins_plan_nothing_under_every_flag`
           at src/routing.rs:346

S120. A `PluginKind::Sensor` can never become a leg under any flag, and a plugin with
      `event_dispatched: false` (hue) registers but is never a leg either.
      Source: `src/registry.rs:53 PluginKind`, `src/routing.rs:72-130 channel_plan`.
      Pin: `a_selected_sensor_is_never_a_leg_on_the_alert_path`
           at src/routing.rs:219
      also `a_selected_sensor_is_never_a_leg_under_local_only_either`
           at src/routing.rs:267
      also `a_selected_sensor_is_never_a_leg_under_remote_only_either`
           at src/routing.rs:309
      also `a_plugin_that_is_not_event_dispatched_is_never_a_leg_however_it_is_selected`
           at src/routing.rs:364

S121. `ReportMode` renders as the wire words `async` (Silent) and `sync` (ReportOutcome), and only
      `--remote-only` produces `ReportOutcome`, so neither the phone nor the banner ever carries a
      reporting leg.
      Source: `src/routing.rs:23-38 ReportMode`.
      Pin: `a_mode_names_what_the_channel_contract_spells_in_the_event`
           at src/routing.rs:458
      also `the_alert_path_labels_the_hermes_leg_silent_on_the_wire`
           at tests/dispatch.rs:67
      also `no_plan_over_the_real_roster_hands_the_phone_or_the_banner_a_reporting_leg`
           at src/routing.rs:464

S122. The roster is six registrations in this order: `router` (Sensor), `presence` (Sensor),
      `mobile` (presence-gated), `macos-banner` (local), `hermes` (durable), `hue` (local, not
      event-dispatched); `REQUIRES` pairs `presence` with `hue`; `CORE` is `["mobile",
      "macos-banner"]`.
      Source: `src/registry.rs:255-321 ROSTER`, `src/registry.rs:330 REQUIRES`, `src/registry.rs:343
      CORE`.
      Pin: `the_unconfigured_machine_knows_every_sensor_and_still_plans_channels_only`
           at src/routing.rs:384
      also `the_check_list_holds_one_entry_per_registration_in_registration_order`
           at src/doctor.rs:897

S123. `Registry::enabled` refuses an unregistered plugin name whether or not it is switched on, refuses
      a plugin enabled without the one it borrows a credential from, and returns a `Selection` in
      registration order; a duplicate registration is refused and `build_registry` panics on it.
      Source: `src/registry.rs:102-219 Registry`, `src/registry.rs:69 RegistryError`.
      Pin: `one_typod_table_name_costs_a_configured_machine_no_channel`
           at tests/dispatch.rs:744
      also `a_config_that_enables_nothing_names_every_plugin_sends_nothing_and_exits_one`
           at tests/dispatch.rs:3443

S124. `select_plugins`: a loaded config selects what it enabled; a loaded config naming an unregistered
      plugin selects the whole roster with `pns: config error (<detail>); running every built-in
      plugin`; a missing config selects the core silently; an unreadable one selects the core with
      `pns: config error (<detail>); running the core plugins (mobile, macos-banner)`.
      Source: `src/registry.rs:368-392 select_plugins`, `src/registry.rs:396-407` (the two warnings).
      Pin: `one_typod_table_name_costs_a_configured_machine_no_channel`
           at tests/dispatch.rs:744
      also `a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`
           at tests/dispatch.rs:781
      also `the_doctor_tells_a_machine_with_no_config_that_there_is_no_config`
           at tests/dispatch.rs:3154

S125. The exact core-fallback sentence is not asserted on the event path.
      Source: `src/registry.rs:396-407 core_warning`.
      Pin: UNPINNED. Only the pulse-mode test covers an unreadable config end to end.

S126. Every leg is handed one rendered `Event { agent, state, project, branch, detail, title, message,
      preview, pane }`, serialized with `mode` as the tenth, per-leg field; the title is `agent ·
      state · project`, the message falls back detail, state, `done`, and the branch prefix is
      `branch: body`.
      Source: `src/main.rs:3343 rendered_event`, `src/channels/mod.rs:21-52 Event`, `src/render.rs:15
      title`, `src/render.rs:42 message`.
      Pin: `a_channel_is_handed_the_rendered_event_not_the_raw_arguments`
           at tests/dispatch.rs:80
      also `the_event_is_the_channel_contracts_json_object`
           at src/channels/mod.rs:122
      also `the_mode_is_the_only_per_leg_field_so_one_event_serializes_both_ways`
           at src/channels/mod.rs:182
      also `title_falls_back_to_relay_and_done_when_the_caller_gave_neither`
           at src/render.rs:154
      also `message_falls_back_to_done_when_it_was_given_nothing_at_all`
           at src/render.rs:197

S127. The preview is the message up to 260 characters, cut at the last sentence end that fits, else
      clipped to 259 plus an ellipsis; the reply cap is 8,000 characters keeping the tail; exactly
      four whitespace characters collapse.
      Source: `src/render.rs PREVIEW_MAX_CHARS`, `src/render.rs:78-108 preview`, `src/render.rs:120
      clipped`, `src/render.rs:62-73 flatten_reply`.
      Pin: `a_body_at_the_cap_passes_through_untouched`
           at src/render.rs:285
      also `one_character_over_the_cap_with_no_sentence_end_is_hard_cut_and_marked`
           at src/render.rs:291
      also `a_sentence_ending_exactly_at_the_cap_is_where_the_cut_lands`
           at src/render.rs:299
      also `a_reply_exactly_at_the_cap_is_left_whole`
           at src/render.rs:259
      also `one_character_past_the_cap_is_already_a_cut`
           at src/render.rs:264
      also `whitespace_outside_the_four_is_content_the_turn_wrote_rather_than_a_separator`
           at src/render.rs:231

S128. Dispatch precedence: with `PNS_CHANNELS_DIR` set non-empty, executables win for every name; with
      it unset or empty, a compiled-in plugin wins and `<dir>/<name>.sh` serves only names with no
      native arm; the default directory is `$HOME/.local/libexec/pns/channels`.
      Source: `src/channels/mod.rs:112 native_first`, `src/main.rs:3855-3876 deliver_leg`,
      `src/main.rs:3882 resolve_path`.
      Pin: `an_explicit_channels_dir_means_executables_win`
           at src/channels/banner.rs:370
      also `the_banner_leg_delivers_natively_and_the_executable_channel_stays_silent`
           at tests/native.rs:106

S129. One channel's failure or panic costs no sibling its turn: every channel is constructed before the
      first delivery, each `deliver_leg` runs under `catch_unwind`, and a panic is `Failed("the <name>
      channel PANICKED; nothing was sent")` with no panic text quoted.
      Source: `src/main.rs:3245-3325 dispatch_legs`.
      Pin: `a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings`
           at tests/dispatch.rs:209
      also `a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`
           at tests/dispatch.rs:3207

S130. No test drives a real panic through `dispatch_legs`, `pulse_outcome` or `lights_report`.
      Source: `src/main.rs:3245-3325 dispatch_legs`, `src/main.rs:3768 pulse_outcome`,
      `src/main.rs:3728 lights_report`.
      Pin: UNPINNED. `PANICKED` appears in `tests/` nowhere.

S131. Only a `ReportOutcome` leg's `Delivered` or `Failed` sentence reaches stdout, prefixed `pns: `;
      `Unlaunched` prints in neither mode; `Silent` never prints.
      Source: `src/channels/mod.rs:72-107 Delivery`, `src/main.rs:2798-3104 run_event`.
      Pin: `either_verdict_reaches_the_operator_on_a_reporting_leg_and_nothing_does_otherwise`
           at src/channels/mod.rs:144
      also `every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`
           at tests/dispatch.rs:2826
      also `an_absent_channel_is_simply_not_installed`
           at tests/dispatch.rs:221

S132. Seven sites still dispatch on a destination NAME: `deliver_leg`'s match, `dispatch_legs`'s
      `mobile` refusal gate, `disabled_backend_warnings`'s `router` and `mobile` literals,
      `run_event`'s `durable_route` any-name-is-hermes, `deliver_recap`'s literal `hermes`,
      `enabled_hue_table` and `plugin_settings(config, "hermes")`, and the doctor's by-name pairing.
      Source: `src/main.rs:3855 deliver_leg`, `src/main.rs:3245 dispatch_legs`, `src/main.rs:3457
      disabled_backend_warning`, `src/main.rs:2798 run_event`, `src/main.rs:8325 deliver_recap`,
      `src/main.rs:3359 enabled_hue_table`, `src/main.rs:3495 plugin_settings`, `src/main.rs:4147
      doctor_mode`.
      Pin: UNPINNED. These are the central switches the refactor removes; nothing pins them and
      nothing should.

## 6. Destination and sensor contracts

### 6.1 `macos-banner`

S133. The banner spawns `terminal-notifier` by name through PATH under the 5 s, 1 MiB runner, with argv
      `-title <title> -message <preview> -sound default -activate <bundle> -execute <click>` in that
      order, and nothing else is ever spawned.
      Source: `src/channels/banner.rs:64-82 notifier_args`, `src/channels/banner.rs:104-128 deliver`.
      Pin: `a_delivered_leg_posts_the_banner_with_the_click_baked_in`
           at src/channels/banner.rs:302
      also `nothing_but_the_notifier_is_ever_spawned`
           at src/channels/banner.rs:318

S134. Every operator-facing argv value gets one unconditional leading backslash, because the notifier
      drops a value whose first character is `(`, `[`, `{`, `-`, `<`, a quote or a zero-width space.
      Source: `src/channels/banner.rs:57 verbatim_argument`.
      Pin: `every_case_in_the_matrix_encodes_to_its_exact_argv_value`
           at src/channels/banner.rs:198
      also `no_case_in_the_matrix_can_encode_to_a_value_the_parser_eats`
           at src/channels/banner.rs:209

S135. The activate target is `PNS_TERMINAL_BUNDLE_ID`, else the inherited `__CFBundleIdentifier`, else
      `com.mitchellh.ghostty`; the click string is `<herdr> workspace focus <ws>; <herdr> agent focus
      <pane>` with herdr's absolute path.
      Source: `src/channels/banner.rs:24-32 click_command`, `src/channels/banner.rs
      DEFAULT_TERMINAL_BUNDLE_ID`, `src/main.rs:3782 banner_channel`.
      Pin: `an_unknown_terminal_activates_the_default`
           at src/channels/banner.rs:360
      also `a_delivered_leg_posts_the_banner_with_the_click_baked_in`
           at src/channels/banner.rs:302

S136. A spawn that answered is `Delivered("posted the banner")`; one that never ran, exited non-zero
      or blew its deadline is `Failed("banner FAILED (terminal-notifier did not run)")`.
      Source: `src/channels/banner.rs:104-128 deliver`.
      Pin: `a_spawn_that_answered_is_delivered_and_one_that_never_ran_names_the_notifier`
           at src/channels/banner.rs:329

### 6.2 `mobile` (backend `moshi`)

S137. `[plugins.mobile]` must name `type = "moshi"`; an absent or empty type, or any other type, is a
      refusal quoting the key or the value, printed once as `pns: config error (<reason>); no card is
      pushed` and carried onto the leg so `dispatch_legs` fails `mobile` ahead of either seam.
      Source: `src/channels/moshi.rs:58 mobile_backend`, `src/channels/moshi.rs:31 MOSHI_TYPE`,
      `src/main.rs:3422 read_mobile`, `src/main.rs:3245-3325 dispatch_legs`.
      Pin: `the_table_has_to_name_a_backend_and_the_refusal_names_the_key`
           at src/channels/moshi.rs:295
      also `a_type_no_compiled_in_backend_answers_is_refused_quoting_it`
           at src/home.rs:1508, src/channels/moshi.rs:318
      also `a_mobile_table_naming_no_compiled_in_backend_pushes_no_card_through_either_seam`
           at tests/dispatch.rs:3077

S138. A switched-off `[plugins.mobile]` or `[plugins.router]` table with a bad `type` is not refused on
      the event path; the doctor alone says `pns: [plugins.<table>] is switched off and names no
      backend this binary answers (the only type is "<type>"); nothing refuses it until it is enabled`.
      Source: `src/main.rs:3457 disabled_backend_warning`.
      Pin: `the_doctor_says_a_switched_off_table_names_no_backend_and_an_event_never_does`
           at tests/dispatch.rs:3177

S139. The card is one HTTPS POST of `{"token", "title", "message": <preview>}` plus an optional
      `{"data": {"type": "url", "url": "moshi://herdr?pane=<pane>"}}` when the pane is safe, to
      `PNS_MOSHI_URL` else `https://api.getmoshi.app/api/webhook`, under a 10 s deadline, following
      no redirect, 2xx delivered and anything else failed.
      Source: `src/channels/moshi.rs:114 webhook_body`, `src/channels/moshi.rs:107 herdr_link`,
      `src/channels/moshi.rs:145 deliver`, `src/channels/moshi.rs:190 POST_DEADLINE`,
      `src/channels/moshi.rs:195-232 UreqPost`, `src/main.rs:3801 moshi_channel`.
      Pin: `the_body_carries_token_title_and_the_preview_as_the_message`
           at src/channels/moshi.rs:405
      also `a_token_posts_once_to_the_url_with_the_preview_never_the_message`
           at src/channels/moshi.rs:503
      also `a_safe_pane_becomes_a_pane_precise_herdr_link`
           at src/channels/moshi.rs:358
      also `a_pane_the_safety_guard_refuses_gets_no_link_rather_than_an_escaped_one`
           at src/channels/moshi.rs:370
      also `a_link_rides_as_the_one_url_action_and_no_link_leaves_the_slot_absent`
           at src/channels/moshi.rs:425
      also `the_posted_card_links_to_the_origin_pane_and_a_paneless_one_ships_plain`
           at src/channels/moshi.rs:520
      also `a_redirect_is_not_a_delivery_however_the_endpoint_dresses_it_up`
           at src/channels/moshi.rs:593
      also `a_redirecting_endpoint_is_never_followed`
           at src/channels/moshi.rs:637
      also `the_deadline_fires_instead_of_parking_the_notification_path`
           at src/channels/moshi.rs:563

S140. A missing or empty token posts nothing and is `Failed("push SKIPPED -- no moshi token in the
      config ([plugins.mobile] token); nothing was sent")`; a refused or unreachable endpoint is
      `Failed("push FAILED (the moshi endpoint refused it or could not be reached)")`; the token
      reaches the request body and never stdout, stderr or an error string.
      Source: `src/channels/moshi.rs:173 NO_TOKEN_LINE`, `src/channels/moshi.rs:145 deliver`,
      `src/channels/moshi.rs:76 moshi_secret`.
      Pin: `a_missing_token_posts_nothing_and_fails_by_naming_the_config_key_to_write`
           at src/channels/moshi.rs:450
      also `a_push_the_endpoint_took_is_delivered_and_one_it_did_not_is_failed_without_the_token`
           at src/channels/moshi.rs:474
      also `native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output`
           at tests/native.rs:134
      also `a_dead_moshi_endpoint_is_silent_because_the_only_report_would_carry_the_token`
           at tests/native.rs:169

### 6.3 `hermes`

S141. The hermes record is one POST of `{"agent", "state", "project", "detail": <full message>}` signed
      HMAC-SHA256 over the exact body bytes under `[plugins.hermes] key`, sent as lowercase hex in
      `X-Webhook-Signature`, following no redirect; the key never rides in the body, the URL or any
      printed line.
      Source: `src/channels/hermes.rs:48 hermes_body`, `src/channels/hermes.rs:60 sign`,
      `src/channels/hermes.rs:212-252 HermesChannel`, `src/channels/hermes.rs:253-286 UreqSignedPost`.
      Pin: `the_body_carries_the_full_message_because_discord_has_no_ceiling`
           at src/channels/hermes.rs:352
      also `the_signature_matches_the_published_hmac_sha256_vector`
           at src/channels/hermes.rs:365
      also `the_empty_and_unicode_bodies_match_openssls_own_hmac`
           at src/channels/hermes.rs:460
      also `a_key_posts_once_with_the_signature_of_the_exact_body_bytes`
           at src/channels/hermes.rs:577
      also `the_key_never_rides_in_the_body_the_url_or_the_signature`
           at src/channels/hermes.rs:472
      also `a_redirecting_gateway_is_the_final_answer_and_the_signed_body_stays_home`
           at src/channels/hermes.rs:505
      also `sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent`
           at tests/native.rs:187

S142. The five outcome sentences are `posted HTTP <code>`, `post FAILED HTTP <code>`, `post FAILED
      (curl reported no HTTP status at all)`, `post FAILED HTTP 000 (no response; is the hermes gateway
      up?)` and `post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was
      sent`; a 2xx is delivered and every other answer, the no-key case included, is `Failed`.
      Source: `src/channels/hermes.rs:168 outcome_line`, `src/channels/hermes.rs:182 skipped_line`,
      `src/channels/hermes.rs:159 DELIVERED_STATUS`.
      Pin: `sync_outcomes_are_spelled_exactly_as_the_bash_spells_them`
           at src/channels/hermes.rs:393
      also `no_key_means_no_post_in_either_mode_and_the_verdict_is_a_failure`
           at src/channels/hermes.rs:610
      also `a_2xx_is_delivered_and_every_other_answer_is_failed_carrying_its_own_sentence`
           at src/channels/hermes.rs:630
      also `a_gateway_that_answers_401_is_named_rather_than_read_as_a_downed_gateway`
           at tests/native.rs:211
      also `a_malformed_url_is_never_attempted_which_is_its_own_outcome`
           at src/channels/hermes.rs:486
      also `a_closed_port_is_no_response`
           at src/channels/hermes.rs:494

S143. A silent leg posts under `ASYNC_DEADLINE` (10 s); a reporting leg posts under
      `remote_deadline(PNS_REMOTE_TIMEOUT)`: default 5 s, garbled falls back to 5, `0` is no deadline,
      above 86,400 clamps to 86,400.
      Source: `src/channels/hermes.rs:189 ASYNC_DEADLINE`, `src/channels/hermes.rs:202
      remote_deadline`, `src/main.rs:3810 hermes_channel`.
      Pin: `the_sync_deadline_validates_and_defaults_to_five`
           at src/channels/hermes.rs:420
      also `an_explicit_zero_deadline_is_no_deadline_like_curls_dash_m_zero`
           at src/channels/hermes.rs:431
      also `an_absurd_deadline_clamps_to_a_day_instead_of_panicking_the_edge`
           at src/channels/hermes.rs:436
      also `sync_carries_the_validated_sync_deadline`
           at src/channels/hermes.rs:600
      also `an_async_hermes_with_a_real_key_stays_silent_even_when_the_post_fails`
           at tests/native.rs:230

S144. The exact `pns: posted HTTP 200` line that the weekly log helper greps for is pinned only on the
      writer's side through the capture server, never as the reader's contract.
      Source: `src/channels/hermes.rs:168 outcome_line`.
      Pin: UNPINNED. `dot_local/libexec/unattended-upgrades/helpers/log-entries.sh` records the
      reader side as unpinned.

S145. `uu` imports `SignedPost`, `UreqSignedPost`, `PostOutcome`, `delivered`, `outcome_line` and
      `sign` from `pns::channels::hermes` by path dependency, so one signed-POST seam exists.
      Source: `dot_local/share/uu/Cargo.toml:36`, `dot_local/share/uu/src/main.rs:18`.
      Pin: UNPINNED here; uu's own suite (`cargo test --manifest-path dot_local/share/uu/Cargo.toml`)
      is the gate.

### 6.4 Executable channels

S146. An executable channel is `Command::new(<dir>/<name>.sh)` with the event JSON plus a newline on
      piped stdin, stdout and stderr inherited, waited on with NO deadline and NO output ceiling; a
      spawn that failed is `Unlaunched("could not launch the channel at <path> (<error>); nothing was
      sent")` and a channel that ran is `Silent` whatever its exit status.
      Source: `src/main.rs:3916-3934 deliver`.
      Pin: `the_delivered_event_is_newline_terminated_for_line_oriented_channels`
           at tests/dispatch.rs:697
      also `an_absent_channel_is_simply_not_installed`
           at tests/dispatch.rs:221
      also `a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`
           at tests/dispatch.rs:3267

S147. The absence of a deadline on an executable channel, and a channel writing onto the event's
      inherited stdout, are pinned by nothing.
      Source: `src/main.rs:3916-3934 deliver`.
      Pin: UNPINNED. Recorded as a finding, not merely a missing test.

### 6.5 `hue` as an attention indicator, `router` and `presence` as sensors

S148. Hue is registered as a channel with `event_dispatched: false`, so no notification routes to it;
      the doctor checks it as a `Pulse`, and the event path drives it through `fire_pulse` and the
      lamp writes in section 10.
      Source: `src/registry.rs:255-321 ROSTER`, `src/doctor.rs:117 kind_of`.
      Pin: `a_plugin_that_is_not_event_dispatched_is_never_a_leg_however_it_is_selected`
           at src/routing.rs:364
      also `a_selected_channel_no_event_dispatches_is_a_pulse_rather_than_a_send`
           at src/doctor.rs:972

S149. The router is a sensor: enabled, it is selected and named as known, no leg is planned for it, no
      event ever reaches it, and its reading is consumed by nothing in the delivery plan today.
      Source: `src/registry.rs:255-321 ROSTER`, `src/home.rs` (module comment).
      Pin: `the_binarys_own_roster_knows_the_router_sensor`
           at tests/dispatch.rs:975
      also `only_a_home_reading_alerts_and_the_sensor_is_never_a_destination`
           at tests/dispatch.rs:1274

S150. The presence sensor's reading is printed by the doctor in place of the sensor skip, never
      counted as a send.
      Source: `src/doctor.rs:117 kind_of`, `src/doctor.rs:165 presence_said`.
      Pin: `the_selected_room_sensor_is_a_reading_rather_than_the_sensor_skip`
           at src/doctor.rs:789
      also `a_reading_is_never_counted_as_a_send_however_good_it_is`
           at src/doctor.rs:880
      also `every_way_of_not_knowing_says_which_way_it_is`
           at src/doctor.rs:829

## 7. Every file pns reads or writes

The state directory is `PNS_STATE_DIR`, else `$HOME/.local/state/pns` (`src/main.rs:732 state_dir`),
created on demand. Every file this crate creates there is mode 0600 (`src/main.rs:2051
STATE_FILE_MODE`); directories take the umask.

### 7.1 Publication and ring protocols

S151. A one-line state file is published by writing `<name>.new.<pid>` at 0600 (mode set on the open
      handle after creation) and renaming it over the target in the same directory; a failed rename
      unlinks the pending file.
      Source: `src/main.rs:774-803 publish_state_line`.
      Pin: `a_publish_whose_rename_fails_leaves_no_pending_file_behind`
           at tests/dispatch.rs:2790
      also `the_journal_is_created_readable_and_writable_by_its_owner_alone`
           at tests/dispatch.rs:4398

S152. A ring append is one exclusive critical section: take `<ring>.lock` by `create_new` (200
      attempts, 1 ms apart, a holder older than 5 s read as an orphan), refuse anything at the ring's
      path that is not a regular file, append the line with its newline in one write, read back under
      the caller's ceiling, prune to the caller's depth, republish by rename, release the lock on drop.
      Source: `src/main.rs:1871-1945 append_ring_line`, `src/main.rs:1818 claim_ring_lock`,
      `src/main.rs:1794 RING_LOCK_ATTEMPTS`, `src/main.rs:1800 RING_LOCK_STALE_SECS`,
      `src/main.rs:6696 HeldLock`.
      Pin: `the_shared_append_prunes_each_ring_to_its_own_callers_depth`
           at tests/dispatch.rs:4200
      also `a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event`
           at tests/dispatch.rs:4315
      also `the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone`
           at tests/dispatch.rs:3673

S153. A ring the append cannot read back heals to the one line just written, unless the read-back
      failed with `NotFound` because a claim renamed the file away.
      Source: `src/main.rs:1961 republish_after`.
      Pin: `a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line`
           at tests/dispatch.rs:5814

S154. Every state-file read refuses a non-regular file without opening it and refuses a file over the
      caller's ceiling (`RING_READ_MAX` 256 KiB for the rings, `ACTIVITY_READ_MAX` 1 MiB for the
      activity ring).
      Source: `src/system.rs:367-382 readable_state_file`, `src/main.rs:1987 RING_READ_MAX`,
      `src/main.rs:2000 ACTIVITY_READ_MAX`.
      Pin: `a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay`
           at tests/dispatch.rs:5053
      also `a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`
           at tests/dispatch.rs:4497
      also `a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found`
           at tests/dispatch.rs:5538

S155. Ownership of a contended state file is taken by rename onto a name carrying the claimant's pid,
      or by `create_new`, and never by unlink (measured: eight racers unlinking one path were all told
      they succeeded).
      Source: `src/main.rs:1754 take_claim`, `src/main.rs:1596 claim_journal`, `src/main.rs:1335
      claim_moment`, `src/main.rs:4867 claim_lock`; `docs/decisions/0001`.
      Pin: `a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`
           at tests/dispatch.rs:5240
      also `racing_present_events_deliver_exactly_one_replay_between_them`
           at tests/dispatch.rs:5601
      also `an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind`
           at tests/dispatch.rs:6531

S156. `owner_is_gone(pid)` is one liveness answer for every claim in the directory: only `ESRCH`
      counts as gone, `EPERM` is alive, a non-positive owner is refused outright.
      Source: `src/main.rs:1669 owner_is_gone`.
      Pin: `a_held_batch_whose_owner_is_gone_is_adopted_exactly_once`
           at tests/dispatch.rs:5361
      also `a_held_batch_whose_owner_is_still_running_is_left_exactly_where_it_is`
           at tests/dispatch.rs:5323
      also `a_hand_planted_negative_hold_name_is_never_read_as_a_pid`
           at tests/dispatch.rs:5429

### 7.2 The files, one statement each

S157. `decisions`: the decision ring, one `<epoch> <key=value ...>` line per event (nudges and
      observations included), kept at 5, appended after dispatch and before the pulse by
      `record_decision`, fail-quiet, read only by the doctor. No free text: agent, state, permission
      mode, agent id and tool name pass `printable` (ASCII alphanumerics plus `.`, `-`, `_`, else the
      literal `unprintable`, then cut to 32), the pane appears only as `pane=present|none` and
      `pane_dropped=`, everything else is a number, boolean, variant name or roster name.
      Source: `src/main.rs:2005 DECISIONS`, `src/main.rs:814 record_decision`,
      `src/decision_log.rs:85-154 line`, `src/decision_log.rs:202-213 printable`,
      `src/decision_log.rs:33 KEPT`.
      Pin: `an_event_appends_exactly_one_decision_carrying_what_it_decided_and_what_the_legs_did`
           at tests/dispatch.rs:3616
      also `an_event_that_reached_no_channel_at_all_still_records_its_decision`
           at tests/dispatch.rs:3648
      also `the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone`
           at tests/dispatch.rs:3673
      also `a_state_directory_that_cannot_be_written_costs_the_event_nothing`
           at tests/dispatch.rs:3704
      also `a_line_names_the_event_and_every_gate_input_behind_one_epoch_second`
           at src/decision_log.rs:419
      also `an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable`
           at src/decision_log.rs:619
      also `no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans`
           at src/decision_log.rs:565

S158. `missed-notifications`: the journal, one JSON object per line `{at, agent, state, project,
      branch, detail}` built with `json!`, each text field flattened and capped at 260, kept at 25,
      written only on `Attempt::First` when `was_missed` holds (`!skip_phone && !watching &&
      !plan.banner && !plan.phone_card`), read by key never by position, and consumed only by the
      replay's claim.
      Source: `src/main.rs:2011 MISSED_NOTIFICATIONS`, `src/main.rs:839-858 record_missed`,
      `src/missed_notifications.rs:169-180 entry`, `src/missed_notifications.rs:190-230 entries`,
      `src/missed_notifications.rs:79-83 was_missed`, `src/missed_notifications.rs:49 KEPT`.
      Pin: `a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown`
           at tests/dispatch.rs:4239
      also `a_delivered_event_journals_nothing_at_all`
           at tests/dispatch.rs:4274
      also `the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone`
           at tests/dispatch.rs:4288
      also `an_away_event_is_missed_even_when_the_session_reported_the_pane_visible`
           at src/missed_notifications.rs:600
      also `a_card_skipped_because_another_route_already_raised_one_is_not_missed`
           at src/missed_notifications.rs:612
      also `a_line_nothing_can_parse_costs_the_entries_around_it_nothing`
           at tests/dispatch.rs:5455
      also `a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing`
           at tests/dispatch.rs:4358

S159. `was_missed` asks the PLAN, never a delivery outcome, so an event narrowed with both flags, or
      one whose plan carded a machine with no phone channel, is not journaled; the doc comment admits
      it. This is the highest-cost defect the refactor exists to fix. The pin below holds today's
      plan-level rule; its successor will pin the outcome-level one.
      Source: `src/missed_notifications.rs:79-83 was_missed`.
      Pin: `a_delivered_event_journals_nothing_at_all`
           at tests/dispatch.rs:4274

S160. `activity`: the activity ring, every event in the journal's own shape, fields capped at 120, kept
      at 150, read back under 1 MiB, never claimed and never consumed; the recap counts its window.
      Source: `src/main.rs:2016 ACTIVITY`, `src/main.rs:983-991 record_activity`, `src/main.rs:2031
      ACTIVITY_KEPT`, `src/main.rs:2038 ACTIVITY_MAX_CHARS`.
      Pin: `every_event_is_recorded_in_the_activity_ring_delivered_or_not`
           at tests/dispatch.rs:5747
      also `a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line`
           at tests/dispatch.rs:5814

S161. `last-present`: one epoch, the return window's near edge; advanced only forward, only from
      inside a moment claim, only when `is_present` (surface not `Away`), and only after the card site
      has counted the window; an unparseable marker is no edge, never epoch zero.
      Source: `src/main.rs:2021 LAST_PRESENT`, `src/main.rs:1025-1039 mark_present`,
      `src/main.rs:1056 advance_marker`, `src/main.rs:1072 read_epoch`,
      `src/missed_notifications.rs:133-135 is_present`.
      Pin: `a_present_event_moves_the_last_present_marker_and_an_away_event_does_not`
           at tests/dispatch.rs:5870
      also `the_windows_near_edge_never_moves_backward_however_late_an_event_publishes`
           at tests/dispatch.rs:6589
      also `a_marker_no_reader_can_parse_opens_no_window_rather_than_one_from_epoch_zero`
           at tests/dispatch.rs:5975
      also `the_marker_advances_so_a_second_present_event_recaps_nothing`
           at tests/dispatch.rs:6396

S162. `last-present.claim.<pid>[.<epoch>]`: the window claim, one at a time, taken by renaming the
      marker; free when this run's, when its owner is gone, or when older than 300 s; adopted by a
      second rename and removed by the adopter.
      Source: `src/main.rs:1335 claim_moment`, `src/main.rs:1393 stranded_window_claim`,
      `src/main.rs:1440 window_claim_is_free`, `src/main.rs:1457 STALE_WINDOW_CLAIM_SECS`,
      `src/main.rs:1419 window_claim_suffix`.
      Pin: `a_window_claim_whose_owner_is_gone_is_adopted_rather_than_lost_or_left_behind`
           at tests/dispatch.rs:6493
      also `the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`
           at tests/dispatch.rs:5148
      also `racing_present_events_recap_one_loud_window_exactly_once_between_them`
           at tests/dispatch.rs:6615

S163. The 300 s age test on a window claim is exercised by no test.
      Source: `src/main.rs:1440 window_claim_is_free`.
      Pin: UNPINNED. Both window-claim tests plant a claim with no epoch segment.

S164. `missed-notifications.claim.<pid>` and `missed-notifications.held.<pid>.<seq>`: the journal claim
      and hold. The journal is renamed to the claim, verified after the rename (an irregular file goes
      straight back), renamed again to the held name outside the adoption prefix, read, and only then
      removed; a read that fails leaves the hold on disk whole.
      Source: `src/main.rs:1596 claim_journal`, `src/main.rs:1710 claim_by_rename`, `src/main.rs:1754
      take_claim`, `src/main.rs:1531 Claimed`.
      Pin: `a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`
           at tests/dispatch.rs:5240
      also `an_unreadable_old_claim_cannot_starve_the_good_batch_behind_it`
           at tests/dispatch.rs:5393
      also `a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return`
           at tests/dispatch.rs:5290
      also `a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found`
           at tests/dispatch.rs:5538

S165. `claim_by_rename`'s refusal to rename over a claim already at this pid's own name is pinned by no
      test and cannot be.
      Source: `src/main.rs:1710 claim_by_rename`.
      Pin: UNPINNED. Stated in the source.

S166. `session-<id>.start`: the turn marker, one epoch, written by `prompt` only when absent, claimed
      by rename to `session-<id>.start.claim.<pid>` by `stop` and `stop-failure`, never swept.
      Source: `src/main.rs:724 turn_marker`, `src/main.rs:683-703 start_of_turn`, `src/main.rs:2075
      consume_turn_marker`.
      Pin: `the_first_prompt_of_a_turn_writes_a_marker_and_a_later_one_does_not_reset_it`
           at tests/hooks.rs:51
      also `stopping_consumes_the_marker_so_a_second_stop_cannot_re_fire_the_tier`
           at tests/hooks.rs:99

S167. `quiet-until`: one epoch, published by `pns quiet <duration>`, unlinked by `pns quiet off`, read
      by every event and by the bare report. See S030 and S104.
      Source: `src/main.rs:9186 QUIET_UNTIL`, `src/main.rs:9104-9175 quiet_mode`.
      Pin: `a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it`
           at tests/dispatch.rs:2439
      also `off_removes_the_state_file_and_the_next_event_decorates_again`
           at tests/dispatch.rs:2482

S168. `home-staleness`: one line, the staleness episode identity (config key names and the words
      `device`, `other`, `none`), written or cleared by a Home reading only; NotHome and Unknown leave
      it untouched; an unusable state directory costs one repeated warning.
      Source: `src/main.rs:2057 STALENESS_MEMORY`, `src/main.rs:754 remember_staleness`,
      `src/main.rs:741 remembered_staleness`, `src/home.rs episode_id`.
      Pin: `the_home_diagnostic_always_shows_the_evidence_and_warns_once_per_stale_state`
           at tests/dispatch.rs:1087
      also `a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing`
           at tests/dispatch.rs:1165
      also `an_episode_identity_spells_the_state_and_never_the_values_that_moved`
           at src/home.rs:1957

S169. `policy-settings-audit`: see S071; twenty lines, read by nothing in production.
      Source: `src/main.rs:364 POLICY_SETTINGS_AUDIT`, `src/main.rs:377 record_policy_settings_change`.
      Pin: `a_policy_settings_change_is_recorded_to_a_bounded_audit_trail`
           at tests/hooks.rs:5863

S170. `phone-attention.marker`: read for its link mtime only; no writer exists in `src/`. An outside
      program's touch is the whole signal.
      Source: `src/main.rs:2389-2417 system_probes`, `src/system.rs PhoneMarkerProbe for
      SystemProbes`.
      Pin: `the_marker_probe_reads_the_link_itself_never_its_target`
           at src/system.rs:1622

S171. `lights-news`: one line `<done_at> <failed_at>` (`0` for not yet), merged forward by every
      `Done` or `Failed` event whatever the delivery did and whatever the lamp switches say, owned by
      rename (`lights-news.claim.<pid>`, two attempts 2 ms apart, then a blind merge); anything but two
      counts is no news.
      Source: `src/main.rs:6717 LIGHTS_NEWS`, `src/main.rs:6344-6371 record_news`, `src/main.rs:6374
      claim_news`, `src/main.rs:6393 NEWS_CLAIM_ATTEMPTS`, `src/lights.rs:154 render_news`,
      `src/lights.rs:167 parse_news`, `src/lights.rs:194 news_after`.
      Pin: `a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`
           at tests/dispatch.rs:2241
      also `the_news_record_is_written_whatever_the_lamps_are_doing`
           at tests/dispatch.rs:2279
      also `the_news_record_survives_as_one_line_and_anything_else_is_no_news`
           at src/lights.rs:1626
      also `the_news_record_is_written_for_a_finished_or_a_dead_turn_and_read_back_as_it_was`
           at src/main.rs:11518

S172. `lights-held`: one line, space-separated tokens, each a bare fixture path or
      `<path>@<end-ms>:<brightness>:<word>`; written bare before the arm and with phases after the
      breath; removed when nothing is held; an unreadable record holds EVERYTHING for the pulse and is
      kept, not cleared, by the return.
      Source: `src/main.rs:6648 LIGHTS_HELD`, `src/main.rs:6502 read_held`, `src/main.rs:6544
      remember_held`, `src/lights.rs:879 render_held_token`, `src/lights.rs:900 parse_held_token`.
      Pin: `a_held_records_phase_round_trips_through_remember_held_and_read_held`
           at src/main.rs:9961
      also `a_bare_token_on_disk_still_reads_as_a_held_lamp_with_no_phase`
           at src/main.rs:9989
      also `a_held_record_that_is_absent_holds_nothing_and_one_that_will_not_read_holds_everything`
           at src/main.rs:9939
      also `a_held_entrys_phase_round_trips_through_its_rendered_token`
           at src/lights.rs:2795
      also `a_bare_token_reads_as_no_phase_and_a_malformed_one_falls_back_to_bare`
           at src/lights.rs:2859

S173. `lights-streak`: one line `<since> <last_seen>`, advanced by the tick alone; a garbled file is
      refused rather than read as 1970; cleared (removed) behind the 120 s grace.
      Source: `src/main.rs:6617 LIGHTS_STREAK`, `src/main.rs:6469 advance_streak`, `src/main.rs:6614
      WORKING_GRACE_SECS`, `src/lights.rs:90 render_streak`, `src/lights.rs:100 parse_streak`,
      `src/lights.rs:120 next_streak`.
      Pin: `the_streak_starts_survives_a_gap_between_turns_and_clears_behind_the_grace`
           at src/lights.rs:1480

S174. `lights-blocked/<session>`: one epoch per waiting session (S114, S115).
      Source: `src/lights.rs:1021 blocked_dir`.
      Pin: `a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it`
           at tests/hooks.rs:2894

S175. `lights-loop/<pane>`: one epoch per pane holding a loop lease, written by `pns loop begin`,
      renewed in place (no create, write then `set_len`) by the pane's own events, swept by the tick
      once past `lease_timeout_secs` (default 3900, both edges closed), removed by `pns loop end`.
      Source: `src/lights.rs:394 lease_dir`, `src/lights.rs:399 lease_marker`, `src/main.rs:5338
      renew_loop_lease`, `src/main.rs:5358 sweep_leases`, `src/main.rs:5303 end_lease`.
      Pin: `a_lease_is_renewed_only_while_it_exists_and_swept_once_it_times_out`
           at src/main.rs:11409
      also `a_renewal_writes_through_the_lease_it_found_rather_than_publishing_a_new_one`
           at src/main.rs:11456
      also `a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`
           at tests/dispatch.rs:2241

S176. `lights-shell/<shell-pid>`: one epoch per interactive shell running a tracked command, written
      by the bashrc (section 9) and never by this crate; the tick sweeps a file whose name is not a
      positive pid or whose process is gone, leaves a live shell's empty marker alone, and reads the
      OLDEST live epoch.
      Source: `src/main.rs:6645 LIGHTS_SHELL_DIR`, `src/main.rs:6416-6440 sweep_shell_markers`.
      Pin: `the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding`
           at src/main.rs:11665
      also `a_marker_whose_shell_is_gone_is_swept_and_never_read`
           at src/main.rs:11689
      also `a_name_that_is_not_a_shell_pid_is_swept`
           at src/main.rs:11712
      also `a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone`
           at src/main.rs:11742

S177. `lights-quiet`: the lamp mute (S112), removed when nothing is muted, republished whole by every
      `pns lights quiet` write.
      Source: `src/main.rs:6760 LIGHTS_QUIET`, `src/main.rs:5647 publish_muted`, `src/main.rs:5675
      muted_state`.
      Pin: `off_clears_one_place_and_leaves_the_others_where_they_were`
           at src/lights.rs:3522
      also `the_report_names_every_live_place_and_says_so_when_there_are_none`
           at src/lights.rs:3212

S178. `lights-said` and `lights-quiet-said`: the tick's and the event path's separate say-once
      memories; a complaint is said once, again only when it changes, and forgotten when it clears so
      its return is news.
      Source: `src/main.rs:6720 LIGHTS_SAID`, `src/main.rs:6751 LIGHTS_QUIET_SAID`, `src/main.rs:6567
      say_lights_once`, `src/lights.rs:1062 say`.
      Pin: `a_complaint_that_cleared_is_forgotten_so_its_return_is_news_again`
           at src/main.rs:11243
      also `a_tick_says_a_complaint_once_and_says_it_again_only_when_it_changes`
           at src/lights.rs:3086

S179. `lights-tick.lock`: taken by `create_new` before the tick resolves anything, believed for 37 s
      (`MAX_REFRESH_SECS` + the tick's bridge deadline + 1), taken over by rename when older, released
      on drop; a lock whose mtime cannot be read counts as live.
      Source: `src/main.rs:6665 LIGHTS_TICK_LOCK`, `src/main.rs:6676 lights_tick_stale_secs`,
      `src/main.rs:4867 claim_lock`, `src/main.rs:4911 lock_aged_out`.
      Pin: `a_second_tick_stands_down_while_a_first_still_holds_the_lamps`
           at src/main.rs:10549

S180. Legacy deletion targets: every tick removes `lights-glow`, `lights-working-since` and the
      `lights-needs` directory without reading them, with no marker recording that it did.
      Source: `src/main.rs:6601-6606 sweep_legacy_state`.
      Pin: `the_first_tick_sweeps_the_state_the_old_names_held`
           at src/main.rs:11224

S181. Working-name families: `<name>.new.<pid>` (a pending publish), `<name>.sweep.<pid>` (a sweep
      claim), `<name>.claim.<pid>`, `<name>.held.<pid>.<seq>`, `<ring>.lock`, `~claim.<pid>.<seq>.<id>`
      and `~pending.<pid>.<id>` in the spool. `working_owner` reads a marker directory's working file
      by its RIGHTMOST suffix and a positive pid; one whose owner is alive is never swept.
      Source: `src/lights.rs:473 working_owner`, `src/lights.rs:496 sweep_claim`,
      `src/daemon.rs WORKING_PREFIX`.
      Pin: `a_working_file_is_told_from_a_marker_by_the_process_id_that_owns_it`
           at src/lights.rs:1523
      also `a_working_file_is_told_by_its_rightmost_suffix_not_its_first`
           at src/lights.rs:1548
      also `a_pending_file_whose_run_is_gone_is_collected_and_a_marker_that_spells_it_is_swept`
           at src/main.rs:11894

S182. `nag/<session>.pending`: one JSON record `{agent, project, branch, detail, pane, armed}` per
      waiting approval, published by `arm_nag`, claimed by the fire as `<name>.claim.<pid>` built from
      the WHOLE file name, removed by `clear_nag` best-effort; `nag/fire.lock` is the fire window,
      taken by `create_new`, aged out at 60 s and then taken by rename.
      Source: `src/nag.rs:99 nag_dir`, `src/nag.rs:105 record_path`, `src/nag.rs:152 RECORD_SUFFIX`,
      `src/nag.rs:169 FIRE_LOCK`, `src/nag.rs:180 FIRE_STALE_SECS`, `src/main.rs:4810 claim_record`,
      `src/main.rs:4855 claim_fire`.
      Pin: `arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first`
           at tests/hooks.rs:3708
      also `two_ids_that_differ_only_after_a_dot_claim_two_different_names`
           at src/nag.rs:429
      also `fires_racing_over_one_directory_still_produce_exactly_one_card`
           at tests/hooks.rs:3382
      also `an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim`
           at src/nag.rs:377

S183. The `fire.lock` age-out path and a stranded `<session>.pending.claim.<pid>` are exercised by no
      test.
      Source: `src/main.rs:4867 claim_lock`, `src/main.rs:4401-4568 nag_mode`.
      Pin: UNPINNED. Recorded in `docs/specs/nagging.md`.

S184. `daemon/<id>`: the spool, one TAB-separated `key=value` line per job with `args` as a JSON
      array, at most 8,192 bytes, id at most 64 characters of `[A-Za-z0-9._:-]` not starting with `.`,
      published by rename from `~pending.<pid>.<id>` by clients, claimed by rename to
      `~claim.<pid>.<seq>.<id>` by the daemon, handed back by `hard_link` (create-if-absent) only.
      Source: `src/daemon.rs render`, `src/daemon.rs parse`, `src/daemon.rs RECORD_MAX`,
      `src/daemon.rs ID_MAX`, `src/daemon.rs name_is_safe`, `src/daemon.rs claim`, `src/daemon.rs
      hand_back`, `src/daemon.rs publish_if_absent`.
      Pin: `the_two_optional_fields_round_trip_as_absent_rather_than_as_a_sentinel`
           at src/daemon.rs:770
      also `a_record_that_is_not_a_record_is_refused_by_name_rather_than_guessed_at`
           at src/daemon.rs:453
      also `a_record_whose_id_is_not_its_filename_is_refused`
           at src/daemon.rs:1323
      also `a_refresh_published_while_a_job_is_claimed_survives_the_daemons_re_arm`
           at src/daemon.rs:1225
      also `a_registration_landing_while_the_old_record_is_claimed_is_not_deleted_by_the_cleanup`
           at src/daemon.rs:1261

S185. `daemon-markers/<name>`: an empty 0600 file whose presence cancels a job carrying
      `unless_marker`; a symlinked markers directory cancels nothing; nothing sweeps the directory.
      Source: `src/daemon.rs marker_dir`, `src/daemon.rs marker_exists`, `src/main.rs:4942
      write_marker`.
      Pin: `a_present_marker_cancels_the_job_before_anything_runs`
           at src/daemon.rs:692
      also `a_symlinked_markers_directory_cancels_nothing`
           at src/daemon.rs:1352
      also `a_marker_on_disk_cancels_a_scheduled_job_end_to_end`
           at tests/daemon.rs:551

S186. `daemon-heartbeat`: `<pid> <epoch>`, published by rename every pass, beside the spool and never
      inside it, never removed; the doctor grades it by age (10 s stale) and never by pid.
      Source: `src/daemon.rs heartbeat_path`, `src/daemon.rs publish_heartbeat`, `src/daemon.rs
      HEARTBEAT_STALE_SECS`, `src/doctor.rs:648 daemon_line`.
      Pin: `a_heartbeat_round_trips_and_anything_else_is_no_heartbeat_at_all`
           at src/doctor.rs:1763
      also `the_daemons_doctor_line_tells_the_truth_in_four_states`
           at src/doctor.rs:1677
      also `the_daemon_does_not_write_a_log_line_per_tick`
           at tests/daemon.rs:296

S187. `presence`: one line `<poll> [<edge> <0|1> <room>]`, at most 4,096 bytes read, room at most 64
      characters with no control character, published by the presence poll and read by the doctor and
      the event path's presence status; a line the parser would refuse is never rendered.
      Source: `src/presence_file.rs:34 STATE_FILE`, `src/presence_file.rs:39 READ_MAX`,
      `src/presence_file.rs:43 ROOM_MAX`, `src/presence_file.rs:82 parse_presence_line`,
      `src/presence_file.rs:132 render`.
      Pin: `a_full_line_carries_the_two_epochs_the_motion_flag_and_the_room`
           at src/presence_file.rs:153
      also `a_malformed_line_is_no_reading_rather_than_a_partial_one`
           at src/presence_file.rs:180
      also `what_the_writer_renders_is_what_the_reader_parses`
           at src/presence_file.rs:211
      also `a_reading_the_parser_would_refuse_is_no_line_at_all`
           at src/presence_file.rs:254
      also `a_room_name_past_the_bound_is_malformed_rather_than_truncated`
           at src/presence_file.rs:305

S188. `presence-poll.lock`: a kernel `flock` held across one whole poll by `try_lock`; a second live
      contender stands down; a killed holder's lock is claimable at once; a symlink, FIFO or device at
      the name is refused rather than followed or waited on.
      Source: `src/presence_lock.rs:38 LOCK_FILE`, `src/presence_lock.rs:79 claim`.
      Pin: `two_live_contenders_and_exactly_one_is_inside_the_poll`
           at src/presence_lock.rs:119
      also `the_poll_a_killed_holder_was_inside_is_claimable_at_once`
           at src/presence_lock.rs:158
      also `a_symlink_at_the_lock_name_is_refused_rather_than_followed`
           at src/presence_lock.rs:229
      also `a_fifo_at_the_lock_name_is_refused_rather_than_waited_on`
           at src/presence_lock.rs:241

S189. The config file is `$HOME/.config/pns/config.toml`, read whole with no deadline; a dangling
      symlink is `Unreadable`; `Missing`, `Unreadable`, `Malformed` and `Invalid` are four distinct
      outcomes and a `Malformed` detail is rebuilt from the parser's message and a line number,
      never the offending line.
      Source: `src/config.rs config_path`, `src/config.rs:1838-1855 load_config`, `src/config.rs:932
      parse_config`.
      Pin: `a_malformed_line_is_reported_without_echoing_its_value`
           at src/config.rs:2190
      also `the_shipped_config_template_still_parses_through_this_schema`
           at src/config.rs:4326

S190. `pns setup` writes `$HOME/.config/pns/config.toml.new.<pid>.<nanos>` at 0600 by `create_new`,
      hard-links it to `config.toml`, removes the pending name, and under `--force` first claims
      `config.toml.<UTC stamp>.backup` by `create_new` and renames the old config onto it; nothing
      else on disk is touched.
      Source: `src/main.rs:8897 publish_config`, `src/main.rs:8930 pending_name`, `src/main.rs:8941
      write_then_publish`, `src/main.rs:9019 keep_aside_at`, `src/main.rs:9078 CONFIG_FILE_MODE`.
      Pin: `a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file`
           at src/main.rs:12392
      also `a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over`
           at src/main.rs:12422
      also `a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into`
           at src/main.rs:12566
      also `a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one`
           at src/main.rs:12448
      also `a_same_second_backup_collision_names_the_backup_it_could_not_claim`
           at src/main.rs:12631

S191. Nothing in the crate calls `fsync`, `sync_all` or `sync_data`; durability across a power loss is
      not settled by the code.
      Source: `src/main.rs:774-803 publish_state_line`, `src/main.rs:8941 write_then_publish`.
      Pin: UNPINNED. A finding for the persistence step, not a missing test.

S192. `~/.config/pns/codex-home`: the condenser's stripped Codex home, created 0700, with a 0600
      `config.toml` holding `model = "gpt-5.5"` and `model_reasoning_effort = "low"` written only when
      absent, and `auth.json` re-linked to `$HOME/.codex/auth.json` on every run.
      Source: `src/main.rs:2259-2293 condenser_home`.
      Pin: UNPINNED. The re-entry guard test proves the home is used; nothing reads the modes back.

S193. The daemon's own stdout and stderr, and every job child's stderr, land in
      `~/.local/log/pns-daemon.log`; the daemon writes no line per tick and no line per successful
      firing.
      Source: `Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl:34-37`, `src/main.rs:7106
      spawn_job`.
      Pin: `the_daemon_does_not_write_a_log_line_per_tick`
           at tests/daemon.rs:296
      also `a_daemon_that_ran_a_job_says_nothing_about_having_run_it`
           at tests/daemon.rs:327
      also `a_job_childs_own_complaint_reaches_the_daemons_log`
           at tests/daemon.rs:361

## 8. The LaunchAgent and the daemon tick

S194. `com.webdavis.pns-daemon` runs `<home>/.local/libexec/pns/pns daemon run` with `RunAtLoad`,
      `KeepAlive { SuccessfulExit = false }`, `ThrottleInterval 10`, both streams to
      `~/.local/log/pns-daemon.log`, and a PATH beginning `<home>/.local/bin:/opt/homebrew/bin`.
      Source: `Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl:7-52`.
      Pin: UNPINNED. A declaration, out of test scope by the 2026-08-05 ruling.

S195. `daemon run` reads the tick once from `PNS_DAEMON_TICK_MS` (10 to 60,000 ms, anything else the
      1,000 ms default, never clamped), exits 0 printing `pns daemon: disabled in the config; exiting`
      when `[daemon] enabled = false`, refuses permanently and exits 0 when the spool path is not a
      directory, and otherwise loops: sleep one tick, count, every thirtieth tick re-read the switch
      and reconcile the presence poll job, then one pass.
      Source: `src/main.rs:6773-6829 daemon_run`, `src/main.rs:7318 daemon_tick`, `src/main.rs:7222
      daemon_enabled`, `src/main.rs:7215 SWITCH_TICKS`, `src/daemon.rs prepare_spool`.
      Pin: `turning_the_config_switch_off_stops_a_running_daemon`
           at tests/daemon.rs:654
      also `a_spool_that_is_not_a_directory_refuses_the_start_and_exits_zero`
           at tests/daemon.rs:707
      also `a_spool_path_that_is_not_a_directory_is_a_permanent_refusal`
           at src/daemon.rs:1402

S196. A config that cannot be read reads as ENABLED, with `pns daemon: the config could not be read
      (<detail>); carrying on enabled` on stderr; no `[daemon]` table is enabled too.
      Source: `src/main.rs:7222 daemon_enabled`, `src/config.rs DEFAULT_DAEMON_ENABLED`.
      Pin: `the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name`
           at src/config.rs:2718

S197. One pass reaps first, then publishes the heartbeat, then drains the spool; with no readable
      clock it reaps and returns.
      Source: `src/main.rs:6848 daemon_pass`.
      Pin: `a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone`
           at src/main.rs:11093

S198. The drain scans the spool sorted, skipping `~`-prefixed working files; a read-only peek settles
      only `Wait`; every other verdict claims by rename first and re-reads; an irregular entry is left
      alone, never opened, and named once per daemon lifetime; a record that is not a record is
      dropped naming the rule it broke.
      Source: `src/main.rs:6910-6956 drain_spool`, `src/main.rs:6965-7013 act`, `src/daemon.rs
      spool_entries`, `src/daemon.rs peek`.
      Pin: `an_irregular_spool_entry_is_left_alone_and_never_opened`
           at tests/daemon.rs:501
      also `a_hand_edited_spool_record_whose_args_fail_validation_is_dropped`
           at tests/daemon.rs:610

S199. `decide(job, now, marker_exists, running)`: `now > until` drops as expired; a present marker
      drops; a running child of the same id waits; `now < due` waits; otherwise fires. Both edges
      closed.
      Source: `src/daemon.rs:294-308 decide`.
      Pin: `a_job_fires_only_inside_its_window_and_both_edges_are_closed`
           at src/daemon.rs:654
      also `a_job_whose_lease_expired_while_the_machine_slept_is_dropped_never_run_late`
           at src/daemon.rs:676
      also `a_present_marker_cancels_the_job_before_anything_runs`
           at src/daemon.rs:692
      also `a_running_child_holds_the_next_occurrence_to_a_wait_rather_than_a_fire`
           at src/daemon.rs:708

S200. A fired job re-arms at `now + every` with `until` unchanged (nothing when past the lease), hands
      the repeat back create-if-absent, releases its claim, then spawns; a dropped job prints
      `pns daemon: dropped \`<id>\` because <its lease had expired|its marker was already there>`.
      Source: `src/main.rs:7042-7081 fire`, `src/daemon.rs:327-330 rearm`, `src/main.rs:6965-7013
      act`.
      Pin: `a_repeating_job_re_arms_at_now_plus_every_and_a_one_shot_does_not_re_arm`
           at src/daemon.rs:727
      also `a_repeating_job_keeps_firing_until_its_lease_runs_out_then_stops`
           at tests/daemon.rs:125
      also `a_marker_on_disk_cancels_a_scheduled_job_end_to_end`
           at tests/daemon.rs:551
      also `the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there`
           at tests/hooks.rs:3995

S201. A job child is `current_exe()` with the record's argv, stdin and stdout null, stderr inherited,
      in its own process group; its bound is `tick * 30` (30 s) for every job but `lights`, which gets
      `max(30 s, MAX_REFRESH_SECS + tick_bridge_deadline + tick)` = 37 s; past the bound the whole
      group gets SIGKILL, then the child, then a `wait`; the reap uses `try_wait` and never blocks.
      Source: `src/main.rs:7106 spawn_job`, `src/main.rs:7199 child_bound`, `src/main.rs:7173
      CHILD_TICKS`, `src/main.rs:7124 reap`, `src/main.rs:7149 kill_group`.
      Pin: `a_hung_child_does_not_stall_the_tick_and_is_killed`
           at tests/daemon.rs:208
      also `a_child_outlives_the_longest_interval_plus_the_write_and_the_reap_that_follow_it`
           at src/main.rs:10411

S202. `kill_group` refuses pid 0, 1 and anything not representable as `pid_t`.
      Source: `src/main.rs:7149 kill_group`.
      Pin: UNPINNED. Guard is code and comment only.

S203. On the daemon's exit, by SIGTERM or by the switch, a child mid-flight is orphaned, not killed,
      and the heartbeat is left to age out.
      Source: `src/main.rs:6773-6829 daemon_run`, `src/main.rs:7106 spawn_job`.
      Pin: UNPINNED. `DaemonGuard` kills with SIGKILL and asserts nothing about children.

S204. The daemon registers its own `presence` job (`presence poll --daemon`, every `poll_secs`, lease
      300 s, due kept) when the presence table is armed, and cancels it when the table is absent,
      switched off or refused.
      Source: `src/main.rs:7271-7305 ensure_presence_poll`, `src/main.rs:7237 PRESENCE_JOB`,
      `src/main.rs:7243 PRESENCE_LEASE_SECS`.
      Pin: `an_armed_sensor_registers_the_poll_at_its_own_interval`
           at src/main.rs:9747
      also `a_sensor_that_is_off_cancels_the_poll_it_had_registered`
           at src/main.rs:9780
      also `a_sweep_refreshes_the_lease_without_moving_a_poll_that_is_already_due`
           at src/main.rs:9801

S205. A registration is refused when `due` is more than 30 days from now in either direction, when
      `every` is outside 1 to 86,400, when `args` is empty, over 32 words or over 4,096 bytes, when
      `until < due`, or when the rendered record exceeds 8,192 bytes.
      Source: `src/daemon.rs:172-224 validate_shape`, `src/daemon.rs:231 validate_registration`.
      Pin: `a_due_outside_a_bounded_window_of_now_is_refused_at_registration`
           at src/daemon.rs:630
      also `an_argv_that_renders_past_the_record_cap_is_refused_by_name`
           at src/daemon.rs:1296

S206. The `HEARTBEAT_STALE_SECS` (10) does not scale with `PNS_DAEMON_TICK_MS`, so a daemon run above a
      10 s tick reads as not running.
      Source: `src/daemon.rs HEARTBEAT_STALE_SECS`, `src/daemon.rs DEFAULT_TICK_SECS`.
      Pin: UNPINNED. Recorded in `docs/specs/daemon-jobs.md`.

## 9. The shell notifier contract

S207. `dot_bashrc.tmpl` registers `__cmd_notify_preexec` and `__cmd_notify_precmd` through
      bash-preexec inside a darwin gate; the timer starts in `PS0`.
      Source: `dot_bashrc.tmpl:454-465`, `dot_bashrc.tmpl:593-595`.
      Pin: `test/unit/pns-shell-notifier-engine-choice.sh` (a shell test; the bats file below covers
      the marker half).

S208. One skip list serves both functions: a command whose first word is `vim`, `nvim`, `less`, `man`,
      `top`, `btop`, `ssh`, `herdr`, `claude`, `hermes`, `codex` or `fzf` publishes no marker and
      raises no notification; a word that merely starts with one of those is a build.
      Source: `dot_bashrc.tmpl:494-495 __cmd_notify_is_interactive_tui`.
      Pin: `test/unit/pns-shell-lights-marker.bats:152` (every interactive TUI on the skip list
      publishes no marker), `test/unit/pns-shell-lights-marker.bats:171` (a build whose name merely
      starts with a TUI's is still a build).

S209. `preexec` writes `$EPOCHSECONDS` to `${PNS_STATE_DIR:-$HOME/.local/state/pns}/lights-shell/$$`,
      one file per shell, and `precmd` removes it; a failed command still clears it, a shell exiting
      without another prompt clears it on `EXIT`, another pane's marker is never touched, and a state
      directory that cannot be created costs the marker and never the command.
      Source: `dot_bashrc.tmpl:481-482`, `dot_bashrc.tmpl:502-539`, `dot_bashrc.tmpl:557`.
      Pin: `test/unit/pns-shell-lights-marker.bats:97,104,109,115,127,135,187,194,212`.

S210. `precmd` computes the elapsed seconds and calls `~/.local/libexec/pns/pns` with `--agent shell
      --state done|failed --project <cwd basename> --detail "<cmd> (<dur>)" --pane "$HERDR_PANE_ID"`:
      at 300 s or longer it adds `--long-running`, from 30 s it calls without, under 30 s it calls
      nothing; both calls are backgrounded with output discarded.
      Source: `dot_bashrc.tmpl:541-590`.
      Pin: `test/unit/pns-shell-notifier-engine-choice.sh` (the engine path and the tier boundary).

S211. The 30 s and 300 s tiers and the skip list are decided in bash today; the operator ruling of
      2026-09-03 moves them into pns behind `--elapsed` and `pns shell begin|end`, and deletes the bats
      file in the same change in favour of Rust tests.
      Source: `dot_bashrc.tmpl:574-590`; `dot_agents/skills/clean-code-rust/PNS-EXAMPLE.md`.
      Pin: UNPINNED in Rust today; the bats file pins the bash.

S212. The other in-repo producers all call the same binary with the producer flags: the weekly
      unattended-upgrades jobs (`--remote-only --channel <route>` for the log route), `update-skills`,
      `report-plugin-updates`, `homebrew-weekly-upgrade`, `cutover-gate.sh`, and `uu` through its
      `[alerts] binary` (argv not derivable here); the weekly log helper greps the stdout line for
      `^pns: posted HTTP 2`.
      Source: `dot_local/share/pns/docs/specs/legacy-producer-flags.md` (the callers table),
      `dot_config/uu/private_config.toml.tmpl:35-36`.
      Pin: `test/unit/pns-weekly-engine-resolution.sh` (the engine path resolution in the weekly
      jobs).

S213. The Codex hook installer writes exactly `PNS_AGENT=codex <home>/.local/libexec/pns/pns hook stop`
      and `... hook blocked` into `~/.codex/hooks.json`, pruning the retired relay spellings and
      touching no other entry; it exits 0 when the engine is not deployed.
      Source: `dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh:9-51`.
      Pin: `test/unit/pns-codex-install-hooks.sh`.

S214. The Claude Code hooks declared in the settings template are: `UserPromptSubmit` → `hook prompt`
      (sync), `Stop` → `hook stop`, `PostModelSwitch` → `hook model-switch >/dev/null`, `StopFailure`
      → `hook stop-failure`, `Notification` (three quota matchers) → `hook quota`, `ConfigChange`
      (five source matchers) → `hook config-change`, `PermissionRequest` → `hook blocked` (sync),
      `PermissionDenied` → `hook denied`, `Elicitation` → `hook asked`, `PostToolBatch` → `hook
      resolved`, `PostToolUse` with `AskUserQuestion` → `hook asked` and `ExitPlanMode` → `hook
      plan-ready`; every arm but `prompt` and `blocked` is `async`.
      Source: `private_dot_claude/modify_settings.json:325-387`.
      Pin: UNPINNED. A declaration; the hook behaviors behind it are pinned in section 2.

S215. The builder runs `cargo build --release --locked --quiet --bin pns --manifest-path
      dot_local/share/pns/Cargo.toml` from `~/.cargo/bin/cargo`, installs `target/release/pns` to
      `~/.local/libexec/pns/pns` with mode 755 only when the bytes changed, and kickstarts the daemon,
      casing the status (0 loud, 113 silent on a first install, anything else fails the apply and
      leaves a retry marker).
      Source: `.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl:46-147`.
      Pin: `test/unit/pns-engine-build-install.sh`.

S216. `just test-rust` runs `cargo test --locked --workspace`, `cargo fmt --all --check` and `cargo
      clippy --locked --workspace --all-targets -- -D warnings` for pns, and the non-workspace forms
      for uu; `just pns-config-render` runs the `pns-config-render` bin over `config-values.toml` into
      `private_config.toml.tmpl`.
      Source: `justfile:170-175`, `justfile:327-329`.
      Pin: UNPINNED. Recipes are declarations; CI (`just test`) runs them.

## 10. The lamps as an attention indicator

S217. One event yields one behaviour word: `failed` is `Failed`; a `LAMP_BLOCKED` word is `Blocked`
      only when a `[lights]` table exists; everything else is `Done`.
      Source: `src/pulse.rs:147-155 state_behaviour`.
      Pin: `a_state_the_lamps_have_no_word_for_reports_done`
           at src/pulse.rs:221
      also `without_a_lamp_map_a_waiting_agent_reports_done_exactly_as_it_did_before`
           at src/pulse.rs:241

S218. The pulse fires when `plan.pulse` or the behaviour is `Blocked`, unless silenced; it is the LAST
      thing the event path does after every channel, and a bridge that will not answer costs the
      lamps and nothing else.
      Source: `src/main.rs:2798-3104 run_event`, `src/main.rs:3510-3584 fire_pulse_unless_quiet`.
      Pin: `a_lights_table_changes_nothing_about_an_ordinary_notification`
           at tests/dispatch.rs:1570
      also `a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`
           at tests/dispatch.rs:1648

S219. With no `[lights]` table the pulse signals whole rooms: the `room` listing is fetched, each
      configured name (`[plugins.hue] rooms`, `HUE_PULSE_ROOMS` split on newlines winning, default
      `["3F - Studio", "2F - Kitchen"]`) maps to its `grouped_light`, and one `on_off_color` signal of
      3,000 ms with no `dimming` is PUT per room.
      Source: `src/channels/hue.rs HuePulse::run`, `src/channels/hue.rs signal_fixtures`,
      `src/channels/hue.rs UNMAPPED_SIGNAL_DURATION_MS`, `src/channels/hue.rs DEFAULT_ROOMS`,
      `src/channels/hue.rs hue_settings`.
      Pin: `the_no_map_body_states_no_brightness_and_keeps_its_own_duration`
           at src/channels/hue.rs:1237
      also `the_environment_override_wins_and_splits_on_newlines`
           at src/channels/hue.rs:1123

S220. With a `[lights]` table the pulse resolves the map once and writes `pulse_body` per routed lamp
      at `light/<id>`, skipping a lamp the mute covers or that holds a state; the locked shapes are
      4,000 ms at brightness 100 for `done` and `failed`.
      Source: `src/main.rs:3642 run_pulse_writes`, `src/lights.rs:639 pulse_fires`,
      `src/channels/hue.rs pulse_body`, `src/config.rs DEFAULT_DONE`.
      Pin: `the_pulse_body_carries_the_locked_colour_duration_and_brightness`
           at src/channels/hue.rs:1948
      also `a_pulse_fires_on_a_lamp_it_is_routed_for_unless_a_held_state_has_that_lamp`
           at src/lights.rs:2244
      also `a_blocked_turn_lights_the_lamps_once_the_map_exists`
           at tests/dispatch.rs:2008

S221. The `failure` pulse and the `unread` failure breath share one `FAILURE_COLOR`.
      Source: `src/pulse.rs:23-66` (the colour constants).
      Pin: `each_held_state_renders_its_own_locked_colour_and_shape`
           at src/channels/hue.rs:1992

S222. Names resolve lamp, then room, then zone, each question (`shows`, `dim_window`) on its own, the
      winning level supplying the whole answer; a name not on the bridge or holding no lamp is
      reported; two zones answering one question is a refusal naming both; a bridge that refused any
      listing resolves nothing; the bridge's current membership is the truth and names match exactly.
      Source: `src/channels/hue.rs:448-501 resolve`, `src/channels/hue.rs resolve_on_bridge`,
      `src/channels/hue.rs inventory`.
      Pin: `the_most_specific_declaration_that_names_a_lamp_supplies_its_whole_behaviour_set`
           at src/channels/hue.rs:1439
      also `each_question_resolves_on_its_own_so_a_lamp_can_state_one_and_inherit_the_other`
           at src/channels/hue.rs:1576
      also `a_lamp_two_zones_both_answer_for_is_refused_with_both_named`
           at src/channels/hue.rs:1499
      also `a_dim_question_two_zones_both_answer_leaves_that_lamp_dark_rather_than_bright`
           at src/channels/hue.rs:1534
      also `a_lamp_moved_to_another_room_answers_the_room_it_is_in_now`
           at src/channels/hue.rs:1672
      also `a_case_folded_name_is_a_typo_rather_than_a_name_to_forgive`
           at src/channels/hue.rs:1659

S223. The house holds four states ranked `Blocked` > `Looping` > `UnreadFailure` > `UnreadSuccess`;
      each lamp shows the most urgent state its own `shows` routing lists, or nothing.
      Source: `src/lights.rs:531 Held`, `src/lights.rs:602 active_held`, `src/lights.rs:624 shown`.
      Pin: `every_held_state_is_active_at_once_and_they_rank_blocked_loop_then_unread`
           at src/lights.rs:2196
      also `one_lamp_shows_the_most_urgent_state_it_is_routed_for_and_nothing_it_is_not`
           at src/lights.rs:2218

S224. `unread` arms on news newer than the last interaction, failure at once and success after
      `[lights.unread] after_secs` (default 300, 0 to 86,400), never while anything is working, never
      with no interaction, never from the future; red wins when both are pending; the interaction edge
      is the freshest of desk, phone input and phone marker, with the clock read after the samples.
      Source: `src/lights.rs:253 unread_arming`, `src/lights.rs:296 last_interaction`,
      `src/main.rs:6289 last_interaction`.
      Pin: `unread_arms_on_news_the_operator_has_not_been_back_for_and_on_nothing_else`
           at src/lights.rs:1732
      also `success_news_waits_out_its_delay_and_failure_news_does_not`
           at src/lights.rs:1781
      also `the_interaction_edge_is_the_freshest_of_the_three_roads`
           at src/lights.rs:1868

S225. The loop lamp arms on any of: an agent streak at least `threshold_secs` old (default 300) while
      agents are still working, a shell marker whose own start is that old, or a live lease; the two
      clocks are never pooled.
      Source: `src/lights.rs:362 loop_running`, `src/lights.rs:315 Loop`, `src/lights.rs:33
      workspace_agent_statuses`, `src/lights.rs:68 any_working`.
      Pin: `work_past_the_threshold_arms_the_loop_lamp_and_both_edges_are_closed`
           at src/lights.rs:1983
      also `a_live_lease_arms_the_loop_lamp_with_nothing_working_and_an_expired_one_does_not`
           at src/lights.rs:2019
      also `a_shell_command_is_measured_from_its_own_start_and_not_from_an_agents_streak`
           at src/lights.rs:1923

S226. The tick: sweep the legacy names, derive the house from the machine (herdr workspace list, the
      streak, the shell markers, the leases, the blocked markers, the news, the interaction edge),
      take the tick lock, resolve, compute what breathes, re-read the record and stand down if it
      moved, clear the DIFFERENCE by name, write the bare held record, breathe, write the phase
      record, say complaints once, and re-register itself while anything is in flight; exit 0 and
      print nothing on every healthy path.
      Source: `src/main.rs:5742-5861 lights_tick`, `src/main.rs:5926-6089 run_tick_writes`,
      `src/main.rs:6209 lights_house`, `src/main.rs:6131 drive_breaths`.
      Pin: `a_tick_arms_a_held_lamp_records_it_and_a_dark_house_puts_it_out_by_name`
           at src/main.rs:10153
      also `a_lamp_this_arm_wrote_to_stays_held_rather_than_being_put_out_behind_the_arm`
           at src/main.rs:10270
      also `a_phased_record_clears_by_its_bare_path_never_by_the_suffix`
           at src/main.rs:10232
      also `a_tick_whose_bridge_answered_nothing_keeps_the_record_it_was_holding`
           at src/main.rs:10611
      also `a_record_cleared_during_the_breath_is_left_cleared_rather_than_resurrected`
           at src/main.rs:11061
      also `the_phase_reaches_disk_only_after_the_breath_that_earned_it_has_run`
           at src/main.rs:11020
      also `a_tick_whose_record_moved_under_it_stands_down_rather_than_re_arming_the_lamps`
           at src/main.rs:10509
      also `a_tick_with_nothing_left_to_show_puts_out_the_glow_it_was_holding`
           at tests/dispatch.rs:8542
      also `a_held_record_that_will_not_publish_stops_the_arm_rather_than_lighting_a_lamp`
           at src/main.rs:10377

S227. The tick renews its own lease (300 s) while a streak, a shell marker or a lease is in flight and
      lets it lapse otherwise; the feature switched off still puts a held lamp out where a bridge can be
      named, and hue switched off keeps the record.
      Source: `src/main.rs:5742-5861 lights_tick`, `src/main.rs:3209 schedule_lights_tick`.
      Pin: `a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold`
           at tests/dispatch.rs:8241
      also `a_tick_with_nothing_in_flight_lets_its_own_lease_lapse`
           at tests/dispatch.rs:8274
      also `switching_the_lamps_off_puts_out_a_held_glow_and_switching_hue_off_keeps_the_record`
           at tests/dispatch.rs:8471

S228. A breath fills the interval: fades are issued from the tick's own start, each leading the one
      before by `FADE_LEAD_MS` = 50, the first fade carrying colour and `on`, every later one
      brightness and duration alone, pooled across lamps into one schedule and never past the budget;
      a phase token resumes the next leg when its due is within one step, else starts over.
      Source: `src/lights.rs:796 breath_fades`, `src/lights.rs:965 resume_from`, `src/lights.rs:751
      step_ms`, `src/channels/hue.rs breath_arm_body`, `src/channels/hue.rs fade_body`.
      Pin: `each_fade_leads_the_one_before_it_so_the_lamp_never_pauses_at_an_end`
           at src/lights.rs:2359
      also `every_last_fade_is_issued_inside_the_budget_and_lands_after_it`
           at src/lights.rs:2371
      also `a_budget_that_cannot_fit_even_one_fade_is_empty`
           at src/lights.rs:2522
      also `a_resumed_breath_composes_across_two_ticks_on_a_fake_clock`
           at src/main.rs:10869
      also `a_phase_sitting_further_ahead_than_one_step_reads_as_stale`
           at src/lights.rs:2957
      also `a_lamp_that_changed_state_starts_its_new_colour_at_once_rather_than_resuming`
           at src/main.rs:10955
      also `two_breathing_lamps_share_one_schedule_rather_than_running_back_to_back`
           at src/main.rs:10647
      also `the_arm_states_the_colour_and_the_first_fade_and_every_fade_after_it_states_neither`
           at src/channels/hue.rs:2109

S229. Putting a held lamp out is `{"on":{"on":false}}`, never a restore.
      Source: `src/channels/hue.rs clear_body`.
      Pin: `what_puts_a_held_lamp_out_is_off_and_not_a_restore`
           at src/channels/hue.rs:2153

S230. The operator's return (an event whose surface is not `Away`, both switches live) writes the
      clear to every path in `lights-held` and forgets the file; an ordinary event costs one failed
      open and no network.
      Source: `src/main.rs:3119 clear_held_lamps`.
      Pin: `the_operators_return_puts_out_a_glow_without_any_daemon_running`
           at tests/dispatch.rs:8405
      also `an_event_holding_no_glow_reaches_the_bridge_for_nothing`
           at tests/dispatch.rs:8448

S231. Every event registers the lights tick (`due` kept if pending, `until = due.max(now + lease)`,
      `every = refresh_secs`, args `["lights", "tick"]`), with a 300 s lease, or 43,200 s when the
      event was journaled; a registration that cannot be written costs the event nothing.
      Source: `src/main.rs:3175 register_lights_tick`, `src/main.rs:3209 schedule_lights_tick`,
      `src/main.rs:714 ORDINARY_LEASE_SECS`, `src/main.rs:719 JOURNALLED_LEASE_SECS`.
      Pin: `an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer`
           at tests/dispatch.rs:8095
      also `a_registration_that_cannot_be_written_costs_the_event_nothing`
           at tests/dispatch.rs:8142

S232. The bridge transport disables TLS verification, sends `hue-application-key`, and bounds each call
      at 10 s (`BRIDGE_DEADLINE`), `refresh_secs / 5` on the tick (at least 1 s), and 1 s for the typed
      mute command; nothing tests the real transport.
      Source: `src/channels/hue.rs UreqBridge`, `src/channels/hue.rs BRIDGE_DEADLINE`,
      `src/channels/hue.rs TYPED_COMMAND_DEADLINE`, `src/main.rs:6745 tick_bridge_deadline`.
      Pin: UNPINNED. Every body and path assertion is a unit test through the `Bridge` trait.

S233. The presence poll reads the bridge's per-room `grouped_motion` roll-up for the watched rooms,
      picks the newest edge at the precision the bridge sends, refuses a room whose report is invalid,
      and publishes the reading; a bridge that did not answer leaves the last reading where it was.
      Source: `src/presence_hue.rs:41 poll`, `src/presence_hue.rs:55 reading`,
      `src/presence_instant.rs:29 instant_from_utc`, `src/main.rs:5072-5110 presence_poll`.
      Pin: `a_bridge_that_did_not_answer_leaves_the_last_reading_where_it_was`
           at src/main.rs:9428
      also `an_instant_becomes_the_second_it_names_and_the_fraction_inside_it`
           at src/presence_instant.rs:133
      also `a_day_the_month_does_not_have_is_refused_rather_than_rolled_forward`
           at src/presence_instant.rs:177

S234. `classify` turns the reading into `PresenceStatus`: a known room with the age of its edge,
      `nowhere` for a fresh poll that found nobody, and five ways of not knowing (no reading, no clock,
      stale past `stale_after_secs`, a future epoch, an unwatched room).
      Source: `src/presence.rs:79-112 classify`, `src/presence.rs:40-49 PresenceStatus`.
      Pin: `a_known_room_is_named_with_the_age_of_its_motion_edge`
           at src/doctor.rs:810
      also `a_fresh_poll_that_found_nobody_says_nowhere_rather_than_unknown`
           at src/doctor.rs:821
      also `every_way_of_not_knowing_says_which_way_it_is`
           at src/doctor.rs:829

S235. The room presence reading is consumed by no delivery decision today; the in-flight presence
      policy branch (`pns-hue-presence-policy`) adds the snapshot at decide, the narrowing over the
      eligible lamps, the `presence-decisions` ring and the `desk_room` config bound.
      Source: `src/main.rs:2798-3104 run_event` (no presence read); the branch diff.
      Pin: UNPINNED on `main`; the branch carries its own tests.

## 11. The nag

S236. `[nag] after_secs` is the switch and the schedule: 30 to 3600 arms it, 0 and an absent table are
      off, anything else is refused by name; a config nobody can parse reads as off.
      Source: `src/config.rs nag_schedule`, `src/main.rs:4967 nag_after_secs`, `src/main.rs:4765
      NAG_OFF`.
      Pin: `the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error`
           at src/config.rs:2772
      also `a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name`
           at src/config.rs:2814

S237. `arm_nag` (claude only, after the forward starts, before the notification) unlinks this session's
      answered marker, publishes the record, and registers `nag:<session>` with `due = now +
      after_secs`, `until = due + after_secs`, `unless_marker nag-<session>`, args `["nag"]`; a refused
      registration drops the record and says so; an unsafe or over-long session id arms nothing.
      Source: `src/main.rs:4650-4749 arm_nag`, `src/nag.rs:119 job_id`, `src/nag.rs:112 marker_name`.
      Pin: `arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first`
           at tests/hooks.rs:3708
      also `an_approval_whose_nudge_could_not_be_scheduled_leaves_no_record_behind`
           at tests/hooks.rs:3781
      also `nothing_is_armed_when_nothing_should_be`
           at tests/hooks.rs:3824
      also `a_session_id_that_cannot_be_a_filename_names_nothing_at_all`
           at src/nag.rs:397
      also `arming_writes_nothing_the_harness_could_read_as_a_decision`
           at tests/hooks.rs:3919

S238. `clear_nag` writes the answered marker first, whether or not a record is there, then removes the
      record; a clear landing inside the fire's claim window still writes the marker.
      Source: `src/main.rs:4603-4649 clear_nag`.
      Pin: `an_answered_approval_is_never_nudged_by_either_clearing_signal`
           at tests/hooks.rs:3605
      also `a_clear_landing_inside_the_fires_claim_window_still_writes_the_marker`
           at tests/hooks.rs:3659

S239. The fire takes `nag/fire.lock` by exclusive create (losers say nothing and exit 0), claims each
      `.pending` record by rename before reading it, judges each as unreadable, answered, stale or
      counted, writes every answered marker before the card, delivers one card whatever the count,
      and removes the claims after; an unreadable record is named on stderr and dropped.
      Source: `src/main.rs:4401-4568 nag_mode`, `src/main.rs:4855 claim_fire`, `src/main.rs:4810
      claim_record`, `src/nag.rs:318 fate`, `src/nag.rs:278 is_stale`.
      Pin: `fires_racing_over_one_directory_still_produce_exactly_one_card`
           at tests/hooks.rs:3382
      also `three_unanswered_approvals_produce_one_card_that_says_three`
           at tests/hooks.rs:3314
      also `an_unanswered_approval_is_nudged_once_through_the_ordinary_paths`
           at tests/hooks.rs:3229
      also `a_second_fire_nudges_nothing`
           at tests/hooks.rs:3284
      also `a_record_is_counted_only_when_nothing_says_otherwise`
           at src/nag.rs:507

S240. Staleness is `armed > now || now > armed + 2 * after_secs`; a nudge is an ordinary delivery
      through `decide` as `Attempt::Nudge`, suppressible, and a suppressed nudge is lost.
      Source: `src/nag.rs:278 is_stale`, `src/nag.rs:222 nudge`, `src/main.rs:2798-3104 run_event`.
      Pin: `a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`
           at tests/hooks.rs:1733
      also `the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there`
           at tests/hooks.rs:3995

S241. The per-record rename in `claim_record` is not killed by any test; the code says so.
      Source: `src/main.rs:4810 claim_record`.
      Pin: UNPINNED. Kept on the measurement.

## 12. Missed notifications and the replay

S242. A returning event (surface not `Away`, plan raising a banner or a card, at least one decorative
      leg) claims the whole return moment once: rename `last-present` (or adopt a free stranded
      claim), read the edge off the claim, claim the journal, restore the edge at once, remove the
      claim; a racer that finds the moment held delivers no card of any kind and does not republish
      the edge.
      Source: `src/main.rs:1114-1241 replay_missed`, `src/main.rs:1284 Moment`, `src/main.rs:1335
      claim_moment`.
      Pin: `an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind`
           at tests/dispatch.rs:6531
      also `racing_present_events_recap_one_loud_window_exactly_once_between_them`
           at tests/dispatch.rs:6615
      also `the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`
           at tests/dispatch.rs:5148

S243. The replay delivers at most one card off the batch it claimed, as a dispatch and never a second
      event: the card is `missed_notifications::recap_card` over the entries (title `pns · missed`, a
      NEEDS YOU count), sent to the same legs the live event reached, and nothing is journaled or
      recorded for it.
      Source: `src/main.rs:1114-1241 replay_missed`, `src/missed_notifications.rs:338-372 recap_card`,
      `src/missed_notifications.rs:260-282 summary`.
      Pin: `a_present_event_delivers_one_extra_notification_carrying_the_whole_journal`
           at tests/dispatch.rs:4624
      also `the_recap_card_is_exactly_what_the_entries_compose_and_nothing_a_model_said`
           at tests/dispatch.rs:6134

S244. `[recap] replay_card = false` gates the card and never the journal; the doctor's wording changes
      with the switch.
      Source: `src/main.rs:1114-1241 replay_missed`, `src/main.rs:7602 missed_line`,
      `src/missed_notifications.rs:463-494 waiting_line`.
      Pin: `the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it`
           at tests/dispatch.rs:4432

S245. The window is `(since, until]` over the activity ring; a loud window (at least `min_events`,
      default 8) with a durable route spawns `pns recap --since --until` detached in its own process
      group with `PNS_REMOTE_TIMEOUT` defaulted to 30 only when unset, all three streams null, and
      nothing waits on it.
      Source: `src/main.rs:1263 activity_in`, `src/main.rs:1494-1516 spawn_recap`, `src/main.rs:1521
      RECAP_DEADLINE_SECS`.
      Pin: `events_stamped_at_the_markers_own_second_belong_to_it_and_not_to_the_window_after`
           at tests/dispatch.rs:6016
      also `an_activity_window_with_no_marker_to_open_it_recaps_nothing_and_still_catches_up`
           at tests/dispatch.rs:5939
      also `the_recap_child_runs_in_a_process_group_of_its_own`
           at tests/dispatch.rs:6262

## 13. The return recap

S246. `pns recap` reads the window off the activity ring (an unreadable ring is an empty window),
      reads the config once (a config that will not load names no route and no summarizer and forces
      `digest_as_thread` off), and composes a body under two budgets, 25 lines and 1,800 characters.
      Source: `src/main.rs:7752-7857 recap_mode`, `src/recap.rs:34 MAX_LINES`, `src/recap.rs:53
      MAX_CHARS`, `src/recap.rs:931 fit`.
      Pin: `the_body_opens_with_the_window_and_its_count_and_puts_needs_you_above_the_night`
           at src/recap.rs:1180
      also `a_window_too_long_for_the_budget_cuts_lines_and_never_a_count_or_a_needs_you`
           at src/recap.rs:1284
      also `a_worst_case_window_stays_inside_one_discord_message`
           at src/recap.rs:1347

S247. The header `While you were away, <from>-<to> · <n> event(s)` is composed before any cut; the
      NEEDS YOU section is never cut; the night is one `HH:MM <mark> <agent>/<state> <project>:
      <detail>` line per event, oldest first, and is the only section a summarizer may rewrite,
      prefixed `- ` per line and cut to the window's length.
      Source: `src/recap.rs:771 header`, `src/recap.rs:779 needs_you_section`, `src/recap.rs:822
      night_section`, `src/recap.rs:874 described`, `src/recap.rs:901 mark`.
      Pin: `the_night_is_oldest_first_one_line_per_event_and_marked_by_its_state`
           at src/recap.rs:1218
      also `a_needs_you_list_longer_than_the_whole_budget_is_still_never_cut`
           at src/recap.rs:1466
      also `a_summarized_line_that_reads_as_a_heading_cannot_render_as_one`
           at src/recap.rs:1582
      also `a_summarized_night_is_never_longer_than_the_window_it_summarizes`
           at src/recap.rs:1638
      also `the_windows_own_count_and_what_needs_you_survive_whatever_the_model_says`
           at tests/dispatch.rs:6804

S248. Merged pull requests come from one `gh pr list --repo <r> --state merged --search
      merged:<utc(since+1)>..<utc(until)> --json number,title,body --limit 50` per configured repo
      under 30 s and 512 KiB; any repo failing fails the whole section; no `[recap] repos` means no
      `gh` process at all.
      Source: `src/main.rs:7967 merged_pull_requests`, `src/main.rs:8162-8180 GH_LIMIT GH_DEADLINE
      GH_READ_MAX`.
      Pin: `a_configured_repositorys_merges_become_the_new_behavior_section`
           at tests/dispatch.rs:7127
      also `a_gh_that_will_not_answer_costs_the_recap_only_its_own_section`
           at tests/dispatch.rs:7207
      also `no_repos_key_means_no_gh_process_is_ever_started`
           at tests/dispatch.rs:7250

S249. Review notes are one directory listed once, regular files matching the glob's file-name part
      with mtime in `(since, until]`, newest first, at most 25, each opened `O_NOFOLLOW` and
      re-checked on the handle, read to 64 KiB; a note that will not open is said, not dropped; a
      glob pointing nowhere makes the section unavailable.
      Source: `src/main.rs:8055 notes_matching`, `src/main.rs:8138 read_note`, `src/main.rs:8183
      MAX_NOTES`, `src/main.rs:8185 NOTE_READ_MAX`.
      Pin: `only_the_notes_the_glob_names_and_the_window_covers_are_ever_read`
           at tests/dispatch.rs:7362
      also `a_glob_that_matches_nothing_says_so_and_one_pointing_nowhere_says_something_else`
           at tests/dispatch.rs:7413
      also `a_note_that_matched_and_would_not_open_says_so_rather_than_vanishing`
           at tests/dispatch.rs:7460

S250. One summarizer budget (`summarizer_deadline_secs`, default 240, max 3600, 0 means never
      spawned) is spent across up to three questions in order; the argv goes straight to `Command`;
      every failure falls to the plain list with `(The summarizer did not answer, so this is the
      plain list.)`; an answer over 16 KiB or carrying U+FFFD is refused whole; each line is
      sanitized and cut to 120.
      Source: `src/main.rs:8206 summarize`, `src/main.rs:7919 left_of`, `src/recap.rs:652 answer`,
      `src/recap.rs:689 safe_line`, `src/recap.rs:1107 MAX_ANSWER_BYTES`.
      Pin: `one_recap_spends_one_summarizer_budget_however_many_questions_it_asks`
           at tests/dispatch.rs:7567
      also `a_summarizer_that_exits_non_zero_falls_to_the_plain_list_and_says_so`
           at tests/dispatch.rs:6901
      also `a_summarizer_still_thinking_at_its_deadline_falls_to_the_plain_list_and_says_so`
           at tests/dispatch.rs:6924
      also `an_empty_window_says_so_itself_and_never_starts_a_summarizer_at_all`
           at tests/dispatch.rs:7040
      also `an_answer_past_the_byte_cap_is_refused_rather_than_composed_into_a_message`
           at src/recap.rs:1569
      also `an_answer_the_runner_had_to_repair_is_refused_rather_than_posted`
           at src/recap.rs:1830
      also `a_summarizer_answering_with_a_megabyte_gets_the_plain_list_posted_instead`
           at tests/dispatch.rs:7080

S251. An external section keeps only summarized lines that cite a fetched source as a whole token,
      one line per source, at most four, judged before any clip; the remainder counts the sources no
      line names.
      Source: `src/recap.rs:391 external_section`, `src/recap.rs:433 vouched`, `src/recap.rs:454
      cites`.
      Pin: `a_line_citing_no_merge_pns_fetched_is_dropped_and_counted_rather_than_posted`
           at src/recap.rs:1952
      also `a_receipt_with_anything_glued_to_either_end_names_a_different_source`
           at src/recap.rs:1985
      also `one_merge_vouches_for_one_line_and_the_rest_are_counted_as_missing`
           at src/recap.rs:2061
      also `a_line_the_width_would_have_cut_into_a_receipt_is_judged_as_it_came`
           at src/recap.rs:2023
      also `a_summarized_merge_section_keeps_only_the_lines_its_own_sources_vouch_for`
           at tests/dispatch.rs:7513

S252. `is_invisible` is the whole Unicode 17.0 Cf category, checked against an independent table for
      every code point; `safe_line` drops control and format characters whole.
      Source: `src/recap.rs:730 is_invisible`, `src/recap.rs:689 safe_line`.
      Pin: `is_invisible_agrees_with_unicode_17_0_across_every_code_point`
           at src/recap.rs:1773
      also `a_summarizers_line_cannot_carry_an_invisible_or_a_reordering_character`
           at src/recap.rs:1745
      also `the_arabic_letter_mark_is_stripped_like_every_other_format_character`
           at src/recap.rs:1757

S253. The recap posts to the `pns-recap` route with exactly one fallback to the default route, and
      the wall clock is one `HH:MM` function with `--:--` for an unreadable moment.
      Source: `src/main.rs:8295 post_recap`, `src/main.rs:8325 deliver_recap`, `src/main.rs:8369
      RECAP_ROUTE`, `src/main.rs:8253 wall_clock`, `src/main.rs:8381 NO_WALL_CLOCK`.
      Pin: `a_recap_the_thread_route_will_not_take_falls_back_to_the_default_and_says_so`
           at tests/native.rs:353
      also `a_recap_the_gateway_refused_says_so_out_loud_and_still_exits_zero`
           at tests/native.rs:302

S254. The `--:--` placeholder, the summarizer child's inherited environment, and a forking `gh` or
      summarizer are pinned by nothing.
      Source: `src/main.rs:8206 summarize`, `src/system.rs:76-148 run_bounded`.
      Pin: UNPINNED. Recorded in `docs/specs/return-recap.md`.

## 14. The doctor

S255. `pns doctor` prints the fixed opening sentence, one line per REGISTRATION in roster order (a
      selected sensor is a skip or a presence reading, hue is a pulse, the three channels are sends),
      a summary `pns doctor: <n> sent, <n> failed, <n> skipped`, then the pairing, Focus, daemon, nag,
      lights, decision ring and journal sections.
      Source: `src/main.rs:4147-4376 doctor_mode`, `src/main.rs:7723 DOCTOR_OPENING`,
      `src/doctor.rs:93 checks`, `src/doctor.rs:139 line`, `src/doctor.rs:212 summary`.
      Pin: `the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`
           at tests/dispatch.rs:3014
      also `the_check_list_holds_one_entry_per_registration_in_registration_order`
           at src/doctor.rs:897
      also `the_summary_counts_every_check_exactly_once`
           at src/doctor.rs:1053
      also `a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel`
           at src/doctor.rs:997

S256. The doctor bypasses every gate structurally (it calls `dispatch_legs`, never `decide`), sends
      `agent pns, state doctor`, the fixed detail, no pane, every leg `sync`, and its lamps, banner,
      card and hermes post are LIVE effects.
      Source: `src/main.rs:4147-4376 doctor_mode`, `src/main.rs:7736 DOCTOR_DETAIL`.
      Pin: `the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`
           at tests/dispatch.rs:3298
      also `the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`
           at tests/dispatch.rs:3353

S257. A leg's outcome is paired by NAME: `Delivered` is sent, `Failed` and `Unlaunched` are failed,
      `Silent` is `sent, this channel reports no outcome`; a leg absent from the deliveries is `FAILED,
      the leg was never dispatched`.
      Source: `src/main.rs:4147-4376 doctor_mode`, `src/doctor.rs:74 Outcome`.
      Pin: `a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`
           at tests/dispatch.rs:3267
      also `a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`
           at tests/dispatch.rs:3207

S258. The pulse check dials only when the hue table resolves (bridge and key both non-empty),
      otherwise `hue: FAILED, pulse SKIPPED -- no hue bridge and key in the config ([plugins.hue]
      bridge, key); nothing was signalled`; `Signalled(0)` is a failure naming both causes it cannot
      choose between.
      Source: `src/main.rs:3704 hue_resolves`, `src/main.rs:3768 pulse_outcome`, `src/main.rs:7731
      NO_HUE_BRIDGE_LINE`.
      Pin: `a_pulse_with_no_bridge_to_dial_names_the_settings_rather_than_the_rooms`
           at tests/dispatch.rs:3380
      also `a_pulse_the_bridge_answered_nothing_for_still_names_both_causes_it_cannot_choose_between`
           at tests/dispatch.rs:3409
      also `the_pulse_line_claims_neither_a_flash_nor_a_cause_it_cannot_know`
           at src/doctor.rs:1030

S259. Pairing spawns `moshi-hook status --json` (5 s) then `moshi-hook status` (8 s), never `probe`;
      reads only `paired`, `hostId`, `displayName`; refuses an answer over 1 MiB before parsing (the
      reader stops at 2 MiB); relays the one `server:` line at column zero as `pns doctor: moshi
      says: <text>`; and filters every relayed value to printable ASCII, 200 characters.
      Source: `src/main.rs:7505 read_pairing`, `src/main.rs:7539 PAIRING_READ_MAX`, `src/doctor.rs:301
      pairing_report`, `src/doctor.rs:340 ANSWER_MAX`, `src/doctor.rs:349 server_said`,
      `src/doctor.rs:401 printable`, `src/doctor.rs:410 RELAY_MAX`.
      Pin: `the_doctor_runs_moshi_hook_exactly_twice_and_never_probes`
           at tests/dispatch.rs:7697
      also `a_moshi_hook_that_never_returns_does_not_park_the_doctor`
           at tests/dispatch.rs:7752
      also `an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read`
           at tests/dispatch.rs:7919
      also `a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line`
           at src/doctor.rs:1434
      also `an_identity_moshi_named_cannot_forge_a_report_line_either`
           at src/doctor.rs:1470
      also `an_over_long_relayed_value_stops_at_the_cap`
           at src/doctor.rs:1504
      also `only_a_server_line_at_column_zero_is_relayed`
           at src/doctor.rs:1376

S260. The Focus line is one of five sentences plus an optional catalog clause; the daemon line one of
      six states graded by heartbeat age; the nag line one of two; the lights section one of six
      states; none moves the exit code.
      Source: `src/main.rs:7639 focus_line`, `src/main.rs:7693 daemon_line`, `src/doctor.rs:648
      daemon_line`, `src/doctor.rs:710 nag_line`, `src/doctor.rs:484 lights_lines`.
      Pin: `the_doctor_tells_the_truth_about_a_named_focus_in_every_state`
           at tests/dispatch.rs:7968
      also `a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health`
           at tests/dispatch.rs:8042
      also `the_daemons_doctor_line_tells_the_truth_in_four_states`
           at src/doctor.rs:1677
      also `a_daemon_switched_off_but_still_beating_is_reported_as_still_beating`
           at src/doctor.rs:1720
      also `the_nag_line_names_the_schedule_or_says_the_feature_is_off`
           at src/doctor.rs:725
      also `the_lights_section_says_which_of_its_six_states_the_config_is_in`
           at src/doctor.rs:1111
      also `every_lights_state_says_something_rather_than_printing_nothing`
           at src/doctor.rs:1192

S261. The decision section renders the last five ring entries newest first with control bytes
      escaped, and the journal line COUNTS entries and never parses one; the doctor writes nothing to
      its state directory, appends no decision and journals nothing.
      Source: `src/main.rs:7569 decision_section`, `src/main.rs:7602 missed_line`,
      `src/decision_log.rs:261-276 section`, `src/missed_notifications.rs:463-494 waiting_line`.
      Pin: `the_doctor_prints_the_decision_section_after_its_summary_newest_first`
           at tests/dispatch.rs:3959
      also `the_doctor_records_no_decision_of_its_own`
           at tests/dispatch.rs:4124
      also `the_doctor_leaves_the_journal_exactly_as_it_found_it`
           at tests/dispatch.rs:4538
      also `a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`
           at tests/dispatch.rs:4467
      also `an_unreadable_entry_is_quoted_short_and_with_its_control_bytes_escaped`
           at src/decision_log.rs:897

S262. The doctor reports nothing about the operator's own `pns quiet` mute.
      Source: `src/main.rs:4147-4376 doctor_mode` (no reader of `quiet-until`).
      Pin: UNPINNED. An established gap, not a missing test.

## 15. Setup

S263. The walk prints the preamble and asks between six and fifteen prompts in file order, each
      feature's credentials right after the question that armed it; every answer is trimmed, only a
      typed `y`/`yes` (any case) arms, a comma list keeps only non-empty values, and a blank
      credential declines its feature and says so.
      Source: `src/main.rs:8554 walk`, `src/main.rs:8854 means_yes`, `src/main.rs:8872 list`,
      `src/main.rs:8637 nothing_given`, `src/main.rs:9081 SETUP_PREAMBLE`.
      Pin: `the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`
           at src/main.rs:12307
      also `a_comma_separated_answer_names_only_the_values_somebody_typed`
           at src/main.rs:12338
      also `an_answer_of_nothing_but_spaces_is_a_blank_one`
           at src/main.rs:12322
      also `a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`
           at src/setup.rs:330

S264. The four secret prompts arm `Hushed` before printing: echo off, `ECHONL` on, nine signals held,
      restored on drop in that order; a secret never reaches the pty transcript and always reaches
      the published 0600 file.
      Source: `src/main.rs:8670 ask_hidden`, `src/main.rs:8727-8828 Hushed`.
      Pin: `a_secret_typed_into_setup_never_reaches_the_pty_output`
           at tests/setup.rs:288
      also `a_signal_sent_during_the_hidden_read_is_held_until_the_guard_drops`
           at tests/setup.rs:422

S265. A read from the background (`EIO` with the terminal owned by another group) says `bring it to
      the foreground with fg`; a non-UTF-8 paste is `the answers could not be read: ...`; a closed
      input is `the answers ended before the walk did`.
      Source: `src/main.rs:8678 read_answer`, `src/main.rs:8711 read_failure`, `src/main.rs:8692
      reading_from_the_background`.
      Pin: `a_background_read_names_job_control_rather_than_an_io_fault`
           at src/main.rs:12601
      also `a_non_utf8_paste_is_reported_as_a_read_failure_rather_than_the_answers_ending`
           at tests/setup.rs:249

S266. The only backend the walk accepts is `unifi` (Enter or any case spelling); anything else
      declines the home probe and the walk continues.
      Source: `src/main.rs:8866 router_backend`, `src/home.rs UNIFI_TYPE`.
      Pin: `the_only_backend_the_walk_accepts_is_one_the_home_probe_answers`
           at src/main.rs:12353
      also `a_backend_the_home_probe_cannot_answer_declines_the_probe_rather_than_arming_it`
           at src/setup.rs:377

S267. Composition is `compose_config(&Answers)`, pure, rendered through `config_text::render`; a
      declined walk still writes the core live (banner and mobile enabled, `type = "moshi"`), every
      declined table commented out, no chezmoi action, every default asserted against the code's own.
      Source: `src/setup.rs:141 compose_config`, `src/setup.rs:46 Answers::values`.
      Pin: `a_walk_that_armed_nothing_still_writes_the_core`
           at src/setup.rs:222
      also `the_values_it_writes_unprompted_are_the_ones_the_code_defaults_to`
           at src/setup.rs:258
      also `a_skipped_token_is_commented_out_rather_than_written_empty`
           at src/setup.rs:276
      also `every_armed_feature_reaches_the_parsed_config_carrying_its_own_answers`
           at src/setup.rs:287
      also `a_wizard_render_carries_no_chezmoi_action_because_every_answer_is_a_literal`
           at src/setup.rs:492
      also `a_credential_carrying_quotes_and_backslashes_reaches_the_config_as_itself`
           at src/setup.rs:418

S268. The composed text goes through `parse_config` before anything is written; a refusal prints
      `pns setup: what it composed does not load (<detail>); nothing was written` and exits 2.
      Source: `src/main.rs:8434-8542 setup_mode`.
      Pin: UNPINNED end to end; the unit tests parse both ends of the walk on every run.

S269. The backup is `config.toml.<UTC stamp>.backup`, moved (never copied) before the new file is
      linked, chmodded 0600 only when a regular file; no clock means no backup and a refusal; a
      same-second collision refuses; a symlinked config moves the link, not its target.
      Source: `src/main.rs:9001 keep_aside`, `src/main.rs:9019 keep_aside_at`, `src/setup.rs:156
      backup_path`.
      Pin: `the_backup_sits_beside_the_config_stamped_with_the_instant_it_was_moved`
           at src/setup.rs:506
      also `a_clock_that_cannot_be_read_names_no_backup_at_all`
           at src/setup.rs:519
      also `a_forced_run_keeps_the_config_it_replaced_rather_than_what_that_config_named`
           at src/main.rs:12532
      also `a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside`
           at src/main.rs:12481
      also `a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace`
           at src/main.rs:12693

S270. The `also_kept` tail (backup written, link failed, config path empty) has no test.
      Source: `src/main.rs:8983 also_kept`.
      Pin: UNPINNED. Recorded in `docs/specs/setup-and-publication.md`.

## 16. The home probe

S271. `[plugins.router]` must be enabled with `type = "unifi"`, a non-empty `router_url`, an `api_key`,
      and at least one of `device_mac` (normalized to lowercase colon form), `device_hostname` (exact)
      or `device_ipv4` (parsed); every way it is not set up prints its own `home:` line and reads
      nothing.
      Source: `src/home.rs router_settings`, `src/home.rs device_identity`, `src/home.rs
      router_api_key`, `src/home.rs SetupFailure`.
      Pin: `every_way_the_home_probe_is_not_set_up_says_which_one_it_is`
           at tests/dispatch.rs:1400
      also `an_enabled_unifi_router_table_yields_its_url_and_device`
           at src/home.rs:1525
      also `a_router_table_naming_no_device_at_all_is_refused_naming_every_key`
           at src/home.rs:1572
      also `a_well_formed_mac_in_any_case_or_separator_validates_to_one_spelling`
           at src/home.rs:1713
      also `a_malformed_device_mac_is_refused_naming_the_key_and_quoting_the_value`
           at src/home.rs:1680

S272. The UniFi adapter GETs `/proxy/network/integration/v1/sites` then `.../sites/<id>/clients?
      limit=200` with `X-API-KEY`, 5 s per call, 1,000,000 bytes per body, no redirect followed, TLS
      verification disabled; the site id must be hex digits and dashes; a body that is not JSON, has no
      `data`, or is an incomplete page is no answer.
      Source: `src/home.rs UniFiRouter`, `src/home.rs first_site_id`, `src/home.rs parse_clients`,
      `src/home.rs ROUTER_DEADLINE`, `src/home.rs ROUTER_BODY_CAP`.
      Pin: `the_adapter_sends_the_key_and_walks_sites_then_clients`
           at src/home.rs:2401
      also `an_id_that_could_escape_the_url_path_is_refused_outright`
           at src/home.rs:1875
      also `a_redirecting_router_is_never_followed_and_reads_unknown`
           at src/home.rs:2428
      also `a_body_past_the_cap_reads_unknown_rather_than_being_swallowed`
           at src/home.rs:2450
      also `a_page_the_phone_could_be_beyond_is_no_answer_rather_than_not_home`
           at src/home.rs:1818

S273. Any configured key matching any client reads `Home` (the strongest matched names it); a
      complete listing no key matched is the only `NotHome`; every failure to read is `Unknown`, never
      `NotHome`; the evidence is one line per configured key in precedence order with another
      client's label escaped.
      Source: `src/home.rs home_reading`, `src/home.rs read_home`, `src/home.rs report`.
      Pin: `one_reading_runs_fetch_parse_judge_in_order`
           at src/home.rs:1410
      also `any_one_configured_key_matching_reads_home_while_the_others_match_nothing`
           at src/home.rs:1079
      also `a_complete_listing_no_key_matched_is_not_home_and_anything_less_is_unknown`
           at src/home.rs:1367
      also `a_reading_carries_one_entry_per_configured_key_in_precedence_order`
           at src/home.rs:1157
      also `the_evidence_under_the_verdict_says_what_each_key_found_escaping_the_label`
           at src/home.rs:2175

S274. A Home verdict with a key pointing elsewhere is a staleness, said in the terminal and delivered
      as one alert (`agent pns, state stale`, key names only, to `stale_alert_channel` or the default
      route) once per episode identity; duplicates are the direction to fail in.
      Source: `src/home.rs stale_identifiers`, `src/home.rs stale_warning`, `src/home.rs
      stale_alert_channel`, `src/main.rs:4012-4126 home_mode`.
      Pin: `a_staleness_is_a_home_verdict_with_a_key_pointing_somewhere_else`
           at src/home.rs:1891
      also `a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence`
           at tests/dispatch.rs:1194
      also `the_same_stale_state_alerts_once_and_a_returning_one_alerts_again`
           at tests/dispatch.rs:1235
      also `the_alert_carries_no_secret_and_no_raw_router_text`
           at tests/dispatch.rs:1321
      also `an_unusable_stale_alert_route_complains_and_still_delivers_the_alert`
           at tests/dispatch.rs:1367
      also `the_stale_alert_posts_to_the_hermes_route_the_config_named`
           at tests/native.rs:266

## 17. Configuration and rendering

S275. `TABLE_KEYS` is the schema's one statement, checked in both directions: every declared key is
      read by the arm that declares it, every table refuses an unknown key by name listing what it
      serves, and the renderer's `LAYOUT` matches the roster exactly.
      Source: `src/config.rs:524-624 TABLE_KEYS`, `src/config_text.rs:55 LAYOUT`.
      Pin: `every_key_the_roster_declares_is_read_by_the_table_that_declares_it`
           at src/config.rs:3998
      also `every_table_refuses_an_unknown_key_by_name_and_lists_what_it_serves`
           at src/config.rs:3729
      also `every_layout_table_matches_the_config_roster_exactly_in_both_directions`
           at src/config_text.rs:1708

S276. A loaded config is authoritative and an absent `enabled` reads false; `[daemon] enabled`
      defaults on; `[nag]` defaults off; `[recap]` defaults `replay_card`, `digest`,
      `digest_as_thread` true, `min_events` 8, `summarizer_deadline_secs` 240 (max 3600), no repos, no
      notes; `[focus] silence` is a list of names.
      Source: `src/config.rs:832-872 Config`, `src/config.rs Recap`, `src/config.rs:932-1009
      parse_config`.
      Pin: `the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name`
           at src/config.rs:2718
      also `the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error`
           at src/config.rs:2772
      also `recap_defaults_are_asserted_against_the_code_rather_than_copied_literals`
           at src/config_text.rs:1258

S277. `[lights]` absent is `None`; a bare `[lights]` is every locked default (`refresh_secs` 12 in 10 to
      30; pulses 4000 ms at 100; blocked 2000 ms 100/30 with `give_up_after_secs` 57,600; unread
      4000 ms 60/10 after 300; loop 4000 ms 80/10 with a 200 ms flare to 100, threshold 300, lease
      3900; dim 3000 ms 7/1); every number is bounded on both sides and refused by name; `low` above
      `high` is refused; a knob that does not apply to a behaviour does not exist on it.
      Source: `src/config.rs Lights`, `src/config.rs bounded`, `src/config.rs percent`,
      `src/config.rs ends_agree`.
      Pin: `no_lights_table_is_none_and_an_empty_one_is_every_locked_default`
           at src/config.rs:3047
      also `every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them`
           at src/config.rs:3186
      also `a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down`
           at src/config.rs:3330
      also `a_knob_that_does_not_apply_to_a_behaviour_does_not_exist_on_it`
           at src/config.rs:3155
      also `a_behaviour_table_moves_the_keys_it_states_and_leaves_the_rest_at_their_locked_values`
           at src/config.rs:3116

S278. `[lights.lamp|room|zone.<name>]` declarations read exactly `shows`, `dim_window` and
      `dim_behaviours`; `shows = []` is an override and an absent `shows` inherits; `dim_behaviours`
      without a `dim_window` is refused.
      Source: `src/config.rs parse_targets`, `src/config.rs TARGET_KEYS`.
      Pin: `an_unknown_declaration_key_is_refused_by_name_with_the_path_the_operator_wrote`
           at src/config.rs:3543
      also `a_declaration_at_any_of_the_three_levels_reads_the_same_three_keys`
           at src/config.rs:3449
      also `a_declaration_that_states_nothing_states_nothing_rather_than_defaulting`
           at src/config.rs:3477
      also `dim_behaviours_with_no_window_to_run_them_in_is_refused_rather_than_read_and_dropped`
           at src/config.rs:3514

S279. `[plugins.presence]` (type `hue`, `rooms`, `exclude`, `poll_secs` 2 to 60 default 5,
      `stale_after_secs` default 15 and at least `poll_secs`) parses into `Presence`; a refused table
      is `pns: config error (<detail>); the room sensor is unread`.
      Source: `src/config.rs:1944-2024 parse_presence`, `src/config.rs Presence`.
      Pin: `a_rendered_presence_block_parses_back_and_the_registry_selects_the_sensor`
           at src/config_text.rs:1671

S280. `type` is the one word that selects a backend under every table that has one; the retired
      `brand`, `phone` and top-level `[home]` spellings are refused by name.
      Source: `src/config.rs parse_config`, `src/config.rs TABLE_KEYS`.
      Pin: `type_is_the_word_that_selects_a_backend_and_the_old_brand_is_refused`
           at src/config.rs:3706

S281. `render(values)` walks `LAYOUT` once: core tables live, opt-in tables commented when absent,
      leftovers refused by name; a secret marker `{ keepassxc = "<entry>", field = "<Field>" }` with
      the field `Password` or `UserName` renders as `{{ (keepassxc "<entry>").<Field> | toToml }}`
      with no author quotes; a literal is
      quoted with `{` and `}` escaped; a `note` becomes commented lines and is refused when it opens
      an action or carries a control character.
      Source: `src/config_text.rs:736-777 render`, `src/config_text.rs:1062 secret_action`,
      `src/config_text.rs:1041 SECRET_FIELDS`, `src/config_text.rs:705 quoted`,
      `src/config_text.rs:959 take_note`.
      Pin: `render_walks_every_layout_table_and_writes_no_heading_outside_it`
           at src/config_text.rs:1124
      also `an_opt_in_table_absent_renders_commented_and_present_renders_live`
           at src/config_text.rs:1640
      also `a_secret_marker_renders_as_the_chezmoi_action_and_a_literal_renders_quoted`
           at src/config_text.rs:1344
      also `a_username_secret_marker_renders_the_exact_action_and_round_trips_through_the_stub`
           at src/config_text.rs:1383
      also `a_hostile_literal_crosses_as_one_inert_string_and_never_as_structure`
           at src/config_text.rs:1895
      also `a_literal_holding_a_chezmoi_action_opening_crosses_with_its_braces_broken_up`
           at src/config_text.rs:1921
      also `a_note_holding_a_chezmoi_action_opening_is_refused_by_name`
           at src/config_text.rs:1948
      also `a_note_holding_a_newline_stays_commented_on_every_line`
           at src/config_text.rs:1551

S282. `pns-config-render <values> <template>` reads, refuses a literal at any of five secret-bearing
      keys by path (never echoing the value), renders, stubs the chezmoi actions and self-parses, and
      only then writes `BANNER + render + FOOTER` with a plain `std::fs::write`; any other argument
      count prints usage and exits 2; a refusal leaves a pre-existing template byte-identical.
      Source: `src/bin/pns-config-render.rs:50-116`.
      Pin: `a_values_file_that_renders_something_the_parser_rejects_is_refused_without_writing`
           at tests/config_render.rs:70
      also `a_literal_value_at_any_secret_bearing_key_is_refused_without_writing`
           at tests/config_render.rs:98
      also `an_unknown_values_entry_is_refused_without_writing`
           at tests/config_render.rs:141
      also `the_written_template_starts_with_the_generated_banner_and_the_darwin_wrapper`
           at tests/config_render.rs:168
      also `running_the_binary_twice_against_the_same_values_file_writes_identical_bytes`
           at tests/config_render.rs:198
      also `the_binary_over_the_committed_values_file_writes_the_committed_template_exactly`
           at tests/config_render.rs:232
      also `missing_arguments_print_usage_and_exit_2`
           at tests/config_render.rs:256
      also `a_third_argument_prints_usage_and_exit_2`
           at tests/config_render.rs:267

S283. The shipped template is pinned three ways from inside the crate against files four directories
      up: byte equality with `render` over the committed values, the 22 live table headings, and the
      resolved-config snapshot; the pins have to leave the crate (decision 0011).
      Source: `src/config.rs SHIPPED_TEMPLATE`, `src/config.rs CONFIG_VALUES`, `src/config.rs
      LIVE_TABLES`, `src/config.rs RESOLVED_CONFIG_SNAPSHOT`.
      Pin: `the_committed_template_is_render_over_the_committed_values_file`
           at src/config.rs:4060
      also `every_table_the_operator_runs_is_still_live_in_the_shipped_template`
           at src/config.rs:4135
      also `the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot`
           at src/config.rs:4207
      also `the_shipped_template_states_the_blocked_backstop_at_its_default_uncommented`
           at src/config.rs:4349

S284. A broken config fails OPEN on the delivery path (core plugins, loud) and CLOSED on every lamp path
      (no pulse, no tick arm, every mute refused by name), enabled on the daemon switch, and 5 s on the
      submit deadline.
      Source: `src/registry.rs:368-392 select_plugins`, `src/main.rs:3952 pulse_mode`,
      `src/main.rs:5742 lights_tick`, `src/main.rs:7222 daemon_enabled`, `src/main.rs:2587
      configured_submit_deadline`.
      Pin: `one_typod_table_name_costs_a_configured_machine_no_channel`
           at tests/dispatch.rs:744
      also `a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`
           at tests/dispatch.rs:781
      also `the_tick_exits_zero_with_no_config_no_table_hue_off_and_an_unreachable_bridge`
           at tests/dispatch.rs:8367

S285. `load_config` has no read deadline, and there is no config version key.
      Source: `src/config.rs:1838-1855 load_config`, `src/config.rs:524-624 TABLE_KEYS`.
      Pin: UNPINNED. Two open questions for the configuration step.

## 18. Counts

Computed over this file on 2026-09-05 (`grep -c '^S[0-9][0-9][0-9]\. '` and
`grep -c '^      Pin: UNPINNED'`):

| Count                                          | Value |
| ---------------------------------------------- | ----- |
| Statements                                     | 285   |
| Pinned by at least one test                    | 243   |
| UNPINNED                                       | 42    |
| Test references (a test may pin many statements) | 766   |
| Distinct Rust tests referenced                 | 699   |

The crate's own register, `dot_local/share/pns/docs/specs/unpinned-behaviors.md`, lists 79 test-gap
rows and 26 open-question rows harvested from the seventeen area specifications; the UNPINNED
statements above are the subset that reach the interface this inventory describes, plus the
declarations (the LaunchAgent, the hook table, the justfile recipes) that the 2026-08-05 testing
ruling keeps out of test scope on purpose. Every UNPINNED statement a plan step moves gets its test
first, against the code where it lives, in the pull request before the move.
