#!/usr/bin/env bash
#
# osquery-firewall-gatekeeper-monitor.sh — polled every 60s by a launchd StartInterval agent.
# The security-posture monitor: queries the live firewall (alf), Gatekeeper, AND screen-lock
# state via osqueryi in the gui/501 USER session, compares against the previous run, and pages
# only on a protection turning OFF. Silent in steady state.
#
# R2-3: screen-lock-off detection lives HERE, not in the root-daemon pack. The `screenlock`
# osquery table is scoped to the logged-in user, so the ROOT osqueryd daemon (no user session)
# never returns a screenlock row — the pack's screenlock_off/screenlock_state queries were DEAD
# (live proof: 0 screenlock rows in the daemon's results.log). This poller runs as a gui/501 user
# LaunchAgent whose osqueryi DOES have the user session (verified live: enabled=1 reads here), so
# it is the correct place to detect the lock turning off.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
GAP="$STATE.gap" # page-once marker for a monitoring gap (R2-9)
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Read current posture in a single combined query (one osqueryi startup per tick). screenlock is
# folded in per R2-3.
posture=$("$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper,
    (SELECT enabled FROM screenlock) AS screenlock
" 2>/dev/null | jq -c '.[0] // empty' 2>/dev/null || true)

cur_fw=$(jq -r '.firewall // empty' <<<"$posture" 2>/dev/null || echo "")
cur_gk=$(jq -r '.gatekeeper // empty' <<<"$posture" 2>/dev/null || echo "")
cur_sl=$(jq -r '.screenlock // empty' <<<"$posture" 2>/dev/null || echo "")

# R2-9: validate EVERY scalar against its exact allowed value set BEFORE any classification or
# state update. A partial/empty/malformed reading (e.g. firewall='' gatekeeper='1') is a
# MONITORING GAP — the security state is UNKNOWN, not safe — so it must NOT be persisted as a
# baseline and must page (a blind monitor cannot see a protection turn off). Preserve the last
# valid state; page once per gap (a marker), and clear it on recovery so a transient does not spam.
if ! [[ $cur_fw =~ ^[012]$ && $cur_gk =~ ^[01]$ && $cur_sl =~ ^[01]$ ]]; then
  if [[ ! -f $GAP ]]; then
    : >"$GAP"
    gap_body="**Security-posture monitoring gap**"$'\n'"- The posture query returned an unreadable value (firewall='$cur_fw' gatekeeper='$cur_gk' screenlock='$cur_sl') — the firewall / Gatekeeper / screen-lock state is currently UNKNOWN."$'\n'"- A blind monitor cannot see a protection turn off. Did osqueryi or the LaunchAgent break? **Check now.**"$'\n'"- Diagnose: run the posture query by hand, then re-check."
    gap_title="🔴 **CRITICAL**"
    send_alert CRIT "$gap_title" "$gap_body" "Sosumi" || true
  fi
  exit 0
fi
# A valid sample cleared the gap (recovery) — drop the marker so the next gap pages again.
rm -f "$GAP" 2>/dev/null || true

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
sl_to_text() {
  case "$1" in
    0) echo "OFF" ;;
    1) echo "on" ;;
    *) echo "?($1)" ;;
  esac
}

# Validate any existing baseline before trusting it: it must be owner-only (mode 600) AND parse to
# THREE integer states (firewall, gatekeeper, screenlock). A group/world-readable or corrupt state
# file is not trustworthy (it could be planted to mask a disabled protection), so it is discarded
# and the live sample is evaluated as a first observation. GNU-first stat, BSD fallback. (FX6)
prev_valid=0
prev_fw=""
prev_gk=""
prev_sl=""
if [[ -f $STATE ]]; then
  st_mode=$(stat -c '%a' "$STATE" 2>/dev/null || stat -f '%Lp' "$STATE" 2>/dev/null || echo "")
  prev_fw=$(jq -r '.firewall // empty' <"$STATE" 2>/dev/null || echo "")
  prev_gk=$(jq -r '.gatekeeper // empty' <"$STATE" 2>/dev/null || echo "")
  prev_sl=$(jq -r '.screenlock // empty' <"$STATE" 2>/dev/null || echo "")
  if [[ $st_mode == "600" && $prev_fw =~ ^[0-9]+$ && $prev_gk =~ ^[0-9]+$ && $prev_sl =~ ^[0-9]+$ ]]; then
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
  if [[ $cur_sl == "0" ]]; then
    crit_blocks+=("**Screen lock is OFF (first check)**"$'\n'"- **Now:** **OFF**"$'\n'"- Monitoring just started and the screen-lock password requirement is already off — anyone at the machine has access. Did you turn it off? If not, **investigate now**."$'\n'"- Re-enable it: System Settings → Lock Screen → Require password")
  fi
  if [[ ${#crit_blocks[@]} -eq 0 ]]; then exit 0; fi # healthy seed, silent
  body=$(printf '%s\n\n' "${crit_blocks[@]}")
  body=${body%$'\n\n'}
  title="🔴 **CRITICAL**"
  if [[ ${#crit_blocks[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#crit_blocks[@]}"; fi
  send_alert CRIT "$title" "$body" "Sosumi" || true
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
if [[ $cur_sl != "$prev_sl" && $cur_sl == "0" ]]; then
  crit_blocks+=("**Screen lock turned OFF**"$'\n'"- **Was:** $(sl_to_text "$prev_sl")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did — **investigate now**."$'\n'"- Re-enable it: System Settings → Lock Screen → Require password")
fi

# No OFF transition — silent.
[[ ${#crit_blocks[@]} -eq 0 ]] && exit 0

body=$(printf '%s\n\n' "${crit_blocks[@]}")
body=${body%$'\n\n'}
title="🔴 **CRITICAL**"
if [[ ${#crit_blocks[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#crit_blocks[@]}"; fi
send_alert CRIT "$title" "$body" "Sosumi" || true
