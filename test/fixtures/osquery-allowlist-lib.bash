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

teardown_allowlist_harness() { [[ -n ${ALLOWLIST_HOME:-} ]] && rm -rf "$ALLOWLIST_HOME"; }

# Run the writer with the harness env (args passed verbatim). HOME is the temp harness
# home so a captured launchd path/program under it relativizes to ~/ in isolation, never
# reading or writing the operator's real home.
run_allowlist() {
  HOME="$ALLOWLIST_HOME" \
    OSQUERY_LAUNCHD_ALLOWLIST="$OSQUERY_LAUNCHD_ALLOWLIST" \
    OSQUERYI="$ALLOWLIST_OSQUERYI" \
    bash "$ALLOWLIST_TOOL" "$@"
}

# Seed one NDJSON tuple line into the allowlist directly (bypassing capture), so a
# deny/list test starts from a known store: seed_allowlist_tuple <label> <path> <program> [sha256].
seed_allowlist_tuple() {
  mkdir -p "$(dirname "$OSQUERY_LAUNCHD_ALLOWLIST")"
  jq -cn --arg label "$1" --arg path "$2" --arg program "$3" --arg sha256 "${4:-}" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' >>"$OSQUERY_LAUNCHD_ALLOWLIST"
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
