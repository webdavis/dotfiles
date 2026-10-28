#!/usr/bin/env bash

set -euo pipefail

readonly pns_state_needs_you=blocked
readonly pns_state_failed=failed
readonly exit_failure=1

alert_is_version_one() {
  local alert=$1
  jq -e '.version == 1' <<<"$alert" >/dev/null
}

alert_kind() {
  local alert=$1
  jq -er '.kind' <<<"$alert"
}

alert_host() {
  local alert=$1
  jq -er '.host' <<<"$alert"
}

alert_detail() {
  local alert=$1
  jq -er '"\(.lane // "uu"): \(.message)"' <<<"$alert"
}

pns_state_for() {
  local kind=$1
  case $kind in
    pending) printf '%s' "$pns_state_needs_you" ;;
    *) printf '%s' "$pns_state_failed" ;;
  esac
}

send_to_pns() {
  local state=$1 host=$2 detail=$3
  pns send --producer uu --state "$state" --delivery-class health --project "$host" --detail "$detail"
}

main() {
  local alert
  alert="$(cat)"

  local kind host detail
  if ! alert_is_version_one "$alert" || ! kind="$(alert_kind "$alert")" || ! host="$(alert_host "$alert")" || ! detail="$(alert_detail "$alert")"; then
    printf 'error[bad-alert]: stdin is not a version 1 uu alert.\n' >&2
    exit "$exit_failure"
  fi

  if ! send_to_pns "$(pns_state_for "$kind")" "$host" "$detail"; then
    printf 'error[pns-refused]: pns did not accept the alert.\n' >&2
    exit "$exit_failure"
  fi
}

main "$@"
