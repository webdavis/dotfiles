#!/usr/bin/env bash
#
# route_findings (results-alerter/route.sh) is the three-outcome gate: it consumes
# normalized findings ({q, act, cols, ep}), computes each one's base severity via
# route_severity, and routes it to EXACTLY ONE outcome:
#   PAGE      -> the finding is emitted (with .sev="CRIT") as NDJSON on stdout, to
#                be rendered by render_page.
#   DIGEST    -> the finding is recorded via digest_append (a spool side effect);
#                nothing is emitted.
#   LOG-ONLY  -> nothing is emitted and nothing is spooled.
#
# The tier per detector mirrors c69baab's gate case. This behavior (B10) wires the
# CORE routing only: the allowlist verdict (a user LaunchAgent), the pipeline
# verdict (a pipeline file event), and the signing enrichment are NOT wired yet -
# a launchd add and a pipeline file event page unconditionally for now (TODO
# B11/B12/B13).
#
# Unit test: fixture normalized findings, each tagged with a unique token, run
# through ONE gate pass. A token in stdout => paged; in the spool => digested; in
# neither => log-only.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"
ALLOWLIST_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"

fail() {
  printf 'osquery-route-gate: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $ROUTE ]] || fail "missing helper: $ROUTE"
[[ -f $ALLOWLIST_HELPER ]] || fail "missing helper: $ALLOWLIST_HELPER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
spool="$work/digest-spy.ndjson"

# digest_append is a collaborator of the gate, pinned in its own suite
# (osquery-digest-store.sh). Here we double it with a recording spy so this test
# exercises the gate's ROUTING (did the finding go to digest?), not the helper's
# private-spool internals, and stays fast. The spy records the raw finding, so
# every finding's unique TAGnn token surfaces in whichever channel it was routed to.
#
# Each finding carries a unique TAGnn token in a natural field. For a paged
# finding the whole object is on stdout; for a digested finding the raw object is
# in the spy file.
findings=(
  # -- PAGE --
  '{"q":"new_admin_user","act":"added","cols":{"username":"adminTAG01","uid":"501"},"ep":""}'
  '{"q":"filevault_off","act":"added","cols":{"note":"TAG02"},"ep":""}'
  '{"q":"agent_secretfile_changed","act":"added","cols":{"path":"/Users/x/.config/relay/webhook-secretTAG03"},"ep":""}'
  '{"q":"agent_exposure_changed","act":"added","cols":{"name":"ncTAG04","address":"0.0.0.0","port":"4444"},"ep":""}'
  '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suidTAG05"},"ep":"/tmp/suidTAG05"}'
  '{"q":"persistence_launchd","act":"added","cols":{"path":"/Users/x/Library/LaunchAgents/com.TAG06.plist","label":"com.TAG06","program":"/Users/x/bin/tag06"},"ep":"/Users/x/Library/LaunchAgents/com.TAG06.plist"}'
  '{"q":"file_events_recent","act":"added","cols":{"category":"sshd_config","target_path":"/etc/ssh/sshd_configTAG07"},"ep":"/etc/ssh/sshd_configTAG07"}'
  '{"q":"file_events_recent","act":"added","cols":{"category":"ssh","target_path":"/Users/x/.ssh/authorized_keys","note":"TAG08"},"ep":"/Users/x/.ssh/authorized_keys"}'
  '{"q":"file_events_recent","act":"added","cols":{"category":"pipeline_integrity","target_path":"/Users/x/.local/libexec/osquery/results-alerterTAG09.sh","sha256":"abc","action":"UPDATED"},"ep":"/Users/x/.local/libexec/osquery/results-alerterTAG09.sh"}'
  # -- DIGEST --
  '{"q":"agent_authfile_changed","act":"added","cols":{"path":"/Users/x/.codex/config.tomlTAG10"},"ep":""}'
  '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.ext.TAG11"},"ep":""}'
  '{"q":"listening_ports_non_loopback","act":"added","cols":{"name":"procTAG12","address":"0.0.0.0","port":"9999"},"ep":""}'
  '{"q":"file_events_recent","act":"added","cols":{"category":"ssh","target_path":"/Users/x/.ssh/id_rsaTAG13"},"ep":"/Users/x/.ssh/id_rsaTAG13"}'
  '{"q":"file_events_recent","act":"added","cols":{"category":"sudoers","target_path":"/etc/sudoersTAG14"},"ep":"/etc/sudoersTAG14"}'
  '{"q":"file_events_recent","act":"added","cols":{"category":"allowlist_file","target_path":"/Users/x/.config/osquery-TAG15/page-launchd-allowlist.txt"},"ep":""}'
  # -- LOG-ONLY --
  '{"q":"agent_exposure_changed","act":"removed","cols":{"name":"ncTAG16","address":"0.0.0.0","port":"4444"},"ep":""}'
  '{"q":"firewall_state","act":"added","cols":{"global_state":"0","note":"TAG17"},"ep":""}'
  '{"q":"homebrew_packages","act":"added","cols":{"name":"pkgTAG18"},"ep":""}'
  '{"q":"kernel_extensions_new","act":"added","cols":{"name":"kextTAG19"},"ep":""}'
  '{"q":"filevault_off","act":"removed","cols":{"note":"TAG20"},"ep":""}'
)

# One gate pass. The spy records every digested finding to the spool file.
# allowlist_verdict is sourced (the persistence arm consults it); with the
# allowlist file pointed at a nonexistent path, the user-agent finding (TAG06) is
# not-found -> pages under default-deny.
page_out="$(printf '%s\n' "${findings[@]}" |
  DIGEST_SPY="$spool" OSQUERY_LAUNCHD_ALLOWLIST="$work/no-allowlist.txt" bash -c '
    source "$1"
    source "$2"
    digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
    route_findings
  ' _ "$ROUTE" "$ALLOWLIST_HELPER")"

# classify <token> <page|digest|logonly>: assert where the token surfaced.
classify() {
  local token="$1" want="$2" in_page=no in_spool=no
  grep -qF "$token" <<<"$page_out" && in_page=yes
  [[ -f $spool ]] && grep -qF "$token" "$spool" && in_spool=yes
  case "$want" in
    page) [[ $in_page == yes && $in_spool == no ]] || fail "$token expected PAGE, got page=$in_page digest=$in_spool" ;;
    digest) [[ $in_spool == yes && $in_page == no ]] || fail "$token expected DIGEST, got page=$in_page digest=$in_spool" ;;
    logonly) [[ $in_page == no && $in_spool == no ]] || fail "$token expected LOG-ONLY, got page=$in_page digest=$in_spool" ;;
  esac
}

# -- PAGE --
classify TAG01 page # new_admin_user (criterion 1)
classify TAG02 page # filevault_off added (criterion 2)
classify TAG03 page # agent_secretfile_changed (criterion 3: the 2 secrets)
classify TAG04 page # agent_exposure_changed added (criterion 3: off-loopback)
classify TAG05 page # suid_bin_unexpected added
classify TAG06 page # persistence_launchd added (unconditional now; TODO B11 allowlist)
classify TAG07 page # file_events sshd_config
classify TAG08 page # file_events ssh authorized_keys
classify TAG09 page # file_events pipeline_integrity (unconditional now; TODO B12 pipeline)

# -- DIGEST --
classify TAG10 digest # agent_authfile_changed (criterion 3: the 3 non-secret configs)
classify TAG11 digest # system_extensions_new
classify TAG12 digest # listening_ports_non_loopback added
classify TAG13 digest # file_events ssh (a non-authorized_keys file)
classify TAG14 digest # file_events sudoers
classify TAG15 digest # file_events allowlist_file (the page-allowlist edit itself)

# -- LOG-ONLY --
classify TAG16 logonly # agent_exposure_changed removed (the exposure being fixed)
classify TAG17 logonly # firewall_state (poller-owned; routing here would double-page)
classify TAG18 logonly # homebrew_packages (INFO drift)
classify TAG19 logonly # kernel_extensions_new (wrong signal)
classify TAG20 logonly # filevault_off removed (encryption restored)

# -- Contract: every paged line is exactly one CRIT finding; counts add up. --
page_count="$(grep -c . <<<"$page_out" || true)"
[[ $page_count -eq 9 ]] || fail "expected 9 page-candidates, got $page_count"
[[ "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$page_out")" == true ]] ||
  fail "every page-candidate must carry .sev == CRIT"
digest_count="$(grep -c . "$spool" 2>/dev/null || true)"
[[ $digest_count -eq 6 ]] || fail "expected 6 digested findings, got $digest_count"

printf 'osquery-route-gate: OK (9 page-candidates all CRIT, 6 digested, 5 log-only; criteria 1-3 tiers pinned; poller-owned + drift are log-only)\n'
