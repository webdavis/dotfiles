# Bash severity and gate acceptance examples

Source: `results-alerter/route.sh` from `50763deea9c48396d1d4356a5fee1cbf05992bd5`, SHA-256
`a6b53037daa7fcdac2881bea3bfb0d7932542e08c2794095121c3a913bc8b7dd`. The first table was captured before
the new Rust tests. Each run sourced that exact helper under private HOME, XDG and temporary directories.
A signing executable printed the supplied text and exited with the supplied status; verdict helpers were
inert fixtures. Every listed run exited 0. Empty page and digest output means log-only.

| Capture                                            | Base tier | Page output                                                                                                                            | Digest output                                                                                                    |
| -------------------------------------------------- | --------- | -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| sip_off_log_only                                   | CRIT      | `(empty)`                                                                                                                              | `(empty)`                                                                                                        |
| sip_removed_log_only                               | NOTICE    | `(empty)`                                                                                                                              | `(empty)`                                                                                                        |
| removed_setuid_log_only                            | CRIT      | `(empty)`                                                                                                                              | `(empty)`                                                                                                        |
| added_setuid_pages                                 | CRIT      | `{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/fixture/setuid"},"ep":"/fixture/setuid","signing":"UNSIGNED","sev":"CRIT"}` | `(empty)`                                                                                                        |
| failed_enrichment_stdout_digests_without_promotion | NOTICE    | `(empty)`                                                                                                                              | `{"q":"system_extensions_new","act":"added","cols":{},"ep":"/fixture/program","signing":"partial signing fact"}` |
| failed_enrichment_kernel_stays_log_only            | NOTICE    | `(empty)`                                                                                                                              | `(empty)`                                                                                                        |
| other_file_category_log_only                       | NOTICE    | `(empty)`                                                                                                                              | `(empty)`                                                                                                        |
| missing_severity_pages_fallback                    | INFO      | `{"q":"homebrew_packages","act":"added","cols":{},"ep":"","sev":"CRIT"}`                                                               | `(empty)`                                                                                                        |
| missing_severity_sip_stays_log_only                | NOTICE    | `(empty)`                                                                                                                              | `(empty)`                                                                                                        |
| missing_severity_authfile_stays_digest             | INFO      | `(empty)`                                                                                                                              | `{"q":"agent_authfile_changed","act":"added","cols":{},"ep":""}`                                                 |

The System Integrity Protection (SIP) off input was
`{"q":"sip_state","act":"added","cols":{"enabled":"0"},"ep":""}`. The removed-setuid input carried
`act: removed` and `/fixture/setuid` as its enrichment path; the signing fixture printed `UNSIGNED` and
exited 10. A failed system-extension enricher printed `partial signing fact` and exited 5; the text
remained attached to its digest without promotion. A failed kernel-extension enrichment stayed log-only.

A follow-up capture strengthened the page fixture to an unexpected setuid binary, whose path the
normalizer selects for enrichment. This capture preceded that fixture change. Both cases used
`{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/fixture/program"},"ep":"/fixture/program"}`
and enricher exit 5. Nonempty output produced this page; empty output produced the same page without
`signing`:

```json
{
  "q": "suid_bin_unexpected",
  "act": "added",
  "cols": {
    "path": "/fixture/program"
  },
  "ep": "/fixture/program",
  "signing": "partial signing fact",
  "sev": "CRIT"
}
```

Missing-severity captures used a classifier that consumed the input and exited 5 without output. All
three emitted the same standard-error diagnostic below, including the SIP and configuration rows whose
explicit gate override stayed quiet or digested:

```text
route.sh: no severity for finding 1 of 1 (route_severity exit 5, 0 severities returned); treating it as CRITICAL so it pages
```

## Detector outcomes

A later whole-detector capture used `act: added`, empty columns and enrichment path, and an allowlist
verdict that could not vouch. This table records each observed detector outcome, not only its count.

| Detector                          | Base tier | Outcome |
| --------------------------------- | --------- | ------- |
| new_admin_user                    | CRIT      | Page    |
| file_events_recent                | NOTICE    | LogOnly |
| es_launchd_writes                 | NOTICE    | LogOnly |
| agent_authfile_changed            | INFO      | Digest  |
| agent_binary_changed              | INFO      | LogOnly |
| agent_exposure_changed            | INFO      | Page    |
| agent_secretfile_changed          | INFO      | Page    |
| chrome_extensions                 | INFO      | LogOnly |
| firefox_addons                    | INFO      | LogOnly |
| homebrew_packages                 | INFO      | LogOnly |
| installed_apps                    | INFO      | LogOnly |
| safari_extensions                 | INFO      | LogOnly |
| kernel_extensions_new             | NOTICE    | LogOnly |
| listening_ports_non_loopback      | INFO      | Digest  |
| persistence_launchd               | NOTICE    | Page    |
| persistence_launchd_overrides     | NOTICE    | LogOnly |
| persistence_startup_items_crontab | NOTICE    | LogOnly |
| recent_logins                     | INFO      | LogOnly |
| suid_bin_unexpected               | CRIT      | Page    |
| system_extensions_new             | NOTICE    | Digest  |
| filevault_off                     | CRIT      | Page    |
| filevault_state                   | NOTICE    | LogOnly |
| firewall_state                    | NOTICE    | LogOnly |
| gatekeeper_state                  | NOTICE    | LogOnly |
| remote_access_sharing_state       | NOTICE    | Page    |
| sip_state                         | NOTICE    | LogOnly |

Path captures retained the trailing-slash distinctions: `/System/Library/` is log-only even when
untrusted, while `/System/Library` and `/System/Libraryevil/x` page when untrusted. With a matching
allowlist, `/x/LaunchDaemons/x` pages, while `/x/LaunchDaemons` and `/x/LaunchDaemonsevil/x` stay
log-only. `authorized_keys2` and the bare `authorized_keys` basename page; an `.old` suffix and a
trailing slash digest. An untrusted kernel extension with empty signing output still pages without a
Signing field. These supplementary captures confirm source-derived boundaries; the three required
unpinned cases in the first table were captured before any row 2.2 test or implementation.
