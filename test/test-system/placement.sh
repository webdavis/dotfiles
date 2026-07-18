#!/usr/bin/env bash
# placement.sh -- regression suite for test/validate-tests.sh (the placement /
# mode / symlink guard). Proves each guard rule catches its evasion:
#   - checked discovery: a find or sort that fails must fail the guard.
#   - symlink rejection: a symlinked test file and a symlinked suite dir both
#     fail the guard (a physical `find -type f` would skip them).
#   - flat placement: a *.sh or *.bats must live directly in a suite (nested or
#     stray fails; a non-executable suite *.sh fails).
#   - the test-system suite, the root allowlist, and the helpers/ exemption.
# Collect-then-report: every failed assertion is recorded and the run reports
# them all at the end.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers/find-repo-root.sh
source "$here/helpers/find-repo-root.sh"
# shellcheck source=helpers/capture-output.sh
source "$here/helpers/capture-output.sh"
# shellcheck source=helpers/write-probe-scripts.sh
source "$here/helpers/write-probe-scripts.sh"
# shellcheck source=helpers/create-test-trees.sh
source "$here/helpers/create-test-trees.sh"
# shellcheck source=helpers/report-test-failures.sh
source "$here/helpers/report-test-failures.sh"

REPO_ROOT="$(find_repo_root)" || exit 1
GUARD="$REPO_ROOT/test/validate-tests.sh"

# run_guard <output-variable-name> <status-variable-name> <root> [env NAME=VAL ...]
# Run the guard against <root>, writing its output and exit code into the two
# caller-named variables (forwarded to capture_output's nameref parameters).
run_guard() {
  local output_variable_name="$1" status_variable_name="$2" root="$3"
  shift 3
  capture_output "$output_variable_name" "$status_variable_name" env "$@" "$GUARD" "$root"
}

# Set in main; global so the EXIT trap can still see it after main returns.
work=""

assert_clean_tree_passes() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" clean unit integration e2e fixtures/lib)"
  write_probe_script "$root/unit/a.sh"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/integration/suite.bats"
  printf 'not executable, not run directly\n' >"$root/fixtures/lib/helper.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -eq 0 ]] || record_failure "clean tree should pass the guard (rc=$guard_status): $guard_output"
}

assert_symlinked_test_file_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" symfile unit)"
  write_probe_script "$root/unit/real.sh"
  ln -s real.sh "$root/unit/link.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a symlinked test file must fail the guard: $guard_output"
  [[ $guard_output == *"symlink"* ]] || record_failure "expected a symlink rejection message: $guard_output"
}

assert_symlinked_suite_directory_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" symdir unit)"
  mkdir -p "$work/symdir/elsewhere"
  write_probe_script "$work/symdir/elsewhere/x.sh"
  ln -s ../elsewhere "$root/e2e"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a symlinked suite dir must fail the guard: $guard_output"
  [[ $guard_output == *"symlink"* ]] || record_failure "expected a symlink rejection message for the suite dir: $guard_output"
}

assert_nested_bats_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" nestbats integration/sub)"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/integration/sub/suite.bats"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a nested *.bats must fail the guard: $guard_output"
  [[ $guard_output == *"nested"* ]] || record_failure "expected a nested-placement message for the bats file: $guard_output"
}

assert_stray_root_bats_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" straybats unit)"
  write_probe_script "$root/unit/a.sh"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/stray.bats"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a stray *.bats under test/ must fail the guard: $guard_output"
  [[ $guard_output == *"outside"* ]] || record_failure "expected an outside-the-suites message for the stray bats: $guard_output"
}

assert_nested_sh_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" nestsh unit/sub)"
  write_probe_script "$root/unit/sub/a.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a nested *.sh must fail the guard: $guard_output"
  [[ $guard_output == *"nested"* ]] || record_failure "expected a nested-placement message for the sh file: $guard_output"
}

assert_non_executable_suite_sh_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" nonexec unit)"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/unit/a.sh" # no chmod +x
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a non-executable suite *.sh must fail the guard: $guard_output"
  [[ $guard_output == *"not executable"* ]] || record_failure "expected a not-executable message: $guard_output"
}

# The shim passes the -type l symlink scan through, then fails the -name file
# discovery, so this exercises the files-discovery check specifically.
assert_failing_find_fails_guard() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" pfind unit)"
  mkdir -p "$work/pfind/bin"
  write_probe_script "$root/unit/a.sh"
  {
    printf '#!/usr/bin/env bash\n'
    printf '/usr/bin/find "$@"\n'
    printf 'case " $* " in *" -name "*) exit 7 ;; esac\n'
    printf 'exit 0\n'
  } >"$work/pfind/bin/find"
  chmod +x "$work/pfind/bin/find"
  run_guard guard_output guard_status "$root" "PATH=$work/pfind/bin:$PATH"
  [[ $guard_status -ne 0 ]] || record_failure "partial find (exit 7) did not fail the guard: $guard_output"
  [[ $guard_output == *"discovery failed"* ]] || record_failure "expected a discovery-failed message on find failure: $guard_output"
}

# sort is used only in the file discovery pipeline, not the symlink scan.
assert_failing_sort_fails_guard() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" psort unit)"
  mkdir -p "$work/psort/bin"
  write_probe_script "$root/unit/a.sh"
  {
    printf '#!/usr/bin/env bash\n'
    printf '/usr/bin/sort "$@"\nexit 7\n'
  } >"$work/psort/bin/sort"
  chmod +x "$work/psort/bin/sort"
  run_guard guard_output guard_status "$root" "PATH=$work/psort/bin:$PATH"
  [[ $guard_status -ne 0 ]] || record_failure "partial sort (exit 7) did not fail the guard: $guard_output"
  [[ $guard_output == *"discovery failed"* ]] || record_failure "expected a discovery-failed message on sort failure: $guard_output"
}

assert_test_system_suite_recognized() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" testsystem test-system)"
  write_probe_script "$root/test-system/a.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -eq 0 ]] || record_failure "a flat executable *.sh in test-system should pass (rc=$guard_status): $guard_output"
}

assert_allowlisted_root_scripts_pass() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" rootallow unit)"
  write_probe_script "$root/unit/a.sh"
  write_probe_script "$root/validate-tests.sh"
  write_probe_script "$root/run-test-suite.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -eq 0 ]] || record_failure "allowlisted root control scripts should pass (rc=$guard_status): $guard_output"
}

assert_removed_root_script_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" rootgone unit)"
  write_probe_script "$root/unit/a.sh"
  write_probe_script "$root/run-unit-tests.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "run-unit-tests.sh at test/ root must fail now that it is merged away: $guard_output"
  [[ $guard_output == *"run-unit-tests.sh"* ]] || record_failure "expected run-unit-tests.sh named in the message: $guard_output"
}

assert_stray_root_sh_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" rootstray unit)"
  write_probe_script "$root/unit/a.sh"
  write_probe_script "$root/stray.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a stray script at test/ root must fail: $guard_output"
  [[ $guard_output == *"stray.sh"* ]] || record_failure "expected the stray root script named in the message: $guard_output"
}

assert_non_executable_helper_passes() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" helpers test-system/helpers)"
  write_probe_script "$root/test-system/a.sh"
  printf '# shellcheck shell=bash\ntrue\n' >"$root/test-system/helpers/lib.sh" # sourced, not executable
  run_guard guard_output guard_status "$root"
  [[ $guard_status -eq 0 ]] || record_failure "a sourced helpers/ file in a suite should pass (rc=$guard_status): $guard_output"
}

assert_executable_helper_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" helpersexec unit/helpers)"
  write_probe_script "$root/unit/helpers/forgotten-test.sh" 'exit 23'
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "an executable helpers/ *.sh must fail the guard: $guard_output"
  [[ $guard_output == *"forgotten-test.sh"* ]] || record_failure "expected the executable helper named in the message: $guard_output"
  [[ $guard_output == *"sourced, not executed"* ]] || record_failure "expected the helpers-are-sourced explanation: $guard_output"
}

assert_helper_bats_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" helpersbats unit/helpers)"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/unit/helpers/suite.bats"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a *.bats inside helpers/ must fail the guard: $guard_output"
  [[ $guard_output == *"suite.bats"* ]] || record_failure "expected the helper bats named in the message: $guard_output"
}

assert_fixtures_bats_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" fixturesbats unit fixtures)"
  write_probe_script "$root/unit/a.sh"
  printf '#!/usr/bin/env bats\n@test "hidden" { false; }\n' >"$root/fixtures/hidden.bats"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a *.bats under fixtures/ must fail the guard (it would never run anywhere): $guard_output"
  [[ $guard_output == *"hidden.bats"* ]] || record_failure "expected the fixtures bats named in the message: $guard_output"
}

assert_nested_test_system_sh_fails() {
  local guard_output guard_status root
  root="$(make_test_tree "$work" nesttestsystem test-system/sub)"
  write_probe_script "$root/test-system/sub/a.sh"
  run_guard guard_output guard_status "$root"
  [[ $guard_status -ne 0 ]] || record_failure "a nested *.sh in test-system must fail: $guard_output"
  [[ $guard_output == *"nested"* ]] || record_failure "expected a nested-placement message: $guard_output"
}

main() {
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  assert_clean_tree_passes
  assert_symlinked_test_file_fails
  assert_symlinked_suite_directory_fails
  assert_nested_bats_fails
  assert_stray_root_bats_fails
  assert_nested_sh_fails
  assert_non_executable_suite_sh_fails
  assert_failing_find_fails_guard
  assert_failing_sort_fails_guard
  assert_test_system_suite_recognized
  assert_allowlisted_root_scripts_pass
  assert_removed_root_script_fails
  assert_stray_root_sh_fails
  assert_non_executable_helper_passes
  assert_executable_helper_fails
  assert_helper_bats_fails
  assert_fixtures_bats_fails
  assert_nested_test_system_sh_fails

  report_failures placement
}

main "$@"
