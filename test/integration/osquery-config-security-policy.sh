#!/usr/bin/env bash
#
# The security-policy pack detects transitions and genuine regressions, never
# dead states or identity churn. Render-driven pins over the rendered
# security-policy-regression pack:
#
#   - remote_access_sharing_state is an ON-transition detector: one row per
#     currently-ENABLED high-risk service via UNION ALL (screen_sharing,
#     remote_management, remote_apple_events, internet_sharing), so a new
#     differential row means that service just turned on and pages. It must
#     NOT mention remote_login or file_sharing (intentionally ON on dresden,
#     the operator's own access).
#   - filevault_off is DIFFERENTIAL (no snapshot flag) at interval 3600 with
#     the constant-row WHERE NOT EXISTS shape: snapshot rows land in
#     osqueryd.snapshots.log, which the alerter never reads, so as a snapshot
#     it never paged; the constant row makes it churn-proof.
#   - the four dead or dead-end queries are gone: screenlock_state and
#     screenlock_off (the screenlock table is scoped to the logged-in user,
#     so the ROOT daemon always returned nothing; a user-level poller
#     re-lands that detection in its own slice) and firewall_off and
#     gatekeeper_off (snapshot floors the alerter never read; the _state
#     differentials already cover those transitions).
#   - filevault_state is honest: a row disappearing is NOT a regression
#     signal (APFS volume identity churn drops rows while the data stays
#     encrypted); genuine off is filevault_off's job.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not found (run inside the nix dev shell)\n'
  exit 0
fi

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT
render() { HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$1"; }

fails=0
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  fails=$((fails + 1))
}

PACK_TEMPLATE=".chezmoitemplates/osquery/packs/security-policy-regression.conf"

pack_json="$(render "$PACK_TEMPLATE")" || fail "pack failed to render"
jq empty <<<"$pack_json" 2>/dev/null || fail "rendered pack is not valid JSON"

# No em-dash anywhere in the shipped pack (descriptions included).
if grep -q $'\xe2\x80\x94' <<<"$pack_json"; then
  fail "the rendered pack contains an em-dash"
fi

# query_field <name> <jq-suffix> -- print one field of one pack query, without
# `// empty` so a boolean false survives instead of collapsing.
query_field() {
  jq -r --arg q "$1" ".queries[\$q]$2" <<<"$pack_json"
}

# --- remote_access_sharing_state: the ON-transition detector ------------------
remote_query="$(query_field remote_access_sharing_state .query)"
[[ -n $remote_query && $remote_query != null ]] ||
  fail "remote_access_sharing_state: query missing"
grep -qF "UNION ALL" <<<"$remote_query" ||
  fail "remote_access_sharing_state: lost the UNION ALL one-row-per-enabled-service shape"
for service in screen_sharing remote_management remote_apple_events internet_sharing; do
  grep -qF "'$service'" <<<"$remote_query" ||
    fail "remote_access_sharing_state: lost the '$service' service literal"
  grep -qE "WHERE $service = 1" <<<"$remote_query" ||
    fail "remote_access_sharing_state: $service is not filtered to currently-ENABLED (= 1)"
done
for excluded in remote_login file_sharing; do
  grep -qF "$excluded" <<<"$remote_query" &&
    fail "remote_access_sharing_state: must not watch $excluded (intentionally ON on dresden)"
done
[[ "$(query_field remote_access_sharing_state .interval)" == "3600" ]] ||
  fail "remote_access_sharing_state: expected interval 3600"

# --- filevault_off: differential, churn-proof constant row --------------------
filevault_off_query="$(query_field filevault_off .query)"
[[ -n $filevault_off_query && $filevault_off_query != null ]] ||
  fail "filevault_off: query missing"
if jq -e '.queries.filevault_off | has("snapshot")' <<<"$pack_json" >/dev/null; then
  fail "filevault_off: must be differential (no snapshot flag; the alerter never reads snapshots.log)"
fi
[[ "$(query_field filevault_off .interval)" == "3600" ]] ||
  fail "filevault_off: expected interval 3600"
grep -qF "WHERE NOT EXISTS" <<<"$filevault_off_query" ||
  fail "filevault_off: lost the constant-row WHERE NOT EXISTS shape"
grep -qF "'filevault' AS protection" <<<"$filevault_off_query" ||
  fail "filevault_off: lost the constant 'filevault' row (the churn-proofing)"

# --- the four dead/dead-end queries are gone -----------------------------------
for removed in screenlock_state screenlock_off firewall_off gatekeeper_off; do
  if jq -e --arg q "$removed" '.queries | has($q)' <<<"$pack_json" >/dev/null; then
    fail "$removed: dead query must be removed from the pack"
  fi
done

# --- filevault_state: honest about identity churn ------------------------------
filevault_state_desc="$(query_field filevault_state .description)"
grep -qF "the rows disappearing = regression signal" <<<"$filevault_state_desc" &&
  fail "filevault_state: description still claims disappearing rows are a regression signal"
grep -qF "NOT a regression" <<<"$filevault_state_desc" ||
  fail "filevault_state: description must state a disappearing row is NOT a regression (identity churn)"

if ((fails > 0)); then
  printf '%d security-policy pack assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: the security-policy pack pages on ON-transitions and genuine FileVault-off, with the dead queries gone\n'
