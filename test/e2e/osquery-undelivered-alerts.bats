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

@test "T-SEC-no-secret-log: the webhook secret never appears in any log or stored file" {
  export OSQUERY_WEBHOOK_SECRET="SUPERSECRET123"
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  retry_undelivered_alerts || true
  ! grep -rqF "SUPERSECRET123" \
    "$HARNESS_HOME/.local" "$OSQUERY_UNDELIVERED_ALERTS_DIR" "$CURL_LOG" "$OSQUERY_DELIVERY_LOG" 2>/dev/null
}
