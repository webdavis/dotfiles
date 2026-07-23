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
# unloaded or unknown-state agent, a wedged osqueryd, an unhealthy route, an
# unreadable queue count) resolves to a CRIT, never a silent all-healthy. A
# watchdog that fails quietly is worse than no watchdog.
#
# The watchdog only READS: it probes the delivery queue counts but never drains it
# (the scheduled alert-drainer owns draining), and it renders only KNOWN agent
# labels plus validated-numeric exit codes and counts plus static text, so a value
# influenced by a compromised launchd cannot inject into the page.

set -euo pipefail

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"
# The #priority route the pages actually use (the one send_alert POSTs). Probed
# with a bare GET: no signing header, so the HMAC key never reaches this wire.
HERMES_PRIORITY_URL="${OSQUERY_HERMES_PRIORITY_URL:-http://127.0.0.1:8644/webhooks/osquery-priority}"
ROUTE_TIMEOUT="${OSQUERY_WATCHDOG_ROUTE_TIMEOUT:-3}"
# Cross-run state: per-agent {runs, streak} (so a crash-loop needs a genuine
# RE-RUN, not a frozen daily exit) and the pending backlog {count, growth_streak}
# (so a sustained backlog, not a one-tick burst, pages). Owner-only, atomic.
STATE="${OSQUERY_WATCHDOG_STATE:-$HOME/.local/state/osquery-watchdog-state.json}"
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

# Never leave a partial temp state behind, on any exit path.
trap 'rm -f "$STATE.tmp"' EXIT

# write_state <compact-json>, atomically persist the cross-run state owner-only
# (0600) via a private temp file plus an atomic rename. Returns nonzero on failure
# so the caller can log it (a lost state file only costs one cycle of streak
# memory, never a page: every unhealthy condition re-pages next tick regardless).
write_state() {
  local dir
  dir="$(dirname "$STATE")"
  mkdir -p "$dir" 2>/dev/null || return 1
  (
    umask 077
    printf '%s\n' "$1" >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

# agent_field <launchctl-print-output> <field-label>, print the validated integer
# value of a launchctl-print field ("runs" or "last exit code"). Only the leading
# signed integer after "= " is taken, so a hostile value appended to the line by an
# influenced launchctl cannot inject: the number is all that is ever used, and a
# non-numeric or absent field prints nothing and returns nonzero.
agent_field() {
  local text="$1" label="$2" line
  line="$(printf '%s\n' "$text" | grep -m1 -F "$label = ")" || return 1
  [[ $line =~ =\ *(-?[0-9]+) ]] || return 1
  printf '%s' "${BASH_REMATCH[1]}"
}

# Prior cross-run state. Absent or corrupt (unparseable) starts fresh, never a
# crash: a fresh start means empty streaks and no false growth signal.
prev_state="{}"
if [[ -r $STATE ]]; then
  prev_state="$(cat "$STATE" 2>/dev/null || printf '{}')"
fi
printf '%s' "$prev_state" | jq -e . >/dev/null 2>&1 || prev_state="{}"

uid="$(id -u)"
problems=()

# 1) osqueryd present AND answering. A wedged daemon passes pgrep but cannot answer
#    a one-shot query, the failure mode KeepAlive would not catch.
if ! pgrep -fq '/opt/osquery/.*osqueryd'; then
  problems+=("osqueryd is not running")
elif ! "$OSQUERYI" --json "SELECT 1 AS ok FROM time" >/dev/null 2>&1; then
  problems+=("osqueryd is wedged (not answering queries)")
fi

# 2) Every watched agent is loaded AND not crash-looping. `launchctl print` failing
#    means the agent is not loaded (fail-safe: page). A loaded agent that exits
#    nonzero is a crash-loop candidate, but ONLY when it actually RE-RAN: launchd's
#    `runs` counter must have advanced since the last check. A daily agent's exit
#    is FROZEN between the 15-min checks, so gating on `runs` stops a single daily
#    failure from paging every tick forever. Two failing re-runs (streak >= 2) is
#    the loop; one is a tolerated transient.
agent_state_json="{}"
for label in "${AGENTS[@]}"; do
  if ! print_out="$(launchctl print "gui/$uid/$label" 2>/dev/null)"; then
    problems+=("LaunchAgent not loaded: $label")
    continue
  fi
  runs_readable=1
  runs="$(agent_field "$print_out" 'runs')" || {
    runs=-1
    runs_readable=0
  }
  exit_code="$(agent_field "$print_out" 'last exit code')" || exit_code=0
  prev_runs="$(jq -r --arg l "$label" '.agents[$l].runs // -1' <<<"$prev_state" 2>/dev/null || printf -- '-1')"
  [[ $prev_runs =~ ^-?[0-9]+$ ]] || prev_runs=-1
  prev_streak="$(jq -r --arg l "$label" '.agents[$l].streak // 0' <<<"$prev_state" 2>/dev/null || printf '0')"
  [[ $prev_streak =~ ^[0-9]+$ ]] || prev_streak=0
  if [[ $exit_code -eq 0 ]]; then
    streak=0 # healthy or recovered
  elif [[ $runs_readable -eq 1 && $runs -eq $prev_runs ]]; then
    streak=$prev_streak # a FROZEN nonzero exit (no re-run): do not accumulate
  else
    streak=$((prev_streak + 1)) # a fresh failing re-run (or unreadable runs: fail-safe)
  fi
  agent_state_json="$(jq -c --arg l "$label" --argjson r "$runs" --argjson s "$streak" \
    '. + {($l): {runs: $r, streak: $s}}' <<<"$agent_state_json")"
  if [[ $streak -ge 2 ]]; then
    problems+=("LaunchAgent crash-looping (last exit $exit_code, $streak failing re-runs): $label")
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

# 4) Delivery-backlog health. ANY dead-letter is a permanently-failed alert (the
#    drainer gave up), so it pages unconditionally; an unreadable count is a broken
#    store, which fails safe to a page. A pending backlog pages only on SUSTAINED
#    growth (it grew across two consecutive checks, a growth streak), so a transient
#    burst the drainer absorbs in its next cycle does not false-page. The counts
#    come from the dispatch library's read-only counters; the watchdog never drains.
dead_letter_count="$(osquery_dead_letter_count 2>/dev/null)" || dead_letter_count=""
if [[ $dead_letter_count =~ ^[0-9]+$ ]]; then
  if [[ $dead_letter_count -gt 0 ]]; then
    problems+=("$dead_letter_count alert(s) permanently failed delivery (dead-lettered); the pipeline is broken")
  fi
else
  problems+=("the dead-letter count is unreadable (the alert store may be broken)")
fi

pending_count="$(osquery_pending_alert_count 2>/dev/null)" || pending_count=""
prev_pending="$(jq -r '.pending.count // -1' <<<"$prev_state" 2>/dev/null || printf -- '-1')"
[[ $prev_pending =~ ^-?[0-9]+$ ]] || prev_pending=-1
prev_growth_streak="$(jq -r '.pending.growth_streak // 0' <<<"$prev_state" 2>/dev/null || printf '0')"
[[ $prev_growth_streak =~ ^[0-9]+$ ]] || prev_growth_streak=0
if [[ $pending_count =~ ^[0-9]+$ ]]; then
  pending_for_state=$pending_count
  if [[ $prev_pending -ge 0 && $pending_count -gt $prev_pending ]]; then
    growth_streak=$((prev_growth_streak + 1))
  else
    growth_streak=0
  fi
  if [[ $growth_streak -ge 2 ]]; then
    problems+=("the undelivered-alert backlog has grown for $growth_streak consecutive checks (now $pending_count pending); delivery is not keeping up")
  fi
else
  problems+=("the pending-alert count is unreadable (the alert store may be broken)")
  pending_for_state=-1
  growth_streak=0
fi

# The refreshed state to persist once the tick is resolved. Built now, written only
# AFTER a page is durably queued (notify-before-persist), so a page that cannot be
# stored never advances a streak or growth baseline and masks the signal.
new_state="$(jq -cn --argjson agents "$agent_state_json" \
  --argjson pc "$pending_for_state" --argjson gs "$growth_streak" \
  '{agents: $agents, pending: {count: $pc, growth_streak: $gs}}')"

# Healthy: persist the refreshed baselines (no page to order against) and exit
# silent. A persist failure here only forgets one cycle of streak memory.
if [[ ${#problems[@]} -eq 0 ]]; then
  write_state "$new_state" || printf 'uptime-watchdog: could not persist state (%s)\n' "$STATE" >&2
  exit 0
fi

# Unhealthy: page ONE CRIT (level-triggered, so a persisting outage keeps
# reminding every tick). A dead pipeline is always CRITICAL and always carries a
# sound, so it reaches the #priority channel and pings. bt holds a literal backtick
# so a command name renders as Discord inline-code without shell expansion. The
# body renders only known labels + validated numerics + static text, no raw output.
bt='`'
title="🔴 **CRITICAL**"
if [[ ${#problems[@]} -gt 1 ]]; then title="🔴 **CRITICAL** (${#problems[@]} issues)"; fi
body="**Monitoring is DOWN**"
for problem in "${problems[@]}"; do body+=$'\n'"- $problem"; done
body+=$'\n'"- **Diagnose:** ${bt}launchctl list | grep -i osquery${bt}"
body+=$'\n'"- Restart the down component, then re-check."

# Notify-before-persist: only advance the persisted baselines after send_alert
# durably queues the page. send_alert is write-ahead durable, so on a hard store
# failure it returns nonzero: leave the state as-is and exit nonzero, so launchd
# logs the failure and the next tick re-detects instead of masking the signal.
if send_alert CRIT "$title" "$body" "Sosumi"; then
  write_state "$new_state" || printf 'uptime-watchdog: could not persist state (%s)\n' "$STATE" >&2
  exit 0
fi
printf 'uptime-watchdog: send_alert could not durably queue the CRIT page; state not advanced, retrying next tick\n' >&2
exit 1
