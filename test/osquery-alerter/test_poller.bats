#!/usr/bin/env bats
# Firewall/Gatekeeper posture poller: an OFF transition pages #priority; a re-enable
# is silent (no #osquery channel exists in v2).

load lib

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
