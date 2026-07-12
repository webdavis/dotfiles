#!/usr/bin/env bats
# Firewall/Gatekeeper posture poller: an OFF transition pages #priority; a re-enable
# is silent (no #osquery channel exists in v2).

load ../fixtures/osquery-alerter-lib

setup() { setup_poller_harness; }
teardown() { teardown_harness; }

@test "T-POLL-off: a firewall OFF transition pages CRIT" {
  run_poller '{"firewall":"1","gatekeeper":"1"}' '{"firewall":"0","gatekeeper":"1"}'
  assert_page_has "Firewall turned OFF"
}

@test "T-POLL-reenable-silent: a firewall re-enable produces no dispatch" {
  run_poller '{"firewall":"0","gatekeeper":"1"}' '{"firewall":"1","gatekeeper":"1"}'
  assert_no_dispatch
}

@test "T-POLL-gk-off: a Gatekeeper OFF transition pages CRIT" {
  # The poller pages on a Gatekeeper-OFF transition via a distinct block from the
  # firewall one; both prior tests held gatekeeper at 1, so this arm was unexercised.
  run_poller '{"firewall":"1","gatekeeper":"1"}' '{"firewall":"1","gatekeeper":"0"}'
  assert_page_has "Gatekeeper turned OFF"
}

# FX6: the first sample was persisted silently even when a protection was already OFF,
# so a machine that booted with the firewall down was baselined as "normal" and never
# paged. Seed silently ONLY a healthy first sample; an unsafe first observation pages.

@test "T-POLL-firstrun-healthy-silent: a healthy first sample seeds silently (mode 600)" {
  run_poller_firstrun '{"firewall":"1","gatekeeper":"1"}'
  assert_no_dispatch
  assert_mode 600 "$POSTURE_STATE" # the baseline is written owner-only
}

@test "T-POLL-firstrun-fw-off: a first sample with the firewall already OFF pages (FX6)" {
  run_poller_firstrun '{"firewall":"0","gatekeeper":"1"}'
  assert_page_has Firewall
  assert_page_has OFF
}

@test "T-POLL-firstrun-gk-off: a first sample with Gatekeeper already OFF pages (FX6)" {
  run_poller_firstrun '{"firewall":"1","gatekeeper":"0"}'
  assert_page_has Gatekeeper
  assert_page_has OFF
}

@test "T-POLL-badperms-unsafe: an untrusted (non-600) baseline hiding a persistently-off firewall pages (FX6)" {
  # A group/world-readable state file is not trustworthy (it could be planted to mask a
  # disabled protection). Here the baseline AND the live state both say firewall OFF, so
  # the transition logic sees NO change and would stay silent forever. Rejecting the
  # untrusted baseline re-evaluates the live sample as a first observation → it pages.
  run_poller_badperms '{"firewall":"0","gatekeeper":"1"}' '{"firewall":"0","gatekeeper":"1"}'
  assert_page_has Firewall
  assert_page_has OFF
}
