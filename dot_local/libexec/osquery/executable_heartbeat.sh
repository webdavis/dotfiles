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

# newest_canary_timestamp - print the NEWEST heartbeat_canary row's timestamp as a
# plain integer, or nothing when there is no readable, well-formed canary. Select the
# canary rows by PARSED .name and take the last (newest). fromjson? drops a torn or
# non-JSON line instead of aborting (the resilient idiom normalize.sh uses to read
# these same logs), and matching the PARSED .name is whitespace-tolerant, so the read
# does not couple to osquery's compact serialization. Prefer the envelope unixTime (an
# integer); fall back to the snapshot column. This is the ONE place the log-derived
# value is read AND validated numeric, so a non-numeric or malformed value can never
# reach the freshness decision or the rendered message.
newest_canary_timestamp() {
  local candidate
  [[ -r $OSQUERY_SNAPSHOTS_LOG ]] || return 0
  candidate="$(jq -rR 'fromjson? | select(.name == "heartbeat_canary")
    | (.unixTime // .snapshot[0].unix_time) // empty' "$OSQUERY_SNAPSHOTS_LOG" 2>/dev/null |
    tail -1 || true)"
  [[ $candidate =~ ^[0-9]+$ ]] || return 0
  printf '%s' "$candidate"
}

main() {
  local now last_canary_timestamp age title detail
  now="$(date -u +%s)"
  [[ $now =~ ^[0-9]+$ ]] || now=0

  last_canary_timestamp="$(newest_canary_timestamp)"

  if [[ -n $last_canary_timestamp ]] && ((now - last_canary_timestamp <= canary_max_age)); then
    age=$((now - last_canary_timestamp))
    # A future-dated canary (an NTP step-back leaving the timestamp slightly ahead of
    # now) is still fresh, but its age is negative; clamp it so the message never
    # renders a nonsensical "(-120s ago)". An if (not `&& age=0`) keeps it set -e safe.
    if ((age < 0)); then age=0; fi
    title="✅ osquery pipeline healthy · $(date -u +%Y-%m-%d)"
    detail="- osqueryd is alive and running its schedule: its heartbeat canary is fresh (${age}s ago). This verifies the root daemon itself. The uptime watchdog verifies each monitor agent is loaded and pages if one is down. Silence since the last message means all clear."
    # The EMPTY sound is deliberate: it keeps the message locally silent AND makes
    # send_alert thread tier=muted into the webhook body. A proof-of-life must never
    # ping like a real page. Fire-and-forget: the heartbeat advances no state, so a
    # send failure is low-stakes (the next day re-fires; the watchdog is the pager).
    send_alert CRIT "$title" "$detail" "" || true
  else
    # Not fresh: osqueryd is not producing scheduled results. Report unhealthy,
    # never a blind checkmark. STALE (a real, over-bound age) and MISSING (no canary
    # row at all) are reported HONESTLY as distinct states: an absent timestamp has
    # no elapsed age, so it must not be dressed up as a stale age. Only the
    # arithmetic AGE (validated-numeric operands) is ever rendered, no raw log field.
    title="⚠️ osquery heartbeat · $(date -u +%Y-%m-%d)"
    if [[ -n $last_canary_timestamp ]]; then
      age=$((now - last_canary_timestamp))
      detail="- osqueryd scheduled heartbeat canary is STALE (last ${age}s ago, over ${canary_max_age}s). The root daemon is not producing scheduled results (stopped or wedged). The uptime watchdog pages on this; this note is the silent daily record."
    else
      detail="- osqueryd scheduled heartbeat canary is MISSING (no canary snapshot found). The root daemon is not producing scheduled results, or has never run the schedule. The uptime watchdog pages on this; this note is the silent daily record."
    fi
    # Muted too (empty sound): the heartbeat never pings, even when it reports a
    # problem. The watchdog is what pages; this note is the silent daily record.
    send_alert CRIT "$title" "$detail" "" || true
  fi
}

# Run only when executed, not when sourced: a test sources this file to exercise
# newest_canary_timestamp in isolation without launching the whole flow.
if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  main "$@"
fi
