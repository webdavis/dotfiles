#!/usr/bin/env bash
#
# dot_config/nvim/tests/run.lua is the gate `just test-nvim` runs, and a gate is
# only a gate if a failing spec makes the PROCESS fail. The runner's own
# self-checks cannot establish that: they run inside the same runner, so a
# runner broken to always exit 0 reports "FAIL run_spec" in its output and still
# hands the shell a zero status. Measured: a copy with `os.exit(0)` forced prints
# the failure and exits 0.
#
# So the exit code is asserted here instead, from outside the process, against a
# scratch spec of each polarity. run.lua globs `*_spec.lua` out of its OWN
# directory, so each scratch directory holds the runner plus exactly one spec.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/dot_config/nvim/tests/run.lua"

failures=0

fail() {
  printf 'nvim-spec-runner-exit-code: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

[[ -f $RUNNER ]] || {
  printf 'nvim-spec-runner-exit-code: FAIL -- missing runner: %s\n' "$RUNNER" >&2
  exit 1
}
command -v nvim >/dev/null 2>&1 || {
  printf 'nvim-spec-runner-exit-code: FAIL -- nvim is not on PATH\n' >&2
  exit 1
}

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT

# run_spec_dir <name> <spec-body> -- stage the runner beside one spec and run it.
# Echoes the runner's output; returns the runner's exit status.
run_spec_dir() {
  local name="$1" body="$2" dir
  dir="$sandbox/$name"
  mkdir -p "$dir"
  cp "$RUNNER" "$dir/run.lua"
  printf '%s\n' "$body" >"$dir/${name}_spec.lua"
  nvim --headless --clean -l "$dir/run.lua" 2>&1
}

# --- a failing case must fail the process ----------------------------------
if output="$(run_spec_dir failing 'return { ["deliberately fails"] = function() assert(false, "as designed") end }')"; then
  fail "a failing spec exited 0: $output"
fi
[[ $output == *"FAIL failing_spec: deliberately fails"* ]] ||
  fail "a failing spec did not report the case: $output"

# --- a passing case must not ----------------------------------------------
if output="$(run_spec_dir passing 'return { ["deliberately passes"] = function() assert(true) end }')"; then
  [[ $output == *"ok passing_spec: deliberately passes"* ]] ||
    fail "a passing spec did not report the case: $output"
else
  fail "a passing spec exited non-zero: $output"
fi

if [[ $failures -gt 0 ]]; then
  printf 'nvim-spec-runner-exit-code: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi

printf 'nvim-spec-runner-exit-code: OK (failing spec exits non-zero, passing spec exits zero)\n'
