# Controls and poller acceptance map

All 424 predecessor leaves retain their exact names and bodies. These 25 first tests cover the typed
policy in row 2.7. The retained poller fixture functions were orphan harness helpers, not runnable tests;
no predecessor assertion is replaced. The retained capture inventory names each helper and separates the
later process and publication obligations.

Each row has an unchanged-source green control and an actual failing production-source fault. The 32
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
reader and target admission, and sanitized descriptions. The Rust tests exercise typed inputs; the future
JSON decoder remains an adapter obligation.

A first classifier test accidentally used `unknown` as a no-match input for the literal needle `no`. Its
failure is retained; `unreadable` corrects the fixture while preserving substring matching.

The scope excludes process discovery/arguments/deadlines, actual baseline files and write failures, and
submission ordering against a real durable port. The proposed-state assertions cover the domain part of
those decisions; S265 still requires the planned application and adapters. The poller delivery remains
open until its consumers and operational gates are complete.
