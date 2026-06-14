#!/usr/bin/env bats
# Log-only / no-deliver tier: retained in results.log, never paged, never digested.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-LOG-sip-no-page: a sip_state OFF row never pages (both name variants)" {
  # SIP is intentionally off on this host, so an on->off transition cannot occur;
  # the snapshot floor would otherwise page forever. The vestigial stale-name
  # variant must be handled identically, not silently classified elsewhere.
  run_alerter "$(row pack_security-policy-regression_sip_state added 1 '{"enabled":"0"}')"
  assert_no_page
  run_alerter "$(row pack_security-regression_sip_state added 1 '{"enabled":"0"}')"
  assert_no_page
  assert_digest_count 0
}
