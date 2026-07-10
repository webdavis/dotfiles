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
# The pin DATA (peer name, IP) lives canonically in the YAML record; this test
# hardcodes none of it. It parses the pin out of the record's own command string
# and validates the record's MECHANICS against whatever the YAML declares — so a
# future IP bump edits only the YAML and this test still guards the machinery:
#   1. the record exists (description mentions MagicDNS) and has sudo: true
#   2. the command is `sh -c '...'`-wrapped (redirect lands as root under the runner)
#   3. the printf pin parses as IP\tFQDN\tSHORT with a Tailscale CGNAT IP and a
#      .ts.net name, and the grep guard watches the same FQDN the printf appends
#   4. run once against a temp hosts file -> exactly that tab-separated line appears
#   5. run twice -> file byte-identical (idempotent guard-then-append), a single
#      pin line, pre-existing content preserved
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"

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

# --- 3. parse the pin from the record itself (no hardcoded tailnet data) -----
# The printf format carries the pin as "IP\tFQDN\tSHORT\n" (literal backslash
# escapes in the YAML string; printf expands them at run time).
printf_regex='printf "([^"]+)"'
[[ $record_command =~ $printf_regex ]] ||
  fail "cannot locate the printf pin format in the command: $record_command"
pin_format="${BASH_REMATCH[1]}"

pin_regex='^([0-9.]+)\\t([A-Za-z0-9.-]+)\\t([A-Za-z0-9-]+)\\n$'
[[ $pin_format =~ $pin_regex ]] ||
  fail "pin format is not IP\\tFQDN\\tSHORT\\n: $pin_format"
pin_ip="${BASH_REMATCH[1]}"
pin_fqdn="${BASH_REMATCH[2]}"
pin_short="${BASH_REMATCH[3]}"

# A mangled record must fail loudly, not pass vacuously: Tailscale node IPs live
# in the 100.64.0.0/10 CGNAT range, and MagicDNS names end in .ts.net.
ip_regex='^100\.[0-9]+\.[0-9]+\.[0-9]+$'
[[ $pin_ip =~ $ip_regex ]] || fail "parsed pin IP is not a Tailscale CGNAT address: $pin_ip"
[[ $pin_fqdn == *.ts.net ]] || fail "parsed pin FQDN is not a MagicDNS name: $pin_fqdn"
[[ $pin_fqdn == "$pin_short".* ]] ||
  fail "short name '$pin_short' is not the FQDN's first label: $pin_fqdn"

# The idempotence guard must watch the same FQDN the printf appends — a mismatch
# would append forever.
guard_regex='grep -qF "([^"]+)"'
[[ $record_command =~ $guard_regex ]] ||
  fail "cannot locate the grep -qF guard in the command: $record_command"
guard_fqdn="${BASH_REMATCH[1]}"
[[ $guard_fqdn == "$pin_fqdn" ]] ||
  fail "guard watches '$guard_fqdn' but printf appends '$pin_fqdn'"

expected_line="$pin_ip"$'\t'"$pin_fqdn"$'\t'"$pin_short"

# --- 4+5. idempotence against a temp hosts file ------------------------------
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
hosts="$work/hosts"
printf '127.0.0.1\tlocalhost\n255.255.255.255\tbroadcasthost\n' >"$hosts"

# Substitute the real path with the temp file (every occurrence: guard + append).
test_command="${record_command///etc\/hosts/$hosts}"
[[ $test_command != "$record_command" ]] ||
  fail "command does not reference /etc/hosts at all: $record_command"

bash -c "$test_command" || fail "record command failed on first run (rc=$?)"
grep -qxF "$expected_line" "$hosts" ||
  fail "after run 1 the tab-separated pin line is missing or malformed: $(grep -F "$pin_fqdn" "$hosts" || echo '<absent>')"
cp "$hosts" "$work/hosts.after1"

bash -c "$test_command" || fail "record command failed on second run (rc=$?)"
cmp -s "$hosts" "$work/hosts.after1" ||
  fail "NOT idempotent: second run changed the file ($(grep -cF "$pin_fqdn" "$hosts") pin lines)"
[[ $(grep -cF "$pin_fqdn" "$hosts") -eq 1 ]] ||
  fail "expected exactly one pin line, found $(grep -cF "$pin_fqdn" "$hosts")"

# The original content must be intact (append, not rewrite).
grep -qxF $'127.0.0.1\tlocalhost' "$hosts" || fail "pre-existing hosts content was clobbered"

echo "etc-hosts-fallback-record: OK (pin derived from the YAML record; sudo sh -c wrapped, tab-separated, idempotent append)"
