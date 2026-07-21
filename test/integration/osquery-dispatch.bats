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
  [[ $send_status -eq 0 ]]          # still fire-and-forget for its callers
  assert_undelivered_alert_count 1  # durably stored, not dropped
  assert_no_post                    # nothing signed/POSTed without a key
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

@test "T-DISP-occurrence-distinct: two same-body incidents at different occurrences store as TWO files (R2-4)" {
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:200:300"
  assert_undelivered_alert_count 2 # both incidents survive, a later same-body incident does not clobber the earlier
}

@test "T-DISP-occurrence-retry-stable: the SAME occurrence retried reuses one id (idempotent store) (R2-4)" {
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  assert_undelivered_alert_count 1
}

# --- a store failure is a HARD failure, not a silent success (R2-6) ---------------
@test "T-DISP-store-hardfail: an unwritable store dir + no secret returns nonzero and loudly alerts (R2-6)" {
  # Point the store at a path whose parent is a FILE, so mkdir cannot succeed for
  # any uid (deterministic, not permission/uid-dependent). With no secret the page
  # cannot be signed either, so it can be neither delivered NOR stored, which MUST
  # be loud and nonzero.
  unset OSQUERY_WEBHOOK_SECRET
  : >"$ALERTER_LOG"
  touch "$HARNESS_HOME/notadir"
  export OSQUERY_UNDELIVERED_ALERTS_DIR="$HARNESS_HOME/notadir/store"
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi"
  [[ $send_status -ne 0 ]]                          # hard delivery failure, NOT a silent success
  assert_undelivered_alert_count 0                  # nothing was stored
  assert_no_post                                    # nothing signed/POSTed without a key
  grep -qiE 'STORE-FAILED' "$OSQUERY_DELIVERY_LOG"  # synchronous: the hard failure is recorded
  wait_for_log_line 'FAILED|lost|could not' "$ALERTER_LOG" # the loud local alert fired
}

@test "T-DISP-store-hardfail-delivery: delivery failure + unwritable store returns nonzero and loudly alerts (R2-6)" {
  : >"$ALERTER_LOG"
  set_curl_codes 503 503 503
  touch "$HARNESS_HOME/notadir2"
  export OSQUERY_UNDELIVERED_ALERTS_DIR="$HARNESS_HOME/notadir2/store"
  run_dispatch send_output send_status CRIT "🔴 title" "detail body" "Sosumi"
  [[ $send_status -ne 0 ]]
  assert_undelivered_alert_count 0
  grep -qiE 'STORE-FAILED' "$OSQUERY_DELIVERY_LOG"
  wait_for_log_line 'FAILED|lost|could not' "$ALERTER_LOG"
}

@test "T-DISP-store-atomic: a stored page is written atomically and drains on recovery (R2-6)" {
  # A store file appears (delivery failed) and drains clean once curl recovers,
  # proving the temp+rename path produces a well-formed, replayable entry (no torn
  # temp left behind).
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail body" "Sosumi" "occ:1:2"
  assert_undelivered_alert_count 1
  [[ -z "$(find "$OSQUERY_UNDELIVERED_ALERTS_DIR" -name '*.tmp.*' 2>/dev/null)" ]] # no torn temp file left
  set_curl_codes 200
  retry_undelivered_alerts
  assert_undelivered_alert_count 0
}

# --- digest/heartbeat traffic DOES reach the remote POST; mark its tier (R2-11) ---
# The dispatcher POSTs for severity==CRIT, and the digest and heartbeat both
# dispatch at CRIT, so they DO reach the remote POST. The `sound` arg only muted
# the LOCAL notifier; the POST carried no tier, so a silent digest looked
# identical to a real page on the wire. Thread an explicit tier the Hermes adapter
# can map to a suppressed notification: a page (non-empty sound) is tier=page, a
# muted digest/heartbeat (empty sound) is tier=muted. Both still POST (both CRIT).

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
  [[ $send_status -eq 0 ]] # the record is retained, so this is not a hard failure
  assert_no_post
  assert_undelivered_alert_count 1
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
