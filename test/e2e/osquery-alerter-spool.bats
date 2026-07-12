#!/usr/bin/env bats
# Delivery durability (H2): a page that can't be delivered must be SPOOLED, never
# silently dropped — a lost page looks exactly like "all clear". The drain replays
# it idempotently, to localhost only, and never aborts its caller.

load ../fixtures/osquery-alerter-lib

setup() { setup_dispatch_harness; }
teardown() { teardown_harness; }

@test "T-SEC-spool-retry: a page failing all retries is spooled (600/700), then drained" {
  set_curl_codes 503 503 503
  run send_alert CRIT "🔴 title" "detail body" "Sosumi"
  [ "$status" -eq 0 ] # a down gateway never fails the caller
  assert_spool_count 1 # spooled, not lost
  assert_mode 700 "$OSQUERY_SPOOL_DIR"
  assert_mode 600 "$(find "$OSQUERY_SPOOL_DIR" -type f | head -1)"
  grep -q SPOOLED "$OSQUERY_DELIVERY_LOG"
  # gateway recovers: drain delivers and clears the spool
  run_drain
  assert_spool_count 0
}

@test "T-SEC-spool-idem: re-spooling the same page stays one file; drain replays once" {
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 title" "same detail" "Sosumi"
  send_alert CRIT "🔴 title" "same detail" "Sosumi" # identical body → same request_id
  assert_spool_count 1
  local stored_rid
  stored_rid=$(cut -f2 "$(find "$OSQUERY_SPOOL_DIR" -type f | head -1)")
  : >"$CURL_LOG"
  run_drain
  assert_post_count 1
  grep -qF "X-Request-ID: $stored_rid" "$CURL_LOG" # the stored request_id is reused verbatim
  assert_spool_count 0                             # so the gateway dedups instead of double-posting
  : >"$CURL_LOG"
  run_drain # nothing left
  assert_post_count 0
}

@test "T-SEC-localhost: send and drain POST only to 127.0.0.1; a tampered url is not sent" {
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  assert_post_count 3              # positive anchor: the three retries actually POSTed
  assert_posted_to '127.0.0.1:8644'
  run grep -v '127.0.0.1:8644' "$CURL_LOG"
  [ -z "$output" ] # ...and every send POST was to loopback, nothing else
  local spool_file
  spool_file=$(find "$OSQUERY_SPOOL_DIR" -type f | head -1)
  awk 'BEGIN{FS=OFS="\t"} {$3="http://10.0.0.5:8644/webhooks/osquery-priority"; print}' \
    "$spool_file" >"$spool_file.t" && mv "$spool_file.t" "$spool_file"
  : >"$CURL_LOG"
  run_drain
  ! grep -q '10.0.0.5' "$CURL_LOG" # never sent off-box
  assert_spool_count 1 # the off-box entry is RETAINED (skipped), not silently dropped
}

@test "T-SEC-drain-setE: drain is set -e-safe on empty/malformed spool" {
  mkdir -p "$OSQUERY_SPOOL_DIR"
  printf 'garbage-no-tabs\n' >"$OSQUERY_SPOOL_DIR/bad"
  printf '%s\t%s\n' 123 incomplete >"$OSQUERY_SPOOL_DIR/short"
  run bash -c "set -euo pipefail; source '$DISPATCH'; _drain_spool; echo DONE"
  [ "$status" -eq 0 ]
  [[ "$output" == *DONE* ]]
}

@test "T-SEC-no-secret-log: the webhook secret never appears in any log or spool file" {
  export OSQUERY_WEBHOOK_SECRET="SUPERSECRET123"
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  run_drain || true
  ! grep -rqF "SUPERSECRET123" \
    "$HARNESS_HOME/.local" "$OSQUERY_SPOOL_DIR" "$CURL_LOG" "$OSQUERY_DELIVERY_LOG" 2>/dev/null
}
