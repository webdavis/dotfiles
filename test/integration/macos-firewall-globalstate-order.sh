#!/usr/bin/env bash
# macos-firewall-globalstate-order.sh -- the Application Firewall must be globally
# ENABLED before stealth mode is set (R1-3). `socketfilterfw --setstealthmode on`
# only yields active protection when the firewall's global state is on; on a fresh
# or drifted machine with the firewall off, a lone stealth record writes a
# preference over an INACTIVE firewall. So macos_system_setup.yaml must declare
# `--setglobalstate on` BEFORE `--setstealthmode on`, and the Tier-2 runner must
# emit them in that order (both under sudo, both idempotent "set to on" commands).
#
# Renders the REAL after_41 runner against the REAL .chezmoidata and asserts both
# firewall commands are emitted, each with a sudo prefix, with globalstate strictly
# before stealth. SKIPs cleanly on a non-darwin host (the template renders empty).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not on PATH; cannot render after_41\n'
  exit 0
}
[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rendered="$work/rendered.sh"
render_home="$work/home"
mkdir -p "$render_home"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" || fail "chezmoi failed to render $TEMPLATE"

if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

gs_cmd='sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setglobalstate on'
sm_cmd='sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setstealthmode on'

grep -qxF "$gs_cmd" "$rendered" ||
  fail "runner must emit the firewall global-state enable ('$gs_cmd'); stealth is inert without it (rendered: $(cat "$rendered"))"
grep -qxF "$sm_cmd" "$rendered" ||
  fail "runner must emit the stealth-mode enable ('$sm_cmd')"

gs_line="$(grep -nF "$gs_cmd" "$rendered" | head -n1 | cut -d: -f1)"
sm_line="$(grep -nF "$sm_cmd" "$rendered" | head -n1 | cut -d: -f1)"
[[ $gs_line -lt $sm_line ]] ||
  fail "global-state enable (line $gs_line) must come BEFORE stealth (line $sm_line): enable the firewall, then set stealth"

printf 'macos-firewall-globalstate-order: OK (globalstate-on precedes stealth-on; both sudo, both idempotent set-on)\n'
