#!/usr/bin/env bats
# executable_allowlist.sh - the ONE writer for the launchd page-allowlist (-a allow,
# -d deny, -l list). It is the security boundary every caller (manual curation, the
# tap-button bot, the /osquery skill) goes through, so its capture and validation are
# the test focus. R2-1: an entry is a TUPLE - `-a` captures the label's identity (plist
# path + program + plist sha256) from the launchd table, so the alerter later suppresses
# a full-tuple match only and PAGES a label reused with a different identity.

load ../fixtures/osquery-allowlist-lib

setup() { setup_allowlist_harness; }
teardown() { teardown_allowlist_harness; }

# Set the launchd row the capture stub returns: a one-element array of {path, program}.
stub_launchd() {
  export ALLOWLIST_OSQUERYI_ROW="$(jq -cn --arg p "$1" --arg prog "$2" '[{path:$p, program:$prog}]')"
}

@test "adding a label to an empty allowlist captures its launchd identity as one NDJSON tuple with \$HOME stored as ~/" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "/opt/homebrew/bin/bash $ALLOWLIST_HOME/.local/bin/foo.sh"

  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected the writer to exit 0 on a valid label, got $status: $output"
    false
  }

  assert_allowlist_label_count 1
  assert_allowlisted com.foo.agent

  # The stored tuple carries the four fields: the captured path + program (each with a
  # leading $HOME rewritten to ~/), and the plist's real sha256.
  local sha
  sha="$(shasum -a 256 "$plist" | awk '{print $1}')"
  run jq -e --arg h "$sha" \
    'select(.label == "com.foo.agent"
            and .path == "~/Library/LaunchAgents/com.foo.agent.plist"
            and .program == "/opt/homebrew/bin/bash ~/.local/bin/foo.sh"
            and .sha256 == $h)' \
    "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ] || {
    echo "expected one tuple with label+path+program (\$HOME as ~/) + the plist sha256; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}
