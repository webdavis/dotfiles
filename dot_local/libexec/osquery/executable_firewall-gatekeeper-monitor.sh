#!/usr/bin/env bash
#
# firewall-gatekeeper-monitor.sh, polled every 60s by a launchd StartInterval
# agent. Queries the live firewall (alf) and gatekeeper state via osqueryi,
# compares against the previous run, and fires alerter only on transitions.
# Silent in steady state.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Read current posture in a single combined query so we get one osqueryi
# startup per tick instead of two.
posture="$("$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper
" 2>/dev/null | jq -c '.[0]')"

if [[ -z $posture || $posture == "null" ]]; then
  # osqueryi failed (daemon not up yet on fresh boot, or alf/gatekeeper
  # tables transiently unavailable). Exit silently; next tick will retry.
  exit 0
fi

current_firewall="$(jq -r '.firewall' <<<"$posture")"
current_gatekeeper="$(jq -r '.gatekeeper' <<<"$posture")"

# First run: write state and exit. No notification: we don't know what the
# previous state was, so we can't claim a transition.
if [[ ! -f $STATE ]]; then
  printf '%s\n' "$posture" >"$STATE"
  exit 0
fi

previous_firewall="$(jq -r '.firewall' <"$STATE")"
previous_gatekeeper="$(jq -r '.gatekeeper' <"$STATE")"

# Update state file BEFORE notification so a slow alerter can't cause duplicate
# notifications on the next tick.
printf '%s\n' "$posture" >"$STATE"

# Build human-readable messages for each transition.
firewall_state_text() {
  case "$1" in
    0) echo "OFF" ;;
    1) echo "on (allow signed)" ;;
    2) echo "on (block all)" ;;
    *) echo "?($1)" ;;
  esac
}
gatekeeper_state_text() {
  case "$1" in
    0) echo "DISABLED" ;;
    1) echo "enabled" ;;
    *) echo "?($1)" ;;
  esac
}

# A protection turning OFF is CRITICAL → a focused #priority block; a re-enable is
# a NOTICE → a compact #osquery line. The block mirrors osquery-results-alerter's
# protection-off shape: bold header, Was/Now state, then a decision-first next step
# ("Did you turn this off? …") ahead of the re-enable line.
critical_blocks=()
notice_lines=()

if [[ $current_firewall != "$previous_firewall" ]]; then
  if [[ $current_firewall == "0" ]]; then
    critical_blocks+=("**Firewall turned OFF**"$'\n'"- **Was:** $(firewall_state_text "$previous_firewall")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did: **investigate now**."$'\n'"- Re-enable it: System Settings → Network → Firewall")
  else
    notice_lines+=("🟡 **Firewall**: from: $(firewall_state_text "$previous_firewall"), to: $(firewall_state_text "$current_firewall")")
  fi
fi
if [[ $current_gatekeeper != "$previous_gatekeeper" ]]; then
  if [[ $current_gatekeeper == "0" ]]; then
    critical_blocks+=("**Gatekeeper turned OFF**"$'\n'"- **Was:** $(gatekeeper_state_text "$previous_gatekeeper")"$'\n'"- **Now:** **DISABLED**"$'\n'"- Did you turn this off? If not, something else did: **investigate now**."$'\n'"- Re-enable it: System Settings → Privacy & Security")
  else
    notice_lines+=("🟡 **Gatekeeper**: from: $(gatekeeper_state_text "$previous_gatekeeper"), to: $(gatekeeper_state_text "$current_gatekeeper")")
  fi
fi

# No transitions. Silent.
[[ ${#critical_blocks[@]} -eq 0 && ${#notice_lines[@]} -eq 0 ]] && exit 0

if [[ ${#critical_blocks[@]} -gt 0 ]]; then
  body="$(printf '%s\n\n' "${critical_blocks[@]}")"
  body=${body%$'\n\n'}
  title="🔴 **CRITICAL**"
  if [[ ${#critical_blocks[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#critical_blocks[@]}"; fi
  send_alert CRIT "$title" "$body" "Sosumi"
fi
if [[ ${#notice_lines[@]} -gt 0 ]]; then
  body="$(printf '%s\n' "${notice_lines[@]}")"
  body=${body%$'\n'}
  send_alert NOTICE "🟡 **Notice** · ${#notice_lines[@]}" "$body" "Glass"
fi
