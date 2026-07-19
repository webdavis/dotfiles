# shellcheck shell=bash
# run-dispatch-and-drain.sh -- drive the dispatch library's entry points from a
# test and capture each run's combined output and exit code WITHOUT aborting the
# caller on a nonzero exit. Sourced by the dispatch suites; no main.
#
# The two destinations are nameref output parameters, so the helper keeps no
# global state: a caller declares two locals and passes their names. Following
# test-system/helpers/capture-output.sh, two internal destination names are
# RESERVED and rejected up front (passing one would make the nameref alias
# itself), and the two destinations must be different names (the same name would
# let the status write clobber the captured output). The caller's errexit setting
# is saved and restored, so this works whether or not the caller runs under
# `set -e`.

# _capture_dispatch_run <output-variable-name> <status-variable-name> <command> [args...]
_capture_dispatch_run() {
  local _capture_dispatch_reserved
  for _capture_dispatch_reserved in _capture_dispatch_output_destination _capture_dispatch_status_destination; do
    if [[ $1 == "$_capture_dispatch_reserved" || $2 == "$_capture_dispatch_reserved" ]]; then
      printf 'run-dispatch-and-drain: destination name %s is reserved by this helper; pick another variable name\n' "$_capture_dispatch_reserved" >&2
      return 2
    fi
  done
  if [[ $1 == "$2" ]]; then
    printf 'run-dispatch-and-drain: the output and status destinations must be different variable names (both were %s)\n' "$1" >&2
    return 2
  fi
  local -n _capture_dispatch_output_destination="$1"
  local -n _capture_dispatch_status_destination="$2"
  shift 2
  local _capture_dispatch_errexit_was_on=0
  [[ $- == *e* ]] && _capture_dispatch_errexit_was_on=1
  set +e
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  _capture_dispatch_output_destination="$("$@" 2>&1)"
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  _capture_dispatch_status_destination=$?
  [[ $_capture_dispatch_errexit_was_on -eq 1 ]] && set -e
  return 0
}

# run_dispatch <output-variable-name> <status-variable-name> <send_alert-args...>
# Run the library's send_alert with the given arguments, capturing its combined
# output and exit code into the caller-named variables.
run_dispatch() {
  local output_variable_name="$1" status_variable_name="$2"
  shift 2
  _capture_dispatch_run "$output_variable_name" "$status_variable_name" send_alert "$@"
}

# run_retry_undelivered_alerts <output-variable-name> <status-variable-name>
# Run the library's retry drain, capturing its combined output and exit code.
run_retry_undelivered_alerts() {
  _capture_dispatch_run "$1" "$2" retry_undelivered_alerts
}
