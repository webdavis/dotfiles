#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=test/fixtures/osquery-poller-lib.bash
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/test/fixtures/osquery-poller-lib.bash"

function set_up() {
  setup_poller_harness
  relocated_controls="$POLLER_HOME/.local/libexec/posture/controls.json"
  mkdir -p "$(dirname "$relocated_controls")"
  cp "$OSQUERY_POSTURE_CONTROLS" "$relocated_controls"
}

function run_default_poller() {
  # shellcheck disable=SC2119 # This invocation deliberately supplies no poll operands.
  OSQUERY_POSTURE_CONTROLS="" run_poller >"$POLLER_HOME/stdout" 2>"$POLLER_HOME/stderr"
}

function test_the_default_controls_seed_the_baseline_without_a_gap() {
  run_default_poller
  assert_successful_code
  assert_empty "$(cat "$POLLER_HOME/stderr")"
  assert_empty "$(cat "$POLLER_SEND_ALERT_LOG")"
  assert_same on "$(jq -r .filevault "$OSQUERY_POSTURE_STATE")"
  assert_same 'fdesetup status' "$(cat "$POLLER_PROBE_CALLS")"
  assert_empty "$(cat "$POLLER_MUTATION_LOG")"
}

function test_stale_legacy_controls_cannot_hide_a_missing_relocated_file() {
  mv "$relocated_controls" "$POLLER_HOME/.local/libexec/osquery/posture-controls.json"
  run_default_poller
  assert_successful_code
  assert_contains 'posture-controls file missing at' "$(cat "$POLLER_SEND_ALERT_LOG")"
  assert_contains "$relocated_controls" "$(cat "$POLLER_SEND_ALERT_LOG")"
  assert_same controls_file "$(cat "$OSQUERY_POSTURE_STATE.gap")"
  assert_empty "$(cat "$POLLER_PROBE_CALLS")"
}

function test_an_unreadable_relocated_file_reports_the_open_error_and_pages_the_gap() {
  chmod 000 "$relocated_controls"
  run_default_poller
  assert_successful_code
  assert_contains "$relocated_controls" "$(cat "$POLLER_HOME/stderr")"
  assert_contains 'Permission denied' "$(cat "$POLLER_HOME/stderr")"
  assert_contains 'not a JSON array' "$(cat "$POLLER_SEND_ALERT_LOG")"
  assert_same controls_file "$(cat "$OSQUERY_POSTURE_STATE.gap")"
  assert_empty "$(cat "$POLLER_PROBE_CALLS")"
}

function test_an_explicit_controls_override_still_drives_the_observation() {
  printf '[]\n' >"$relocated_controls"
  # shellcheck disable=SC2119 # This invocation deliberately supplies no poll operands.
  run_poller >"$POLLER_HOME/stdout" 2>"$POLLER_HOME/stderr"
  assert_successful_code
  assert_empty "$(cat "$POLLER_HOME/stderr")"
  assert_empty "$(cat "$POLLER_SEND_ALERT_LOG")"
  assert_same on "$(jq -r .filevault "$OSQUERY_POSTURE_STATE")"
}
