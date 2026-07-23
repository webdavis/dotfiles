#!/usr/bin/env bash
#
# uptime-watchdog.sh, polled every 15 min by launchd. Asserts the osquery
# notification pipeline is actually ALIVE, because a dead pipeline otherwise looks
# identical to "all quiet" (the alerter is edge-triggered and the queries are
# differential, so genuine silence is normal). Fires a single CRITICAL page via the
# shared dispatcher if any component is down or wedged; silent when everything is
# healthy. Deliberately does NOT use results.log mtime as a signal: hours of
# healthy silence are expected.
#
# Cardinal invariant: FAIL-SAFE toward paging. Any ambiguous or failed check (an
# unloaded agent, a wedged osqueryd, an unhealthy route) resolves to a CRIT, never
# a silent all-healthy. A watchdog that fails quietly is worse than no watchdog.

set -euo pipefail

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"
# The #priority route the pages actually use (the one send_alert POSTs). Probed
# with a bare GET: no signing header, so the HMAC key never reaches this wire.
HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
ROUTE_TIMEOUT="${OSQUERY_WATCHDOG_ROUTE_TIMEOUT:-3}"
# Every deployed osquery LaunchAgent EXCEPT this watchdog (which, if running, is
# loaded by definition). No osquery plist sets KeepAlive, so launchd will not
# reload an unloaded agent: this list is the sole liveness backstop. A calendar or
# interval agent that is merely idle between runs still reports loaded, so listing
# it here cannot false-alarm.
AGENTS=(
  "com.webdavis.osquery-results-alerter"
  "com.webdavis.osquery-firewall-gatekeeper-monitor"
  "com.webdavis.osquery-alert-drainer"
  "com.webdavis.osquery-digest"
  "com.webdavis.osquery-heartbeat"
  "com.webdavis.osquery-tailscale-monitor"
)

# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

uid="$(id -u)"
problems=()

# 1) osqueryd present AND answering. A wedged daemon passes pgrep but cannot answer
#    a one-shot query, the failure mode KeepAlive would not catch.
if ! pgrep -fq '/opt/osquery/.*osqueryd'; then
  problems+=("osqueryd is not running")
elif ! "$OSQUERYI" --json "SELECT 1 AS ok FROM time" >/dev/null 2>&1; then
  problems+=("osqueryd is wedged (not answering queries)")
fi

# 2) Every watched agent is loaded. `launchctl print` failing means the agent is
#    not loaded (fail-safe: page).
for label in "${AGENTS[@]}"; do
  if ! launchctl print "gui/$uid/$label" >/dev/null 2>&1; then
    problems+=("LaunchAgent not loaded: $label")
  fi
done

# 3) The hermes #priority route is configured and reachable. A GET to the POST-only
#    route returns 405 (route present, rejects GET) or 2xx = healthy; 000 (gateway
#    down), 404 (route not configured), or 5xx (gateway erroring) are unhealthy.
route_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time "$ROUTE_TIMEOUT" "$HERMES_PRIORITY_URL" 2>/dev/null)" || route_code=000
[[ $route_code =~ ^[0-9]+$ ]] || route_code=000
case "$route_code" in
  2[0-9][0-9] | 405) : ;; # route present and reachable
  *) problems+=("hermes #priority route unhealthy (HTTP $route_code) at $HERMES_PRIORITY_URL") ;;
esac

if [[ ${#problems[@]} -eq 0 ]]; then exit 0; fi

# Unhealthy: page ONE CRIT (level-triggered, so a persisting outage keeps
# reminding every tick). A dead pipeline is always CRITICAL and always carries a
# sound, so it reaches the #priority channel and pings. bt holds a literal backtick
# so a command name renders as Discord inline-code without shell expansion. The
# body renders only known labels + a validated numeric route code + static text.
bt='`'
title="🔴 **CRITICAL**"
if [[ ${#problems[@]} -gt 1 ]]; then title="🔴 **CRITICAL** (${#problems[@]} issues)"; fi
body="**Monitoring is DOWN**"
for problem in "${problems[@]}"; do body+=$'\n'"- $problem"; done
body+=$'\n'"- **Diagnose:** ${bt}launchctl list | grep -i osquery${bt}"
body+=$'\n'"- Restart the down component, then re-check."

# send_alert is write-ahead durable, so a hard delivery failure is already loudly
# surfaced: keep it off set -e's abort path (the page is fire-and-forget here).
send_alert CRIT "$title" "$body" "Sosumi" || true
