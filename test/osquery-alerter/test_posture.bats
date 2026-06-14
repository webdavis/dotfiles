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
  assert_digest_count 0
}

@test "T-PAGE-sharing: a high-risk remote-access service turning on pages" {
  # Rebuilt from the dead log-only detector: the query emits a row per ENABLED
  # high-risk sharing service (screen sharing / remote management / etc.), so a new
  # row = an ON transition. SSH/Remote Login is the operator's own path → excluded.
  run_alerter "$(row pack_security-policy-regression_remote_access_sharing_state added 1 '{"service":"screen_sharing"}')"
  assert_page_has screen_sharing
  assert_digest_count 0
}

@test "T-PAGE-filevault-off: a genuine FileVault-off (differential filevault_off) pages (issue #18)" {
  # filevault_off emits one constant row only when NO APFS volume is encrypted. As a
  # differential query its off-row is logged "added" to results.log — the path the
  # alerter reads — so a real disable now reaches #priority (the false-NEGATIVE half
  # of #18: as a snapshot it went to snapshots.log, which the alerter never reads).
  run_alerter "$(row pack_security-policy-regression_filevault_off added 1 '{"protection":"filevault"}')"
  assert_page_has "FileVault turned OFF"
  assert_digest_count 0
}

@test "T-NEG-filevault-churn: a removed filevault_state row (APFS volume churn) does NOT page (issue #18)" {
  # The 2026-06-02 incident: a FileVault-on APFS volume (/dev/disk3s1, filevault_status
  # "on", encrypted "1") left the differential set while the data volume stayed
  # encrypted. A removed filevault_state row must NOT be read as FileVault-off — that
  # was the false-POSITIVE half of #18. It now falls through to NOTICE (log-only).
  run_alerter "$(row pack_security-policy-regression_filevault_state removed 1 '{"name":"/dev/disk3s1","filevault_status":"on","encryption_status":"encrypted","encrypted":"1"}')"
  assert_no_page
  assert_digest_count 0
}
