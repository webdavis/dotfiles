#!/usr/bin/env bats
# Task 1 — the three-outcome gate (page / digest / log-only).

load ../fixtures/osquery-alerter-lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-admin: a new admin user pages with an actionable block, never digests" {
  run_alerter "$(row new_admin_user added 1 '{"username":"backdoor","uid":"503"}')"
  assert_page_has "New administrator account" # the proper header, not the raw query name
  assert_page_has backdoor                    # the offending username
  assert_page_has "admin access"              # an actionable next step is present
  assert_digest_count 0                       # a page must never also land in the digest
}

@test "T-DIG-launchd-user: a new user LaunchAgent digests, does not page" {
  run_alerter "$(row persistence_launchd added 1 '{"label":"com.foo.agent","path":"/Users/x/Library/LaunchAgents/com.foo.agent.plist"}')"
  assert_no_page
  assert_digest_count 1
}

@test "T-DIG-launchd-allow: an allowlisted user LaunchAgent (full-tuple match) neither pages nor digests" {
  # R2-1: suppression is a TUPLE match now. A finding whose (label, path, program) matches an
  # allowlist entry is fully suppressed end to end through the gate. (The reused-label PAGE case
  # and the writer's capture live in osquery-alerter-persistence.bats / -allowlist.bats.)
  seed_allowlist_tuple com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo
  run_alerter "$(row persistence_launchd added 1 '{"label":"com.foo.agent","path":"/Users/x/Library/LaunchAgents/com.foo.agent.plist","program":"/opt/homebrew/opt/foo/bin/foo"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-PAGE-launchd-sysdaemon: a new system LaunchDaemon pages, does not digest" {
  run_alerter "$(row persistence_launchd added 1 '{"label":"com.evil.daemon","path":"/Library/LaunchDaemons/com.evil.daemon.plist"}')"
  assert_page_has com.evil.daemon
  assert_digest_count 0
}

@test "T-NEG-launchd-apple: a /System/Library LaunchDaemon is log-only (Apple churn)" {
  run_alerter "$(row persistence_launchd added 1 '{"label":"com.apple.foo","path":"/System/Library/LaunchDaemons/com.apple.foo.plist"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-NEG-launchd-daemon-removed: deleting a system LaunchDaemon does not page (good-news removed row)" {
  # Uninstalling a privileged helper (Docker, a VPN, any pkg) deletes its LaunchDaemon.
  # A removed row must NOT page "New startup item … likely malware" — only an added row.
  run_alerter "$(row persistence_launchd removed 1 '{"label":"com.docker.vmnetd","path":"/Library/LaunchDaemons/com.docker.vmnetd.plist"}')"
  assert_no_page
  assert_digest_count 0
}

@test "T-SEP-baseline: a digest detector at counter==0 neither pages nor floods the digest" {
  # The counter==0 discard runs BEFORE the gate, so a first-observation (baseline) row
  # of a digest detector cannot flood the daily digest on the first osqueryd run. The
  # discriminator is the counter, not the detector class: a real (counter>0) event of
  # the SAME detector does enter the store.
  run_alerter "$(row persistence_launchd added 0 '{"label":"com.baseline.agent","path":"/Users/x/Library/LaunchAgents/com.baseline.agent.plist"}')"
  assert_no_page
  assert_digest_count 0
  run_alerter "$(row persistence_launchd added 1 '{"label":"com.real.agent","path":"/Users/x/Library/LaunchAgents/com.real.agent.plist"}')"
  assert_no_page
  assert_digest_count 1
}
