#!/usr/bin/env bash
#
# A newly created administrator account pages, and the ROOT daemon proves its
# own liveness on schedule. Two TOP-LEVEL schedule queries in osquery.conf carry
# the guarantee (top-level, not a pack, on purpose: the alerter gate matches the
# bare query name, whereas a pack entry renders as pack_<pack>_<name> and would
# never match):
#
#   new_admin_user   -- a new differential row is a newly created admin (gid 80,
#                       privilege escalation) and pages; the baseline seeds
#                       silently via the counter==0 discard; removed:false so
#                       deleting an admin does not page; platform darwin.
#   heartbeat_canary -- a snapshot query the ROOT osqueryd runs each interval,
#                       writing one row to osqueryd.snapshots.log so a checker
#                       (a later slice) can prove the daemon runs its schedule;
#                       snapshot on purpose so the alerter (which reads only
#                       results.log) never sees it.
#
# Render-driven: chezmoi renders osquery.conf exactly as at apply time and jq
# asserts the shipped schedule JSON, so a template regression fails here before
# it silently changes what the root daemon watches.
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

CONF_TEMPLATE=".chezmoitemplates/osquery/osquery.conf"

conf_json="$(render "$CONF_TEMPLATE")" || fail "osquery.conf failed to render"
jq empty <<<"$conf_json" 2>/dev/null || fail "rendered osquery.conf is not valid JSON"

# No em-dash anywhere in the shipped config (descriptions included).
if grep -q $'\xe2\x80\x94' <<<"$conf_json"; then
  fail "the rendered osquery.conf contains an em-dash"
fi

# sched_field <query-name> <jq-suffix> -- print one field of one schedule entry
# without `// empty`, so a boolean false comes through instead of collapsing.
sched_field() {
  jq -r --arg q "$1" ".schedule[\$q]$2" <<<"$conf_json"
}

# --- new_admin_user: the privilege-escalation pager (top-level, not a pack) ---
admin_query="$(sched_field new_admin_user .query)"
[[ -n $admin_query && $admin_query != null ]] ||
  fail "new_admin_user: query missing from osquery.conf schedule"
[[ $admin_query == "SELECT u.username, u.uid FROM users u JOIN user_groups ug ON u.uid = ug.uid WHERE ug.gid = 80;" ]] ||
  fail "new_admin_user: query semantics changed (got '${admin_query}')"
[[ "$(sched_field new_admin_user .interval)" == "3600" ]] ||
  fail "new_admin_user: expected interval 3600"
[[ "$(sched_field new_admin_user .removed)" == "false" ]] ||
  fail "new_admin_user: expected removed:false (deleting an admin must not page)"
[[ "$(sched_field new_admin_user .platform)" == "darwin" ]] ||
  fail "new_admin_user: expected platform darwin"

# --- heartbeat_canary: the daemon-liveness snapshot ---------------------------
canary_query="$(sched_field heartbeat_canary .query)"
[[ -n $canary_query && $canary_query != null ]] ||
  fail "heartbeat_canary: query missing from osquery.conf schedule"
[[ $canary_query == "SELECT unix_time FROM time;" ]] ||
  fail "heartbeat_canary: query semantics changed (got '${canary_query}')"
[[ "$(sched_field heartbeat_canary .interval)" == "600" ]] ||
  fail "heartbeat_canary: expected interval 600"
[[ "$(sched_field heartbeat_canary .snapshot)" == "true" ]] ||
  fail "heartbeat_canary: expected snapshot:true (so the alerter never sees it)"

if ((fails > 0)); then
  printf '%d schedule-canary assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: new_admin_user pages on a new admin (top-level) and heartbeat_canary snapshots daemon liveness\n'
