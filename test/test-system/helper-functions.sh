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

# Filled by capture_output (from the sourced helper); predeclared so the file
# reads cleanly on its own.
captured_status=0
captured_output=""

# Set in main; global so the EXIT trap can still see it after main returns.
work=""

main() {
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  # ---- make_test_tree fails loudly when mkdir cannot create the tree ---------
  # A command substitution swallows errexit, so without an explicit check the
  # function used to print a path that does not exist and return 0; a caller
  # would then build fixtures into nowhere.
  capture_output make_test_tree /dev/null probe unit
  [[ $captured_status -ne 0 ]] ||
    record_failure "make_test_tree under an unwritable parent must return nonzero (got 0, printed: $captured_output)"

  # ---- make_test_tree fails when a subdir cannot be created ------------------
  # The root is creatable but the subdir path is blocked by a regular file.
  mkdir -p "$work/blocked/name/test"
  : >"$work/blocked/name/test/unit"
  capture_output make_test_tree "$work/blocked" name unit/sub
  [[ $captured_status -ne 0 ]] ||
    record_failure "make_test_tree with an uncreatable subdir must return nonzero (got 0)"

  # ---- the success path still prints the created test/ dir -------------------
  capture_output make_test_tree "$work" ok unit fixtures/lib
  [[ $captured_status -eq 0 ]] ||
    record_failure "make_test_tree on a writable parent should succeed (rc=$captured_status): $captured_output"
  [[ $captured_output == "$work/ok/test" ]] ||
    record_failure "make_test_tree should print the test/ dir path (got: $captured_output)"
  [[ -d "$work/ok/test/unit" && -d "$work/ok/test/fixtures/lib" ]] ||
    record_failure "make_test_tree did not create the requested subdirs"

  report_failures helper-functions
}

main "$@"
