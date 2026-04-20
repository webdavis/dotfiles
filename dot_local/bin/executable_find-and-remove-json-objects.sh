#!/usr/bin/env bash

# Exit immediately if any command fails; treat unset vars as errors.
set -euo pipefail

# Check if a command is installed.
function verify_command_installed() {
  type "$1" >/dev/null 2>&1 || {
    printf "%s command is required but it's not installed.\n" "$1" >&2
    exit 1
  }
}

verify_command_installed jq
verify_command_installed rg
verify_command_installed sponge # moreutils; in brew manifest

JSON_OBJECT="${1:-}"
REGEX="${2:-}"

[[ -z $JSON_OBJECT ]] && {
  printf "Error: first argument (JSON object selector) is empty.\n" >&2
  exit 1
}
[[ -z $REGEX ]] && {
  printf "Error: second argument (regex) is empty.\n" >&2
  exit 1
}

timestamp="$(date +%Y%m%d%H%M%S)"
backup_dir="backup_$timestamp"
mkdir -p "$backup_dir"

# Backup and delete JSON objects from files containing regex.
rg --files-with-matches "$REGEX" | while IFS= read -r file; do
  cp "$file" "${backup_dir}/"
  jq "del(.[] | select(${JSON_OBJECT}? == '$REGEX'))" "$file" | sponge "$file"
done
