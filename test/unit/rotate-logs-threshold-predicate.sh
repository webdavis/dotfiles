#!/usr/bin/env bash
#
# rotate-logs.sh decides what to rotate from two pure predicates:
# exceeds_size_threshold (is this file big enough?) and is_valid_byte_count
# (is this number safe to compare arithmetically?). Both are pure: one input
# pair in, a boolean out, no filesystem access. Pin them directly so the
# boundary cases are covered without seeding a single fixture file.
#
# The boundary matters: the threshold is inclusive (>=), so a file of exactly
# ROTATE_LOGS_AT_BYTES rotates. The validator matters because bash compares
# numbers with arithmetic evaluation, where a leading zero means OCTAL and an
# oversized value wraps; an unvalidated stat(1) result would silently misjudge.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROTATE_LOGS="$REPO_ROOT/dot_local/libexec/executable_compress-and-truncate-local-logs.sh"

failures=0

fail() {
  printf 'rotate-logs-threshold-predicate: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

[[ -f $ROTATE_LOGS ]] || {
  printf 'rotate-logs-threshold-predicate: FAIL -- missing script: %s\n' "$ROTATE_LOGS" >&2
  exit 1
}

# Source for its functions only. The script guards its own entry point so a
# source never runs a rotation pass.
# shellcheck source=/dev/null
source "$ROTATE_LOGS"

# refute <description> <command...> -- assert the command FAILS. A bare
# `! cmd` under `set -e` only decides the test in final position, so negative
# assertions go through this helper instead.
refute() {
  local description="$1"
  shift
  if "$@"; then
    fail "$description: expected failure, got success"
  fi
}

assert() {
  local description="$1"
  shift
  if ! "$@"; then
    fail "$description: expected success, got failure"
  fi
}

# --- exceeds_size_threshold: inclusive lower bound -------------------------
assert "size above threshold rotates" exceeds_size_threshold 10485761 10485760
assert "size exactly at threshold rotates (inclusive)" exceeds_size_threshold 10485760 10485760
refute "size one byte below threshold is kept" exceeds_size_threshold 10485759 10485760
refute "empty file is kept" exceeds_size_threshold 0 10485760
refute "a one-byte threshold still keeps an empty file" exceeds_size_threshold 0 1

# A malformed size must never be treated as "big enough". Fail-safe direction
# for a rotator is to KEEP the file, never to truncate on a value it could not
# parse.
refute "non-numeric size does not rotate" exceeds_size_threshold "abc" 10
refute "empty size does not rotate" exceeds_size_threshold "" 10
refute "negative size does not rotate" exceeds_size_threshold "-5" 10
refute "non-numeric threshold does not rotate" exceeds_size_threshold 999999 "abc"

# Leading zeros are the octal trap: bash arithmetic reads 010 as EIGHT unless
# the comparison forces base 10. Both assertions below hold only under decimal
# (octal 8 would fail each one), so they discriminate rather than merely pass.
assert "leading-zero size compares as decimal (10 >= 9)" exceeds_size_threshold "010" 9
assert "leading-zero size compares as decimal (10 >= 10)" exceeds_size_threshold "010" 10
refute "leading-zero size is still bounded above (10 < 11)" exceeds_size_threshold "010" 11
# "008" is not a legal octal literal at all; an unforced comparison would raise
# an arithmetic error rather than answer. It must answer, and answer NO.
refute "invalid-octal digit string answers instead of erroring" exceeds_size_threshold "008" 9

# --- is_valid_byte_count: range-bounded before arithmetic ------------------
assert "plain integer is a valid byte count" is_valid_byte_count 1024
assert "zero is a valid byte count" is_valid_byte_count 0
refute "empty string is not a valid byte count" is_valid_byte_count ""
refute "unset argument is not a valid byte count" is_valid_byte_count
refute "non-numeric is not a valid byte count" is_valid_byte_count "12x"
refute "negative is not a valid byte count" is_valid_byte_count "-1"
refute "whitespace is not a valid byte count" is_valid_byte_count " 12"
# Bound the upper side too: an absurdly long digit string would wrap a 64-bit
# arithmetic comparison, so it is rejected rather than silently misread.
refute "oversized digit string is rejected before arithmetic" \
  is_valid_byte_count "999999999999999999999999999999"

if [[ $failures -gt 0 ]]; then
  printf 'rotate-logs-threshold-predicate: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi

printf 'rotate-logs-threshold-predicate: OK (inclusive bound, octal trap, range guard)\n'
