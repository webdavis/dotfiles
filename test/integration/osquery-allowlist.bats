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

@test "the writer's lock never leaks to a child, so it releases when the writer exits even if a spawned child lingers" {
  # Sol R2: exec 9>> leaves the lock fd inheritable. If any child in the locked section
  # (osqueryi/jq/shasum/mv/...) outlives the writer, it keeps the kernel lock held and
  # every later -a/-d blocks forever. Here the osqueryi capture spawns a grandchild that
  # lingers past the writer's exit; with the fd closed in every child (9>&-) the lock
  # still releases on writer exit, so a fresh acquire succeeds immediately.
  [[ -x /usr/bin/lockf ]] || skip "no /usr/bin/lockf (the writer runs unlocked on non-darwin by design)"
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" /opt/homebrew/opt/foo/bin/foo

  # The grandchild has to outlive a REAL `chezmoi apply` plus the manifest-runner
  # spy that `-a` performs before this test looks at it, and it is detached, so a
  # wide window costs the run nothing while a narrow one fails the pin on a
  # contended runner for a reason that has nothing to do with fd hygiene. The
  # already-exited branch below says "widen ALLOWLIST_OSQUERYI_LINGER"; this is that.
  export ALLOWLIST_OSQUERYI_LINGER=60
  export ALLOWLIST_OSQUERYI_LINGER_PID_FILE="$ALLOWLIST_HOME/linger-pid"
  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected -a to exit 0, got $status: $output"
    false
  }

  # The lingering grandchild is still alive (guards against a false pass where the child
  # already exited and freed the fd on its own).
  local linger_pid
  linger_pid="$(cat "$ALLOWLIST_HOME/linger-pid")"
  kill -0 "$linger_pid" 2>/dev/null || {
    echo "test setup: the lingering child $linger_pid already exited; widen ALLOWLIST_OSQUERYI_LINGER"
    false
  }

  # The writer has exited; the lock must be free despite the still-alive child, proving
  # no child inherited the lock fd.
  run allowlist_lock_is_free
  [ "$status" -eq 0 ] || {
    echo "expected the lock free after the writer exited (a child inherited the lock fd and still holds it), lockf exit $status"
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

# --- The writer deploys through chezmoi, not by hand -------------------------
# The allowlist is chezmoi-managed, so a tuple written straight to the deployed
# file is erased by the next apply (verified: a plain managed file is rewritten
# from source every time) and the seed silently stops working. It is also
# manifest-covered now, so an out-of-band write no longer suppresses anything
# even before that. The writer therefore edits the SOURCE, applies that one
# target, and refreshes the manifest, in that order.

@test "-a writes the chezmoi SOURCE and deploys it, so the seed survives a later apply" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "$ALLOWLIST_HOME/bin/foo.sh"

  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected exit 0, got $status: $output"
    false
  }

  # The SOURCE holds the tuple (this is the authority the manifest is derived from).
  run grep -qF '"label":"com.foo.agent"' "$ALLOWLIST_SOURCE_FILE"
  [ "$status" -eq 0 ] || {
    echo "the tuple is not in the chezmoi source: $(cat "$ALLOWLIST_SOURCE_FILE")"
    false
  }
  # ...and the deployed file matches it, so the alerter sees the seed immediately.
  assert_allowlisted com.foo.agent

  # THE HEADLINE: an independent apply does not undo the seed. Writing the deployed
  # file directly would lose it here, which is the bug this shape removes.
  "$ALLOWLIST_CHEZMOI" apply --force >/dev/null 2>&1
  assert_allowlisted com.foo.agent
}

@test "-a refreshes the pipeline manifest after the apply, so the deployed allowlist is bound and can still suppress" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "$ALLOWLIST_HOME/bin/foo.sh"

  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ]
  assert_manifest_refreshed
}

@test "-d removes from the SOURCE and refreshes the manifest, so the removal survives an apply" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "$ALLOWLIST_HOME/bin/foo.sh"
  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ]

  : >"$ALLOWLIST_MANIFEST_LOG"
  run run_allowlist -d com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "expected exit 0 on deny, got $status: $output"
    false
  }
  assert_not_allowlisted com.foo.agent
  assert_manifest_refreshed
  "$ALLOWLIST_CHEZMOI" apply --force >/dev/null 2>&1
  assert_not_allowlisted com.foo.agent
}

@test "-a is idempotent: re-adding an unchanged identity leaves the source byte-identical and adds no duplicate" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "$ALLOWLIST_HOME/bin/foo.sh"

  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ]
  local first
  first="$(cat "$ALLOWLIST_SOURCE_FILE")"

  run run_allowlist -a com.foo.agent
  [ "$status" -eq 0 ] || {
    echo "a repeated -a must succeed, got $status: $output"
    false
  }
  [ "$(cat "$ALLOWLIST_SOURCE_FILE")" = "$first" ] || {
    echo "a repeated -a changed the source; before: $first  after: $(cat "$ALLOWLIST_SOURCE_FILE")"
    false
  }
  assert_allowlist_label_count 1
  [ "$(source_entry_lines | grep -cF '"label":"com.foo.agent"')" -eq 1 ] || {
    echo "the source gained a duplicate tuple: $(cat "$ALLOWLIST_SOURCE_FILE")"
    false
  }
}

@test "a failed apply rolls the source back and reports non-zero, leaving no half-applied state" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "$ALLOWLIST_HOME/bin/foo.sh"

  local before
  before="$(cat "$ALLOWLIST_SOURCE_FILE")"

  # A chezmoi that resolves the source path but fails the apply.
  cat >"$ALLOWLIST_HOME/bin/chezmoi-badapply" <<EOF
#!/usr/bin/env bash
[[ \$1 == apply ]] && { printf 'chezmoi: apply exploded\n' >&2; exit 1; }
exec "$ALLOWLIST_CHEZMOI" "\$@"
EOF
  chmod +x "$ALLOWLIST_HOME/bin/chezmoi-badapply"

  run env HOME="$ALLOWLIST_HOME" OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERYI="$ALLOWLIST_OSQUERYI" CHEZMOI="$ALLOWLIST_HOME/bin/chezmoi-badapply" \
    OSQUERY_PIPELINE_MANIFEST_RUNNER="$ALLOWLIST_MANIFEST_RUNNER" \
    ALLOWLIST_MANIFEST_LOG="$ALLOWLIST_MANIFEST_LOG" \
    bash "$ALLOWLIST_TOOL" -a com.foo.agent
  [ "$status" -ne 0 ] || {
    echo "a failed apply must exit non-zero, got 0: $output"
    false
  }
  [[ $output == *"apply"* ]] || {
    echo "the failure must name the failed step; got: $output"
    false
  }
  [ "$(cat "$ALLOWLIST_SOURCE_FILE")" = "$before" ] || {
    echo "the source was left edited after a failed apply: $(cat "$ALLOWLIST_SOURCE_FILE")"
    false
  }
  # A manifest refresh over a source that was rolled back would sign a state the
  # operator never asked for, so the step must not have run at all.
  refute_manifest_refreshed
}

@test "a failed manifest refresh is LOUD and non-zero, names the stale manifest, and does not swallow sudo's message" {
  local plist="$ALLOWLIST_HOME/Library/LaunchAgents/com.foo.agent.plist"
  mkdir -p "$(dirname "$plist")"
  printf 'plist-bytes\n' >"$plist"
  stub_launchd "$plist" "$ALLOWLIST_HOME/bin/foo.sh"

  ALLOWLIST_MANIFEST_RC=1 run run_allowlist -a com.foo.agent
  [ "$status" -ne 0 ] || {
    echo "a failed manifest refresh must exit non-zero, got 0: $output"
    false
  }
  # The runner's own stderr (here sudo's) must reach the operator, not /dev/null.
  [[ $output == *"password is required"* ]] || {
    echo "the manifest runner's stderr was swallowed; got: $output"
    false
  }
  # ...and the message has to say what state the host is in, because this is the
  # one failure that leaves the deployed allowlist ahead of the manifest.
  [[ $output == *manifest* ]] || {
    echo "the failure must name the manifest as the stale component; got: $output"
    false
  }
}

@test "the default manifest runner is the real known-good-manifests script inside the chezmoi source" {
  # The seam above is a test double. Pin the DEFAULT so it cannot drift from the
  # runner that actually exists in the source tree.
  run grep -qF 'run_after_05-osquery-known-good-manifests.sh' "$ALLOWLIST_TOOL"
  [ "$status" -eq 0 ] || {
    echo "the writer does not name the real manifest runner as its default"
    false
  }
  [ -f "${BATS_TEST_DIRNAME}/../../.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh" ] || {
    echo "the runner the writer names does not exist in the source tree"
    false
  }
}
