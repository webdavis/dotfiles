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
