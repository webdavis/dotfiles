#!/usr/bin/env bash
#
# osquery-uptime-watchdog.sh — polled every 15 min by launchd. Asserts the
# osquery notification pipeline is actually ALIVE, because a dead pipeline
# otherwise looks identical to "all quiet" (the alerter is edge-triggered and
# the queries are differential, so genuine silence is normal). Fires a single
# CRITICAL alert via the shared dispatcher if any component is down or wedged;
# silent when everything is healthy. Deliberately does NOT use results.log
# mtime as a signal — hours of healthy silence are expected.

set -euo pipefail

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"
# Probe the #priority route — the path pages actually use. The old /webhooks/osquery
# route was decommissioned, so probing it no longer proves a page can be delivered.
HERMES_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
# Cross-run state (R2-7): per-agent consecutive-nonzero-exit streaks (so a one-off transient does
# not page, but a persistent crash-loop does) and the last-seen page-spool count (to detect a
# growing spool). Owner-only.
STATE="${OSQUERY_WATCHDOG_STATE:-$HOME/.local/state/osquery-watchdog-state.json}"
SPOOL_DIR="${OSQUERY_SPOOL_DIR:-$HOME/.local/state/osquery-spool}"
SPOOL_STALE_MIN="${OSQUERY_SPOOL_STALE_MIN:-30}"
# Every deployed osquery LaunchAgent EXCEPT this watchdog (which, if running, is
# loaded by definition). No osquery plist sets KeepAlive, so launchd will not reload
# an unloaded agent — this list is the sole liveness backstop. A calendar/interval
# agent that is merely idle between runs still reports loaded (exit 0), so listing it
# here cannot false-alarm. The tailscale poller pages on public-internet exposure, the
# digest owns the daily summary, and the heartbeat is the daily proof-of-life whose
# silence the user trusts — all MUST be covered.
AGENTS=(
  "com.webdavis.osquery-results-alerter"
  "com.webdavis.osquery-firewall-gatekeeper-monitor"
  "com.webdavis.osquery-digest"
  "com.webdavis.osquery-tailscale-monitor"
  "com.webdavis.osquery-heartbeat"
)

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

# Backstop drain: the alerter flushes the spool on every results.log change, but if
# nothing is changing this tick (every 15 min) still replays any page spooled while
# the gateway was down. `|| true` keeps it off this watchdog's own failure path.
_drain_spool || true

problems=()

# Prior cross-run state (per-agent exit streaks + last spool count). Empty/corrupt → start fresh.
prev_state="{}"
[ -r "$STATE" ] && prev_state=$(cat "$STATE" 2>/dev/null || echo "{}")
printf '%s' "$prev_state" | jq -e . >/dev/null 2>&1 || prev_state="{}"

# 1) osqueryd present AND answering — a wedged daemon passes pgrep but can't
#    answer a one-shot query, which is the failure mode KeepAlive won't catch.
if ! pgrep -fq '/opt/osquery/.*osqueryd'; then
  problems+=("osqueryd is not running")
elif ! "$OSQUERYI" --json "SELECT 1 AS ok FROM time" >/dev/null 2>&1; then
  problems+=("osqueryd is wedged (not answering queries)")
fi

# 2) Every deployed osquery LaunchAgent is loaded AND not crash-looping (R2-7). A registered job
#    that exits nonzero every run (a StartInterval crash-loop) stays "loaded", so registration
#    alone is not liveness — inspect LastExitStatus. Alert only when it is nonzero on TWO
#    consecutive watchdog checks, so a single transient exit does not page. Streaks accumulate
#    into streaks_json incrementally (a missing entry means "reset to 0" next run).
streaks_json="{}"
for agent in "${AGENTS[@]}"; do
  if ! agent_out=$(launchctl list "$agent" 2>/dev/null); then
    problems+=("LaunchAgent not loaded: $agent")
    continue
  fi
  les=$(printf '%s\n' "$agent_out" | awk -F'[=;]' '/"LastExitStatus"/{gsub(/[^0-9-]/,"",$2); print $2; exit}')
  if [[ $les =~ ^-?[0-9]+$ && $les -ne 0 ]]; then
    prev_streak=$(printf '%s' "$prev_state" | jq -r --arg a "$agent" '.exit_streak[$a] // 0' 2>/dev/null || echo 0)
    [[ $prev_streak =~ ^[0-9]+$ ]] || prev_streak=0
    streak=$((prev_streak + 1))
    streaks_json=$(jq -c --arg a "$agent" --argjson n "$streak" '.[$a]=$n' <<<"$streaks_json")
    if [ "$streak" -ge 2 ]; then
      problems+=("LaunchAgent crash-looping (LastExitStatus=$les, ${streak} consecutive checks): $agent")
    fi
  fi
done

# 3) The hermes #priority route must EXIST and be reachable (R2-7). A GET to the POST-only route
#    returns 405 (route present) or 2xx = healthy; 000 (gateway down), 404 (route not configured),
#    or 5xx (gateway erroring) are unhealthy. The old "any status != 000 = up" wrongly accepted a
#    404 or 5xx as healthy. This proves the route is configured; delivery health is (4) below.
code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "$HERMES_URL" 2>/dev/null) || code=000
case "$code" in
  2?? | 405) : ;; # route present and reachable
  *) problems+=("hermes #priority route unhealthy (HTTP $code) at $HERMES_URL") ;;
esac

# 4) Page-spool health (R2-7). A page that cannot deliver is spooled, so a STALE spooled page
#    (older than SPOOL_STALE_MIN) or a spool that GREW since the last check means pages are not
#    draining — the exact 502-on-POST case a 405 GET cannot see. Either alerts.
spool_count=0
if [ -d "$SPOOL_DIR" ]; then
  spool_count=$(find "$SPOOL_DIR" -type f ! -name '*.tmp.*' 2>/dev/null | wc -l | tr -d ' ')
  if [ -n "$(find "$SPOOL_DIR" -type f ! -name '*.tmp.*' -mmin "+$SPOOL_STALE_MIN" 2>/dev/null | head -1)" ]; then
    problems+=("page spool is STALE: an undelivered page has sat > ${SPOOL_STALE_MIN}m — Discord delivery is broken")
  fi
fi
prev_spool=$(printf '%s' "$prev_state" | jq -r '.spool_count // 0' 2>/dev/null || echo 0)
[[ $prev_spool =~ ^[0-9]+$ ]] || prev_spool=0
if [ "$spool_count" -gt "$prev_spool" ] && [ "$spool_count" -gt 0 ]; then
  problems+=("page spool is GROWING ($prev_spool → $spool_count undelivered pages) — pages are not draining")
fi

# Persist the new state (per-agent streaks + current spool count) atomically, owner-only, BEFORE
# alerting so a slow dispatch cannot double-count.
new_state=$(jq -cn --argjson streaks "$streaks_json" --argjson spool "$spool_count" \
  '{exit_streak:$streaks, spool_count:$spool}')
mkdir -p "$(dirname "$STATE")"
(
  umask 077
  printf '%s\n' "$new_state" >"$STATE.tmp"
) && mv -f "$STATE.tmp" "$STATE"

if [ ${#problems[@]} -eq 0 ]; then exit 0; fi

# A dead pipeline is always CRITICAL → #priority. Focused block: what is down
# (one bullet each) plus instructive diagnostic + restart steps. bt holds a literal
# backtick so the command renders as Discord inline-code without shell expansion.
bt='`'
body="**Monitoring is DOWN**"
for p in "${problems[@]}"; do body+=$'\n'"- $p"; done
body+=$'\n'"- **Diagnose:** ${bt}launchctl list | grep -i osquery${bt}"
body+=$'\n'"- Restart the down component, then re-check."
title="🔴 **CRITICAL**"
if [ ${#problems[@]} -gt 1 ]; then title="🔴 **CRITICAL** · ${#problems[@]}"; fi
# `|| true`: send_alert returns nonzero on a hard delivery failure (R2-6); the watchdog is
# fire-and-forget (the failure is already loudly surfaced) — keep it off set -e's abort path.
send_alert CRIT "$title" "$body" "Sosumi" || true
