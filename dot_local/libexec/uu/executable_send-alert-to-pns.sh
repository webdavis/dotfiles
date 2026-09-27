#!/usr/bin/env bash

set -euo pipefail

readonly pns_state=failed
readonly exit_failure=1

alert_is_version_one() {
  local alert=$1
  jq -e '.version == 1' <<<"$alert" >/dev/null
}

alert_host() {
  local alert=$1
  jq -er '.host' <<<"$alert"
}

alert_detail() {
  local alert=$1
  jq -er '"\(.lane // "uu"): \(.message)"' <<<"$alert"
}

send_to_pns() {
  local host=$1 detail=$2
  pns send --producer uu --state "$pns_state" --delivery-class health --project "$host" --detail "$detail"
}

main() {
  local alert
  alert="$(cat)"

  local host detail
  if ! alert_is_version_one "$alert" || ! host="$(alert_host "$alert")" || ! detail="$(alert_detail "$alert")"; then
    printf 'error[bad-alert]: stdin is not a version 1 uu alert.\n' >&2
    exit "$exit_failure"
  fi

  if ! send_to_pns "$host" "$detail"; then
    printf 'error[pns-refused]: pns did not accept the alert.\n' >&2
    exit "$exit_failure"
  fi
}

main "$@"
