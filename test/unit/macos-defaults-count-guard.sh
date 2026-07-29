#!/usr/bin/env bash
# macos-defaults-count-guard.sh, a multi-document data file must be REFUSED, and
# a well-formed one must still answer.
#
# yq answers once per document and separates the answers, so on a two-document
# file the record "count" arrives as $'1\n---\n0' and the shape as
# $'!!seq\n---\n!!seq'. Neither is usable, and the count in particular is about
# to drive `((...))` loop bounds and the `-ne` comparison that catches a forged
# record; bash arithmetic on a non-numeric string raises a syntax error and
# evaluates FALSE, so the forgery check would fall through.
#
# This asserts the BEHAVIOUR, not one guard. `defaults_records_declared_count`
# holds two checks that both reject a multi-document file, the numeric-count
# check and the list-shape check, and either is sufficient on this input.
# Measured: removing the numeric check alone leaves this test green, because the
# shape check refuses the same file. So the numeric check is defence in depth
# here rather than the sole barrier, and a test claiming to pin it specifically
# would be claiming a guarantee it does not provide.
#
# What that leaves open is recorded in the follow-up: whether an input exists
# that yields a non-numeric count with a clean !!seq shape. If none does, the
# numeric check is redundant and should be kept or dropped deliberately rather
# than left looking load-bearing.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
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

printf 'macos-defaults-count-guard: OK (a multi-document count is refused and named; single and empty files still answer)\n'
