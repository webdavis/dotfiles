#!/usr/bin/env bash
# The daily heartbeat (heartbeat.sh): the POSITIVE proof-of-life. Fired daily, it
# sends ONE silent message to #priority so the operator can trust silence = safe.
# R2-8: it must verify the ROOT DAEMON. A standalone osqueryi one-shot answers even
# while osqueryd is stopped or wedged, so instead the heartbeat checks that the
# daemon's OWN scheduled heartbeat_canary snapshot is FRESH. Always muted (its
# message never pings; send_alert's pipeline-broken alarms are audible for every
# producer by design, and this suite stubs send_alert so it judges only the sound
# the heartbeat ASKS for), honest (reports a stale canary rather than a blind
# checkmark); the uptime watchdog is what PAGES.
#
# This suite exercises the script as a black box against a stubbed dispatch: a
# message-recording spy replaces the real send_alert at the exact libexec path the
# script sources, so a test asserts whether (and how) the heartbeat dispatched
# without touching the network or the real SQLite store.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The shebang stays for shellcheck
# and for editors, and is never executed. test/validate-tests.sh pins that shape;
# `just test-integration` runs it.
#
# Every check below is a real bashunit assertion. bashunit runs each test function
# under `set +euo pipefail`, so a bare `grep -q ...` reports nothing and passes
# silently, and a helper that merely `return 1`s is equally invisible. The bats
# file's case-insensitive greps keep that case-insensitivity here rather than
# quietly narrowing to case-sensitive matching: a case-sensitive REFUTE forbids
# less than the original did, which would weaken the test.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HEARTBEAT="$REPO_ROOT/dot_local/libexec/osquery/executable_heartbeat.sh"

function set_up() { set_up_heartbeat_harness; }
function tear_down() { tear_down_heartbeat_harness; }

# set_up_heartbeat_harness (makeSUT factory) - a throwaway HOME whose only dispatch
# library is a recording spy, plus a controllable daemon snapshot log the heartbeat
# reads for canary freshness. Every export happens here, nothing at file-load time.
set_up_heartbeat_harness() {
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
  cp "$REPO_ROOT/dot_local/libexec/osquery/executable_canary-freshness.sh" \
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

# tear_down_heartbeat_harness - remove ONLY a temp dir this harness created. The
# ownership marker is set after our own mktemp, so a pre-set HARNESS_HOME (marker
# unset) is left untouched.
tear_down_heartbeat_harness() {
  [[ -n ${_HEARTBEAT_HARNESS_OWNED_DIR:-} ]] || return 0
  rm -rf "$_HEARTBEAT_HARNESS_OWNED_DIR"
  unset _HEARTBEAT_HARNESS_OWNED_DIR
}

# lowercased_file <file> - the file's text, lowercased. The bats file's positive
# checks matched case-insensitively (grep -i); expressing that as an ordinary
# bashunit assertion over lowercased text keeps the original tolerance instead of
# silently narrowing the assertion to one exact casing.
lowercased_file() { tr '[:upper:]' '[:lower:]' <"$1"; }

# refute_file_contains <fixed-substring> <file> - the substring does NOT appear in
# the file, case-insensitively, exactly as the bats file's `grep -qiF` refute did.
# Written as a bashunit assertion rather than a function that returns 1: bashunit
# runs tests under `set +e`, so a bare `! grep` never fails (bash exempts a
# `!`-inverted command from errexit) and a helper that only returns 1 is just as
# silent. Matching lines become the assertion's "actual", so a failure shows them.
# Case-insensitivity is load-bearing here: a case-sensitive refute would forbid
# less than the original and weaken the test.
refute_file_contains() {
  assert_same '' "$(grep -iF -- "$1" "$2" || true)"
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

# run_heartbeat_capturing - run the heartbeat and record its exit status in
# heartbeat_status. bashunit has no bats `run`, so the status is captured by hand.
# The heartbeat's own stdout and stderr are discarded, as bats' `run` discarded
# them here: no case in this file asserts on the script's output, only on what the
# recording spy captured, so keeping it would just interleave noise into the report.
run_heartbeat_capturing() {
  heartbeat_status=0
  run_heartbeat >/dev/null 2>&1 || heartbeat_status=$?
}

# send_alert_call_count - how many times the recording spy was called.
send_alert_call_count() { grep -c '^CALL$' "$SEND_ALERT_LOG" 2>/dev/null || printf '0'; }

# B1
function test_a_fresh_canary_sends_exactly_one_crit_message_that_reads_healthy() {
  seed_canary 30 # the daemon wrote a canary 30s ago -> alive and scheduling
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  # CRIT is load-bearing: only a CRIT reaches the #priority webhook, so a non-CRIT
  # send would return after the local notification and the daily-message-means-alive
  # protocol would die silently.
  assert_same CRIT "$(cat "$SEND_ALERT_SEVERITY")"
  assert_contains healthy "$(lowercased_file "$SEND_ALERT_TITLE")"
}

# B2
function test_the_healthy_message_is_silent_so_a_proof_of_life_never_pings() {
  # GATE (never-pings): the muted tier is the security invariant. An empty sound
  # keeps the message locally silent AND threads tier=muted into the webhook body,
  # so a daily proof-of-life can never desensitize the operator to a real page.
  seed_canary 30
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_empty "$(cat "$SEND_ALERT_SOUND")"
}

# B3
function test_a_stale_canary_reports_unhealthy_the_stopped_daemon_case_a_one_shot_would_miss() {
  # GATE (fail-safe, R2-8): osqueryd stopped an hour ago, so the newest scheduled
  # canary is an hour old. A standalone osqueryi one-shot (the stub on PATH) would
  # still answer FRESH and give a blind checkmark; reading the daemon's own canary
  # freshness catches the stopped daemon instead. Reports unhealthy, never healthy.
  seed_canary 3600
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  assert_same CRIT "$(cat "$SEND_ALERT_SEVERITY")" # only CRIT reaches #priority
  assert_matches 'stale|not producing' "$(lowercased_file "$SEND_ALERT_BODY")"
  # Precise: refute the healthy TITLE signal, not the bare substring "healthy" (which
  # also matches "unhealthy" - a case-insensitive substring would false-forbid it).
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
  # Never shelled a one-shot; it read the scheduled canary.
  assert_file_not_exists "$OSQUERYI_CALLED"
}

# B4
function test_the_unhealthy_message_is_also_silent_so_the_heartbeat_never_pings_even_degraded() {
  # GATE (never-pings): even when it reports a problem the heartbeat stays muted. The
  # watchdog owns paging; a degraded heartbeat that pinged would double-signal what
  # the watchdog already pages, and desensitize the operator to real pages.
  seed_canary 3600
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_empty "$(cat "$SEND_ALERT_SOUND")"
}

# B5
function test_no_canary_at_all_reports_unhealthy_as_missing_never_a_blind_checkmark() {
  # GATE (fail-safe): an empty or absent snapshots log (fresh deploy, or the daemon
  # never ran the schedule) carries no canary row. Not-fresh means unhealthy, the
  # safe direction. The harness default snapshots log is empty, so seed nothing. The
  # message must say MISSING honestly, never mislabel it STALE with a bogus age
  # (an absent timestamp is not a real elapsed age).
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  assert_same CRIT "$(cat "$SEND_ALERT_SEVERITY")" # only CRIT reaches #priority
  assert_matches 'missing|no canary' "$(lowercased_file "$SEND_ALERT_BODY")"
  refute_file_contains "stale" "$SEND_ALERT_BODY"
  # Precise healthy-signal refute (not the bare "healthy" substring, which matches "unhealthy").
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
}

# B6
function test_the_healthy_message_is_honest_about_what_it_verified() {
  # R2-8 honesty: the healthy body claims only what the canary proves (the ROOT
  # DAEMON is alive and running its schedule), points at the watchdog for per-agent
  # liveness, and must NOT overclaim that every monitor is scheduled or loaded.
  seed_canary 30
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  local body
  body="$(lowercased_file "$SEND_ALERT_BODY")"
  assert_matches 'daemon|schedule|canary' "$body" # it verified the daemon, not a one-shot
  assert_contains watchdog "$body"                # points at who owns agent liveness
  refute_file_contains "all monitors scheduled" "$SEND_ALERT_BODY"
}

# B7
function test_a_malformed_canary_timestamp_is_rejected_unhealthy_and_cannot_inject() {
  # GATE (injection-safety): the ONLY log-derived value the heartbeat touches is the
  # canary timestamp, used solely as $((now - last_ts)) AFTER a ^[0-9]+$ check. A
  # metacharacter-laden value is rejected (treated as MISSING -> unhealthy), never
  # rendered into the message, and never executed. This is why the heartbeat needs
  # no sanitize + code-span wrap: it renders no free-text field, only static text
  # plus a validated-numeric age.
  # The command substitution and backticks MUST stay literal: this is the hostile
  # value the heartbeat has to refuse to execute, so expanding it here would run the
  # payload in the test instead of feeding it to the subject. Only $HARNESS_HOME,
  # which sits outside the quotes, expands.
  # shellcheck disable=SC2016
  local payload='$(touch '"$HARNESS_HOME"'/PWNED)`touch '"$HARNESS_HOME"'/PWNED2`; DROP 9999'
  seed_raw_canary "$payload"
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  # Rejected -> treated as missing.
  assert_matches 'missing|no canary' "$(lowercased_file "$SEND_ALERT_BODY")"
  refute_file_contains "touch" "$SEND_ALERT_BODY" # the raw value never reaches the body
  refute_file_contains "$payload" "$SEND_ALERT_BODY"
  refute_file_contains "touch" "$SEND_ALERT_TITLE"
  # No command execution from the payload.
  assert_file_not_exists "$HARNESS_HOME/PWNED"
  assert_file_not_exists "$HARNESS_HOME/PWNED2"
}

# B7a
function test_an_over_range_canary_epoch_is_rejected_never_a_64_bit_overflow_false_fresh() {
  # A timestamp of 2^64 + now wraps in bash's signed 64-bit back to ~now, so both
  # freshness bounds read fresh and the heartbeat would false-report HEALTHY. The
  # shared seam range-bounds the value, so it is rejected and the heartbeat reports
  # MISSING (unhealthy) instead.
  local overflow
  overflow="$(/usr/bin/bc <<<"$(date -u +%s) + 18446744073709551616")"
  seed_raw_canary "$overflow"
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  assert_matches 'missing|no canary' "$(lowercased_file "$SEND_ALERT_BODY")"
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
}

# B7b
function test_a_leading_zero_canary_epoch_is_rejected_never_an_octal_parse_fall_through() {
  # A leading-zero value (09999999999) makes bash arithmetic parse it as octal and
  # error. The shared seam rejects it, so the heartbeat reports MISSING (unhealthy)
  # instead of aborting or falling through.
  seed_raw_canary '09999999999'
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  assert_matches 'missing|no canary' "$(lowercased_file "$SEND_ALERT_BODY")"
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
}

# B8
function test_freshness_is_judged_from_the_newest_canary_row_when_several_exist() {
  # osqueryd appends one canary per interval, so the LAST line is the newest. A run
  # of rows (an old one from before a gap, then a fresh one) must be judged by the
  # freshest (last), not the first: a daemon that stopped and is producing again is
  # healthy now. This pins the tail-1 selection (a head-1 bug would see only the old
  # row and false-alarm).
  seed_canary 5000 # an old canary, from before a gap
  seed_canary 30   # the newest canary: the daemon is producing again
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  assert_contains healthy "$(lowercased_file "$SEND_ALERT_TITLE")"
}

# format-tolerance
function test_a_spaced_json_canary_reads_the_same_as_compact() {
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
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  assert_contains healthy "$(lowercased_file "$SEND_ALERT_TITLE")"
}

# clock-skew
function test_a_future_dated_canary_reads_healthy_with_a_non_negative_rendered_age() {
  # An NTP step-back can leave the newest canary timestamped slightly AHEAD of now. It
  # is still fresh (the daemon is producing recent results), so it reads healthy; the
  # rendered age is clamped to >= 0 so the silent daily message never shows a
  # nonsensical negative age like "(-120s ago)".
  local ts
  ts=$(($(date -u +%s) + 120)) # 2 minutes in the future (the clock stepped back)
  jq -cn --argjson t "$ts" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:($t|tostring)}],unixTime:$t,hostIdentifier:"dresden"}' \
    >>"$OSQUERY_SNAPSHOTS_LOG"
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_contains healthy "$(lowercased_file "$SEND_ALERT_TITLE")"
  # Never a negative age such as "(-120s ago)".
  refute_file_contains "(-" "$SEND_ALERT_BODY"
}

# healthy-honesty
function test_the_healthy_body_is_a_recent_observation_not_a_present_tense_overclaim() {
  # A fresh canary proves only that osqueryd produced a scheduled result up to
  # canary_max_age AGO, not that it is alive RIGHT NOW (real-time liveness is the
  # watchdog's job). The healthy body must state that recent observation, not
  # present-tense current liveness.
  seed_canary 30
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  local body
  body="$(lowercased_file "$SEND_ALERT_BODY")"
  assert_contains 'produced a scheduled heartbeat canary' "$body" # honest recent observation
  assert_contains 'as recently as that' "$body"
  # Present-tense overclaims.
  refute_file_contains "is alive and running its schedule" "$SEND_ALERT_BODY"
  refute_file_contains "verifies the root daemon" "$SEND_ALERT_BODY"
}

# implausible-future
function test_a_canary_far_in_the_future_reports_unhealthy_implausible_not_healthy() {
  # GATE (fail-safe): the freshness window is TWO-SIDED. A canary timestamped well
  # beyond the window in the FUTURE (clock skew or a bad row) is not a trustworthy
  # liveness signal, so it fails the future half and reports unhealthy IMPLAUSIBLE,
  # never healthy. (A small +120s skew stays healthy; see the clock-skew case.) The
  # rendered skew is a POSITIVE number.
  local ts
  ts=$(($(date -u +%s) + 100000)) # far beyond any reasonable freshness window
  jq -cn --argjson t "$ts" \
    '{name:"heartbeat_canary",action:"snapshot",snapshot:[{unix_time:($t|tostring)}],unixTime:$t,hostIdentifier:"dresden"}' \
    >>"$OSQUERY_SNAPSHOTS_LOG"
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
  assert_matches 'implausible|future' "$(lowercased_file "$SEND_ALERT_BODY")"
  # A positive skew, never a negative number.
  refute_file_contains "(-" "$SEND_ALERT_BODY"
}

# clock-failure
function test_a_non_numeric_clock_reports_unhealthy_never_false_healthy_via_now_zero() {
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
  run_heartbeat_capturing
  assert_exit_code 0 "" "$heartbeat_status"
  assert_same 1 "$(send_alert_call_count)"
  refute_file_contains "pipeline healthy" "$SEND_ALERT_TITLE"
  assert_matches 'cannot determine|current time' "$(lowercased_file "$SEND_ALERT_BODY")"
}

# seam
function test_newest_canary_timestamp_returns_the_newest_validated_integer_else_empty() {
  # The read is extracted into a directly testable seam; the source-guard lets a test
  # source the script without launching main. An empty log -> empty; a well-formed
  # canary -> a plain integer; a non-numeric value -> validated to empty at this one site.
  # shellcheck source=/dev/null
  source "$HEARTBEAT"
  local first status=0
  first="$(newest_canary_timestamp)" || status=$?
  assert_exit_code 0 "" "$status"
  assert_empty "$first" # no canary row yet
  seed_canary 42
  assert_matches '^[0-9]+$' "$(newest_canary_timestamp)" # a plain integer
  : >"$OSQUERY_SNAPSHOTS_LOG"
  seed_raw_canary "not-a-number"
  # Malformed -> validated to empty, never reaches the decision.
  assert_empty "$(newest_canary_timestamp)"
}

# fire-and-forget
function test_a_hard_send_failure_never_fails_the_heartbeat() {
  # The heartbeat advances no state and delegates durability to send_alert, so a send
  # that returns nonzero (a hard persist failure) must not fail the launchd job: the
  # next day re-fires and the watchdog is the real safety net. SEND_ALERT_RC=1 forces
  # the spy to fail; the heartbeat's `|| true` swallows it and still exits 0.
  seed_canary 30
  local status=0
  SEND_ALERT_RC=1 run_heartbeat >/dev/null 2>&1 || status=$?
  assert_exit_code 0 "" "$status"
  assert_same 1 "$(send_alert_call_count)" # it did attempt the send
}
