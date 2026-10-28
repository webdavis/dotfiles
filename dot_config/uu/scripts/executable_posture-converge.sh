#!/usr/bin/env bash

set -euo pipefail

readonly osqueryd_binary="/opt/osquery/lib/osquery.app/Contents/MacOS/osqueryd"
readonly exit_failure=1
readonly exit_pending=100

print_error() {
  local kind=$1
  local message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

oldest_daemon_start() {
  local pids pid started oldest=""
  pids="$(pgrep -x osqueryd)" || return 1
  for pid in $pids; do
    started="$(LC_ALL=C ps -o lstart= -p "$pid")" || continue
    started="$(LC_ALL=C date -j -f '%a %b %e %T %Y' "$started" +%s)" || continue
    if [[ -z $oldest ]] || ((started < oldest)); then
      oldest=$started
    fi
  done
  [[ -n $oldest ]] && printf '%s\n' "$oldest"
}

binary_changed_at() {
  stat -f %m "$osqueryd_binary" 2>/dev/null
}

daemon_runs_the_installed_build() {
  local started=$1
  local changed=$2
  ((started > changed))
}

sudo_is_available() {
  sudo -v
}

main() {
  local started changed status=0

  if ! started="$(oldest_daemon_start)"; then
    print_error missing-tool "no osqueryd is running"
    exit "$exit_failure"
  fi
  if ! changed="$(binary_changed_at)"; then
    print_error missing-tool "$osqueryd_binary is not installed"
    exit "$exit_failure"
  fi

  if daemon_runs_the_installed_build "$started" "$changed"; then
    exit 0
  fi

  if ! sudo_is_available; then
    print_error needs-sudo "osqueryd runs a build older than the installed one"
    exit "$exit_pending"
  fi

  posture converge >&2 || status=$?
  if ((status != 0)); then
    print_error converge-failed "posture converge exited $status, so osqueryd may still run the old build"
    exit "$exit_failure"
  fi
  printf 'osqueryd: restarted on the new build\n'
}

main "$@"
