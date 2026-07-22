#!/usr/bin/env bats
# Head-of-line skip-and-continue (DR-B T5): a single poison record must never
# starve the rest of the queue. Whatever a row's fate -- delivered, deferred as
# transient, dead-lettered as permanent or over-threshold, or skipped as
# malformed -- the drain visits EVERY due row in one pass and a row's outcome
# never blocks the rows behind it. The drain is errexit-safe: a failing record
# is logged and the loop continues, never aborting and never swallowing the
# failure silently.

setup() {
  local helpers="$BATS_TEST_DIRNAME/../helpers"
  # shellcheck source=test/helpers/build-dispatch-harness.sh
  source "$helpers/build-dispatch-harness.sh"
  # shellcheck source=test/helpers/wait-for-log-line.sh
  source "$helpers/wait-for-log-line.sh"
  build_dispatch_harness
}
teardown() { teardown_dispatch_harness; }

# Count dead_letter_alerts rows; a store without the table yet counts as zero.
dead_letter_count() {
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    'SELECT COUNT(*) FROM dead_letter_alerts;' 2>/dev/null || echo 0
}

@test "T-DRAIN-continue-past-permanent: a permanent poison row in the middle does not starve the rows behind it" {
  export OSQUERY_DRAIN_MAX_ATTEMPTS=20
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-front "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-poison "$url" "$body_b64"
  _osquery_store_alert_row 3000 osquery-back "$url" "$body_b64"
  : >"$CURL_LOG"
  set_curl_codes 200 403 200 # front delivers, poison is refused, back delivers

  retry_undelivered_alerts

  # The row BEHIND the poison was delivered in the SAME pass.
  grep -qF 'X-Request-ID: osquery-back' "$CURL_LOG"
  # front and back delivered (gone); poison moved to dead_letter.
  assert_pending_alert_count 0
  [[ "$(dead_letter_count)" == "1" ]]
  [[ -n "$(sqlite3_query "SELECT 1 FROM dead_letter_alerts WHERE request_id='osquery-poison';")" ]]
  # Positive anchor: every row was visited (all three POSTed).
  assert_post_count 3
}

@test "T-DRAIN-continue-past-malformed: an undecodable poison row in the middle is skipped and the rows behind it still deliver" {
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' good_body
  good_body=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-a "$url" "$good_body"
  _osquery_store_alert_row 2000 osquery-corrupt "$url" '####' # not decodable base64
  _osquery_store_alert_row 3000 osquery-b "$url" "$good_body"
  : >"$CURL_LOG"
  set_curl_codes 200 200 # only a and b POST; corrupt is skipped before any POST

  retry_undelivered_alerts

  grep -qF 'X-Request-ID: osquery-b' "$CURL_LOG"       # behind the poison, still delivered
  ! grep -qF 'X-Request-ID: osquery-corrupt' "$CURL_LOG" # never POSTed
  assert_pending_alert_count 1                         # corrupt retained, a and b delivered
  [[ "$(sqlite3_query 'SELECT request_id FROM pending_alerts;')" == "osquery-corrupt" ]]
  grep -q 'MALFORMED-ROW' "$OSQUERY_DELIVERY_LOG"      # logged, not silently swallowed
  [[ "$(dead_letter_count)" == "0" ]]                  # a malformed row is retained, not dead-lettered
}

@test "T-DRAIN-mixed-batch-full-drain: a mixed batch drains completely in one pass, each row handled by class, none starved" {
  export OSQUERY_DRAIN_MAX_ATTEMPTS=50
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=3600 # so the transient row defers, not redelivers
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  # Interleave the four classes by occurrence order, a deliverable at each end.
  _osquery_store_alert_row 1000 osquery-deliver-1 "$url" "$body_b64" # 2xx
  _osquery_store_alert_row 2000 osquery-transient "$url" "$body_b64" # 503 -> defer
  _osquery_store_alert_row 3000 osquery-permanent "$url" "$body_b64" # 403 -> dead-letter
  _osquery_store_alert_row 4000 osquery-threshold "$url" "$body_b64" # attempts-maxed -> dead-letter pre-POST
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_alerts SET attempts=99 WHERE request_id='osquery-threshold';"
  _osquery_store_alert_row 5000 osquery-deliver-2 "$url" "$body_b64" # 2xx, LAST (behind every failure)
  : >"$CURL_LOG"
  : >"$ALERTER_LOG"
  # POST order: deliver-1(200), transient(503), permanent(403), [threshold pre-POST skip], deliver-2(200).
  set_curl_codes 200 503 403 200

  retry_undelivered_alerts

  # Both deliverables delivered, including the LAST row sitting behind every failure.
  grep -qF 'X-Request-ID: osquery-deliver-1' "$CURL_LOG"
  grep -qF 'X-Request-ID: osquery-deliver-2' "$CURL_LOG"
  # Each failing row handled by its own class:
  [[ "$(sqlite3_query "SELECT attempts FROM pending_alerts WHERE request_id='osquery-transient';")" == "1" ]]
  [[ -n "$(sqlite3_query "SELECT 1 FROM dead_letter_alerts WHERE request_id='osquery-permanent';")" ]]
  [[ -n "$(sqlite3_query "SELECT 1 FROM dead_letter_alerts WHERE request_id='osquery-threshold';")" ]]
  ! grep -qF 'X-Request-ID: osquery-threshold' "$CURL_LOG" # pre-send give-up, never POSTed
  # Final tallies: only the transient remains pending; two dead-lettered.
  assert_pending_alert_count 1
  [[ "$(sqlite3_query 'SELECT request_id FROM pending_alerts;')" == "osquery-transient" ]]
  [[ "$(dead_letter_count)" == "2" ]]
  # The whole queue was visited: four POSTs (the threshold row alone is pre-POST).
  assert_post_count 4
  # Exactly ONE summary CRIT for the pass (two dead-letters), not one per row.
  wait_for_log_line 'dead-letter' "$ALERTER_LOG"
  [[ "$(grep -ciF 'dead-letter' "$ALERTER_LOG")" == "1" ]]
}

@test "T-DRAIN-errexit-first-row-failure: under set -e a failing FIRST record does not abort the drain; the queue finishes and exit is 0" {
  # The library is sourced into scripts that run under `set -euo pipefail` (the
  # drainer executable). A failing FIRST record must not abort the pass: the
  # per-row delivery runs inside an `if`, so its nonzero return is consumed and
  # the loop keeps going. Runs the drain in a real errexit subshell to prove it.
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-poison-first "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-tail "$url" "$body_b64"
  : >"$CURL_LOG"
  set_curl_codes 403 200 # first row refused (dead-letter), tail delivers

  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]     # the drain did not abort on the first failing record
  [[ $output == *DONE* ]] # ...and ran to completion

  grep -qF 'X-Request-ID: osquery-tail' "$CURL_LOG" # the tail behind the poison delivered
  assert_pending_alert_count 0
  [[ "$(dead_letter_count)" == "1" ]] # the first record was dead-lettered, not retried forever
}
