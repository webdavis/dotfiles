#!/usr/bin/env bash

set -euo pipefail

readonly cargo_bin="$HOME/.cargo/bin"
readonly alarm_project="hue bridge"
readonly pns_state_done="done"
readonly pns_state_failed="failed"
readonly exit_failure=1

print_error() {
  local kind=$1
  local message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

pns_is_present() {
  command -v pns >/dev/null
}

message_is_version_one() {
  local message=$1
  jq -e '.version == 1' <<<"$message" &>/dev/null
}

message_field() {
  local message=$1
  local field=$2
  jq -er --arg field "$field" '.[$field] | strings' <<<"$message"
}

pns_state_for() {
  local kind=$1
  case $kind in
    announce) printf '%s' "$pns_state_done" ;;
    alarm) printf '%s' "$pns_state_failed" ;;
    *) return 1 ;;
  esac
}

pns_project_for() {
  local message=$1
  local kind=$2
  case $kind in
    announce) message_field "$message" room ;;
    alarm) printf '%s' "$alarm_project" ;;
    *) return 1 ;;
  esac
}

send_to_pns() {
  local state=$1
  local project=$2
  local detail=$3
  pns send --producer lights --state "$state" --project "$project" --detail "$detail" --scope local_only
}

main() {
  PATH="$PATH:$cargo_bin"

  local message
  message="$(cat)"

  local kind state project detail
  if ! message_is_version_one "$message" || ! kind="$(message_field "$message" kind)" || ! state="$(pns_state_for "$kind")" || ! project="$(pns_project_for "$message" "$kind")" || ! detail="$(message_field "$message" detail)"; then
    print_error bad-message "stdin is not a version 1 lights notification."
    exit "$exit_failure"
  fi

  if ! pns_is_present; then
    print_error missing-tool "pns is not on PATH."
    exit "$exit_failure"
  fi

  if ! send_to_pns "$state" "$project" "$detail"; then
    print_error pns-refused "pns did not accept the notification."
    exit "$exit_failure"
  fi
}

main "$@"
