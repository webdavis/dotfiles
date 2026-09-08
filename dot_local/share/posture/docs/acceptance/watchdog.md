# Watchdog and audit acceptance map

All 407 heartbeat-predecessor outcomes remain required under their existing names. This row adds the 17
permanent behavior contracts below. There were no existing runnable watchdog or manifest-audit leaves to
rename; the retained Bash fixture libraries supplied cases. No predecessor test or assertion is removed.
The domain has no process or filesystem access.

Each new leaf has an actual failing source fault and an unchanged-source green control. Source copies,
compiler commands, dependency files and binaries are distinct for every fault. The private delivery
receipt carries their exact identities and timings.

| Full new test name                                                                                         | Source statements                                     | Independent faults                                                                     |
| ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `audit::tests::an_audit_keeps_earlier_findings_when_either_manifest_refuses`                               | S227, S230                                            | `mixed-findings-lost`                                                                  |
| `audit::tests::an_unbuilt_regular_artifact_is_content_drift_before_attribute_reads`                        | S227, S234 and the existing unbuilt manifest contract | `unbuilt-read-as-clean`                                                                |
| `audit::tests::audit_bounds_refuse_at_entry_and_shared_deadline_edges`                                     | S236, S237                                            | `entry-ceiling-bypassed`, `deadline-edge-admitted`, `configured-budget-inert`          |
| `audit::tests::audit_limits_accept_both_edges_and_refuse_noncanonical_or_outside_values`                   | S237                                                  | `leading-zero-admitted`, `lower-bound-lost`, `upper-edge-refused`                      |
| `audit::tests::each_drift_column_is_reported_in_manifest_order`                                            | S228, S229, S230, S239                                | `owner-drift-lost`                                                                     |
| `audit::tests::file_kind_and_read_failures_keep_distinct_audit_findings`                                   | S228, S236, S238, S239                                | `symlink-misclassified`, `size-edge-refused`                                           |
| `audit::tests::missing_untrusted_and_malformed_manifests_never_become_an_all_clear`                        | S231 to S235                                          | `unavailable-is-clean`                                                                 |
| `watchdog::agents::tests::crash_streaks_require_a_new_failure_but_frozen_failures_keep_their_alarm`        | S213                                                  | `crash-threshold-delayed`, `frozen-exit-accumulates`                                   |
| `watchdog::agents::tests::exit_classification_preserves_numeric_prefixes_and_refuses_unanchored_sentinels` | S214                                                  | `unanchored-sentinel-admitted`                                                         |
| `watchdog::agents::tests::the_six_watched_agents_keep_their_labels_and_unloaded_state_is_not_retained`     | S212                                                  | `wrong-agent-label`                                                                    |
| `watchdog::audit::tests::audit_confirmation_pages_once_restarts_on_change_and_forgets_after_clean`         | S221                                                  | `audit-confirmation-delayed`, `paged-marker-ignored`                                   |
| `watchdog::audit::tests::audit_pages_render_only_fixed_kind_labels_and_count_columns_in_report_order`      | S219, S220, S221                                      | `audit-kind-label-lost`, `report-order-affects-identity`, `divergence-count-unbounded` |
| `watchdog::audit::tests::invalid_fingerprints_page_every_tick_and_streaks_clamp_on_read_and_write`         | S222, S223                                            | `streak-write-unclamped`, `uppercase-fingerprint-admitted`                             |
| `watchdog::audit::tests::mixed_refusals_and_hostile_tokens_use_the_fixed_unknown_problem`                  | S218, S230                                            | `mixed-refusal-leaks-path`                                                             |
| `watchdog::tests::a_watchdog_requires_clock_process_and_both_sides_of_canary_freshness`                    | S208, S211                                            | `clock-refusal-misdiagnosed`                                                           |
| `watchdog::tests::only_exact_healthy_route_statuses_suppress_the_watchdog_problem`                         | S215                                                  | `missing-route-admitted`                                                               |
| `watchdog::tests::state_refusal_is_a_problem_and_one_page_keeps_problem_order_and_sound`                   | S210, S225                                            | `page-sound-lost`                                                                      |

The mixed same-manifest and second-manifest captures retain exact stdout and exit status. Audit-state
captures cover first, second, already-paged, changed, clean, unhashable and clamped states, every fixed
refusal, and hostile paths. Agent captures cover first and second failures, frozen and decreased runs,
unreadable runs, recovery, sentinels, absent fields and numeric-prefix privacy.

S209, S224 and S226 require the later state adapter and use case: this row does not claim whole-file JSON
(JavaScript Object Notation) validation, publication or notify-before-persist execution. S216 and the pns
queue/health probes remain in the planned watchdog cutover. Physical process ownership and bounded reads
must be tested at their actual adapter seams before that cutover.
