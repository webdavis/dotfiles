# shellcheck shell=bash
# run-dispatch-and-drain.sh -- drive the dispatch library's entry points from a
# test and capture each run's combined output and exit code WITHOUT aborting the
# caller on a nonzero exit. Sourced by the dispatch suites; no main.
#
# The two destinations are nameref output parameters, so the helper keeps no
# global state: a caller declares two locals and passes their names. Naming
# system (shared with test-system/helpers/capture-output.sh): every internal
# local is prefixed with the full function name (_capture_dispatch_run_*), and
# the guard rejects any destination name matching that reserved internal prefix.
# A destination sharing an internal's name would make the nameref alias the
# helper's own local instead of the caller's variable, and the write would be
# silently lost. Guarding the whole prefix covers every current and future
# internal, unlike a hand-kept name list. The two destinations must be different
# names (the same name would let the status write clobber the captured output).
# The caller's errexit setting is saved and restored, so this works whether or
# not the caller runs under `set -e`.

# _capture_dispatch_run <output-variable-name> <status-variable-name> <command> [args...]
_capture_dispatch_run() {
  local _capture_dispatch_run_destination_name
  for _capture_dispatch_run_destination_name in "$1" "$2"; do
    if [[ $_capture_dispatch_run_destination_name == _capture_dispatch_run_* ]]; then
      printf 'run-dispatch-and-drain: destination name %s matches the reserved internal prefix _capture_dispatch_run_; pick another variable name\n' "$_capture_dispatch_run_destination_name" >&2
      return 2
    fi
  done
  if [[ $1 == "$2" ]]; then
    printf 'run-dispatch-and-drain: the output and status destinations must be different variable names (both were %s)\n' "$1" >&2
    return 2
  fi
  local -n _capture_dispatch_run_output_destination="$1"
  local -n _capture_dispatch_run_status_destination="$2"
  shift 2
  local _capture_dispatch_run_errexit_was_on=0
  [[ $- == *e* ]] && _capture_dispatch_run_errexit_was_on=1
  set +e
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  _capture_dispatch_run_output_destination="$("$@" 2>&1)"
  # shellcheck disable=SC2034 # nameref: the assignment writes the caller's variable
  _capture_dispatch_run_status_destination=$?
  [[ $_capture_dispatch_run_errexit_was_on -eq 1 ]] && set -e
  return 0
}

# run_dispatch <output-variable-name> <status-variable-name> <send_alert-args...>
# Run the library's send_alert with the given arguments, capturing its combined
# output and exit code into the caller-named variables. Deliberately declares no
# locals of its own: an intermediate local holding a destination name would be
# one more name a caller could collide with.
run_dispatch() {
  _capture_dispatch_run "$1" "$2" send_alert "${@:3}"
}

# run_retry_undelivered_alerts <output-variable-name> <status-variable-name>
# Run the library's retry drain, capturing its combined output and exit code.
run_retry_undelivered_alerts() {
  _capture_dispatch_run "$1" "$2" retry_undelivered_alerts
}
