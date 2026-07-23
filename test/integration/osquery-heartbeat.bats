#!/usr/bin/env bats
# The daily heartbeat (heartbeat.sh): the POSITIVE proof-of-life. Fired daily, it
# sends ONE silent message to #priority so the operator can trust silence = safe.
# R2-8: it must verify the ROOT DAEMON. A standalone osqueryi one-shot answers even
# while osqueryd is stopped or wedged, so instead the heartbeat checks that the
# daemon's OWN scheduled heartbeat_canary snapshot is FRESH. Always muted (never
# pings), honest (reports a stale canary rather than a blind checkmark); the uptime
# watchdog is what PAGES.
#
# This suite exercises the script as a black box against a stubbed dispatch: a
# message-recording spy replaces the real send_alert at the exact libexec path the
# script sources, so a test asserts whether (and how) the heartbeat dispatched
# without touching the network or the real SQLite store.

HEARTBEAT="${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_heartbeat.sh"

setup() { setup_heartbeat_harness; }
teardown() { teardown_heartbeat_harness; }

# setup_heartbeat_harness (makeSUT factory) - a throwaway HOME whose only dispatch
# library is a recording spy, plus a controllable daemon snapshot log the heartbeat
# reads for canary freshness. Every export happens here, nothing at file-load time.
setup_heartbeat_harness() {
  HARNESS_HOME="$(mktemp -d)"
  # Record ownership only after our own mktemp, so teardown removes this path and
  # never a pre-set or inherited HARNESS_HOME.
  _HEARTBEAT_HARNESS_OWNED_DIR="$HARNESS_HOME"
  export HOME="$HARNESS_HOME"

  # The recording spy for send_alert, at the exact libexec path the heartbeat
  # sources. One CALL marker per call (so a test counts sends and "no dispatch" is
  # an empty log) plus the severity/title/body/sound of the LAST call, so a test
  # asserts HOW it dispatched without a real send. SEND_ALERT_RC (default 0) lets a
  # test force a hard send failure.
  local dispatch_dir="$HARNESS_HOME/.local/libexec/osquery"
  mkdir -p "$dispatch_dir"
  export SEND_ALERT_LOG="$HARNESS_HOME/send-alert.log"
  export SEND_ALERT_SEVERITY="$HARNESS_HOME/send-alert.severity"
  export SEND_ALERT_TITLE="$HARNESS_HOME/send-alert.title"
  export SEND_ALERT_BODY="$HARNESS_HOME/send-alert.body"
  export SEND_ALERT_SOUND="$HARNESS_HOME/send-alert.sound"
  : >"$SEND_ALERT_LOG"
  cat >"$dispatch_dir/alert-dispatch.sh" <<'SPY'
# Recording spy for alert-dispatch.sh: capture each send_alert call so a test can
# assert whether, and how, the heartbeat dispatched without a real send.
send_alert() {
  printf 'CALL\n' >>"$SEND_ALERT_LOG"
  printf '%s' "${1-}" >"$SEND_ALERT_SEVERITY"
  printf '%s' "${2-}" >"$SEND_ALERT_TITLE"
  printf '%s' "${3-}" >"$SEND_ALERT_BODY"
  printf '%s' "${4-}" >"$SEND_ALERT_SOUND"
  return "${SEND_ALERT_RC:-0}"
}
SPY

  # The daemon snapshot log the heartbeat reads for canary freshness. Left EMPTY by
  # default (a fresh deploy: the daemon has written no canary yet), so a test opts in
  # to a fresh, stale, or malformed canary.
  export OSQUERY_SNAPSHOTS_LOG="$HARNESS_HOME/.local/log/osquery/osqueryd.snapshots.log"
  mkdir -p "$(dirname "$OSQUERY_SNAPSHOTS_LOG")"
  : >"$OSQUERY_SNAPSHOTS_LOG"
}

# teardown_heartbeat_harness - remove ONLY a temp dir this harness created. The
# ownership marker is set after our own mktemp, so a pre-set HARNESS_HOME (marker
# unset) is left untouched.
teardown_heartbeat_harness() {
  [[ -n ${_HEARTBEAT_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_HEARTBEAT_HARNESS_OWNED_DIR"
  unset _HEARTBEAT_HARNESS_OWNED_DIR
}

# seed_canary <seconds-ago> - append a heartbeat_canary snapshot row timestamped
# that many seconds in the past, in the shape osqueryd writes to the snapshot log.
seed_canary() {
  local ts
  ts=$(($(date -u +%s) - $1))
  jq -cn --argjson t "$ts" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:($t|tostring)}],unixTime:$t,hostIdentifier:"dresden"}' \
    >>"$OSQUERY_SNAPSHOTS_LOG"
}

# run_heartbeat - run the real heartbeat under the harness env (HOME is the temp
# home so the sourced spy and default paths resolve inside the sandbox).
run_heartbeat() {
  HOME="$HARNESS_HOME" \
    OSQUERY_SNAPSHOTS_LOG="$OSQUERY_SNAPSHOTS_LOG" \
    bash "$HEARTBEAT"
}

@test "B1: a fresh canary sends exactly one message that reads healthy" {
  seed_canary 30 # the daemon wrote a canary 30s ago -> alive and scheduling
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiF "healthy" "$SEND_ALERT_TITLE"
}

@test "B2: the healthy message is silent (empty sound), a proof-of-life never pings" {
  # GATE (never-pings): the muted tier is the security invariant. An empty sound
  # keeps the message locally silent AND threads tier=muted into the webhook body,
  # so a daily proof-of-life can never desensitize the operator to a real page.
  seed_canary 30
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ -z "$(cat "$SEND_ALERT_SOUND")" ]
}
