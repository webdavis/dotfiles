#!/usr/bin/env bash

# Exit immediately if any command fails.
set -eo pipefail

# Check if the jq and rg commands are installed.
function verify_command_installed() {
  type "$1" >/dev/null 2>&1 || { echo >&2 "${1} command is required but it's not installed."; exit 1; }
}

verify_command_installed jq
verify_command_installed rg

JSON_OBJECT="$1"
REGEX="$2"

[[ -z "$JSON_OBJECT" ]] && { echo "Error: \$JSON_OBJECT is empty. This script requires one argument as a JSON object."; exit 1; }
[[ -z "$REGEX" ]] && { echo "Error: \$REGEX is empty. This script requires one argument as a regex."; exit 1; }

timestamp="$(date +%Y%m%d%H%M%S)"
backup_dir="backup_$timestamp"

# Backup and delete JSON objects from files containing regex.
rg --files-with-matches "$REGEX" | while IFS= read -r file; do
  cp "$file" "${backup_dir}/"
  jq "del(.[] | select(${JSON_OBJECT}? == '$REGEX'))" "$file" | sponge "$file"
done
