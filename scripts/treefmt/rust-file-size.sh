#!/usr/bin/env bash

set -euo pipefail

readonly maximum_physical_lines=500

count_physical_lines() {
  local file=$1
  awk 'END { print NR }' <"$file"
}

line_count_is_over_the_limit() {
  local line_count=$1
  ((line_count > maximum_physical_lines))
}

report_file_over_the_limit() {
  local file=$1 line_count=$2
  printf 'rust-file-size: %s has %d physical lines (limit %d)\n' \
    "$file" "$line_count" "$maximum_physical_lines" >&2
}

check_line_limit() {
  local file=$1
  local line_count
  line_count="$(count_physical_lines "$file")" || return 1
  if line_count_is_over_the_limit "$line_count"; then
    report_file_over_the_limit "$file" "$line_count"
    return 1
  fi
}

check_line_limit_of_every_file() {
  local file check_status=0
  for file in "$@"; do
    check_line_limit "$file" || check_status=1
  done
  return "$check_status"
}

main() {
  check_line_limit_of_every_file "$@" || exit 1
}

main "$@"
