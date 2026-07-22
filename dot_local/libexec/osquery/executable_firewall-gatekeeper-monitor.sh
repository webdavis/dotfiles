#!/usr/bin/env bash
#
# firewall-gatekeeper-monitor.sh, polled every 60s by a launchd StartInterval
# agent. The security-posture monitor: it reads the live firewall (alf),
# Gatekeeper, AND screen-lock state via osqueryi in the gui/501 user session,
# compares against the previous run's baseline, and pages CRIT only on a
# protection turning OFF. Silent in steady state.
#
# R2-3: screen-lock-off detection lives HERE, not in the root-daemon pack. The
# screenlock osquery table is scoped to the logged-in user, so the ROOT osqueryd
# daemon (no user session) never returns a screenlock row (the pack's screenlock
# queries were dead). This poller runs as a gui/501 user LaunchAgent whose
# osqueryi DOES have the user session, so it is the correct place to read it.

set -euo pipefail

STATE="${OSQUERY_POSTURE_STATE:-$HOME/.local/state/osquery-posture-state.json}"
GAP="$STATE.gap" # page-once marker for a monitoring gap (R2-9)
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Never leave a partial temp baseline behind, on any exit path (a mid-write
# failure, or the empty-read guard below).
trap 'rm -f "$STATE.tmp"' EXIT

# Read the current posture in a single combined query (one osqueryi startup per
# tick, not one per protection). screenlock is folded in per R2-3.
posture=$("$OSQUERYI" --json "
  SELECT
    (SELECT global_state FROM alf) AS firewall,
    (SELECT assessments_enabled FROM gatekeeper) AS gatekeeper,
    (SELECT enabled FROM screenlock) AS screenlock
" 2>/dev/null | jq -c '.[0] // empty' 2>/dev/null || true)

cur_fw=$(jq -r '.firewall // empty' <<<"$posture" 2>/dev/null || echo "")
cur_gk=$(jq -r '.gatekeeper // empty' <<<"$posture" 2>/dev/null || echo "")
cur_sl=$(jq -r '.screenlock // empty' <<<"$posture" 2>/dev/null || echo "")

# R2-9 monitoring gap. Any scalar missing or out of its exact domain (firewall
# 0/1/2, Gatekeeper 0/1, screenlock 0/1) means the security state is UNKNOWN, not
# safe; an empty or failed read leaves all three empty and lands here too. Do NOT
# persist it (it would poison the baseline) and do NOT compare it (it would
# fabricate a transition): the last good baseline is preserved. Page ONCE per gap.
# With no marker, notify FIRST (reusing the notify-before-persist durability) and
# write the marker only on send_alert success; if send_alert fails, leave no
# marker, log, and exit nonzero so the next tick retries (at-least-once). An
# existing marker suppresses re-paging. (Values are bracketed, [] for empty, to
# keep the page apostrophe-free for the alerting stack.)
if ! [[ $cur_fw =~ ^[012]$ && $cur_gk =~ ^[01]$ && $cur_sl =~ ^[01]$ ]]; then
  if [[ ! -f $GAP ]]; then
    gap_body="**Security-posture monitoring gap**"$'\n'"- The posture query returned an unreadable value (firewall=[$cur_fw] gatekeeper=[$cur_gk] screenlock=[$cur_sl]): the firewall / Gatekeeper / screen-lock state is currently UNKNOWN."$'\n'"- A blind monitor cannot see a protection turn off. Did osqueryi or the LaunchAgent break? **Check now.**"$'\n'"- Diagnose: run the posture query by hand, then re-check."
    if ! send_alert CRIT "🔴 **CRITICAL**" "$gap_body" "Sosumi"; then
      printf 'firewall-gatekeeper-monitor: send_alert could not queue the monitoring-gap page; no marker written, retrying next tick\n' >&2
      exit 1
    fi
    : >"$GAP"
  fi
  exit 0
fi

# A valid read cleared the gap (recovery): drop the marker so a future gap pages
# again. Done before the normal transition/persist logic.
rm -f "$GAP" 2>/dev/null || true

# Validate any existing baseline BEFORE trusting it (and before write_state
# overwrites it): it must be owner-only (mode 600) AND parse to three in-domain
# scalars (same domains as above). A group/world-readable, corrupt, or
# out-of-domain baseline is not trustworthy (it could be planted to mask a
# disabled protection, or fabricate a transition), so it is treated as no prior
# baseline. GNU-first stat, BSD fallback.
prev_valid=0
prev_fw=""
prev_gk=""
prev_sl=""
if [[ -f $STATE ]]; then
  st_mode=$(stat -c '%a' "$STATE" 2>/dev/null || stat -f '%Lp' "$STATE" 2>/dev/null || echo "")
  prev_fw=$(jq -r '.firewall // empty' <"$STATE" 2>/dev/null || echo "")
  prev_gk=$(jq -r '.gatekeeper // empty' <"$STATE" 2>/dev/null || echo "")
  prev_sl=$(jq -r '.screenlock // empty' <"$STATE" 2>/dev/null || echo "")
  if [[ $st_mode == "600" && $prev_fw =~ ^[012]$ && $prev_gk =~ ^[01]$ && $prev_sl =~ ^[01]$ ]]; then
    prev_valid=1
  fi
fi

# Persist the current posture owner-only (0600) so a later run can trust its own
# baseline. Written via a private temp file plus an atomic rename. Ordering for an
# OFF transition is notify-before-persist (see below): the baseline advances ONLY
# after send_alert durably enqueues the page. In steady state (no transition) it
# just refreshes the baseline.
write_state() {
  (
    umask 077
    printf '%s\n' "$posture" >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

# No trustworthy prior baseline (first run, or a lost/planted/corrupt state file):
# seed the baseline silently. Paging a protection that is ALREADY off at first
# sight is a later behavior.
if [[ $prev_valid -eq 0 ]]; then
  write_state
  exit 0
fi

# Human-readable state text for the Was: line of each transition block.
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

# A trusted baseline exists: page CRIT only on a protection turning OFF. A
# re-enable (OFF -> ON) is good news, not actionable, and there is no notice
# channel, so it is silent. Each block mirrors the results-alerter protection-off
# shape: bold header, Was/Now state, then a decision-first next step.
crit_blocks=()
if [[ $cur_fw != "$prev_fw" && $cur_fw == "0" ]]; then
  crit_blocks+=("**Firewall turned OFF**"$'\n'"- **Was:** $(fw_to_text "$prev_fw")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did, **investigate now**."$'\n'"- Re-enable it: System Settings → Network → Firewall")
fi
if [[ $cur_gk != "$prev_gk" && $cur_gk == "0" ]]; then
  crit_blocks+=("**Gatekeeper turned OFF**"$'\n'"- **Was:** $(gk_to_text "$prev_gk")"$'\n'"- **Now:** **DISABLED**"$'\n'"- Did you turn this off? If not, something else did, **investigate now**."$'\n'"- Re-enable it: System Settings → Privacy & Security")
fi
if [[ $cur_sl != "$prev_sl" && $cur_sl == "0" ]]; then
  crit_blocks+=("**Screen lock turned OFF**"$'\n'"- **Was:** $(sl_to_text "$prev_sl")"$'\n'"- **Now:** **OFF**"$'\n'"- Did you turn this off? If not, something else did, **investigate now**."$'\n'"- Re-enable it: System Settings → Lock Screen → Require password")
fi

# No OFF transition (steady state or a re-enable): refresh the baseline, silent.
if [[ ${#crit_blocks[@]} -eq 0 ]]; then
  write_state
  exit 0
fi

# An OFF transition: NOTIFY BEFORE PERSIST. send_alert is write-ahead durable, so
# durably enqueue the page FIRST, then advance the baseline only once it is safely
# queued. If send_alert fails (could not persist the page), leave the baseline on
# its prior value and exit nonzero so the next tick re-detects and retries; do not
# swallow the failure. A crash in the narrow window after the page is queued but
# before the baseline advances re-pages next tick (at-least-once), never loses the
# page. One page for the tick, even when several protections turned off together.
body=$(printf '%s\n\n' "${crit_blocks[@]}")
body=${body%$'\n\n'}
title="🔴 **CRITICAL**"
if [[ ${#crit_blocks[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#crit_blocks[@]}"; fi

if ! send_alert CRIT "$title" "$body" "Sosumi"; then
  printf 'firewall-gatekeeper-monitor: send_alert could not queue the CRIT page; baseline not advanced, retrying next tick\n' >&2
  exit 1
fi

write_state
