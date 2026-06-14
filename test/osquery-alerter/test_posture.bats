#!/usr/bin/env bats
# Posture page tier. Characterization lock: suid still pages via the alerter's
# pre-gate (v1) classification rather than an explicit gate arm, so this test is a
# tripwire — a future gate refactor that silently dropped it (privilege escalation
# is exactly the page you cannot afford to lose) would turn this red.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-suid: a new unexpected setuid-root binary pages" {
  run_alerter "$(row pack_intrusion-detection_suid_bin_unexpected added 1 '{"path":"/Users/x/.local/bin/backdoor","username":"root","permissions":"rwsr-xr-x"}')"
  assert_page_has backdoor
}

@test "T-PAGE-sharing: a high-risk remote-access service turning on pages" {
  # Rebuilt from the dead log-only detector: the query emits a row per ENABLED
  # high-risk sharing service (screen sharing / remote management / etc.), so a new
  # row = an ON transition. SSH/Remote Login is the operator's own path → excluded.
  run_alerter "$(row pack_security-policy-regression_remote_access_sharing_state added 1 '{"service":"screen_sharing"}')"
  assert_page_has screen_sharing
}
