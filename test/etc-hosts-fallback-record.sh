#!/usr/bin/env bash
# etc-hosts-fallback-record.sh — the MagicDNS /etc/hosts fallback pin must exist as
# a declarative record in .chezmoidata/macos_system_setup.yaml and be IDEMPOTENT:
# the Tier-2 sudo runner (run_onchange_after_41) re-runs every record whenever the
# YAML changes, so a plain append would duplicate the line on each apply.
#
# The runner renders `{{ if .sudo }}sudo {{ end }}{{ .command }}` — the command is
# inlined verbatim into the rendered script. A bare `... >>/etc/hosts` redirect
# would therefore run in the OUTER user shell (sudo covers only the first simple
# command, never the redirect), so the record must wrap the whole guard-then-append
# in `sh -c '...'`: `sudo sh -c '...'` puts the redirect inside the root shell.
#
# Asserts:
#   1. the record exists (description mentions MagicDNS) and has sudo: true
#   2. the command is `sh -c '...'`-wrapped (redirect lands as root under the runner)
#   3. run once against a temp hosts file -> the tab-separated pin line is appended
#   4. run twice -> file byte-identical (idempotent guard-then-append)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"
PIN_NAME="mister.tail2f2430.ts.net"
PIN_LINE=$'100.109.58.54\tmister.tail2f2430.ts.net\tmister'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guard: plain test/*.sh scripts run outside the Nix shell.
if ! command -v yq >/dev/null 2>&1; then
  printf 'SKIP: yq not on PATH; cannot extract the system_setup record\n'
  exit 0
fi
[[ -f $YAML ]] || fail "missing data file: $YAML"

# --- 1. the record exists, with sudo: true ----------------------------------
record_command="$(yq eval \
  '.macos.system_setup[] | select(.description | contains("MagicDNS")) | .command' "$YAML")"
[[ -n $record_command ]] ||
  fail "no MagicDNS /etc/hosts fallback record in $YAML (description must mention MagicDNS)"

record_sudo="$(yq eval \
  '.macos.system_setup[] | select(.description | contains("MagicDNS")) | .sudo' "$YAML")"
[[ $record_sudo == "true" ]] ||
  fail "the MagicDNS record must set sudo: true (got: $record_sudo)"

# --- 2. the redirect must live inside an sh -c wrapper -----------------------
# The runner prefixes `sudo ` and inlines the command verbatim; without the
# wrapper the >> redirect would run as the invoking user and fail on /etc/hosts.
[[ $record_command == "sh -c '"* ]] ||
  fail "command must be sh -c '...'-wrapped so the redirect runs as root under the runner: $record_command"

# --- 3+4. idempotence against a temp hosts file ------------------------------
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
hosts="$work/hosts"
printf '127.0.0.1\tlocalhost\n255.255.255.255\tbroadcasthost\n' >"$hosts"

# Substitute the real path with the temp file (every occurrence: guard + append).
test_command="${record_command///etc\/hosts/$hosts}"
[[ $test_command != "$record_command" ]] ||
  fail "command does not reference /etc/hosts at all: $record_command"

bash -c "$test_command" || fail "record command failed on first run (rc=$?)"
grep -qxF "$PIN_LINE" "$hosts" ||
  fail "after run 1 the tab-separated pin line is missing or malformed: $(grep "$PIN_NAME" "$hosts" || echo '<absent>')"
cp "$hosts" "$work/hosts.after1"

bash -c "$test_command" || fail "record command failed on second run (rc=$?)"
cmp -s "$hosts" "$work/hosts.after1" ||
  fail "NOT idempotent: second run changed the file ($(grep -c "$PIN_NAME" "$hosts") pin lines)"

# The original content must be intact (append, not rewrite).
grep -qxF $'127.0.0.1\tlocalhost' "$hosts" || fail "pre-existing hosts content was clobbered"

echo "etc-hosts-fallback-record: OK (sudo sh -c wrapped, tab-separated, idempotent append)"
