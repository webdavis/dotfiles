#!/usr/bin/env bash

set -euo pipefail

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "${CHEZMOI_SOURCE_DIR:?}/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

readonly posture_bin="${CHEZMOI_HOME_DIR:-$HOME}/.cargo/bin/posture"
readonly osquery_directory="/var/osquery"

running_on_macos() {
  [[ "$(uname)" == Darwin ]]
}

posture_is_deployed() {
  [[ -x $posture_bin ]]
}

converge_osquery_directory() {
  "$posture_bin" converge
}

converge_failed() {
  local converge_status=$1
  ((converge_status != 0))
}

main() {
  if ! running_on_macos; then
    return
  fi

  report_section 'osquery' "converge $osquery_directory"

  if ! posture_is_deployed; then
    report_line "error[not-deployed]: $posture_bin is not deployed, so $osquery_directory was NOT converged. Resolve the deferred posture build and re-run the apply." >&2
    return
  fi

  report_line "converging $osquery_directory..."
  local converge_status=0
  converge_osquery_directory || converge_status=$?
  if converge_failed "$converge_status"; then
    report_line "error[converge-failed]: posture converge exited with status $converge_status; see its output above." >&2
    exit "$converge_status"
  fi
  report_line "converged $osquery_directory."
}

main "$@"
