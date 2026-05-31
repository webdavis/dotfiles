#!/usr/bin/env bash
#
# osquery-posture-watch.sh — polled every 60s by a launchd StartInterval
# agent. Queries the live firewall (alf) and gatekeeper state via osqueryi,
# compares against the previous run, and fires alerter only on transitions.
# Silent in steady state.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
OSQUERYI="${OSQUERYI:-/usr/local/bin/osqueryi}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-send-alert.sh"

mkdir -p "$(dirname "$STATE")"

# Read current posture in a single combined query so we get one osqueryi
# startup per tick instead of two.
posture=$("$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper
" 2>/dev/null | jq -c '.[0]')

if [[ -z $posture || $posture == "null" ]]; then
  # osqueryi failed (daemon not up yet on fresh boot, or alf/gatekeeper
  # tables transiently unavailable). Exit silently; next tick will retry.
  exit 0
fi

cur_fw=$(jq -r '.firewall' <<<"$posture")
cur_gk=$(jq -r '.gatekeeper' <<<"$posture")

# First run: write state and exit. No notification — we don't know what the
# previous state was, so we can't claim a transition.
if [[ ! -f $STATE ]]; then
  printf '%s\n' "$posture" >"$STATE"
  exit 0
fi

prev_fw=$(jq -r '.firewall' <"$STATE")
prev_gk=$(jq -r '.gatekeeper' <"$STATE")

# Update state file BEFORE notification so a slow alerter can't cause duplicate
# notifications on the next tick.
printf '%s\n' "$posture" >"$STATE"

# Build human-readable messages for each transition.
fw_to_text() {
  case "$1" in
    0) echo "OFF" ;;
    1) echo "on (allow signed)" ;;
    2) echo "on (block all)" ;;
    *) echo "?($1)" ;;
  esac
}
gk_to_text() {
  case "$1" in
    0) echo "DISABLED" ;;
    1) echo "enabled" ;;
    *) echo "?($1)" ;;
  esac
}

msgs=()
bad=0 # if any transition lands in a "bad" state, use Sosumi instead of Glass

if [[ $cur_fw != "$prev_fw" ]]; then
  msgs+=("Firewall: $(fw_to_text "$prev_fw") → $(fw_to_text "$cur_fw")")
  [[ $cur_fw == "0" ]] && bad=1
fi
if [[ $cur_gk != "$prev_gk" ]]; then
  msgs+=("Gatekeeper: $(gk_to_text "$prev_gk") → $(gk_to_text "$cur_gk")")
  [[ $cur_gk == "0" ]] && bad=1
fi

# No transitions — silent.
[[ ${#msgs[@]} -eq 0 ]] && exit 0

message=$(printf '%s\n' "${msgs[@]}")
# Match the severity scheme used by osquery-results-notify: a protection
# turning off is CRITICAL (🔴); re-enabling is a NOTICE (🟡).
if [[ $bad -eq 1 ]]; then
  title="🔴 CRITICAL — protection disabled"
  sound="Sosumi"
else
  title="🟡 Notice — protection change"
  sound="Glass"
fi

# Dual-channel (local notifier + #osquery Discord) via the shared helper.
send_alert "$title" "$message" "$sound"
