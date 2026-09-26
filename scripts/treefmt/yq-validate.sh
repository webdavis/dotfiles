#!/usr/bin/env bash

set -euo pipefail

file_is_valid_yaml() {
  local file=$1
  yq eval '.' "$file" >/dev/null
}

report_invalid_yaml_file() {
  local file=$1
  printf 'yq-validate: invalid YAML: %s\n' "$file" >&2
}

validate_every_yaml_file() {
  local file validation_status=0
  for file in "$@"; do
    if ! file_is_valid_yaml "$file"; then
      report_invalid_yaml_file "$file"
      validation_status=1
    fi
  done
  return "$validation_status"
}

main() {
  validate_every_yaml_file "$@" || exit 1
}

main "$@"
