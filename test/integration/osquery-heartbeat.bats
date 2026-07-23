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

  # A recording osqueryi stub on PATH (R2-8): if the heartbeat ever shelled a
  # one-shot osqueryi (the reverted anti-pattern), this stub answers FRESH and
  # leaves a marker. The heartbeat must NEVER call it: it reads the daemon's OWN
  # scheduled canary log, so a stopped daemon (a stale canary) is still caught even
  # though a one-shot osqueryi would lie with a fresh answer.
  mkdir -p "$HARNESS_HOME/bin"
  export OSQUERYI_CALLED="$HARNESS_HOME/osqueryi-was-called"
  cat >"$HARNESS_HOME/bin/osqueryi" <<'STUB'
#!/usr/bin/env bash
touch "$OSQUERYI_CALLED"
printf '[{"unix_time":"%s"}]\n' "$(date -u +%s)"
STUB
  chmod +x "$HARNESS_HOME/bin/osqueryi"
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
# home so the sourced spy and default paths resolve inside the sandbox; the temp
# bin is first on PATH so the osqueryi one-shot stub would be found IF the heartbeat
# ever called it, which it must not).
run_heartbeat() {
  HOME="$HARNESS_HOME" \
    OSQUERY_SNAPSHOTS_LOG="$OSQUERY_SNAPSHOTS_LOG" \
    PATH="$HARNESS_HOME/bin:$PATH" \
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

@test "B3: a stale canary reports unhealthy, the stopped-daemon case a one-shot would miss" {
  # GATE (fail-safe, R2-8): osqueryd stopped an hour ago, so the newest scheduled
  # canary is an hour old. A standalone osqueryi one-shot (the stub on PATH) would
  # still answer FRESH and give a blind checkmark; reading the daemon's own canary
  # freshness catches the stopped daemon instead. Reports unhealthy, never healthy.
  seed_canary 3600
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiE "stale|not producing" "$SEND_ALERT_BODY"
  ! grep -qiF "healthy" "$SEND_ALERT_TITLE"
  ! grep -qiF "healthy" "$SEND_ALERT_BODY"
  [ ! -e "$OSQUERYI_CALLED" ] # never shelled a one-shot; it read the scheduled canary
}

@test "B4: the unhealthy message is also silent, the heartbeat never pings even degraded" {
  # GATE (never-pings): even when it reports a problem the heartbeat stays muted. The
  # watchdog owns paging; a degraded heartbeat that pinged would double-signal what
  # the watchdog already pages, and desensitize the operator to real pages.
  seed_canary 3600
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ -z "$(cat "$SEND_ALERT_SOUND")" ]
}

@test "B6: the healthy message is honest about what it verified (R2-8)" {
  # R2-8 honesty: the healthy body claims only what the canary proves (the ROOT
  # DAEMON is alive and running its schedule), points at the watchdog for per-agent
  # liveness, and must NOT overclaim that every monitor is scheduled or loaded.
  seed_canary 30
  run run_heartbeat
  [ "$status" -eq 0 ]
  grep -qiE "daemon|schedule|canary" "$SEND_ALERT_BODY" # it verified the daemon, not a one-shot
  grep -qiF "watchdog" "$SEND_ALERT_BODY"               # points at who owns agent liveness
  ! grep -qiF "all monitors scheduled" "$SEND_ALERT_BODY"
}

@test "B5: no canary at all reports unhealthy as MISSING, never a blind checkmark" {
  # GATE (fail-safe): an empty or absent snapshots log (fresh deploy, or the daemon
  # never ran the schedule) carries no canary row. Not-fresh means unhealthy, the
  # safe direction. The harness default snapshots log is empty, so seed nothing. The
  # message must say MISSING honestly, never mislabel it STALE with a bogus age
  # (an absent timestamp is not a real elapsed age).
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiE "missing|no canary" "$SEND_ALERT_BODY"
  ! grep -qiF "stale" "$SEND_ALERT_BODY"
  ! grep -qiF "healthy" "$SEND_ALERT_TITLE"
  ! grep -qiF "healthy" "$SEND_ALERT_BODY"
}
