#!/usr/bin/env bats
# Shared dispatcher (osquery-alert-dispatch.sh): v2 delivers ONLY a CRIT page to the
# #priority webhook. Any other severity does the local notification and never POSTs —
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
  # inside the signed bytes — the master-spec body shape and the homelab-migration
  # seam both require {event_type, host, alert:{...}}.
  run grep -F '"host":"' "$CURL_LOG"
  [ "$status" -eq 0 ]
}

@test "T-DISP-nosecret-spool: a CRIT with no webhook secret spools the page and loudly names the broken channel (FX4)" {
  # The old code logged a WARN and returned SUCCESS without spooling — the critical
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
  grep -qiE 'secret|Discord|broken|deliver' "$ALERTER_LOG" # loud local notice names the channel
}
