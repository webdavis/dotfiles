#!/usr/bin/env bats
# Task 1 — the three-outcome gate (page / digest / log-only).

load lib

setup() { setup_harness; }
teardown() { teardown_harness; }

@test "T-PAGE-admin: a new admin user pages" {
  run_alerter "$(row new_admin_user added 1 '{"username":"backdoor","uid":"503"}')"
  assert_page_has backdoor
}

@test "T-DIG-launchd-user: a new user LaunchAgent digests, does not page" {
  run_alerter "$(row persistence_launchd added 1 '{"label":"com.foo.agent","path":"/Users/x/Library/LaunchAgents/com.foo.agent.plist"}')"
  assert_no_page
  assert_digest_count 1
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
