#!/usr/bin/env bash
# osquery-config-linklocal.sh (FX9). The two off-loopback listener queries wrongly
# excluded IPv6 link-local (fe80::) alongside real loopback. Link-local is reachable by
# any host on the same link (RFC 4291), not loopback, so an agent bound to a link-local
# address is a real off-box exposure that must not be filtered out. This renders each
# pack and asserts the query's WHERE clause still excludes real loopback (127.0.0.0/8,
# ::1) but no longer excludes fe80.
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

# <pack-template> <query-name>
check_query() {
  local pack="$1" qname="$2" query
  query="$(render "$pack" | jq -r --arg q "$qname" '.queries[$q].query')"
  [[ -n $query && $query != null ]] || {
    fail "$qname: not found in $pack"
    return
  }
  # The WHERE clause must NOT exclude link-local anymore.
  grep -qiE "fe80" <<<"$query" && fail "$qname still excludes link-local (fe80): $query"
  # ...but real loopback exclusion must remain.
  grep -qF "127.%" <<<"$query" || fail "$qname lost the 127.0.0.0/8 loopback exclusion"
  grep -qF "::1" <<<"$query" || fail "$qname lost the ::1 loopback exclusion"
}

check_query ".chezmoitemplates/osquery/packs/agent-attack-surface.conf" agent_exposure_changed
check_query ".chezmoitemplates/osquery/packs/intrusion-detection.conf" listening_ports_non_loopback

if ((fails > 0)); then
  printf '%d link-local query assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: both off-loopback listener queries include link-local, exclude only real loopback\n'
