#!/usr/bin/env bash

set -euo pipefail

readonly installer_url="https://raw.githubusercontent.com/backnotprop/plannotator/main/scripts/install.sh"
readonly binary_only_flag="--minimal"

fetch_installer() {
  curl -fsSL "$installer_url"
}

run_installer() {
  local installer=$1
  bash -c "$installer" -- "$binary_only_flag" </dev/null >/dev/null
}

main() {
  local installer
  if ! installer="$(fetch_installer)"; then
    printf 'error[download-failed]: could not download the installer from %s.\n' "$installer_url" >&2
    exit 1
  fi

  if ! run_installer "$installer"; then
    printf 'error[install-failed]: the installer failed; plannotator was not reinstalled.\n' >&2
    exit 1
  fi

  printf 'plannotator: reinstalled the latest release\n'
}

main "$@"
