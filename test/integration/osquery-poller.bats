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

@test "T-POLL-read-failure-preserves-baseline: a hard osqueryi failure exits 0 and leaves the good baseline byte-for-byte intact at 0600" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  export POLLER_OSQUERYI_EXIT=1 # osqueryi hard-fails: no output, non-zero exit

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0 on a failed read (retry next tick), got $status: $output"
    false
  }

  # A blind read must never overwrite a good baseline: it would blind or misfeed
  # the next run's comparison.
  assert_baseline_unchanged
  assert_mode 600 "$OSQUERY_POSTURE_STATE"
}

@test "T-POLL-empty-read-preserves-baseline: an empty osqueryi result exits 0 and leaves the good baseline byte-for-byte intact at 0600" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  set_posture '[]' # osqueryi succeeds but returns no row

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0 on an empty read, got $status: $output"
    false
  }

  # An empty read must not blank the baseline to a 1-byte file.
  assert_baseline_unchanged
  assert_mode 600 "$OSQUERY_POSTURE_STATE"
}

# --- B2: a protection turning OFF pages CRIT; steady state is silent -------------
# With a valid prior baseline, compare per protection and page on an OFF
# transition. Re-enable (OFF -> ON) is silent, matching c69baab.

@test "T-POLL-firewall-off-pages: firewall 1->0 pages one CRIT naming the firewall, queued BEFORE the baseline advances" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_page_count 1
  assert_page_severity_is CRIT # only a CRIT reaches the #priority webhook
  assert_page_body_has 'Firewall turned OFF'
  assert_page_body_lacks 'Gatekeeper' # only the protection that changed is named
  assert_page_body_lacks 'Screen lock'
  # Ordering (notify-before-persist): at page time the baseline still holds the
  # PRIOR value, so a crash before the advance re-detects and re-pages next tick.
  assert_page_saw_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  # Once the page is durably queued, the baseline advances to the observed OFF value.
  assert_baseline_scalar firewall 0
}

@test "T-POLL-page-failure-keeps-baseline: a send_alert failure does not advance the baseline, so the next tick re-detects and re-pages" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline
  export POLLER_SEND_ALERT_EXIT=1 # dispatch cannot durably queue the page
  set_posture '[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -ne 0 ]] || {
    echo "expected the poller to surface the send_alert failure (nonzero), got $status: $output"
    false
  }
  assert_page_count 1        # it DID attempt the page
  assert_baseline_unchanged  # but the baseline did NOT advance to the OFF value

  # Next tick: dispatch now succeeds. The still-ON baseline re-detects the OFF
  # transition and re-pages (at-least-once), then advances. Nothing was lost.
  export POLLER_SEND_ALERT_EXIT=0
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the retry to exit 0, got $status: $output"
    false
  }
  assert_page_count 2 # re-detected and re-paged, never silently lost
  assert_baseline_scalar firewall 0
}

@test "T-POLL-firewall-blockall-off-pages: firewall 2->0 (block-all to off) pages CRIT naming the firewall" {
  seed_baseline '{"firewall":"2","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'Firewall turned OFF'
  assert_page_body_has 'on (block all)' # the Was: text comes from fw_to_text(2)
}

@test "T-POLL-gatekeeper-off-pages: gatekeeper 1->0 pages one CRIT naming Gatekeeper" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"1","gatekeeper":"0","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'Gatekeeper turned OFF'
  assert_page_body_lacks 'Firewall'
  assert_page_body_lacks 'Screen lock'
}

@test "T-POLL-screenlock-off-pages: screen-lock 1->0 pages one CRIT naming the screen lock" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"0"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'Screen lock turned OFF'
  assert_page_body_lacks 'Firewall'
  assert_page_body_lacks 'Gatekeeper'
}

@test "T-POLL-multi-off-one-page: two protections turning off in one tick page a single CRIT naming both" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"0","gatekeeper":"0","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_page_count 1 # one page for the tick, not one per protection
  assert_page_severity_is CRIT
  assert_page_body_has 'Firewall turned OFF'
  assert_page_body_has 'Gatekeeper turned OFF'
}

@test "T-POLL-steady-silent: an all-ON posture unchanged from a valid baseline pages nothing" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_no_page # steady state is silent even with a valid prior baseline
}

@test "T-POLL-reenable-silent: a re-enable (firewall 0->1) pages nothing, matching c69baab" {
  seed_baseline '{"firewall":"0","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_no_page # a protection turning back ON is not actionable and has no notice channel
}

# --- strict scalar validation: a partial/out-of-domain value is a failed read ----
# Each scalar must be in its exact domain (firewall 0/1/2, gatekeeper 0/1,
# screenlock 0/1). Applied to the current read (do not poison the baseline) and to
# the prior-baseline trust check (do not fabricate a transition).

@test "T-POLL-partial-read-preserves-baseline: a partial posture (missing a field) preserves the baseline, and does not poison the next comparison" {
  seed_baseline '{"firewall":"1","gatekeeper":"1","screenlock":"1"}'
  snapshot_baseline

  # Tick 1: the screenlock field is absent. That is a monitoring gap, not a
  # posture: it must NOT persist (which would poison the baseline) and must NOT page.
  set_posture '[{"firewall":"1","gatekeeper":"1"}]'
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0 on a partial read, got $status: $output"
    false
  }
  assert_baseline_unchanged
  assert_no_page

  # Tick 2: a healthy read with screenlock now OFF. Because tick 1 did not poison
  # the baseline, the prior is still the good all-ON, so screenlock 1->0 pages.
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"0"}]'
  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0 on tick 2, got $status: $output"
    false
  }
  assert_page_count 1
  assert_page_severity_is CRIT
  assert_page_body_has 'Screen lock turned OFF'
}

@test "T-POLL-out-of-domain-prior-not-trusted: an out-of-domain prior baseline value is distrusted, so no false transition pages" {
  # A prior firewall of "00" is not a valid domain value. Trusting it via a string
  # compare against a current "0" would read "00" != "0" and fabricate a CRIT.
  seed_baseline '{"firewall":"00","gatekeeper":"1","screenlock":"1"}'
  set_posture '[{"firewall":"0","gatekeeper":"1","screenlock":"1"}]'

  run run_poller
  [[ $status -eq 0 ]] || {
    echo "expected the poller to exit 0, got $status: $output"
    false
  }

  assert_no_page # the untrusted prior is treated as no-prior: seed silently, no false CRIT
}
