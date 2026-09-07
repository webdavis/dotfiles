# Severity and gate boundary

Row 2.2 adds pure typed policy to the domain. The existing detector and enrichment-path policy remains
unchanged. The allowlist operation is one injected predicate over `LaunchdIdentity`; its future
implementation is the pure tuple policy in row 2.3. Integrity arrives as an explicit page/log-only
verdict. These arguments keep files, trust checks, parsing and processes outside this row.

`Signing` separates the untrusted verdict from its text. The Bash comment says an erroring enricher
leaves no Signing field, but a captured exit-5 process with nonempty standard output attaches that text
without promotion. The port preserves the executable behavior. `None` means no signing result; it does
not turn an unavailable enrichment source into a reason to drop the finding.

Missing severity resolves to Critical before the detector gate. The Bash diagnostic says it pages, but
the explicit System Integrity Protection (SIP) and authentication-configuration overrides still produce
log-only and digest outcomes. The alert use case must retain the shortfall diagnostic and continue its
batch. The domain does not fabricate batch context or perform output writes.

Adapters map the exact string `added` to `Action::Added`, `removed` to `Action::Removed`, and other
values to `Action::Other`. They map only the detector's relevant string state `0` to
`ProtectionState::Off`; numeric zero, absent and other values are not that string. FileVault-off uses its
added-row presence instead. Raw-field defaults and parsing remain with the results-log adapter.

Page serialization must emit `sev: CRIT`. Signing and triage strings remain unsanitized facts here; the
page and digest renderers retain their field-specific sanitation and caps. Triage parsing must still
reject missing or non-string members before producing `Some(Triage)`. No protocol, dependency,
persistence, command-line, builder or live-call-site change is included.
