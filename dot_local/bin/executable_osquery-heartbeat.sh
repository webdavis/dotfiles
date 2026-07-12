#!/usr/bin/env bash
#
# osquery-heartbeat.sh - the daily proof-of-life. Fired by launchd at 09:00, it sends ONE silent
# message to the #priority channel so the user can trust that silence = safe: if the daily ✅
# arrives, the osquery pipeline is scheduled and alive. The uptime watchdog (every 15 min) is the
# ALARM that PAGES when a component is down; this is the complementary POSITIVE affirmation. It is
# always silent (empty sound) - a proof-of-life must never ping like a real page.
#
# R2-8: it verifies the ROOT DAEMON, not a fresh osqueryi. The old probe launched a standalone
# osqueryi one-shot, which answers even while osqueryd is stopped or wedged (the probe row was the
# osqueryi process's own pid) - a blind ✅. Instead it checks that the daemon's scheduled
# heartbeat_canary SNAPSHOT (osquery.conf, every 600s → osqueryd.snapshots.log) is FRESH: a fresh
# canary proves the daemon is alive AND actively running its schedule. A stale/absent canary means
# the daemon is not producing scheduled results → report unhealthy (still silent; the watchdog pages).
set -euo pipefail

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

SNAPSHOTS_LOG="${OSQUERY_SNAPSHOTS_LOG:-$HOME/.local/log/osquery/osqueryd.snapshots.log}"
# Freshness bound: the canary runs every 600s, so 1800s (3 intervals) tolerates a missed tick or
# two while still catching a genuinely stopped/wedged daemon within the daily window.
CANARY_MAX_AGE="${OSQUERY_CANARY_MAX_AGE:-1800}"

now=$(date -u +%s)

# Newest heartbeat_canary row's timestamp (the daemon writes one per interval). Prefer the
# envelope unixTime (the daemon's own log-write instant); fall back to the snapshot column.
last_ts=""
if [[ -r $SNAPSHOTS_LOG ]]; then
  last_ts=$(grep -F '"name":"heartbeat_canary"' "$SNAPSHOTS_LOG" 2>/dev/null | tail -1 |
    jq -r '(.unixTime // .snapshot[0].unix_time) // empty' 2>/dev/null || true)
fi

# CRIT selects the #priority channel (the dispatcher's only route); the empty sound makes it
# silent AND threads tier=muted into the POST (R2-11), so this proof-of-life never pings.
if [[ $last_ts =~ ^[0-9]+$ ]] && ((now - last_ts <= CANARY_MAX_AGE)); then
  send_alert CRIT "✅ osquery pipeline healthy · $(date -u +%Y-%m-%d)" \
    "- osqueryd is alive and running its schedule: its heartbeat canary is fresh ($((now - last_ts))s ago). (This verifies the root daemon itself; the uptime watchdog verifies each monitor agent is loaded and pages if one is down.) Silence since the last message means all clear." "" || true
else
  if [[ $last_ts =~ ^[0-9]+$ ]]; then
    detail="- osqueryd's scheduled heartbeat canary is STALE (last $((now - last_ts))s ago, > ${CANARY_MAX_AGE}s) - the root daemon is not producing scheduled results (stopped or wedged)."
  else
    detail="- osqueryd's scheduled heartbeat canary is MISSING (no canary snapshot found) - the root daemon is not producing scheduled results, or has never run the schedule."
  fi
  send_alert CRIT "⚠️ osquery heartbeat · $(date -u +%Y-%m-%d)" \
    "$detail The uptime watchdog pages on this; this note is the silent daily record." "" || true
fi
