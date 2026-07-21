#!/usr/bin/env bash
# nameref-guards.sh -- regression suite for the two nameref-output capture
# helpers (test/test-system/helpers/capture-output.sh and
# test/helpers/run-dispatch-and-drain.sh). Each helper writes results through
# caller-named namerefs, so a destination name that matches one of the helper's
# own internal locals would make the nameref alias the helper instead of the
# caller: the call returns 0 and the caller's variable silently never receives
# the write. The guard contract under test: every internal local is prefixed
# with the full function name, any destination name matching that prefix is
# rejected up front, and the two destination names must differ.
# Collect-then-report: every failed assertion is recorded and reported at the end.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=test/test-system/helpers/report-test-failures.sh
source "$REPO_ROOT/test/test-system/helpers/report-test-failures.sh"
# shellcheck source=test/test-system/helpers/capture-output.sh
source "$REPO_ROOT/test/test-system/helpers/capture-output.sh"
# shellcheck source=test/helpers/run-dispatch-and-drain.sh
source "$REPO_ROOT/test/helpers/run-dispatch-and-drain.sh"

# The probe that exposed the reserve-list hole: the guard's own loop variable
# was an internal local missing from the hand-kept reserve list, so the nameref
# aliased it and the caller's variable silently never received the write.
assert_capture_output_rejects_its_own_loop_variable_name() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(capture_output capture_output_reserved_name harmless_status_name true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "capture_output accepted its own internal loop-variable name as a destination (rc 0; the write would be silently lost)"
  [[ $rejection_output == *capture_output_* ]] ||
    record_failure "the capture_output prefix rejection should name the offending prefix (got: $rejection_output)"
}

# The prefix guard must cover names no current internal uses yet, so a future
# internal local can never reopen the hole.
assert_capture_output_rejects_any_prefixed_name() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(capture_output harmless_output_name capture_output_probe_of_a_future_internal true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "capture_output must reject ANY destination matching its internal prefix, including names no internal uses yet (rc 0)"
}

assert_capture_output_normal_names_still_work() {
  local probe_output probe_status
  capture_output probe_output probe_status printf 'captured-text'
  [[ $probe_status -eq 0 ]] ||
    record_failure "capture_output with ordinary names should report the command's exit 0 (got $probe_status)"
  [[ $probe_output == "captured-text" ]] ||
    record_failure "capture_output with ordinary names should deliver the output (got: $probe_output)"
}

# _capture_dispatch_run: same contract, prefixed with ITS full function name.
assert_dispatch_capture_rejects_prefixed_output_name() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(_capture_dispatch_run _capture_dispatch_run_errexit_was_on harmless_status_name true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "_capture_dispatch_run accepted a destination matching its internal prefix (rc 0; the write would be silently lost)"
  [[ $rejection_output == *_capture_dispatch_run_* ]] ||
    record_failure "the _capture_dispatch_run prefix rejection should name the offending prefix (got: $rejection_output)"
}

assert_dispatch_capture_rejects_prefixed_status_name() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(_capture_dispatch_run harmless_output_name _capture_dispatch_run_probe_of_a_future_internal true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "_capture_dispatch_run must reject ANY status destination matching its internal prefix (rc 0)"
}

assert_dispatch_capture_rejects_identical_names() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(_capture_dispatch_run same_name same_name true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "_capture_dispatch_run must reject identical output and status destinations (rc 0)"
  [[ $rejection_output == *different* ]] ||
    record_failure "the identical-names rejection should say the names must differ (got: $rejection_output)"
}

assert_dispatch_capture_normal_names_still_work() {
  local probe_output probe_status
  _capture_dispatch_run probe_output probe_status printf 'dispatch-captured'
  [[ $probe_status -eq 0 ]] ||
    record_failure "_capture_dispatch_run with ordinary names should report the command's exit 0 (got $probe_status)"
  [[ $probe_output == "dispatch-captured" ]] ||
    record_failure "_capture_dispatch_run with ordinary names should deliver the output (got: $probe_output)"
}

# capture_output must restore the caller's ORIGINAL errexit state, not force
# set -e: a caller that deliberately runs under set +e must stay set +e after a
# capture, so its own later failing commands do not abort the script.
assert_capture_output_preserves_errexit_off_caller() {
  local probe_output probe_status
  set +e
  probe_output="$(bash -c '
    source "$1"
    set +e
    captured="" status=""
    capture_output captured status true
    false
    echo SURVIVED
  ' _ "$REPO_ROOT/test/test-system/helpers/capture-output.sh" 2>&1)"
  probe_status=$?
  set -e
  [[ $probe_status -eq 0 && $probe_output == *SURVIVED* ]] ||
    record_failure "capture_output must not turn a set +e caller into set -e (rc=$probe_status, output: $probe_output)"
}

# The other direction: a set -e caller keeps errexit after the capture (the
# capture of a failing command itself must not abort, but a later bare failing
# command must still do so).
assert_capture_output_restores_errexit_on_caller() {
  local probe_output
  set +e
  probe_output="$(bash -c '
    set -e
    source "$1"
    captured="" status=""
    capture_output captured status false
    echo AFTER
    false
    echo NOT_REACHED
  ' _ "$REPO_ROOT/test/test-system/helpers/capture-output.sh" 2>&1)"
  set -e
  [[ $probe_output == *AFTER* ]] ||
    record_failure "capturing a failing command must not abort a set -e caller (output: $probe_output)"
  [[ $probe_output != *NOT_REACHED* ]] ||
    record_failure "capture_output must restore errexit for a set -e caller (output: $probe_output)"
}

main() {
  assert_capture_output_rejects_its_own_loop_variable_name
  assert_capture_output_rejects_any_prefixed_name
  assert_capture_output_normal_names_still_work
  assert_capture_output_preserves_errexit_off_caller
  assert_capture_output_restores_errexit_on_caller
  assert_dispatch_capture_rejects_prefixed_output_name
  assert_dispatch_capture_rejects_prefixed_status_name
  assert_dispatch_capture_rejects_identical_names
  assert_dispatch_capture_normal_names_still_work

  report_failures nameref-guards
}

main "$@"
