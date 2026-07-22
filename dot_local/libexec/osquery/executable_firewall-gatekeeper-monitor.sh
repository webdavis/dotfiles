#!/usr/bin/env bash
#
# firewall-gatekeeper-monitor.sh, polled every 60s by a launchd StartInterval
# agent. The security-posture monitor: it reads the live firewall (alf),
# Gatekeeper, AND screen-lock state via osqueryi in the gui/501 user session and
# persists it as an owner-only baseline that a later run compares against.
#
# R2-3: screen-lock-off detection lives HERE, not in the root-daemon pack. The
# screenlock osquery table is scoped to the logged-in user, so the ROOT osqueryd
# daemon (no user session) never returns a screenlock row (the pack's screenlock
# queries were dead). This poller runs as a gui/501 user LaunchAgent whose
# osqueryi DOES have the user session, so it is the correct place to read it.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Read the current posture in a single combined query (one osqueryi startup per
# tick, not one per protection). screenlock is folded in per R2-3.
posture=$("$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper,
    (SELECT enabled FROM screenlock) AS screenlock
" 2>/dev/null | jq -c '.[0] // empty' 2>/dev/null || true)

# Persist the current posture owner-only (0600) so a later run can trust its own
# baseline. Written via a private temp file plus an atomic rename, and BEFORE any
# notification, so a slow alerter cannot double-notify off a half-written state.
write_state() {
  (
    umask 077
    printf '%s\n' "$posture" >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

write_state
