# Poll and Funnel acceptance map

All 424 predecessor leaves retain their exact names and bodies. These 31 first tests cover the typed
policy in rows 2.7 and 2.8. The retained poller and Funnel fixture functions were orphan harness helpers,
not runnable tests; no predecessor assertion is replaced. The retained capture inventory names each
helper and separates the later process and publication obligations.

Each row has an unchanged-source green control and an actual failing production-source fault. The 39
faults use independent source copies and targets, with compiler commands, dependency files and binary
hashes in the delivery receipt. No fixture-only fault is counted.

| Full new test name                                                                                          | Source statements | Independent faults                                    |
| ----------------------------------------------------------------------------------------------------------- | ----------------- | ----------------------------------------------------- |
| `controls::tests::a_control_count_accepts_one_and_two_but_refuses_zero`                                     | S254              | `zero-controls-admitted`                              |
| `controls::tests::a_controls_document_must_be_present_and_exactly_one_array`                                | S254              | `missing-file-admitted`                               |
| `controls::tests::a_reader_expectation_cannot_cross_into_another_domain_or_an_empty_value`                  | S254              | `foreign-reader-domain-admitted`                      |
| `controls::tests::a_target_requires_the_absolute_prefix_and_refuses_only_the_captured_line_separators`      | S254              | `relative-target-admitted`, `line-separator-admitted` |
| `controls::tests::all_eight_readers_keep_their_exact_domains_and_target_requirements`                       | S254              | `eighth-reader-lost`                                  |
| `controls::tests::an_invalid_later_control_never_returns_a_partially_monitored_prefix`                      | S255              | `partial-controls-admitted`                           |
| `controls::tests::control_ids_accept_exact_ascii_ranges_and_refuse_their_neighbors`                         | S254, S256        | `id-lower-edge-refused`, `id-upper-neighbor-admitted` |
| `controls::tests::description_admission_uses_its_sanitized_value_and_caps_both_fields_at_160`               | S254, S266        | `control-cap-159`, `control-cap-161`                  |
| `controls::tests::each_reserved_or_duplicate_id_refuses_the_whole_control_set`                              | S255, S256        | `reserved-id-admitted`, `duplicate-id-admitted`       |
| `controls::tests::only_the_exact_verify_tier_is_admitted`                                                   | S254              | `verify-tier-inert`                                   |
| `controls::tests::targets_are_required_only_for_the_two_rule_readers_in_both_directions`                    | S254              | `required-target-inert`, `unexpected-target-admitted` |
| `funnel::tests::a_true_entry_at_any_projected_depth_is_active_and_invalid_values_win`                       | S268, S269        | `funnel-false-active`                                 |
| `funnel::tests::absent_null_and_false_allow_funnel_are_inactive_but_wrong_shapes_gap`                       | S269              | `wrong-funnel-shape-inactive`                         |
| `funnel::tests::baseline_trust_distinguishes_absence_corruption_and_failed_publication`                     | S272, S273, S274  | `persist-failure-baseline-trusted`                    |
| `funnel::tests::exposed_keys_are_sorted_unique_inert_spans_with_exact_200_character_edges`                  | S276              | `funnel-cap-199`, `funnel-cap-201`                    |
| `funnel::tests::failed_reads_keep_the_active_baseline_and_corrupt_idle_state_gets_one_gap`                  | S273, S275, S277  | `failed-funnel-read-advances`                         |
| `funnel::tests::funnel_pages_on_open_or_untrusted_active_but_not_steady_active_or_close`                    | S275              | `steady-funnel-pages-again`                           |
| `poll::tests::baseline::baseline_requires_exact_mode_one_object_and_every_trio_scalar_domain`               | S260              | `baseline-mode-ignored`, `trio-upper-bound-admitted`  |
| `poll::tests::baseline::control_priors_are_rearmed_independently_when_expect_target_or_domain_changes`      | S261              | `changed-target-not-rearmed`                          |
| `poll::tests::classify::all_five_filevault_forms_preserve_deferred_enablement_as_off`                       | S248              | `deferred-filevault-called-on`                        |
| `poll::tests::classify::autologin_checks_declaration_presence_and_only_the_exact_absence_diagnostic`        | S249              | `autologin-error-called-absent`                       |
| `poll::tests::classify::failed_or_conflicting_probes_never_believe_healthy_printed_text`                    | S246              | `failed-probe-believed`, `conflicting-probe-believed` |
| `poll::tests::classify::lulu_base_rules_require_a_successful_nonempty_read_with_no_profile_key`             | S251              | `profile-key-ignored`                                 |
| `poll::tests::classify::pid_status_and_output_must_agree_in_both_directions`                                | S247              | `pid-status-output-mismatch`                          |
| `poll::tests::gap::an_unreadable_trio_without_a_trusted_prior_stops_after_the_gap`                          | S246, S262        | `untrusted-trio-still-persisted`                      |
| `poll::tests::gap::each_gap_member_pages_on_arrival_and_rearms_after_its_own_recovery`                      | S257              | `new-gap-member-suppressed`                           |
| `poll::tests::gap::missing_controls_and_first_firewall_off_produce_two_independent_pages`                   | S259, S263, S267  | `gap-blinds-firewall-exposure`                        |
| `poll::tests::gap::one_gapped_control_cannot_blind_clean_members_and_only_its_prior_is_kept`                | S258, S259        | `gapped-control-prior-dropped`                        |
| `poll::tests::gap::refused_controls_preserve_prior_fields_and_profile_uncertainty_blinds_only_rule_readers` | S251, S258        | `refused-file-prior-erased`                           |
| `poll::tests::transitions::first_exposures_combine_in_order_while_healthy_seed_and_recovery_are_silent`     | S263, S267        | `first-observation-order-reversed`                    |
| `poll::tests::transitions::steady_deviation_stays_silent_but_each_transition_to_off_pages`                  | S264              | `steady-off-pages-again`                              |

Bash captures pin missing controls plus first firewall-off as two pages, both before a baseline exists.
The page text is copied into crate-owned fixtures. Twenty-four controls captures cover whole refusal,
reader and target admission, and sanitized descriptions. Funnel false/null/42/true-map captures are
inactive/inactive/gap/active; invalid beside active and multiple documents also gap. The Rust tests
exercise the typed projections of these inputs. They do not claim the future JSON decoder is tested.

A first classifier test accidentally used `unknown` as a no-match input for the literal needle `no`. Its
failure is retained; `unreadable` corrects the fixture while preserving substring matching. An initial
capture stopped before the Funnel run because its harness required `BATS_TEST_DIRNAME`; the corrected
private environment and subsequent complete capture are retained separately.

The policy packet excludes process discovery/arguments/deadlines, actual baseline files and write
failures, and submission ordering against a real durable port. The proposed-state assertions cover the
domain part of those decisions; S265, S270, S271 and S278 still require the planned application and
adapters. The poller and Funnel deliveries remain open until their consumers and operational gates are
complete.

## Controls-file reader

The reader adds four adapter leaves while preserving all 455 predecessor names and bodies, including the
31 policy rows above. Twenty-four raw Bash captures pin scalar and compound field bytes, duplicate keys,
malformed documents and later invalid rows. Six filesystem captures cover missing paths, directories,
named pipes, broken and regular symlinks, and an unreadable regular file.

| Full new test name                                                                                    | Source statements | Independent source faults        |
| ----------------------------------------------------------------------------------------------------- | ----------------- | -------------------------------- |
| `controls_file::tests::controls_files_match_bash_valid_scalar_and_compound_field_bytes`               | S254, S266        | `command-bytes`                  |
| `controls_file::tests::controls_files_refuse_every_captured_invalid_document_without_partial_records` | S254, S255        | `valid-prefix`, `first-document` |
| `controls_file::tests::controls_files_report_missing_kinds_and_read_refusal_without_blocking`         | S254              | `missing-kind`                   |
| `controls_file::tests::controls_files_follow_a_regular_symlink_without_rewriting_its_target`          | S254              | `reject-symlink`                 |

All four leaves fail their assertions against the initial reject-all reader. Each source fault then fails
its named leaf against a separately compiled source variant, with the healthy source unchanged. The
shared projection move preserves the existing field-selection and rendering logic; its 19 existing
allowlist projection leaves retain their names and bodies. The final adapter suite passes all 167 leaves.

The unreadable-file capture includes Bash's shell-redirection diagnostic. The reader returns the captured
malformed-file refusal without printing, so caller diagnostic parity remains part of the poller cutover.
This packet does not run probes, publish state or activate a caller.

## Native poller inputs

Ten new adapter leaves cover the completed-output runner, eight control readers and combined query. Ten
Bash control captures pin commands, status/output pairs, profile preflight and rule resolution. Nineteen
query captures pin first-row selection and stream/scalar bytes. Native commands are replaced only by
inert response doubles during capture; the process-owner tests use private child processes and injected
short deadlines.

| Full new test name                                                                            | Source statements                                     | Independent source faults                                                                                          |
| --------------------------------------------------------------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `command::tests::outcomes::completed_probes_retain_nonzero_status_and_ordered_output`         | S243, S246, S247                                      | `completed-exit-erased`, `signal-exit-erased`                                                                      |
| `command::tests::outcomes::a_poll_probe_gets_a_new_budget_after_the_previous_probe_times_out` | S244                                                  | `per-probe-budget-shared`                                                                                          |
| `command::tests::outcomes::a_total_inspection_budget_is_not_restarted_by_a_later_probe`       | Existing inspection/publication total-budget contract | `total-budget-reset-sequential`                                                                                    |
| `probes::tests::all_eight_control_probes_keep_captured_arguments_output_and_profile_order`    | S245, S247, S250                                      | `probe-user-scope-lost`                                                                                            |
| `probes::tests::failed_control_exits_never_believe_healthy_output_or_pid_mismatches`          | S246 to S249                                          | `probe-failed-exit-believed`                                                                                       |
| `probes::tests::lulu_reads_keep_profile_refusal_resolution_order_and_exact_archive_matches`   | S250 to S253                                          | `profile-guard-ignored`, `archive-substring-believed`                                                              |
| `probes::tests::probe_launch_and_deadline_failures_remain_indeterminate`                      | S244, S246                                            | `probe-failure-called-known`                                                                                       |
| `osqueryi::tests::the_trio_uses_one_captured_query_and_keeps_first_row_scalar_bytes`          | S241, S243                                            | `query-wrong-table`                                                                                                |
| `osqueryi::tests::query_streams_and_legacy_scalars_keep_captured_diagnostic_values`           | S241, S243, captured projection bytes                 | `query-stream-prefix-only`, `query-field-newlines-lost`, `query-command-nul-retained`, `query-scalar-stops-stream` |
| `osqueryi::tests::a_failed_query_keeps_its_status_and_discards_healthy_printed_values`        | S243, S244                                            | `query-failed-exit-believed`                                                                                       |

The initial two runner assertions and seven input assertions fail against their minimal predecessor
bodies. Further captures exposed scalar-stream continuation and command-output null-byte removal; their
actual failed assertions are retained. Sixteen independent production faults fail the named leaves or the
unchanged `command::tests::a_failed_child_does_not_supply_a_successful_reading` regression
(`success-only-exit-ignored`). Each variant is compiled from its own source directory using one released
target serially, with compiler command, dependency inputs and binary hashes recorded.

The old zero-budget leaf did not catch restarting a nonzero budget. That surviving fault is retained, not
counted as a kill. The new sequential total-budget leaf catches the same source fault after a first owned
timeout. Existing test bodies remain unchanged; four test doubles implement the completed-result method
while their original success/error scripts and assertions remain intact.

The shared projection only separates selected text from the final command-substitution cleanup, allowing
query streams to retain internal newlines. Existing controls and allowlist projection assertions still
cover their original cleanup behavior. No process probe, baseline publication or deployed caller is
activated by this packet.

## Poller state files

Eight new leaves cover baseline reads, the two gap markers and baseline publication. All 469 predecessor
names and test bodies remain. Sixteen baseline captures pin whole-object trust, scalar types, declaration
fields and the symlink's own mode. Nine marker captures distinguish literal spaces from internal tabs and
newlines. Five publication captures distinguish write, rename and chmod failures; twelve fold captures
pin current rows, retained prior fields and control declaration pairs.

| Full new test name                                                                                                 | Source statements      | Independent source faults                                                                                            |
| ------------------------------------------------------------------------------------------------------------------ | ---------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `osqueryi::tests::query_retains_the_compact_first_projection_for_baseline_publication`                             | S241, S243, S258       | `query-retained-row-erased`                                                                                          |
| `state_files::tests::baseline::a_baseline_read_preserves_captured_scalars_and_control_declaration_fields`          | S260, S261             | `saved-control-erased`                                                                                               |
| `state_files::tests::baseline::baseline_trust_refuses_wrong_modes_shapes_and_symlink_own_modes_without_blocking`   | S260                   | `baseline-mode-ignored`, `symlink-referent-mode`, `baseline-first-object-only`                                       |
| `state_files::tests::encoding::published_baselines_preserve_captured_rows_prior_fields_and_control_declarations`   | S258, S261, S262, S265 | `baseline-prior-overlay-lost`, `baseline-scalar-types-erased`, `control-target-erased`, `control-expectation-erased` |
| `state_files::tests::markers::marker_coverage_keeps_literal_spaces_and_does_not_cover_newline_or_tab_members`      | S257                   | `marker-whitespace-members`                                                                                          |
| `state_files::tests::markers::markers_refresh_members_clear_recovery_and_report_unwritable_paths_separately`       | S257, S265             | `marker-stale-members-retained`, `marker-removal-refusal-hidden`                                                     |
| `state_files::tests::publication::baseline_publication_replaces_sibling_temporary_content_and_finishes_owner_only` | S265                   | `publication-does-not-chmod`                                                                                         |
| `state_files::tests::publication::publication_failures_preserve_actual_pre_and_post_rename_file_outcomes`          | S265                   | `chmod-refusal-hidden`, `chmod-before-rename`, `temporary-exit-cleanup-lost`                                         |

The initial six-case reader/writer run failed five assertions. Its reject-all reader already passed the
refusal case; the mode, symlink and whole-document faults independently prove that case. Both
retained-row and encoded-publication assertions failed before their implementation. All eight new leaves
pass, as do all 185 adapter leaves, with a maximum measured case of 100 milliseconds. Sixteen production
faults fail their named leaves, at most 79 milliseconds. Each uses a separate source directory and an
actual compiler invocation in one serially reused target, with dependency inputs and binary hashes
retained.

The publication test performs a real refused rename in a private unwritable parent. The chmod closure
fails after a real rename, proving that the new file remains despite the returned failure. Another fault
moves chmod before rename and fails that same observation. The fixed temporary is removed when its owner
exits, matching the existing invocation trap. All named-pipe fixtures and commands are private; the query
stub prints captured rows and never inspects the machine.

The adapter retains the selected query rows inside `PostureTrio` for publication. It reuses the existing
field parser and renderer, preserves numbers versus strings and unknown fields, and leaves the public
reading and SQL unchanged. Application values contain no JSON. The ordering slice below covers gap
acceptance before marker refresh and exposure acceptance before baseline advancement. Concrete adapter
composition, caller diagnostics and deployed caller cutover remain; these packets do not complete the
poller cutover.

## Poll application ordering

Eight new application leaves preserve all 477 predecessor names. The initial no-op flow fails all eight
actual assertions. The completed flow passes all 80 application cases, including the 72 retained leaves;
no full workspace run is claimed here. Ten guarded production-source faults fail twelve named assertions.
Each arm uses the same exclusively owned source and target, records its actual compiler invocation,
dependency inputs and artifact hash, then restores exact bytes. All new healthy and failing cases
complete within one millisecond. The healthy binary is restored byte-for-byte after the faults.

| Full new test name                                                                                            | Source statements | Independent source faults                                                                     |
| ------------------------------------------------------------------------------------------------------------- | ----------------- | --------------------------------------------------------------------------------------------- |
| `poll::tests::gap::a_new_gap_is_accepted_and_marked_before_an_independent_exposure_and_baseline`              | S257, S263, S267  | `marker-before-acceptance`, `publication-before-exposure`, `security-page-called-observation` |
| `poll::tests::gap::a_refused_gap_advances_no_marker_exposure_or_baseline`                                     | S257, S267        | `marker-before-acceptance`                                                                    |
| `poll::tests::gap::an_already_covered_gap_refreshes_current_members_even_when_marker_writes_refuse`           | S257              | `covered-gap-not-refreshed`                                                                   |
| `poll::tests::gap::an_unreadable_trio_without_prior_stops_after_the_accepted_gap`                             | S262              | `early-stop-clears-persistence`                                                               |
| `poll::tests::gap::exposure_refusal_preserves_baseline_after_the_independent_gap_was_accepted`                | S267              | `publication-before-exposure`                                                                 |
| `poll::tests::publication::clean_recovery_and_successful_publication_clear_their_markers_best_effort`         | S257, S265        | `success-retains-persistence-gap`                                                             |
| `poll::tests::publication::publication_failure_keeps_the_captured_file_outcome_and_pages_its_independent_gap` | S265              | `publication-refusal-called-success`, `persistence-gap-detail-erased-corrected`               |
| `poll::tests::publication::persistence_gap_refusal_keeps_coverage_and_an_already_covered_gap_only_refreshes`  | S257, S265        | `refused-persistence-gap-marked`, `covered-persistence-pages-again`                           |

The application tests record submission, marker and publication order, all six submission-failure
classes, best-effort marker refusal and both file-publication outcomes. The publication closure models
those outcomes; the earlier state tests retain the actual write, rename, chmod and file-mode evidence. A
new inert Bash capture copies `persist_baseline` and records its exact degraded-monitor body and failure
status, which the application fixture compares byte-for-byte.

The first detail-erasure fault edit omitted a closing delimiter and failed compilation. That attempt is
retained as compile-only evidence; its corrected body-only edit fails the intended assertion. No
compile-only result is counted among the ten killed faults. The red-to-green helper changed marker
methods to shared borrows so a concrete caller can use the same file adapter in the publication closure;
all eight leaf assertion bodies remain unchanged apart from formatting.

`Alert.occurrence_id` is borrowed from the separately frozen producer packet. New poll pages supply
`None`, security classification and the supplied time, with event `gap` or `page`. This verifies the
application request, not engine routing or a deployed notification. `PollPage` keeps its captured Sosumi
field, but ordinary PNS banners currently request the fixed default sound. Exact sound-name parity
remains at caller cutover; the independent alarm retains its separate fixed Sosumi contract. Caller
stderr, concrete read order, activation and the Bash caller replacement also remain outside this slice.
