#!/usr/bin/env bash
# Head-of-line skip-and-continue (DR-B T5): a single poison record must never
# starve the rest of the queue. Whatever a row's fate -- delivered, deferred as
# transient, dead-lettered as permanent or over-threshold, or skipped as
# malformed -- the drain visits EVERY due row in one pass and a row's outcome
# never blocks the rows behind it. The drain is errexit-safe: a failing record
# is logged and the loop continues, never aborting and never swallowing the
# failure silently.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The shebang stays for shellcheck
# and for editors, and is never executed. test/validate-tests.sh pins that shape;
# `just test-integration` runs it.
#
# Every check below is a real bashunit assertion rather than a bare command.
# bashunit runs each test function under `set +euo pipefail`, so a bare
# `grep -q ...` (and every helper that merely `return 1`s, such as the harness's
# own assert_pending_alert_count) reports nothing and passes silently. The row
# counts therefore read through plain count functions and assert_same, and the
# negative checks use assert_file_not_contains, which is the same `grep -F -q`
# the bats file's explicit `if ... then false; fi` blocks ran.

_suite_helpers_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")/../helpers" && pwd)"
# shellcheck source=test/helpers/build-dispatch-harness.sh
source "$_suite_helpers_directory/build-dispatch-harness.sh"
# shellcheck source=test/helpers/wait-for-log-line.sh
source "$_suite_helpers_directory/wait-for-log-line.sh"

function set_up() { build_dispatch_harness; }
function tear_down() { teardown_dispatch_harness; }

# Count dead_letter_alerts rows; a store without the table yet counts as zero.
dead_letter_count() {
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    'SELECT COUNT(*) FROM dead_letter_alerts;' 2>/dev/null || echo 0
}

# Count pending_alerts rows; an absent store counts as zero (nothing stored yet).
pending_alert_count() {
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    'SELECT COUNT(*) FROM pending_alerts;' 2>/dev/null || echo 0
}

# Count the POSTs the curl stub recorded, so a test can prove every due row was
# visited (or that one specific row was never sent).
post_count() {
  grep -c 'POST' "$CURL_LOG" 2>/dev/null || printf '0'
}

# T-DRAIN-continue-past-permanent
function test_a_permanent_poison_row_in_the_middle_does_not_starve_the_rows_behind_it() {
  export OSQUERY_DRAIN_MAX_ATTEMPTS=20
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800
  local url='http://127.0.0.1:8644/webhooks/priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-front "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-poison "$url" "$body_b64"
  _osquery_store_alert_row 3000 osquery-back "$url" "$body_b64"
  : >"$CURL_LOG"
  set_curl_codes 200 403 200 # front delivers, poison is refused, back delivers

  retry_undelivered_alerts

  # The row BEHIND the poison was delivered in the SAME pass.
  assert_file_contains "$CURL_LOG" 'X-Request-ID: osquery-back'
  # front and back delivered (gone); poison moved to dead_letter.
  assert_same 0 "$(pending_alert_count)"
  assert_same 1 "$(dead_letter_count)"
  assert_not_empty "$(sqlite3_query "SELECT 1 FROM dead_letter_alerts WHERE request_id='osquery-poison';")"
  # Positive anchor: every row was visited (all three POSTed).
  assert_same 3 "$(post_count)"
}

# T-DRAIN-continue-past-malformed
function test_an_undecodable_poison_row_in_the_middle_is_skipped_and_the_rows_behind_it_still_deliver() {
  local url='http://127.0.0.1:8644/webhooks/priority' good_body
  good_body=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-a "$url" "$good_body"
  _osquery_store_alert_row 2000 osquery-corrupt "$url" '####' # not decodable base64
  _osquery_store_alert_row 3000 osquery-b "$url" "$good_body"
  : >"$CURL_LOG"
  set_curl_codes 200 200 # only a and b POST; corrupt is skipped before any POST

  retry_undelivered_alerts

  # Behind the poison, still delivered.
  assert_file_contains "$CURL_LOG" 'X-Request-ID: osquery-b'
  # The corrupt row was never POSTed at all.
  assert_file_not_contains "$CURL_LOG" 'X-Request-ID: osquery-corrupt'
  assert_same 1 "$(pending_alert_count)" # corrupt retained, a and b delivered
  assert_same osquery-corrupt "$(sqlite3_query 'SELECT request_id FROM pending_alerts;')"
  # Logged, not silently swallowed.
  assert_file_contains "$OSQUERY_DELIVERY_LOG" 'MALFORMED-ROW'
  assert_same 0 "$(dead_letter_count)" # a malformed row is retained, not dead-lettered
}

# T-DRAIN-mixed-batch-full-drain
function test_a_mixed_batch_drains_completely_in_one_pass_each_row_handled_by_class_none_starved() {
  export OSQUERY_DRAIN_MAX_ATTEMPTS=50
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=3600 # so the transient row defers, not redelivers
  local url='http://127.0.0.1:8644/webhooks/priority' body_b64
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
  assert_file_contains "$CURL_LOG" 'X-Request-ID: osquery-deliver-1'
  assert_file_contains "$CURL_LOG" 'X-Request-ID: osquery-deliver-2'
  # Each failing row handled by its own class:
  assert_same 1 "$(sqlite3_query "SELECT attempts FROM pending_alerts WHERE request_id='osquery-transient';")"
  assert_not_empty "$(sqlite3_query "SELECT 1 FROM dead_letter_alerts WHERE request_id='osquery-permanent';")"
  assert_not_empty "$(sqlite3_query "SELECT 1 FROM dead_letter_alerts WHERE request_id='osquery-threshold';")"
  # Pre-send give-up: the threshold row was never POSTed.
  assert_file_not_contains "$CURL_LOG" 'X-Request-ID: osquery-threshold'
  # Final tallies: only the transient remains pending; two dead-lettered.
  assert_same 1 "$(pending_alert_count)"
  assert_same osquery-transient "$(sqlite3_query 'SELECT request_id FROM pending_alerts;')"
  assert_same 2 "$(dead_letter_count)"
  # The whole queue was visited: four POSTs (the threshold row alone is pre-POST).
  assert_same 4 "$(post_count)"
  # Exactly ONE summary CRIT for the pass (two dead-letters), not one per row. The
  # wait is a polling predicate, so its status is asserted rather than discarded:
  # a genuinely absent line must fail the test, not pass under `set +e`.
  wait_for_log_line 'dead-letter' "$ALERTER_LOG"
  assert_successful_code
  assert_same 1 "$(grep -ciF 'dead-letter' "$ALERTER_LOG")"
}

# T-DRAIN-errexit-first-row-failure
function test_under_errexit_a_failing_first_record_does_not_abort_the_drain_and_exit_is_zero() {
  # The library is sourced into scripts that run under `set -euo pipefail` (the
  # drainer executable). A failing FIRST record must not abort the pass: the
  # per-row delivery runs inside an `if`, so its nonzero return is consumed and
  # the loop keeps going. Runs the drain in a real errexit subshell to prove it.
  local url='http://127.0.0.1:8644/webhooks/priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-poison-first "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-tail "$url" "$body_b64"
  : >"$CURL_LOG"
  set_curl_codes 403 200 # first row refused (dead-letter), tail delivers

  # bashunit has no `run`, so the status and the merged output are captured by
  # hand, the way bats' `run` combined them.
  local output status=0
  output="$(bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE" 2>&1)" || status=$?
  assert_exit_code 0 "" "$status" # the drain did not abort on the first failing record
  assert_contains DONE "$output"  # ...and ran to completion

  # The tail behind the poison delivered.
  assert_file_contains "$CURL_LOG" 'X-Request-ID: osquery-tail'
  assert_same 0 "$(pending_alert_count)"
  assert_same 1 "$(dead_letter_count)" # the first record was dead-lettered, not retried forever
}
