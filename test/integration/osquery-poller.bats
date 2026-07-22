#!/usr/bin/env bats
# The security-posture poller (executable_firewall-gatekeeper-monitor.sh) runs as a
# gui/501 user LaunchAgent every 60s. B1 scope: it reads the firewall, Gatekeeper,
# and screen-lock posture in ONE osqueryi query and persists it as an owner-only
# (0600) baseline, written before any notification. No transition paging yet (B2),
# no monitoring-gap logic yet (B3).
#
# R2-3: screen-lock lives HERE, not in the root-daemon pack. The screenlock osquery
# table is scoped to the logged-in user, so only this user-session poller can read
# it (the root daemon never returns a screenlock row).

load ../fixtures/osquery-poller-lib

setup() { setup_poller_harness; }
teardown() { teardown_poller_harness; }

@test "T-POLL-read-combined: one osqueryi query reads firewall, Gatekeeper, AND screen-lock together" {
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0 on a healthy read, got $status: $output"
    false
  }

  assert_osqueryi_call_count 1             # one combined query per tick, not one per protection
  assert_query_reads 'global_state'        # the firewall column (alf)
  assert_query_reads 'assessments_enabled' # the Gatekeeper column
  assert_query_reads 'screenlock'          # R2-3: the screen-lock read is IN the combined query
}

@test "T-POLL-persist-0600: the poller persists the posture as an owner-only (0600) baseline with no temp left behind" {
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  [[ -f $OSQUERY_POSTURE_STATE ]] || {
    echo "expected the baseline file at $OSQUERY_POSTURE_STATE, but it is missing"
    false
  }
  # Owner-only: a group/world-readable baseline could be planted to mask a
  # disabled protection, so the write must land at 0600.
  assert_mode 600 "$OSQUERY_POSTURE_STATE"
  [[ ! -e $OSQUERY_POSTURE_STATE.tmp ]] || {
    echo "expected the write_state temp file to be gone, but $OSQUERY_POSTURE_STATE.tmp remains"
    false
  }
}

@test "T-POLL-persist-scalars: the baseline carries the exact firewall, Gatekeeper, and screen-lock scalars it read" {
  set_posture '[{"firewall":"2","gatekeeper":"1","screenlock":"0"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_baseline_scalar firewall 2   # alf global_state 2 = block-all
  assert_baseline_scalar gatekeeper 1
  assert_baseline_scalar screenlock 0
}

@test "T-POLL-silent-first-run: a healthy first run persists the baseline and pages nothing, notifying only after the baseline exists" {
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0 on a healthy first run, got $status: $output"
    false
  }

  assert_no_page               # read+persist only: no transition paging in B1
  assert_persist_before_notify # any future page fires only AFTER the baseline is written
}
