# shellcheck shell=bash
# capture_output <command> [args...] -- run the command, keeping its combined
# stdout+stderr in captured_output and its exit code in captured_status. Wraps
# the run in `set +e` / `set -e` so a nonzero exit never aborts the caller.
# Sourced by the test-system suite; no main.

# The globals capture_output sets, predeclared so a sourcer can reference them.
captured_output=""
captured_status=0

capture_output() {
  set +e
  captured_output="$("$@" 2>&1)"
  captured_status=$?
  set -e
}
