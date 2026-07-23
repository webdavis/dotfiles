#!/usr/bin/env bash
#
# digest.sh - the daily osquery digest builder. Drains the digest spool (NDJSON,
# written by the alerter's digest_append) into ONE grouped, silent, non-paging
# message, then rotates the live store aside for forensics. Empty-suppressed: an
# absent, zero-byte, or whitespace-only store produces no message and no error.
#
# Cadence is owned by the daily launchd agent, NOT this script: there is no
# internal time gate, so a manual invocation builds (or stays silent) exactly the
# way the scheduled one does. That keeps the "when" in one declarative place (the
# LaunchAgent's StartCalendarInterval) and makes the builder trivially testable.
set -euo pipefail

# The shared dispatch library, from the libexec home (the same deployed path the
# other consumers source; the literal string lets the relocation guard assert it).
# send_alert is the write-ahead-durable sender the CRIT page path also uses, so
# the digest inherits that durability without its own delivery machinery.
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

# The digest spool the alerter's digest_append accumulates into, one NDJSON line
# per non-paging finding. Same default path and OSQUERY_DIGEST_STORE override as
# the write side (results-alerter/digest-store.sh), so reader and writer agree on
# the file without a shared constant that could drift between them.
OSQUERY_DIGEST_STORE_DEFAULT="$HOME/.local/state/osquery-digest-spool/digest.ndjson"

# rotated_work_file <store> - the unique work-file path this run claims its batch
# into. Derived from the store path plus a UTC unix timestamp and a .build suffix:
# unique-per-run so a stale work file from a crashed run is never silently reused,
# and .build names the in-flight batch for forensics.
rotated_work_file() { printf '%s.%s.build' "$1" "$(date -u +%s)"; }

# restore_batch <work_file> <store> - put the claimed batch back as the live store
# so the next daily run retries it. This is the ERR trap's action for a build
# failure BEFORE the send: a silently dropped digest is invisible to this single
# user, so a failed build must leave the findings for another run, not lose them.
restore_batch() { mv -f "$1" "$2" 2>/dev/null || true; }

# render_digest_body <work_file> - render the grouped, capped digest body from the
# rotated batch. Implemented in the grouping behavior; defined here as the build
# step the ERR trap wraps, so a render failure restores the batch instead of
# losing the day's findings.
render_digest_body() { :; }

main() {
  local store work_file
  store="${OSQUERY_DIGEST_STORE:-$OSQUERY_DIGEST_STORE_DEFAULT}"

  # Empty-suppression, first gate: an absent or zero-byte store has nothing to
  # summarize, so stay silent. -s is false for both a missing and an empty file.
  [[ -s $store ]] || return 0

  # Atomically claim the batch: move the live store aside so findings the alerter
  # appends WHILE we build land in a fresh $store for the next run instead of being
  # consumed (then rotated away) by this one. A failed mv leaves the store
  # untouched, so nothing is lost; stay silent and let the next run retry.
  work_file="$(rotated_work_file "$store")"
  mv -f "$store" "$work_file" 2>/dev/null || return 0

  # From here until the send, any build failure must restore the batch rather than
  # drop the day's digest. The send itself is fire-and-forget (a lost daily digest
  # is low-stakes and send_alert is write-ahead durable on its own), so the send
  # behavior clears this trap; it guards the BUILD only.
  trap 'restore_batch "$work_file" "$store"' ERR

  # Empty-suppression, second gate, now on the CLAIMED batch: a whitespace-only or
  # zero-byte batch has no real records, so discard it and stay silent. Reading the
  # work file (not the live store) both guards the exact batch this run claimed and
  # clears accumulated blank lines from the live store on every run.
  grep -q '[^[:space:]]' "$work_file" 2>/dev/null || {
    rm -f "$work_file"
    return 0
  }

  render_digest_body "$work_file"
}

# Run only when executed, not when sourced: a test sources this file to exercise
# an individual step (e.g. to force a build failure and assert the ERR-trap
# restore) without launching the whole flow.
if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  main "$@"
fi
