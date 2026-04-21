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
# Uses a tempfile+mv instead of `sponge` (moreutils) to avoid a conflict
# between moreutils' bundled `parallel` and GNU parallel; see commit log
# for the drop-moreutils rationale.
rg --files-with-matches "$REGEX" | while IFS= read -r file; do
  cp "$file" "${backup_dir}/"
  tmp=$(mktemp "${file}.XXXXXX")
  if jq "del(.[] | select(${JSON_OBJECT}? == '$REGEX'))" "$file" >"$tmp"; then
    mv "$tmp" "$file"
  else
    rm -f "$tmp"
    printf "Error: jq failed on %s; leaving original untouched.\n" "$file" >&2
    exit 1
  fi
done
