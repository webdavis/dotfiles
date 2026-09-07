# Enrichment acceptance map

This is a behavior-preserving Bash-to-Rust port with new typed boundaries and a bounded process adapter.
It combines the enricher part of former row 2.10, its necessary adapters and use case from 3.2/3.3, and
cutover 4.1. Cursor, triage and every other cutover retain their own batches.

The baseline contains 105 Rust outcomes and 34 affected shell outcomes, recorded by full name in the
private evidence. All existing domain tests remain. The foundation migration test
`every_word_is_refused_with_usage_on_stderr_and_exit_2` becomes
`every_unimplemented_word_is_refused_with_usage_on_stderr_and_exit_2`: enrich is now implemented. The
other usage tests remain, and the two new command tests exercise the actual compiled binary.

The 56 original Bash captures cover S134 through S141. Three additional captures pin null-byte
normalization and its diagnostic. Domain and application examples preserve the captured fact bytes, exit
statuses and probe order. Test fixture paths replace the private capture-directory prefix with
`/fixture`; no test reads that path on the operator's machine.

The real router tests preserve all existing names in `test/unit/osquery-route.test.sh`,
`test/e2e/osquery-alerter-criteria.test.sh`, and `test/e2e/osquery-alerter-hostile-columns.test.sh`.
Their executable doubles now require the same `enrich` plus one path argument as the installed binary.
Five new `test/unit/osquery-enricher-call.test.sh` cases pin the default path, executable override with
spaces, exit 0, exit 5 with stdout, and nonexecutable refusal. The old enricher source is removed in the
same batch as its caller change.

The former row 4.1 drill named an unsigned script for exit 10. Actual Bash returns metadata and exit 0
for a plain non-Mach-O script (S141); the untrusted-code drill uses an unsigned binary or bundle. This
corrects the example without changing classification.

Before release, the operator applies, runs `posture enrich` against the intended signed application and
an unsigned Mach-O binary or bundle, checks fact output and exit statuses 0/10, then retires the old
deployed `enrich-finding.sh`. The source package and its tests never apply or inspect live destinations.

## New and revised Rust leaf names

| Source inside the package                                     | Leaf name                                                                   |
| ------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `crates/posture-adapters/src/codesign/tests.rs`               | `codesign_receives_the_unsplit_path_and_merged_output`                      |
| `crates/posture-adapters/src/codesign/tests.rs`               | `plist_extraction_preserves_empty_values_and_exact_key_arguments`           |
| `crates/posture-adapters/src/codesign/tests.rs`               | `quarantine_uses_the_selected_path_and_requires_nonempty_output`            |
| `crates/posture-adapters/src/codesign/tests.rs`               | `only_regular_files_reach_file_and_its_mach_o_reading_selects_code`         |
| `crates/posture-adapters/src/codesign/tests.rs`               | `command_substitution_removes_nul_before_signing_classification`            |
| `crates/posture-adapters/src/codesign/tests.rs`               | `a_quarantine_attribute_containing_only_nul_is_empty`                       |
| `crates/posture-adapters/src/command/tests/lifecycle.rs`      | `a_closed_output_pipe_does_not_remove_the_child_deadline`                   |
| `crates/posture-adapters/src/command/tests/lifecycle.rs`      | `timeout_terminates_the_owned_process_group`                                |
| `crates/posture-adapters/src/command/tests/lifecycle.rs`      | `continuous_output_cannot_extend_the_absolute_deadline`                     |
| `crates/posture-adapters/src/command/tests.rs`                | `stdout_and_stderr_share_the_original_write_order`                          |
| `crates/posture-adapters/src/command/tests.rs`                | `separate_stderr_is_discarded_and_trailing_newlines_are_retained_by_runner` |
| `crates/posture-adapters/src/command/tests.rs`                | `a_failed_child_does_not_supply_a_successful_reading`                       |
| `crates/posture-adapters/src/command/tests.rs`                | `a_missing_executable_is_unavailable`                                       |
| `crates/posture-adapters/src/command/tests.rs`                | `an_exhausted_budget_never_starts_another_probe`                            |
| `crates/posture-adapters/src/metadata/tests.rs`               | `missing_metadata_remains_not_applicable`                                   |
| `crates/posture-adapters/src/metadata/tests.rs`               | `symbolic_modes_keep_file_kind_special_bits_and_their_execute_partner`      |
| `crates/posture-adapters/src/metadata/tests.rs`               | `the_named_root_owner_is_not_replaced_by_a_numeric_identifier`              |
| `crates/posture-adapters/src/metadata/tests.rs`               | `metadata_time_keeps_the_legacy_format`                                     |
| `crates/posture-adapters/src/metadata/tests.rs`               | `metadata_describes_a_live_symlink_but_not_a_broken_one`                    |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_sh`                                                       |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_bash`                                                     |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_zsh`                                                      |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_dash`                                                     |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_ksh`                                                      |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_python`                                                   |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_python2`                                                  |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_python3`                                                  |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_perl`                                                     |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_ruby`                                                     |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_node`                                                     |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_osascript`                                                |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_php`                                                      |
| `crates/posture-application/src/enrich/tests/interpreters.rs` | `bash_interpreter_env`                                                      |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_no_path`                                                              |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_empty_path`                                                           |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_missing_file`                                                         |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_extra_operand`                                                        |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_regular_mach_o`                                                       |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_regular_file_failed`                                                  |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_ordinary_metadata`                                                    |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_metadata_failure`                                                     |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_directory_metadata`                                                   |
| `crates/posture-application/src/enrich/tests/ordinary.rs`     | `bash_hostile_path`                                                         |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_plist_unresolved`                                                     |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_plist_program_first`                                                  |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_plist_fallback`                                                       |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_position_1`                                                    |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_position_2`                                                    |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_position_3`                                                    |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_position_4`                                                    |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_position_5`                                                    |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_position_6`                                                    |
| `crates/posture-application/src/enrich/tests/resolution.rs`   | `bash_script_first_file`                                                    |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_bundle_app`                                                           |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_bundle_kext`                                                          |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_bundle_systemextension`                                               |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_bundle_dext`                                                          |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_bundle_appex`                                                         |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_failure`                                                      |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_not_signed`                                                   |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_adhoc`                                                        |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_not_signed_precedes_adhoc`                                    |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_no_authority`                                                 |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_empty_first_authority`                                        |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_software_signing`                                             |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_apple_prefix`                                                 |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_developer_id`                                                 |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_other_authority`                                              |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_authority_equals`                                             |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_authority_leading_space`                                      |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_authority_case`                                               |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_signing_stdout_authority`                                             |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_downloaded`                                                           |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_quarantine_failure`                                                   |
| `crates/posture-application/src/enrich/tests/signing.rs`      | `bash_quarantine_empty`                                                     |
| `crates/posture-cli/src/tests.rs`                             | `the_cli_composes_enrichment_and_preserves_its_fact_and_exit_status`        |
| `crates/posture-cli/tests/usage.rs`                           | `every_unimplemented_word_is_refused_with_usage_on_stderr_and_exit_2`       |
| `crates/posture-cli/tests/usage.rs`                           | `enrich_with_an_absent_or_empty_path_is_successful_and_silent`              |
| `crates/posture-cli/tests/usage.rs`                           | `enrich_inspects_a_private_non_code_file_and_ignores_trailing_operands`     |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_failure`                                                           |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_not_signed`                                                        |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_adhoc`                                                             |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_not_signed_precedes_adhoc`                                         |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_no_authority`                                                      |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_empty_first_authority`                                             |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_software_signing`                                                  |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_apple_prefix`                                                      |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_developer_id`                                                      |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_other_authority`                                                   |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_authority_equals`                                                  |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_authority_leading_space`                                           |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_authority_case`                                                    |
| `crates/posture-domain/src/enrich/tests.rs`                   | `signing_stdout_authority`                                                  |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_sh`                                                            |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_bash`                                                          |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_zsh`                                                           |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_dash`                                                          |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_ksh`                                                           |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_python`                                                        |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_python2`                                                       |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_python3`                                                       |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_perl`                                                          |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_ruby`                                                          |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_node`                                                          |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_osascript`                                                     |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_php`                                                           |
| `crates/posture-domain/src/enrich/tests.rs`                   | `interpreter_env`                                                           |
| `crates/posture-domain/src/enrich/tests.rs`                   | `similar_names_are_not_interpreters`                                        |
