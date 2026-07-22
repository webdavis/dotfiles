#!/usr/bin/env bash
#
# The dispatch library exposes two read-only queue-health counters the watchdog
# slice will poll: osquery_pending_alert_count (undelivered pages still waiting)
# and osquery_dead_letter_count (pages the drain gave up on). Both must be robust
# to a not-yet-created database or table -- a health probe reports zero before
# anything has been stored, never an error -- and must be read-only (a probe must
# never create the file, the schema, or a WAL sidecar).
#
# Unit test: source the real library against a throwaway DB path and pin the
# missing-file, missing-table, and populated cases. No network, no drain, no
# flows.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"

fail() {
  printf 'osquery-queue-counts: FAIL -- %s\n' "$*" >&2
  exit 1
}

command -v sqlite3 >/dev/null 2>&1 || {
  printf 'SKIP: sqlite3 not on PATH; cannot exercise the counters\n'
  exit 0
}
[[ -f $DISPATCH ]] || fail "missing dispatch library: $DISPATCH"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export OSQUERY_UNDELIVERED_ALERTS_DB="$work/undelivered.sqlite3"

# shellcheck source=/dev/null
source "$DISPATCH"

assert_eq() { # <expected> <actual> <context>
  [[ $2 == "$1" ]] || fail "$3: expected '$1', got '$2'"
}

# (a) The DB file does not exist yet: both counters read zero, no error, and the
# read-only probe must not CREATE the database.
if [[ -e $OSQUERY_UNDELIVERED_ALERTS_DB ]]; then
  fail "precondition: the DB should not exist yet"
fi
assert_eq 0 "$(osquery_pending_alert_count)" "no DB: pending count"
assert_eq 0 "$(osquery_dead_letter_count)" "no DB: dead-letter count"
if [[ -e $OSQUERY_UNDELIVERED_ALERTS_DB ]]; then
  fail "a read-only count probe created the DB file"
fi

# (b) The DB exists but neither counted table does: still zero, no error.
sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" 'CREATE TABLE unrelated (x);'
assert_eq 0 "$(osquery_pending_alert_count)" "no table: pending count"
assert_eq 0 "$(osquery_dead_letter_count)" "no table: dead-letter count"

# (c) Seed N pending and M dead-lettered rows: the counters return N and M. Only
# COUNT(*) is exercised, so minimal tables are enough.
sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" <<'SQL'
CREATE TABLE pending_alerts (request_id TEXT, next_attempt_after INTEGER);
CREATE TABLE dead_letter_alerts (request_id TEXT);
INSERT INTO pending_alerts (request_id, next_attempt_after) VALUES ('a', 0), ('b', 0), ('c', 0);
INSERT INTO dead_letter_alerts (request_id) VALUES ('x'), ('y');
SQL
assert_eq 3 "$(osquery_pending_alert_count)" "seeded: pending count"
assert_eq 2 "$(osquery_dead_letter_count)" "seeded: dead-letter count"

printf 'osquery-queue-counts: OK (missing-DB, missing-table, and populated all counted read-only)\n'
