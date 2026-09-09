#!/usr/bin/env bash
# bashunit sources this file; all inputs stay in owned scratch space.

set_up_before_script() {
  RUST_SIZE_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  RUST_SIZE_SCRIPT="$RUST_SIZE_REPO/scripts/treefmt/rust-file-size.sh"
  RUST_SIZE_FIXTURE="$(mktemp -d)"
}

set_up() {
  RUST_SIZE_CASE="$(mktemp -d "$RUST_SIZE_FIXTURE/case.XXXXXX")"
}

function test_five_hundred_physical_lines_pass_silently_without_changes() {
  local file="$RUST_SIZE_CASE/at limit.rs" output result
  awk 'BEGIN { for (i = 0; i < 500; i++) print "" }' >"$file"
  cp "$file" "$RUST_SIZE_CASE/before"
  output="$("$RUST_SIZE_SCRIPT" "$file" 2>&1)" && result=0 || result=$?
  assert_same 0 "$result"
  assert_same "" "$output"
  diff -u "$RUST_SIZE_CASE/before" "$file"
  assert_successful_code
}

function test_oversized_files_are_all_reported_and_fail_the_batch() {
  local first="$RUST_SIZE_CASE/first.rs" second="$RUST_SIZE_CASE/second.rs" output result
  awk 'BEGIN { for (i = 0; i < 501; i++) print "// line" }' >"$first"
  cp "$first" "$second"
  output="$("$RUST_SIZE_SCRIPT" "$first" "$second" 2>&1)" && result=0 || result=$?
  assert_same 1 "$result"
  assert_same "rust-file-size: $first has 501 physical lines (limit 500)
rust-file-size: $second has 501 physical lines (limit 500)" "$output"
}

function test_an_unterminated_final_line_counts_toward_the_limit() {
  local file="$RUST_SIZE_CASE/unterminated.rs" output result
  awk 'BEGIN { for (i = 0; i < 500; i++) print "// line"; printf "// final" }' >"$file"
  output="$("$RUST_SIZE_SCRIPT" "$file" 2>&1)" && result=0 || result=$?
  assert_same 1 "$result"
  assert_same "rust-file-size: $file has 501 physical lines (limit 500)" "$output"
}
