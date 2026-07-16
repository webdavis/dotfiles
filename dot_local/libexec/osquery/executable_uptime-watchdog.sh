#!/usr/bin/env bash
#
# uptime-watchdog.sh, polled every 15 min by launchd. Asserts the
# osquery notification pipeline is actually ALIVE, because a dead pipeline
# otherwise looks identical to "all quiet" (the alerter is edge-triggered and
# the queries are differential, so genuine silence is normal). Fires a single
# CRITICAL alert via the shared dispatcher if any component is down or wedged;
# silent when everything is healthy. Deliberately does NOT use results.log
# mtime as a signal: hours of healthy silence are expected.

set -euo pipefail

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"
HERMES_URL="${OSQUERY_HERMES_URL:-http://127.0.0.1:8644/webhooks/osquery}"
AGENTS=(
  "com.webdavis.osquery-results-alerter"
  "com.webdavis.osquery-firewall-gatekeeper-monitor"
)

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

problems=()

# 1) osqueryd present AND answering: a wedged daemon passes pgrep but can't
#    answer a one-shot query, which is the failure mode KeepAlive won't catch.
if ! pgrep -fq '/opt/osquery/.*osqueryd'; then
  problems+=("osqueryd is not running")
elif ! "$OSQUERYI" --json "SELECT 1 AS ok FROM time" >/dev/null 2>&1; then
  problems+=("osqueryd is wedged (not answering queries)")
fi

# 2) Both notifier LaunchAgents are loaded.
for agent in "${AGENTS[@]}"; do
  if ! launchctl list "$agent" >/dev/null 2>&1; then
    problems+=("LaunchAgent not loaded: $agent")
  fi
done

# 3) hermes gateway reachable (any HTTP status = up; 000 = unreachable). The
#    local alerter in send_alert still fires even if this is what is down.
http_status="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "$HERMES_URL" 2>/dev/null)" || http_status=000
if [[ $http_status == "000" ]]; then
  problems+=("hermes gateway unreachable at $HERMES_URL")
fi

if [[ ${#problems[@]} -eq 0 ]]; then exit 0; fi

# A dead pipeline is always CRITICAL → #priority. Focused block: what is down
# (one bullet each) plus instructive diagnostic + restart steps. backtick holds a
# literal backtick so the command renders as Discord inline-code without shell
# expansion.
backtick='`'
body="**Monitoring is DOWN**"
for problem in "${problems[@]}"; do body+=$'\n'"- $problem"; done
body+=$'\n'"- **Diagnose:** ${backtick}launchctl list | grep -i osquery${backtick}"
body+=$'\n'"- Restart the down component, then re-check."
title="🔴 **CRITICAL**"
if [[ ${#problems[@]} -gt 1 ]]; then title="🔴 **CRITICAL** · ${#problems[@]}"; fi
send_alert CRIT "$title" "$body" "Sosumi"
