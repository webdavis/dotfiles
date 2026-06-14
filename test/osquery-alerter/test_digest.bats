#!/usr/bin/env bats
# Digest/suspicious tier: useful-but-not-rare findings accumulate in the digest
# store for a once-daily grouped summary instead of paging.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-DIG-sysext: a new system extension digests, does not page" {
  run_alerter "$(row pack_intrusion-detection_system_extensions_new added 1 '{"identifier":"io.tailscale.ipn","team":"W5364U7YZB"}')"
  assert_no_page
  assert_digest_count 1
}

@test "T-DIG-sudoers: a sudoers change digests, does not page" {
  # sudoers churns far more than sshd_config (visudo / chezmoi atomic writes:
  # 19 real events vs 1). It belongs in the daily digest, not an interruption.
  run_alerter "$(file_event_row sudoers /private/etc/sudoers.d/foo UPDATED)"
  assert_no_page
  assert_digest_count 1
}

@test "T-DIG-screenlock: a screenlock OFF row digests, does not page" {
  # Posture drift (low actionability). Delivery on Dresden still pending the
  # "does the query actually emit?" confirmation (tier matrix row 16).
  run_alerter "$(row pack_security-policy-regression_screenlock_state added 1 '{"enabled":"0"}')"
  assert_no_page
  assert_digest_count 1
}
