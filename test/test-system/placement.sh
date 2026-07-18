#!/usr/bin/env bash
# placement.sh -- regression suite for test/validate-tests.sh (the
# placement / mode / symlink guard). Proves each guard rule catches its evasion:
#   F1  checked discovery: a find or sort that fails must FAIL the guard.
#   F4  symlink rejection: a symlinked test file AND a symlinked camp dir must
#       both fail the guard (a physical `find -type f` would skip them).
#   F5a bats placement: a *.bats must live DIRECTLY in a camp, same flat rule as
#       *.sh (nested or stray *.bats fails; flat *.bats passes).
#   plus the pre-existing placement/mode rules (nested *.sh, non-exec *.sh), the
#   test-system suite, the root allowlist, and the helpers/ exemption.
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

# Filled by capture_output (from the sourced helper); predeclared so the file
# reads cleanly on its own.
captured_status=0
captured_output=""

# run_guard <root> [env NAME=VAL ...] -- run the guard, storing its exit code in
# captured_status and its output in captured_output.
run_guard() {
  local root="$1"
  shift
  capture_output env "$@" "$GUARD" "$root"
}

# Set in main; global so the EXIT trap can still see it after main returns.
work=""

main() {
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  # ---- a clean tree passes (flat *.sh executable, flat *.bats, fixtures) ------
  local root
  root="$(make_test_tree "$work" clean unit integration e2e fixtures/lib)"
  write_probe_script "$root/unit/a.sh"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/integration/suite.bats"
  printf 'not executable, not run directly\n' >"$root/fixtures/lib/helper.sh"
  run_guard "$root"
  [[ $captured_status -eq 0 ]] || record_failure "clean tree should pass the guard (rc=$captured_status): $captured_output"

  # ---- F4: a symlinked test FILE fails ---------------------------------------
  root="$(make_test_tree "$work" symfile unit)"
  write_probe_script "$root/unit/real.sh"
  ln -s real.sh "$root/unit/link.sh"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a symlinked test file must fail the guard: $captured_output"
  [[ $captured_output == *"symlink"* ]] || record_failure "expected a symlink rejection message: $captured_output"

  # ---- F4: a symlinked camp DIR fails ----------------------------------------
  root="$(make_test_tree "$work" symdir unit)"
  mkdir -p "$work/symdir/elsewhere"
  write_probe_script "$work/symdir/elsewhere/x.sh"
  ln -s ../elsewhere "$root/e2e"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a symlinked camp dir must fail the guard: $captured_output"
  [[ $captured_output == *"symlink"* ]] || record_failure "expected a symlink rejection message for the camp dir: $captured_output"

  # ---- F5a: a nested *.bats fails (flat-placement rule extends to bats) -------
  root="$(make_test_tree "$work" nestbats integration/sub)"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/integration/sub/suite.bats"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a nested *.bats must fail the guard: $captured_output"
  [[ $captured_output == *"nested"* ]] || record_failure "expected a nested-placement message for the bats file: $captured_output"

  # ---- F5a: a stray *.bats directly under test/ fails ------------------------
  root="$(make_test_tree "$work" straybats unit)"
  write_probe_script "$root/unit/a.sh"
  printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/stray.bats"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a stray *.bats under test/ must fail the guard: $captured_output"
  [[ $captured_output == *"outside"* ]] || record_failure "expected an outside-the-camps message for the stray bats: $captured_output"

  # ---- placement: a nested *.sh fails ----------------------------------------
  root="$(make_test_tree "$work" nestsh unit/sub)"
  write_probe_script "$root/unit/sub/a.sh"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a nested *.sh must fail the guard: $captured_output"
  [[ $captured_output == *"nested"* ]] || record_failure "expected a nested-placement message for the sh file: $captured_output"

  # ---- mode: a non-executable camp *.sh fails --------------------------------
  root="$(make_test_tree "$work" nonexec unit)"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$root/unit/a.sh" # no chmod +x
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a non-executable camp *.sh must fail the guard: $captured_output"
  [[ $captured_output == *"not executable"* ]] || record_failure "expected a not-executable message: $captured_output"

  # ---- F1: a find that fails the FILE discovery must fail the guard -----------
  # The shim passes the -type l symlink scan through, then fails the -name file
  # discovery, so this exercises the files-discovery check specifically.
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
  run_guard "$root" "PATH=$work/pfind/bin:$PATH"
  [[ $captured_status -ne 0 ]] || record_failure "partial find (exit 7) did not fail the guard: $captured_output"
  [[ $captured_output == *"discovery failed"* ]] || record_failure "expected a discovery-failed message on find failure: $captured_output"

  # ---- F1: a sort that fails the discovery pipeline must fail the guard -------
  # sort is used only in the file discovery pipeline, not the symlink scan.
  root="$(make_test_tree "$work" psort unit)"
  mkdir -p "$work/psort/bin"
  write_probe_script "$root/unit/a.sh"
  {
    printf '#!/usr/bin/env bash\n'
    printf '/usr/bin/sort "$@"\nexit 7\n'
  } >"$work/psort/bin/sort"
  chmod +x "$work/psort/bin/sort"
  run_guard "$root" "PATH=$work/psort/bin:$PATH"
  [[ $captured_status -ne 0 ]] || record_failure "partial sort (exit 7) did not fail the guard: $captured_output"
  [[ $captured_output == *"discovery failed"* ]] || record_failure "expected a discovery-failed message on sort failure: $captured_output"

  # ---- test-system is a recognized suite: a flat executable *.sh passes ------
  root="$(make_test_tree "$work" testsystem test-system)"
  write_probe_script "$root/test-system/a.sh"
  run_guard "$root"
  [[ $captured_status -eq 0 ]] || record_failure "a flat executable *.sh in test-system should pass (rc=$captured_status): $captured_output"

  # ---- allowlisted control scripts at test/ root pass ------------------------
  root="$(make_test_tree "$work" rootallow unit)"
  write_probe_script "$root/unit/a.sh"
  write_probe_script "$root/validate-tests.sh"
  write_probe_script "$root/run-test-suite.sh"
  run_guard "$root"
  [[ $captured_status -eq 0 ]] || record_failure "allowlisted root control scripts should pass (rc=$captured_status): $captured_output"

  # ---- a former control script no longer allowlisted at test/ root fails ------
  root="$(make_test_tree "$work" rootgone unit)"
  write_probe_script "$root/unit/a.sh"
  write_probe_script "$root/run-unit-tests.sh"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "run-unit-tests.sh at test/ root must fail now that it is merged away: $captured_output"
  [[ $captured_output == *"run-unit-tests.sh"* ]] || record_failure "expected run-unit-tests.sh named in the message: $captured_output"

  # ---- a stray script at test/ root fails ------------------------------------
  root="$(make_test_tree "$work" rootstray unit)"
  write_probe_script "$root/unit/a.sh"
  write_probe_script "$root/stray.sh"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a stray script at test/ root must fail: $captured_output"
  [[ $captured_output == *"stray.sh"* ]] || record_failure "expected the stray root script named in the message: $captured_output"

  # ---- a suite helpers/ sourced file passes (sourced, non-executable) --------
  root="$(make_test_tree "$work" helpers test-system/helpers)"
  write_probe_script "$root/test-system/a.sh"
  printf '# shellcheck shell=bash\ntrue\n' >"$root/test-system/helpers/lib.sh" # sourced, not executable
  run_guard "$root"
  [[ $captured_status -eq 0 ]] || record_failure "a sourced helpers/ file in a suite should pass (rc=$captured_status): $captured_output"

  # ---- a nested test in test-system still fails ------------------------------
  root="$(make_test_tree "$work" nesttestsystem test-system/sub)"
  write_probe_script "$root/test-system/sub/a.sh"
  run_guard "$root"
  [[ $captured_status -ne 0 ]] || record_failure "a nested *.sh in test-system must fail: $captured_output"
  [[ $captured_output == *"nested"* ]] || record_failure "expected a nested-placement message: $captured_output"

  report_failures placement
}

main "$@"
