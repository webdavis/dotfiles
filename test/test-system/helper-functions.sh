#!/usr/bin/env bash
# helper-functions.sh -- regression suite for the sourced helper libraries in
# test/test-system/helpers/. Pins behavior the other suite tests lean on, so a
# helper regression shows up here and not as a confusing failure elsewhere.
# Collect-then-report: every failed assertion is recorded and the run reports
# them all at the end.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers/capture-output.sh
source "$here/helpers/capture-output.sh"
# shellcheck source=helpers/create-test-trees.sh
source "$here/helpers/create-test-trees.sh"
# shellcheck source=helpers/report-test-failures.sh
source "$here/helpers/report-test-failures.sh"

# Set in main; global so the EXIT trap can still see it after main returns.
work=""

# A command substitution swallows errexit, so without an explicit check the
# function used to print a path that does not exist and return 0; a caller
# would then build fixtures into nowhere.
assert_make_test_tree_fails_on_unwritable_parent() {
  local helper_output helper_status
  capture_output helper_output helper_status make_test_tree /dev/null probe unit
  [[ $helper_status -ne 0 ]] ||
    record_failure "make_test_tree under an unwritable parent must return nonzero (got 0, printed: $helper_output)"
}

# The root is creatable but the subdir path is blocked by a regular file.
assert_make_test_tree_fails_on_blocked_subdir() {
  local helper_output helper_status
  mkdir -p "$work/blocked/name/test"
  : >"$work/blocked/name/test/unit"
  capture_output helper_output helper_status make_test_tree "$work/blocked" name unit/sub
  [[ $helper_status -ne 0 ]] ||
    record_failure "make_test_tree with an uncreatable subdir must return nonzero (got 0)"
}

assert_make_test_tree_success_prints_created_path() {
  local helper_output helper_status
  capture_output helper_output helper_status make_test_tree "$work" ok unit fixtures/lib
  [[ $helper_status -eq 0 ]] ||
    record_failure "make_test_tree on a writable parent should succeed (rc=$helper_status): $helper_output"
  [[ $helper_output == "$work/ok/test" ]] ||
    record_failure "make_test_tree should print the test/ dir path (got: $helper_output)"
  [[ -d "$work/ok/test/unit" && -d "$work/ok/test/fixtures/lib" ]] ||
    record_failure "make_test_tree did not create the requested subdirs"
}

# Passing one of the helper's own internal names would make the nameref
# circular; it must be a clean rejection, not warnings and a silent zero.
assert_capture_output_rejects_reserved_names() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(capture_output capture_output_text_destination collision_status true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "capture_output must reject the reserved output-destination name (got rc 0)"
  [[ $rejection_output == *reserved* ]] ||
    record_failure "the reserved-name rejection should say the name is reserved (got: $rejection_output)"
  set +e
  rejection_output="$(capture_output collision_output capture_output_status_destination true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "capture_output must reject the reserved status-destination name (got rc 0)"
}

# One variable for both destinations would let the status write clobber the
# output.
assert_capture_output_rejects_identical_names() {
  local rejection_output rejection_status
  set +e
  rejection_output="$(capture_output same_variable_name same_variable_name true 2>&1)"
  rejection_status=$?
  set -e
  [[ $rejection_status -ne 0 ]] ||
    record_failure "capture_output must reject identical output and status destination names (got rc 0)"
  [[ $rejection_output == *different* ]] ||
    record_failure "the identical-names rejection should say the names must differ (got: $rejection_output)"
}

main() {
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  assert_make_test_tree_fails_on_unwritable_parent
  assert_make_test_tree_fails_on_blocked_subdir
  assert_make_test_tree_success_prints_created_path
  assert_capture_output_rejects_reserved_names
  assert_capture_output_rejects_identical_names

  report_failures helper-functions
}

main "$@"
