#!/usr/bin/env bash
#
# heartbeat.sh - the daily proof-of-life. A user LaunchAgent fires it once a day
# and it sends ONE silent message to the #priority route so the operator can
# trust that silence means safe: if the daily message arrives, the osquery
# pipeline is scheduled and alive. It is the POSITIVE complement of the uptime
# watchdog (every 15 min), which is the ALARM that pages when a component is down.
# This proof-of-life is always muted: it must never ping like a real page.
#
# R2-8: it verifies the ROOT DAEMON, not a fresh osqueryi. A standalone osqueryi
# one-shot answers even while osqueryd is stopped or wedged (a blind checkmark).
# Instead it reads the daemon's OWN scheduled heartbeat_canary snapshot
# (osquery.conf, every 600s, written to osqueryd.snapshots.log) and checks it is
# FRESH: a fresh canary proves the daemon is alive AND actively running its
# schedule. A stale or absent canary means the daemon is not producing scheduled
# results, so it reports unhealthy (still silent; the watchdog is what pages).
set -euo pipefail

# The shared dispatch library from the libexec home (the same deployed path the
# other consumers source). send_alert is the write-ahead-durable sender, so the
# heartbeat inherits that durability without its own delivery machinery.
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

# The daemon-scheduled canary snapshot log. osqueryd writes one heartbeat_canary
# row here per interval; the heartbeat reads its freshness, never a one-shot.
OSQUERY_SNAPSHOTS_LOG="${OSQUERY_SNAPSHOTS_LOG:-$HOME/.local/log/osquery/osqueryd.snapshots.log}"

# Freshness bound: the canary runs every 600s, so 1800s (three intervals)
# tolerates a missed tick or two while still catching a genuinely stopped or
# wedged daemon within the daily window. Validated numeric so a malformed env
# override can never render as free text or break the arithmetic.
canary_max_age="${OSQUERY_CANARY_MAX_AGE:-1800}"
[[ $canary_max_age =~ ^[0-9]+$ ]] || canary_max_age=1800

main() {
  local now last_ts age title detail
  now="$(date -u +%s)"
  [[ $now =~ ^[0-9]+$ ]] || now=0

  # The NEWEST heartbeat_canary row's timestamp. Prefer the envelope unixTime (the
  # daemon log-write instant); fall back to the snapshot column. The extracted
  # value is used ONLY after the numeric validation below, so no free-text log
  # field can ever reach the rendered message.
  last_ts=""
  if [[ -r $OSQUERY_SNAPSHOTS_LOG ]]; then
    last_ts="$(grep -F '"name":"heartbeat_canary"' "$OSQUERY_SNAPSHOTS_LOG" 2>/dev/null | tail -1 |
      jq -r '(.unixTime // .snapshot[0].unix_time) // empty' 2>/dev/null || true)"
  fi

  if [[ $last_ts =~ ^[0-9]+$ ]] && ((now - last_ts <= canary_max_age)); then
    age=$((now - last_ts))
    title="✅ osquery pipeline healthy · $(date -u +%Y-%m-%d)"
    detail="- osqueryd is alive and running its schedule: its heartbeat canary is fresh (${age}s ago). This verifies the root daemon itself. The uptime watchdog verifies each monitor agent is loaded and pages if one is down. Silence since the last message means all clear."
    # The EMPTY sound is deliberate: it keeps the message locally silent AND makes
    # send_alert thread tier=muted into the webhook body. A proof-of-life must never
    # ping like a real page. Fire-and-forget: the heartbeat advances no state, so a
    # send failure is low-stakes (the next day re-fires; the watchdog is the pager).
    send_alert CRIT "$title" "$detail" "" || true
  else
    # A stale canary: osqueryd is not producing scheduled results (stopped or
    # wedged). Report unhealthy, never a blind checkmark. Only the arithmetic AGE
    # (validated-numeric operands) is rendered, no raw log field.
    age=$((now - last_ts))
    detail="- osqueryd scheduled heartbeat canary is STALE (last ${age}s ago, over ${canary_max_age}s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record."
    title="⚠️ osquery heartbeat · $(date -u +%Y-%m-%d)"
    # Muted too (empty sound): the heartbeat never pings, even when it reports a
    # problem. The watchdog is what pages; this note is the silent daily record.
    send_alert CRIT "$title" "$detail" "" || true
  fi
}

# Run only when executed, not when sourced: a test may source this file to
# exercise an individual helper without launching the whole flow.
if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  main "$@"
fi
