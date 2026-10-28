#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINDINGS_TABLE="${REPO_ROOT}/dot_config/chord/bindings.toml"
CHORD_INSTALL_COMMAND="cargo install --git https://github.com/webdavis/chord chord"
OUTPUTS=(bash menu)

usage() {
  cat >&2 <<'USAGE'
usage: chord.sh check
       chord.sh render
USAGE
  exit 1
}

fail() {
  local message="$1"
  echo "error: ${message}" >&2
  exit 1
}

require_chord() {
  command -v chord >/dev/null || fail "chord is not installed: ${CHORD_INSTALL_COMMAND}"
}

run_for_each_output() {
  local verb="$1"
  local output
  for output in "${OUTPUTS[@]}"; do
    chord "$verb" "$output" --table "$BINDINGS_TABLE"
  done
}

check_command() {
  run_for_each_output check
}

render_command() {
  run_for_each_output render
}

main() {
  [[ $# -eq 1 ]] || usage
  require_chord

  case "$1" in
    check) check_command ;;
    render) render_command ;;
    *) usage ;;
  esac
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
