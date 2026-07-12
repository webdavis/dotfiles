#!/usr/bin/env bats
# Cursor integrity (R2-2). A missing/malformed cursor made the alerter write inode+EOF and
# exit WITHOUT parsing, so an entire queued batch (including an unsafe row) was silently
# skipped - deleting the cursor was a way to suppress alerts. A missing/corrupt cursor is an
# ALERTING FAILURE: process the recent tail (never seek-to-EOF), and emit a LOUD warning that
# the cursor was reset. The checkpoint advances only AFTER the batch is durably handled.

load ../fixtures/osquery-alerter-lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-CURSOR-missing-processes: a missing cursor processes the queued unsafe row, not skips it (R2-2)" {
  # filevault_off counter==0 = FileVault already off at first sight (an unsafe absolute state).
  # With the old seek-to-EOF, this queued row was written past and never paged.
  run_alerter_cursor missing "$(row pack_security-policy-regression_filevault_off added 0 '{"protection":"filevault"}')"
  assert_page_has "FileVault turned OFF"
}

@test "T-CURSOR-corrupt-processes: a corrupt cursor processes the queued unsafe row, not skips it (R2-2)" {
  run_alerter_cursor corrupt "$(row pack_security-policy-regression_filevault_off added 0 '{"protection":"filevault"}')"
  assert_page_has "FileVault turned OFF"
}

@test "T-CURSOR-missing-warns: a missing cursor emits a LOUD reset warning (R2-2)" {
  # The operator must learn the cursor was reset - a cursor deletion could otherwise hide a
  # skipped batch. The warning is loud (a real sound), distinct from the batch page.
  run_alerter_cursor missing "$(row pack_security-policy-regression_filevault_off added 0 '{"protection":"filevault"}')"
  assert_page_has "cursor"      # a reset warning was dispatched
  # loud: the reset warning carries a non-empty sound (it is not a muted note)
  run grep -c $'^CRIT\t.*[Cc]ursor.*\tSosumi$' "$SEND_ALERT_LOG"
  [ "$output" -ge 1 ]
}

@test "T-CURSOR-corrupt-warns: a corrupt cursor also emits the reset warning (R2-2)" {
  run_alerter_cursor corrupt "$(row pack_security-policy-regression_filevault_off added 0 '{"protection":"filevault"}')"
  assert_page_has "cursor"
}

@test "T-CURSOR-valid-no-warn: a valid cursor does NOT emit a reset warning (R2-2 no false alarm)" {
  # The steady-state path (a valid cursor) must stay quiet about the cursor - only a real
  # reset warns, so the warning does not cry wolf on every ordinary run.
  run_alerter "$(row pack_security-policy-regression_filevault_off added 1 '{"protection":"filevault"}')"
  assert_page_has "FileVault turned OFF"
  ! grep -qiF 'cursor' "$SEND_ALERT_LOG"
}
