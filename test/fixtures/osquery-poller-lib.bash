#!/usr/bin/env bash
# Test harness (makeSUT) for the security-posture poller
# (dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh).
#
# The poller runs as a gui/501 user LaunchAgent every 60s: it reads the live
# firewall (alf), Gatekeeper, and screen-lock posture through osqueryi and
# persists it as an owner-only baseline. This harness stands the poller up in
# isolation and records what it does through two recording spies:
#
#   - a programmable, recording osqueryi stub: it appends the SQL it was handed
#     to $POLLER_OSQUERYI_QUERY and a marker per call to $POLLER_OSQUERYI_CALLS,
#     then prints $POLLER_OSQUERYI_JSON, so a test sets a known posture and can
#     prove BOTH what the poller asked for and what it read, with no real
#     osquery/launchd dependency; and
#   - a recording send_alert spy, installed as a stand-in dispatch library at the
#     new libexec path the poller sources ($HOME/.local/libexec/osquery/
#     alert-dispatch.sh). It never delivers; it records each call's argv and
#     whether the baseline already existed at the moment of the call, so a test
#     can prove the poller stays silent AND that any page fires only AFTER the
#     baseline is written.
#
# A fresh temp HOME keeps every run off the operator's real ~/.local/state and
# ~/.local/libexec. Sourced by the poller suite; no main.

POLLER_TOOL="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh"

# set_posture <json-array> -- the JSON array of row objects the osqueryi stub
# returns. osquery --json emits an array and the poller reads .[0]; scalars are
# strings, matching osquery's JSON output for these integer columns.
set_posture() {
  export POLLER_OSQUERYI_JSON="$1"
}

setup_poller_harness() {
  export POLLER_HOME
  POLLER_HOME="$(mktemp -d)"
  # Ownership marker set only after our own mktemp, so teardown removes this
  # path and never a pre-set or inherited POLLER_HOME.
  _POLLER_HARNESS_OWNED_DIR="$POLLER_HOME"

  mkdir -p "$POLLER_HOME/bin" \
    "$POLLER_HOME/.local/libexec/osquery" \
    "$POLLER_HOME/.local/state"

  # The env-overridable baseline path, under the sandbox HOME.
  export OSQUERY_POSTURE_STATE="$POLLER_HOME/.local/state/osquery-posture-state.json"

  # Recording osqueryi stub: log the query and a per-call marker, then print the
  # programmed posture. It ignores the SQL for its OUTPUT (the test drives the
  # posture directly) but RECORDS it so a test can assert the read shape.
  export POLLER_OSQUERYI_QUERY="$POLLER_HOME/osqueryi-query.log"
  export POLLER_OSQUERYI_CALLS="$POLLER_HOME/osqueryi-calls.log"
  : >"$POLLER_OSQUERYI_QUERY"
  : >"$POLLER_OSQUERYI_CALLS"
  cat >"$POLLER_HOME/bin/osqueryi" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$POLLER_OSQUERYI_QUERY"
printf 'call\n' >>"$POLLER_OSQUERYI_CALLS"
# POLLER_OSQUERYI_EXIT models a hard osqueryi failure: a non-zero value means no
# stdout and that exit status (a missing binary, or the daemon not up on a fresh
# boot). Zero, the default, prints the programmed posture.
exit_code="${POLLER_OSQUERYI_EXIT:-0}"
if [[ $exit_code -ne 0 ]]; then
  exit "$exit_code"
fi
printf '%s\n' "${POLLER_OSQUERYI_JSON:-[]}"
SHIM
  chmod +x "$POLLER_HOME/bin/osqueryi"
  export POLLER_OSQUERYI="$POLLER_HOME/bin/osqueryi"

  # Recording send_alert spy at the NEW dispatch path the poller sources. It
  # never delivers; it records each call's argv and whether the baseline already
  # existed at call time, so a test can prove persist happens before any page.
  export POLLER_SEND_ALERT_LOG="$POLLER_HOME/send-alert.log"
  export POLLER_SEND_ALERT_SEVERITY="$POLLER_HOME/send-alert-severity.log"
  export POLLER_SEND_ALERT_STATE_WITNESS="$POLLER_HOME/send-alert-state-witness.log"
  export POLLER_SEND_ALERT_STATE_AT_CALL="$POLLER_HOME/send-alert-state-at-call.log"
  : >"$POLLER_SEND_ALERT_LOG"
  : >"$POLLER_SEND_ALERT_SEVERITY"
  : >"$POLLER_SEND_ALERT_STATE_WITNESS"
  : >"$POLLER_SEND_ALERT_STATE_AT_CALL"
  cat >"$POLLER_HOME/.local/libexec/osquery/alert-dispatch.sh" <<'SHIM'
# shellcheck shell=bash
send_alert() {
  # Severity (arg 1) on its own line: one line per call, so a test counts pages.
  printf '%s\n' "${1:-}" >>"$POLLER_SEND_ALERT_SEVERITY"
  # Full argv (severity, title, body, sound) for body/naming assertions.
  printf '%s\n' "$*" >>"$POLLER_SEND_ALERT_LOG"
  if [[ -f ${OSQUERY_POSTURE_STATE:-/nonexistent} ]]; then
    printf 'state-present\n' >>"$POLLER_SEND_ALERT_STATE_WITNESS"
    # The baseline as it stood when the page fired, so a test can prove the
    # ordering (notify-before-persist: the baseline still holds the PRIOR value).
    cat "$OSQUERY_POSTURE_STATE" >>"$POLLER_SEND_ALERT_STATE_AT_CALL"
  else
    printf 'state-absent\n' >>"$POLLER_SEND_ALERT_STATE_WITNESS"
  fi
  # POLLER_SEND_ALERT_EXIT models a dispatch that could NOT durably queue the
  # page (nonzero). Default 0 (queued): the poller then advances the baseline.
  return "${POLLER_SEND_ALERT_EXIT:-0}"
}
SHIM

  # Default posture: all protections ON (healthy). A test overrides via
  # set_posture before running the poller.
  set_posture '[{"firewall":"1","gatekeeper":"1","screenlock":"1"}]'
}

teardown_poller_harness() {
  [[ -n ${_POLLER_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_POLLER_HARNESS_OWNED_DIR"
  unset _POLLER_HARNESS_OWNED_DIR
}

# run_poller [args...] -- run the poller under the harness env. HOME is the temp
# home so the sourced dispatch spy and the default state path resolve inside the
# sandbox; OSQUERYI points at the recording stub.
run_poller() {
  HOME="$POLLER_HOME" \
    OSQUERYI="$POLLER_OSQUERYI" \
    OSQUERY_POSTURE_STATE="$OSQUERY_POSTURE_STATE" \
    bash "$POLLER_TOOL" "$@"
}

# assert_mode <octal> <path> -- the path carries the expected permission bits.
# GNU stat first (the nix shell), BSD stat as the fallback (the portable order).
assert_mode() {
  local mode
  mode=$(stat -c '%a' "$2" 2>/dev/null || stat -f '%Lp' "$2" 2>/dev/null)
  if [[ $mode != "$1" ]]; then
    printf 'expected mode %s on %s, got %s\n' "$1" "$2" "$mode" >&2
    return 1
  fi
}

# assert_osqueryi_call_count <n> -- the poller invoked osqueryi exactly <n>
# times (one combined query per tick, not one per protection).
assert_osqueryi_call_count() {
  local count
  count=$(wc -l <"$POLLER_OSQUERYI_CALLS") # one marker line per invocation
  count=${count//[[:space:]]/}
  if [[ $count -ne $1 ]]; then
    printf 'expected %s osqueryi call(s), got %s\n' "$1" "$count" >&2
    return 1
  fi
}

# assert_query_reads <substring> -- the recorded osqueryi query contains
# <substring> (a table/column the combined read must ask for).
assert_query_reads() {
  if ! grep -qF -- "$1" "$POLLER_OSQUERYI_QUERY"; then
    printf 'expected the osqueryi query to read %s; query was:\n%s\n' \
      "$1" "$(cat "$POLLER_OSQUERYI_QUERY")" >&2
    return 1
  fi
}

# assert_baseline_scalar <key> <value> -- the persisted baseline's <key> equals
# <value> (a JSON scalar, string-typed as osquery emits).
assert_baseline_scalar() {
  local got
  got=$(jq -r --arg k "$1" '.[$k] // empty' "$OSQUERY_POSTURE_STATE" 2>/dev/null || echo "")
  if [[ $got != "$2" ]]; then
    printf 'expected baseline .%s == %s, got %q; baseline:\n%s\n' \
      "$1" "$2" "$got" "$(cat "$OSQUERY_POSTURE_STATE" 2>/dev/null || echo '(no file)')" >&2
    return 1
  fi
}

# assert_no_page -- the poller did not call send_alert at all (silent).
assert_no_page() {
  if [[ -s $POLLER_SEND_ALERT_LOG ]]; then
    printf 'expected NO page, but send_alert was called:\n%s\n' \
      "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_persist_before_notify -- no send_alert call ever saw a missing baseline,
# so every notification (if any) fired only AFTER the baseline was written.
assert_persist_before_notify() {
  if grep -qF 'state-absent' "$POLLER_SEND_ALERT_STATE_WITNESS" 2>/dev/null; then
    printf 'expected the baseline to exist before any notification, but a send_alert call saw it missing\n' >&2
    return 1
  fi
}

# seed_baseline <compact-json-object> -- write a known-good posture baseline at
# 0600, so a sad-path test can prove a failed or empty read leaves it untouched.
seed_baseline() {
  mkdir -p "$(dirname "$OSQUERY_POSTURE_STATE")"
  printf '%s\n' "$1" >"$OSQUERY_POSTURE_STATE"
  chmod 600 "$OSQUERY_POSTURE_STATE"
}

# snapshot_baseline -- copy the current baseline aside so assert_baseline_unchanged
# can compare byte-for-byte after a run.
snapshot_baseline() {
  cp "$OSQUERY_POSTURE_STATE" "$POLLER_HOME/baseline.snapshot"
}

# assert_baseline_unchanged -- the baseline is byte-for-byte identical to the last
# snapshot (a failed or empty read must neither clobber nor blank it).
assert_baseline_unchanged() {
  if ! cmp -s "$POLLER_HOME/baseline.snapshot" "$OSQUERY_POSTURE_STATE"; then
    printf 'expected the baseline byte-for-byte preserved.\nsnapshot:\n%s\nnow:\n%s\n' \
      "$(cat "$POLLER_HOME/baseline.snapshot" 2>/dev/null || echo '(no snapshot)')" \
      "$(cat "$OSQUERY_POSTURE_STATE" 2>/dev/null || echo '(missing)')" >&2
    return 1
  fi
}

# assert_page_count <n> -- send_alert was called exactly <n> times (one severity
# line per call; the body may span many lines, so the severity log is the count).
assert_page_count() {
  local count
  count=$(wc -l <"$POLLER_SEND_ALERT_SEVERITY")
  count=${count//[[:space:]]/}
  if [[ $count -ne $1 ]]; then
    printf 'expected %s page(s), got %s; send_alert log:\n%s\n' \
      "$1" "$count" "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_severity_is <severity> -- a page fired and every page carried
# <severity> (only a CRIT reaches the #priority webhook, so the severity arg is
# the security-relevant one, not just the title text).
assert_page_severity_is() {
  if [[ ! -s $POLLER_SEND_ALERT_SEVERITY ]]; then
    printf 'expected a %s page, but send_alert was never called\n' "$1" >&2
    return 1
  fi
  if grep -qvxF "$1" "$POLLER_SEND_ALERT_SEVERITY"; then
    printf 'expected every page at severity %s, got:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_SEVERITY")" >&2
    return 1
  fi
}

# assert_page_body_has <substring> -- some page's argv contained <substring> (the
# body names which protection turned off, or its prior state text).
assert_page_body_has() {
  if ! grep -qF -- "$1" "$POLLER_SEND_ALERT_LOG"; then
    printf 'expected a page naming %s; send_alert log:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_body_lacks <substring> -- no page mentioned <substring> (a steady
# protection is never named in an unrelated transition's page).
assert_page_body_lacks() {
  if grep -qF -- "$1" "$POLLER_SEND_ALERT_LOG"; then
    printf 'expected NO page mentioning %s; send_alert log:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_LOG")" >&2
    return 1
  fi
}

# assert_page_saw_baseline <compact-json> -- at the moment send_alert fired, the
# persisted baseline already held <compact-json>, proving write_state ran before
# the page (the ordering that lets a slow alerter never double-page off a stale
# baseline).
assert_page_saw_baseline() {
  if ! grep -qF -- "$1" "$POLLER_SEND_ALERT_STATE_AT_CALL"; then
    printf 'expected the baseline at page time to be %s; saw:\n%s\n' \
      "$1" "$(cat "$POLLER_SEND_ALERT_STATE_AT_CALL" 2>/dev/null || echo '(none)')" >&2
    return 1
  fi
}

# assert_gap_marker -- the page-once monitoring-gap marker (STATE.gap) exists.
assert_gap_marker() {
  if [[ ! -f $OSQUERY_POSTURE_STATE.gap ]]; then
    printf 'expected the gap marker %s.gap to exist, but it does not\n' "$OSQUERY_POSTURE_STATE" >&2
    return 1
  fi
}

# assert_no_gap_marker -- the monitoring-gap marker does not exist (never paged,
# or cleared on recovery).
assert_no_gap_marker() {
  if [[ -f $OSQUERY_POSTURE_STATE.gap ]]; then
    printf 'expected NO gap marker, but %s.gap exists\n' "$OSQUERY_POSTURE_STATE" >&2
    return 1
  fi
}
