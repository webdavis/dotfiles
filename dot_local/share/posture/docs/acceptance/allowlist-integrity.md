# Allowlist and integrity acceptance map

The original 57 workspace test names and outcomes remain. This row adds the following domain tests. The
baseline, Bash captures, fail-first results and independent mutation controls are retained in the
delivery evidence. Existing shell tests remain in place until their entry-point cutovers.

## Existing named routing contracts

| Existing leaf                                                                                       | New composition leaf                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed                | allowlist::tests::routing::c4a_full_allowlisted_tuple_is_suppressed                                                                                                      |
| C4b: the same allowlisted label with a different program pages (reused label)                       | allowlist::tests::routing::c4b_reused_label_pages                                                                                                                        |
| C4c: an unknown user LaunchAgent pages (default-deny, operator ruling)                              | allowlist::tests::routing::c4c_unknown_user_agent_pages                                                                                                                  |
| C4d: an allowlisted-but-untrusted program pages (enrichment beats suppression)                      | allowlist::tests::routing::c4d_allowlisted_but_untrusted_program_pages                                                                                                   |
| an untrusted program behind a fully allowlisted label still pages and the trusted one is suppressed | allowlist::tests::routing::c4d_allowlisted_but_untrusted_program_pages and c4a_full_allowlisted_tuple_is_suppressed                                                      |
| with nothing able to vouch, tracked edits and an unknown agent page while a neighbour stays silent  | integrity::tests::with_nothing_able_to_vouch_tracked_edits_page_while_neighbor_stays_silent and allowlist::tests::routing::with_nothing_able_to_vouch_an_own_agent_pages |

The captured C4 cases above came from `test/e2e/osquery-alerter-criteria.bats` at the recorded baseline.
PR #408 migrated them to `test/e2e/osquery-alerter-criteria.test.sh` on main
`0c925a03653e44cf89e01a8ab7e1569b3724ec80`. These current bashunit successors retain the same contracts:

| Historical case | Current bashunit function                                                                  |
| --------------- | ------------------------------------------------------------------------------------------ |
| C4a             | `test_c4a_a_persistence_agent_fully_matching_an_allowlisted_own_agent_tuple_is_suppressed` |
| C4b             | `test_c4b_the_same_allowlisted_label_with_a_different_program_pages`                       |
| C4c             | `test_c4c_an_unknown_user_launch_agent_pages_by_default_deny`                              |
| C4d             | `test_c4d_an_allowlisted_but_untrusted_program_pages_because_enrichment_beats_suppression` |

The route cases remain in `test/unit/osquery-route.test.sh`, whose original names remain mapped by the
row 2.2 documents.

## New domain leaves

| Rust leaf                                                                                                         | Contract                                      |
| ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `allowlist::tests::changed_missing_or_differently_cased_current_hash_is_reused`                                   | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::allow_removes_all_matching_objects_and_appends_fresh_identity_after_preserved_lines` | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::deny_retains_every_other_line_in_order_without_appending`                            | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::invalid_system_label_refuses_before_source_curation`                                 | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::labels_require_two_ascii_characters_and_the_complete_allowed_alphabet`               | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::malformed_or_multiple_value_line_refuses_the_whole_curation`                         | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::plist_relativization_is_leading_only_while_program_replaces_all_home_occurrences`    | Allowlist identity, trust or curation         |
| `allowlist::tests::curation::readding_first_entry_moves_it_last_but_readding_last_preserves_order`                | Allowlist identity, trust or curation         |
| `allowlist::tests::current_pin_match_skips_plist_vouch_but_requires_list_vouch`                                   | Allowlist identity, trust or curation         |
| `allowlist::tests::either_empty_identity_column_degrades_without_vouch`                                           | Allowlist identity, trust or curation         |
| `allowlist::tests::expansion_preserves_bash_replacement_of_embedded_home_tokens`                                  | Allowlist identity, trust or curation         |
| `allowlist::tests::first_matching_entry_wins_even_when_degraded_or_reused`                                        | Allowlist identity, trust or curation         |
| `allowlist::tests::full_own_agent_tuple_requires_plist_then_allowlist_vouches`                                    | Allowlist identity, trust or curation         |
| `allowlist::tests::hostile_field_bytes_do_not_shift_identity_columns`                                             | Allowlist identity, trust or curation         |
| `allowlist::tests::missing_or_unreadable_allowlist_cannot_suppress`                                               | Allowlist identity, trust or curation         |
| `allowlist::tests::path_or_program_divergence_is_reused_before_any_vouch`                                         | Allowlist identity, trust or curation         |
| `allowlist::tests::routing::c4a_full_allowlisted_tuple_is_suppressed`                                             | Allowlist identity, trust or curation         |
| `allowlist::tests::routing::c4b_reused_label_pages`                                                               | Allowlist identity, trust or curation         |
| `allowlist::tests::routing::c4c_unknown_user_agent_pages`                                                         | Allowlist identity, trust or curation         |
| `allowlist::tests::routing::c4d_allowlisted_but_untrusted_program_pages`                                          | Allowlist identity, trust or curation         |
| `allowlist::tests::routing::with_nothing_able_to_vouch_an_own_agent_pages`                                        | Allowlist identity, trust or curation         |
| `allowlist::tests::unknown_label_does_not_consume_vouch`                                                          | Allowlist identity, trust or curation         |
| `allowlist::tests::unpinned_plist_match_cannot_bypass_final_list_refusal`                                         | Allowlist identity, trust or curation         |
| `allowlist::tests::unpinned_plist_refusal_does_not_consume_final_list_vouch`                                      | Allowlist identity, trust or curation         |
| `integrity::tests::current_state_answer_controls_suppression_independently_of_the_event_digest`                   | Integrity timing and failure direction        |
| `integrity::tests::only_empty_event_hash_requests_the_rename_delay_before_current_vouch`                          | Integrity timing and failure direction        |
| `integrity::tests::shared_deployed_verdict_refuses_unreadable_symlink_and_nonregular_state`                       | Integrity timing and failure direction        |
| `integrity::tests::stale_good_event_cannot_bless_replaced_current_bytes`                                          | Integrity timing and failure direction        |
| `integrity::tests::symlink_and_nonregular_target_page_without_delay_or_vouch`                                     | Integrity timing and failure direction        |
| `integrity::tests::tracked_deletion_pages_before_any_current_state_read`                                          | Integrity timing and failure direction        |
| `integrity::tests::untracked_neighbor_is_silent_even_for_deletion_without_reading_current_state`                  | Integrity timing and failure direction        |
| `integrity::tests::with_nothing_able_to_vouch_tracked_edits_page_while_neighbor_stays_silent`                     | Integrity timing and failure direction        |
| `known_good::tests::audit_line_grammar_refuses_extra_separators_and_legacy_short_lines`                           | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::hash_case_folds_but_mode_and_uid_remain_verbatim`                                             | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::strict_grammar_bounds_hash_mode_uid_and_absolute_path_on_both_sides`                          | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::strict_tuple_grammar_preserves_spaces_and_explicit_unbuilt`                                   | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::dedicated_pipeline_and_posture_trees_are_always_tracked`                            | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::every_unusable_manifest_refuses_suppression_in_both_arms`                           | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::exactly_one_manifest_is_selected_without_cross_vouch`                               | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::healthy_bin_manifest_tracks_only_its_named_paths_even_with_nested_bins`             | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::missing_unreadable_empty_or_untrustworthy_bin_manifest_tracks_every_bin_neighbor`   | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::nonempty_manifest_with_no_parsed_path_keeps_existing_membership_miss`               | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tracking::own_plists_are_home_anchored_and_allowlist_is_one_exact_file`                       | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::trust::explicit_fixture_authority_skips_trust_attributes`                                     | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::trust::only_root_owned_manifests_without_group_or_world_write_are_trusted`                    | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::tuple_match_binds_each_column_to_the_exact_path`                                              | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::unbuilt_and_malformed_digest_never_vouch_for_forged_current_content`                          | Manifest tuple, trust or tracked-set decision |
| `known_good::tests::unreadable_or_empty_columns_never_vouch_even_when_both_sides_are_empty`                       | Manifest tuple, trust or tracked-set decision |

Bash captures before Rust covered degraded entries, pins, final-list vouch ordering, first-match
precedence, embedded home tokens, curation refusal/order, tuple fields, trust attributes, the managed-bin
failure direction, manifest selection, deletion, symlinks, non-regular files and stale event hashes. A
separate capture used the audit's exact regular expression at hash lengths 63/64/65, mode lengths 3/4/5,
owner lengths 0/1/10/11, non-octal and non-decimal characters, relative and root-only paths, extra
separators and explicit unbuilt records.

The old allowlist and manifest fixture libraries were read as behavioral evidence. Their real chezmoi
apply, privileged installation and cleanup helpers were not executed. Captures sourced only existing
policy functions and used private files or inert command answers. JSON parsing and exact writer
publication bytes remain adapter contracts; this row tests their typed policy results.

## Captured boundary examples

Captures ran `/opt/homebrew/bin/bash` with `set -euo pipefail`, private HOME, every XDG root and search
directory, Claude configuration and temporary directories initialized before fixture setup. The runtime
environment was scrubbed before assigning those roots. The allowlist and pipeline helpers were sourced
unchanged. The writer capture sourced its exact function-only prefix; it never entered its argument loop,
lock, apply, install or cleanup paths.

Representative commands were `allowlist_verdict "$label" "$path" "$program"`,
`_pipeline_manifest_has_tuple "$path" "$hash" "$mode" "$uid"`, `_pipeline_is_tracked "$path"`,
`_pipeline_manifest_for "$path"`, `pipeline_verdict "$path" "$event_hash" "$verb"`,
`_without_label "$label" "$source"`, and `is_valid_label "$label"`. The audit grammar capture evaluated
its unchanged `line_pattern` with `[[ $line =~ $line_pattern ]]`.

| Fixture                                                              | Observed result                                                 |
| -------------------------------------------------------------------- | --------------------------------------------------------------- |
| Matching unpinned own-agent tuple                                    | Verdict 0; vouch calls plist, then list.                        |
| Matching plist with refused list vouch                               | Verdict 1; calls plist, then list.                              |
| Refused unpinned plist vouch                                         | Verdict 1; only plist called.                                   |
| Matching current pin                                                 | Verdict 0; only list called.                                    |
| Uppercase copy of current lowercase pin                              | Verdict 2; no vouch called.                                     |
| First matching entry is label-only, followed by full entry           | Verdict 1.                                                      |
| First matching entry has a different program, followed by full entry | Verdict 2.                                                      |
| Two concatenated objects on one line                                 | Consumer verdict 1; writer refuses curation.                    |
| Empty stored path or program                                         | Verdict 1; no vouch called.                                     |
| Stored `prefix~/two`                                                 | Expands to `prefix$HOME/two`.                                   |
| Program `$HOME/a/$HOME/b`                                            | Relativizes to `~/a/~/b`; plist path retains the second HOME.   |
| Empty line or line starting with #                                   | Writer preserves raw content and emits a newline.               |
| Indented comment or whitespace-only line                             | Writer exits 1 with the single-JSON-tuple refusal.              |
| Object without a label                                               | Writer preserves the object.                                    |
| Both manifest and observed digest are unbuilt                        | Tuple comparison exits 1.                                       |
| Same digest in opposite case, all other columns equal                | Tuple comparison exits 0.                                       |
| Owner 0501 against observed 501, or mode 644 against 0644            | Tuple comparison exits 1.                                       |
| Double-space or tab separators                                       | Event tuple consumer accepts; strict audit grammar refuses.     |
| Missing, unreadable, empty or untrustworthy bin manifest             | Every tested bin neighbor is tracked.                           |
| Nonempty trusted manifest containing only invalid text               | Bin membership misses; dedicated pipeline paths remain tracked. |
| Matching old event digest but refused current-state vouch            | Integrity verdict 0, page.                                      |
| Different event digest but accepted current-state vouch              | Integrity verdict 1, silent.                                    |
| Empty event digest on a tracked regular file                         | Delay request 0.3, then current-state vouch.                    |
| Deleted, symlink or non-regular tracked path                         | Verdict 0 without delay or vouch.                               |

The writer's refusal preserves its exact standard-error shape:

```text
refused: the allowlist source holds a line that is not a single JSON tuple; repair <fixture-source> by hand before curating it
```

The capture reports 70 helper outcomes plus 19 audit grammar outcomes. The initial Rust test
implementation contained callable empty policy bodies: it compiled and failed 33 assertions. The
completed policies passed 105 workspace outcomes, including all 57 prior names and 48 new names. This is
package evidence; repository gates are run against the eventual integrated worktree.

| Unchanged Bash source                  | SHA-256                                                            |
| -------------------------------------- | ------------------------------------------------------------------ |
| `results-alerter/allowlist-verdict.sh` | `5642ff11426a69ce3eabc065b730e0cbe97d175b0b41b80baa7eb83dd6507f6f` |
| `results-alerter/pipeline-verdict.sh`  | `d18de8ad24a33392e31c33f51eb9e0cb375cab51b20d41f0bce90543156ee865` |
| `executable_allowlist.sh`              | `c1b3b9f09187a59086150093608071c8cf5db95307b0911d0f35ef9b54269076` |
| `executable_pipeline-audit.sh`         | `d3364ec305ea715d4131225ce53b7e7df3cbb1a13d20f07f7570b5abc654a6a0` |

## Curation cutover name map

All 212 names from the merged enrichment workspace remain. The earlier acceptance maps retain their
permanent and adapter contracts. `every_unimplemented_word_is_refused_with_usage_on_stderr_and_exit_2`
remains a migration test: its three implemented allowlist operands move to the native command tests
below. Its remaining words still refuse with usage. The seven domain curation tests are unchanged.

The retired `osquery-allowlist-lib.bash` contained fixture functions, with no runnable test leaves.
Recorded Bash outcomes cover refresh ordering, literal deny membership, raw listing, query projection and
each publisher failure direction. The new tests below exercise those contracts through typed ports or
owned native fixtures. The decimal formatter and its 55 leaves are copied from the retained private codec
proof; their original independent fault evidence is reused.

| New leaf                                                                                                               | Owning source                                                      | Classification                  |
| ---------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------- |
| `allowlist_file::tests::listing_preserves_entry_bytes_and_only_skips_empty_or_leading_comment_lines`                   | `crates/posture-adapters/src/allowlist_file/tests.rs`              | adapter contract                |
| `allowlist_file::tests::missing_and_empty_deployed_lists_both_print_nothing`                                           | `crates/posture-adapters/src/allowlist_file/tests.rs`              | adapter contract                |
| `allowlist_file::tests::deny_membership_is_a_raw_compact_substring_even_inside_a_comment`                              | `crates/posture-adapters/src/allowlist_file/tests.rs`              | adapter contract                |
| `allowlist_file::tests::listing_matches_bash_read_nul_discard_before_comment_and_blank_detection`                      | `crates/posture-adapters/src/allowlist_file/tests.rs`              | adapter contract                |
| `allowlist_file::tests::source_resolution_preserves_argument_boundaries_and_command_substitution_bytes`                | `crates/posture-adapters/src/allowlist_file/tests.rs`              | adapter contract                |
| `allowlist_file::tests::source_read_preserves_blank_lines_and_torn_final_entry_with_typed_projection`                  | `crates/posture-adapters/src/allowlist_file/tests.rs`              | adapter contract                |
| `allowlist_projection::number::tests::captured_number_0`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_1`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_2`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_3`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_4`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_5`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_6`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_7`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_8`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_9`                                                               | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_10`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_11`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_12`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_13`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_14`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_15`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_16`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_17`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_18`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_19`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_20`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_21`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_22`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_23`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_24`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_25`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_26`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_27`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_28`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_29`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_30`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_31`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_32`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_33`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_34`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_35`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_36`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_37`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_38`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_39`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_40`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_41`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_42`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_43`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_44`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_45`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_46`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_47`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_48`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_49`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_50`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_51`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_52`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_53`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::number::tests::captured_number_54`                                                              | `crates/posture-adapters/src/allowlist_projection/number/tests.rs` | copied decimal adapter contract |
| `allowlist_projection::query::tests::query_decimal`                                                                    | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_nan`                                                                        | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_infinity`                                                                   | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_false`                                                                      | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_object`                                                                     | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_array`                                                                      | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_duplicate_object_key`                                                       | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::query::tests::query_projection_uses_only_the_first_row_and_defaults_absent_fields`              | `crates/posture-adapters/src/allowlist_projection/query/tests.rs`  | adapter contract                |
| `allowlist_projection::tests::an_inert_nan_infinity_or_leading_zero_keeps_its_original_bytes`                          | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::label_numbers_keep_the_measured_decimal_spelling_and_nan_is_a_nonempty_null_label`       | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::duplicate_labels_use_the_last_value_without_rewriting_the_source_object`                 | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::false_missing_and_structured_labels_cannot_match_a_valid_writer_label`                   | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::low_surrogate_and_invalid_utf8_projection_preserve_the_original_source_bytes`            | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::unmatched_high_surrogates_are_refused_even_in_an_inert_field`                            | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::strings_that_look_like_numbers_or_escapes_do_not_become_parser_tokens`                   | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::source_grammar_requires_one_complete_object_and_preserves_only_true_comments_and_blanks` | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::labels_discard_nul_and_trailing_newlines_as_bash_command_substitution_does`              | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::deeply_nested_inert_fields_do_not_use_a_recursive_value_tree`                            | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `allowlist_projection::tests::source_projection_preserves_the_existing_container_stack_boundary`                       | `crates/posture-adapters/src/allowlist_projection/tests.rs`        | adapter contract                |
| `command::tests::lifecycle::inherited_publication_io_still_terminates_descendants_at_the_total_deadline`               | `crates/posture-adapters/src/command/tests/lifecycle.rs`           | adapter contract                |
| `command::tests::terminal::publication_modes_keep_terminal_input_and_forward_separate_errors`                          | `crates/posture-adapters/src/command/tests/terminal.rs`            | adapter contract                |
| `launchd_table::tests::capture_uses_the_launchd_query_and_hashes_the_exact_first_plist_bytes`                          | `crates/posture-adapters/src/launchd_table/tests.rs`               | adapter contract                |
| `launchd_table::tests::capture_follows_a_regular_plist_symlink_but_refuses_a_missing_or_nonregular_plist`              | `crates/posture-adapters/src/launchd_table/tests.rs`               | adapter contract                |
| `launchd_table::tests::failed_or_timed_out_queries_are_not_retried_and_cannot_supply_an_identity`                      | `crates/posture-adapters/src/launchd_table/tests.rs`               | adapter contract                |
| `launchd_table::tests::empty_or_invalid_query_fields_refuse_before_hash_capture`                                       | `crates/posture-adapters/src/launchd_table/tests.rs`               | adapter contract                |
| `locks::tests::lock_setup_creates_the_same_deployed_sibling_and_blocks_a_second_writer`                                | `crates/posture-adapters/src/locks/tests.rs`                       | adapter contract                |
| `locks::tests::lock_parent_and_lock_file_setup_errors_both_fail_closed`                                                | `crates/posture-adapters/src/locks/tests.rs`                       | adapter contract                |
| `locks::tests::an_exec_child_cannot_keep_the_write_lock_after_the_writer_releases_it`                                  | `crates/posture-adapters/src/locks/tests.rs`                       | adapter contract                |
| `publisher::staged::tests::encoding_preserves_raw_bytes_and_emits_the_fixed_four_field_tuple`                          | `crates/posture-adapters/src/publisher/staged/tests.rs`            | adapter contract                |
| `publisher::staged::tests::staged_source_has_private_permissions_and_exact_bytes_before_publication`                   | `crates/posture-adapters/src/publisher/staged/tests.rs`            | adapter contract                |
| `publisher::tests::publication_orders_source_apply_location_and_manifest_with_inherited_io`                            | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `publisher::tests::apply_failure_or_timeout_restores_source_but_does_not_claim_deployment_was_unchanged`               | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `publisher::tests::manifest_failure_or_timeout_keeps_new_source_and_deployed_bytes`                                    | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `publisher::tests::a_failed_source_directory_lookup_reports_stale_without_attempting_a_manifest`                       | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `publisher::tests::an_explicit_missing_manifest_does_not_trigger_a_source_directory_lookup`                            | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `publisher::tests::missing_original_source_rolls_back_to_an_empty_file_after_failed_apply`                             | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `publisher::tests::a_failed_source_write_never_runs_apply_or_manifest`                                                 | `crates/posture-adapters/src/publisher/tests.rs`                   | adapter contract                |
| `allowlist::tests::add_captures_before_source_and_holds_the_lock_through_publication`                                  | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::deny_publishes_only_retained_lines_without_capturing_an_agent`                                      | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::a_literal_deny_miss_skips_even_a_corrupt_source_and_publication`                                    | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::list_returns_deployed_raw_entries_without_a_lock_or_source_lookup`                                  | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::lock_setup_failure_refuses_before_validation_or_any_source_effect`                                  | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::an_invalid_label_releases_its_lock_without_capture_or_publication`                                  | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::capture_failure_never_resolves_or_writes_source`                                                    | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::source_resolution_failure_does_not_publish_or_fall_back_to_deployed_state`                          | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::an_invalid_source_line_refuses_the_entire_change`                                                   | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::source_read_failure_cannot_be_reported_as_a_successful_deny`                                        | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::publication_failure_is_preserved_and_releases_the_guard`                                            | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::tests::unrelated_entry_bytes_survive_curation_without_utf8_replacement`                                    | `crates/posture-application/src/allowlist/tests.rs`                | permanent curation behavior     |
| `allowlist::configuration::tests::explicit_configuration_keeps_unsplit_paths_and_nonempty_manifest_override`           | `crates/posture-cli/src/allowlist/configuration/tests.rs`          | adapter contract                |
| `allowlist::configuration::tests::empty_overrides_use_home_and_missing_executable_fallbacks`                           | `crates/posture-cli/src/allowlist/configuration/tests.rs`          | adapter contract                |
| `allowlist::configuration::tests::executable_discovery_skips_nonexecutable_files_and_keeps_path_order`                 | `crates/posture-cli/src/allowlist/configuration/tests.rs`          | adapter contract                |
| `allowlist::tests::add_and_deny_pass_the_first_label_to_curation_and_ignore_extra_operands`                            | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::list_forwards_raw_deployed_bytes_and_calls_the_list_use_case`                                       | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::malformed_subcommands_fail_usage_without_running_curation`                                          | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::success_messages_keep_the_absolute_program_and_absent_deny_note`                                    | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::validation_capture_and_lock_refusals_are_nonzero_without_success_output`                            | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::apply_failure_reports_rollback_and_possible_partial_deployment_honestly`                            | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::rollback_failure_is_reported_without_claiming_the_source_was_restored`                              | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::manifest_failure_keeps_the_stale_warning_and_never_reports_success`                                 | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::stdout_failure_cannot_be_reported_as_success`                                                       | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `allowlist::tests::invalid_label_diagnostics_keep_the_original_argument_bytes`                                         | `crates/posture-cli/src/allowlist/tests.rs`                        | adapter contract                |
| `native_add_publishes_the_captured_tuple_after_retained_raw_source_lines`                                              | `crates/posture-cli/tests/allowlist.rs`                            | adapter contract                |
| `native_deny_filters_source_and_list_reads_the_deployed_raw_bytes`                                                     | `crates/posture-cli/tests/allowlist.rs`                            | adapter contract                |
| `native_apply_failure_restores_source_and_reports_partial_deployment_without_manifest`                                 | `crates/posture-cli/tests/allowlist.rs`                            | adapter contract                |
| `native_manifest_failure_keeps_both_new_copies_and_reports_stale`                                                      | `crates/posture-cli/tests/allowlist.rs`                            | adapter contract                |
| `native_deny_literal_miss_skips_a_corrupt_source_and_all_publication`                                                  | `crates/posture-cli/tests/allowlist.rs`                            | adapter contract                |
| `native_lock_setup_failure_precedes_validation_and_runs_no_commands`                                                   | `crates/posture-cli/tests/allowlist.rs`                            | adapter contract                |

The heartbeat reader shares the input projection and decimal formatter. The 55 formatter cases listed
here keep their bodies and move to `legacy_json::number::tests`; the exact old-to-new name map is in
`heartbeat.md`. Their source is now `crates/posture-adapters/src/legacy_json/number/tests.rs`.
