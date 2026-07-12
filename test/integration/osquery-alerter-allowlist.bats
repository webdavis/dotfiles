#!/usr/bin/env bats
# osquery-allowlist.sh — the ONE writer for the launchd page-allowlist (-a allow,
# -d deny, -l list). It is the security boundary every caller (manual curation, the
# PR#2 button bot, the /osquery skill) goes through, so validation is the test focus.
# R2-1: an entry is now a TUPLE — `-a` captures the label's identity (plist path + program
# + plist sha256) from the launchd table, so the alerter suppresses a full-tuple match only.

load ../fixtures/osquery-alerter-lib

setup() { setup_allowlist_harness; }
teardown() { teardown_allowlist_harness; }

# A stub launchd row for the capture path: <path> <program>.
stub_launchd() {
  export ALLOWLIST_OSQUERYI_ROW="$(jq -cn --arg p "$1" --arg prog "$2" '[{path:$p, program:$prog}]')"
}

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

@test "T-AL-dedup: refreshing an existing label stays one entry" {
  run_allowlist -a com.foo.agent
  run_allowlist -a com.foo.agent
  assert_allowlist_label_count 1
}

@test "T-AL-capture: -a captures the label's identity tuple (path, program, sha256) from launchd (R2-1)" {
  local plist="$ALLOWLIST_HOME/com.foo.agent.plist"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo
  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ]
  # The stored tuple carries the captured path + program + the plist's real sha256.
  run jq -e --arg p "$plist" --arg prog /opt/homebrew/opt/foo/bin/foo --arg h "$(shasum -a 256 "$plist" | awk '{print $1}')" \
    'select(.label=="com.foo.agent" and .path==$p and .program==$prog and .sha256==$h)' "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ]
}

@test "T-AL-degraded: -a with no loaded LaunchAgent stores a label-only entry and warns (R2-1 fail-safe)" {
  # No launchd row for the label (the stub returns []): capture yields an empty path/program.
  # The entry is label-only (the alerter will NOT suppress on it), and the writer warns.
  run run_allowlist -a com.notloaded.agent
  [ "$status" -eq 0 ]
  [[ "$output" == *degraded* || "$output" == *"NOT be suppressed"* ]]
  run jq -e 'select(.label=="com.notloaded.agent" and .path=="" and .program=="")' "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ]
}

@test "T-AL-relativize: a captured \$HOME path is stored as ~/ (user-agnostic) (R2-1)" {
  local plist="$HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "/opt/homebrew/bin/bash $HOME/.local/bin/foo.sh"
  run_allowlist -a com.foo.agent
  # The stored path/program use ~/ , not the absolute home, so the file is portable.
  run jq -e 'select(.label=="com.foo.agent" and (.path|startswith("~/")) and (.program|contains("~/.local/bin/foo.sh")))' "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ]
  rm -f "$plist"
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
