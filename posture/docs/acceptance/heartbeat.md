# Heartbeat acceptance map

The 2.5 heartbeat policy, readers and clock, and the 5.1 producer are implemented. The 6.1 command now
composes them. Its proposed named route `posture` still requires the operator's pns-keyed Hermes binding,
so the plist change and Bash retirement remain held. The 17 current Bash tests retain their recorded
passing results and unchanged assertions; the private harness only redirected the repository root and
retained its owned fixtures instead of deleting them. The original historical names remain in
`test-baseline.tsv`. No existing heartbeat test is retired here.

The current tests in `test/integration/osquery-heartbeat.test.sh` map to the following typed pins. Their
Bash route and sound assertions remain necessary until the operator verifies the proposed route; an
application observation alone does not prove Discord or banner delivery.

| Current Bash leaf, retained                                                            | Typed pin                                                                              |
| -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `test_a_fresh_canary_sends_exactly_one_crit_message_that_reads_healthy`                | `heartbeat::tests::fresh_canary_submits_one_silent_observation_after_clock_and_log`    |
| `test_the_healthy_message_is_silent_so_a_proof_of_life_never_pings`                    | `heartbeat::tests::fresh_canary_submits_one_silent_observation_after_clock_and_log`    |
| `test_a_stale_canary_reports_unhealthy_the_stopped_daemon_case_a_one_shot_would_miss`  | `heartbeat::tests::missing_unreadable_stale_and_implausible_are_observations`          |
| `test_the_unhealthy_message_is_also_silent_so_the_heartbeat_never_pings_even_degraded` | `heartbeat::tests::missing_unreadable_stale_and_implausible_are_observations`          |
| `test_no_canary_at_all_reports_unhealthy_as_missing_never_a_blind_checkmark`           | `heartbeat::tests::missing_unreadable_stale_and_implausible_are_observations`          |
| `test_the_healthy_message_is_honest_about_what_it_verified`                            | `heartbeat::tests::captured_fresh`                                                     |
| `test_a_malformed_canary_timestamp_is_rejected_unhealthy_and_cannot_inject`            | `canary::tests::decimal_epoch_accepts_zero_and_ten_digits_only`                        |
| `test_an_over_range_canary_epoch_is_rejected_never_a_64_bit_overflow_false_fresh`      | `canary::tests::decimal_epoch_accepts_zero_and_ten_digits_only`                        |
| `test_a_leading_zero_canary_epoch_is_rejected_never_an_octal_parse_fall_through`       | `canary::tests::decimal_epoch_accepts_zero_and_ten_digits_only`                        |
| `test_freshness_is_judged_from_the_newest_canary_row_when_several_exist`               | `snapshots_log::tests::captured_last_not_maximum`                                      |
| `test_a_spaced_json_canary_reads_the_same_as_compact`                                  | `snapshots_log::tests::captured_spaced`                                                |
| `test_a_future_dated_canary_reads_healthy_with_a_non_negative_rendered_age`            | `canary::tests::future_boundary_and_both_neighbors_clamp_age_to_zero`                  |
| `test_the_healthy_body_is_a_recent_observation_not_a_present_tense_overclaim`          | `heartbeat::tests::captured_fresh`                                                     |
| `test_a_canary_far_in_the_future_reports_unhealthy_implausible_not_healthy`            | `canary::tests::future_boundary_and_both_neighbors_clamp_age_to_zero`                  |
| `test_a_non_numeric_clock_reports_unhealthy_never_false_healthy_via_now_zero`          | `heartbeat::tests::unknown_clock_submits_unverified_without_reading_snapshots`         |
| `test_newest_canary_timestamp_returns_the_newest_validated_integer_else_empty`         | `snapshots_log::tests::captured_invalid_last_masks_prior`                              |
| `test_a_hard_send_failure_never_fails_the_heartbeat`                                   | `heartbeat::tests::every_submission_failure_is_fire_and_forget_without_retry_or_state` |

S195 gains `snapshots_log::tests::captured_torn_before_and_after_valid`. S201 gains the exact zero-age
assertion in `canary::tests::future_boundary_and_both_neighbors_clamp_age_to_zero`, beyond the old
negative-parenthesis check. Both sides and both neighbors of the freshness boundary are pinned. The ten
text captures include resolved default-bound and send-failure inputs; those text tests do not claim to
implement command-line parsing or delivery. No argument has been cut over yet.

The shared formatter's adapter contracts move without body changes:

| Previous leaf                                             | Successor leaf                                   |
| --------------------------------------------------------- | ------------------------------------------------ |
| `allowlist_projection::number::tests::captured_number_0`  | `legacy_json::number::tests::captured_number_0`  |
| `allowlist_projection::number::tests::captured_number_1`  | `legacy_json::number::tests::captured_number_1`  |
| `allowlist_projection::number::tests::captured_number_2`  | `legacy_json::number::tests::captured_number_2`  |
| `allowlist_projection::number::tests::captured_number_3`  | `legacy_json::number::tests::captured_number_3`  |
| `allowlist_projection::number::tests::captured_number_4`  | `legacy_json::number::tests::captured_number_4`  |
| `allowlist_projection::number::tests::captured_number_5`  | `legacy_json::number::tests::captured_number_5`  |
| `allowlist_projection::number::tests::captured_number_6`  | `legacy_json::number::tests::captured_number_6`  |
| `allowlist_projection::number::tests::captured_number_7`  | `legacy_json::number::tests::captured_number_7`  |
| `allowlist_projection::number::tests::captured_number_8`  | `legacy_json::number::tests::captured_number_8`  |
| `allowlist_projection::number::tests::captured_number_9`  | `legacy_json::number::tests::captured_number_9`  |
| `allowlist_projection::number::tests::captured_number_10` | `legacy_json::number::tests::captured_number_10` |
| `allowlist_projection::number::tests::captured_number_11` | `legacy_json::number::tests::captured_number_11` |
| `allowlist_projection::number::tests::captured_number_12` | `legacy_json::number::tests::captured_number_12` |
| `allowlist_projection::number::tests::captured_number_13` | `legacy_json::number::tests::captured_number_13` |
| `allowlist_projection::number::tests::captured_number_14` | `legacy_json::number::tests::captured_number_14` |
| `allowlist_projection::number::tests::captured_number_15` | `legacy_json::number::tests::captured_number_15` |
| `allowlist_projection::number::tests::captured_number_16` | `legacy_json::number::tests::captured_number_16` |
| `allowlist_projection::number::tests::captured_number_17` | `legacy_json::number::tests::captured_number_17` |
| `allowlist_projection::number::tests::captured_number_18` | `legacy_json::number::tests::captured_number_18` |
| `allowlist_projection::number::tests::captured_number_19` | `legacy_json::number::tests::captured_number_19` |
| `allowlist_projection::number::tests::captured_number_20` | `legacy_json::number::tests::captured_number_20` |
| `allowlist_projection::number::tests::captured_number_21` | `legacy_json::number::tests::captured_number_21` |
| `allowlist_projection::number::tests::captured_number_22` | `legacy_json::number::tests::captured_number_22` |
| `allowlist_projection::number::tests::captured_number_23` | `legacy_json::number::tests::captured_number_23` |
| `allowlist_projection::number::tests::captured_number_24` | `legacy_json::number::tests::captured_number_24` |
| `allowlist_projection::number::tests::captured_number_25` | `legacy_json::number::tests::captured_number_25` |
| `allowlist_projection::number::tests::captured_number_26` | `legacy_json::number::tests::captured_number_26` |
| `allowlist_projection::number::tests::captured_number_27` | `legacy_json::number::tests::captured_number_27` |
| `allowlist_projection::number::tests::captured_number_28` | `legacy_json::number::tests::captured_number_28` |
| `allowlist_projection::number::tests::captured_number_29` | `legacy_json::number::tests::captured_number_29` |
| `allowlist_projection::number::tests::captured_number_30` | `legacy_json::number::tests::captured_number_30` |
| `allowlist_projection::number::tests::captured_number_31` | `legacy_json::number::tests::captured_number_31` |
| `allowlist_projection::number::tests::captured_number_32` | `legacy_json::number::tests::captured_number_32` |
| `allowlist_projection::number::tests::captured_number_33` | `legacy_json::number::tests::captured_number_33` |
| `allowlist_projection::number::tests::captured_number_34` | `legacy_json::number::tests::captured_number_34` |
| `allowlist_projection::number::tests::captured_number_35` | `legacy_json::number::tests::captured_number_35` |
| `allowlist_projection::number::tests::captured_number_36` | `legacy_json::number::tests::captured_number_36` |
| `allowlist_projection::number::tests::captured_number_37` | `legacy_json::number::tests::captured_number_37` |
| `allowlist_projection::number::tests::captured_number_38` | `legacy_json::number::tests::captured_number_38` |
| `allowlist_projection::number::tests::captured_number_39` | `legacy_json::number::tests::captured_number_39` |
| `allowlist_projection::number::tests::captured_number_40` | `legacy_json::number::tests::captured_number_40` |
| `allowlist_projection::number::tests::captured_number_41` | `legacy_json::number::tests::captured_number_41` |
| `allowlist_projection::number::tests::captured_number_42` | `legacy_json::number::tests::captured_number_42` |
| `allowlist_projection::number::tests::captured_number_43` | `legacy_json::number::tests::captured_number_43` |
| `allowlist_projection::number::tests::captured_number_44` | `legacy_json::number::tests::captured_number_44` |
| `allowlist_projection::number::tests::captured_number_45` | `legacy_json::number::tests::captured_number_45` |
| `allowlist_projection::number::tests::captured_number_46` | `legacy_json::number::tests::captured_number_46` |
| `allowlist_projection::number::tests::captured_number_47` | `legacy_json::number::tests::captured_number_47` |
| `allowlist_projection::number::tests::captured_number_48` | `legacy_json::number::tests::captured_number_48` |
| `allowlist_projection::number::tests::captured_number_49` | `legacy_json::number::tests::captured_number_49` |
| `allowlist_projection::number::tests::captured_number_50` | `legacy_json::number::tests::captured_number_50` |
| `allowlist_projection::number::tests::captured_number_51` | `legacy_json::number::tests::captured_number_51` |
| `allowlist_projection::number::tests::captured_number_52` | `legacy_json::number::tests::captured_number_52` |
| `allowlist_projection::number::tests::captured_number_53` | `legacy_json::number::tests::captured_number_53` |
| `allowlist_projection::number::tests::captured_number_54` | `legacy_json::number::tests::captured_number_54` |

New behavior pins, classified by their owner:

| Source                                                     | Leaf                                                                 | Classification                |
| ---------------------------------------------------------- | -------------------------------------------------------------------- | ----------------------------- |
| `crates/posture-adapters/src/clock/tests.rs`               | `epoch_and_seconds_within_day_are_utc`                               | adapter contract              |
| `crates/posture-adapters/src/clock/tests.rs`               | `utc_day_changes_at_midnight_and_handles_leap_day`                   | adapter contract              |
| `crates/posture-adapters/src/clock/tests.rs`               | `a_pre_epoch_clock_is_unavailable_instead_of_zero`                   | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests/files.rs` | `real_file_reads_a_canary_after_more_than_one_read_buffer`           | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests/files.rs` | `absent_log_and_directory_fail_without_a_false_canary`               | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests/files.rs` | `a_fifo_log_refuses_without_waiting_for_a_writer`                    | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests/files.rs` | `a_read_failure_is_not_an_empty_successful_snapshot`                 | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_empty`                                                     | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_compact`                                                   | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_spaced`                                                    | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_other_name`                                                | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_last_not_maximum`                                          | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_invalid_last_masks_prior`                                  | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_null_last_retains_prior`                                   | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_false_last_retains_prior`                                  | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_torn_before_and_after_valid`                               | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_no_final_newline`                                          | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_fallback`                                                  | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_false_fallback`                                            | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_prefer_envelope`                                           | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_zero`                                                      | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_ten_digit_maximum`                                         | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_eleven_digits`                                             | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_leading_zero_string`                                       | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_leading_zero_number`                                       | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_numeric_exponent`                                          | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_numeric_fraction`                                          | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_negative_zero`                                             | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_positive_number`                                           | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_nan`                                                       | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_infinity`                                                  | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_inert_nan`                                                 | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_inert_low_surrogate`                                       | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_inert_high_surrogate`                                      | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_invalid_utf8_inert`                                        | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_duplicate_timestamp`                                       | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_duplicate_name`                                            | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_string_newline_last_value`                                 | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_string_newline_invalid_tail`                               | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_string_trailing_newline`                                   | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_string_nul`                                                | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_empty_string_masks_prior`                                  | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_array_masks_prior`                                         | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_object_masks_prior`                                        | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_scalar_between_rows`                                       | adapter contract              |
| `crates/posture-adapters/src/snapshots_log/tests.rs`       | `captured_deep_inert`                                                | adapter contract              |
| `crates/posture-application/src/heartbeat/tests.rs`        | `fresh_canary_submits_one_silent_observation_after_clock_and_log`    | permanent behavioral contract |
| `crates/posture-application/src/heartbeat/tests.rs`        | `unknown_clock_submits_unverified_without_reading_snapshots`         | permanent behavioral contract |
| `crates/posture-application/src/heartbeat/tests.rs`        | `missing_unreadable_stale_and_implausible_are_observations`          | permanent behavioral contract |
| `crates/posture-application/src/heartbeat/tests.rs`        | `every_submission_failure_is_fire_and_forget_without_retry_or_state` | permanent behavioral contract |
| `crates/posture-domain/src/canary/tests.rs`                | `decimal_epoch_accepts_zero_and_ten_digits_only`                     | permanent behavioral contract |
| `crates/posture-domain/src/canary/tests.rs`                | `past_boundary_and_both_neighbors`                                   | permanent behavioral contract |
| `crates/posture-domain/src/canary/tests.rs`                | `future_boundary_and_both_neighbors_clamp_age_to_zero`               | permanent behavioral contract |
| `crates/posture-domain/src/canary/tests.rs`                | `missing_is_distinct_from_a_real_stale_epoch`                        | permanent behavioral contract |
| `crates/posture-domain/src/canary/tests.rs`                | `zero_window_requires_equality`                                      | permanent behavioral contract |
| `crates/posture-domain/src/canary/tests.rs`                | `clock_distance_does_not_wrap_into_freshness`                        | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_fresh`                                                     | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_past_boundary`                                             | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_stale`                                                     | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_future`                                                    | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_future_boundary`                                           | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_implausible`                                               | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_missing`                                                   | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_clock_unknown`                                             | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_send_failure`                                              | permanent behavioral contract |
| `crates/posture-domain/src/heartbeat/tests.rs`             | `captured_invalid_bound`                                             | permanent behavioral contract |

## Heartbeat command, plan 6.1

The real command runs against a private installed-engine double. It ignores trailing operands, reads the
HOME-derived snapshot path despite a conflicting legacy override, preserves `020` as the displayed
sixteen-second bound, and submits one unmarked observation on `posture`. The double acknowledges the
original request identity with `ledger_committed`. The command exits zero with empty output and leaves
snapshot bytes and heartbeat state untouched. The existing four application tests retain their bodies;
only their subject constructor now supplies `HeartbeatWindow`.

| New qualified leaf                                                                                                             | Statements and contract                                               | Caught source fault                           |
| ------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- | --------------------------------------------- |
| `posture-domain: heartbeat::window::tests::literal_bounds_preserve_valid_octal_text_and_refuse_invalid_or_overflow_values`     | S200/S203, lexical octal bounds and checked full unsigned range       | decimalized octal; unsigned bound clamped     |
| `posture-domain: heartbeat::window::tests::invalid_literals_request_a_diagnostic_but_non_numeric_overrides_keep_quiet_default` | S200/S203, deliberate invalid-literal validation                      | invalid literal silenced                      |
| `posture-cli: heartbeat::configuration::tests::home_constructs_paths_without_reading_legacy_path_overrides`                    | Section 3.11 and S200, raw HOME paths and construction-only injection | legacy path override reintroduced             |
| `posture-cli: heartbeat::tests::the_command_reads_the_selected_canary_and_submits_one_unmarked_posture_observation`            | S198/S204/S205, actual producer composition and no baseline           | route omitted                                 |
| `posture-cli: heartbeat::tests::engine_failure_attempts_an_independent_alarm_but_never_changes_best_effort_status`             | S205 and section 5.1, failure direction and independent alarm         | alarm path replaced; best-effort exit changed |
| `posture-cli: heartbeat::tests::invalid_literal_emits_one_fixed_diagnostic_and_still_submits_the_default_observation`          | S200/S203, fixed diagnostic with default observation                  | invalid-literal diagnostic omitted            |
| `posture-cli/tests/heartbeat: heartbeat_ignores_trailing_operands_and_invokes_the_private_installed_engine_once`               | S193/S200/S204/S205, real command argument and engine boundary        | trailing operands refused                     |

All seven new leaves failed their actual assertions before implementation. The exact `u64::MAX` assertion
also catches the independent clamping fault. Nine source faults fail the named tests after compilation of
their actual source; each fault and each healthy case completes within one second. The initial
path-override parity test and fault are superseded by the section 3.11 contract. Their raw results remain
retained; the revised configuration and real command cases both failed against the environment-reading
predecessor before that correction. The existing usage test retains its name and removes only heartbeat
from its unimplemented-word inputs.

The captures deliberately differ in five cases. With a seventeen-second old canary, `08` and
`18446744073709551616` now use 1800 and report healthy with the fixed diagnostic, instead of Bash's
arithmetic errors or wrapped stale result. `9223372036854775808` and `18446744073709551615` retain their
actual unsigned bounds and report healthy, instead of a negative implausible skew or stale result. With a
seventeen-second future canary and `08`, the default bound reports healthy with age zero and the same
fixed diagnostic, instead of a negative stale age. These are deliberate S200/S203 validation fixes; valid
in-range literals and nonnumeric defaults retain their captured text and decisions.

The command does not retire the source-only canary seam from S206. The first 17-row table remains the
exact Bash-to-Rust successor map; the new command and producer cases establish composition, while silent
Discord/banner arrival and the named route remain operator acceptance. No deployment test, live send,
LaunchAgent load, source deletion or installed-script removal was performed.

Production `OSQUERY_SNAPSHOTS_LOG` overrides are intentionally retired under section 3.11. The command
ignores an alternate legacy path while reading the HOME-derived snapshot; direct configuration injection
continues to test selected private paths. This path change is separate from the five arithmetic
corrections above. No later heartbeat exception to section 3.11 was found.
