#!/usr/bin/env bats
# Per-query baseline policy (FX5). The old unconditional counter==0 discard silently
# ACCEPTED a pre-existing compromise: an admin already added, sharing already enabled,
# a listener already exposed, FileVault already off — all seed silently on the first
# osqueryd run. The fix keeps the silent seed only for MEMBERSHIP queries (a diff
# against a baseline set); ABSOLUTE-STATE queries emit a row ONLY when the current state
# is unsafe, so a counter==0 row is an unsafe FIRST observation that must PAGE.

load ../fixtures/osquery-alerter-lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-BASE-membership-silent: a counter==0 new_admin_user row seeds silently (membership)" {
  # The admin group's baseline (root + the operator) is a first-observation membership
  # row; it must NOT page — only a later differential (a newly-added admin) pages.
  run_alerter "$(row new_admin_user added 0 '{"username":"stephen","uid":"501"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-BASE-filevault-unsafe: a counter==0 filevault_off row pages (already off at baseline)" {
  # filevault_off emits one row ONLY when NO volume is encrypted. A first-observation
  # row therefore means FileVault was ALREADY off when monitoring started — an unsafe
  # pre-existing state, not a benign baseline. It must page, not seed silently.
  run_alerter "$(row pack_security-policy-regression_filevault_off added 0 '{"protection":"filevault"}')"
  assert_page_has "FileVault turned OFF"
  assert_digest_count 0
}

@test "T-BASE-sharing-unsafe: a counter==0 remote_access_sharing_state row pages (already enabled)" {
  # A row per ENABLED high-risk service; a counter==0 row means the service was already
  # on when monitoring started — a pre-existing remote-control path, must page.
  run_alerter "$(row pack_security-policy-regression_remote_access_sharing_state added 0 '{"service":"screen_sharing"}')"
  assert_page_has screen_sharing
  assert_digest_count 0
}

@test "T-BASE-exposure-unsafe: a counter==0 agent_exposure_changed row pages (already exposed)" {
  # A row per off-loopback agent listener; a counter==0 row means the port was already
  # exposed off-box at baseline — a pre-existing exposure, must page.
  run_alerter "$(row agent_exposure_changed added 0 '{"address":"0.0.0.0","port":"8000","name":"workspace-mcp"}')"
  assert_page_has 8000
  assert_digest_count 0
}
