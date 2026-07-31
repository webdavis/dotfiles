#!/usr/bin/env bash
# macos-defaults-count-guard.sh, the record count must refuse every value it
# cannot use, and defaults_records_declared_count must still answer for a file
# it can.
#
# The numeric check is `declared_record_count_is_usable`, a pure predicate over
# yq's answer. Cases 4 to 9 call it DIRECTLY, which is the only way left to pin
# its digit ceiling at both boundaries and the only way that names it as the
# guard under test.
#
# It used to be pinned through the whole function, on a data file whose
# .macos.defaults was a SCALAR carrying an explicit `!!seq` tag: the tag
# satisfied the old shape check while yq's `length` reported the scalar's length
# in BYTES, so a ten-megabyte fixture arrived at the numeric check with a count
# of 10000000 and a clean shape. That fixture WAS the vulnerability, not a way
# of reaching past it. The shape check now asks yq for the node's `kind`, which
# a document author cannot overrule, so it refuses a tagged scalar before the
# count is ever computed (test/unit/macos-defaults-shape-guard.sh) and no cheap
# data file reaches the ceiling any more. Calling the predicate directly pins
# the boundary exactly rather than through a fixture whose byte count has to be
# taken on trust, it keeps the ceiling pinned at both ends, and it retires the
# ten-megabyte fixture the old form needed: measured three runs each on the same
# machine, the old file cost 0.71 to 1.04 s and this one costs 0.08 to 0.22 s.
#
# Cases 1 to 3 stay end-to-end, on the function. They assert what an operator
# sees for a whole data file, which no predicate call can stand in for:
#
#   Case 1, a MULTI-DOCUMENT file. yq answers once per document and separates
#   the answers, so a two-document file makes the shape arrive as
#   $'seq !!seq\n---\nseq !!seq' and the count as $'1\n---\n0'. Both the shape
#   classifier (which refuses any answer that is not one line) and the numeric
#   predicate refuse that independently, so the case asserts the BEHAVIOUR
#   rather than claiming to pin one guard. Case 6 holds the numeric half of it
#   directly.
#
#   Cases 2 and 3 are the false-positive direction and carry the file: a guard
#   that refuses everything passes every refusal case here and nothing else.
#
#   Cases 11 and 12 pin the guard's CALL SITE, which is a different claim from
#   either boundary and was pinned by nothing. Calling the predicate directly
#   proves the predicate; it says nothing about whether
#   defaults_records_declared_count still consults it. Deleting the
#   `declared_record_count_is_usable` call left every other case in this file
#   green. What retiring the ten-megabyte fixture cost was exactly this, and the
#   fixture is not the way to get it back: it WAS the vulnerability. A STUBBED
#   yq buys the same wiring pin for nothing, by answering a healthy shape and an
#   oversized count for a file that is neither.
set -euo pipefail

# The guard admits at most seven digits, so these are the largest count it
# accepts and the smallest it refuses.
readonly LARGEST_ACCEPTED_RECORD_COUNT=9999999
readonly SMALLEST_REFUSED_RECORD_COUNT=10000000

# The shape answer a stubbed yq gives so the wiring cases reach the count check:
# a healthy record list, which is the only verdict that does not refuse first.
readonly HEALTHY_SHAPE_ANSWER='seq !!seq'

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# require_count_accepted <count> <description>, the false-positive direction for
# the numeric predicate: this value must be USABLE.
require_count_accepted() { # <count> <description>
  local count="$1" description="$2"
  declared_record_count_is_usable "$count" ||
    fail "$description must be accepted as a record count, and was refused: $(printf '%q' "$count")"
}

# refute_count_accepted <count> <description>, require the numeric predicate to
# refuse this value.
#
# A named helper rather than an inline `! declared_record_count_is_usable ...`:
# under `set -e` an inverted command only decides the test in final position, so
# a bare negation between other statements is a position lottery.
refute_count_accepted() { # <count> <description>
  local count="$1" description="$2"
  if declared_record_count_is_usable "$count"; then
    fail "$description must be refused as a record count, and was accepted: $(printf '%q' "$count")"
  fi
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; run inside the nix dev shell"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

# ---- the stubbed-yq harness, for the call-site cases ------------------------
# The stub keys on the library's OWN expression constants, passed in as
# environment. It refuses anything it was not told to answer, so a library that
# renames or reshapes an expression makes these cases fail loudly instead of
# quietly stubbing a call nobody makes any more.
mkdir -p "$work/stub-bin"
cat >"$work/stub-bin/yq" <<'STUB'
#!/usr/bin/env bash
# A yq that answers two known expressions and nothing else. Invoked as
# `yq eval -r <expression> <path>`, so the expression is the argument after -r.
set -euo pipefail
expression=""
previous_argument=""
for argument in "$@"; do
  [[ $previous_argument == '-r' ]] && expression="$argument"
  previous_argument="$argument"
done
case $expression in
  "$STUB_YQ_SHAPE_EXPRESSION") printf '%s\n' "$STUB_YQ_SHAPE_ANSWER" ;;
  "$STUB_YQ_COUNT_EXPRESSION") printf '%s\n' "$STUB_YQ_COUNT_ANSWER" ;;
  *)
    printf 'stub yq: asked an expression it was not told to answer: %q\n' "$expression" >&2
    exit 3
    ;;
esac
STUB
chmod +x "$work/stub-bin/yq"

# The file the stubbed cases are asked about. Its CONTENT is irrelevant to the
# stub, but it must exist and carry no byte order mark: the mark predicate reads
# real bytes with `head` and is not stubbed.
printf 'macos:\n  defaults: []\n' >"$work/stubbed.yaml"

# declared_count_with_stubbed_yq <shape-answer> <count-answer>, run
# defaults_records_declared_count in a fresh shell whose yq is the stub, and set
# stubbed_count_status / stubbed_count_output from it.
#
# A separate process rather than a PATH change in this one: bash caches resolved
# command paths, so a stub that appears on PATH after the real yq has been run
# may never be consulted, and a wiring pin that silently tested the real yq
# would be worse than no pin at all.
stubbed_count_status=0
stubbed_count_output=""
declared_count_with_stubbed_yq() { # <shape-answer> <count-answer>
  stubbed_count_status=0
  stubbed_count_output="$(
    PATH="$work/stub-bin:$PATH" \
      STUB_YQ_SHAPE_EXPRESSION="$DEFAULTS_RECORDS_SHAPE_EXPRESSION" \
      STUB_YQ_COUNT_EXPRESSION="$DEFAULTS_RECORDS_COUNT_EXPRESSION" \
      STUB_YQ_SHAPE_ANSWER="$1" STUB_YQ_COUNT_ANSWER="$2" \
      bash -c 'source "$1"; defaults_records_declared_count "$2"' _ "$LIB" "$work/stubbed.yaml" 2>&1
  )" || stubbed_count_status=$?
}

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
# "unusable" and has nothing to search the file for. `---` is the document
# separator yq puts between its per-document answers, so its presence is what
# tells the operator that the file has more than one document in it.
printf '%s' "$output" | grep -q -- '---' ||
  fail "the refusal does not show yq's unusable per-document answer, so it does not say what is wrong: $output"

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

# ---- 4: the smallest count past the digit ceiling is refused ----------------
refute_count_accepted "$SMALLEST_REFUSED_RECORD_COUNT" "the smallest eight-digit count"

# ---- 5: the largest in-range count is still accepted ------------------------
# The other side of the ceiling, and the ONLY case that catches a bound the next
# reader lowers. Every other case here stays green as the bound drops: no other
# accepted value has more than one digit, and case 4 only gets stricter.
require_count_accepted "$LARGEST_ACCEPTED_RECORD_COUNT" "the largest seven-digit count"

# ---- 6: a multi-line count is refused ---------------------------------------
# The numeric half of case 1, held directly. Bash raises no syntax error on this
# value: it reads the whole thing as ONE arithmetic expression and the `---`
# separator as a run of unary minus signs, so $'1\n---\n0' evaluates to 1
# (measured, bash 5.3.15) and the equality check that catches a forged record
# would silently compare against the DIFFERENCE of the per-document counts.
refute_count_accepted $'1\n---\n0' "a two-document count"

# ---- 7: a leading zero is refused -------------------------------------------
# Bash reads a leading zero as octal, so $((0010)) is 8. A count that means one
# thing to the guard and another to the arithmetic is not usable.
refute_count_accepted '0010' "a count with a leading zero"

# ---- 8: a value bash arithmetic would WRAP is refused ------------------------
# The reason the bound lives in the digit quantifier rather than in a `-gt`
# comparison: a comparison has to evaluate the value to reject it, and this one
# evaluates to 0 (measured), so `^[0-9]+$` plus `-gt` would accept it.
refute_count_accepted '18446744073709551616' "a count past the 64-bit range"

# ---- 9: non-numeric and empty counts are refused -----------------------------
# yq answers with an integer node for a sequence, so neither shape reaches the
# predicate from today's producer. Both are pinned anyway: the predicate is the
# barrier if a future producer ever answers something else.
refute_count_accepted 'abc' "a non-numeric count"
refute_count_accepted '' "an empty count"
refute_count_accepted ' 1' "a count with leading whitespace"
refute_count_accepted '-1' "a negative count"

# ---- 10: the smallest counts are still accepted ------------------------------
# The floor of the false-positive direction. Without them the predicate could be
# inverted to refuse everything and cases 4 and 6 to 9 would all stay green.
require_count_accepted '0' "a count of zero"
require_count_accepted '1' "a count of one"

# ---- 11: the CONTROL for the wiring cases ------------------------------------
# The stub must be reached and its answers must be believed, or case 12 proves
# nothing: a harness whose stub is never consulted refuses this file for some
# unrelated reason and looks identical to a working one. Asserting the accepted
# side FIRST, and asserting the count comes back byte for byte, is what
# distinguishes "the stub answered" from "something said no".
declared_count_with_stubbed_yq "$HEALTHY_SHAPE_ANSWER" "$LARGEST_ACCEPTED_RECORD_COUNT"
[[ $stubbed_count_status -eq 0 ]] ||
  fail "the stubbed-yq harness is broken: a healthy shape and an in-range count must be accepted, got status $stubbed_count_status ($stubbed_count_output)"
[[ $stubbed_count_output == "$LARGEST_ACCEPTED_RECORD_COUNT" ]] ||
  fail "the stubbed-yq harness is broken: expected the count $LARGEST_ACCEPTED_RECORD_COUNT to come back unchanged, got: $stubbed_count_output"

# ---- 12: defaults_records_declared_count still CONSULTS the numeric guard ----
# The wiring, pinned as its own claim. yq answers a perfectly healthy shape here,
# so every other check in the function passes and the numeric guard is the only
# thing standing between an oversized count and a caller that will use it as a
# loop bound. Delete the `declared_record_count_is_usable` call and this is the
# only case in the file that notices.
declared_count_with_stubbed_yq "$HEALTHY_SHAPE_ANSWER" "$SMALLEST_REFUSED_RECORD_COUNT"
[[ $stubbed_count_status -eq 2 ]] ||
  fail "a healthy shape with an oversized count $SMALLEST_REFUSED_RECORD_COUNT must be refused with status 2, so the record count is never used as a loop bound; got status $stubbed_count_status ($stubbed_count_output)"
printf '%s' "$stubbed_count_output" | grep -qF "$SMALLEST_REFUSED_RECORD_COUNT" ||
  fail "the refusal does not carry the offending count, so an operator has nothing to look for: $stubbed_count_output"
printf '%s' "$stubbed_count_output" | grep -qF 'unusable record count' ||
  fail "the oversized count was refused by some other check, so this case does not pin the numeric guard's call site: $stubbed_count_output"

printf 'macos-defaults-count-guard: OK (a multi-document file is refused and its unusable answer shown; the digit ceiling refuses %s and accepts %s; the whole function still consults the numeric guard; single and empty files still answer)\n' \
  "$SMALLEST_REFUSED_RECORD_COUNT" "$LARGEST_ACCEPTED_RECORD_COUNT"
