#!/usr/bin/env bash
# macos-defaults-count-guard.sh, defaults_records_declared_count must refuse
# every count it cannot use, and must still answer for a file it can.
#
# The function holds two independent checks, a numeric-count check and a
# list-shape check. This file pins each on an input where it is the ONLY one
# that fires, so neither can be dropped on the strength of the other.
#
# NEITHER check owns the multi-document case (case 1) on its own. yq answers
# once per document and separates the answers, so a two-document file makes the
# count arrive as $'1\n---\n0' and the shape as $'!!seq\n---\n!!seq', and each
# check refuses that independently: disabling either one leaves case 1 green,
# disabling both fails it. Case 1 therefore asserts the BEHAVIOUR rather than
# claiming to pin one guard.
#
# The NUMERIC check owns the digit ceiling (cases 4 and 5), and there it is the
# only barrier. `!!seq` is a tag, not a proof: an explicit `!!seq` on a scalar
# makes the shape check answer a clean `!!seq` while yq's `length` reports the
# SCALAR's character count (measured, yq v4.53.3). A count of 10000000 therefore
# reaches the numeric check with the shape check fully satisfied, and nothing
# else in the function refuses it. Both boundary cases are required: the refusal
# on its own still passes against a guard whose ceiling has been LOWERED,
# because every other fixture in this file counts 0, 1, or 2.
set -euo pipefail

# The guard admits at most seven digits, so these are the largest count it
# accepts and the smallest it refuses.
readonly LARGEST_ACCEPTED_RECORD_COUNT=9999999
readonly SMALLEST_REFUSED_RECORD_COUNT=10000000

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# write_seq_tagged_scalar <path> <character-count>, write a data file whose
# .macos.defaults is a SCALAR carrying an explicit !!seq tag. yq then reports the
# shape as a clean !!seq while `length` answers with the scalar's character
# count, which is what lets the boundary cases below vary the count while
# holding the shape check satisfied. The body is spaces from a single printf, so
# even a ten-megabyte fixture costs no second process.
write_seq_tagged_scalar() { # <path> <character-count>
  local path="$1" character_count="$2"
  {
    printf 'macos:\n  defaults: !!seq "'
    printf '%*s' "$character_count" ''
    printf '"\n'
  } >"$path"
}

# require_clean_seq_shape <path>, fail unless this fixture SATISFIES the shape
# check inside defaults_records_declared_count. Asserted before each boundary
# case: without it the case would pass no matter which of the two checks did the
# refusing, which is exactly the ambiguity these cases exist to remove.
require_clean_seq_shape() { # <path>
  local path="$1" shape
  shape="$(yq eval -r '(.macos.defaults // []) | tag' "$path")" ||
    fail "could not read the record shape of $path"
  [[ $shape == '!!seq' ]] ||
    fail "fixture $path does not satisfy the shape check (tag $shape), so a refusal would not isolate the count check"
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# ---- 1: a multi-document file is REFUSED -----------------------------------
cat >"$work/multi.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.a, key: AKey, value: "1", type: bool, tier: enforce}
---
macos:
  defaults: []
EOF
status=0
output="$(defaults_records_declared_count "$work/multi.yaml" 2>&1)" || status=$?
[[ $status -eq 2 ]] ||
  fail "a multi-document data file must be refused with status 2, got $status (output: $output)"
# The message must carry the offending value. Without it an operator sees only
# "unusable" and has nothing to search the file for.
printf '%s' "$output" | grep -q -- '---' ||
  fail "the refusal does not show the unusable count, so it does not say what is wrong: $output"

# ---- 2: a well-formed file still answers -----------------------------------
# The guard must reject a shape, not everything. Without this the test would
# still pass against a function that refuses unconditionally.
cat >"$work/single.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.a, key: AKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.b, key: BKey, value: "1", type: bool, tier: enforce}
EOF
status=0
count="$(defaults_records_declared_count "$work/single.yaml" 2>&1)" || status=$?
[[ $status -eq 0 ]] || fail "a well-formed file must be accepted, got status $status ($count)"
[[ $count == 2 ]] || fail "expected a count of 2, got: $count"

# ---- 3: an empty record list is a valid count of zero ----------------------
cat >"$work/empty.yaml" <<'EOF'
macos:
  defaults: []
EOF
status=0
count="$(defaults_records_declared_count "$work/empty.yaml" 2>&1)" || status=$?
[[ $status -eq 0 && $count == 0 ]] ||
  fail "an empty record list must count as 0 and be accepted, got status $status ($count)"

# ---- 4: a count past the digit ceiling is refused, SHAPE CHECK CLEAN --------
# The case that pins the numeric check specifically. The shape assertion is what
# makes it a pin rather than an observation: it establishes that the other check
# in the function is satisfied, so the refusal can only have come from the count.
write_seq_tagged_scalar "$work/over-ceiling.yaml" "$SMALLEST_REFUSED_RECORD_COUNT"
require_clean_seq_shape "$work/over-ceiling.yaml"
status=0
output="$(defaults_records_declared_count "$work/over-ceiling.yaml" 2>&1)" || status=$?
[[ $status -eq 2 ]] ||
  fail "a record count of $SMALLEST_REFUSED_RECORD_COUNT must be refused with status 2, got $status (output: $output)"
printf '%s' "$output" | grep -q -- "count $SMALLEST_REFUSED_RECORD_COUNT" ||
  fail "the refusal does not name the offending count $SMALLEST_REFUSED_RECORD_COUNT: $output"

# ---- 5: the largest in-range count is still accepted ------------------------
# The other side of the ceiling. Without it, lowering the guard's digit bound
# would leave every case above green: the fixtures elsewhere in this file count
# 0, 1, and 2, and case 4 only gets stricter as the bound drops.
write_seq_tagged_scalar "$work/at-ceiling.yaml" "$LARGEST_ACCEPTED_RECORD_COUNT"
require_clean_seq_shape "$work/at-ceiling.yaml"
status=0
count="$(defaults_records_declared_count "$work/at-ceiling.yaml" 2>&1)" || status=$?
[[ $status -eq 0 ]] ||
  fail "a record count of $LARGEST_ACCEPTED_RECORD_COUNT must be accepted, got status $status ($count)"
[[ $count == "$LARGEST_ACCEPTED_RECORD_COUNT" ]] ||
  fail "expected a count of $LARGEST_ACCEPTED_RECORD_COUNT, got: $count"

printf 'macos-defaults-count-guard: OK (a multi-document count is refused and named; the digit ceiling refuses %s and accepts %s with the shape check clean; single and empty files still answer)\n' \
  "$SMALLEST_REFUSED_RECORD_COUNT" "$LARGEST_ACCEPTED_RECORD_COUNT"
