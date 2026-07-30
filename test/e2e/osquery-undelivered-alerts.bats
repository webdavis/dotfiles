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
  assert_pending_alert_count 1 # stored, not lost
  assert_mode 700 "$(dirname "$OSQUERY_UNDELIVERED_ALERTS_DB")"
  assert_mode 600 "$OSQUERY_UNDELIVERED_ALERTS_DB"
  grep -q STORED "$OSQUERY_DELIVERY_LOG"
  # gateway recovers: the drain delivers and clears the stored alert
  run_retry_undelivered_alerts drain_output drain_status
  assert_pending_alert_count 0
}

@test "T-SEC-undelivered-idem: re-storing the SAME occurrence stays one row; drain replays once (R2-4)" {
  # Idempotency is keyed on OCCURRENCE IDENTITY, not body content (R2-4): the SAME
  # occurrence re-stored stays one row and reuses one request_id, so the gateway
  # dedups a retry. (Two DISTINCT occurrences that render the same body get two
  # rows, the collapse bug this design fixes.)
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 title" "same detail" "Sosumi" "occ:disk3:900:1000"
  send_alert CRIT "🔴 title" "same detail" "Sosumi" "occ:disk3:900:1000" # same occurrence, same request_id
  assert_pending_alert_count 1
  local stored_request_id
  stored_request_id=$(sqlite3_query 'SELECT request_id FROM pending_alerts;')
  : >"$CURL_LOG"
  retry_undelivered_alerts
  assert_post_count 1
  grep -qF "X-Request-ID: $stored_request_id" "$CURL_LOG" # the stored request_id is reused verbatim
  assert_pending_alert_count 0                            # so the gateway dedups instead of double-posting
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
  # Tamper the AUTHORITATIVE store (the pending_alerts row) with an off-box url.
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_alerts SET url='http://10.0.0.5:8644/webhooks/osquery-priority';"
  : >"$CURL_LOG"
  retry_undelivered_alerts
  ! grep -q '10.0.0.5' "$CURL_LOG" # never sent off-box
  assert_pending_alert_count 1     # the off-box entry is RETAINED (skipped), not silently dropped
}

@test "T-SEC-drain-setE: drain is set -e-safe on an absent, corrupt, or malformed store" {
  # An absent database (nothing ever stored) is a quiet no-op.
  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]
  [[ $output == *DONE* ]]
  # A corrupt database file (not SQLite at all) must not abort the caller.
  mkdir -p "$(dirname "$OSQUERY_UNDELIVERED_ALERTS_DB")"
  printf 'this is not a sqlite database\n' >"$OSQUERY_UNDELIVERED_ALERTS_DB"
  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]
  [[ $output == *DONE* ]]
  # A malformed row (a body that is not decodable base64) is skipped, and the
  # drain continues to completion.
  rm -f "$OSQUERY_UNDELIVERED_ALERTS_DB"
  _osquery_store_alert_row 1000 osquery-malformed 'http://127.0.0.1:8644/webhooks/osquery-priority' '%%%not-base64%%%'
  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]
  [[ $output == *DONE* ]]
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

@test "T-SEC-sqlite-drain-order: rows drain by occurrence_ts then sequence_number, not insert or name order" {
  # The drain must read the SQLite store and order by occurrence time with the
  # insert-assigned sequence_number as the tiebreaker. Seeding contradicts every
  # wrong ordering: the OLDER occurrence is inserted LATER (a backward clock
  # step), and the equal-timestamp pair is named so lexical order (aa before zz)
  # contradicts insert order (zz first). Only ORDER BY occurrence_ts,
  # sequence_number yields the expected delivery sequence.
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 2000 osquery-inserted-first-newer "$url" "$body_b64"
  _osquery_store_alert_row 1000 osquery-inserted-second-older "$url" "$body_b64" # clock stepped back
  _osquery_store_alert_row 3000 osquery-zz-tie "$url" "$body_b64"                # equal ts, lower sequence
  _osquery_store_alert_row 3000 osquery-aa-tie "$url" "$body_b64"                # equal ts, higher sequence
  set_curl_codes 200 200 200 200
  retry_undelivered_alerts
  local posted_order
  posted_order=$(grep -oE 'X-Request-ID: osquery-[a-z-]+' "$CURL_LOG" | sed 's/X-Request-ID: //' | paste -sd, -)
  [[ $posted_order == "osquery-inserted-second-older,osquery-inserted-first-newer,osquery-zz-tie,osquery-aa-tie" ]]
  assert_pending_alert_count 0 # every delivered row was deleted
}

@test "T-SEC-malformed-row-diagnostic: a row the drain cannot decode is skipped LOUDLY, later rows still drain (F3)" {
  # A hand-corrupted row (a body that is not decodable base64) must never be
  # skipped in silence: a silently stuck row is invisible forever. The drain
  # logs a MALFORMED-ROW line naming the request_id and keeps draining the
  # rows after it.
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-corrupt "$url" '####'
  _osquery_store_alert_row 2000 osquery-good "$url" "$body_b64"
  set_curl_codes 200 200
  retry_undelivered_alerts
  grep -q 'MALFORMED-ROW' "$OSQUERY_DELIVERY_LOG"     # loud, never silent
  grep -q 'osquery-corrupt' "$OSQUERY_DELIVERY_LOG"   # names the stuck row
  grep -qF 'X-Request-ID: osquery-good' "$CURL_LOG"   # the drain continued past it
  assert_pending_alert_count 1                        # corrupt retained, good delivered
}

@test "T-SEC-atomic-persist: a kill during persist never leaves a bootstrapped store missing the alert (F1)" {
  # The persist must be atomic end-to-end: the schema bootstrap and this alert's
  # INSERT commit together, so a kill at ANY instant leaves either no schema at
  # all (the persist never completed and send_alert never reported success) or
  # the row fully present. The forbidden half-state is a bootstrapped table
  # without the alert: that page would be gone with no diagnostic. The killer
  # fires the moment the schema becomes visible; a non-atomic persist (schema
  # committed first, row in a second sqlite3 invocation) gets caught in the gap.
  cat >"$HARNESS_HOME/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
printf '503'
STUB
  chmod +x "$HARNESS_HOME/bin/curl"
  local iteration send_pid schema_present
  for iteration in 1 2 3 4 5 6 7 8 9 10; do
    rm -f "$OSQUERY_UNDELIVERED_ALERTS_DB" "$OSQUERY_UNDELIVERED_ALERTS_DB"-*
    (send_alert CRIT "🔴 kill $iteration" "detail" "Sosumi" "occ:kill:$iteration" >/dev/null 2>&1) &
    send_pid=$!
    while kill -0 "$send_pid" 2>/dev/null; do
      schema_present="$(sqlite3_query "SELECT count(*) FROM sqlite_master WHERE name='pending_alerts';" 2>/dev/null)" || schema_present=""
      if [[ $schema_present == "1" ]]; then
        kill -KILL "$send_pid" 2>/dev/null || true
        break
      fi
    done
    wait "$send_pid" 2>/dev/null || true
    sleep 0.2 # let any in-flight sqlite3 child of the killed sender settle
    schema_present="$(sqlite3_query "SELECT count(*) FROM sqlite_master WHERE name='pending_alerts';" 2>/dev/null)" || schema_present=""
    if [[ $schema_present == "1" ]]; then
      assert_pending_alert_count 1 # a visible schema MUST mean the row committed with it
    fi
  done
}

@test "T-SEC-no-secret-log: the webhook secret never appears in any log or the stored database" {
  export OSQUERY_WEBHOOK_SECRET="SUPERSECRET123"
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  retry_undelivered_alerts || true
  ! grep -rqF "SUPERSECRET123" \
    "$HARNESS_HOME/.local" "$OSQUERY_UNDELIVERED_ALERTS_DB" "$CURL_LOG" "$OSQUERY_DELIVERY_LOG" 2>/dev/null
}
