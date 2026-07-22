#!/usr/bin/env bats
# executable_allowlist.sh - the ONE writer for the launchd page-allowlist (-a allow,
# -d deny, -l list). It is the security boundary every caller (manual curation, the
# tap-button bot, the /osquery skill) goes through, so its capture and validation are
# the test focus. R2-1: an entry is a TUPLE - `-a` captures the label's identity (plist
# path + program + plist sha256) from the launchd table, so the alerter later suppresses
# a full-tuple match only and PAGES a label reused with a different identity.

bats_require_minimum_version 1.5.0

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

@test "re-adding an existing label refreshes its tuple in place: exactly one line for it, carrying the new identity, and other labels untouched" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"

  # A second, unrelated label is captured first: it must survive the refresh verbatim.
  local other_plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.other.agent.plist"
  mkdir -p "$(dirname "$other_plist")"
  printf 'other-plist-bytes\n' >"$other_plist"
  stub_launchd "$other_plist" /opt/homebrew/bin/other
  run_allowlist -a com.other.agent
  local other_line
  other_line="$(grep -F '"label":"com.other.agent"' "$OSQUERY_LAUNCHD_ALLOWLIST")"

  # Seed com.foo.agent with identity A (program A, plist bytes A -> sha A).
  printf 'plist-bytes-A\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo-A
  run_allowlist -a com.foo.agent

  # Re-add com.foo.agent with a DIFFERENT identity B (program B, plist bytes B -> sha B).
  printf 'plist-bytes-B\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo-B
  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected the refreshing -a to exit 0, got $status: $output"
    false
  }

  # Exactly one line remains for the label (a refresh, not a duplicate append).
  local foo_count
  foo_count="$(grep -cF '"label":"com.foo.agent"' "$OSQUERY_LAUNCHD_ALLOWLIST")"
  [ "$foo_count" -eq 1 ] || {
    echo "expected exactly one line for com.foo.agent after refresh, got $foo_count: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }

  # That one line carries identity B (the latest capture), not the stale identity A.
  local sha_b
  sha_b="$(shasum -a 256 "$plist" | awk '{print $1}')"
  run jq -e --arg h "$sha_b" \
    'select(.label == "com.foo.agent"
            and .program == "/opt/homebrew/opt/foo/bin/foo-B"
            and .sha256 == $h)' \
    "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ] || {
    echo "expected com.foo.agent refreshed to identity B (program foo-B + sha B); file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }

  # The unrelated label's line is byte-for-byte unchanged.
  run grep -qxF "$other_line" "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ] || {
    echo "expected com.other.agent's line preserved verbatim through the refresh; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}

@test "denying (-d) a label removes its entry and leaves every other label byte-identical" {
  seed_allowlist_tuple com.foo.agent '~/Library/LaunchAgents/com.foo.agent.plist' /opt/homebrew/opt/foo/bin/foo
  seed_allowlist_tuple com.bar.agent '~/Library/LaunchAgents/com.bar.agent.plist' /opt/homebrew/opt/bar/bin/bar
  local bar_line
  bar_line="$(grep -F '"label":"com.bar.agent"' "$OSQUERY_LAUNCHD_ALLOWLIST")"

  run run_allowlist -d com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected -d of a present label to exit 0, got $status: $output"
    false
  }

  assert_not_allowlisted com.foo.agent
  assert_allowlist_label_count 1
  run grep -qxF "$bar_line" "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ] || {
    echo "expected com.bar.agent's line untouched by the deny; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}

@test "denying (-d) an absent label is a clean no-op: exit 0, file unchanged, nothing on stderr" {
  seed_allowlist_tuple com.bar.agent '~/Library/LaunchAgents/com.bar.agent.plist' /opt/homebrew/opt/bar/bin/bar
  local before
  before="$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"

  run run_allowlist -d com.absent.agent
  [ "$status" -eq 0 ] || {
    echo "expected -d of an absent label to exit 0 (clean no-op), got $status: $output"
    false
  }
  [ -z "$(run_allowlist -d com.absent.agent 2>&1 >/dev/null)" ] || {
    echo "expected -d of an absent label to write nothing to stderr"
    false
  }
  [ "$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" = "$before" ] || {
    echo "expected the allowlist unchanged by a no-op deny; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}

@test "listing (-l) prints exactly the current entry lines to stdout and exits 0" {
  seed_allowlist_tuple com.foo.agent '~/Library/LaunchAgents/com.foo.agent.plist' /opt/homebrew/opt/foo/bin/foo
  seed_allowlist_tuple com.bar.agent '~/Library/LaunchAgents/com.bar.agent.plist' /opt/homebrew/opt/bar/bin/bar
  local expected
  expected="$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"

  run run_allowlist -l
  [ "$status" -eq 0 ] || {
    echo "expected -l to exit 0, got $status: $output"
    false
  }
  [ "$output" = "$expected" ] || {
    echo "expected -l to print exactly the two seeded tuple lines; got: $output"
    false
  }
}

@test "listing (-l) on an empty or absent allowlist prints nothing and exits 0" {
  run run_allowlist -l
  [ "$status" -eq 0 ] || {
    echo "expected -l on an absent allowlist to exit 0, got $status: $output"
    false
  }
  [ -z "$output" ] || {
    echo "expected -l on an absent allowlist to print nothing; got: $output"
    false
  }
}

# The writer is the security boundary: every mutating verb validates the label first, so a
# system-daemon page can never be falsely suppressed by an allowlist entry. These pin the
# is_valid_label contract for BOTH mutating verbs (-a and -d). An empty, malformed, or
# Apple/system label is refused (non-zero exit, an explanation on stderr, no store touched);
# a valid non-Apple label using the full allowed charset (. _ @ -) is accepted.

@test "adding (-a) refuses an empty, malformed, or Apple/system label: non-zero exit, stderr explains, no store created" {
  for bad in '' 'com foo' '*' '../etc' 'a/b' 'com.apple.foo' 'COM.APPLE.FOO' 'com.apple'; do
    run --separate-stderr run_allowlist -a "$bad"
    [ "$status" -ne 0 ] || {
      echo "expected -a '$bad' refused with a non-zero exit, got 0"
      false
    }
    [[ "$stderr" == *refused* ]] || {
      echo "expected -a '$bad' to explain the refusal on stderr, got: $stderr"
      false
    }
    [ ! -e "$OSQUERY_LAUNCHD_ALLOWLIST" ] || {
      echo "expected no allowlist created by a refused -a '$bad'; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
      false
    }
  done
}

@test "denying (-d) refuses an empty, malformed, or Apple/system label: non-zero exit, stderr explains, no store created" {
  for bad in '' 'com foo' '*' '../etc' 'a/b' 'com.apple.foo' 'COM.APPLE.FOO' 'com.apple'; do
    run --separate-stderr run_allowlist -d "$bad"
    [ "$status" -ne 0 ] || {
      echo "expected -d '$bad' refused with a non-zero exit, got 0"
      false
    }
    [[ "$stderr" == *refused* ]] || {
      echo "expected -d '$bad' to explain the refusal on stderr, got: $stderr"
      false
    }
    [ ! -e "$OSQUERY_LAUNCHD_ALLOWLIST" ] || {
      echo "expected no allowlist created by a refused -d '$bad'; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
      false
    }
  done
}

@test "a deny that completes during an overlapping add is never silently undone: the writers serialize and the denied label stays out" {
  # Sol R1-1 (lost update): -a and -d are read-modify-write-publish. Unserialized, a slow
  # -a (capture in flight) publishes AFTER a completed -d and restores the denied tuple.
  # With the write lock, the -d blocks until the -a publishes, then removes the label, so
  # the deny always wins and the final file is the serialized result, never an interleave.
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  seed_allowlist_tuple com.other.agent '~/Library/LaunchAgents/com.other.agent.plist' /opt/homebrew/bin/other

  # A slow -a: its osqueryi capture signals start, then holds the capture open.
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo
  export ALLOWLIST_OSQUERYI_STARTED_FILE="$ALLOWLIST_HOME/capture-started"
  export ALLOWLIST_OSQUERYI_DELAY=2
  run_allowlist -a com.foo.agent &
  local add_pid=$!

  # Deterministic ordering: wait until the add's capture has actually started (it is
  # inside its critical section), then run the deny for the same label.
  local waited=0
  until [[ -e $ALLOWLIST_OSQUERYI_STARTED_FILE ]]; do
    sleep 0.1
    waited=$((waited + 1))
    [[ $waited -lt 100 ]] || {
      echo "expected the add's capture to start within 10s (sentinel never appeared)"
      false
    }
  done
  unset ALLOWLIST_OSQUERYI_STARTED_FILE ALLOWLIST_OSQUERYI_DELAY

  run run_allowlist -d com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected the racing -d to exit 0, got $status: $output"
    false
  }
  wait "$add_pid" || {
    echo "expected the overlapped -a to exit 0"
    false
  }

  # The deny wins: the label is absent from the final file (the add did not republish it).
  assert_not_allowlisted com.foo.agent
  # And the race never interleaved into a corrupt or duplicated file: the untouched
  # label survives exactly once and every entry line is still valid NDJSON.
  assert_allowlisted com.other.agent
  assert_allowlist_label_count 1
  run jq -e . "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ] || {
    echo "expected every surviving line to be valid NDJSON; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}

@test "a hash-capture failure during -a fails closed: non-zero exit, no tuple written, the failure named on stderr" {
  # Sol R1-2: an empty sha256 is reserved for the operator-curated own-agent seed and must
  # never be writer-produced. If -a captured a real plist path but could not pin its hash,
  # writing the tuple anyway would let a later plist swap at that same path/program hide
  # behind the unpinned entry. So the writer refuses instead.
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo
  seed_allowlist_tuple com.other.agent '~/Library/LaunchAgents/com.other.agent.plist' /opt/homebrew/bin/other
  local before
  before="$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"

  # A broken hash tool, first on PATH (each bats test runs in its own process, so the
  # export does not leak).
  mkdir -p "$ALLOWLIST_HOME/shims"
  printf '#!/usr/bin/env bash\nexit 1\n' >"$ALLOWLIST_HOME/shims/shasum"
  chmod +x "$ALLOWLIST_HOME/shims/shasum"
  export PATH="$ALLOWLIST_HOME/shims:$PATH"

  run --separate-stderr run_allowlist -a com.foo.agent
  [ "$status" -ne 0 ] || {
    echo "expected -a to fail closed when the plist hash cannot be captured, got exit 0"
    false
  }
  [[ "$stderr" == *sha256* || "$stderr" == *hash* ]] || {
    echo "expected the hash-capture failure named on stderr, got: $stderr"
    false
  }
  assert_not_allowlisted com.foo.agent
  [ "$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" = "$before" ] || {
    echo "expected the allowlist byte-identical after the refused write; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}

@test "a successful -a capture always pins the tuple with a 64-hex sha256, never an empty one" {
  # The regression guard for the fail-closed rule: the live capture path (working shasum)
  # stores a real content pin, so only the operator-curated seed may carry sha256 == "".
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo

  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected -a with a working hash tool to exit 0, got $status: $output"
    false
  }
  run jq -e 'select(.label == "com.foo.agent") | .sha256 | test("^[0-9a-f]{64}$")' \
    "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ] || {
    echo "expected the stored tuple pinned with a 64-hex sha256; file: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
    false
  }
}

@test "a valid non-Apple label using the full allowed charset (. _ @ -) is accepted by both -a and -d" {
  # -a accepts it and captures its stubbed identity (a live capture must be complete
  # and sha256-pinned, so the stub provides a real plist + program).
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/homebrew.mxcl.postgresql@17.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/postgresql@17/bin/postgres
  run run_allowlist -a 'homebrew.mxcl.postgresql@17'
  [ "$status" -eq 0 ] || {
    echo "expected -a of a valid @-bearing label accepted, got $status: $output"
    false
  }
  assert_allowlisted 'homebrew.mxcl.postgresql@17'

  # -d accepts it too (not refused): removing the just-added label exits 0.
  run run_allowlist -d 'homebrew.mxcl.postgresql@17'
  [ "$status" -eq 0 ] || {
    echo "expected -d of a valid @-bearing label accepted, got $status: $output"
    false
  }
  assert_not_allowlisted 'homebrew.mxcl.postgresql@17'
}
