#!/usr/bin/env bash
#
# osquery-firewall-gatekeeper-monitor.sh — polled every 60s by a launchd StartInterval
# agent. Queries the live firewall (alf) and gatekeeper state via osqueryi,
# compares against the previous run, and fires alerter only on transitions.
# Silent in steady state.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

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

# Persist the current posture owner-only (0600) so a later run can trust its own
# baseline. Written BEFORE any notification so a slow alerter can't double-notify.
write_state() {
  (
    umask 077
    printf '%s\n' "$posture" >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

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

# Validate any existing baseline before trusting it: it must be owner-only (mode 600)
# AND parse to two integer states. A group/world-readable or corrupt state file is not
# trustworthy (it could be planted to mask a disabled protection), so it is discarded and
# the live sample is evaluated as a first observation. GNU-first stat, BSD fallback. (FX6)
prev_valid=0
prev_fw=""
prev_gk=""
if [[ -f $STATE ]]; then
  st_mode=$(stat -c '%a' "$STATE" 2>/dev/null || stat -f '%Lp' "$STATE" 2>/dev/null || echo "")
  prev_fw=$(jq -r '.firewall // empty' <"$STATE" 2>/dev/null || echo "")
  prev_gk=$(jq -r '.gatekeeper // empty' <"$STATE" 2>/dev/null || echo "")
  if [[ $st_mode == "600" && $prev_fw =~ ^[0-9]+$ && $prev_gk =~ ^[0-9]+$ ]]; then
    prev_valid=1
  fi
fi

# A protection turning OFF is CRITICAL → a focused #priority block. A re-enable is
# good news, not actionable, and v2 has no #osquery channel, so it is silent. The
# block mirrors osquery-results-alerter's protection-off shape: bold header, Was/Now
# state, then a decision-first next step ("Did you turn this off? …").
crit_blocks=()

# First observation (no trustworthy baseline): seed silently ONLY when the posture is
# healthy. If a protection is ALREADY off at first sight, that is a pre-existing exposure
# that must PAGE, not be silently accepted as "normal" (FX6). Always persist the sample.
if [[ $prev_valid -eq 0 ]]; then
  write_state
  if [[ $cur_fw == "0" ]]; then
    crit_blocks+=("**Firewall is OFF (first check)**"$'\n'"- **Now:** **OFF**"$'\n'"- Monitoring just started and the firewall is already off — this may be a pre-existing exposure. Did you turn it off? If not, **investigate now**."$'\n'"- Re-enable it: System Settings → Network → Firewall")
  fi
  if [[ $cur_gk == "0" ]]; then
    crit_blocks+=("**Gatekeeper is OFF (first check)**"$'\n'"- **Now:** **DISABLED**"$'\n'"- Monitoring just started and Gatekeeper is already disabled — this may be a pre-existing exposure. Did you turn it off? If not, **investigate now**."$'\n'"- Re-enable it: System Settings → Privacy & Security")
  fi
  if [[ ${#crit_blocks[@]} -eq 0 ]]; then exit 0; fi # healthy seed, silent
  body=$(printf '%s\n\n' "${crit_blocks[@]}")
  body=${body%$'\n\n'}
  title="🔴 **CRITICAL**"
  if [[ ${#crit_blocks[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#crit_blocks[@]}"; fi
  send_alert CRIT "$title" "$body" "Sosumi"
  exit 0
fi

# Trusted baseline exists: page only on an OFF transition. Update state before notifying.
write_state

if [[ $cur_fw != "$prev_fw" && $cur_fw == "0" ]]; then
  crit_blocks+=("**Firewall turned OFF**"$'\n'"- **Was:** $(fw_to_text "$prev_fw")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did — **investigate now**."$'\n'"- Re-enable it: System Settings → Network → Firewall")
fi
if [[ $cur_gk != "$prev_gk" && $cur_gk == "0" ]]; then
  crit_blocks+=("**Gatekeeper turned OFF**"$'\n'"- **Was:** $(gk_to_text "$prev_gk")"$'\n'"- **Now:** **DISABLED**"$'\n'"- Did you turn this off? If not, something else did — **investigate now**."$'\n'"- Re-enable it: System Settings → Privacy & Security")
fi

# No OFF transition — silent.
[[ ${#crit_blocks[@]} -eq 0 ]] && exit 0

body=$(printf '%s\n\n' "${crit_blocks[@]}")
body=${body%$'\n\n'}
title="🔴 **CRITICAL**"
if [[ ${#crit_blocks[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#crit_blocks[@]}"; fi
send_alert CRIT "$title" "$body" "Sosumi"
