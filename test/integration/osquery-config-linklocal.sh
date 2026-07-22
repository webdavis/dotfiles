#!/usr/bin/env bash
#
# Off-loopback listener queries treat IPv6 link-local as REAL exposure: only
# true loopback is excluded. Link-local (fe80::) is reachable by any host on
# the same link (RFC 4291), not loopback, so a listener bound to a link-local
# address is a real off-box exposure that must not be filtered out of either
# watch. Render-driven: each pack renders exactly as at apply time and the
# query's WHERE clause is asserted to exclude real loopback (127.0.0.0/8, ::1)
# while never mentioning fe80.
#
# Two queries carry the guarantee: intrusion-detection's
# listening_ports_non_loopback (the S9 fix re-landed here) and
# agent-attack-surface's agent_exposure_changed (landed already fixed in B1,
# so its half is a green-at-red regression guard).
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

# check_query <pack-template> <query-name> -- the query excludes only true
# loopback and never link-local.
check_query() {
  local pack="$1" query_name="$2" query
  query="$(render "$pack" | jq -r --arg q "$query_name" '.queries[$q].query')"
  [[ -n $query && $query != null ]] || {
    fail "$query_name: not found in $pack"
    return
  }
  # Link-local must NOT be excluded: it is on-link reachable, a real exposure.
  grep -qiE "fe80" <<<"$query" && fail "$query_name still excludes link-local (fe80): $query"
  # ...while the real loopback exclusions must remain.
  grep -qF "127.%" <<<"$query" || fail "$query_name lost the 127.0.0.0/8 loopback exclusion"
  grep -qF "::1" <<<"$query" || fail "$query_name lost the ::1 loopback exclusion"
}

check_query ".chezmoitemplates/osquery/packs/intrusion-detection.conf" listening_ports_non_loopback
check_query ".chezmoitemplates/osquery/packs/agent-attack-surface.conf" agent_exposure_changed

if ((fails > 0)); then
  printf '%d link-local query assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: both off-loopback listener queries include link-local, exclude only real loopback\n'
