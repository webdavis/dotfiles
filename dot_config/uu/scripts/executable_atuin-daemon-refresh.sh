#!/usr/bin/env bash

set -euo pipefail

readonly exit_failure=1
readonly daemon_record="$HOME/.local/share/atuin/atuin-daemon.pid"
readonly daemon_label="com.webdavis.atuin-daemon"

print_error() {
  local kind=$1
  local message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

atuin_is_installed() {
  command -v atuin >/dev/null 2>&1
}

daemon_record_exists() {
  [[ -f $daemon_record ]]
}

daemon_process_id() {
  sed -n '1p' "$daemon_record"
}

daemon_version() {
  sed -n '2p' "$daemon_record"
}

daemon_is_running() {
  local process_id=$1
  [[ -n $process_id ]] && kill -0 "$process_id" 2>/dev/null
}

installed_version() {
  local shown
  shown="$(atuin --version)" && awk '{print $2}' <<<"$shown"
}

version_is_known() {
  local version=$1
  [[ -n $version ]]
}

restart_daemon() {
  launchctl kickstart -k "gui/$(id -u)/$daemon_label"
}

main() {
  local old new

  if ! atuin_is_installed; then
    print_error missing-tool "atuin is not on PATH"
    exit "$exit_failure"
  fi
  if ! daemon_record_exists || ! daemon_is_running "$(daemon_process_id)"; then
    exit 0
  fi

  old="$(daemon_version)"
  if ! new="$(installed_version)" || ! version_is_known "$new"; then
    print_error version-unknown "atuin --version printed no version"
    exit "$exit_failure"
  fi
  if [[ $old == "$new" ]]; then
    exit 0
  fi

  if ! restart_daemon; then
    print_error restart-failed "launchctl kickstart of $daemon_label failed"
    exit "$exit_failure"
  fi
  printf 'atuin: daemon restarted %s to %s\n' "$old" "$new"
}

main "$@"
