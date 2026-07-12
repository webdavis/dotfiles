#!/usr/bin/env bats
# osquery-allowlist.sh — the ONE writer for the launchd page-allowlist (-a allow,
# -d deny, -l list). It is the security boundary every caller (manual curation, the
# PR#2 button bot, the /osquery skill) goes through, so validation is the test focus.

load ../fixtures/osquery-alerter-lib

setup() { setup_allowlist_harness; }
teardown() { teardown_allowlist_harness; }

@test "T-AL-regex-at: a real @-bearing label is accepted" {
  run run_allowlist -a 'homebrew.mxcl.postgresql@17'
  [ "$status" -eq 0 ]
  assert_allowlisted 'homebrew.mxcl.postgresql@17'
}

@test "T-AL-reject-junk: malformed or system labels are refused, nothing appended" {
  for junk in '*' '../etc' '' 'com foo' 'com.apple.Finder' 'COM.APPLE.Finder' 'com.apple'; do
    run run_allowlist -a "$junk"
    [ "$status" -ne 0 ]
  done
  assert_allowlist_label_count 0
}

@test "T-AL-exact: dedup is exact full-line, not prefix (com.foo vs com.foobar)" {
  run_allowlist -a com.foobar
  run_allowlist -a com.foo
  assert_allowlisted com.foobar
  assert_allowlisted com.foo
  assert_allowlist_label_count 2
}

@test "T-AL-dedup: adding an existing label is a no-op (one line)" {
  run_allowlist -a com.foo.agent
  run_allowlist -a com.foo.agent
  assert_allowlist_label_count 1
}

@test "T-AL-deny: -d removes an allowed label" {
  run_allowlist -a com.foo.agent
  assert_allowlisted com.foo.agent
  run run_allowlist -d com.foo.agent
  [ "$status" -eq 0 ]
  assert_not_allowlisted com.foo.agent
  assert_allowlist_label_count 0
}

@test "T-AL-list: -l prints the current labels" {
  run_allowlist -a com.foo.agent
  run_allowlist -a com.bar.agent
  run run_allowlist -l
  [ "$status" -eq 0 ]
  [[ "$output" == *com.foo.agent* ]]
  [[ "$output" == *com.bar.agent* ]]
}
