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
cargo test --locked --workspace --manifest-path pns/Cargo.toml
```

The NAME column did not come from that run's output, and this matters. A test that prints to standard
output can interleave with the harness's own `test <name> ... ok` lines, and on this suite it did: a
speed-guard warning merged with the following result line and produced a name that no test has. Names are
therefore taken from the deterministic listing:

```
cargo test --locked --workspace --manifest-path pns/Cargo.toml -- --list
```

Regenerate with the same two commands. Take names from `--list` and results from the run, never names
from the run. Both carry `--workspace` because pns is a workspace now: without that word cargo covers the
root package alone and drops every member crate's tests from the regenerated baseline, silently.

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
behavior that no test pins leaves the set unchanged when it breaks. `docs/specs/unpinned-behaviors.md` is
the list of those, and the rule that goes with it: before a later pull request moves the code behind one
of them, it writes the missing test first, against the code in its current location, and lands it before
the move. That list is the known minimum rather than a complete one, so the per-behavior mutation check
stays required whether or not a behavior appears on it.

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
unlisted names keep their existing location and classification. The unchanged names include the inherited
harness speed guards, which remain harness checks rather than product behavior contracts. Their later
test-layout owner remains step 16. No test is removed by this batch.

`the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` retains its historical name, but
its interrupted arm deliberately changes: completed delivery still consumes the hold, while killing the
engine inside replay dispatch now preserves one complete held journal. The near-edge restoration
assertion remains. A separate deletion-before-outcome mutant fails the new held-batch assertion, so a
stable name here is not claimed as evidence of unchanged behavior.

| Leaf name (unchanged)                                                                     | Previous source                           | Current source                                                          | Category                                  |
| ----------------------------------------------------------------------------------------- | ----------------------------------------- | ----------------------------------------------------------------------- | ----------------------------------------- |
| `a_bare_token_on_disk_still_reads_as_a_held_lamp_with_no_phase`                           | `src/lights_state_runtime/tests.rs`       | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs`         | Adapter contract, moved with its behavior |
| `a_claim_that_fails_for_another_reason_is_not_blamed_on_a_same_second_run`                | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_clock_that_cannot_be_read_names_no_backup_at_all`                                      | `src/setup.rs`                            | `crates/pns-adapters/src/protocols/config_publication/backup_name.rs`   | Adapter contract, moved with its behavior |
| `a_complaint_that_cleared_is_forgotten_so_its_return_is_news_again`                       | `src/lights_state_runtime/tests.rs`       | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs`         | Adapter contract, moved with its behavior |
| `a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over`              | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace`     | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_due_outside_a_bounded_window_of_now_is_refused_at_registration`                        | `src/daemon/record_tests.rs`              | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs`         | Adapter contract, moved with its behavior |
| `a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file`           | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one`                  | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside`                        | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_forced_run_keeps_a_config_the_existence_check_reads_as_absent`                         | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_forced_run_keeps_the_config_it_replaced_rather_than_what_that_config_named`            | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_held_record_that_is_absent_holds_nothing_and_one_that_will_not_read_holds_everything`  | `src/lights_state_runtime/tests.rs`       | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs`         | Adapter contract, moved with its behavior |
| `a_held_records_phase_round_trips_through_remember_held_and_read_held`                    | `src/lights_state_runtime/tests.rs`       | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs`         | Adapter contract, moved with its behavior |
| `a_hostile_detail_still_produces_exactly_one_entry_on_one_line`                           | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `a_job_with_every_field_set_round_trips_through_its_on_disk_form`                         | `src/daemon/record_tests.rs`              | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs`         | Adapter contract, moved with its behavior |
| `a_line_carries_the_arbitrated_plan_and_each_legs_verdict`                                | `src/decision_log/tests/line.rs`          | `crates/pns-adapters/src/persistence/rings/decisions/tests/line.rs`     | Adapter contract, moved with its behavior |
| `a_line_carries_the_payloads_mode_agent_and_tool_or_says_none`                            | `src/decision_log/tests/identity.rs`      | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `a_line_names_the_event_and_every_gate_input_behind_one_epoch_second`                     | `src/decision_log/tests/line.rs`          | `crates/pns-adapters/src/persistence/rings/decisions/tests/line.rs`     | Adapter contract, moved with its behavior |
| `a_line_this_cannot_read_is_skipped_rather_than_hiding_the_ones_behind_it`                | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `a_line_with_no_readable_clock_leads_with_a_dash_rather_than_epoch_zero`                  | `src/decision_log/tests/line.rs`          | `crates/pns-adapters/src/persistence/rings/decisions/tests/line.rs`     | Adapter contract, moved with its behavior |
| `a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone`                              | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `a_marker_whose_shell_is_gone_is_swept_and_never_read`                                    | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `a_name_that_is_not_a_shell_pid_is_swept`                                                 | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `a_payload_field_outside_the_printable_allowlist_is_recorded_as_unprintable`              | `src/decision_log/tests/identity.rs`      | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `a_pending_file_left_behind_wide_open_is_narrowed_before_the_rename_publishes_it`         | `src/state_rings/tests.rs`                | `crates/pns-adapters/src/persistence/ring/tests.rs`                     | Adapter contract, moved with its behavior |
| `a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into`          | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_pending_file_whose_run_is_gone_is_collected_and_a_marker_that_spells_it_is_swept`      | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `a_record_missing_a_key_degrades_to_a_thinner_one_and_a_line_that_is_not_json_is_refused` | `src/nag/tests.rs`                        | `crates/pns-adapters/src/protocols/nag/tests.rs`                        | Adapter contract, moved with its behavior |
| `a_record_that_is_not_a_record_is_refused_by_name_rather_than_guessed_at`                 | `src/daemon/record_tests.rs`              | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs`         | Adapter contract, moved with its behavior |
| `a_record_whose_id_is_not_its_filename_is_refused`                                        | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `a_record_with_every_field_set_round_trips_through_its_on_disk_form`                      | `src/nag/tests.rs`                        | `crates/pns-adapters/src/protocols/nag/tests.rs`                        | Adapter contract, moved with its behavior |
| `a_refresh_published_while_a_job_is_claimed_survives_the_daemons_re_arm`                  | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `a_registration_landing_while_the_old_record_is_claimed_is_not_deleted_by_the_cleanup`    | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `a_ring_that_vanished_under_the_append_is_never_republished_over`                         | `src/state_rings/tests.rs`                | `crates/pns-adapters/src/persistence/ring/tests.rs`                     | Adapter contract, moved with its behavior |
| `a_room_name_carrying_a_newline_stays_one_entry`                                          | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `a_room_name_carrying_the_readers_own_field_marker_still_reads_back_whole`                | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `a_routing_left_whole_carries_its_reason_and_names_no_room`                               | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `a_same_second_backup_collision_names_the_backup_it_could_not_claim`                      | `src/setup_publish_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter contract, moved with its behavior |
| `a_session_id_that_cannot_be_a_filename_names_nothing_at_all`                             | `src/nag/tests.rs`                        | `crates/pns-adapters/src/protocols/nag/tests.rs`                        | Adapter contract, moved with its behavior |
| `a_short_entry_reads_its_absent_fields_as_empty_and_a_junk_line_costs_the_batch_nothing`  | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `a_spool_path_that_is_not_a_directory_is_a_permanent_refusal`                             | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `a_summary_of_one_reads_as_a_single_notification_in_the_singular`                         | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `a_summary_of_three_names_three_and_puts_the_newest_first`                                | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind`             | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `a_switched_off_card_says_the_misses_are_recorded_and_that_nothing_delivers_them`         | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `a_symlinked_markers_directory_cancels_nothing`                                           | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `a_wait_nobody_has_answered_still_holds_its_lamp_until_the_configured_backstop`           | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live`                    | `src/blocked_wait_markers/tests.rs`       | `crates/pns-adapters/src/protocols/markers/blocked/tests.rs`            | Adapter contract, moved with its behavior |
| `an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable`            | `src/decision_log/tests/identity.rs`      | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `an_argv_that_renders_past_the_record_cap_is_refused_by_name`                             | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `an_entry_carries_the_epoch_and_the_five_values_a_card_is_rebuilt_from`                   | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `an_entry_reads_back_into_the_six_values_the_writer_put_there`                            | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `an_entry_whose_keys_arrive_in_another_order_reads_back_the_same`                         | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `an_entry_written_with_no_readable_clock_records_a_null_rather_than_a_zero`               | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `an_id_cannot_escape_the_spool_directory`                                                 | `src/daemon/record_tests.rs`              | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs`         | Adapter contract, moved with its behavior |
| `an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim`                        | `src/nag/tests.rs`                        | `crates/pns-adapters/src/protocols/nag/tests.rs`                        | Adapter contract, moved with its behavior |
| `every_other_out_of_range_field_is_refused_by_name_too`                                   | `src/daemon/record_tests.rs`              | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs`         | Adapter contract, moved with its behavior |
| `every_text_field_is_flattened_and_cut_to_the_cap_a_card_renders`                         | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `every_way_a_routing_can_be_left_whole_names_its_own_reason`                              | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `no_directory_and_an_empty_one_both_read_as_nothing`                                      | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans`                   | `src/decision_log/tests/identity.rs`      | `crates/pns-adapters/src/persistence/rings/decisions/tests/identity.rs` | Adapter contract, moved with its behavior |
| `the_backup_sits_beside_the_config_stamped_with_the_instant_it_was_moved`                 | `src/setup.rs`                            | `crates/pns-adapters/src/protocols/config_publication/backup_name.rs`   | Adapter contract, moved with its behavior |
| `the_first_tick_sweeps_the_state_the_old_names_held`                                      | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `the_job_count_counts_records_and_not_whatever_is_in_the_directory`                       | `src/daemon/spool/spool_tests.rs`         | `crates/pns-adapters/src/protocols/spool/tests/spool_tests.rs`          | Adapter contract, moved with its behavior |
| `the_newest_entry_is_the_one_read_back`                                                   | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `the_news_record_is_written_for_a_finished_or_a_dead_turn_and_read_back_as_it_was`        | `src/lights_state_runtime/tests.rs`       | `crates/pns-adapters/src/persistence/rings/lamp_state/tests.rs`         | Adapter contract, moved with its behavior |
| `the_record_carries_the_reading_the_desk_clock_and_the_router_verdict`                    | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `the_router_verdict_is_recorded_as_one_word_and_never_its_evidence`                       | `src/presence_journal.rs`                 | `crates/pns-adapters/src/persistence/rings/presence/tests.rs`           | Adapter contract, moved with its behavior |
| `the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding`                          | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `the_sweep_leaves_a_marker_that_is_mid_publish_alone`                                     | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `the_ticks_blocked_reading_takes_its_backstop_from_the_config_on_both_halves`             | `src/lights_marker_runtime/tests.rs`      | `crates/pns-adapters/src/protocols/markers/tests.rs`                    | Adapter contract, moved with its behavior |
| `the_two_optional_fields_round_trip_as_absent_rather_than_as_a_sentinel`                  | `src/daemon/record_tests.rs`              | `crates/pns-adapters/src/protocols/spool/tests/record_tests.rs`         | Adapter contract, moved with its behavior |
| `the_waiting_line_cannot_emit_an_entrys_content`                                          | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `the_waiting_line_counts_the_journal_and_says_the_entries_wait_to_be_replayed`            | `src/missed_notifications/codec_tests.rs` | `crates/pns-adapters/src/persistence/rings/journal/tests.rs`            | Adapter contract, moved with its behavior |
| `two_ids_that_differ_only_after_a_dot_claim_two_different_names`                          | `src/nag/tests.rs`                        | `crates/pns-adapters/src/protocols/nag/tests.rs`                        | Adapter contract, moved with its behavior |

The following named tests add coverage at owned file-protocol boundaries. The journal and nag ownership
pins cover existing behavior. The replay lifetime, private turn creation and retention, and setup
recovery and warnings cover the approved behavior repairs. They use private files and owned, bounded
process fixtures.

| New leaf name                                                               | Source                                                                  | Category                              |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------- |
| `a_backup_security_failure_restores_the_old_config_and_reports_the_failure` | `crates/pns-adapters/src/protocols/config_publication/restore/tests.rs` | Adapter or use-case behavior contract |
| `a_claim_that_became_fresh_is_restored_without_replacing_an_arrival`        | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs`         | Adapter or use-case behavior contract |
| `a_completed_failed_replay_still_consumes_its_batch`                        | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs`          | Adapter or use-case behavior contract |
| `a_dangling_turn_marker_never_writes_its_symlink_target`                    | `crates/pns-adapters/src/protocols/turn_markers/tests.rs`               | Adapter or use-case behavior contract |
| `a_failed_forced_publication_restores_the_previous_config`                  | `crates/pns-adapters/src/protocols/config_publication/tests.rs`         | Adapter or use-case behavior contract |
| `a_read_claim_stays_on_disk_for_its_live_owner_until_completion`            | `crates/pns-adapters/src/protocols/journal_claims/take/tests.rs`        | Adapter or use-case behavior contract |
| `a_record_claim_never_overwrites_a_batch_that_owner_already_holds`          | `crates/pns-adapters/src/protocols/nag/claims/tests.rs`                 | Adapter or use-case behavior contract |
| `a_replay_hold_is_adopted_after_the_owned_process_exits`                    | `crates/pns-adapters/src/protocols/return_window/tests/process.rs`      | Adapter or use-case behavior contract |
| `a_sweep_claim_already_owned_by_another_invocation_is_untouched`            | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs`         | Adapter or use-case behavior contract |
| `an_existing_claim_for_this_process_preserves_both_waiting_batches`         | `crates/pns-adapters/src/protocols/journal_claims/take/tests.rs`        | Adapter or use-case behavior contract |
| `an_expired_claim_is_removed_without_consuming_a_later_prompt`              | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs`         | Adapter or use-case behavior contract |
| `an_unwinding_replay_preserves_its_hold_and_a_live_racer_cannot_take_it`    | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs`          | Adapter or use-case behavior contract |
| `completion_removes_only_its_hold_and_preserves_a_new_journal`              | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs`          | Adapter or use-case behavior contract |
| `restoration_failure_keeps_the_backup_and_names_the_unrestored_state`       | `crates/pns-adapters/src/protocols/config_publication/restore/tests.rs` | Adapter or use-case behavior contract |
| `restoration_preserves_a_later_config_and_the_old_backup`                   | `crates/pns-adapters/src/protocols/config_publication/restore/tests.rs` | Adapter or use-case behavior contract |
| `setup_refuses_a_directory_before_questions_without_suggesting_force`       | `crates/pns-adapters/src/protocols/config_publication/check/tests.rs`   | Adapter or use-case behavior contract |
| `setup_warns_before_secrets_about_managed_replacement_and_secret_diffs`     | `tests/setup/managed_warning.rs`                                        | Adapter or use-case behavior contract |
| `the_first_turn_marker_is_private_and_a_later_prompt_keeps_its_bytes`       | `crates/pns-adapters/src/protocols/turn_markers/tests.rs`               | Adapter or use-case behavior contract |
| `the_replay_use_case_keeps_its_hold_through_delivery_then_completes_it`     | `crates/pns-adapters/src/protocols/return_window/tests/mod.rs`          | Adapter or use-case behavior contract |
| `the_sweep_preserves_links_and_directories_and_restores_torn_claims`        | `crates/pns-adapters/src/protocols/turn_markers/sweep/tests.rs`         | Adapter or use-case behavior contract |
| `the_turn_sweep_removes_only_markers_strictly_older_than_seven_days`        | `crates/pns-adapters/src/protocols/turn_markers/tests.rs`               | Adapter or use-case behavior contract |
| `two_record_claimers_have_one_owner_even_without_the_fire_lock`             | `crates/pns-adapters/src/protocols/nag/claims/tests.rs`                 | Adapter or use-case behavior contract |
| `unreadable_clocks_and_unowned_names_do_not_expire_turn_markers`            | `crates/pns-adapters/src/protocols/turn_markers/tests.rs`               | Adapter or use-case behavior contract |

## SQLite repositories and first-run import (plan 11.3 and 12.1)

This batch adds 51 leaves and removes or renames no predecessor. Existing filesystem implementations and
their tests remain for the ordered consumer cutover. All predecessor bodies remain unchanged; the later
application union owns its separately recorded moves. These new names specify behavior 28 through 36 in
`docs/specs/persistence-and-process-lifecycle.md`. The import leaves are migration contracts; the others
are repository adapter contracts. Four memory leaves run the same contracts as SQLite. Their fault
controls change only the concrete test repository and are identified as such in the delivery evidence.

The original name sets and results remain the baseline. Focused fault evidence is separate from the final
workspace run; neither a count nor a green retry substitutes for exact names and source identity.

| Added leaf under `persistence::sqlite::tests::`                                                              | Behavior |
| ------------------------------------------------------------------------------------------------------------ | -------- |
| `a_future_schema_is_rejected_before_changing_its_journal_mode`                                               | 28       |
| `a_new_store_commits_its_schema_in_a_private_wal_database`                                                   | 28       |
| `a_newer_schema_is_refused_without_rewriting_its_version_or_data`                                            | 28       |
| `a_publicly_readable_database_is_refused_without_changing_its_permissions`                                   | 28       |
| `a_reopened_store_keeps_committed_records`                                                                   | 28       |
| `a_symlink_database_is_refused_without_touching_its_target`                                                  | 28       |
| `an_irregular_database_is_refused_without_opening_it`                                                        | 28       |
| `contracts::the_memory_activity_runs_the_same_one_hundred_fifty_entry_contract`                              | 29       |
| `contracts::the_memory_activity_runs_the_same_window_contract`                                               | 29       |
| `contracts::the_memory_decisions_run_the_same_retention_and_privacy_contract`                                | 29       |
| `contracts::the_memory_journal_runs_the_same_retention_contract`                                             | 29       |
| `contracts::the_sqlite_activity_ring_prunes_only_the_one_hundred_fifty_first_entry`                          | 29       |
| `contracts::the_sqlite_activity_window_excludes_the_near_edge_and_unknown_clocks`                            | 29       |
| `contracts::the_sqlite_decision_history_keeps_five_exact_private_codec_records`                              | 29       |
| `contracts::the_sqlite_journal_keeps_twenty_five_entries_in_arrival_order`                                   | 29       |
| `failures::a_busy_delivery_record_returns_and_logs_the_miss_without_disclosing_event_text`                   | 30       |
| `failures::a_failed_prune_rolls_back_the_new_record_and_preserves_the_whole_prior_ring`                      | 30       |
| `history::policy_settings_history_retains_twenty_receipts_with_unknown_clock_and_empty_path_wording`         | 29       |
| `history::presence_history_retains_five_exact_codec_lines_and_the_latest_narrowing`                          | 29       |
| `import::a_completed_import_never_replays_old_files_over_newer_records_or_duplicates_history`                | 33       |
| `import::a_live_legacy_return_owner_defers_import_without_marking_any_family_complete`                       | 35       |
| `import::an_empty_imported_ring_stays_distinct_from_an_absent_ring`                                          | 33       |
| `import::appending_imported_history_matches_legacy_separator_and_pruning_bytes`                              | 33       |
| `import::failure_to_commit_import_completion_rolls_back_every_imported_record_and_can_be_retried`            | 33       |
| `import::first_run_import_preserves_raw_history_and_all_scalar_families_without_changing_legacy_bytes`       | 33       |
| `import::malformed_imported_epochs_and_lamp_records_keep_their_original_fail_directions_and_report_the_loss` | 34       |
| `import::unreadable_imports_are_reported_and_remain_unknown_until_the_owning_repository_is_written`          | 34       |
| `import::unreadable_notification_and_history_imports_remain_diagnostic_until_replaced`                       | 34       |
| `import_claims::abandoned_legacy_batches_keep_their_order_and_survive_pending_ring_pruning`                  | 35       |
| `import_claims::an_interrupted_import_leaves_no_completion_marker_or_partial_history`                        | 33       |
| `import_claims::an_unreadable_abandoned_window_is_reported_and_left_available_for_recovery`                  | 35       |
| `import_claims::legacy_ring_locks_defer_import_through_the_exact_stale_edge_and_unknown_or_future_clocks`    | 35       |
| `lamps::a_failed_held_lamp_replacement_rolls_back_instead_of_losing_the_previous_names`                      | 32       |
| `lamps::held_lamp_paths_and_phases_round_trip_in_order_and_an_empty_write_clears_them`                       | 32       |
| `lamps::lamp_mutes_keep_place_bytes_and_expiry_order_and_clear_as_one_set`                                   | 32       |
| `lamps::lamp_news_merges_both_kinds_without_moving_either_epoch_backwards`                                   | 32       |
| `lamps::the_working_streak_survives_the_exact_grace_edge_and_resets_one_second_later`                        | 32       |
| `ports::a_busy_streak_publication_still_returns_the_computed_next_streak_without_claiming_it_was_stored`     | 36       |
| `ports::lamp_repository_ports_preserve_unknown_reads_and_refuse_unwritten_changes`                           | 36       |
| `ports::the_presence_port_records_the_callers_snapshot_and_the_house_port_reads_committed_news`              | 36       |
| `ports::typed_complaint_and_staleness_memories_keep_their_independent_values_and_clear_only_the_named_one`   | 36       |
| `processes::a_committed_journal_hold_is_adopted_after_its_owned_process_exits`                               | 31       |
| `processes::a_second_process_cannot_write_during_a_transaction_and_a_killed_writer_leaves_no_partial_record` | 30       |
| `returns::a_busy_return_claim_fails_closed_without_advancing_the_edge_or_consuming_the_journal`              | 31       |
| `returns::an_unfinished_return_is_preserved_when_its_store_drops_and_another_live_owner_cannot_take_it`      | 31       |
| `returns::claiming_only_the_return_edge_never_takes_the_journal_and_an_unknown_clock_never_moves_the_edge`   | 31       |
| `returns::pruning_later_arrivals_cannot_evict_the_batch_an_active_return_still_owns`                         | 31       |
| `returns::the_return_edge_and_waiting_journal_are_claimed_together_and_completion_preserves_later_arrivals`  | 31       |
| `settings::a_busy_setting_change_reports_failure_and_keeps_the_previous_expiry`                              | 32       |
| `settings::quiet_expiry_and_staleness_survive_reopening_and_clear_independently`                             | 32       |
| `settings::the_two_lamp_complaint_memories_never_overwrite_or_forget_each_other`                             | 32       |

## Delivery ledger storage, plan 11.4

The SQLite predecessor's 1,649 listed outcomes remain, with 20 added adapter contracts below. Three
historical schema test names remain while their current/future literals advance from 1/2 to 2/3:
`a_new_store_commits_its_schema_in_a_private_wal_database`,
`a_newer_schema_is_refused_without_rewriting_its_version_or_data`, and
`a_future_schema_is_rejected_before_changing_its_journal_mode`. Their privacy and refusal contracts stay
in force. No dispatch or command-line behavior is replaced by this storage payload.

Names below are under `persistence::sqlite::ledger::tests::`. Behavior numbers refer to
`specs/persistence-and-process-lifecycle.md`.

| Added adapter contract                                                                                    | Behavior |
| --------------------------------------------------------------------------------------------------------- | -------- |
| `atomicity::a_failed_leg_insert_rolls_back_the_event_routes_and_initial_attempts`                         | 37       |
| `atomicity::a_failed_outcome_write_preserves_the_unfinished_claim_for_a_later_record`                     | 40       |
| `atomicity::a_failed_version_two_schema_upgrade_rolls_back_its_tables_and_version`                        | 41       |
| `atomicity::a_version_one_database_upgrades_without_changing_its_existing_records`                        | 41       |
| `claims::a_claim_from_another_database_cannot_complete_a_matching_row`                                    | 39       |
| `claims::a_live_initial_lease_blocks_retries_until_its_exact_boundary_and_preserves_original_route`       | 39       |
| `claims::a_stale_generation_cannot_acknowledge_a_released_leg`                                            | 39       |
| `claims::invalid_or_overflow_edge_lease_windows_do_not_create_or_claim_rows`                              | 39       |
| `failures::busy_ledger_writes_refuse_within_the_budget_without_recording_sensitive_content`               | 40       |
| `outcomes::a_completed_generation_cannot_be_rewritten_and_a_late_uncontested_completion_can_settle`       | 38       |
| `outcomes::acknowledged_legs_are_retained_and_never_leased_again`                                         | 38       |
| `outcomes::retry_claims_follow_event_sequence_and_retained_attempt_order_without_replacing_history`       | 38       |
| `outcomes::retry_outcomes_keep_their_details_and_are_due_only_at_the_exact_retry_instant`                 | 38       |
| `prepare::an_identical_inflight_submission_returns_history_without_a_second_dispatch_claim`               | 37       |
| `prepare::conflicting_payload_or_any_resolved_leg_fact_is_refused_without_overwriting_the_original`       | 37       |
| `prepare::distinct_producer_or_request_ids_keep_identical_content_as_separate_monotonic_events`           | 37       |
| `prepare::duplicate_destination_instances_are_refused_and_an_empty_plan_is_retained`                      | 37       |
| `prepare::preparation_commits_the_original_identity_payload_routes_and_unknown_attempts_before_returning` | 37       |
| `processes::a_retry_claim_excludes_a_competing_process_and_survives_its_owners_death`                     | 39       |
| `processes::an_initial_claim_excludes_a_competing_process_and_survives_its_owners_death`                  | 39       |

## Internal-record consumer cutover, plan 12.1

Thirteen added leaves cover import-before-access, stable keyed decisions, doctor import diagnostics and
bounded direct-write complaints. The delivery evidence retains their fail-first assertions and
independent source faults. The retry correction preserves the original facts and other leg outcomes while
two competing completions update the same decision. Numbers below refer to
`specs/persistence-and-process-lifecycle.md`. The existing schema tests advance their current/future
literals from 2/3 to 3/4; the version-one upgrade case now observes current version 3. Their names,
rollback and refusal assertions remain.

| Added leaf                                                                                                                               | Behavior |
| ---------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `persistence::sqlite::tests::consumer_start::the_first_consumer_read_imports_existing_records_without_changing_legacy_bytes`             | 42       |
| `persistence::sqlite::tests::consumer_start::an_active_legacy_owner_refuses_consumer_reads_and_writes_without_an_empty_success`          | 42       |
| `persistence::sqlite::tests::consumer_start::completed_consumer_import_ignores_later_legacy_edits_and_needs_no_writer_lock_to_read`      | 42       |
| `persistence::sqlite::tests::consumer_start::consumer_import_retains_unknown_families_and_reports_them_without_blocking_other_records`   | 42       |
| `persistence::sqlite::tests::decision_outcomes::a_duplicate_begin_keeps_the_recorded_outcome_and_distinct_submission_keys_stay_distinct` | 43       |
| `persistence::sqlite::tests::decision_outcomes::a_per_leg_revision_keeps_arrival_order_and_uses_the_existing_private_codec`              | 43       |
| `persistence::sqlite::tests::decision_outcomes::a_late_revision_never_resurrects_a_pruned_decision_or_reorders_remaining_events`         | 43       |
| `persistence::sqlite::tests::decision_outcomes::appending_keyed_decisions_preserves_legacy_separator_and_pruning_bytes`                  | 43       |
| `persistence::sqlite::tests::decision_outcomes::a_refused_revision_reports_failure_and_preserves_the_prior_outcome`                      | 43       |
| `persistence::sqlite::tests::write_reports::direct_record_writes_report_each_failed_store_without_creating_a_legacy_authority`           | 45       |
| `doctor::tests::imports::doctor_names_retained_import_failures_after_history_without_changing_delivery_health`                       | 44       |
| `doctor::tests::imports::doctor_reports_an_unavailable_import_check_without_claiming_healthy_state_or_failing_delivery`              | 44       |
| `persistence::sqlite::tests::decision_outcomes::a_retry_refuses_malformed_duplicate_or_missing_leg_fields_without_changing_the_row`      | 43       |

The acceptance fixtures read actual database rows for decisions, journals, quiet state and lamps. Legacy
bytes are setup inputs only before first import and remain available for recovery afterward.
Writer-refusal cases now hold an actual competing transaction. The historical leaf
`a_publish_whose_rename_fails_leaves_no_pending_file_behind` retains its name but asserts transaction
refusal, no quiet row and no pending publication file. An identical-value SQLite update can leave no
observable change, so the quiet report guard pins an actual changed expiry through a retained reader.

`the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` retains its historical name while
asserting the approved interrupted-run policy: completed attempts consume their owned batch, including a
failed delivery; a killed owner leaves the same claimed rows recoverable. A count of unchanged names does
not establish unchanged behavior for this leaf. Doctor's first-access fixtures permit import and still
forbid logical event records or claims of its own.

## Dispatch behavior modules, plan 16

The mechanical split preserves all 206 dispatch test leaves, including the ignored adoption soak, with
the same bodies and rationale. Shared fixture bodies move behind private module exports. These are
successor names within the existing dispatch test target; no test is added or removed for the move.
Existing support speed-guard leaves remain harness checks, not permanent product guarantees. Unchanged
oversized daemon and setup acceptance files remain assigned to the planned closure.

| Previous full name                                                                         | Successor full name                                                                                         |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner`                         | `plan_rows::away_from_the_desk_cards_the_phone_and_logs_but_raises_no_banner`                               |
| `at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery`                  | `plan_rows::at_the_desk_with_the_pane_out_of_sight_the_banner_is_the_whole_delivery`                        |
| `at_the_desk_watching_the_pane_only_the_log_fires`                                         | `plan_rows::at_the_desk_watching_the_pane_only_the_log_fires`                                               |
| `the_alert_path_labels_the_hermes_leg_silent_on_the_wire`                                  | `plan_rows::the_alert_path_labels_the_hermes_leg_silent_on_the_wire`                                        |
| `a_channel_is_handed_the_rendered_event_not_the_raw_arguments`                             | `plan_rows::a_channel_is_handed_the_rendered_event_not_the_raw_arguments`                                   |
| `local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`                          | `overrides::local_only_keeps_the_banner_and_reaches_nothing_off_the_machine`                                |
| `remote_only_delivers_through_hermes_alone`                                                | `overrides::remote_only_delivers_through_hermes_alone`                                                      |
| `hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible`          | `overrides::hermes_is_sync_on_the_log_path_which_is_what_makes_an_undelivered_entry_visible`                |
| `both_narrowing_flags_together_deliver_nothing_and_say_so`                                 | `overrides::both_narrowing_flags_together_deliver_nothing_and_say_so`                                       |
| `at_the_desk_the_phone_is_skipped_and_only_the_phone`                                      | `overrides::at_the_desk_the_phone_is_skipped_and_only_the_phone`                                            |
| `relay_skip_phone_drops_the_phone_and_only_the_phone`                                      | `overrides::relay_skip_phone_drops_the_phone_and_only_the_phone`                                            |
| `relay_skip_phone_beats_relay_force_phone`                                                 | `overrides::relay_skip_phone_beats_relay_force_phone`                                                       |
| `relay_force_phone_overrides_presence`                                                     | `overrides::relay_force_phone_overrides_presence`                                                           |
| `a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings`                | `overrides::a_channel_that_fails_neither_fails_the_caller_nor_suppresses_its_siblings`                      |
| `an_absent_channel_is_simply_not_installed`                                                | `overrides::an_absent_channel_is_simply_not_installed`                                                      |
| `a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile`                   | `phone_surface::a_back_tap_newer_than_the_last_desk_input_moves_the_operator_to_mobile`                     |
| `desk_input_after_the_tap_cancels_it`                                                      | `phone_surface::desk_input_after_the_tap_cancels_it`                                                        |
| `a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight`                | `phone_surface::a_tap_with_moshi_closed_cards_the_phone_even_with_the_pane_in_plain_sight`                  |
| `a_narrowing_flag_still_beats_a_fresh_tap`                                                 | `phone_surface::a_narrowing_flag_still_beats_a_fresh_tap`                                                   |
| `skip_phone_still_beats_a_fresh_tap`                                                       | `phone_surface::skip_phone_still_beats_a_fresh_tap`                                                         |
| `a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log`                               | `phone_surface::a_phone_in_hand_watching_the_pane_gets_nothing_but_the_log`                                 |
| `a_phone_in_hand_showing_another_tab_still_cards`                                          | `phone_surface::a_phone_in_hand_showing_another_tab_still_cards`                                            |
| `an_unreadable_view_delivers_rather_than_suppressing_on_doubt`                             | `phone_surface::an_unreadable_view_delivers_rather_than_suppressing_on_doubt`                               |
| `force_phone_is_caller_intent_and_beats_the_whole_surface_model`                           | `phone_surface::force_phone_is_caller_intent_and_beats_the_whole_surface_model`                             |
| `a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event`                  | `producer_argv::a_pane_with_shell_metacharacters_is_scrubbed_from_every_delivered_event`                    |
| `a_scrub_warning_is_not_printed_when_no_channel_will_run`                                  | `producer_argv::a_scrub_warning_is_not_printed_when_no_channel_will_run`                                    |
| `a_non_unicode_argument_never_breaks_the_exit_zero_edge`                                   | `producer_argv::a_non_unicode_argument_never_breaks_the_exit_zero_edge`                                     |
| `the_help_flag_prints_the_usage_and_reaches_nothing_at_all`                                | `producer_argv::the_help_flag_prints_the_usage_and_reaches_nothing_at_all`                                  |
| `a_word_that_names_no_command_is_refused_and_delivers_nothing`                             | `producer_argv::a_word_that_names_no_command_is_refused_and_delivers_nothing`                               |
| `a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`                        | `producer_argv::a_dash_led_first_word_is_no_longer_a_free_pass_for_an_empty_event`                          |
| `a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it`                       | `producer_argv::a_typed_empty_word_is_refused_unlike_the_bare_invocation_beside_it`                         |
| `help_in_flag_position_wins_wherever_it_reaches_the_event_parser`                          | `producer_argv::help_in_flag_position_wins_wherever_it_reaches_the_event_parser`                            |
| `help_in_value_position_is_still_just_a_value`                                             | `producer_argv::help_in_value_position_is_still_just_a_value`                                               |
| `the_first_run_walk_refuses_a_terminal_nobody_is_at_and_writes_nothing`                    | `setup_argv::the_first_run_walk_refuses_a_terminal_nobody_is_at_and_writes_nothing`                         |
| `the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone`            | `setup_argv::the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone`                 |
| `a_setup_typed_wrong_is_refused_with_what_it_takes_rather_than_walked_anyway`              | `setup_argv::a_setup_typed_wrong_is_refused_with_what_it_takes_rather_than_walked_anyway`                   |
| `a_producer_invocation_led_by_a_stray_word_still_delivers`                                 | `producer_events::a_producer_invocation_led_by_a_stray_word_still_delivers`                                 |
| `a_bare_invocation_is_still_the_empty_event_the_contract_calls_valid`                      | `producer_events::a_bare_invocation_is_still_the_empty_event_the_contract_calls_valid`                      |
| `the_delivered_event_is_newline_terminated_for_line_oriented_channels`                     | `producer_events::the_delivered_event_is_newline_terminated_for_line_oriented_channels`                     |
| `a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud`                                | `channel_settings::a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud`                               |
| `one_typod_table_name_costs_a_configured_machine_no_channel`                               | `channel_settings::one_typod_table_name_costs_a_configured_machine_no_channel`                              |
| `a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`                       | `channel_settings::a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`                      |
| `an_absent_config_stays_silent_in_pulse_mode`                                              | `channel_settings::an_absent_config_stays_silent_in_pulse_mode`                                             |
| `pulse_help_prints_its_own_usage_before_any_config_load`                                   | `channel_settings::pulse_help_prints_its_own_usage_before_any_config_load`                                  |
| `pulse_refuses_a_code_it_cannot_read_instead_of_guessing_it_failed`                        | `channel_settings::pulse_refuses_a_code_it_cannot_read_instead_of_guessing_it_failed`                       |
| `an_unknown_plugin_never_resurrects_a_disabled_pulse`                                      | `channel_settings::an_unknown_plugin_never_resurrects_a_disabled_pulse`                                     |
| `the_pulse_config_warning_says_what_pulse_mode_actually_did`                               | `channel_settings::the_pulse_config_warning_says_what_pulse_mode_actually_did`                              |
| `the_binarys_own_roster_knows_the_router_sensor`                                           | `channel_roster::the_binarys_own_roster_knows_the_router_sensor`                                            |
| `the_home_diagnostic_always_shows_the_evidence_and_warns_once_per_stale_state`             | `home_diagnostic::the_home_diagnostic_always_shows_the_evidence_and_warns_once_per_stale_state`             |
| `a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing`               | `home_diagnostic::a_state_directory_that_cannot_be_used_leaves_the_whole_diagnostic_standing`               |
| `a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence`                | `home_diagnostic::a_new_stale_state_is_delivered_as_one_alert_carrying_the_warning_sentence`                |
| `the_same_stale_state_alerts_once_and_a_returning_one_alerts_again`                        | `home_alerts::the_same_stale_state_alerts_once_and_a_returning_one_alerts_again`                            |
| `only_a_home_reading_alerts_and_the_sensor_is_never_a_destination`                         | `home_alerts::only_a_home_reading_alerts_and_the_sensor_is_never_a_destination`                             |
| `the_alert_carries_no_secret_and_no_raw_router_text`                                       | `home_alerts::the_alert_carries_no_secret_and_no_raw_router_text`                                           |
| `an_unusable_stale_alert_route_complains_and_still_delivers_the_alert`                     | `home_alerts::an_unusable_stale_alert_route_complains_and_still_delivers_the_alert`                         |
| `every_way_the_home_probe_is_not_set_up_says_which_one_it_is`                              | `home_setup::every_way_the_home_probe_is_not_set_up_says_which_one_it_is`                                   |
| `a_lights_table_changes_nothing_about_an_ordinary_notification`                            | `quiet_window::a_lights_table_changes_nothing_about_an_ordinary_notification`                               |
| `a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`          | `quiet_window::a_pulse_earned_inside_the_quiet_window_reaches_no_bridge_and_costs_no_other_leg`             |
| `a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`                      | `quiet_window::a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`                         |
| `the_hand_run_pulse_reaches_the_bridge_inside_the_quiet_window`                            | `quiet_window::the_hand_run_pulse_reaches_the_bridge_inside_the_quiet_window`                               |
| `the_window_is_read_in_the_zone_the_child_was_given`                                       | `quiet_window::the_window_is_read_in_the_zone_the_child_was_given`                                          |
| `without_a_lights_table_nothing_new_reaches_the_bridge`                                    | `lights_routes::without_a_lights_table_nothing_new_reaches_the_bridge`                                      |
| `a_blocked_turn_lights_the_lamps_once_the_map_exists`                                      | `lights_routes::a_blocked_turn_lights_the_lamps_once_the_map_exists`                                        |
| `an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg`                 | `lights_routes::an_event_inside_every_dim_window_still_resolves_the_map_and_costs_no_leg`                   |
| `a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`                      | `lights_routes::a_house_quiet_hours_nobody_can_parse_costs_the_routed_lamps_nothing`                        |
| `the_operators_own_mute_takes_the_blocked_lamp_with_everything_else`                       | `lights_routes::the_operators_own_mute_takes_the_blocked_lamp_with_everything_else`                         |
| `an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`                  | `lights_routes::an_ad_hoc_lights_quiet_takes_the_lamps_and_leaves_every_other_leg_alone`                    |
| `a_corrupt_lights_quiet_is_complained_about_once_rather_than_on_every_event`               | `lights_records::a_corrupt_lights_quiet_is_complained_about_once_rather_than_on_every_event`                |
| `a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`                    | `lights_records::a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`                     |
| `the_news_record_is_written_whatever_the_lamps_are_doing`                                  | `lights_records::the_news_record_is_written_whatever_the_lamps_are_doing`                                   |
| `a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch`                 | `lights_records::a_lights_mute_expires_off_this_run_s_own_clock_and_not_off_a_fixed_epoch`                  |
| `a_lights_quiet_write_that_failed_reports_the_disk_and_not_the_list_it_built`              | `lights_records::a_lights_quiet_write_that_failed_reports_the_disk_and_not_the_list_it_built`               |
| `a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it`             | `mutes::a_typed_duration_is_published_as_an_expiry_and_reporting_it_does_not_move_it`                       |
| `off_removes_the_state_file_and_the_next_event_decorates_again`                            | `mutes::off_removes_the_state_file_and_the_next_event_decorates_again`                                      |
| `a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge`                    | `mutes::a_muted_away_event_reaches_the_durable_log_alone_and_never_the_bridge`                              |
| `a_corrupt_state_file_delivers_everything_and_complains_once_per_event`                    | `mutes::a_corrupt_state_file_delivers_everything_and_complains_once_per_event`                              |
| `an_absent_state_file_is_the_ordinary_state_and_says_nothing`                              | `mutes::an_absent_state_file_is_the_ordinary_state_and_says_nothing`                                        |
| `a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state`            | `mutes::a_word_the_mute_does_not_serve_prints_usage_exits_nonzero_and_writes_no_state`                      |
| `a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event`        | `mute_refusals::a_state_file_that_cannot_be_read_delivers_everything_and_complains_once_per_event`          |
| `a_mute_that_could_not_be_written_reports_the_mute_that_still_stands`                      | `mute_refusals::a_mute_that_could_not_be_written_reports_the_mute_that_still_stands`                        |
| `a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind`                | `mute_refusals::a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind`                  |
| `a_publish_whose_rename_fails_leaves_no_pending_file_behind`                               | `mute_refusals::a_publish_whose_rename_fails_leaves_no_pending_file_behind`                                 |
| `every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`            | `hermes_lines::every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before`               |
| `the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`      | `doctor_channels::the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one`      |
| `a_mobile_table_naming_no_compiled_in_backend_pushes_no_card_through_either_seam`          | `doctor_channels::a_mobile_table_naming_no_compiled_in_backend_pushes_no_card_through_either_seam`          |
| `the_doctor_names_the_type_when_the_type_is_the_fault_and_never_the_token`                 | `doctor_channels::the_doctor_names_the_type_when_the_type_is_the_fault_and_never_the_token`                 |
| `the_doctor_tells_a_machine_with_no_config_that_there_is_no_config`                        | `doctor_channels::the_doctor_tells_a_machine_with_no_config_that_there_is_no_config`                        |
| `the_doctor_says_a_switched_off_table_names_no_backend_and_an_event_never_does`            | `doctor_channels::the_doctor_says_a_switched_off_table_names_no_backend_and_an_event_never_does`            |
| `a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`           | `doctor_delivery::a_failure_on_the_first_channel_costs_no_later_leg_its_turn_and_still_exits_one`           |
| `a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`         | `doctor_delivery::a_channel_that_could_not_be_launched_is_a_failure_rather_than_a_send_nobody_made`         |
| `the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`          | `doctor_delivery::the_doctor_reaches_every_channel_through_a_mute_a_desk_and_both_phone_overrides`          |
| `the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`                             | `doctor_delivery::the_doctor_reaches_the_bridge_inside_the_lights_quiet_window`                             |
| `a_pulse_with_no_bridge_to_dial_names_the_settings_rather_than_the_rooms`                  | `doctor_delivery::a_pulse_with_no_bridge_to_dial_names_the_settings_rather_than_the_rooms`                  |
| `a_pulse_the_bridge_answered_nothing_for_still_names_both_causes_it_cannot_choose_between` | `doctor_delivery::a_pulse_the_bridge_answered_nothing_for_still_names_both_causes_it_cannot_choose_between` |
| `a_config_that_enables_nothing_names_every_plugin_sends_nothing_and_exits_one`             | `doctor_delivery::a_config_that_enables_nothing_names_every_plugin_sends_nothing_and_exits_one`             |
| `the_doctor_reads_the_room_off_the_state_file_and_judges_it_against_the_configs_own_rooms` | `doctor_presence::the_doctor_reads_the_room_off_the_state_file_and_judges_it_against_the_configs_own_rooms` |
| `a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel`              | `doctor_presence::a_doctor_given_any_extra_word_prints_usage_exits_two_and_reaches_no_channel`              |
| `an_event_appends_exactly_one_decision_carrying_what_it_decided_and_what_the_legs_did`     | `records::an_event_appends_exactly_one_decision_carrying_what_it_decided_and_what_the_legs_did`             |
| `an_event_that_reached_no_channel_at_all_still_records_its_decision`                       | `records::an_event_that_reached_no_channel_at_all_still_records_its_decision`                               |
| `the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone`                       | `records::the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone`                               |
| `a_state_directory_that_cannot_be_written_costs_the_event_nothing`                         | `records::a_state_directory_that_cannot_be_written_costs_the_event_nothing`                                 |
| `a_fifo_at_the_rings_path_is_never_opened_and_never_parks_the_event`                       | `records::a_fifo_at_the_rings_path_is_never_opened_and_never_parks_the_event`                               |
| `a_ring_holding_bytes_that_are_not_text_heals_to_a_bounded_readable_one`                   | `records::a_ring_holding_bytes_that_are_not_text_heals_to_a_bounded_readable_one`                           |
| `a_ring_that_ends_mid_line_never_fuses_the_next_record_onto_it`                            | `records::a_ring_that_ends_mid_line_never_fuses_the_next_record_onto_it`                                    |
| `a_ring_too_large_to_read_back_is_replaced_rather_than_slurped`                            | `records::a_ring_too_large_to_read_back_is_replaced_rather_than_slurped`                                    |
| `events_racing_each_other_lose_no_line_and_leave_no_pending_file`                          | `records::events_racing_each_other_lose_no_line_and_leave_no_pending_file`                                  |
| `the_doctor_prints_the_decision_section_after_its_summary_newest_first`                    | `doctor_records::the_doctor_prints_the_decision_section_after_its_summary_newest_first`                     |
| `the_doctors_exit_code_does_not_move_for_a_log_that_is_absent_or_unreadable`               | `doctor_records::the_doctors_exit_code_does_not_move_for_a_log_that_is_absent_or_unreadable`                |
| `a_ring_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`          | `doctor_records::a_ring_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`           |
| `a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind`                 | `doctor_records::a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind`                  |
| `the_doctor_records_no_decision_of_its_own`                                                | `doctor_records::the_doctor_records_no_decision_of_its_own`                                                 |
| `the_shared_append_prunes_each_ring_to_its_own_callers_depth`                              | `journal::the_shared_append_prunes_each_ring_to_its_own_callers_depth`                                      |
| `a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown`           | `journal::a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown`                   |
| `a_delivered_event_journals_nothing_at_all`                                                | `journal::a_delivered_event_journals_nothing_at_all`                                                        |
| `the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone`                       | `journal::the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone`                               |
| `a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event`               | `journal::a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event`                       |
| `a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing`                    | `journal::a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing`                            |
| `the_journal_is_created_readable_and_writable_by_its_owner_alone`                          | `journal::the_journal_is_created_readable_and_writable_by_its_owner_alone`                                  |
| `the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it`                  | `doctor_journal::the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it`                   |
| `a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`       | `doctor_journal::a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`        |
| `a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`              | `doctor_journal::a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`               |
| `the_doctor_leaves_the_journal_exactly_as_it_found_it`                                     | `doctor_journal::the_doctor_leaves_the_journal_exactly_as_it_found_it`                                      |
| `a_present_event_delivers_one_extra_notification_carrying_the_whole_journal`               | `replay::a_present_event_delivers_one_extra_notification_carrying_the_whole_journal`                        |
| `a_replay_is_never_a_second_event_in_the_ring_or_the_journal`                              | `replay::a_replay_is_never_a_second_event_in_the_ring_or_the_journal`                                       |
| `an_away_event_delivers_no_replay_and_leaves_the_journal_byte_identical`                   | `replay::an_away_event_delivers_no_replay_and_leaves_the_journal_byte_identical`                            |
| `a_switched_off_replay_card_delivers_no_catch_up_and_leaves_the_journal_whole`             | `replay::a_switched_off_replay_card_delivers_no_catch_up_and_leaves_the_journal_whole`                      |
| `a_switched_off_replay_card_still_journals_the_misses_it_makes`                            | `replay::a_switched_off_replay_card_still_journals_the_misses_it_makes`                                     |
| `a_muted_event_queues_its_own_miss_and_replays_nothing`                                    | `replay::a_muted_event_queues_its_own_miss_and_replays_nothing`                                             |
| `an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`       | `focus::an_event_raised_inside_a_focus_the_config_names_decorates_nothing_and_is_journaled`                 |
| `an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual`              | `focus::an_event_raised_inside_a_focus_the_config_never_named_is_delivered_as_usual`                        |
| `a_focus_store_that_cannot_be_read_costs_no_notification_at_all`                           | `focus::a_focus_store_that_cannot_be_read_costs_no_notification_at_all`                                     |
| `a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay`              | `replay_refusals::a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay`              |
| `an_event_with_nothing_waiting_delivers_and_leaves_exactly_what_it_did_before`             | `replay_refusals::an_event_with_nothing_waiting_delivers_and_leaves_exactly_what_it_did_before`             |
| `an_event_narrowed_to_no_channel_at_all_leaves_the_journal_where_it_found_it`              | `replay_refusals::an_event_narrowed_to_no_channel_at_all_leaves_the_journal_where_it_found_it`              |
| `the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`                     | `replay_refusals::the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`                     |
| `a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`                   | `replay_claims::a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`                     |
| `a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return`                      | `replay_claims::a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return`                        |
| `a_held_batch_whose_owner_is_still_running_is_left_exactly_where_it_is`                    | `replay_claims::a_held_batch_whose_owner_is_still_running_is_left_exactly_where_it_is`                      |
| `a_held_batch_whose_owner_is_gone_is_adopted_exactly_once`                                 | `replay_claims::a_held_batch_whose_owner_is_gone_is_adopted_exactly_once`                                   |
| `an_unreadable_old_claim_cannot_starve_the_good_batch_behind_it`                           | `replay_claims::an_unreadable_old_claim_cannot_starve_the_good_batch_behind_it`                             |
| `a_hand_planted_negative_hold_name_is_never_read_as_a_pid`                                 | `replay_claims::a_hand_planted_negative_hold_name_is_never_read_as_a_pid`                                   |
| `a_line_nothing_can_parse_costs_the_entries_around_it_nothing`                             | `replay_claims::a_line_nothing_can_parse_costs_the_entries_around_it_nothing`                               |
| `a_present_event_narrowed_to_the_log_leaves_the_queue_for_a_surface_that_shows_it`         | `replay_claims::a_present_event_narrowed_to_the_log_leaves_the_queue_for_a_surface_that_shows_it`           |
| `a_machine_with_only_a_durable_channel_never_consumes_the_queue_it_cannot_show`            | `replay_claims::a_machine_with_only_a_durable_channel_never_consumes_the_queue_it_cannot_show`              |
| `a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found`                  | `replay_claims::a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found`                    |
| `racing_present_events_deliver_exactly_one_replay_between_them`                            | `replay_races::racing_present_events_deliver_exactly_one_replay_between_them`                               |
| `racing_present_events_adopt_one_stranded_claim_exactly_once`                              | `replay_races::racing_present_events_adopt_one_stranded_claim_exactly_once`                                 |
| `every_event_is_recorded_in_the_activity_ring_delivered_or_not`                            | `activity::every_event_is_recorded_in_the_activity_ring_delivered_or_not`                                   |
| `a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line`           | `activity::a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line`                  |
| `a_present_event_moves_the_last_present_marker_and_an_away_event_does_not`                 | `return_window::a_present_event_moves_the_last_present_marker_and_an_away_event_does_not`                   |
| `an_activity_window_with_no_marker_to_open_it_recaps_nothing_and_still_catches_up`         | `return_window::an_activity_window_with_no_marker_to_open_it_recaps_nothing_and_still_catches_up`           |
| `a_marker_no_reader_can_parse_opens_no_window_rather_than_one_from_epoch_zero`             | `return_window::a_marker_no_reader_can_parse_opens_no_window_rather_than_one_from_epoch_zero`               |
| `events_stamped_at_the_markers_own_second_belong_to_it_and_not_to_the_window_after`        | `return_window::events_stamped_at_the_markers_own_second_belong_to_it_and_not_to_the_window_after`          |
| `a_window_under_the_threshold_delivers_the_catch_up_card_unchanged`                        | `return_window::a_window_under_the_threshold_delivers_the_catch_up_card_unchanged`                          |
| `a_window_over_the_threshold_delivers_one_recap_card_with_what_needs_you_first`            | `return_window::a_window_over_the_threshold_delivers_one_recap_card_with_what_needs_you_first`              |
| `the_recap_card_is_exactly_what_the_entries_compose_and_nothing_a_model_said`              | `recap_card::the_recap_card_is_exactly_what_the_entries_compose_and_nothing_a_model_said`                   |
| `the_digest_reaches_discord_from_a_process_the_event_never_waited_for`                     | `recap_process::the_digest_reaches_discord_from_a_process_the_event_never_waited_for`                       |
| `the_recap_child_runs_in_a_process_group_of_its_own`                                       | `recap_process::the_recap_child_runs_in_a_process_group_of_its_own`                                         |
| `a_switched_off_digest_posts_no_recap_and_leaves_the_catch_up_card_alone`                  | `recap_process::a_switched_off_digest_posts_no_recap_and_leaves_the_catch_up_card_alone`                    |
| `a_machine_with_no_durable_route_never_points_a_card_at_a_recap_nothing_can_carry`         | `recap_routes::a_machine_with_no_durable_route_never_points_a_card_at_a_recap_nothing_can_carry`            |
| `the_marker_advances_so_a_second_present_event_recaps_nothing`                             | `recap_routes::the_marker_advances_so_a_second_present_event_recaps_nothing`                                |
| `a_recap_told_a_window_it_cannot_read_prints_usage_exits_two_and_posts_nothing`            | `recap_routes::a_recap_told_a_window_it_cannot_read_prints_usage_exits_two_and_posts_nothing`               |
| `a_window_claim_whose_owner_is_gone_is_adopted_rather_than_lost_or_left_behind`            | `return_moment::a_window_claim_whose_owner_is_gone_is_adopted_rather_than_lost_or_left_behind`              |
| `an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind`                  | `return_moment::an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind`                    |
| `the_windows_near_edge_never_moves_backward_however_late_an_event_publishes`               | `return_moment::the_windows_near_edge_never_moves_backward_however_late_an_event_publishes`                 |
| `racing_present_events_recap_one_loud_window_exactly_once_between_them`                    | `return_moment::racing_present_events_recap_one_loud_window_exactly_once_between_them`                      |
| `a_configured_summarizers_lines_become_the_night_in_order`                                 | `summarizer::a_configured_summarizers_lines_become_the_night_in_order`                                      |
| `the_windows_own_count_and_what_needs_you_survive_whatever_the_model_says`                 | `summarizer::the_windows_own_count_and_what_needs_you_survive_whatever_the_model_says`                      |
| `a_summarizer_that_exits_non_zero_falls_to_the_plain_list_and_says_so`                     | `summarizer_failures::a_summarizer_that_exits_non_zero_falls_to_the_plain_list_and_says_so`                 |
| `a_summarizer_that_answers_with_nothing_falls_to_the_plain_list_and_says_so`               | `summarizer_failures::a_summarizer_that_answers_with_nothing_falls_to_the_plain_list_and_says_so`           |
| `a_summarizer_still_thinking_at_its_deadline_falls_to_the_plain_list_and_says_so`          | `summarizer_failures::a_summarizer_still_thinking_at_its_deadline_falls_to_the_plain_list_and_says_so`      |
| `a_summarizer_that_never_answers_costs_the_card_nothing`                                   | `summarizer_failures::a_summarizer_that_never_answers_costs_the_card_nothing`                               |
| `a_summarizer_that_is_not_installed_at_all_falls_to_the_plain_list_and_says_so`            | `summarizer_failures::a_summarizer_that_is_not_installed_at_all_falls_to_the_plain_list_and_says_so`        |
| `a_summarizer_answering_in_bytes_that_are_not_text_falls_to_the_plain_list`                | `summarizer_failures::a_summarizer_answering_in_bytes_that_are_not_text_falls_to_the_plain_list`            |
| `an_empty_window_says_so_itself_and_never_starts_a_summarizer_at_all`                      | `summarizer_failures::an_empty_window_says_so_itself_and_never_starts_a_summarizer_at_all`                  |
| `a_summarizer_answering_with_a_megabyte_gets_the_plain_list_posted_instead`                | `summarizer_failures::a_summarizer_answering_with_a_megabyte_gets_the_plain_list_posted_instead`            |
| `a_configured_repositorys_merges_become_the_new_behavior_section`                          | `recap_merges::a_configured_repositorys_merges_become_the_new_behavior_section`                             |
| `a_gh_that_will_not_answer_costs_the_recap_only_its_own_section`                           | `recap_merges::a_gh_that_will_not_answer_costs_the_recap_only_its_own_section`                              |
| `no_repos_key_means_no_gh_process_is_ever_started`                                         | `recap_merges::no_repos_key_means_no_gh_process_is_ever_started`                                            |
| `a_pull_request_body_of_somebody_elses_text_reaches_discord_as_one_cited_line`             | `recap_merges::a_pull_request_body_of_somebody_elses_text_reaches_discord_as_one_cited_line`                |
| `only_the_notes_the_glob_names_and_the_window_covers_are_ever_read`                        | `recap_notes::only_the_notes_the_glob_names_and_the_window_covers_are_ever_read`                            |
| `a_glob_that_matches_nothing_says_so_and_one_pointing_nowhere_says_something_else`         | `recap_notes::a_glob_that_matches_nothing_says_so_and_one_pointing_nowhere_says_something_else`             |
| `a_note_that_matched_and_would_not_open_says_so_rather_than_vanishing`                     | `recap_notes::a_note_that_matched_and_would_not_open_says_so_rather_than_vanishing`                         |
| `a_summarized_merge_section_keeps_only_the_lines_its_own_sources_vouch_for`                | `recap_notes::a_summarized_merge_section_keeps_only_the_lines_its_own_sources_vouch_for`                    |
| `one_recap_spends_one_summarizer_budget_however_many_questions_it_asks`                    | `recap_notes::one_recap_spends_one_summarizer_budget_however_many_questions_it_asks`                        |
| `the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section`       | `doctor_pairing::the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section`        |
| `the_doctor_runs_moshi_hook_exactly_twice_and_never_probes`                                | `doctor_pairing::the_doctor_runs_moshi_hook_exactly_twice_and_never_probes`                                 |
| `a_doctor_with_no_moshi_hook_to_run_says_so_and_leaves_the_exit_code_to_the_sends`         | `doctor_pairing::a_doctor_with_no_moshi_hook_to_run_says_so_and_leaves_the_exit_code_to_the_sends`          |
| `a_moshi_hook_that_never_returns_does_not_park_the_doctor`                                 | `doctor_pairing::a_moshi_hook_that_never_returns_does_not_park_the_doctor`                                  |
| `an_unpaired_host_exits_one_while_the_summary_still_reads_zero_failed`                     | `doctor_pairing::an_unpaired_host_exits_one_while_the_summary_still_reads_zero_failed`                      |
| `the_pairing_check_records_nothing_of_its_own`                                             | `doctor_pairing::the_pairing_check_records_nothing_of_its_own`                                              |
| `an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read`                     | `doctor_pairing::an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read`                      |
| `the_doctor_tells_the_truth_about_a_named_focus_in_every_state`                            | `doctor_focus::the_doctor_tells_the_truth_about_a_named_focus_in_every_state`                               |
| `a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health`               | `doctor_focus::a_mode_catalog_the_doctor_cannot_read_is_said_and_never_reported_as_health`                  |
| `an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer`                    | `loop_lease::an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer`                         |
| `a_registration_that_cannot_be_written_costs_the_event_nothing`                            | `loop_lease::a_registration_that_cannot_be_written_costs_the_event_nothing`                                 |
| `a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold`                | `loop_lease::a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold`                     |
| `a_tick_with_nothing_in_flight_lets_its_own_lease_lapse`                                   | `loop_lease::a_tick_with_nothing_in_flight_lets_its_own_lease_lapse`                                        |
| `a_lease_taken_by_hand_schedules_the_tick_that_reads_it`                                   | `loop_lease::a_lease_taken_by_hand_schedules_the_tick_that_reads_it`                                        |
| `a_bare_lights_command_is_a_usage_error_rather_than_an_event`                              | `lights_tick::a_bare_lights_command_is_a_usage_error_rather_than_an_event`                                  |
| `the_tick_says_nothing_at_all_however_many_times_it_runs`                                  | `lights_tick::the_tick_says_nothing_at_all_however_many_times_it_runs`                                      |
| `the_tick_exits_zero_with_no_config_no_table_hue_off_and_an_unreachable_bridge`            | `lights_tick::the_tick_exits_zero_with_no_config_no_table_hue_off_and_an_unreachable_bridge`                |
| `the_operators_return_puts_out_a_glow_without_any_daemon_running`                          | `lights_tick::the_operators_return_puts_out_a_glow_without_any_daemon_running`                              |
| `an_event_holding_no_glow_reaches_the_bridge_for_nothing`                                  | `lights_tick::an_event_holding_no_glow_reaches_the_bridge_for_nothing`                                      |
| `switching_the_lamps_off_puts_out_a_held_glow_and_switching_hue_off_keeps_the_record`      | `lights_tick::switching_the_lamps_off_puts_out_a_held_glow_and_switching_hue_off_keeps_the_record`          |
| `a_tick_with_nothing_left_to_show_puts_out_the_glow_it_was_holding`                        | `lights_tick::a_tick_with_nothing_left_to_show_puts_out_the_glow_it_was_holding`                            |

## Shared support decomposition, plan 16

The original 849-line support module moves into its existing sandbox, stubs, router, daemon guard,
process and budget responsibilities. All 56 function signatures and body tokens, literal strings and
original rationale remain, with only parent visibility and formatter changes. The largest child is 172
lines. This move adds no tests. The following full names remain unchanged in each of the six integration
targets that compile support, retaining all 96 occurrences. They remain harness speed checks, with the
existing historical limitations, rather than product delivery guarantees.

| Previous and successor full name                                                      |
| ------------------------------------------------------------------------------------- |
| `support::guard_tests::a_ci_run_resolves_a_ceiling_four_times_the_local_one`          |
| `support::guard_tests::a_fast_sandbox_is_not_over_budget`                             |
| `support::guard_tests::a_real_sandbox_past_the_ceiling_fails_naming_the_test_budget`  |
| `support::guard_tests::a_real_sandbox_past_the_ceiling_with_allow_slow_does_not_fail` |
| `support::guard_tests::a_real_sandbox_well_inside_the_ceiling_does_not_fail`          |
| `support::guard_tests::a_sandbox_exactly_on_the_ceiling_is_not_over_it`               |
| `support::guard_tests::a_sandbox_one_ms_past_the_ceiling_is_over_it`                  |
| `support::guard_tests::a_sandbox_over_the_local_line_is_still_inside_the_ci_one`      |
| `support::guard_tests::a_sandbox_past_the_budget_is_over_budget`                      |
| `support::guard_tests::a_sandbox_past_the_ceiling_with_no_excuse_is_over_ceiling`     |
| `support::guard_tests::an_already_panicking_thread_is_never_double_panicked`          |
| `support::guard_tests::an_empty_ci_variable_is_not_a_ci_run`                          |
| `support::guard_tests::an_excused_sandbox_is_never_over_ceiling`                      |
| `support::guard_tests::any_non_empty_ci_value_is_a_ci_run`                            |
| `support::guard_tests::no_ci_signal_resolves_the_local_ceiling`                       |
| `support::guard_tests::the_live_ceiling_follows_this_process_environment`             |

## Durable return handoff, plan 11.4

Eleven new leaves cover the S158, S242 and S243 successor behavior below. The original S159 predicate and
submission-tail change is a separate part of the same delivery batch. Numbers refer to
`specs/persistence-and-process-lifecycle.md`. The historical schema names advance current/future values
from 3/4 to 4/5 and retain their existing privacy, rollback and refusal assertions.

| Added leaf                                                                                                                               | Behavior |
| ---------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| `persistence::sqlite::ledger::tests::journal::a_completed_original_cannot_be_rejournaled_and_duplicate_pending_identity_is_not_appended` | 47       |
| `persistence::sqlite::ledger::tests::journal::mixed_legacy_and_keyed_journal_appends_preserve_bytes_and_identity_across_pruning`         | 47       |
| `persistence::sqlite::ledger::tests::journal::only_an_acknowledged_decorative_leg_clears_its_original_keyed_miss`                        | 47       |
| `persistence::sqlite::ledger::tests::journal::refusing_keyed_miss_removal_rolls_back_completion_and_preserves_the_claim`                 | 47       |
| `persistence::sqlite::tests::returns::replay::a_queued_replay_retains_original_identity_and_never_retries_its_acknowledged_leg`          | 46       |
| `persistence::sqlite::tests::returns::replay::an_abandoned_replay_keeps_its_batch_and_window_separate_from_later_arrivals`               | 46       |
| `persistence::sqlite::tests::returns::replay::an_empty_journal_digest_still_has_one_owned_return_batch`                                  | 46       |
| `replay_missed::tests::handoff::a_replay_keeps_the_original_batch_identity_and_window_after_adoption`                                    | 46       |
| `replay_missed::tests::handoff::a_replay_not_owned_by_the_ledger_preserves_its_journal`                                                  | 46       |
| `replay_missed::tests::handoff::an_adopted_queued_replay_completes_without_publishing_or_dispatching_again`                              | 46       |
| `return_replay::tests::only_a_persisted_or_existing_submission_transfers_the_replay_journal`                                             | 46       |

The retained leaf `abandoned_legacy_batches_keep_their_order_and_survive_pending_ring_pruning` now claims
the abandoned batch separately before the later pending window. The killed-owner successor
`a_committed_journal_hold_is_adopted_after_its_owned_process_exits` retains the batch's original near
edge. The retained live-owner leaf now refuses the whole return instead of returning an empty successful
claim. Their source preimages and updated bodies are preserved in the delivery evidence; unchanged name
counts alone do not establish unchanged behavior.

The historical `replay_refusals::the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` is
replaced by
`replay_refusals::a_queued_replay_releases_its_journal_after_attempts_and_preserves_it_on_interruption`.
It preserves the completed and interrupted arms, and now observes the ledger event joined to the exact
held request identity before interrupting dispatch. Its owned fixture deadline is 650 milliseconds. A
failed destination can still leave a completed journal handoff because its retry is owned by the ledger.
The unpersisted-handoff case above distinguishes that from failure before ownership. Existing
file-protocol fixtures explicitly supply their simulated durable handoff; production replay uses SQLite
and the shared submission runtime.

The schema-5 storage continuation adds
`persistence::sqlite::ledger::tests::metadata::schema_four_rows_stay_without_metadata_while_original_request_metadata_survives_and_controls_duplicates`,
mapped to persistence-and-process-lifecycle statement 48. It observes a schema-4 row, exact retained
metadata after reopening, duplicate conflict direction and an actual retry claim. Independent omitted
write, ignored equality and skipped migration faults each fail this leaf. Existing schema tests retain
their names and assertions, with the current/future version fixtures advanced to 5/6. Constructor-only
None additions preserve the existing legacy and aggregate fixtures.

## Configured delivery classes

This behavior change preserves existing leaf names. Existing `DecisionRequest` fixture constructors
explicitly use `SilencePolicy::Respect`; their assertions are unchanged. The new leaves map as follows:

| Added leaf                                                                                                           | Contract                                                                                                                  |
| -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `request::tests::classes::a_delivery_class_round_trips_but_absence_keeps_the_original_request_bytes`                 | protocol-v1/S011: typed class, bounds, correlated refusal and unchanged unmarked bytes                                    |
| `config::tests::delivery::delivery_classes_default_explicitly_and_only_valid_configured_names_can_bypass`            | quiet-behavior 7: explicit default, exact membership and malformed-policy refusal                                         |
| `decision::tests::delivery_class::a_class_exception_preserves_only_the_selected_banner_and_phone_under_each_silence` | quiet-behavior 7: selected surface only, pulse stays off, scope and skip preserved                                        |
| `submit_notification::tests::gates::an_original_class_exception_does_not_unmute_an_unmarked_return_summary`          | quiet-behavior 7: original exception does not replay an aggregate through silence                                         |
| `delivery_class::json_class_policy_crosses_the_real_mute_and_focus_edge_without_changing_hermes`                     | protocol-v1/S011 and quiet-behavior 7: real JSON edge, retained metadata, duplicate conflict and unchanged Hermes payload |
| `delivery_class::malformed_class_or_configuration_never_grants_a_mute_exception`                                     | protocol-v1/S011 and quiet-behavior 7: refusal before effects and invalid-config failure direction                        |

The focused class tests run with private state, inert executable destinations and bounded owned children.
They establish this policy only; the final combined repository sweep remains the delivery gate.
