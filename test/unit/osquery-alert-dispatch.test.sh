#!/usr/bin/env bash
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
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit; test/validate-tests.sh pins that shape. A test body runs
# WITHOUT errexit, so every check below is a real assertion: a bare `[[ ]]`
# would report nothing.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"
APOSTROPHE_URL="http://127.0.0.1:8644/webhooks/o'brien-priority"
APOSTROPHE_REASON="operator's note: gateway refused the page"

# The helper is function definitions plus env-defaulted globals, so it is sourced
# ONCE here rather than per test: bashunit runs every test body in its own
# subshell, and set_up below repoints the store before any function is called.
# shellcheck source=dot_local/libexec/osquery/executable_alert-dispatch.sh
source "$DISPATCH"

# Drive every write helper inside ONE strict-bash run, so the escapes execute
# under the version being pinned, and record each step's exit status in a file
# the tests read. Nothing here asserts: a failure must surface as the named
# behavior's red test, not as an unattributable setup error.
set_up_before_script() {
  local strict_bash=/bin/bash
  [[ -x $strict_bash ]] || strict_bash="$(command -v bash)"
  FILE_FIXTURE="$(mktemp -d)"
  QUOTE_WORK="$FILE_FIXTURE/quote-safety"
  mkdir -p "$QUOTE_WORK"
  "$strict_bash" -s "$DISPATCH" "$QUOTE_WORK" <<'STRICT_SCENARIO'
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

tear_down_after_script() { discard_fixture "$FILE_FIXTURE"; }

set_up() {
  TEST_FIXTURE="$(mktemp -d)"
  OSQUERY_UNDELIVERED_ALERTS_DB="$TEST_FIXTURE/undelivered.sqlite3"
}

tear_down() { discard_fixture "$TEST_FIXTURE"; }

# discard_fixture <path>: remove one mktemp -d this file created, and nothing
# else. Plain rm -rf, the convention every other test in this repo uses; the
# suite also runs on a CI host with no Trash.
discard_fixture() {
  [[ -n ${1:-} && -d $1 ]] || return 0
  rm -rf "$1"
}

# step_status <name>: the exit status the strict-bash scenario recorded.
step_status() {
  local recorded
  IFS= read -r recorded <"$QUOTE_WORK/status.$1"
  printf '%s' "$recorded"
}

# quote_query <sql>: read one value out of the scenario's store, read-only.
quote_query() { sqlite3 -readonly "$QUOTE_WORK/store.sqlite3" "$1"; }

# path_exists <path>: a predicate for assert_false, so a negative check fails the
# test on its own rather than relying on an errexit a bashunit body does not set.
path_exists() { [[ -e $1 ]]; }

# --- the read-only queue-health counters -------------------------------------

function test_both_counters_read_zero_before_anything_has_ever_been_stored() {
  assert_same 0 "$(osquery_pending_alert_count)"
  assert_same 0 "$(osquery_dead_letter_count)"
}

function test_a_count_probe_never_creates_the_database_it_reads() {
  osquery_pending_alert_count >/dev/null
  osquery_dead_letter_count >/dev/null
  assert_false path_exists "$OSQUERY_UNDELIVERED_ALERTS_DB"
}

function test_a_counter_reads_zero_while_its_table_is_still_un_bootstrapped_not_an_error() {
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" 'CREATE TABLE unrelated (x);'
  assert_same 0 "$(osquery_pending_alert_count)"
  assert_same 0 "$(osquery_dead_letter_count)"
}

function test_the_counters_report_how_many_pages_are_queued_and_how_many_the_drain_gave_up_on() {
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" <<'SQL'
CREATE TABLE pending_alerts (request_id TEXT, next_attempt_after INTEGER);
CREATE TABLE dead_letter_alerts (request_id TEXT);
INSERT INTO pending_alerts (request_id, next_attempt_after) VALUES ('a', 0), ('b', 0), ('c', 0);
INSERT INTO dead_letter_alerts (request_id) VALUES ('x'), ('y');
SQL
  assert_same 3 "$(osquery_pending_alert_count)"
  assert_same 2 "$(osquery_dead_letter_count)"
}

function test_an_unreadable_store_fails_the_probe_instead_of_reporting_a_false_zero() {
  local pending dead
  printf 'this is not a sqlite database, it is garbage\n' >"$OSQUERY_UNDELIVERED_ALERTS_DB"
  pending="$(osquery_pending_alert_count 2>&1)"
  assert_unsuccessful_code
  assert_empty "$pending"
  dead="$(osquery_dead_letter_count 2>&1)"
  assert_unsuccessful_code
  assert_empty "$dead"
}

# --- quote handling under the strict (macOS system) bash ---------------------

function test_an_apostrophe_in_the_page_url_is_stored_intact_instead_of_being_rejected_by_corrupted_sql() {
  assert_same 0 "$(step_status url-store)"
  assert_same "$APOSTROPHE_URL" \
    "$(quote_query "SELECT url FROM pending_alerts WHERE request_id='osquery-apos-url';")"
}

function test_the_drain_select_carries_an_apostrophe_url_through_to_the_delivery_attempt() {
  local rows
  OSQUERY_UNDELIVERED_ALERTS_DB="$QUOTE_WORK/store.sqlite3"
  rows="$(_osquery_pending_alert_rows)"
  assert_contains "$(printf 'osquery-apos-url\t%s' "$APOSTROPHE_URL")" "$rows"
}

function test_an_apostrophe_in_a_dead_letter_reason_completes_the_move_out_of_the_pending_queue() {
  assert_same 0 "$(step_status reason-store)"
  assert_same 0 "$(step_status reason-deadletter)"
  assert_same "$APOSTROPHE_REASON" \
    "$(quote_query "SELECT reason FROM dead_letter_alerts WHERE request_id='osquery-apos-reason';")"
  assert_same 0 \
    "$(quote_query "SELECT COUNT(*) FROM pending_alerts WHERE request_id='osquery-apos-reason';")"
}

function test_an_apostrophe_request_id_survives_retry_bookkeeping_and_its_delete_by_id() {
  assert_same 0 "$(step_status id-store)"
  assert_same 0 "$(step_status id-transient)"
  assert_same 0 "$(step_status id-delete)"
  assert_same 0 \
    "$(quote_query "SELECT COUNT(*) FROM pending_alerts WHERE request_id='osquery-o''brien';")"
}
