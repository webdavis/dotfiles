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

  # The heartbeat sources the shared canary-freshness seam from the deployed libexec
  # path (newest_canary_timestamp lives there now, shared with the uptime watchdog);
  # install the real helper into the sandbox so that source resolves.
  cp "${BATS_TEST_DIRNAME}/../../dot_local/libexec/osquery/executable_canary-freshness.sh" \
    "$dispatch_dir/canary-freshness.sh"

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

# refute_file_contains <fixed-substring> <file> - fail (return 1) when the substring
# (fixed, case-insensitive) appears in the file. This is the robust NEGATIVE
# assertion: a bare `! grep ...` is NOT reliable in bats, because bats runs each test
# under set -e and bash exempts a `!`-inverted command from set -e ("the return
# status is being inverted with !"), so `! grep` NEVER fails the test - a silent
# no-op. A plain function whose non-zero return set -e DOES catch closes that gap.
refute_file_contains() {
  if grep -qiF -- "$1" "$2"; then
    printf 'expected %q NOT to appear in %s, but it does:\n%s\n' "$1" "$2" "$(cat "$2")" >&2
    return 1
  fi
  return 0
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

# seed_raw_canary <unix_time-value> - append a heartbeat_canary row whose unix_time
# and unixTime carry an ARBITRARY (possibly attacker-controlled) string, to model a
# tampered or malformed snapshot log. jq JSON-encodes the value, so the heartbeat
# reads it back verbatim as a string via jq -r.
seed_raw_canary() {
  jq -cn --arg t "$1" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:$t}],unixTime:$t,hostIdentifier:"dresden"}' \
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

@test "B1: a fresh canary sends exactly one CRIT message that reads healthy" {
  seed_canary 30 # the daemon wrote a canary 30s ago -> alive and scheduling
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  # CRIT is load-bearing: only a CRIT reaches the #priority webhook, so a non-CRIT
  # send would return after the local notification and the daily-message-means-alive
  # protocol would die silently.
  [ "$(cat "$SEND_ALERT_SEVERITY")" = "CRIT" ]
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
  [ "$(cat "$SEND_ALERT_SEVERITY")" = "CRIT" ] # only CRIT reaches #priority
  grep -qiE "stale|not producing" "$SEND_ALERT_BODY"
  # Precise: refute the healthy TITLE signal, not the bare substring "healthy" (which
  # also matches "unhealthy" - a case-insensitive substring would false-forbid it).
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
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

@test "B5: no canary at all reports unhealthy as MISSING, never a blind checkmark" {
  # GATE (fail-safe): an empty or absent snapshots log (fresh deploy, or the daemon
  # never ran the schedule) carries no canary row. Not-fresh means unhealthy, the
  # safe direction. The harness default snapshots log is empty, so seed nothing. The
  # message must say MISSING honestly, never mislabel it STALE with a bogus age
  # (an absent timestamp is not a real elapsed age).
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  [ "$(cat "$SEND_ALERT_SEVERITY")" = "CRIT" ] # only CRIT reaches #priority
  grep -qiE "missing|no canary" "$SEND_ALERT_BODY"
  refute_file_contains "stale" "$SEND_ALERT_BODY"
  # Precise healthy-signal refute (not the bare "healthy" substring, which matches "unhealthy").
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
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
  refute_file_contains "all monitors scheduled" "$SEND_ALERT_BODY"
}

@test "B7: a malformed canary timestamp is rejected, unhealthy, and cannot inject" {
  # GATE (injection-safety): the ONLY log-derived value the heartbeat touches is the
  # canary timestamp, used solely as $((now - last_ts)) AFTER a ^[0-9]+$ check. A
  # metacharacter-laden value is rejected (treated as MISSING -> unhealthy), never
  # rendered into the message, and never executed. This is why the heartbeat needs
  # no sanitize + code-span wrap: it renders no free-text field, only static text
  # plus a validated-numeric age.
  local payload='$(touch '"$HARNESS_HOME"'/PWNED)`touch '"$HARNESS_HOME"'/PWNED2`; DROP 9999'
  seed_raw_canary "$payload"
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiE "missing|no canary" "$SEND_ALERT_BODY" # rejected -> treated as missing
  refute_file_contains "touch" "$SEND_ALERT_BODY"  # the raw value never reaches the body
  refute_file_contains "$payload" "$SEND_ALERT_BODY"
  refute_file_contains "touch" "$SEND_ALERT_TITLE"
  [ ! -e "$HARNESS_HOME/PWNED" ]  # no command execution from the payload
  [ ! -e "$HARNESS_HOME/PWNED2" ] # no command execution from the payload
}

@test "B7a: an over-range canary epoch is rejected (fail-safe MISSING), never a 64-bit-overflow false fresh" {
  # A timestamp of 2^64 + now wraps in bash's signed 64-bit back to ~now, so both
  # freshness bounds read fresh and the heartbeat would false-report HEALTHY. The
  # shared seam range-bounds the value, so it is rejected and the heartbeat reports
  # MISSING (unhealthy) instead.
  local overflow
  overflow="$(/usr/bin/bc <<<"$(date -u +%s) + 18446744073709551616")"
  seed_raw_canary "$overflow"
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiE "missing|no canary" "$SEND_ALERT_BODY"
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
}

@test "B7b: a leading-zero canary epoch is rejected (fail-safe MISSING), never an octal-parse fall-through" {
  # A leading-zero value (09999999999) makes bash arithmetic parse it as octal and
  # error. The shared seam rejects it, so the heartbeat reports MISSING (unhealthy)
  # instead of aborting or falling through.
  seed_raw_canary '09999999999'
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiE "missing|no canary" "$SEND_ALERT_BODY"
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
}

@test "B8: freshness is judged from the NEWEST canary row when several exist" {
  # osqueryd appends one canary per interval, so the LAST line is the newest. A run
  # of rows (an old one from before a gap, then a fresh one) must be judged by the
  # freshest (last), not the first: a daemon that stopped and is producing again is
  # healthy now. This pins the tail-1 selection (a head-1 bug would see only the old
  # row and false-alarm).
  seed_canary 5000 # an old canary, from before a gap
  seed_canary 30   # the newest canary: the daemon is producing again
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiF "healthy" "$SEND_ALERT_TITLE"
}

@test "format-tolerance: a spaced-JSON canary reads the same as compact (JSON-semantic reader)" {
  # osquery 5.23.1 emits COMPACT single-line JSON (verified against the real deployed
  # osqueryd.snapshots.log on this host), but the reader must not couple to that byte
  # layout: it selects the canary by PARSED .name via fromjson? (the same idiom
  # normalize.sh uses), so a spaced serialization is read identically. A compact
  # grep -F would MISS a spaced line and false-report the canary MISSING (fail-safe,
  # but perpetual daily noise). This line is deliberately spaced (space after each colon).
  local ts
  ts=$(($(date -u +%s) - 30))
  printf '{"name": "heartbeat_canary", "action": "snapshot", "unixTime": %s, "snapshot": [{"unix_time": "%s"}]}\n' \
    "$ts" "$ts" >>"$OSQUERY_SNAPSHOTS_LOG"
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  grep -qiF "healthy" "$SEND_ALERT_TITLE"
}

@test "clock-skew: a future-dated canary reads healthy with a non-negative rendered age" {
  # An NTP step-back can leave the newest canary timestamped slightly AHEAD of now. It
  # is still fresh (the daemon is producing recent results), so it reads healthy; the
  # rendered age is clamped to >= 0 so the silent daily message never shows a
  # nonsensical negative age like "(-120s ago)".
  local ts
  ts=$(($(date -u +%s) + 120)) # 2 minutes in the future (the clock stepped back)
  jq -cn --argjson t "$ts" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:($t|tostring)}],unixTime:$t,hostIdentifier:"dresden"}' \
    >>"$OSQUERY_SNAPSHOTS_LOG"
  run run_heartbeat
  [ "$status" -eq 0 ]
  grep -qiF "healthy" "$SEND_ALERT_TITLE"
  refute_file_contains "(-" "$SEND_ALERT_BODY" # never a negative age such as "(-120s ago)"
}

@test "healthy-honesty: the healthy body is a recent observation, not a present-tense overclaim" {
  # A fresh canary proves only that osqueryd produced a scheduled result up to
  # canary_max_age AGO, not that it is alive RIGHT NOW (real-time liveness is the
  # watchdog's job). The healthy body must state that recent observation, not
  # present-tense current liveness.
  seed_canary 30
  run run_heartbeat
  [ "$status" -eq 0 ]
  grep -qiF "produced a scheduled heartbeat canary" "$SEND_ALERT_BODY" # honest recent observation
  grep -qiF "as recently as that" "$SEND_ALERT_BODY"
  refute_file_contains "is alive and running its schedule" "$SEND_ALERT_BODY" # present-tense overclaim
  refute_file_contains "verifies the root daemon" "$SEND_ALERT_BODY"
}

@test "implausible-future: a canary far in the future reports unhealthy IMPLAUSIBLE, not healthy" {
  # GATE (fail-safe): the freshness window is TWO-SIDED. A canary timestamped well
  # beyond the window in the FUTURE (clock skew or a bad row) is not a trustworthy
  # liveness signal, so it fails the future half and reports unhealthy IMPLAUSIBLE,
  # never healthy. (A small +120s skew stays healthy; see clock-skew.) The rendered
  # skew is a POSITIVE number.
  local ts
  ts=$(($(date -u +%s) + 100000)) # far beyond any reasonable freshness window
  jq -cn --argjson t "$ts" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:($t|tostring)}],unixTime:$t,hostIdentifier:"dresden"}' \
    >>"$OSQUERY_SNAPSHOTS_LOG"
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
  grep -qiE "implausible|future" "$SEND_ALERT_BODY"
  refute_file_contains "(-" "$SEND_ALERT_BODY" # a positive skew, never a negative number
}

@test "clock-failure: a non-numeric clock reports unhealthy, never false-healthy via now=0" {
  # GATE (fail-safe): if the system clock read returns non-numeric (or fails), the
  # heartbeat cannot judge freshness. It must NOT fall back to now=0 (which makes
  # every historical canary look fresh, a false-healthy); it reports unhealthy that it
  # cannot determine the current time. A real, fresh canary is seeded to prove even
  # that does not read healthy without a trustworthy clock.
  seed_canary 30
  cat >"$HARNESS_HOME/bin/date" <<'STUB'
#!/usr/bin/env bash
printf 'not-a-time\n'
STUB
  chmod +x "$HARNESS_HOME/bin/date"
  run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ]
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
  grep -qiE "cannot determine|current time" "$SEND_ALERT_BODY"
}

@test "seam: newest_canary_timestamp returns the newest validated integer, else empty" {
  # The read is extracted into a directly testable seam; the source-guard lets a test
  # source the script without launching main. An empty log -> empty; a well-formed
  # canary -> a plain integer; a non-numeric value -> validated to empty at this one site.
  source "$HEARTBEAT"
  run newest_canary_timestamp
  [ "$status" -eq 0 ]
  [ -z "$output" ] # no canary row yet
  seed_canary 42
  run newest_canary_timestamp
  [[ "$output" =~ ^[0-9]+$ ]] # a plain integer
  : >"$OSQUERY_SNAPSHOTS_LOG"
  seed_raw_canary "not-a-number"
  run newest_canary_timestamp
  [ -z "$output" ] # malformed -> validated to empty, never reaches the decision
}

@test "fire-and-forget: a hard send failure never fails the heartbeat (exit 0)" {
  # The heartbeat advances no state and delegates durability to send_alert, so a send
  # that returns nonzero (a hard persist failure) must not fail the launchd job: the
  # next day re-fires and the watchdog is the real safety net. SEND_ALERT_RC=1 forces
  # the spy to fail; the heartbeat's `|| true` swallows it and still exits 0.
  seed_canary 30
  SEND_ALERT_RC=1 run run_heartbeat
  [ "$status" -eq 0 ]
  [ "$(grep -c '^CALL$' "$SEND_ALERT_LOG")" -eq 1 ] # it did attempt the send
}
