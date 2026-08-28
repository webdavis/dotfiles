#!/usr/bin/env bats
# The undelivered-alerts store inside alert-dispatch.sh: the two read-only
# queue-health counters the watchdog polls, and the quote handling every write
# helper depends on.
#
# The counters must be robust to a store that does not exist yet (a health probe
# reports zero before anything has been stored, and never creates the database it
# reads) and must FAIL SAFE on a store that exists but cannot be read, because a
# zero there would hide a real backlog behind an all-clear.
#
# The quote handling is bash-version-dependent and was verified both ways: the
# old escape shape (an unquoted \' inside a double-quoted expansion) doubles
# correctly under bash 5 and corrupts the SQL under bash 3.2, the macOS system
# bash, where a URL carrying an apostrophe was loudly REJECTED by the store. The
# round-trip scenario therefore runs under /bin/bash (3.2 on macOS), once per
# file, recording each helper's exit status for the tests to assert.

REPO_ROOT="${BATS_TEST_DIRNAME%/test/unit}"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"
APOSTROPHE_URL="http://127.0.0.1:8644/webhooks/o'brien-priority"
APOSTROPHE_REASON="operator's note: gateway refused the page"

# Drive every write helper inside ONE strict-bash run, so the escapes execute
# under the version being pinned, and record each step's exit status in a file
# the tests read. Nothing here asserts: a failure must surface as the named
# behavior's red test, not as an unattributable setup error.
setup_file() {
  local strict_bash=/bin/bash
  [[ -x $strict_bash ]] || strict_bash="$(command -v bash)"
  mkdir -p "$BATS_FILE_TMPDIR/quote-safety"
  "$strict_bash" -s "$DISPATCH" "$BATS_FILE_TMPDIR/quote-safety" <<'STRICT_SCENARIO'
set -uo pipefail
source "$1"
work="$2"
export OSQUERY_UNDELIVERED_ALERTS_DB="$work/store.sqlite3"
export OSQUERY_DELIVERY_LOG="$work/delivery.log"
body_b64="$(printf '%s' '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')"

# An apostrophe URL inside the allowed localhost prefix.
_osquery_store_alert_row 1000 osquery-apos-url \
  "http://127.0.0.1:8644/webhooks/o'brien-priority" "$body_b64"
printf '%s' "$?" >"$work/status.url-store"

# An apostrophe dead-letter reason on an otherwise ordinary row.
_osquery_store_alert_row 2000 osquery-apos-reason \
  'http://127.0.0.1:8644/webhooks/priority' "$body_b64"
printf '%s' "$?" >"$work/status.reason-store"
_osquery_dead_letter_alert_row osquery-apos-reason none "operator's note: gateway refused the page"
printf '%s' "$?" >"$work/status.reason-deadletter"

# An apostrophe request id through store, retry bookkeeping and delete-by-id.
_osquery_store_alert_row 3000 "osquery-o'brien" \
  'http://127.0.0.1:8644/webhooks/priority' "$body_b64"
printf '%s' "$?" >"$work/status.id-store"
_osquery_record_transient_failure "osquery-o'brien"
printf '%s' "$?" >"$work/status.id-transient"
_osquery_delete_alert_row "osquery-o'brien"
printf '%s' "$?" >"$work/status.id-delete"
STRICT_SCENARIO
}

setup() {
  source "$DISPATCH"
  QUOTE_WORK="$BATS_FILE_TMPDIR/quote-safety"
  OSQUERY_UNDELIVERED_ALERTS_DB="$BATS_TEST_TMPDIR/undelivered.sqlite3"
}

# step_status <name>: the exit status the strict-bash scenario recorded.
step_status() {
  local recorded
  IFS= read -r recorded <"$QUOTE_WORK/status.$1"
  printf '%s' "$recorded"
}

# quote_query <sql>: read one value out of the scenario's store, read-only.
quote_query() { sqlite3 -readonly "$QUOTE_WORK/store.sqlite3" "$1"; }

# --- the read-only queue-health counters -------------------------------------

@test "both counters read zero before anything has ever been stored" {
  [[ "$(osquery_pending_alert_count)" == 0 ]]
  [[ "$(osquery_dead_letter_count)" == 0 ]]
}

@test "a count probe never creates the database it reads" {
  osquery_pending_alert_count >/dev/null
  osquery_dead_letter_count >/dev/null
  [[ ! -e $OSQUERY_UNDELIVERED_ALERTS_DB ]]
}

@test "a counter reads zero while its table is still un-bootstrapped, not an error" {
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" 'CREATE TABLE unrelated (x);'
  [[ "$(osquery_pending_alert_count)" == 0 ]]
  [[ "$(osquery_dead_letter_count)" == 0 ]]
}

@test "the counters report how many pages are queued and how many the drain gave up on" {
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" <<'SQL'
CREATE TABLE pending_alerts (request_id TEXT, next_attempt_after INTEGER);
CREATE TABLE dead_letter_alerts (request_id TEXT);
INSERT INTO pending_alerts (request_id, next_attempt_after) VALUES ('a', 0), ('b', 0), ('c', 0);
INSERT INTO dead_letter_alerts (request_id) VALUES ('x'), ('y');
SQL
  [[ "$(osquery_pending_alert_count)" == 3 ]]
  [[ "$(osquery_dead_letter_count)" == 2 ]]
}

@test "an unreadable store fails the probe instead of reporting a false zero" {
  printf 'this is not a sqlite database, it is garbage\n' >"$OSQUERY_UNDELIVERED_ALERTS_DB"
  run osquery_pending_alert_count
  [[ $status -ne 0 ]]
  [[ -z $output ]]
  run osquery_dead_letter_count
  [[ $status -ne 0 ]]
  [[ -z $output ]]
}

# --- quote handling under the strict (macOS system) bash ---------------------

@test "an apostrophe in the page URL is stored intact instead of being rejected by corrupted SQL" {
  [[ "$(step_status url-store)" == 0 ]]
  [[ "$(quote_query "SELECT url FROM pending_alerts WHERE request_id='osquery-apos-url';")" == "$APOSTROPHE_URL" ]]
}

@test "the drain SELECT carries an apostrophe URL through to the delivery attempt" {
  local rows
  OSQUERY_UNDELIVERED_ALERTS_DB="$QUOTE_WORK/store.sqlite3"
  rows="$(_osquery_pending_alert_rows)"
  [[ $rows == *"osquery-apos-url	$APOSTROPHE_URL"* ]]
}

@test "an apostrophe in a dead-letter reason completes the move out of the pending queue" {
  [[ "$(step_status reason-store)" == 0 ]]
  [[ "$(step_status reason-deadletter)" == 0 ]]
  [[ "$(quote_query "SELECT reason FROM dead_letter_alerts WHERE request_id='osquery-apos-reason';")" == "$APOSTROPHE_REASON" ]]
  [[ "$(quote_query "SELECT COUNT(*) FROM pending_alerts WHERE request_id='osquery-apos-reason';")" == 0 ]]
}

@test "an apostrophe request id survives retry bookkeeping and its delete-by-id" {
  [[ "$(step_status id-store)" == 0 ]]
  [[ "$(step_status id-transient)" == 0 ]]
  [[ "$(step_status id-delete)" == 0 ]]
  [[ "$(quote_query "SELECT COUNT(*) FROM pending_alerts WHERE request_id='osquery-o''brien';")" == 0 ]]
}
