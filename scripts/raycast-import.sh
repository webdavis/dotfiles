#!/usr/bin/env bash

set -euo pipefail

EXPORT_FILE="${HOME}/.config/raycast/raycast.rayconfig"
IMPORT_URL="raycast://extensions/raycast/raycast/import-settings-data"

usage() {
  cat >&2 <<'USAGE'
usage: raycast-import.sh
USAGE
  exit 1
}

fail() {
  local message="$1"
  echo "error: ${message}" >&2
  exit 1
}

require_export_file() {
  [[ -f $EXPORT_FILE ]] || fail "no ${EXPORT_FILE} yet. Run just apply first."
}

print_manual_steps() {
  # Raycast has no silent import, so the file and passphrase are entered by hand.
  echo "Pick ${EXPORT_FILE} and enter the export passphrase."
}

open_import_dialog() {
  open "$IMPORT_URL"
}

main() {
  [[ $# -eq 0 ]] || usage
  require_export_file
  print_manual_steps
  open_import_dialog
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
