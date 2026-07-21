#!/usr/bin/env bats
# Delivery durability: a CRIT page that cannot be delivered must be STORED as an
# undelivered alert, never silently dropped (a lost page looks exactly like
# all-clear). The retry drain replays a stored alert idempotently, to localhost
# only, and never aborts its caller.

setup() {
  local helpers="$BATS_TEST_DIRNAME/../helpers"
  # shellcheck source=test/helpers/build-dispatch-harness.sh
  source "$helpers/build-dispatch-harness.sh"
  # shellcheck source=test/helpers/run-dispatch-and-drain.sh
  source "$helpers/run-dispatch-and-drain.sh"
  build_dispatch_harness
}
teardown() { teardown_dispatch_harness; }

@test "T-SEC-undelivered-retry: a page failing all retries is stored (600/700), then drained" {
  set_curl_codes 503 503 503
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi"
  [[ $send_status -eq 0 ]] # a down gateway never fails the caller
  assert_undelivered_alert_count 1 # stored, not lost
  assert_mode 700 "$OSQUERY_UNDELIVERED_ALERTS_DIR"
  assert_mode 600 "$(first_undelivered_alert_file)"
  grep -q STORED "$OSQUERY_DELIVERY_LOG"
  # gateway recovers: the drain delivers and clears the stored alert
  run_retry_undelivered_alerts drain_output drain_status
  assert_undelivered_alert_count 0
}

@test "T-SEC-undelivered-idem: re-storing the SAME occurrence stays one file; drain replays once (R2-4)" {
  # Idempotency is keyed on OCCURRENCE IDENTITY, not body content (R2-4): the SAME
  # occurrence re-stored stays one file and reuses one request_id, so the gateway
  # dedups a retry. (Two DISTINCT occurrences that render the same body get two
  # files, the collapse bug this design fixes.)
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 title" "same detail" "Sosumi" "occ:disk3:900:1000"
  send_alert CRIT "🔴 title" "same detail" "Sosumi" "occ:disk3:900:1000" # same occurrence, same request_id
  assert_undelivered_alert_count 1
  local stored_request_id
  stored_request_id=$(cut -f2 "$(first_undelivered_alert_file)")
  : >"$CURL_LOG"
  retry_undelivered_alerts
  assert_post_count 1
  grep -qF "X-Request-ID: $stored_request_id" "$CURL_LOG" # the stored request_id is reused verbatim
  assert_undelivered_alert_count 0                        # so the gateway dedups instead of double-posting
  : >"$CURL_LOG"
  retry_undelivered_alerts # nothing left
  assert_post_count 0
}

@test "T-SEC-localhost: send and drain POST only to 127.0.0.1; a tampered url is not sent" {
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  assert_post_count 3 # positive anchor: the three retries actually POSTed
  assert_posted_to '127.0.0.1:8644'
  run grep -v '127.0.0.1:8644' "$CURL_LOG"
  [[ -z $output ]] # ...and every send POST was to loopback, nothing else
  local stored_file
  stored_file=$(first_undelivered_alert_file)
  awk 'BEGIN{FS=OFS="\t"} {$3="http://10.0.0.5:8644/webhooks/osquery-priority"; print}' \
    "$stored_file" >"$stored_file.t" && mv "$stored_file.t" "$stored_file"
  : >"$CURL_LOG"
  retry_undelivered_alerts
  ! grep -q '10.0.0.5' "$CURL_LOG"     # never sent off-box
  assert_undelivered_alert_count 1 # the off-box entry is RETAINED (skipped), not silently dropped
}

@test "T-SEC-drain-setE: drain is set -e-safe on empty or malformed input" {
  mkdir -p "$OSQUERY_UNDELIVERED_ALERTS_DIR"
  printf 'garbage-no-tabs\n' >"$OSQUERY_UNDELIVERED_ALERTS_DIR/bad"
  printf '%s\t%s\n' 123 incomplete >"$OSQUERY_UNDELIVERED_ALERTS_DIR/short"
  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]
  [[ $output == *DONE* ]]
}

@test "T-SEC-write-ahead: the record is persisted BEFORE the first send attempt" {
  # The curl stub records the stored-record count seen at each POST. Under the
  # write-ahead store the FIRST POST must already see the record on disk (count
  # >= 1); the old attempt-then-persist path showed 0 at the first POST, so a
  # crash between the attempt and success would have lost the page.
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail body" "Sosumi" "occ:wa:1"
  local first_witnessed_count
  first_witnessed_count=$(head -1 "$CURL_PERSIST_WITNESS")
  [[ $first_witnessed_count =~ ^[0-9]+$ ]]
  [[ $first_witnessed_count -ge 1 ]]
}

@test "T-SEC-sqlite-write-ahead: a failing CRIT lands as a pending_alerts row (schema, WAL, 600/700) BEFORE the first curl" {
  # DR-A moves the undelivered-alerts store into SQLite. A CRIT whose send fails
  # must land as a ROW in the pending_alerts table, write-ahead: the row is
  # present when the FIRST POST fires, so a crash between persist and success
  # leaves a recoverable record. This pins the schema, the WAL journal mode, and
  # the 600-file/700-parent permission bits the security invariants require.
  set_curl_codes 503 503 503
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi" "occ:sqlite:1"
  [[ $send_status -eq 0 ]] # a down gateway never fails the caller

  assert_pending_alert_count 1 # stored as a row, not lost

  # The schema carries every column DR-B will consume. sequence_number is the
  # AUTOINCREMENT primary key (assigned atomically inside the insert, race-free
  # under concurrent producers); request_id keeps uniqueness through a UNIQUE
  # constraint, the ON CONFLICT target for idempotent re-stores.
  local columns column
  columns=$(sqlite3_query "SELECT group_concat(name, ',') FROM pragma_table_info('pending_alerts');")
  for column in request_id sequence_number occurrence_ts url body_base64 attempts next_attempt_after created_at; do
    if [[ ",$columns," != *",$column,"* ]]; then
      printf 'schema is missing column %s (got: %s)\n' "$column" "$columns" >&2
      return 1
    fi
  done
  local primary_key
  primary_key=$(sqlite3_query "SELECT name FROM pragma_table_info('pending_alerts') WHERE pk=1;")
  [[ $primary_key == "sequence_number" ]]
  local unique_column
  unique_column=$(sqlite3_query "SELECT ii.name FROM pragma_index_list('pending_alerts') il JOIN pragma_index_info(il.name) ii WHERE il.origin='u';")
  [[ $unique_column == "request_id" ]]

  # The connection runs in WAL journal mode (crash-atomic commits).
  local journal_mode
  journal_mode=$(sqlite3_query "PRAGMA journal_mode;")
  [[ $journal_mode == "wal" ]]

  # The DB file is mode 600 inside a mode-700 parent, as the file store was.
  assert_mode 600 "$OSQUERY_UNDELIVERED_ALERTS_DB"
  assert_mode 700 "$(dirname "$OSQUERY_UNDELIVERED_ALERTS_DB")"

  # Write-ahead: the row was already present when the FIRST POST was attempted.
  local first_witnessed_count
  first_witnessed_count=$(head -1 "$CURL_DB_PERSIST_WITNESS")
  [[ $first_witnessed_count =~ ^[0-9]+$ ]]
  [[ $first_witnessed_count -ge 1 ]]
}

@test "T-SEC-sqlite-delete-send: a confirmed 2xx on the SEND path deletes the pending_alerts row" {
  # Delete only after a confirmed 2xx: a page delivered on the first attempt
  # must leave ZERO rows behind, or the store grows a leaked row per delivered
  # page and a later drain re-posts already-delivered pages forever.
  set_curl_codes 200
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi" "occ:del-send:1"
  [[ $send_status -eq 0 ]]
  assert_post_count 1 # the 2xx really happened (positive anchor)
  assert_pending_alert_count 0
}

@test "T-SEC-sqlite-delete-drain: a failed delivery RETAINS the row; the drain's 2xx deletes it" {
  # Failure retains: the row must survive every failed attempt (it is the only
  # durable copy of the page). Recovery deletes: once the drain gets its 2xx the
  # row is gone, so the next drain has nothing to re-post.
  set_curl_codes 503 503 503
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi" "occ:del-drain:1"
  [[ $send_status -eq 0 ]]
  assert_pending_alert_count 1 # retained across the failed attempts
  set_curl_codes 200
  run_retry_undelivered_alerts drain_output drain_status
  [[ $drain_status -eq 0 ]]
  assert_pending_alert_count 0 # deleted only after the drain's confirmed 2xx
}

@test "T-SEC-quarantine-partial: a crashed *.tmp.* partial is quarantined on the next drain, not skipped forever" {
  mkdir -p "$OSQUERY_UNDELIVERED_ALERTS_DIR"
  # A leftover temp from a dead writer (a pid that is not running).
  printf '%s\tosquery-crash\thttp://127.0.0.1:8644/x\tYm9keQ==\n' "$(date -u +%s)" \
    >"$OSQUERY_UNDELIVERED_ALERTS_DIR/osquery-crash.tmp.999999"
  retry_undelivered_alerts
  # No *.tmp.* remains in the store dir (it is not skipped and left forever)...
  [[ -z "$(find "$OSQUERY_UNDELIVERED_ALERTS_DIR" -maxdepth 1 -name '*.tmp.*' 2>/dev/null)" ]]
  # ...it was moved into the quarantine sibling, not silently deleted.
  [[ -n "$(find "${OSQUERY_UNDELIVERED_ALERTS_DIR}.quarantine" -type f 2>/dev/null)" ]]
}

@test "T-SEC-drain-order: the drain delivers in occurrence-time order, not filename order" {
  mkdir -p "$OSQUERY_UNDELIVERED_ALERTS_DIR"
  # Two records: z-old has the EARLIER occurrence ts, a-new the later. By filename
  # glob order a-new sorts first; by occurrence time z-old must deliver first.
  local body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  printf '%s\t%s\t%s\t%s\n' 1000 osquery-z-old 'http://127.0.0.1:8644/webhooks/osquery-priority' "$body_b64" \
    >"$OSQUERY_UNDELIVERED_ALERTS_DIR/osquery-z-old"
  printf '%s\t%s\t%s\t%s\n' 2000 osquery-a-new 'http://127.0.0.1:8644/webhooks/osquery-priority' "$body_b64" \
    >"$OSQUERY_UNDELIVERED_ALERTS_DIR/osquery-a-new"
  set_curl_codes 200 200
  retry_undelivered_alerts
  local first_posted_request_id
  first_posted_request_id=$(grep -oE 'X-Request-ID: osquery-[a-z-]+' "$CURL_LOG" | head -1)
  [[ $first_posted_request_id == "X-Request-ID: osquery-z-old" ]]
}

@test "T-SEC-no-secret-log: the webhook secret never appears in any log or stored file" {
  export OSQUERY_WEBHOOK_SECRET="SUPERSECRET123"
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  retry_undelivered_alerts || true
  ! grep -rqF "SUPERSECRET123" \
    "$HARNESS_HOME/.local" "$OSQUERY_UNDELIVERED_ALERTS_DIR" "$CURL_LOG" "$OSQUERY_DELIVERY_LOG" 2>/dev/null
}
