# The test baseline, and how a later step diffs against it

`docs/test-baseline.tsv` is the recorded state of this crate's test suite on 2026-09-02, at `origin/main`
commit `413eb8d0`, before any refactoring work. It is a SET OF NAMES with results, and it is deliberately
not a count.

A count is worthless as a safety net here. It passes when one test is dropped and another added, and it
passes a rename, which is exactly how a behavioral contract goes missing during a refactor without anyone
noticing. This program has already been bitten by that.

## The shape of the file

Three tab separated columns, with a header row, sorted by target and then by test name:

```
target	test	result
src/lib.rs	args::tests::a_bare_flag_is_not_given_a_value	ok
tests/dispatch.rs	racing_present_events_adopt_one_stranded_claim_exactly_once	ignored
```

1,257 rows. 1,256 `ok` and 1 `ignored`.

| Target                         | Tests |
| ------------------------------ | ----- |
| `src/lib.rs`                   | 748   |
| `src/main.rs`                  | 70    |
| `tests/dispatch.rs`            | 212   |
| `tests/hooks.rs`               | 163   |
| `tests/daemon.rs`              | 22    |
| `tests/native.rs`              | 16    |
| `tests/setup.rs`               | 14    |
| `tests/config_render.rs`       | 8     |
| `src/bin/pns-config-render.rs` | 4     |

`src/bin/http-capture.rs` builds a test target that contains no tests, so it contributes no rows.

The one ignored test is `tests/dispatch.rs:racing_present_events_adopt_one_stranded_claim_exactly_once`,
marked `#[ignore = "soak: a probabilistic hunt, roughly one catch in 200 rounds"]`. It stays ignored and
stays in the baseline, because a soak that is deleted rather than skipped is a contract that quietly
stopped existing.

Some names appear under more than one target. The seven `support::guard_tests::*` names are compiled into
every integration target, so the pair (target, test) is the key, not the test name alone.

## How the file was produced

The result column came from a full run, which passed:

```
cargo test --locked --workspace --manifest-path dot_local/share/pns/Cargo.toml
```

The NAME column did not come from that run's output, and this matters. A test that prints to standard
output can interleave with the harness's own `test <name> ... ok` lines, and on this suite it did: a
speed-guard warning merged with the following result line and produced a name that no test has. Names are
therefore taken from the deterministic listing:

```
cargo test --locked --workspace --manifest-path dot_local/share/pns/Cargo.toml -- --list
```

Regenerate with the same two commands. Take names from `--list` and results from the run, never names
from the run. Both carry `--workspace` because pns is a workspace now: without that word cargo covers
the root package alone and drops every member crate's tests from the regenerated baseline, silently.

## How a later step diffs against it

The comparison is on NAMES, in both directions, and the target column is informational because it will
change: the workspace conversion moves these tests into per-crate targets, and the crate name will appear
in place of `src/lib.rs`.

```
cargo test --locked --workspace -- --list \
  | sed -n 's/: test$//p' | sort -u > after.txt
cut -f2 docs/test-baseline.tsv | tail -n +2 | sort -u > before.txt
diff before.txt after.txt
```

Every line the diff reports is answered in the pull request that caused it, in a table with one row per
name:

| Baseline name | Successor name | Category | Reason |
| ------------- | -------------- | -------- | ------ |

- A test that survives unchanged needs no row.
- A test that is RENAMED gets a row naming its successor. This is the case the count would have missed.
- A test that is SPLIT gets one row per successor.
- A test that is REMOVED gets a row with an empty successor and a reason drawn from the categories below.
  "It tested the old mechanism" is a reason only when the specification it was protecting is named, and
  is shown to be covered elsewhere.

A removal with no row is a regression in the review, not in the code, and is treated as one.

## The other half of the safety net

The name set proves a move dropped no TEST. It cannot prove a move dropped no BEHAVIOR, because a
behavior that no test pins leaves the set unchanged when it breaks.
`docs/specs/unpinned-behaviors.md` is the list of those, and the rule that goes with it: before a later
pull request moves the code behind one of them, it writes the missing test first, against the code in its
current location, and lands it before the move. That list is the known minimum rather than a complete
one, so the per-behavior mutation check stays required whether or not a behavior appears on it.

## Test classification

This is step 3 of the refactoring procedure. It classifies at the level of target and module, with the
individually identified exceptions named. Classifying 1,257 tests one by one would go stale on the first
pull request and would pretend to a precision the refactor has not earned yet, so what follows is the
criterion plus the exceptions found while writing the specifications.

### The criterion

Ask what a test would still be pinning after the mechanism beneath it is replaced.

1. **Permanent behavioral contract.** It pins something observable from outside: an exit code, exact
   operator-facing wording, a fail direction, a threshold and the step either side, an idempotency
   guarantee, a privacy guarantee, a process-cleanup guarantee. It survives the refactor, under its own
   name or under a named successor.
1. **Adapter contract.** It pins one adapter's behavior against controlled infrastructure: a scripted
   transport, a temporary directory, an exact argv, a fixture. It survives, and moves to the crate that
   ends up owning that adapter.
1. **Obsolete implementation-mechanism test.** It pins HOW the current implementation reaches a result,
   where the refactor deliberately replaces the mechanism. It is removed, and its row names the
   specification that still covers the behavior.
1. **Migration test.** It exists to prove a transition: legacy state being swept, a configuration
   migration, or a pin that has to leave this crate.

### Group assignment

| Group                                                                                                                                                                                                                                            | Rows             | Category                      | Note                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/dispatch.rs` root tests                                                                                                                                                                                                                   | 205              | permanent behavioral contract | Black box against the built binary. The program requires this mega-suite be split BY BEHAVIOR, never into `part1` and `part2`                                 |
| `tests/hooks.rs` root tests                                                                                                                                                                                                                      | 156              | permanent behavioral contract | Same, and it carries the hook compatibility surface                                                                                                           |
| `tests/daemon.rs` root tests                                                                                                                                                                                                                     | 15               | permanent behavioral contract | Job, claim and lease behavior across processes                                                                                                                |
| `tests/native.rs` root tests                                                                                                                                                                                                                     | 9                | adapter contract              | The compiled-in destinations, which the dispatch suite deliberately does not reach                                                                            |
| `tests/setup.rs` root tests                                                                                                                                                                                                                      | 7                | permanent behavioral contract | Terminal echo restoration and publication safety are operator-safety contracts                                                                                |
| `tests/config_render.rs`                                                                                                                                                                                                                         | 8                | mixed, see exceptions         | The renderer's own behavior is permanent; the one test pinning the dotfiles template is a migration test                                                      |
| `support::guard_tests::*`                                                                                                                                                                                                                        | 7 names, 35 rows | adapter contract              | They test the speed guard in `tests/support/mod.rs`, which the program explicitly says to keep. They follow the guard wherever shared test support lands      |
| `src/lib.rs` pure-policy modules (`engine`, `surface`, `routing`, `presence`, `pulse`, `quiet`, `safety`, `render`, `decision_log`, `missed_notifications`, `nag`, `args`, `registry`, `lights`, `recap`, `daemon`, `setup`, and the crate root) | 354              | permanent behavioral contract | These are total functions of their arguments. They move into the domain crate largely unchanged, which is the cheapest evidence the refactor preserved policy |
| `src/lib.rs` edge modules (`system`, `home`, `config`, `config_text`, `doctor`, `focus`, `hooks`, `channels::*`)                                                                                                                                 | 394              | adapter contract              | Each is a seam over a real external thing, tested against a stub or a fixture                                                                                 |
| `src/main.rs` unit tests                                                                                                                                                                                                                         | 70               | mixed, see exceptions         | The composition root's private file protocols. This is where the obsolete-mechanism candidates concentrate                                                    |
| `src/bin/pns-config-render.rs`                                                                                                                                                                                                                   | 4                | permanent behavioral contract | The secret-marker refusals, which are a safety contract                                                                                                       |

### Named exceptions

**Migration tests, leaving or proving a transition.**

- The five tests that reach four directories above the crate into the dotfiles checkout, to pin
  `dot_config/pns/private_config.toml.tmpl` against `dot_config/pns/config-values.toml`. Four are unit
  tests in `src/config.rs` reading `include_str!("../../../../dot_config/pns/...")`, and one is
  `tests/config_render.rs:the_binary_over_the_committed_values_file_writes_the_committed_template_exactly`
  building the same paths from `CARGO_MANIFEST_DIR`. It is a dotfiles concern, not a pns concern, and a
  standalone crate cannot carry it. It moves out to a test under `test/` that runs the built renderer,
  and pns keeps its renderer tests against fixtures it owns. See
  `docs/decisions/0011-the-shipped-template-is-pinned-from-outside-the-crate.md`, which records the two
  properties the move must not lose.
- `src/main.rs:tests::the_first_tick_sweeps_the_state_the_old_names_held`, which proves the legacy
  `lights-glow`, `lights-working-since` and `lights-needs` entries are removed. It stays as long as the
  sweep is deployed. See `docs/decisions/0004-the-unread-lamp-and-the-glow-it-replaced.md`.

**Candidates for obsolete implementation-mechanism, decided per pull request, not now.**

The persistence step replaces internal durable multi-record state with a transactional store. Where that
happens, the tests that pin the filesystem protocol beneath it become mechanism tests. Where it does not
happen, because the path, name, mode or existence is itself an external interface, they stay permanent
contracts and their race behavior must still be tested.

The decision is per state family, not per test, and it is made when that family moves. What must NOT be
lost in either case is the behavior these tests actually protect:

- ownership taken by rename or exclusive creation rather than by removal
  (`docs/decisions/0001-ownership-by-rename-not-by-unlink.md`),
- a stale hold being reclaimable,
- a claim being taken at most once under contention,
- a crash between two steps leaving a recoverable state rather than a duplicate delivery.

A pull request that deletes such a test and cannot point at where those four are still pinned has found a
gap, not an obsolete test.

**Not a test, and not to be written.** A file-size check is not a test. Tests here pin the behavior of
tools we wrote (operator ruling, 2026-08-05), and a meta-test about code shape is deleted on sight. The
file-size command is run in the completion report instead.

## File-protocol extraction: steps 11.1, 11.2 and 11.5

Every predecessor leaf name remains. The table below maps the tests whose source location changed;
unlisted names keep their existing location and classification. The unchanged names include the
inherited harness speed guards, which remain harness checks rather than product behavior contracts.
Their later test-layout owner remains step 16. No test is removed by this batch.

`the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` retains its historical name,
but its interrupted arm deliberately changes: completed delivery still consumes the hold, while
killing the engine inside replay dispatch now preserves one complete held journal. The near-edge
restoration assertion remains. A separate deletion-before-outcome mutant fails the new held-batch
assertion, so a stable name here is not claimed as evidence of unchanged behavior.

| Leaf name (unchanged) | Previous source | Current source | Category |
| --- | --- | --- | --- |
| `a_bare_token_on_disk_still_reads_as_a_held_lamp_with_no_phase` | `src/lights_state_runtime/tests.rs` | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs` | Adapter contract, moved with its behavior |
| `a_claim_that_fails_for_another_reason_is_not_blamed_on_a_same_second_run` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_clock_that_cannot_be_read_names_no_backup_at_all` | `src/setup.rs` | `crates/pns-adapters/src/protocols/config_publication/backup_name.rs` | Adapter contract, moved with its behavior |
| `a_complaint_that_cleared_is_forgotten_so_its_return_is_news_again` | `src/lights_state_runtime/tests.rs` | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs` | Adapter contract, moved with its behavior |
| `a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_due_outside_a_bounded_window_of_now_is_refused_at_registration` | `src/daemon/record_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs` | Adapter contract, moved with its behavior |
| `a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_forced_run_keeps_a_config_the_existence_check_reads_as_absent` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_forced_run_keeps_the_config_it_replaced_rather_than_what_that_config_named` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_held_record_that_is_absent_holds_nothing_and_one_that_will_not_read_holds_everything` | `src/lights_state_runtime/tests.rs` | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs` | Adapter contract, moved with its behavior |
| `a_held_records_phase_round_trips_through_remember_held_and_read_held` | `src/lights_state_runtime/tests.rs` | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs` | Adapter contract, moved with its behavior |
| `a_hostile_detail_still_produces_exactly_one_entry_on_one_line` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `a_job_with_every_field_set_round_trips_through_its_on_disk_form` | `src/daemon/record_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs` | Adapter contract, moved with its behavior |
| `a_line_carries_the_arbitrated_plan_and_each_legs_verdict` | `src/decision_log/tests/line.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/line.rs` | Adapter contract, moved with its behavior |
| `a_line_carries_the_payloads_mode_agent_and_tool_or_says_none` | `src/decision_log/tests/identity.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `a_line_names_the_event_and_every_gate_input_behind_one_epoch_second` | `src/decision_log/tests/line.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/line.rs` | Adapter contract, moved with its behavior |
| `a_line_this_cannot_read_is_skipped_rather_than_hiding_the_ones_behind_it` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `a_line_with_no_readable_clock_leads_with_a_dash_rather_than_epoch_zero` | `src/decision_log/tests/line.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/line.rs` | Adapter contract, moved with its behavior |
| `a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `a_marker_whose_shell_is_gone_is_swept_and_never_read` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `a_name_that_is_not_a_shell_pid_is_swept` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `a_payload_field_outside_the_printable_allowlist_is_recorded_as_unprintable` | `src/decision_log/tests/identity.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `a_pending_file_left_behind_wide_open_is_narrowed_before_the_rename_publishes_it` | `src/state_rings/tests.rs` | `crates/pns-adapters/src/persistence/ring/tests.rs` | Adapter contract, moved with its behavior |
| `a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_pending_file_whose_run_is_gone_is_collected_and_a_marker_that_spells_it_is_swept` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `a_record_missing_a_key_degrades_to_a_thinner_one_and_a_line_that_is_not_json_is_refused` | `src/nag/tests.rs` | `crates/pns-adapters/src/protocols/nag/tests.rs` | Adapter contract, moved with its behavior |
| `a_record_that_is_not_a_record_is_refused_by_name_rather_than_guessed_at` | `src/daemon/record_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs` | Adapter contract, moved with its behavior |
| `a_record_whose_id_is_not_its_filename_is_refused` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `a_record_with_every_field_set_round_trips_through_its_on_disk_form` | `src/nag/tests.rs` | `crates/pns-adapters/src/protocols/nag/tests.rs` | Adapter contract, moved with its behavior |
| `a_refresh_published_while_a_job_is_claimed_survives_the_daemons_re_arm` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `a_registration_landing_while_the_old_record_is_claimed_is_not_deleted_by_the_cleanup` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `a_ring_that_vanished_under_the_append_is_never_republished_over` | `src/state_rings/tests.rs` | `crates/pns-adapters/src/persistence/ring/tests.rs` | Adapter contract, moved with its behavior |
| `a_room_name_carrying_a_newline_stays_one_entry` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `a_room_name_carrying_the_readers_own_field_marker_still_reads_back_whole` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `a_routing_left_whole_carries_its_reason_and_names_no_room` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `a_same_second_backup_collision_names_the_backup_it_could_not_claim` | `src/setup_publish_runtime/tests.rs` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter contract, moved with its behavior |
| `a_session_id_that_cannot_be_a_filename_names_nothing_at_all` | `src/nag/tests.rs` | `crates/pns-adapters/src/protocols/nag/tests.rs` | Adapter contract, moved with its behavior |
| `a_short_entry_reads_its_absent_fields_as_empty_and_a_junk_line_costs_the_batch_nothing` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `a_spool_path_that_is_not_a_directory_is_a_permanent_refusal` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `a_summary_of_one_reads_as_a_single_notification_in_the_singular` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `a_summary_of_three_names_three_and_puts_the_newest_first` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `a_switched_off_card_says_the_misses_are_recorded_and_that_nothing_delivers_them` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `a_symlinked_markers_directory_cancels_nothing` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `a_wait_nobody_has_answered_still_holds_its_lamp_until_the_configured_backstop` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live` | `src/blocked_wait_markers/tests.rs` | `crates/pns-adapters/src/protocols/markers/blocked/tests.rs` | Adapter contract, moved with its behavior |
| `an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable` | `src/decision_log/tests/identity.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `an_argv_that_renders_past_the_record_cap_is_refused_by_name` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `an_entry_carries_the_epoch_and_the_five_values_a_card_is_rebuilt_from` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `an_entry_reads_back_into_the_six_values_the_writer_put_there` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `an_entry_whose_keys_arrive_in_another_order_reads_back_the_same` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `an_entry_written_with_no_readable_clock_records_a_null_rather_than_a_zero` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `an_id_cannot_escape_the_spool_directory` | `src/daemon/record_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs` | Adapter contract, moved with its behavior |
| `an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim` | `src/nag/tests.rs` | `crates/pns-adapters/src/protocols/nag/tests.rs` | Adapter contract, moved with its behavior |
| `every_other_out_of_range_field_is_refused_by_name_too` | `src/daemon/record_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs` | Adapter contract, moved with its behavior |
| `every_text_field_is_flattened_and_cut_to_the_cap_a_card_renders` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `every_way_a_routing_can_be_left_whole_names_its_own_reason` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `no_directory_and_an_empty_one_both_read_as_nothing` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans` | `src/decision_log/tests/identity.rs` | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `the_backup_sits_beside_the_config_stamped_with_the_instant_it_was_moved` | `src/setup.rs` | `crates/pns-adapters/src/protocols/config_publication/backup_name.rs` | Adapter contract, moved with its behavior |
| `the_first_tick_sweeps_the_state_the_old_names_held` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `the_job_count_counts_records_and_not_whatever_is_in_the_directory` | `src/daemon/spool/spool_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs` | Adapter contract, moved with its behavior |
| `the_newest_entry_is_the_one_read_back` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `the_news_record_is_written_for_a_finished_or_a_dead_turn_and_read_back_as_it_was` | `src/lights_state_runtime/tests.rs` | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs` | Adapter contract, moved with its behavior |
| `the_record_carries_the_reading_the_desk_clock_and_the_router_verdict` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `the_router_verdict_is_recorded_as_one_word_and_never_its_evidence` | `src/presence_journal.rs` | `crates/pns-adapters/src/persistence/rings/presence/tests.rs` | Adapter contract, moved with its behavior |
| `the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `the_sweep_leaves_a_marker_that_is_mid_publish_alone` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `the_ticks_blocked_reading_takes_its_backstop_from_the_config_on_both_halves` | `src/lights_marker_runtime/tests.rs` | `crates/pns-adapters/src/protocols/markers/tests.rs` | Adapter contract, moved with its behavior |
| `the_two_optional_fields_round_trip_as_absent_rather_than_as_a_sentinel` | `src/daemon/record_tests.rs` | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs` | Adapter contract, moved with its behavior |
| `the_waiting_line_cannot_emit_an_entrys_content` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `the_waiting_line_counts_the_journal_and_says_the_entries_wait_to_be_replayed` | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs` | Adapter contract, moved with its behavior |
| `two_ids_that_differ_only_after_a_dot_claim_two_different_names` | `src/nag/tests.rs` | `crates/pns-adapters/src/protocols/nag/tests.rs` | Adapter contract, moved with its behavior |

The following named tests add coverage at owned file-protocol boundaries. The journal and nag
ownership pins cover existing behavior. The replay lifetime, private turn creation and retention,
and setup recovery and warnings cover the approved behavior repairs. They use private files and
owned, bounded process fixtures.

| New leaf name | Source | Category |
| --- | --- | --- |
| `a_backup_security_failure_restores_the_old_config_and_reports_the_failure` | `crates/pns-adapters/src/protocols/config_publication/restore/tests.rs` | Adapter or use-case behavior contract |
| `a_claim_that_became_fresh_is_restored_without_replacing_an_arrival` | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs` | Adapter or use-case behavior contract |
| `a_completed_failed_replay_still_consumes_its_batch` | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs` | Adapter or use-case behavior contract |
| `a_dangling_turn_marker_never_writes_its_symlink_target` | `crates/pns-adapters/src/protocols/turn_markers/tests.rs` | Adapter or use-case behavior contract |
| `a_failed_forced_publication_restores_the_previous_config` | `crates/pns-adapters/src/protocols/config_publication/tests.rs` | Adapter or use-case behavior contract |
| `a_read_claim_stays_on_disk_for_its_live_owner_until_completion` | `crates/pns-adapters/src/protocols/journal_claims/take/tests.rs` | Adapter or use-case behavior contract |
| `a_record_claim_never_overwrites_a_batch_that_owner_already_holds` | `crates/pns-adapters/src/protocols/nag/claims/tests.rs` | Adapter or use-case behavior contract |
| `a_replay_hold_is_adopted_after_the_owned_process_exits` | `crates/pns-adapters/src/protocols/return_window/tests/process.rs` | Adapter or use-case behavior contract |
| `a_sweep_claim_already_owned_by_another_invocation_is_untouched` | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs` | Adapter or use-case behavior contract |
| `an_existing_claim_for_this_process_preserves_both_waiting_batches` | `crates/pns-adapters/src/protocols/journal_claims/take/tests.rs` | Adapter or use-case behavior contract |
| `an_expired_claim_is_removed_without_consuming_a_later_prompt` | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs` | Adapter or use-case behavior contract |
| `an_unwinding_replay_preserves_its_hold_and_a_live_racer_cannot_take_it` | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs` | Adapter or use-case behavior contract |
| `completion_removes_only_its_hold_and_preserves_a_new_journal` | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs` | Adapter or use-case behavior contract |
| `restoration_failure_keeps_the_backup_and_names_the_unrestored_state` | `crates/pns-adapters/src/protocols/config_publication/restore/tests.rs` | Adapter or use-case behavior contract |
| `restoration_preserves_a_later_config_and_the_old_backup` | `crates/pns-adapters/src/protocols/config_publication/restore/tests.rs` | Adapter or use-case behavior contract |
| `setup_refuses_a_directory_before_questions_without_suggesting_force` | `crates/pns-adapters/src/protocols/config_publication/check/tests.rs` | Adapter or use-case behavior contract |
| `setup_warns_before_secrets_about_managed_replacement_and_secret_diffs` | `tests/setup/managed_warning.rs` | Adapter or use-case behavior contract |
| `the_first_turn_marker_is_private_and_a_later_prompt_keeps_its_bytes` | `crates/pns-adapters/src/protocols/turn_markers/tests.rs` | Adapter or use-case behavior contract |
| `the_replay_use_case_keeps_its_hold_through_delivery_then_completes_it` | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs` | Adapter or use-case behavior contract |
| `the_sweep_preserves_links_and_directories_and_restores_torn_claims` | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs` | Adapter or use-case behavior contract |
| `the_turn_sweep_removes_only_markers_strictly_older_than_seven_days` | `crates/pns-adapters/src/protocols/turn_markers/tests.rs` | Adapter or use-case behavior contract |
| `two_record_claimers_have_one_owner_even_without_the_fire_lock` | `crates/pns-adapters/src/protocols/nag/claims/tests.rs` | Adapter or use-case behavior contract |
| `unreadable_clocks_and_unowned_names_do_not_expire_turn_markers` | `crates/pns-adapters/src/protocols/turn_markers/tests.rs` | Adapter or use-case behavior contract |
