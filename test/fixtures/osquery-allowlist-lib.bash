#!/usr/bin/env bash
# Test harness for the launchd page-allowlist writer (executable_allowlist.sh).
#
# The writer is the ONE security boundary that curates the page-allowlist: -a allow,
# -d deny, -l list. R2-1: an entry is a TUPLE, not a bare label, so -a CAPTURES the
# label's known-good launchd identity (canonical plist path + program + plist sha256)
# from the launchd table before storing it. This harness gives the writer:
#
#   - a fresh temp HOME with its own page-allowlist file the writer curates, so a test
#     never touches the operator's real ~/.config/osquery or ~/Library/LaunchAgents and
#     a captured $HOME path relativizes to ~/ deterministically; and
#   - an osqueryi STUB on which the capture depends (the message-recording spy): it
#     prints $ALLOWLIST_OSQUERYI_ROW, so a test sets a known launchd row and the captured
#     tuple is deterministic with no real launchd dependency. The default empty result
#     models a label with no loaded LaunchAgent (a degraded, label-only capture).

ALLOWLIST_TOOL="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_allowlist.sh"

setup_allowlist_harness() {
  export ALLOWLIST_HOME
  ALLOWLIST_HOME="$(mktemp -d)"
  export OSQUERY_LAUNCHD_ALLOWLIST="$ALLOWLIST_HOME/.config/osquery/page-launchd-allowlist.txt"
  mkdir -p "$ALLOWLIST_HOME/bin"
  setup_allowlist_chezmoi
  cat >"$ALLOWLIST_HOME/bin/osqueryi" <<'SHIM'
#!/usr/bin/env bash
# Concurrency knobs: the sentinel tells a test the capture has STARTED (so it can
# launch a racing command deterministically); the delay holds the capture open so
# the race window is wide enough to be deterministic, not timing-luck.
[[ -n ${ALLOWLIST_OSQUERYI_STARTED_FILE:-} ]] && : >"$ALLOWLIST_OSQUERYI_STARTED_FILE"
[[ -n ${ALLOWLIST_OSQUERYI_DELAY:-} ]] && sleep "$ALLOWLIST_OSQUERYI_DELAY"
# fd-inheritance knob: spawn a background grandchild that outlives the writer. Its stdio
# is detached from the capture pipeline (</dev/null >/dev/null 2>&1) so the pipeline's
# teardown cannot reap it, but it does NOT close fd 9 - so if the writer's lock fd leaked
# into this osqueryi child, the lingerer inherits it and keeps the kernel lock held after
# the writer exits (the leak this exercises). With the fd closed in every child (9>&-),
# the lingerer never receives fd 9 and the lock releases on writer exit.
if [[ -n ${ALLOWLIST_OSQUERYI_LINGER:-} ]]; then
  ( sleep "$ALLOWLIST_OSQUERYI_LINGER" ) </dev/null >/dev/null 2>&1 &
  [[ -n ${ALLOWLIST_OSQUERYI_LINGER_PID_FILE:-} ]] && printf '%s\n' "$!" >"$ALLOWLIST_OSQUERYI_LINGER_PID_FILE"
fi
printf '%s\n' "${ALLOWLIST_OSQUERYI_ROW:-[]}"
SHIM
  chmod +x "$ALLOWLIST_HOME/bin/osqueryi"
  export ALLOWLIST_OSQUERYI="$ALLOWLIST_HOME/bin/osqueryi"
}

# The writer edits the chezmoi SOURCE and deploys with an apply, so the harness needs
# a REAL (tiny) chezmoi source and its own config, isolated from the operator's. A
# stub would only prove the writer calls something named chezmoi; the real binary is
# what proves the target is actually MANAGED (so `source-path` resolves) and that the
# apply really lands the source bytes at the deployed path.
#
# Two more seams the writer needs, both recorded rather than hidden:
#   ALLOWLIST_CHEZMOI          - a wrapper pinning source/dest/config/state, so nested
#                                calls can never touch the operator's real chezmoi.
#   ALLOWLIST_MANIFEST_RUNNER  - a spy for the pipeline-manifest runner. It records
#                                that it ran and exits ALLOWLIST_MANIFEST_RC, so the
#                                loud-failure paths are exercised without sudo. The
#                                real runner has its own suites; what belongs here is
#                                whether the writer invokes it and how it reacts.
setup_allowlist_chezmoi() {
  export ALLOWLIST_SRC="$ALLOWLIST_HOME/chezmoi-src"
  export ALLOWLIST_SOURCE_FILE="$ALLOWLIST_SRC/dot_config/osquery/private_page-launchd-allowlist.txt"
  mkdir -p "$ALLOWLIST_SRC/dot_config/osquery" "$ALLOWLIST_HOME/.config/chezmoi"
  printf 'sourceDir = "%s"\ndestDir = "%s"\n' "$ALLOWLIST_SRC" "$ALLOWLIST_HOME" \
    >"$ALLOWLIST_HOME/.config/chezmoi/chezmoi.toml"
  # The target must exist in the source, or it is not managed and source-path fails.
  : >"$ALLOWLIST_SOURCE_FILE"

  cat >"$ALLOWLIST_HOME/bin/chezmoi" <<EOF
#!/usr/bin/env bash
exec env -u XDG_CONFIG_HOME -u XDG_DATA_HOME chezmoi \\
  --config "$ALLOWLIST_HOME/.config/chezmoi/chezmoi.toml" \\
  --source "$ALLOWLIST_SRC" --destination "$ALLOWLIST_HOME" \\
  --persistent-state "$ALLOWLIST_HOME/chezmoi-state.boltdb" "\$@"
EOF
  chmod +x "$ALLOWLIST_HOME/bin/chezmoi"
  export ALLOWLIST_CHEZMOI="$ALLOWLIST_HOME/bin/chezmoi"

  export ALLOWLIST_MANIFEST_LOG="$ALLOWLIST_HOME/manifest-runner.log"
  : >"$ALLOWLIST_MANIFEST_LOG"
  cat >"$ALLOWLIST_HOME/bin/manifest-runner" <<'SPY'
#!/usr/bin/env bash
printf 'RAN source=%s\n' "${CHEZMOI_SOURCE_DIR:-}" >>"$ALLOWLIST_MANIFEST_LOG"
if [[ -n ${ALLOWLIST_MANIFEST_RC:-} ]] && [[ $ALLOWLIST_MANIFEST_RC -ne 0 ]]; then
  printf 'sudo: a password is required\n' >&2
  exit "$ALLOWLIST_MANIFEST_RC"
fi
SPY
  chmod +x "$ALLOWLIST_HOME/bin/manifest-runner"
  export ALLOWLIST_MANIFEST_RUNNER="$ALLOWLIST_HOME/bin/manifest-runner"

  # Deploy the (empty) source so the harness starts from an applied, consistent state.
  "$ALLOWLIST_CHEZMOI" apply --force >/dev/null 2>&1 || true
}

teardown_allowlist_harness() { [[ -n ${ALLOWLIST_HOME:-} ]] && rm -rf "$ALLOWLIST_HOME"; }

# Run the writer with the harness env (args passed verbatim). HOME is the temp harness
# home so a captured launchd path/program under it relativizes to ~/ in isolation, never
# reading or writing the operator's real home.
run_allowlist() {
  HOME="$ALLOWLIST_HOME" \
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERYI="$ALLOWLIST_OSQUERYI" \
    CHEZMOI="$ALLOWLIST_CHEZMOI" \
    OSQUERY_PIPELINE_MANIFEST_RUNNER="$ALLOWLIST_MANIFEST_RUNNER" \
    ALLOWLIST_MANIFEST_LOG="$ALLOWLIST_MANIFEST_LOG" \
    ALLOWLIST_MANIFEST_RC="${ALLOWLIST_MANIFEST_RC:-0}" \
    bash "$ALLOWLIST_TOOL" "$@"
}

# The chezmoi SOURCE file's entry lines: what the writer must actually be editing.
source_entry_lines() {
  grep -vE '^[[:space:]]*(#|$)' "$ALLOWLIST_SOURCE_FILE" 2>/dev/null || true
}

# Did this run invoke the pipeline-manifest runner?
assert_manifest_refreshed() {
  if ! grep -q '^RAN' "$ALLOWLIST_MANIFEST_LOG" 2>/dev/null; then
    echo "expected the writer to refresh the pipeline manifest; runner log: $(cat "$ALLOWLIST_MANIFEST_LOG" 2>/dev/null || echo '(none)')" >&2
    return 1
  fi
}

refute_manifest_refreshed() {
  if grep -q '^RAN' "$ALLOWLIST_MANIFEST_LOG" 2>/dev/null; then
    echo "expected NO manifest refresh, but the runner ran: $(cat "$ALLOWLIST_MANIFEST_LOG")" >&2
    return 1
  fi
}

# Seed one NDJSON tuple line into the allowlist (bypassing capture), so a deny/list
# test starts from a known store: seed_allowlist_tuple <label> <path> <program> [sha256].
#
# The tuple goes into the chezmoi SOURCE and is then applied, because the source is
# the authority the deployed file is rewritten from on every apply. Seeding the
# deployed file alone would build a state chezmoi erases and the writer does not
# consider present, which is precisely the shape this design removed.
seed_allowlist_tuple() {
  jq -cn --arg label "$1" --arg path "$2" --arg program "$3" --arg sha256 "${4:-}" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' >>"$ALLOWLIST_SOURCE_FILE"
  "$ALLOWLIST_CHEZMOI" apply --force >/dev/null 2>&1
}

# Seed a raw line into the allowlist source verbatim (same source-then-apply route
# as seed_allowlist_tuple), for the shapes a well-formed tuple cannot express: a
# line holding two concatenated JSON documents, a comment, a blank.
seed_allowlist_raw_line() {
  printf '%s\n' "$1" >>"$ALLOWLIST_SOURCE_FILE"
  "$ALLOWLIST_CHEZMOI" apply --force >/dev/null 2>&1
}

# One line holding TWO concatenated tuples for <label>: what a doubled write, or
# a lost newline between two appends, leaves behind. Each document parses alone,
# so a per-document read reports a label for it; only a one-value-per-line rule
# refuses it. The consumer (allowlist-verdict) already applies that rule, so a
# line like this can never suppress anything.
doubled_tuple_line() {
  local tuple
  tuple="$(jq -cn --arg label "$1" --arg path "$2" --arg program "$3" \
    '{label:$label, path:$path, program:$program, sha256:""}')"
  printf '%s%s' "$tuple" "$tuple"
}

# Membership by the JSON .label field (the file is NDJSON tuples now, R2-1).
assert_allowlisted() {
  if ! grep -qF "\"label\":\"$1\"" "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null; then
    echo "expected label '$1' in the allowlist: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null || echo '(no file)')" >&2
    return 1
  fi
}

assert_not_allowlisted() {
  if grep -qF "\"label\":\"$1\"" "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null; then
    echo "expected label '$1' NOT in the allowlist: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" >&2
    return 1
  fi
}

# Try to take the writer's lock non-blocking on a FRESH fd. Exit 0 = the lock is free
# (no leaked, still-held copy); non-zero = still held. Used to prove the lock released
# when the writer exited (no child inherited the lock fd).
allowlist_lock_is_free() {
  local lockf_bin="${OSQUERY_ALLOWLIST_LOCKF_BIN:-/usr/bin/lockf}"
  (exec 8>>"${OSQUERY_LAUNCHD_ALLOWLIST}.lock" && "$lockf_bin" -s -t 0 8)
}

# Count of entry lines (non-comment, non-blank): one NDJSON tuple per line.
assert_allowlist_label_count() {
  local n
  n=$(grep -cvE '^[[:space:]]*(#|$)' "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null || echo 0)
  if [[ $n -ne $1 ]]; then
    echo "expected $1 entr(y/ies), got $n: $(cat "$OSQUERY_LAUNCHD_ALLOWLIST" 2>/dev/null)" >&2
    return 1
  fi
}
