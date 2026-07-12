#!/usr/bin/env bash
# osquery-screenlock-off-query.sh. The screenlock_off detector mirrors filevault_off:
# a DIFFERENTIAL query returning one CONSTANT row only when the screen lock is
# disabled, empty otherwise. This renders the pack, extracts the query, runs it live,
# and asserts the row-count invariant against the host's actual screenlock state:
# lock ON -> zero rows (silent), lock OFF -> exactly one constant protection row.
# Also asserts the schedule shape: differential (no snapshot flag) at interval 3600.
#
# Live + environment-bound (needs osqueryi) -> e2e camp; SKIPs where absent (CI).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || true)}"
[[ -n $OSQUERYI ]] || {
  printf 'SKIP: osqueryi not found\n'
  exit 0
}
command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not found (run inside the nix dev shell)\n'
  exit 0
}

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT
pack_json="$(HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <.chezmoitemplates/osquery/packs/security-policy-regression.conf)"

query="$(jq -r '.queries.screenlock_off.query' <<<"$pack_json")"
[[ -n $query && $query != null ]] || {
  printf 'FAIL: screenlock_off query not found in security-policy-regression.conf\n' >&2
  exit 1
}

# Schedule shape: differential (snapshot absent/false), hourly, darwin.
snapshot="$(jq -r '.queries.screenlock_off.snapshot // false' <<<"$pack_json")"
interval="$(jq -r '.queries.screenlock_off.interval' <<<"$pack_json")"
[[ $snapshot == false ]] || {
  printf 'FAIL: screenlock_off is a snapshot query; the alerter never reads snapshots.log\n' >&2
  exit 1
}
[[ $interval == 3600 ]] || {
  printf 'FAIL: screenlock_off interval is %s, expected 3600\n' "$interval" >&2
  exit 1
}

enabled="$("$OSQUERYI" --json "SELECT enabled FROM screenlock;" 2>/dev/null | jq -r '.[0].enabled // empty')"
[[ -n $enabled ]] || {
  printf 'SKIP: screenlock table returned no row in this context\n'
  exit 0
}

rows="$("$OSQUERYI" --json "$query" 2>/dev/null | jq 'length')"
if [[ $enabled == "1" ]]; then
  [[ $rows -eq 0 ]] || {
    printf 'FAIL: screen lock is ON but screenlock_off returned %s row(s)\n' "$rows" >&2
    exit 1
  }
  printf 'PASS: lock ON -> screenlock_off yields no row (silent)\n'
else
  [[ $rows -eq 1 ]] || {
    printf 'FAIL: screen lock is OFF but screenlock_off returned %s row(s), expected 1 constant row\n' "$rows" >&2
    exit 1
  }
  printf 'PASS: lock OFF -> screenlock_off yields exactly one constant row\n'
fi
