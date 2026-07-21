#!/usr/bin/env bats
# Drain-time failure classification (DR-B T3): a failing POST is not one thing.
# A TRANSIENT status (429, 5xx, or a transport failure) can succeed later, so
# the row stays pending, its attempts count goes up by one, and its
# next_attempt_after moves into the future so the drain stops hammering a
# failing gateway on every tick. A PERMANENT status (401, 403, 404, 413) can
# never succeed by retrying, so the row moves to the dead_letter_alerts table
# immediately, with the status recorded in its reason, instead of being retried
# forever. The batch dead-letter thresholds and their single CRIT are T4; the
# full head-of-line pins are T5.

setup() {
  local helpers="$BATS_TEST_DIRNAME/../helpers"
  # shellcheck source=test/helpers/build-dispatch-harness.sh
  source "$helpers/build-dispatch-harness.sh"
  # shellcheck source=test/helpers/wait-for-log-line.sh
  source "$helpers/wait-for-log-line.sh"
  build_dispatch_harness
}
teardown() { teardown_dispatch_harness; }

# Count dead_letter_alerts rows; a store without the table yet counts as zero
# (nothing has ever been dead-lettered).
dead_letter_count() {
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    'SELECT COUNT(*) FROM dead_letter_alerts;' 2>/dev/null || echo 0
}

@test "T-DLQ-transient-bookkeeping: a transient failure stays pending with attempts+1 and a future next_attempt_after that the drain respects" {
  # A long retry base makes "future" unmistakable: after one failed attempt the
  # row must not be eligible again for about an hour, so an immediate re-drain
  # proves the wait is respected, not just written.
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=3600
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  # One row per transient shape: a 5xx, a 429, and a transport failure (000).
  _osquery_store_alert_row 1000 osquery-transient-503 "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-transient-429 "$url" "$body_b64"
  _osquery_store_alert_row 3000 osquery-transient-000 "$url" "$body_b64"
  set_curl_codes 503 429 000

  local before_drain
  before_drain="$(date -u +%s)"
  retry_undelivered_alerts

  # All three rows are still pending (a transient failure never loses a page)
  # and none was dead-lettered.
  assert_pending_alert_count 3
  [[ "$(dead_letter_count)" == "0" ]]

  # Each row counted its failed attempt and scheduled its next try well into
  # the future (base 3600 * attempt 1, so comfortably past +1800 even with
  # clock slop between the drain and this assert).
  local request_id attempts next_attempt_after
  for request_id in osquery-transient-503 osquery-transient-429 osquery-transient-000; do
    attempts="$(sqlite3_query "SELECT attempts FROM pending_alerts WHERE request_id='$request_id';")"
    if [[ $attempts != "1" ]]; then
      printf '%s: expected attempts=1 after one transient failure, got %s\n' "$request_id" "$attempts" >&2
      return 1
    fi
    next_attempt_after="$(sqlite3_query "SELECT next_attempt_after FROM pending_alerts WHERE request_id='$request_id';")"
    if [[ ! $next_attempt_after -gt $((before_drain + 1800)) ]]; then
      printf '%s: expected a future next_attempt_after (> %s), got %s\n' \
        "$request_id" "$((before_drain + 1800))" "$next_attempt_after" >&2
      return 1
    fi
  done

  # The wait is RESPECTED: an immediate re-drain (gateway now healthy) must
  # skip all three rows, because none of their retry times has arrived. No
  # POST fires and the bookkeeping is untouched by the skipped pass.
  : >"$CURL_LOG"
  set_curl_codes 200 200 200
  retry_undelivered_alerts
  assert_post_count 0
  assert_pending_alert_count 3
  [[ "$(sqlite3_query 'SELECT COUNT(*) FROM pending_alerts WHERE attempts=1;')" == "3" ]]
}

@test "T-DLQ-permanent-immediate: a 401/403/404/413 row moves to dead_letter_alerts at once, with the status in reason, and the drain continues" {
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=3600
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  # Four permanent rows (occurrence order = drain order), then a transient row
  # BEHIND them, so the drain reaching it proves a permanent failure never
  # stops the pass.
  _osquery_store_alert_row 1000 osquery-perm-401 "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-perm-403 "$url" "$body_b64"
  _osquery_store_alert_row 3000 osquery-perm-404 "$url" "$body_b64"
  _osquery_store_alert_row 4000 osquery-perm-413 "$url" "$body_b64"
  _osquery_store_alert_row 5000 osquery-behind-them "$url" "$body_b64"
  set_curl_codes 401 403 404 413 503

  local before_drain
  before_drain="$(date -u +%s)"
  retry_undelivered_alerts

  # Only the transient row is still pending; the four permanent rows are gone
  # from the retry queue (never retried like a transient)...
  assert_pending_alert_count 1
  [[ "$(sqlite3_query 'SELECT request_id FROM pending_alerts;')" == "osquery-behind-them" ]]
  # ...and each landed in dead_letter_alerts exactly once, its payload intact,
  # with the failing status recorded in last_http_status AND readable in the
  # reason, plus a real dead-letter timestamp.
  [[ "$(dead_letter_count)" == "4" ]]
  local status row
  for status in 401 403 404 413; do
    row="$(sqlite3_query "SELECT last_http_status, reason, url, body_base64, occurrence_ts, dead_lettered_at
                            FROM dead_letter_alerts WHERE request_id='osquery-perm-$status';")"
    [[ -n $row ]] || {
      printf 'osquery-perm-%s never reached dead_letter_alerts\n' "$status" >&2
      return 1
    }
    IFS='|' read -r dl_status dl_reason dl_url dl_body dl_occurrence dl_at <<<"$row"
    [[ $dl_status == "$status" ]]
    [[ $dl_reason == *"$status"* ]] # the reason names the failing status
    [[ $dl_url == "$url" ]]
    [[ $dl_body == "$body_b64" ]]
    [[ $dl_occurrence -ge 1000 && $dl_occurrence -le 4000 ]]
    [[ $dl_at -ge $before_drain ]]
  done

  # The transient row behind the permanent ones WAS attempted in the same pass
  # (its 503 is in the curl log) and got its bookkeeping.
  grep -qF 'X-Request-ID: osquery-behind-them' "$CURL_LOG"
  [[ "$(sqlite3_query "SELECT attempts FROM pending_alerts WHERE request_id='osquery-behind-them';")" == "1" ]]

  # Dead-lettering is loud in the delivery log, never a silent disappearance.
  grep -q 'DEAD-LETTERED' "$OSQUERY_DELIVERY_LOG"
  grep -q 'osquery-perm-401' "$OSQUERY_DELIVERY_LOG"
}

# --- DR-B T4: attempt/age thresholds + one batched CRIT per pass ---------------

@test "T-DLQ-attempts-threshold: a row that has failed the max number of times dead-letters with an attempts reason, not another POST" {
  # A transient row is not retried forever: once its attempts reach the max, the
  # drain gives up and moves it to dead_letter instead of POSTing yet again.
  export OSQUERY_DRAIN_MAX_ATTEMPTS=5
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800 # large, so ONLY the attempts trigger fires
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-maxed "$url" "$body_b64"
  # Stand the row up as one that has already failed the max number of times.
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_alerts SET attempts=5 WHERE request_id='osquery-maxed';"
  : >"$CURL_LOG"
  # A 200 is queued on purpose: if the row WERE POSTed it would succeed and be
  # deleted (dead_letter stays empty). A surviving dead_letter row therefore
  # proves the give-up happened BEFORE any send.
  set_curl_codes 200

  retry_undelivered_alerts

  assert_pending_alert_count 0
  [[ "$(dead_letter_count)" == "1" ]]
  ! grep -qF 'X-Request-ID: osquery-maxed' "$CURL_LOG" # never POSTed again
  local reason
  reason="$(sqlite3_query "SELECT reason FROM dead_letter_alerts WHERE request_id='osquery-maxed';")"
  [[ $reason == *attempt* ]] # the reason names the attempts threshold it crossed
  [[ "$(sqlite3_query "SELECT attempts FROM dead_letter_alerts WHERE request_id='osquery-maxed';")" == "5" ]]
  grep -q 'DEAD-LETTERED' "$OSQUERY_DELIVERY_LOG"
}

@test "T-DLQ-age-threshold: a row older than the max age dead-letters with an age reason, not another POST" {
  # A row that has sat undelivered longer than the max age is given up on even
  # if its attempts count is low: a page nobody could deliver in a week is not
  # worth retrying forever.
  export OSQUERY_DRAIN_MAX_ATTEMPTS=1000 # large, so ONLY the age trigger fires
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=100
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-stale "$url" "$body_b64"
  # Age its created_at well past the 100s limit.
  local old_created_at
  old_created_at=$(($(date -u +%s) - 100000))
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_alerts SET created_at=$old_created_at WHERE request_id='osquery-stale';"
  : >"$CURL_LOG"
  set_curl_codes 200

  retry_undelivered_alerts

  assert_pending_alert_count 0
  [[ "$(dead_letter_count)" == "1" ]]
  ! grep -qF 'X-Request-ID: osquery-stale' "$CURL_LOG" # never POSTed again
  local reason
  reason="$(sqlite3_query "SELECT reason FROM dead_letter_alerts WHERE request_id='osquery-stale';")"
  [[ $reason == *age* ]] # the reason names the age threshold it crossed
  grep -q 'DEAD-LETTERED' "$OSQUERY_DELIVERY_LOG"
}

@test "T-DLQ-one-crit-per-pass: several dead-letters in one pass fire exactly ONE summary CRIT naming the count" {
  # Whether one record or many dead-letter in a pass, the operator gets exactly
  # ONE local CRIT summarizing the pass, never one alert per record.
  export OSQUERY_DRAIN_MAX_ATTEMPTS=5
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  # Two permanent-status rows plus one attempts-maxed row: three dead-letters,
  # a permanent-and-threshold mix, all in one drain pass.
  _osquery_store_alert_row 1000 osquery-perm-a "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-perm-b "$url" "$body_b64"
  _osquery_store_alert_row 3000 osquery-maxed "$url" "$body_b64"
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_alerts SET attempts=5 WHERE request_id='osquery-maxed';"
  : >"$ALERTER_LOG"
  : >"$CURL_LOG"
  set_curl_codes 401 403 # the two permanent rows; the maxed row never POSTs

  retry_undelivered_alerts

  assert_pending_alert_count 0
  [[ "$(dead_letter_count)" == "3" ]]
  # Exactly ONE loud local notification for the whole pass (the alerter stub
  # logs one line per invocation), and it summarizes the count (3).
  wait_for_log_line 'dead-letter' "$ALERTER_LOG"
  local crit_lines
  crit_lines=$(grep -ciF 'dead-letter' "$ALERTER_LOG")
  if [[ $crit_lines -ne 1 ]]; then
    printf 'expected exactly ONE summary CRIT, got %s lines:\n%s\n' "$crit_lines" "$(cat "$ALERTER_LOG")" >&2
    return 1
  fi
  grep -qE '(^|[^0-9])3([^0-9]|$)' "$ALERTER_LOG" # the one CRIT names N=3
}

@test "T-DLQ-no-crit-on-transient: a pass that dead-letters nothing fires no CRIT" {
  # Transient failures are NOT dead-letters: a pass where every failure is
  # retryable must leave the CRIT silent (no dead-letter, no pipeline alert).
  export OSQUERY_DRAIN_MAX_ATTEMPTS=20
  export OSQUERY_DRAIN_MAX_AGE_SECONDS=604800
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=0
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-soft-a "$url" "$body_b64"
  _osquery_store_alert_row 2000 osquery-soft-b "$url" "$body_b64"
  : >"$ALERTER_LOG"
  set_curl_codes 503 503

  retry_undelivered_alerts

  assert_pending_alert_count 2 # transient: retained, not dead-lettered
  [[ "$(dead_letter_count)" == "0" ]]
  # No dead-letter this pass, so the summary CRIT never fires. Nothing spawns a
  # background notifier, so an immediate emptiness check is race-free.
  [[ ! -s $ALERTER_LOG ]]
}
