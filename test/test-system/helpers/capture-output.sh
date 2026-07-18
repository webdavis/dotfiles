# shellcheck shell=bash
# capture_output <output-variable-name> <status-variable-name> <command> [args...]
# Run the command, writing its combined stdout+stderr into the first named
# variable and its exit code into the second. The names are nameref output
# parameters, so the helper keeps no global state: callers declare locals and
# pass their names. Wraps the run in `set +e` / `set -e` so a nonzero exit
# never aborts the caller. Sourced by the test-system suite; no main.
#
# The internal nameref names are deliberately distinctive: a nameref that
# shares its caller's variable name would alias itself instead of the caller's
# variable.
capture_output() {
  local -n capture_output_text_destination="$1"
  local -n capture_output_status_destination="$2"
  shift 2
  set +e
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  capture_output_text_destination="$("$@" 2>&1)"
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  capture_output_status_destination=$?
  set -e
}
