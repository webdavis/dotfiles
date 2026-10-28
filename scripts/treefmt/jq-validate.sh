#!/usr/bin/env bash

set -euo pipefail

file_is_valid_json() {
  local file=$1
  jq empty <"$file"
}

report_invalid_json_file() {
  local file=$1
  printf 'jq-validate: invalid JSON: %s\n' "$file" >&2
}

validate_every_json_file() {
  local file validation_status=0
  for file in "$@"; do
    if ! file_is_valid_json "$file"; then
      report_invalid_json_file "$file"
      validation_status=1
    fi
  done
  return "$validation_status"
}

main() {
  validate_every_json_file "$@" || exit 1
}

main "$@"
