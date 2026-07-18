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
# variable. Two destination names are RESERVED and rejected up front,
# `capture_output_text_destination` and `capture_output_status_destination`
# (passing one would make the nameref circular), and the two destinations must
# be different names (the same name would make the status write clobber the
# output).
capture_output() {
  local capture_output_reserved_name
  for capture_output_reserved_name in capture_output_text_destination capture_output_status_destination; do
    if [[ $1 == "$capture_output_reserved_name" || $2 == "$capture_output_reserved_name" ]]; then
      printf 'capture_output: destination name %s is reserved by this helper; pick another variable name\n' "$capture_output_reserved_name" >&2
      return 2
    fi
  done
  if [[ $1 == "$2" ]]; then
    printf 'capture_output: the output and status destinations must be different variable names (both were %s)\n' "$1" >&2
    return 2
  fi
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
