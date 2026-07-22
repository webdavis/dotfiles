#!/usr/bin/env bats
# The scheduled drainer sweeps pending_local_notifications too (DR-C T3): a
# CRIT banner that failed and was persisted is re-attempted on the same 300s
# tick that drains the alert queue, one liveness owner, no second agent. The
# local sweep runs BEFORE the alert drain: the alert drain's degraded-pipeline
# CRIT can persist a NEW local row mid-pass, and sweeping local first means
# that row waits for the next tick instead of being attempted twice in one
# pass. The local channel is the FALLBACK: its failures are log lines only,
# never a webhook CRIT and never a dead-letter tally entry. No thresholds and
# no staleness rendering yet (T4).

setup() {
  local helpers="$BATS_TEST_DIRNAME/../helpers"
  # shellcheck source=test/helpers/build-dispatch-harness.sh
  source "$helpers/build-dispatch-harness.sh"
  build_dispatch_harness
}
teardown() { teardown_dispatch_harness; }

set_alerter_stub_exit() { # <exit-code> -- keep the argv logging, control the outcome
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"%s"\nexit %s\n' \
    "$ALERTER_LOG" "$1" >"$HARNESS_HOME/bin/alerter"
  chmod +x "$HARNESS_HOME/bin/alerter"
}

local_row_count() {
  sqlite3 -readonly "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    'SELECT COUNT(*) FROM pending_local_notifications;' 2>/dev/null || echo 0
}

# The success-delete is owned by the alerter WATCHER (confirmed-exit semantics),
# which settles asynchronously, so a delete assertion polls instead of racing.
wait_for_local_row_count() { # <expected>
  local expected="$1" attempt count
  for ((attempt = 0; attempt < 30; attempt++)); do
    count="$(local_row_count)"
    [[ $count == "$expected" ]] && return 0
    sleep 0.1
  done
  printf 'expected %s local row(s) once the watcher settled, still %s\n' "$expected" "$count" >&2
  return 1
}

@test "T-LND-redeliver-success: a due local row banners once (text intact) and is deleted" {
  # The title carries a tab and apostrophes and the message an apostrophe, so
  # this also pins the export encoding: arbitrary text must reach the notifier
  # byte-identical (a naive tab-separated export would garble it).
  local tricky_title=$'op\'s\tbanner' tricky_message="it's back"
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "$tricky_title" "$tricky_message" "seed-redeliver" # persists (banner fails)
  [[ "$(local_row_count)" == "1" ]]
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0 # the notifier works again

  retry_undelivered_alerts

  wait_for_local_row_count 0 # delivered -> confirmed by the watcher -> deleted
  # Exactly one banner attempt, and the stored text reached it intact.
  [[ "$(grep -c . "$ALERTER_LOG")" == "1" ]]
  grep -qF "$tricky_title" "$ALERTER_LOG"
  grep -qF "$tricky_message" "$ALERTER_LOG"
}

@test "T-LND-redeliver-failure: a still-failing local row is retained with attempts+1 and a future next_attempt_after" {
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=3600
  export OSQUERY_DRAIN_RETRY_RANDOM_SECONDS=0
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "t" "m" "seed-stillfailing"
  [[ "$(local_row_count)" == "1" ]]
  local before_drain
  before_drain="$(date -u +%s)"

  retry_undelivered_alerts

  [[ "$(local_row_count)" == "1" ]] # retained, never lost
  [[ "$(sqlite3_query 'SELECT attempts FROM pending_local_notifications;')" == "1" ]]
  local next_attempt_after
  next_attempt_after="$(sqlite3_query 'SELECT next_attempt_after FROM pending_local_notifications;')"
  [[ $next_attempt_after -gt $((before_drain + 1800)) ]]
  grep -q 'LOCAL-NOTIFY-RETRY-FAILED' "$OSQUERY_DELIVERY_LOG" # a log line, not a CRIT
}

@test "T-LND-not-due: a row whose retry wait has not passed is untouched, no banner attempt" {
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "t" "m" "seed-notdue"
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_local_notifications SET next_attempt_after = $(($(date -u +%s) + 3600));"
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0

  retry_undelivered_alerts

  [[ "$(local_row_count)" == "1" ]]
  [[ ! -s $ALERTER_LOG ]] # zero attempts on a not-yet-due row
  [[ "$(sqlite3_query 'SELECT attempts FROM pending_local_notifications;')" == "0" ]]
}

@test "T-LND-isolation: a failing local row aborts nothing and never feeds the webhook CRIT tally" {
  # One failing local row + one deliverable alert row in the same pass: the
  # alert must still deliver, the pass must fire NO degraded-pipeline CRIT
  # (zero ALERT records dead-lettered; a local failure is not a dead-letter),
  # and the whole pass survives set -e.
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "t" "m" "seed-isolation"
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-deliverable "$url" "$body_b64"
  : >"$ALERTER_LOG"
  : >"$CURL_LOG"
  set_curl_codes 200

  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]
  [[ $output == *DONE* ]]

  grep -qF 'X-Request-ID: osquery-deliverable' "$CURL_LOG" # the alert still delivered
  assert_pending_alert_count 0
  [[ "$(local_row_count)" == "1" ]] # the local row is retained for the next tick
  ! grep -qi 'pipeline degraded' "$ALERTER_LOG" # no CRIT for a local-channel failure
  grep -q 'LOCAL-NOTIFY-RETRY-FAILED' "$OSQUERY_DELIVERY_LOG"
}

@test "T-LND-ordering: a local row persisted by THIS pass's degraded CRIT is not attempted until the next tick" {
  # The alert drain dead-letters a record, fires the pass CRIT, the CRIT banner
  # FAILS, and the wrapper persists it as a local row mid-pass. The local sweep
  # already ran (local first), so that fresh row must show zero attempts and
  # the notifier must have been invoked exactly once (the CRIT attempt itself,
  # not a same-pass retry of the just-persisted row).
  export OSQUERY_DRAIN_MAX_ATTEMPTS=1 # the seeded alert row dead-letters pre-POST
  set_alerter_stub_exit 64
  local url='http://127.0.0.1:8644/webhooks/osquery-priority' body_b64
  body_b64=$(printf '{"event_type":"osquery.alert"}' | base64 | tr -d '\n')
  _osquery_store_alert_row 1000 osquery-doomed "$url" "$body_b64"
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_alerts SET attempts=1 WHERE request_id='osquery-doomed';"
  : >"$ALERTER_LOG"

  retry_undelivered_alerts

  [[ "$(local_row_count)" == "1" ]] # the failed CRIT was captured...
  [[ "$(sqlite3_query 'SELECT attempts FROM pending_local_notifications;')" == "0" ]] # ...but NOT retried this pass
  [[ "$(grep -c . "$ALERTER_LOG")" == "1" ]] # exactly the one CRIT attempt
}

@test "T-LND-skip-and-continue: the first local row failing does not block the second" {
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=3600
  export OSQUERY_DRAIN_RETRY_RANDOM_SECONDS=0
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "first title" "m1" "seed-first"
  _osquery_notify_local_durable "second title" "m2" "seed-second"
  [[ "$(local_row_count)" == "2" ]]
  # Order the rows deterministically: the first seed strictly older, but both
  # RECENT (seconds old), so the age-out behavior added later never expires
  # them; this pin is about ordering under failure, not staleness.
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_local_notifications SET occurrence_ts = CASE WHEN title='first title' THEN $(($(date -u +%s) - 200)) ELSE $(($(date -u +%s) - 100)) END;"
  : >"$ALERTER_LOG"

  retry_undelivered_alerts

  # BOTH rows were attempted (the failing first never starves the second)...
  grep -qF 'first title' "$ALERTER_LOG"
  grep -qF 'second title' "$ALERTER_LOG"
  # ...and both were retained with their bookkeeping counted.
  [[ "$(local_row_count)" == "2" ]]
  [[ "$(sqlite3_query 'SELECT COUNT(*) FROM pending_local_notifications WHERE attempts=1;')" == "2" ]]
}

# --- DR-C T4: two staleness behaviors ------------------------------------------
# Behavior 1: a late-redelivered banner tells the truth about WHEN its event
# occurred (the occurrence time is rendered into the banner, so a banner shown
# hours late cannot read as breaking news).
# Behavior 2: a local notification too old to matter is EXPIRED loudly instead
# of retried forever (deleted with a forensic log line; the operator has long
# since seen the durable Discord copy, so a day-old banner is pure noise).

# Render an epoch as the banner subtitle's UTC ISO 8601 form, the same both
# date flavors: BSD (-r) first, GNU (-d @) as the fallback.
iso8601_of_epoch() {
  date -u -r "$1" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d "@$1" +%Y-%m-%dT%H:%M:%SZ
}

@test "T-LND-late-banner-honest-timestamp: a redelivered banner names its occurrence time" {
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "late title" "late message" "seed-late-honest"
  [[ "$(local_row_count)" == "1" ]]
  local occurrence_ts
  occurrence_ts="$(sqlite3_query 'SELECT occurrence_ts FROM pending_local_notifications;')"
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0

  retry_undelivered_alerts

  wait_for_local_row_count 0 # shown -> confirmed by the watcher -> deleted
  # The banner carried the ORIGINAL occurrence time as its subtitle, so the
  # operator reads a late banner as history, not as breaking news.
  grep -qF -- "--subtitle occurred $(iso8601_of_epoch "$occurrence_ts")" "$ALERTER_LOG"
}

@test "T-LND-fresh-banner-unmarked: a FIRST-ATTEMPT banner carries no staleness subtitle" {
  # The honest-timestamp marking belongs to the drain retry path ONLY: a fresh
  # first-attempt banner IS breaking news and must render exactly as before.
  set_alerter_stub_exit 0
  : >"$ALERTER_LOG"
  _osquery_notify_local_durable "fresh title" "fresh message" "seed-fresh"
  [[ "$(grep -c . "$ALERTER_LOG")" == "1" ]]
  ! grep -qF -- '--subtitle' "$ALERTER_LOG"
  ! grep -qF 'occurred' "$ALERTER_LOG"
}

@test "T-LND-expired-loudly: an over-age row is deleted with a forensic log line, never bannered" {
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "old title" "old message" "seed-expired"
  local notification_id
  notification_id="$(sqlite3_query 'SELECT notification_id FROM pending_local_notifications;')"
  # Age the row past the 1-day default.
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_local_notifications SET occurrence_ts = $(($(date -u +%s) - 200000));"
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0 # a working notifier must still NOT be handed the stale banner

  retry_undelivered_alerts

  [[ ! -s $ALERTER_LOG ]]          # zero banner attempts
  [[ "$(local_row_count)" == "0" ]] # expired -> deleted
  grep -q 'LOCAL-NOTIFY-EXPIRED' "$OSQUERY_DELIVERY_LOG"       # loud, never silent
  grep -qF "$notification_id" "$OSQUERY_DELIVERY_LOG"          # names the row
  grep -qE 'LOCAL-NOTIFY-EXPIRED.*age[=_ ][0-9]+' "$OSQUERY_DELIVERY_LOG" # and its age
}

@test "T-LND-age-knob: the age ceiling is the operator's knob (small expires, large retries)" {
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "knob title" "knob message" "seed-knob"
  # Make the row 60 seconds old.
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_local_notifications SET occurrence_ts = $(($(date -u +%s) - 60));"
  # A LARGE ceiling: the 60s-old row is fresh enough and banners.
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0
  OSQUERY_LOCAL_NOTIFY_MAX_AGE_SECONDS=999999 retry_undelivered_alerts
  grep -qF 'knob title' "$ALERTER_LOG"
  wait_for_local_row_count 0 # shown -> confirmed by the watcher -> deleted
  # Re-seed and shrink the ceiling BELOW the row's age: it expires unseen.
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "knob title 2" "knob message 2" "seed-knob-2"
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_local_notifications SET occurrence_ts = $(($(date -u +%s) - 60));"
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0
  OSQUERY_LOCAL_NOTIFY_MAX_AGE_SECONDS=10 retry_undelivered_alerts
  [[ ! -s $ALERTER_LOG ]]
  [[ "$(local_row_count)" == "0" ]]
  grep -q 'LOCAL-NOTIFY-EXPIRED' "$OSQUERY_DELIVERY_LOG"
}

@test "T-LND-expiry-isolation: expiring a row aborts nothing, fires no CRIT, and later rows still banner" {
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "doomed title" "doomed message" "seed-doomed"
  sqlite3 "$OSQUERY_UNDELIVERED_ALERTS_DB" \
    "UPDATE pending_local_notifications SET occurrence_ts = 1000;" # ancient
  _osquery_notify_local_durable "fresh behind title" "fresh behind message" "seed-behind"
  [[ "$(local_row_count)" == "2" ]]
  : >"$ALERTER_LOG"
  set_alerter_stub_exit 0

  run bash -c "set -euo pipefail; source '$DISPATCH'; retry_undelivered_alerts; echo DONE"
  [[ $status -eq 0 ]]
  [[ $output == *DONE* ]]

  wait_for_local_row_count 0 # ancient expired, fresh shown-confirmed-deleted
  grep -qF 'fresh behind title' "$ALERTER_LOG" # the row behind the expiry still bannered
  ! grep -qF 'doomed title' "$ALERTER_LOG"     # the expired one never did
  ! grep -qi 'pipeline degraded' "$ALERTER_LOG" # expiry is not a dead-letter; no CRIT
  grep -q 'LOCAL-NOTIFY-EXPIRED' "$OSQUERY_DELIVERY_LOG"
}

@test "T-LND-late-failure-durable: a redelivered banner that outlives the grace window and then fails keeps its row" {
  # sol's repro on the retry path: the alerter lives past the ~0.6s grace
  # window (advisory says posted) and THEN exits nonzero. The old inline
  # success-delete would have removed the row on the advisory alone; the
  # watcher-confirmed delete keeps it, because no confirmation ever arrived.
  set_alerter_stub_exit 64
  _osquery_notify_local_durable "late fail title" "late fail message" "seed-latefail"
  [[ "$(local_row_count)" == "1" ]] # write-ahead: the row exists before any retry

  cat >"$HARNESS_HOME/bin/alerter" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$ALERTER_LOG"
sleep 0.7
exit 64
STUB
  chmod +x "$HARNESS_HOME/bin/alerter"
  : >"$ALERTER_LOG"

  retry_undelivered_alerts

  grep -qF 'late fail title' "$ALERTER_LOG" # the retry really attempted the banner
  sleep 1.2 # let the watcher witness the late nonzero exit
  if [[ "$(local_row_count)" != "1" ]]; then
    printf 'expected the row to SURVIVE a banner that failed after the grace window; rows=%s\n' \
      "$(local_row_count)" >&2
    return 1
  fi
}
