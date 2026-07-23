#!/usr/bin/env bash
#
# canary-freshness.sh, sourced helper, not run directly. The shared seam for
# reading the freshness of osqueryd's OWN scheduled heartbeat_canary snapshot.
# Sourced by the daily heartbeat (the proof-of-life) and the uptime watchdog (the
# real-time alarm), so both judge the ROOT DAEMON by the same real artifact.
#
# R2-8: a standalone osqueryi one-shot answers even while osqueryd is stopped or
# wedged (it spins up its own ephemeral engine), so it is a blind checkmark that
# cannot prove the running daemon is alive. The daemon's scheduled canary can: only
# a live daemon actively running its schedule writes a fresh heartbeat_canary row
# (osquery.conf schedules it every 600s into osqueryd.snapshots.log). A fresh row
# proves liveness AND scheduling; a stale or absent row means the daemon is not
# producing scheduled results.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/libexec/osquery/canary-freshness.sh"
#   timestamp="$(newest_canary_timestamp)"   # newest canary epoch, or empty

# The daemon-scheduled canary snapshot log. osqueryd writes one heartbeat_canary
# row here per interval; consumers read its freshness, never a one-shot.
OSQUERY_SNAPSHOTS_LOG="${OSQUERY_SNAPSHOTS_LOG:-$HOME/.local/log/osquery/osqueryd.snapshots.log}"

# newest_canary_timestamp, print the NEWEST heartbeat_canary row's timestamp as a
# plain integer, or nothing when there is no readable, well-formed canary. Select the
# canary rows by PARSED .name and take the last (newest). fromjson? drops a torn or
# non-JSON line instead of aborting (the resilient idiom normalize.sh uses to read
# these same logs), and matching the PARSED .name is whitespace-tolerant, so the read
# does not couple to osquery's compact serialization. Prefer the envelope unixTime (an
# integer); fall back to the snapshot column. This is the ONE place the log-derived
# value is read AND validated, protecting BOTH consumers.
#
# The value is range-bound to a PLAUSIBLE base-10 unix epoch so it can never break a
# consumer's bash arithmetic and let a freshness check fall through silent: a leading
# zero is rejected (bash reads 09999999999 as octal and errors), and the value is
# capped at 10 digits (<= 9999999999, year 2286), well inside signed 64-bit, so an
# over-range epoch cannot overflow and wrap both freshness bounds to "fresh". A
# rejected value returns empty, which every consumer treats as MISSING (fail-safe).
newest_canary_timestamp() {
  local candidate
  [[ -r $OSQUERY_SNAPSHOTS_LOG ]] || return 0
  candidate="$(jq -rR 'fromjson? | select(.name == "heartbeat_canary")
    | (.unixTime // .snapshot[0].unix_time) // empty' "$OSQUERY_SNAPSHOTS_LOG" 2>/dev/null |
    tail -1 || true)"
  [[ $candidate =~ ^(0|[1-9][0-9]{0,9})$ ]] || return 0
  printf '%s' "$candidate"
}
