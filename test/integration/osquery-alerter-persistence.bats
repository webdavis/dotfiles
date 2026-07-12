#!/usr/bin/env bats
# Persistence allowlist as a TUPLE (R2-1). The old code suppressed a persistence_launchd
# finding on the launchd LABEL alone, before resolving the plist path / program, so an
# attacker who reused an allowlisted label but pointed the plist at a malicious program was
# SILENTLY suppressed (neither paged nor digested). The allowlist now binds a label to its
# known-good identity (canonical plist path + program [+ pinned plist hash]); a finding is
# suppressed ONLY on a full-tuple match, and a reused label with a different path/program/hash
# PAGES.

load ../fixtures/osquery-alerter-lib

setup() { setup_harness; }
teardown() { teardown_harness; }

# The R2-1 repro row: an "added" user LaunchAgent for an allowlisted label.
persist_row() { # persist_row <label> <path> <program>
  row persistence_launchd added 1 "$(jq -cn --arg l "$1" --arg p "$2" --arg prog "$3" \
    '{label:$l, path:$p, program:$prog}')"
}

@test "T-PERSIST-tuple-match-suppress: an allowlisted label with the SAME identity is fully suppressed" {
  seed_allowlist_tuple com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo
  run_alerter "$(persist_row com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo)"
  assert_no_page
  assert_digest_count 0   # fully suppressed - neither page nor digest
}

@test "T-PERSIST-tuple-reuse-program-pages: an allowlisted LABEL reused with a different program PAGES (R2-1)" {
  # The exact attack: same label, canonical plist path, but the plist now runs /tmp/evil.
  # Label-only matching silently suppressed this; tuple matching PAGES it.
  seed_allowlist_tuple com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo
  run_alerter "$(persist_row com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /tmp/evil)"
  assert_page_has com.foo.agent
  assert_digest_count 0
}

@test "T-PERSIST-tuple-reuse-path-pages: an allowlisted LABEL at a different plist path PAGES (R2-1)" {
  seed_allowlist_tuple com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo
  run_alerter "$(persist_row com.foo.agent /Users/x/Library/LaunchAgents/evil/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo)"
  assert_page_has com.foo.agent
}

@test "T-PERSIST-not-allowlisted-digests: an unknown user LaunchAgent digests (unchanged)" {
  run_alerter "$(persist_row com.unknown.agent /Users/x/Library/LaunchAgents/com.unknown.agent.plist /opt/homebrew/bin/thing)"
  assert_no_page
  assert_digest_count 1
}

@test "T-PERSIST-label-only-entry-does-not-suppress: a degraded label-only entry cannot vouch, so a finding is NOT suppressed (R2-1 fail-safe)" {
  # A label-only entry (no captured identity) must not suppress on the bare label - that IS the
  # R2-1 bug. It fails safe: the finding routes by its tier (digest) instead of vanishing.
  seed_allowlist_tuple com.foo.agent "" ""
  run_alerter "$(persist_row com.foo.agent /Users/x/Library/LaunchAgents/com.foo.agent.plist /opt/homebrew/opt/foo/bin/foo)"
  assert_no_page
  assert_digest_count 1
}

@test "T-PERSIST-tilde-tuple-suppress: a stored ~/ home-relative tuple matches the absolute finding path" {
  # Committed self-agent entries store ~/ so the file stays user-agnostic; the reader expands
  # ~/ to \$HOME/ before comparing to osquery's absolute path/program.
  seed_allowlist_tuple com.webdavis.osquery-digest \
    '~/Library/LaunchAgents/com.webdavis.osquery-digest.plist' \
    "/opt/homebrew/bin/bash ~/.local/bin/osquery-digest.sh"
  run_alerter "$(persist_row com.webdavis.osquery-digest \
    "$HARNESS_HOME/Library/LaunchAgents/com.webdavis.osquery-digest.plist" \
    "/opt/homebrew/bin/bash $HARNESS_HOME/.local/bin/osquery-digest.sh")"
  assert_no_page
  assert_digest_count 0
}

@test "T-PERSIST-daemon-still-pages: a root LaunchDaemon still pages regardless of the allowlist" {
  # /Library/LaunchDaemons runs as root at boot - it pages by PATH, the allowlist never applies.
  run_alerter "$(persist_row com.foo.daemon /Library/LaunchDaemons/com.foo.daemon.plist /opt/homebrew/bin/foo)"
  assert_page_has com.foo.daemon
}
