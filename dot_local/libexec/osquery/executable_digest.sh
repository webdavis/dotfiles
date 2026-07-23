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
store="${OSQUERY_DIGEST_STORE:-$OSQUERY_DIGEST_STORE_DEFAULT}"

# Empty-suppression, first gate: an absent or zero-byte store has nothing to
# summarize, so stay silent. -s is false for both a missing and an empty file.
[[ -s $store ]] || exit 0

# Empty-suppression, second gate: a store holding only whitespace (blank lines
# left by an interrupted write, say) carries no real records, so stay silent too.
# grep finds no non-whitespace byte.
grep -q '[^[:space:]]' "$store" || exit 0
