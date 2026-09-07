# Severity and routing policy

Given one admitted detector, action and protection fact, `severity` returns Critical, Notice or Info. An
added disabled firewall, Gatekeeper or System Integrity Protection (SIP) state is Critical. An added
FileVault-off row needs no column check. New administrators and unexpected setuid binaries are Critical
in every direction. Other security-policy rows, persistence, extensions, watched files and endpoint
writes are Notice. The remaining admitted detectors are Info (S035 to S040).

Given those facts and optional enrichment, `gate` returns exactly one `Page`, `Digest` or `LogOnly`
outcome. A page is inherently Critical; its serializer must stamp `CRIT`. Missing severity resolves to
Critical before detector overrides. Explicit log-only and digest rules still apply (S042, S067).

Given an inspectable enrichment path and a Critical or Notice tier, an untrusted signing result promotes
Notice to Critical. Nonempty text stays attached even when it came from a failed enricher. Empty text
adds no field. Info findings and empty enrichment paths do not consume signing input. Enrichment never
reduces severity (S044, S045).

Given an added launchd finding, `/System/Library/` items stay log-only, including untrusted ones. A
`/LaunchDaemons/` path component pages. Other items consult the supplied pure allowlist predicate using
three separate borrowed fields. A match suppresses only a noncritical finding. Unknown or reused tuples
page. Removed items stay log-only. Prefix and component neighbours follow the ordinary allowlist rule.

Given a file event, the two authorized-key basenames and `sshd_config` page. Other SSH (Secure Shell)
files and sudoers digest. The five integrity categories use the supplied page/log-only verdict. Optional
triage facts attach only after an integrity page decision, and cannot silence it. Other categories stay
log-only (S056 to S063). The category decision uses whole values, including empty and hostile strings.

No domain operation reads state, starts a process, emits bytes, advances a cursor or contacts a
destination. A repeated call with the same arguments has the same outcome. Timeout, cancellation, writer
failures, batch diagnostics and notify-before-state ordering remain with their adapters and application
use cases.

## Existing test names

All names below passed against copied Bash before this port. The plan says seven criteria cases, while C1
through C4d contains nine names; all nine are retained. Each successor is a Rust leaf name. Existing Bash
tests remain active through cutover. Allowlist and integrity facts are supplied here; row 2.3 owns how
those facts are decided. Page/digest byte encoding and process failures remain with their later owners,
rather than being simulated inside the domain.

### Route

| Existing Bash leaf name                                                                              | Successor or retained owner                                                                                                                                                                                                                 |
| ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a protection in its unsafe state, a new admin account and a new setuid-root binary are CRIT          | severity::tests::unsafe_protections_new_admins_and_setuid_binaries_are_critical                                                                                                                                                             |
| a security-policy row that is not the unsafe state falls to NOTICE, never to INFO                    | severity::tests::other_security_policy_rows_are_notice_never_info                                                                                                                                                                           |
| persistence, extensions, watched files and endpoint-security writes are NOTICE                       | severity::tests::persistence_extensions_watched_files_and_endpoint_writes_are_notice                                                                                                                                                        |
| software drift, listeners, logins and the agent queries are INFO                                     | severity::tests::software_listeners_logins_and_agent_queries_are_info                                                                                                                                                                       |
| the page tier reaches stdout, including the two remote-auth file events                              | gate::tests::page::remote_authentication_file_events_page; gate::tests::page::added_remote_access_and_setuid_binaries_page                                                                                                                  |
| the safe-direction rows and the poller-owned protection reach neither channel                        | gate::tests::log_only::safe_directions_remain_log_only; gate::tests::log_only::explicit_log_only_arms_ignore_untrusted_promotion                                                                                                            |
| the ambiguous tier digests: a credential file, a new listener, a private key, sudoers                | gate::tests::digest::private_keys_sudoers_and_other_ssh_files_digest; gate::tests::digest::added_listeners_digest; gate::tests::digest::c3c_authentication_configuration_changes_only_digest                                                |
| the extension arms honor the untrusted-signing promotion and the log-only arms ignore it             | gate::tests::page::extension_arms_honor_untrusted_promotion; gate::tests::log_only::explicit_log_only_arms_ignore_untrusted_promotion                                                                                                       |
| an untrusted program behind a fully allowlisted label still pages, and the trusted one is suppressed | gate::tests::page::c4d_allowlisted_but_untrusted_program_pages; gate::tests::log_only::c4a_full_allowlisted_tuple_is_suppressed                                                                                                             |
| default-deny: a reused label, an unknown agent and a LaunchDaemon page, an Apple item is skipped     | gate::tests::page::c4b_reused_label_pages; gate::tests::page::c4c_unknown_user_agent_pages; gate::tests::page::launch_daemon_component_pages_before_allowlist; gate::tests::log_only::apple_prefix_is_log_only_before_allowlist_and_signing |
| the signing verdict is attached to a paged finding, trusted or not                                   | gate::tests::page::signing_text_is_attached_to_pages_trusted_or_untrusted                                                                                                                                                                   |
| every finding reaches exactly one channel and every page is stamped CRIT                             | gate::tests::outcomes::every_detector_has_one_page_digest_or_log_only_outcome                                                                                                                                                               |
| with nothing able to vouch, tracked edits and an unknown agent page while a neighbour stays silent   | gate::tests::page::integrity_page_verdict_survives_absent_or_present_display_facts; gate::tests::log_only::integrity_silent_verdict_and_untracked_neighbours_stay_silent; gate::tests::page::c4c_unknown_user_agent_pages                   |
| a finding whose severity never arrived pages, the batch completes, and the shortfall is announced    | gate::tests::page::unavailable_severity_pages_the_fallback_detector; batch completion and diagnostics remain with the alert use case                                                                                                        |
| a healthy severity batch routes normally and warns about nothing                                     | gate::tests::log_only::healthy_fallback_detectors_stay_log_only; diagnostic silence remains with the alert use case                                                                                                                         |
| a failed page emit reports nonzero instead of a clean nothing-to-page                                | Retained Bash adapter contract: output-write failure and status propagation belong to the alert use case and command-line output adapter                                                                                                    |
| a healthy emit writes the candidate and a batch with nothing to page emits nothing, both exiting 0   | Retained Bash adapter contract: output-write success, empty batches and status propagation belong to the alert use case and command-line output adapter                                                                                     |

### Criteria

| Existing Bash leaf name                                                              | Successor or retained owner                                               |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------- |
| C1: new_admin_user added fires a CRIT page                                           | gate::tests::page::c1_new_admin_user_pages                                |
| C2: differential filevault_off added (not snapshot) fires a CRIT page                | gate::tests::page::c2_added_filevault_off_pages                           |
| C3a: agent_exposure_changed added pages                                              | gate::tests::page::c3a_added_agent_exposure_pages                         |
| C3b: agent_secretfile_changed pages                                                  | gate::tests::page::c3b_secret_file_changes_page_in_every_direction        |
| C3c: agent_authfile_changed (config.toml) does NOT page, lands in the digest spool   | gate::tests::digest::c3c_authentication_configuration_changes_only_digest |
| C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed | gate::tests::log_only::c4a_full_allowlisted_tuple_is_suppressed           |
| C4b: the same allowlisted label with a different program pages (reused label)        | gate::tests::page::c4b_reused_label_pages                                 |
| C4c: an unknown user LaunchAgent pages (default-deny, operator ruling)               | gate::tests::page::c4c_unknown_user_agent_pages                           |
| C4d: an allowlisted-but-untrusted program pages (enrichment beats suppression)       | gate::tests::page::c4d_allowlisted_but_untrusted_program_pages            |

### Hostile columns

| Existing Bash leaf name                                                                                 | Successor or retained owner                                                                  |
| ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| HOSTILE-0x1F: a 0x1F-injected path cannot impersonate an allowlisted tuple; the finding pages           | gate::tests::fields::hostile_unit_separator_in_path_cannot_impersonate_the_allowlisted_tuple |
| HOSTILE-newline: a newline in a column does not split the record; the finding pages                     | gate::tests::fields::hostile_newline_in_label_stays_one_field                                |
| HOSTILE-control: the genuine allowlisted own-agent is suppressed, so the injection pins are not vacuous | gate::tests::fields::hostile_control_genuine_tuple_is_suppressed                             |
| HOSTILE-tab: a tab in a column stays opaque; the finding pages                                          | gate::tests::fields::hostile_tab_in_program_stays_one_field                                  |
