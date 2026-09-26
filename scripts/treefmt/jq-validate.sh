#!/usr/bin/env bash

set -uo pipefail

file_is_valid_json() {
  local file=$1
  jq empty <"$file"
}

main() {
  local file status=0
  for file in "$@"; do
    if ! file_is_valid_json "$file"; then
      printf 'jq-validate: invalid JSON: %s\n' "$file" >&2
      status=1
    fi
  done
  exit "$status"
}

main "$@"
