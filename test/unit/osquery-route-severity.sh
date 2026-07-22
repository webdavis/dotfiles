#!/usr/bin/env bash
#
# route_severity (results-alerter/route.sh) is the pure base-severity classifier
# of the routing stage: given one normalized finding ({q, act, cols, ep}) it
# prints CRIT, NOTICE, or INFO. It is the severity MATRIX only; the three-outcome
# page/digest/log-only gate that can override this base tier lands in a later
# behavior. Adapted from c69baab's sev/protection_off, rebased to test the
# prefix-stripped q rather than the full pack-qualified .name.
#
#   CRIT   - a protection turned OFF (protection_off), a new admin account, or a
#            new unexpected setuid-root binary.
#   NOTICE - any other security-policy-regression query, a persistence_* query,
#            a new kernel/system extension, a watched file event, or an
#            Endpoint-Security launchd write.
#   INFO   - everything else (installed-software drift, listening ports, logins,
#            and the agent_* queries whose real tier the gate assigns later).
#
# protection_off is the criterion-2 fix: filevault_off now arrives as a
# DIFFERENTIAL action:added row (not the old snapshot "currently-off" form), so a
# filevault_off finding is CRIT purely on q=="filevault_off" and act=="added";
# firewall/gatekeeper/sip page only when their state column holds the unsafe "0".
#
# Unit test: fixture-driven and streamed. One route_severity pass over an ordered
# batch of findings, then assert each finding's sev against its expected tier.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"

fail() {
  printf 'osquery-route-severity: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $ROUTE ]] || fail "missing helper: $ROUTE"

# route_severity reads normalized-finding NDJSON on stdin and prints one severity
# per finding, in order. A fresh subshell keeps the sourcing side-effect-free.
route_severity_stream() {
  bash -c "source '$ROUTE'; route_severity"
}

# Each row: <expected-sev> <TAB> <finding-json> <TAB> <behavior label>. The
# findings run through ONE route_severity pass; outputs are matched positionally.
cases=(
  # -- CRIT: protection_off (differential added) --
  $'CRIT\t{"q":"filevault_off","act":"added","cols":{},"ep":""}\tfilevault_off differential added -> CRIT (criterion 2, no snapshot form)'
  $'CRIT\t{"q":"firewall_state","act":"added","cols":{"global_state":"0"},"ep":""}\tfirewall_state added with global_state 0 (firewall OFF) -> CRIT'
  $'CRIT\t{"q":"gatekeeper_state","act":"added","cols":{"assessments_enabled":"0"},"ep":""}\tgatekeeper_state added with assessments_enabled 0 (Gatekeeper OFF) -> CRIT'
  $'CRIT\t{"q":"sip_state","act":"added","cols":{"enabled":"0"},"ep":""}\tsip_state added with enabled 0 (SIP OFF) -> CRIT'
  # -- CRIT: new admin + new setuid binary --
  $'CRIT\t{"q":"new_admin_user","act":"added","cols":{"username":"eve"},"ep":""}\tnew_admin_user -> CRIT (criterion 1, pages)'
  $'CRIT\t{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/x"},"ep":"/tmp/x"}\tsuid_bin_unexpected -> CRIT'
  # -- NOT protection_off: safe/other-direction security-policy rows fall to NOTICE --
  $'NOTICE\t{"q":"firewall_state","act":"added","cols":{"global_state":"1"},"ep":""}\tfirewall_state added with global_state 1 (re-enabled) -> NOTICE, not CRIT'
  $'NOTICE\t{"q":"filevault_off","act":"removed","cols":{},"ep":""}\tfilevault_off removed (encryption restored) -> NOTICE, not CRIT'
  $'NOTICE\t{"q":"filevault_state","act":"removed","cols":{},"ep":""}\tfilevault_state removed (APFS churn, issue #18) -> NOTICE, never CRIT'
  $'NOTICE\t{"q":"remote_access_sharing_state","act":"added","cols":{},"ep":""}\tremote_access_sharing_state -> NOTICE at the matrix (gate decides page later)'
  # -- NOTICE tier --
  $'NOTICE\t{"q":"persistence_launchd","act":"added","cols":{},"ep":""}\tpersistence_launchd -> NOTICE'
  $'NOTICE\t{"q":"persistence_startup_items_crontab","act":"added","cols":{},"ep":""}\tpersistence_startup_items_crontab -> NOTICE'
  $'NOTICE\t{"q":"kernel_extensions_new","act":"added","cols":{},"ep":""}\tkernel_extensions_new -> NOTICE'
  $'NOTICE\t{"q":"system_extensions_new","act":"added","cols":{},"ep":""}\tsystem_extensions_new -> NOTICE'
  $'NOTICE\t{"q":"file_events_recent","act":"added","cols":{},"ep":""}\tfile_events_recent -> NOTICE'
  $'NOTICE\t{"q":"es_launchd_writes","act":"added","cols":{},"ep":""}\tes_launchd_writes -> NOTICE'
  # -- INFO default: installed-software drift, ports, logins, and the agent_* queries --
  $'INFO\t{"q":"homebrew_packages","act":"added","cols":{},"ep":""}\thomebrew_packages drift -> INFO'
  $'INFO\t{"q":"installed_apps","act":"added","cols":{},"ep":""}\tinstalled_apps drift -> INFO'
  $'INFO\t{"q":"listening_ports_non_loopback","act":"added","cols":{},"ep":""}\tlistening_ports_non_loopback -> INFO'
  $'INFO\t{"q":"recent_logins","act":"added","cols":{},"ep":""}\trecent_logins -> INFO'
  $'INFO\t{"q":"agent_exposure_changed","act":"added","cols":{},"ep":""}\tagent_exposure_changed -> INFO at the matrix (gate promotes to page later)'
)

# Split the cases into parallel arrays, run one route_severity pass, compare.
expected=()
findings=()
labels=()
for row in "${cases[@]}"; do
  IFS=$'\t' read -r sev finding label <<<"$row"
  expected+=("$sev")
  findings+=("$finding")
  labels+=("$label")
done

got=()
mapfile -t got < <(printf '%s\n' "${findings[@]}" | route_severity_stream)

[[ ${#got[@]} -eq ${#expected[@]} ]] ||
  fail "route_severity emitted ${#got[@]} severities for ${#expected[@]} findings (one per finding expected)"

for i in "${!expected[@]}"; do
  [[ ${got[i]} == "${expected[i]}" ]] ||
    fail "${labels[i]}: expected ${expected[i]}, got ${got[i]}"
done

printf 'osquery-route-severity: OK (protection_off CRIT on differential added; new_admin_user + suid CRIT; NOTICE tier; INFO default; safe/other-direction rows are not CRIT)\n'
