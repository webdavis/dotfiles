# shellcheck shell=bash
# capture_output <output-variable-name> <status-variable-name> <command> [args...]
# Run the command, writing its combined stdout+stderr into the first named
# variable and its exit code into the second. The names are nameref output
# parameters, so the helper keeps no global state: callers declare locals and
# pass their names. Runs the command under `set +e` and then restores the
# caller's ORIGINAL errexit state, so a nonzero exit never aborts the caller
# and a deliberate set +e caller stays set +e afterward. Sourced by the
# test-system suite; no main.
#
# Naming system: every internal local is prefixed with the full function name
# (capture_output_*), and the guard below rejects any destination name matching
# that reserved internal prefix. A destination sharing an internal's name would
# make the nameref alias the helper's own local instead of the caller's
# variable, and the write would be silently lost. Guarding the whole prefix
# covers every current and future internal, unlike a hand-kept name list. The
# two destinations must also be different names (the same name would make the
# status write clobber the output).
capture_output() {
  local capture_output_destination_name
  for capture_output_destination_name in "$1" "$2"; do
    if [[ $capture_output_destination_name == capture_output_* ]]; then
      printf 'capture_output: destination name %s matches the reserved internal prefix capture_output_; pick another variable name\n' "$capture_output_destination_name" >&2
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
  local capture_output_errexit_was_on=0
  [[ $- == *e* ]] && capture_output_errexit_was_on=1
  set +e
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  capture_output_text_destination="$("$@" 2>&1)"
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  capture_output_status_destination=$?
  [[ $capture_output_errexit_was_on -eq 1 ]] && set -e
  return 0
}
