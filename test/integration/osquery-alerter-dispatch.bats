#!/usr/bin/env bats
# Shared dispatcher (osquery-alert-dispatch.sh): v2 delivers ONLY a CRIT page to the
# #priority webhook. Any other severity does the local notification and never POSTs -
# there is no #osquery channel for a producer to leak to.

load ../fixtures/osquery-alerter-lib

setup() { setup_dispatch_harness; }
teardown() { teardown_harness; }

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
  # The curl shim records the full invocation including --data <body>; host must be
  # inside the signed bytes - the master-spec body shape and the homelab-migration
  # seam both require {event_type, host, alert:{...}}.
  run grep -F '"host":"' "$CURL_LOG"
  [ "$status" -eq 0 ]
}

@test "T-DISP-nosecret-spool: a CRIT with no webhook secret spools the page and loudly names the broken channel (FX4)" {
  # The old code logged a WARN and returned SUCCESS without spooling - the critical
  # silently degraded to local-only and was lost. With no secret the page must be spooled
  # durably (it delivers when the secret returns) and a LOUD local notification must name
  # the broken channel; nothing may be signed or POSTed without a key.
  unset OSQUERY_WEBHOOK_SECRET
  : >"$ALERTER_LOG"
  run send_alert CRIT "🔴 title" "detail body" "Sosumi"
  [ "$status" -eq 0 ]                       # still fire-and-forget for its callers
  assert_spool_count 1                      # durably spooled, not dropped
  assert_no_post                            # nothing signed/POSTed without a key
  grep -qiE 'secret|degraded|broken' "$OSQUERY_DELIVERY_LOG"
  # The loud local notice fires via a backgrounded alerter (& so it never blocks dispatch), so
  # poll rather than grep once - a single immediate grep races the stub's append under parallel CI.
  wait_for_log_match 'secret|Discord|broken|deliver' "$ALERTER_LOG" # loud local notice names the channel
}

# --- R2-4: occurrence-identity request id + occurrence-unique spool filenames --------
# request_id was sha256(body): two DISTINCT incidents with the same body collapsed to one
# id, so the gateway deduped them for 1h AND the second overwrote the first in the spool
# (the filename is the id). Repeated authorized-key / firewall / funnel incidents vanished.
# The id must derive from OCCURRENCE IDENTITY (threaded from the caller), reused only for a
# retry of that same occurrence, so two same-body incidents survive as two spool files.

@test "T-DISP-occurrence-distinct: two same-body incidents at different occurrences spool as TWO files (R2-4)" {
  # Force delivery failure (three 5xx per send) so both pages spool; pass a DISTINCT
  # occurrence identity per incident. Same title/detail (same body), different occurrence.
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:200:300"
  assert_spool_count 2   # both incidents survive - a later same-body incident does not clobber the earlier
}

@test "T-DISP-occurrence-retry-stable: the SAME occurrence retried reuses one id (idempotent spool) (R2-4)" {
  # A retry of the SAME occurrence (same identity) must reuse the id so the gateway dedups
  # it and the spool file is idempotent - one file, not a growing pile of duplicates.
  set_curl_codes 503 503 503 503 503 503
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  send_alert CRIT "🔴 firewall" "Firewall turned OFF" "Sosumi" "inode7:100:200"
  assert_spool_count 1
}

# --- R2-6: page-spool failure is a HARD failure, not a silent success ----------------
# _spool_page did not make persistence success part of its result: an unwritable spool
# path returned rc=0 while no file was written. Callers logged "spooled" and advanced
# state though nothing was delivered OR stored. Spool through a checked temp+rename and
# return nonzero on any persistence failure; the caller must treat it as a hard failure.

@test "T-DISP-spool-hardfail: an unwritable spool dir + no secret returns nonzero and loudly alerts (R2-6)" {
  # Point the spool at a path whose parent is a FILE, so mkdir/create cannot succeed for
  # any uid (deterministic, not permission/uid-dependent). With no secret the page cannot
  # be signed either - it can be neither delivered NOR stored, which MUST be loud + nonzero.
  unset OSQUERY_WEBHOOK_SECRET
  : >"$ALERTER_LOG"
  touch "$HARNESS_HOME/notadir"
  export OSQUERY_SPOOL_DIR="$HARNESS_HOME/notadir/spool"
  run send_alert CRIT "🔴 title" "detail body" "Sosumi"
  [ "$status" -ne 0 ]                        # hard delivery failure - NOT a silent success
  assert_spool_count 0                       # nothing was stored
  assert_no_post                             # nothing signed/POSTed without a key
  grep -qiE 'SPOOL-FAILED' "$OSQUERY_DELIVERY_LOG"        # synchronous: the hard failure is recorded
  wait_for_alerter 'FAILED|lost|could not'               # the loud local alert fired (fire-and-forget)
}

@test "T-DISP-spool-hardfail-delivery: delivery failure + unwritable spool returns nonzero and loudly alerts (R2-6)" {
  # Secret present, delivery fails (5xx x3), AND the spool is unwritable → neither
  # delivered nor stored → hard failure (nonzero) + loud local alert.
  : >"$ALERTER_LOG"
  set_curl_codes 503 503 503
  touch "$HARNESS_HOME/notadir2"
  export OSQUERY_SPOOL_DIR="$HARNESS_HOME/notadir2/spool"
  run send_alert CRIT "🔴 title" "detail body" "Sosumi"
  [ "$status" -ne 0 ]
  assert_spool_count 0
  grep -qiE 'SPOOL-FAILED' "$OSQUERY_DELIVERY_LOG"        # synchronous: the hard failure is recorded
  wait_for_alerter 'FAILED|lost|could not'               # the loud local alert fired (fire-and-forget)
}

@test "T-DISP-spool-atomic: a spooled page is written atomically and drains on recovery (R2-6)" {
  # A spool file appears (delivery failed) and drains clean once curl recovers - proving
  # the temp+rename path produces a well-formed, replayable entry (no torn temp left behind).
  set_curl_codes 503 503 503
  send_alert CRIT "🔴 title" "detail body" "Sosumi" "occ:1:2"
  assert_spool_count 1
  [ -z "$(find "$OSQUERY_SPOOL_DIR" -name '*.tmp.*' 2>/dev/null)" ]  # no torn temp file left
  set_curl_codes 200
  run_drain
  assert_spool_count 0
}

# --- R2-11: digest/heartbeat traffic DOES reach the remote POST; mark its tier --------
# The dispatcher POSTs for severity==CRIT, and the digest AND heartbeat both dispatch at
# CRIT (verified), so they DO reach the remote POST - this is NOT moot. The `sound` arg
# only muted the LOCAL notifier; the POST carried no tier, so a silent digest looked
# identical to a real page on the wire. Thread an explicit tier the Hermes adapter can map
# to a suppressed notification: a page (non-empty sound) is tier=page, a muted digest/
# heartbeat (empty sound) is tier=muted. Both still POST (both CRIT).

@test "T-DISP-tier-page: a real page (non-empty sound) POSTs tier=page (R2-11)" {
  send_alert CRIT "🔴 title" "detail" "Sosumi"
  assert_post_count 1
  grep -qF '"tier":"page"' "$CURL_LOG"
}

@test "T-DISP-tier-muted: a muted digest/heartbeat (empty sound) POSTs tier=muted, not page (R2-11)" {
  # A digest reaches the remote POST (it is CRIT) but must be distinguishable from a page:
  # the body carries tier=muted so the Hermes adapter can suppress the notification.
  send_alert CRIT "🗒️ daily digest" "detail" ""
  assert_post_count 1                        # it DOES reach the remote POST (not moot)
  grep -qF '"tier":"muted"' "$CURL_LOG"
  ! grep -qF '"tier":"page"' "$CURL_LOG"
}
