#!/usr/bin/env bats
# Posture page tier. Characterization lock: an ADDED suid still pages via the alerter's
# pre-gate (v1) CRIT classification - its gate arm only drops the good-news removed row
# - so this test is a tripwire: a future refactor that silently dropped the suid page
# (privilege escalation is exactly the page you cannot afford to lose) would turn red.

load ../fixtures/osquery-alerter-lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-suid: a new unexpected setuid-root binary pages" {
  run_alerter "$(row pack_intrusion-detection_suid_bin_unexpected added 1 '{"path":"/Users/x/.local/bin/backdoor","username":"root","permissions":"rwsr-xr-x"}')"
  assert_page_has backdoor
  assert_digest_count 0
}

@test "T-NEG-suid-removed: deleting a setuid-root binary does not page (good-news removed row)" {
  # A setuid binary being removed is the threat going AWAY; it must not page
  # "New setuid root binary … a backdoor" for the good-news direction.
  run_alerter "$(row pack_intrusion-detection_suid_bin_unexpected removed 1 '{"path":"/Users/x/.local/bin/oldtool","username":"root","permissions":"rwsr-xr-x"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-PAGE-cap: a large simultaneous-CRIT batch caps the page body with an overflow marker" {
  # Nine CRIT findings at once would exceed Discord's 2000-char limit and get stuck
  # undelivered in the spool; the page caps at eight blocks plus a marker instead.
  local rows=() i
  for i in $(seq 1 9); do
    rows+=("$(row pack_intrusion-detection_suid_bin_unexpected added 1 "{\"path\":\"/Users/x/.local/bin/tool$i\",\"username\":\"root\"}")")
  done
  run_alerter "$(printf '%s\n' "${rows[@]}")"
  assert_page_has "more CRITICAL finding(s)"
  local body
  body=$(grep $'^CRIT\t' "$SEND_ALERT_LOG" | cut -f3)
  [ "${#body}" -lt 2000 ]                                            # the load-bearing Discord bound
  [ "$(grep -oF 'New setuid root binary' <<<"$body" | wc -l)" -eq 8 ] # exactly eight blocks shown
}

@test "T-PAGE-len-cap: the rendered page is capped under 2000 chars even at max blocks (FX8)" {
  # The eight-block cap bounds COUNT, not length: eight blocks with long fields can still
  # exceed Discord's 2000-char limit, and an over-length POST is rejected and re-spooled
  # forever. A final length cap after rendering guarantees the body fits.
  local rows=() i big
  big=$(printf 'B%.0s' $(seq 1 300))
  for i in $(seq 1 8); do
    rows+=("$(row new_admin_user added 1 "$(jq -cn --arg u "$big$i" '{username:$u,uid:"5"}')")")
  done
  run_alerter "$(printf '%s\n' "${rows[@]}")"
  local body
  body=$(grep $'^CRIT\t' "$SEND_ALERT_LOG" | cut -f3)
  [ "${#body}" -lt 2000 ] # the load-bearing Discord bound
  grep -qF truncated <<<"$body"
}

@test "T-PAGE-sharing: a high-risk remote-access service turning on pages" {
  # Rebuilt from the dead log-only detector: the query emits a row per ENABLED
  # high-risk sharing service (screen sharing / remote management / etc.), so a new
  # row = an ON transition. SSH/Remote Login is the operator's own path → excluded.
  run_alerter "$(row pack_security-policy-regression_remote_access_sharing_state added 1 '{"service":"screen_sharing"}')"
  assert_page_has screen_sharing
  assert_digest_count 0
}

@test "T-NEG-sharing-off: a remote-access service turning OFF (removed row) does not page" {
  # The query emits a row per ENABLED service, so a removed row = a service turning
  # OFF (good news). Only an ON transition (added) is page-worthy; a removed row must
  # not page "service enabled" for a service that just got disabled.
  run_alerter "$(row pack_security-policy-regression_remote_access_sharing_state removed 1 '{"service":"screen_sharing"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-PAGE-filevault-off: a genuine FileVault-off (differential filevault_off) pages (issue #18)" {
  # filevault_off emits one constant row only when NO APFS volume is encrypted. As a
  # differential query its off-row is logged "added" to results.log - the path the
  # alerter reads - so a real disable now reaches #priority (the false-NEGATIVE half
  # of #18: as a snapshot it went to snapshots.log, which the alerter never reads).
  run_alerter "$(row pack_security-policy-regression_filevault_off added 1 '{"protection":"filevault"}')"
  assert_page_has "FileVault turned OFF"
  assert_digest_count 0
}

@test "T-NEG-filevault-churn: a removed filevault_state row (APFS volume churn) does NOT page (issue #18)" {
  # The 2026-06-02 incident: a FileVault-on APFS volume (/dev/disk3s1, filevault_status
  # "on", encrypted "1") left the differential set while the data volume stayed
  # encrypted. A removed filevault_state row must NOT be read as FileVault-off - that
  # was the false-POSITIVE half of #18. It now falls through to NOTICE (log-only).
  run_alerter "$(row pack_security-policy-regression_filevault_state removed 1 '{"name":"/dev/disk3s1","filevault_status":"on","encryption_status":"encrypted","encrypted":"1"}')"
  assert_no_page
  assert_digest_count 0
}
