#!/usr/bin/env bash
# runner-discovery.sh -- regression suite for test/run-test-suite.sh (the shared
# suite runner). Proves the correctness rules the gate leans on cannot silently
# regress:
#   - checked discovery: a find or sort that fails mid-discovery must fail the
#     suite, never green-gate a truncated list.
#   - fd 3 closed for children: a test that drains fd 3 must not swallow the
#     tests queued behind it.
#   - per-suite bats: the suite's own *.bats run and their failure propagates
#     (not only in the aggregate `test`).
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
# shellcheck source=helpers/report-test-failures.sh
source "$here/helpers/report-test-failures.sh"

REPO_ROOT="$(find_repo_root)" || exit 1
RUN_SUITE="$REPO_ROOT/test/run-test-suite.sh"

# run_suite <output-variable-name> <status-variable-name> <suite> [env NAME=VAL ...]
# Run the runner on <suite>, writing its output and exit code into the two
# caller-named variables (forwarded to capture_output's nameref parameters).
run_suite() {
  local output_variable_name="$1" status_variable_name="$2" suite="$3"
  shift 3
  capture_output "$output_variable_name" "$status_variable_name" env "$@" "$RUN_SUITE" "$suite"
}

# Set in main; global so the EXIT trap can still see it after main returns.
work=""

main() {
  local runner_output="" runner_status=0
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  # ---- an fd-3-draining test must not swallow the failing test behind it --
  local suite="$work/fd3"
  mkdir -p "$suite"
  # a-drain sorts first; it slurps fd 3 (if the child inherits it) then exits 0.
  write_probe_script "$suite/a-drain.sh" 'cat <&3 >/dev/null 2>&1 || true' 'exit 0'
  # z-fail sorts last; it must still run and fail the suite.
  write_probe_script "$suite/z-fail.sh" 'exit 1'
  run_suite runner_output runner_status "$suite"
  [[ $runner_status -ne 0 ]] || record_failure "fd-3 drain swallowed the failing test; the suite green-gated: $runner_output"
  [[ $runner_output == *"z-fail.sh"* ]] || record_failure "z-fail never ran (header absent), so fd 3 was not closed for children: $runner_output"

  # ---- a find that fails the *.sh discovery must fail the suite ------------
  # The shim fails ONLY the `-name '*.sh'` discovery (not the later `*.bats` one),
  # so the test isolates the *.sh discovery: an unchecked (process-substitution)
  # *.sh discovery would swallow this failure and green-gate.
  suite="$work/pfind"
  mkdir -p "$suite/bin"
  write_probe_script "$suite/a.sh" 'exit 0'
  cat >"$suite/bin/find" <<'SHIM'
#!/usr/bin/env bash
/usr/bin/find "$@"
case " $* " in *".sh "*) exit 7 ;; esac
exit 0
SHIM
  chmod +x "$suite/bin/find"
  run_suite runner_output runner_status "$suite" "PATH=$suite/bin:$PATH"
  [[ $runner_status -ne 0 ]] || record_failure "partial *.sh find (exit 7) did not fail the suite: $runner_output"
  [[ $runner_output == *"discovery failed"* ]] || record_failure "expected a discovery-failed message on find failure: $runner_output"

  # ---- a sort that fails the *.sh discovery must fail the suite ------------
  # The shim fails ONLY the FIRST sort (the *.sh discovery pipeline; the *.bats
  # discovery sort is the second), isolating the *.sh discovery the same way. Its
  # counter lives beside the shim (${0%/*}), so the heredoc needs no interpolation.
  suite="$work/psort"
  mkdir -p "$suite/bin"
  write_probe_script "$suite/a.sh" 'exit 0'
  cat >"$suite/bin/sort" <<'SHIM'
#!/usr/bin/env bash
count="${0%/*}/count"
n=$(($(cat "$count" 2>/dev/null || echo 0) + 1))
printf '%s' "$n" >"$count"
/usr/bin/sort "$@"
[[ $n -eq 1 ]] && exit 7
exit 0
SHIM
  chmod +x "$suite/bin/sort"
  run_suite runner_output runner_status "$suite" "PATH=$suite/bin:$PATH"
  [[ $runner_status -ne 0 ]] || record_failure "partial *.sh sort (exit 7) did not fail the suite: $runner_output"
  [[ $runner_output == *"discovery failed"* ]] || record_failure "expected a discovery-failed message on sort failure: $runner_output"

  # ---- the suite's own *.bats run, and their failure propagates ----------
  # The stub records its argv to $BATS_ARGV (passed through the runner's env) and
  # exits nonzero, modeling a failing suite without needing real bats/parallel.
  suite="$work/bats"
  mkdir -p "$suite/failbin" "$suite/passbin"
  write_probe_script "$suite/a.sh" 'exit 0'
  printf '#!/usr/bin/env bats\n@test "x" { false; }\n' >"$suite/suite.bats"
  cat >"$suite/failbin/bats" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$BATS_ARGV"
exit 1
SHIM
  chmod +x "$suite/failbin/bats"
  run_suite runner_output runner_status "$suite" "PATH=$suite/failbin:$PATH" "BATS_ARGV=$suite/bats.argv"
  [[ $runner_status -ne 0 ]] || record_failure "a failing suite bats file did not fail the suite: $runner_output"
  grep -q 'suite.bats' "$suite/bats.argv" 2>/dev/null ||
    record_failure "the runner did not invoke bats on the suite's bats (argv: $(cat "$suite/bats.argv" 2>/dev/null || echo none))"

  # ---- a passing suite bats file leaves the suite green ---------
  cat >"$suite/passbin/bats" <<'SHIM'
#!/usr/bin/env bash
exit 0
SHIM
  chmod +x "$suite/passbin/bats"
  run_suite runner_output runner_status "$suite" "PATH=$suite/passbin:$PATH"
  [[ $runner_status -eq 0 ]] || record_failure "suite with passing .sh and passing bats should be green (rc=$runner_status): $runner_output"

  report_failures runner-discovery
}

main "$@"
