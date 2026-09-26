#!/usr/bin/env bash

set -euo pipefail

readonly maximum_physical_lines=500

file_is_within_line_limit() {
  local file=$1
  awk -v file="$file" -v limit="$maximum_physical_lines" '
    END {
      if (NR > limit) {
        printf "rust-file-size: %s has %d physical lines (limit %d)\n", file, NR, limit
        exit 1
      }
    }
  ' <"$file" >&2
}

main() {
  local file status=0
  for file in "$@"; do
    if ! file_is_within_line_limit "$file"; then
      status=1
    fi
  done
  exit "$status"
}

main "$@"
