#!/usr/bin/env bash
# calendar-busy-window.sh, the reference implementer of pns's [quiet.calendar]
# command. Every test here runs a fake `gog` first on PATH, so nothing reaches
# the operator's real account or ~/.config/gogcli.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit; test/validate-tests.sh pins the shape. assert_same, never
# assert_equals: the latter normalizes control characters away (0.50.1).

repo_root() {
  printf '%s' "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
}

set_up_before_script() {
  SCRIPT="$(repo_root)/dot_local/libexec/pns/executable_calendar-busy-window.sh"
}

set_up() {
  sandbox="$(mktemp -d)"
  stubbin="$sandbox/bin"
  mkdir -p "$stubbin"
}

tear_down() {
  rm -rf "$sandbox"
}

# Installs a fake gog that prints the given fixture body verbatim as its
# --json --results-only answer.
install_gog_fixture() {
  cat >"$stubbin/gog" <<STUB
#!/usr/bin/env bash
cat <<'FIXTURE'
$1
FIXTURE
STUB
  chmod +x "$stubbin/gog"
}

install_failing_gog() {
  cat >"$stubbin/gog" <<'STUB'
#!/usr/bin/env bash
exit 1
STUB
  chmod +x "$stubbin/gog"
}

run_script() {
  PATH="$stubbin:$PATH" "$SCRIPT" >"$sandbox/stdout" 2>"$sandbox/stderr"
}

# One event of each kind the script must tell apart: a timed busy event, a
# timed event marked transparent, a cancelled timed event, an all-day event
# (start.date, no dateTime), and a timed event on a non-UTC offset.
MIXED_FIXTURE='[
  {"summary": "standup", "start": {"dateTime": "2026-09-20T15:00:00Z"}, "end": {"dateTime": "2026-09-20T15:30:00Z"}},
  {"summary": "focus block", "start": {"dateTime": "2026-09-20T16:00:00Z"}, "end": {"dateTime": "2026-09-20T16:30:00Z"}, "transparency": "transparent"},
  {"summary": "dropped", "start": {"dateTime": "2026-09-20T17:00:00Z"}, "end": {"dateTime": "2026-09-20T17:30:00Z"}, "status": "cancelled"},
  {"summary": "offsite", "start": {"date": "2026-09-20"}, "end": {"date": "2026-09-21"}},
  {"summary": "east coast call", "start": {"dateTime": "2026-09-20T09:00:00-04:00"}, "end": {"dateTime": "2026-09-20T09:30:00-04:00"}}
]'

function test_a_timed_busy_event_reports_busy_true() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  assert_same 0 "$?"
  assert_same 'true' "$(jq -r '.events[] | select(.start == 1789916400) | .busy' "$sandbox/stdout")"
}

function test_a_transparent_event_reports_busy_false_rather_than_being_dropped() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  assert_same 'false' "$(jq -r '.events[] | select(.start == 1789920000) | .busy' "$sandbox/stdout")"
}

function test_a_cancelled_event_reports_busy_false_rather_than_being_dropped() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  assert_same 'false' "$(jq -r '.events[] | select(.start == 1789923600) | .busy' "$sandbox/stdout")"
}

function test_an_all_day_event_is_skipped_entirely() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  assert_same '0' "$(jq '[.events[] | select(.start == null)] | length' "$sandbox/stdout")"
  assert_same '4' "$(jq '.events | length' "$sandbox/stdout")"
}

function test_a_non_utc_offset_is_converted_to_the_correct_epoch_second() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  # 2026-09-20T09:00:00-04:00 is 13:00:00Z.
  assert_same 'true' "$(jq -r '.events[] | select(.start == 1789909200) | .busy' "$sandbox/stdout")"
  assert_same '1789911000' "$(jq -r '.events[] | select(.start == 1789909200) | .end' "$sandbox/stdout")"
}

function test_events_are_sorted_by_start() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  assert_same '1789909200
1789916400
1789920000
1789923600' "$(jq -r '.events[].start' "$sandbox/stdout")"
}

function test_stdout_is_exactly_the_documented_shape() {
  install_gog_fixture "$MIXED_FIXTURE"
  run_script
  jq -e '.events | type == "array"' "$sandbox/stdout" >/dev/null
  assert_same 0 "$?"
  assert_same '["busy","end","start"]' "$(jq -c '[.events[0] | keys] | .[0]' "$sandbox/stdout")"
}

function test_a_gog_that_exits_non_zero_fails_closed() {
  install_failing_gog
  local status=0
  run_script || status=$?
  assert_not_same 0 "$status"
  assert_empty "$(cat "$sandbox/stdout")"
  assert_same 1 "$(wc -l <"$sandbox/stderr" | tr -d ' ')"
}

function test_a_gog_that_prints_garbage_fails_closed() {
  install_gog_fixture 'not json at all'
  local status=0
  run_script || status=$?
  assert_not_same 0 "$status"
  assert_empty "$(cat "$sandbox/stdout")"
  assert_same 1 "$(wc -l <"$sandbox/stderr" | tr -d ' ')"
}

function test_failure_output_never_quotes_event_data() {
  install_gog_fixture 'not json at all'
  run_script
  ! grep -q 'not json at all' "$sandbox/stderr"
  assert_same 0 "$?"
}
