#!/usr/bin/env bash
#
# The queue-health counters must count CORRECTLY against a live WAL-mode store:
# a reader has to see rows that are committed but still in the write-ahead log,
# not yet checkpointed back into the main database file. This is exactly the
# state a busy store is in between the drain's writes and the next checkpoint, so
# a counter that missed WAL rows would under-report a backed-up queue and the
# watchdog would read a degraded pipeline as healthy.
#
# The counters use a plain `sqlite3 -readonly` connection, which reads committed
# WAL frames like any reader. An `immutable=1` open would skip the WAL and miss
# them; this test would catch that regression (verified by the reviewer). It
# stands up a real WAL database, keeps a concurrent connection OPEN so the
# committed frames stay in the -wal uncheckpointed, and asserts the counts.
#
# Integration, not unit: it holds a live second database connection (a coproc)
# to keep the WAL uncheckpointed. The synchronization is a blocking read on that
# connection's output, so there is no sleep and no flakiness.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"

fail() {
  printf 'osquery-queue-counts-wal: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v sqlite3 >/dev/null 2>&1 || {
  printf 'SKIP: sqlite3 not on PATH; cannot exercise the WAL counters\n'
  exit 0
}
[[ -f $DISPATCH ]] || fail "missing dispatch library: $DISPATCH"

work="$(mktemp -d)"
export OSQUERY_UNDELIVERED_ALERTS_DB="$work/undelivered.sqlite3"

# shellcheck source=/dev/null
source "$DISPATCH"

assert_eq() { # <expected> <actual> <context>
  [[ $2 == "$1" ]] || fail "$3: expected '$1', got '$2'"
}

# Keep DB-absent no-create coverage here too (the read-only probe must not create
# the file when it does not exist yet).
if [[ -e $OSQUERY_UNDELIVERED_ALERTS_DB ]]; then
  fail "precondition: the DB should not exist yet"
fi
assert_eq 0 "$(osquery_pending_alert_count)" "no DB: pending count"
if [[ -e $OSQUERY_UNDELIVERED_ALERTS_DB ]]; then
  fail "a read-only count probe created the DB file"
fi

# Stand up a WAL-mode store and hold a connection OPEN via a coproc so the
# close-time checkpoint never runs and the committed rows stay in the -wal.
coproc HOLDER { sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" 2>&1; }
# shellcheck disable=SC2153 # HOLDER_PID is assigned implicitly by the coproc above
holder_pid=$HOLDER_PID
cleanup() {
  # The braces keep the fd duplication and the stderr silence as two separate
  # redirections (a dead holder makes the printf fail; both are best-effort).
  { printf '.quit\n' >&"${HOLDER[1]}"; } 2>/dev/null || true
  wait "$holder_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

{
  printf 'PRAGMA journal_mode=WAL;\n'
  printf 'PRAGMA wal_autocheckpoint=0;\n'
  printf 'CREATE TABLE pending_alerts (request_id TEXT, next_attempt_after INTEGER);\n'
  printf 'CREATE TABLE dead_letter_alerts (request_id TEXT);\n'
  printf "INSERT INTO pending_alerts (request_id, next_attempt_after) VALUES ('a', 0), ('b', 0), ('c', 0);\n"
  printf "INSERT INTO dead_letter_alerts (request_id) VALUES ('x'), ('y');\n"
  printf "SELECT 'SYNC-DONE';\n"
} >&"${HOLDER[1]}"

# Block until the holder reports it applied the inserts: deterministic, no sleep.
# By the time SYNC-DONE arrives the rows are committed into the WAL, and the
# holder still owns the connection, so no checkpoint has folded them away.
sync_seen=0
while IFS= read -r line <&"${HOLDER[0]}"; do
  if [[ $line == *SYNC-DONE* ]]; then
    sync_seen=1
    break
  fi
done
[[ $sync_seen -eq 1 ]] || fail "the WAL holder never confirmed its inserts"

# The committed frames really are in an uncheckpointed -wal companion file.
if [[ ! -s "$OSQUERY_UNDELIVERED_ALERTS_DB-wal" ]]; then
  fail "expected a non-empty -wal (committed-but-uncheckpointed frames), found none"
fi

# The read-only counters see the committed WAL rows.
assert_eq 3 "$(osquery_pending_alert_count)" "live WAL: pending count"
assert_eq 2 "$(osquery_dead_letter_count)" "live WAL: dead-letter count"

printf 'osquery-queue-counts-wal: OK (counts committed-but-uncheckpointed WAL rows; read-only, no DB create)\n'
