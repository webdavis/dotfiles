#!/usr/bin/env bash
# dispatch-harness-teardown.sh -- guard the shared dispatch harness's teardown
# ownership contract: teardown_dispatch_harness must remove ONLY a temp dir the
# harness itself created, never a pre-set or inherited HARNESS_HOME. Collect all
# failures and report them at the end.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=test/test-system/helpers/report-test-failures.sh
source "$REPO_ROOT/test/test-system/helpers/report-test-failures.sh"
# shellcheck source=test/helpers/build-dispatch-harness.sh
source "$REPO_ROOT/test/helpers/build-dispatch-harness.sh"

# An inherited HARNESS_HOME the harness never created must survive teardown.
assert_teardown_spares_inherited_home() {
  local inherited
  inherited="$(mktemp -d)"
  HARNESS_HOME="$inherited"
  unset _DISPATCH_HARNESS_OWNED_DIR 2>/dev/null || true
  teardown_dispatch_harness
  if [[ ! -d $inherited ]]; then
    record_failure "teardown deleted an inherited HARNESS_HOME it did not create: $inherited"
  fi
  rm -rf "$inherited"
}

# A dir the harness DID create is removed by teardown.
assert_teardown_removes_owned_dir() {
  # build_dispatch_harness resolves the dispatch library from BATS_TEST_DIRNAME.
  BATS_TEST_DIRNAME="$REPO_ROOT/test/e2e"
  build_dispatch_harness
  local owned="$HARNESS_HOME"
  teardown_dispatch_harness
  if [[ -d $owned ]]; then
    record_failure "teardown did not remove the dir the harness created: $owned"
  fi
}

main() {
  assert_teardown_spares_inherited_home
  assert_teardown_removes_owned_dir
  report_failures dispatch-harness-teardown
}

main "$@"
