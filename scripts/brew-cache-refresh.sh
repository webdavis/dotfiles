#!/usr/bin/env bash

set -euo pipefail

DEPLOYED_WRITER="${HOME}/.local/libexec/brew-shellenv-cache-refresh.sh"

usage() {
  cat >&2 <<'USAGE'
usage: brew-cache-refresh.sh
USAGE
  exit 1
}

fail() {
  local message="$1"
  echo "error: ${message}" >&2
  exit 1
}

require_deployed_writer() {
  [[ -x $DEPLOYED_WRITER ]] || fail "${DEPLOYED_WRITER} is not deployed. Run just apply, then retry."
}

refresh_cache() {
  "$DEPLOYED_WRITER"
}

main() {
  [[ $# -eq 0 ]] || usage
  require_deployed_writer
  refresh_cache
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
