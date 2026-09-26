#!/usr/bin/env bash

set -uo pipefail

file_is_valid_yaml() {
  local file=$1
  yq eval '.' "$file" >/dev/null
}

main() {
  local file status=0
  for file in "$@"; do
    if ! file_is_valid_yaml "$file"; then
      printf 'yq-validate: invalid YAML: %s\n' "$file" >&2
      status=1
    fi
  done
  exit "$status"
}

main "$@"
