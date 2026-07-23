#!/usr/bin/env bats
# Shared dispatch library (alert-dispatch.sh): v2 delivers ONLY a CRIT page to the
# #priority webhook. Any other severity does the local notification and never
# POSTs (there is no #osquery channel for a producer to leak to). A CRIT body
# carries the host and a tier field, and an undelivered page is stored durably
# rather than dropped.

setup() {
  local helpers="$BATS_TEST_DIRNAME/../helpers"
  # shellcheck source=test/helpers/build-dispatch-harness.sh
  source "$helpers/build-dispatch-harness.sh"
  # shellcheck source=test/helpers/run-dispatch-and-drain.sh
  source "$helpers/run-dispatch-and-drain.sh"
  # shellcheck source=test/helpers/wait-for-log-line.sh
  source "$helpers/wait-for-log-line.sh"
  build_dispatch_harness
}
teardown() { teardown_dispatch_harness; }

@test "T-DISP-crit-priority: a CRIT page POSTs to the #priority webhook" {
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  assert_posted_to "/webhooks/osquery-priority"
}

@test "T-DISP-noncrit-silent: a non-CRIT severity never POSTs (no #osquery channel)" {
  send_alert NOTICE "🟡 title" "detail" "Glass"
  assert_no_post
}

@test "T-DISP-host-field: the signed webhook body carries a host field (multi-host seam)" {
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  # The curl stub records the full invocation including --data <body>; host must
  # be inside the signed bytes, the master-spec body shape and the multi-host
  # seam both require {event_type, host, alert:{...}}.
  run grep -F '"host":"' "$CURL_LOG"
  [[ $status -eq 0 ]]
}

@test "T-DISP-nosecret-store: a CRIT with no webhook secret stores the page and loudly names the broken channel" {
  # The old code logged a WARN and returned SUCCESS without storing, so the
  # critical silently degraded to local-only and was lost. With no secret the
  # page must be stored durably (it delivers when the secret returns) and a LOUD
  # local notice must name the broken channel; nothing may be signed or POSTed
  # without a key.
  unset OSQUERY_WEBHOOK_SECRET
  : >"$ALERTER_LOG"
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi"
  [[ $send_status -eq 0 ]]        # still fire-and-forget for its callers
  assert_pending_alert_count 1    # durably stored, not dropped
  assert_no_post                  # nothing signed/POSTed without a key
  grep -qiE 'secret|degraded|broken' "$OSQUERY_DELIVERY_LOG"
  # The loud local notice fires via a backgrounded alerter (& so it never blocks
  # dispatch), so poll rather than grep once.
  wait_for_log_line 'secret|Discord|broken|deliver' "$ALERTER_LOG"
}

# --- occurrence-identity request id + occurrence-unique stored filenames (R2-4) ---
# request_id was sha256(body): two DISTINCT incidents with the same body collapsed
# to one id, so the gateway deduped them AND the second overwrote the first in the
# store (the filename is the id). The id must derive from OCCURRENCE IDENTITY, so
# two same-body incidents survive as two stored files.

@test "T-DISP-occurrence-distinct: two same-body incidents at different occurrences store as TWO rows (R2-4)" {
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:200:300"
  assert_pending_alert_count 2 # both incidents survive, a later same-body incident does not clobber the earlier
}

@test "T-DISP-occurrence-retry-stable: the SAME occurrence retried reuses one id (idempotent store) (R2-4)" {
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  assert_pending_alert_count 1
}

# --- a store failure is a HARD failure, not a silent success (R2-6) ---------------
@test "T-DISP-store-hardfail: an unwritable store + no secret returns nonzero and loudly alerts (R2-6)" {
  # Point the DB at a path whose parent is a FILE, so mkdir cannot succeed for
  # any uid (deterministic, not permission/uid-dependent). With no secret the page
  # cannot be signed either, so it can be neither delivered NOR stored, which MUST
  # be loud and nonzero.
  unset OSQUERY_WEBHOOK_SECRET
  : >"$ALERTER_LOG"
  touch "$HARNESS_HOME/notadir"
  export OSQUERY_UNDELIVERED_ALERTS_DB="$HARNESS_HOME/notadir/store.sqlite3"
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi"
  [[ $send_status -ne 0 ]]                          # hard delivery failure, NOT a silent success
  assert_pending_alert_count 0                      # nothing was stored
  assert_no_post                                    # nothing signed/POSTed without a key
  grep -qiE 'STORE-FAILED' "$OSQUERY_DELIVERY_LOG"  # synchronous: the hard failure is recorded
  wait_for_log_line 'FAILED|lost|could not' "$ALERTER_LOG" # the loud local alert fired
}

@test "T-DISP-row-idempotent: the SAME occurrence stored twice keeps exactly ONE pending_alerts row (T5)" {
  # A producer retrying the same occurrence re-stores the same request_id; the
  # ON CONFLICT(request_id) DO NOTHING insert must keep exactly one row, and the
  # re-store is a normal success for the caller, never an error.
  set_curl_codes 503 503 503 503 503 503
  run_dispatch send_output send_status CRIT "🔴 title" "same detail" "Sosumi" "occ:row-idem:1"
  [[ $send_status -eq 0 ]]
  run_dispatch send_output send_status CRIT "🔴 title" "same detail" "Sosumi" "occ:row-idem:1"
  [[ $send_status -eq 0 ]] # the idempotent re-store reports success
  assert_pending_alert_count 1
  [[ "$(sqlite3_query 'SELECT COUNT(DISTINCT request_id) FROM pending_alerts;')" == "1" ]]
}

@test "T-DISP-db-store-hardfail: an unwritable DB path returns nonzero and loudly alerts (T5)" {
  # The DB parent is a FILE, so its mkdir fails for any uid (deterministic, the
  # same trick as the file-store hard-fail pins). The write-ahead persist cannot
  # complete, which must keep the file-store era contract: nonzero return, the
  # STORE-FAILED log line, the loud local fallback, and NO network attempt (the
  # write-ahead order forbids a send before a completed persist).
  : >"$ALERTER_LOG"
  set_curl_codes 503 503 503
  touch "$HARNESS_HOME/dbnotdir"
  export OSQUERY_UNDELIVERED_ALERTS_DB="$HARNESS_HOME/dbnotdir/store.sqlite3"
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi" "occ:db-hardfail:1"
  [[ $send_status -ne 0 ]] # hard delivery failure, NOT a silent success
  assert_pending_alert_count 0
  assert_no_post
  grep -qE 'STORE-FAILED' "$OSQUERY_DELIVERY_LOG"
  # The failure must name the DB path so the operator fixes the RIGHT storage.
  grep -qF "$OSQUERY_UNDELIVERED_ALERTS_DB" "$OSQUERY_DELIVERY_LOG"
  wait_for_log_line 'FAILED|lost|could not' "$ALERTER_LOG"
}

# --- digest/heartbeat traffic DOES reach the remote POST; mark its tier (R2-11) ---
# The dispatcher POSTs for severity==CRIT, and the digest and heartbeat both
# dispatch at CRIT, so they DO reach the remote POST. The `sound` arg only muted
# the LOCAL notifier; the POST carried no tier, so a silent digest looked
# identical to a real page on the wire. Thread an explicit tier the Hermes adapter
# can map to a suppressed notification: a page (non-empty sound) is tier=page, a
# muted digest/heartbeat (empty sound) is tier=muted. Both still POST (both CRIT).

@test "T-DISP-url-whitespace-hardfail: a URL containing whitespace is refused at persist time (F3)" {
  # The drain's row export is tab-separated, so a URL carrying a tab (or any
  # whitespace/control character) would garble its row into an undeliverable,
  # undiagnosed shape. A malformed URL must never enter durable storage: the
  # persist refuses it with the loud hard-fail, before any network attempt.
  : >"$ALERTER_LOG"
  set_curl_codes 503 503 503
  export OSQUERY_HERMES_PRIORITY_URL=$'http://127.0.0.1:8644/webhooks/osquery\tpriority'
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi" "occ:taburl:1"
  [[ $send_status -ne 0 ]] # refused, not silently stored malformed
  assert_pending_alert_count 0
  assert_no_post
  grep -qE 'STORE-FAILED' "$OSQUERY_DELIVERY_LOG"
  wait_for_log_line 'FAILED|lost|could not' "$ALERTER_LOG"
}

@test "T-DISP-tier-page: a real page (non-empty sound) POSTs tier=page (R2-11)" {
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  assert_post_count 1
  grep -qF '"tier":"page"' "$CURL_LOG"
}

@test "T-DISP-tier-muted: a muted digest/heartbeat (empty sound) POSTs tier=muted, not page (R2-11)" {
  send_alert CRIT "🗒️ daily digest" "detail" ""
  assert_post_count 1 # it DOES reach the remote POST (not moot)
  grep -qF '"tier":"muted"' "$CURL_LOG"
  ! grep -qF '"tier":"page"' "$CURL_LOG"
}

@test "T-DISP-body-ts: a CRIT body carries a numeric occurrence ts (the drain ordering key)" {
  # The occurrence time is inside the signed body so a replayed page keeps its
  # original ordering key and the field is tamper-evident.
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  run grep -oE '"ts":[0-9]+' "$CURL_LOG"
  [[ $status -eq 0 ]]
}

@test "T-DISP-store-concurrency: parallel producers and drains lose no rows under WAL plus busy_timeout (T4)" {
  # Eight producers with distinct occurrences send in parallel against an
  # always-failing sender while two drains run alongside them. Every send must
  # report success (a swallowed SQLITE_BUSY would surface as send_alert's hard
  # failure), no writer may hit an unabsorbed lock, and the store must end with
  # EXACTLY eight rows: no lost insert, no unique-constraint collision, and
  # eight distinct ids and sequence numbers. The queued-codes curl stub pops its
  # queue non-atomically, so a deterministic always-503 stub replaces it here.
  #
  # Neutralize the retry backoff for this test (base and random offset both 0) so
  # a row a drain fails transiently (503) comes due IMMEDIATELY instead of waiting
  # out the production retry schedule (60 to 120 seconds). Under a contended
  # runner a phase-1 drain can catch a freshly stored row and push its
  # next_attempt_after minutes ahead; the recovery drains would then skip it as
  # not-yet-due and the store would never clear in-test. Zeroing the schedule (the
  # library's own deterministic-retry knobs) models the next scheduled 300s drain
  # retrying the row, compressed to zero wall-clock. It weakens no guarantee: the
  # row is still stored, signed, and delivered; only the wait between attempts is
  # removed. It MUST precede phase 1, because that is where a drain bumps the row.
  export OSQUERY_DRAIN_RETRY_BASE_SECONDS=0 OSQUERY_DRAIN_RETRY_RANDOM_SECONDS=0
  cat >"$HARNESS_HOME/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
printf '503'
STUB
  chmod +x "$HARNESS_HOME/bin/curl"

  local producer_count=8 i
  local status_dir="$HARNESS_HOME/stress-status"
  mkdir -p "$status_dir"
  for ((i = 1; i <= producer_count; i++)); do
    (
      send_alert CRIT "🔴 stress $i" "detail $i" "Sosumi" "occ:stress:$i" \
        2>"$status_dir/producer-$i.err"
      echo $? >"$status_dir/producer-$i.status"
    ) &
  done
  for i in 1 2; do
    (
      retry_undelivered_alerts 2>"$status_dir/drain-$i.err"
      echo $? >"$status_dir/drain-$i.status"
    ) &
  done
  wait

  for ((i = 1; i <= producer_count; i++)); do
    [[ "$(cat "$status_dir/producer-$i.status")" == "0" ]]
  done
  for i in 1 2; do
    [[ "$(cat "$status_dir/drain-$i.status")" == "0" ]]
  done
  ! grep -rqi 'database is locked' "$status_dir" # busy_timeout serializes, never errors
  [[ "$(sqlite3_query 'SELECT COUNT(*) FROM pending_alerts;')" == "$producer_count" ]]
  [[ "$(sqlite3_query 'SELECT COUNT(DISTINCT request_id) FROM pending_alerts;')" == "$producer_count" ]]
  [[ "$(sqlite3_query 'SELECT COUNT(DISTINCT sequence_number) FROM pending_alerts;')" == "$producer_count" ]]

  # Recovery models production's eventual consistency. Two drains run at once as
  # the concurrency stress (both must exit 0), then bounded SERIAL drains sweep
  # until the store is empty. A page POSTed 200 whose delete loses the write-lock
  # race, or a row a parallel pass did not see in its snapshot, stays PENDING but
  # is NOT lost: the next drain re-posts it (the gateway dedups by request_id) and
  # deletes it. Asserting "0 pending after EXACTLY two parallel passes" was too
  # strict for a contended runner, where two passes are not guaranteed to clear
  # every row; the bounded drain-until-empty loop is the honest model, and its
  # ceiling still fails a genuine hang (a row that never clears) rather than
  # looping forever.
  local stored_request_ids
  stored_request_ids="$(sqlite3_query 'SELECT request_id FROM pending_alerts;')"
  cat >"$HARNESS_HOME/bin/curl" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_LOG"
printf '200'
STUB
  chmod +x "$HARNESS_HOME/bin/curl"
  : >"$CURL_LOG"

  # The concurrency stress: two drains against the stored rows at once.
  local drain_passes=0
  for i in 3 4; do
    (
      retry_undelivered_alerts 2>"$status_dir/drain-$i.err"
      echo $? >"$status_dir/drain-$i.status"
    ) &
  done
  wait
  drain_passes=$((drain_passes + 2))
  [[ "$(cat "$status_dir/drain-3.status")" == "0" ]]
  [[ "$(cat "$status_dir/drain-4.status")" == "0" ]]

  # Then drain to empty. Serial is fine: with the success sender every due row
  # delivers and deletes, so a single pass clears whatever the parallel passes
  # left. The ceiling (twice the row count) sits far above the one pass this
  # needs yet is bounded, so a row that never clears fails the test.
  local max_serial_passes=$((producer_count * 2)) serial_pass
  for ((serial_pass = 1; serial_pass <= max_serial_passes; serial_pass++)); do
    [[ "$(sqlite3_query 'SELECT COUNT(*) FROM pending_alerts;')" != "0" ]] || break
    retry_undelivered_alerts 2>"$status_dir/drain-serial-$serial_pass.err"
    drain_passes=$((drain_passes + 1))
  done

  # The store fully clears within the bounded drain-until-empty.
  assert_pending_alert_count 0
  # Every stored page delivered AT LEAST once (no lost delivery), and no page
  # posted more than the number of drain passes actually run: a single pass reads
  # its row list once and never re-posts a row it already handled, so duplicates
  # come only from a later pass re-posting a still-pending row, bounded by the
  # pass count (the gateway dedups by request_id).
  local request_id post_count
  while IFS= read -r request_id; do
    [[ -n $request_id ]] || continue
    post_count=$(grep -cF "X-Request-ID: $request_id" "$CURL_LOG" || true)
    [[ $post_count -ge 1 && $post_count -le $drain_passes ]]
  done <<<"$stored_request_ids"
}

@test "T-DISP-secret-not-in-argv: the signing key never reaches any openssl argv (F-B)" {
  # Shim openssl to log its full argv, then delegate to the real one so signing
  # still works. A CRIT send must never pass the secret as an openssl argument,
  # any user's `ps` would otherwise see the key.
  local real_openssl
  real_openssl="$(command -v openssl)"
  export OPENSSL_ARGV_LOG="$HARNESS_HOME/openssl_argv.log"
  : >"$OPENSSL_ARGV_LOG"
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf "%%s\\n" "$*" >>"%s"\n' "$OPENSSL_ARGV_LOG"
    printf 'exec %q "$@"\n' "$real_openssl"
  } >"$HARNESS_HOME/bin/openssl"
  chmod +x "$HARNESS_HOME/bin/openssl"
  export OSQUERY_WEBHOOK_SECRET="SUPERSECRET-argv-probe"
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  [[ -s $OPENSSL_ARGV_LOG ]] # openssl WAS invoked (sanity)
  ! grep -qF 'SUPERSECRET-argv-probe' "$OPENSSL_ARGV_LOG"
}

@test "T-DISP-signing-failure: a failed signature makes NO POST and retains the write-ahead record" {
  # The delivery attempt runs inside an if condition, where errexit is
  # suppressed, so a failing signature assignment must be checked explicitly:
  # otherwise execution continues with an EMPTY signature, POSTs it, and a 2xx
  # deletes the write-ahead record. Force the signer to fail and assert the
  # attempt stops before any POST, leaving the record for a later drain.
  _hmac_sha256_hex() { return 17; }
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi" "occ:signfail:1"
  [[ $send_status -eq 0 ]] # the row is retained, so this is not a hard failure
  assert_no_post
  assert_pending_alert_count 1
}

@test "T-DISP-sign-matches-openssl: the argv-free signer equals openssl's HMAC output (F-B)" {
  # Correctness anchor: the manual HMAC must be byte-identical to openssl's -hmac,
  # or a wrong-but-consistent signature would pass the curl-mocked tests yet be
  # rejected by the gateway in production.
  local message="the message" key="k3y" got want
  got="$(printf '%s' "$message" | _hmac_sha256_hex "$key")"
  want="$(printf '%s' "$message" | openssl dgst -sha256 -hmac "$key" | awk '{print $NF}')"
  [[ $got == "$want" ]]
}
