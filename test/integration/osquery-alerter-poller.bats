#!/usr/bin/env bats
# Security-posture poller: an OFF transition of the firewall, Gatekeeper, or screen lock pages
# #priority; a re-enable is silent. R2-3 folds screen-lock-off detection in here (the root daemon
# cannot read the user-scoped screenlock table). R2-9 validates every scalar: a partial/malformed
# reading is a MONITORING GAP (page, do not persist as safe), not a silent success.

load ../fixtures/osquery-alerter-lib

setup() { setup_poller_harness; }
teardown() { teardown_harness; }

@test "T-POLL-off: a firewall OFF transition pages CRIT" {
  run_poller '{"firewall":"1","gatekeeper":"1","screenlock":"1"}' '{"firewall":"0","gatekeeper":"1","screenlock":"1"}'
  assert_page_has "Firewall turned OFF"
}

@test "T-POLL-reenable-silent: a firewall re-enable produces no dispatch" {
  run_poller '{"firewall":"0","gatekeeper":"1","screenlock":"1"}' '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  assert_no_dispatch
}

@test "T-POLL-gk-off: a Gatekeeper OFF transition pages CRIT" {
  run_poller '{"firewall":"1","gatekeeper":"1","screenlock":"1"}' '{"firewall":"1","gatekeeper":"0","screenlock":"1"}'
  assert_page_has "Gatekeeper turned OFF"
}

@test "T-POLL-sl-off: a screen-lock OFF transition pages CRIT (R2-3, in the user-context poller)" {
  run_poller '{"firewall":"1","gatekeeper":"1","screenlock":"1"}' '{"firewall":"1","gatekeeper":"1","screenlock":"0"}'
  assert_page_has "Screen lock turned OFF"
}

@test "T-POLL-sl-reenable-silent: a screen-lock re-enable produces no dispatch" {
  run_poller '{"firewall":"1","gatekeeper":"1","screenlock":"0"}' '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  assert_no_dispatch
}

# FX6 + R2-3: an already-OFF protection at the first observation pages, never seeds silently.

@test "T-POLL-firstrun-healthy-silent: a healthy first sample seeds silently (mode 600)" {
  run_poller_firstrun '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  assert_no_dispatch
  assert_mode 600 "$POSTURE_STATE"
}

@test "T-POLL-firstrun-fw-off: a first sample with the firewall already OFF pages (FX6)" {
  run_poller_firstrun '{"firewall":"0","gatekeeper":"1","screenlock":"1"}'
  assert_page_has Firewall
  assert_page_has OFF
}

@test "T-POLL-firstrun-sl-off: a first sample with the screen lock already OFF pages (R2-3)" {
  run_poller_firstrun '{"firewall":"1","gatekeeper":"1","screenlock":"0"}'
  assert_page_has "Screen lock"
  assert_page_has OFF
}

@test "T-POLL-badperms-unsafe: an untrusted (non-600) baseline hiding a persistently-off firewall pages (FX6)" {
  run_poller_badperms '{"firewall":"0","gatekeeper":"1","screenlock":"1"}' '{"firewall":"0","gatekeeper":"1","screenlock":"1"}'
  assert_page_has Firewall
  assert_page_has OFF
}

# R2-9: validate every scalar. A partial reading is a monitoring gap — page, do not persist safe.

@test "T-POLL-gap-partial-pages: firewall='' gatekeeper=1 screenlock=1 is a GAP, pages, not persisted safe (R2-9)" {
  run_poller_firstrun '{"firewall":"","gatekeeper":"1","screenlock":"1"}'
  assert_page_has "gap"
  [ ! -f "$POSTURE_STATE" ]   # the partial reading was NOT persisted as a baseline
}

@test "T-POLL-gap-bad-scalar-pages: an out-of-range scalar (firewall=9) is a GAP, pages (R2-9)" {
  run_poller_firstrun '{"firewall":"9","gatekeeper":"1","screenlock":"1"}'
  assert_page_has "gap"
}

@test "T-POLL-gap-preserves-baseline: a gap does not overwrite a good prior baseline (R2-9)" {
  # Seed a healthy trusted baseline, then a partial reading arrives. The good baseline must
  # survive (so a later real transition is still detectable against it).
  run_poller '{"firewall":"1","gatekeeper":"1","screenlock":"1"}' '{"firewall":"","gatekeeper":"1","screenlock":"1"}'
  assert_page_has "gap"
  run jq -r '.firewall' "$POSTURE_STATE"
  [ "$output" = "1" ]   # the prior valid firewall state is preserved, not clobbered
}

@test "T-POLL-gap-once: a second consecutive gap does NOT re-page (page-once marker) (R2-9)" {
  run_poller_firstrun '{"firewall":"","gatekeeper":"1","screenlock":"1"}'
  assert_page_has "gap"
  : >"$SEND_ALERT_LOG"
  run_poller_firstrun '{"firewall":"","gatekeeper":"1","screenlock":"1"}'
  assert_no_dispatch
}

@test "T-POLL-gap-recovers: a valid sample after a gap clears the marker, so a later gap pages again (R2-9)" {
  run_poller_firstrun '{"firewall":"","gatekeeper":"1","screenlock":"1"}'   # gap (paged, marker set)
  run_poller_firstrun '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'  # valid → clears the marker
  : >"$SEND_ALERT_LOG"
  run_poller '{"firewall":"1","gatekeeper":"1","screenlock":"1"}' '{"firewall":"","gatekeeper":"1","screenlock":"1"}'
  assert_page_has "gap"   # the gap pages again after recovery
}
