#!/usr/bin/env bats
# file_events page tier: a written SSH key or sshd_config is remote-auth tampering
# and pages. Only CREATED/UPDATED is actionable — a DELETE is your own revert noise.

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-authkeys: an authorized_keys file CREATED pages" {
  run_alerter "$(file_event_row authorized_keys /Users/x/.ssh/authorized_keys CREATED)"
  assert_page_has authorized_keys
  assert_digest_count 0
}

@test "T-NEG-authkeys-delete: an authorized_keys DELETE does not page" {
  run_alerter "$(file_event_row authorized_keys /Users/x/.ssh/authorized_keys DELETED)"
  assert_no_page
  assert_digest_count 0 # a delete is revert noise — pin the no-digest plane like its siblings
}

@test "T-PAGE-sshd: an sshd_config UPDATED pages" {
  run_alerter "$(file_event_row sshd_config /etc/ssh/sshd_config UPDATED)"
  assert_page_has sshd_config
  assert_page_has UPDATED # the real FSEvents verb, not the constant outer "added"
  assert_digest_count 0
}

@test "T-PAGE-pipeline-mismatch: a tooling change whose hash is NOT in the manifest pages" {
  # The alerter's own scripts/plists. Legitimacy = content matches the source-derived,
  # root-owned manifest; any other content (tamper) pages. Never digests (page or silent).
  seed_manifest "aaaa1111  /Users/x/.local/bin/osquery-results-alerter.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-results-alerter.sh UPDATED novelhash9999)"
  assert_page_has osquery-results-alerter.sh
  assert_digest_count 0
}

@test "T-NEG-pipeline-match: a change whose hash IS in the manifest is silent (legit apply)" {
  seed_manifest "goodhash1234  /Users/x/.local/bin/osquery-results-alerter.sh"
  run_alerter "$(file_event_row pipeline_integrity /Users/x/.local/bin/osquery-results-alerter.sh UPDATED goodhash1234)"
  assert_no_page
  assert_digest_count 0
}

@test "T-DIG-allowlist-file: editing the page-allowlist (allowlist_file) digests, never pages" {
  # The page-launchd-allowlist suppresses pages, so editing it is security-relevant —
  # but it is runtime-mutable curation, not an intrusion, so it digests for the daily
  # review rather than paging. (Guards the 'a page-suppressor was edited' path.)
  run_alerter "$(file_event_row allowlist_file /Users/x/.config/osquery/page-launchd-allowlist.txt UPDATED)"
  assert_no_page
  assert_digest_count 1
}
