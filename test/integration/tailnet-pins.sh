#!/usr/bin/env bash
# tailnet-pins.sh — MagicDNS /etc/hosts fallback pins are STRUCTURED DATA
# (`macos.tailnet_pins` in .chezmoidata/macos_system_setup.yaml); the Tier-2 sudo
# runner template (run_onchange_after_41) generates the guard-then-append command
# for each pin. Two test layers:
#
# LAYER 1 — MACHINERY (fixture): copy the REAL template into a temp chezmoi
# source dir with fixture chezmoidata carrying test-owned pins (TEST-NET-1
# addresses, never real tailnet data), render it, and assert:
#   - the EXACT generated command string per pin (expectation hardcoded here —
#     an independent derivation, never re-implemented from the template logic);
#   - `sudo -v` is emitted even when the system_setup commands list is EMPTY
#     (pins must still apply; the upfront timestamp covers them);
#   - a fixture with NO tailnet_pins key still renders (the `index` absent-key
#     gotcha) and keeps the `exit 0` early-return;
#   - executing the generated command (sudo stripped, path substituted) against
#     a temp hosts file twice: exact ip\tfqdn\tshort line present, byte-identical
#     on run 2 (idempotent), pre-existing content preserved.
#
# LAYER 2 — SHAPE (real data): read the real YAML's pins via yq and validate form
# only — fields non-empty, ip inside the proper Tailscale CGNAT range
# 100.64.0.0/10, fqdn ends .ts.net, short == the fqdn's first label. No
# behavioral expectations are derived from real data.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guards: plain test/*.sh scripts run outside the Nix shell.
for tool in chezmoi yq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot exercise the tailnet-pins machinery\n' "$tool"
    exit 0
  fi
done
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -f $YAML ]] || fail "missing data file: $YAML"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# render_fixture <name> <fixture-yaml-body...on stdin> -> $work/<name>.rendered
render_fixture() {
  local name="$1"
  local src="$work/$name-src"
  mkdir -p "$src/.chezmoiscripts" "$src/.chezmoidata"
  cp "$TEMPLATE" "$src/.chezmoiscripts/"
  cat >"$src/.chezmoidata/macos_system_setup.yaml"
  local render_home="$work/$name-home"
  mkdir -p "$render_home"
  HOME="$render_home" CI=1 chezmoi --source "$src" execute-template --no-tty \
    <"$src/.chezmoiscripts/$(basename "$TEMPLATE")" >"$work/$name.rendered" ||
    fail "$name: chezmoi failed to render the template (absent-key gotcha? see {{ index }})"
}

# ---------- LAYER 1a: pins render with an EMPTY commands list ----------------
render_fixture pins <<'EOF'
macos:
  system_setup: []
  tailnet_pins:
    - fqdn: pin.example.test
      ip: "192.0.2.7"
      short: pin
    - fqdn: pin2.example.test
      ip: "192.0.2.8"
      short: pin2
EOF
rendered="$work/pins.rendered"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Independent expectation: the exact command the template must generate per pin.
expected_1='sudo sh -c '\''grep -qF "pin.example.test" /etc/hosts || printf "192.0.2.7\tpin.example.test\tpin\n" >>/etc/hosts'\'''
expected_2='sudo sh -c '\''grep -qF "pin2.example.test" /etc/hosts || printf "192.0.2.8\tpin2.example.test\tpin2\n" >>/etc/hosts'\'''
grep -qxF "$expected_1" "$rendered" ||
  fail "generated pin command 1 missing or wrong; expected exactly: $expected_1 (rendered: $(cat "$rendered"))"
grep -qxF "$expected_2" "$rendered" ||
  fail "generated pin command 2 missing or wrong; expected exactly: $expected_2"
grep -qxF 'sudo -v' "$rendered" ||
  fail "sudo -v not emitted with an empty commands list — the upfront timestamp must cover pin commands"
if grep -qxF 'exit 0' "$rendered"; then
  fail "early-return emitted despite pins being configured — pins would never apply"
fi

# ---------- LAYER 1b: no tailnet_pins key -> early-return survives ----------
render_fixture nopins <<'EOF'
macos:
  system_setup: []
EOF
grep -qxF 'exit 0' "$work/nopins.rendered" ||
  fail "with no commands and no pins the runner must keep its exit-0 early-return"
if grep -qxF 'sudo -v' "$work/nopins.rendered"; then
  fail "spurious sudo -v emitted when there is nothing to run"
fi

# ---------- LAYER 1c: execute the generated commands (idempotence) ----------
hosts="$work/hosts"
printf '127.0.0.1\tlocalhost\n255.255.255.255\tbroadcasthost\n' >"$hosts"

run_pin_command() { # strip the sudo prefix, point at the temp hosts file, run
  local cmd="${1#sudo }"
  cmd="${cmd///etc\/hosts/$hosts}"
  bash -c "$cmd" || fail "generated pin command failed (rc=$?): $cmd"
}
for round in 1 2; do
  run_pin_command "$expected_1"
  run_pin_command "$expected_2"
  if [[ $round -eq 1 ]]; then
    cp "$hosts" "$work/hosts.after1"
  fi
done
grep -qxF $'192.0.2.7\tpin.example.test\tpin' "$hosts" ||
  fail "pin 1 line missing or malformed after execution: $(grep -F pin.example.test "$hosts" || echo '<absent>')"
grep -qxF $'192.0.2.8\tpin2.example.test\tpin2' "$hosts" ||
  fail "pin 2 line missing or malformed after execution"
cmp -s "$hosts" "$work/hosts.after1" ||
  fail "NOT idempotent: round 2 changed the file ($(grep -cF example.test "$hosts") pin lines)"
grep -qxF $'127.0.0.1\tlocalhost' "$hosts" || fail "pre-existing hosts content was clobbered"

# ---------- LAYER 2: shape of the REAL pins data -----------------------------
pin_count="$(yq eval '.macos.tailnet_pins | length' "$YAML")"
[[ $pin_count =~ ^[0-9]+$ && $pin_count -ge 1 ]] ||
  fail "real YAML must declare at least one tailnet pin (got: $pin_count)"

# Tailscale node IPs live in CGNAT 100.64.0.0/10 -> second octet 64-127.
cgnat_regex='^100\.(6[4-9]|[789][0-9]|1[01][0-9]|12[0-7])\.[0-9]{1,3}\.[0-9]{1,3}$'
while IFS=$'\t' read -r fqdn ip short; do
  [[ -n $fqdn && -n $ip && -n $short && $fqdn != "null" && $ip != "null" && $short != "null" ]] ||
    fail "pin has empty/missing fields: fqdn='$fqdn' ip='$ip' short='$short'"
  [[ $ip =~ $cgnat_regex ]] ||
    fail "pin IP '$ip' is not inside the Tailscale CGNAT range 100.64.0.0/10 ($fqdn)"
  [[ $fqdn == *.ts.net ]] || fail "pin FQDN '$fqdn' is not a MagicDNS .ts.net name"
  [[ $fqdn == "$short".* ]] ||
    fail "pin short name '$short' is not the first label of '$fqdn'"
done < <(yq eval '.macos.tailnet_pins[] | [.fqdn, .ip, .short] | @tsv' "$YAML")

echo "tailnet-pins: OK (template generates exact idempotent commands from fixture data; $pin_count real pin(s) well-formed)"
