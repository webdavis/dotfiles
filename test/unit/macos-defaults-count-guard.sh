#!/usr/bin/env bash
set -euo pipefail

readonly LARGEST_ACCEPTED_RECORD_COUNT=9999999
readonly SMALLEST_REFUSED_RECORD_COUNT=10000000

readonly HEALTHY_SHAPE_ANSWER='seq !!seq'

readonly WHOLE_FILE_RULES_SATISFIED_ANSWER='0 0 0'

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/scripts/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

require_count_accepted() {
  local count="$1" description="$2"
  count_is_usable "$count" ||
    fail "$description must be accepted as a record count, and was refused: $(printf '%q' "$count")"
}

refute_count_accepted() {
  local count="$1" description="$2"
  if count_is_usable "$count"; then
    fail "$description must be refused as a record count, and was accepted: $(printf '%q' "$count")"
  fi
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; brew install yq (or run: just setup)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

mkdir -p "$work/stub-bin"
cat >"$work/stub-bin/yq" <<'STUB'
#!/usr/bin/env bash
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
  "$STUB_YQ_RULES_EXPRESSION") printf '%s\n' "$STUB_YQ_RULES_ANSWER" ;;
  *)
    printf 'stub yq: asked an expression it was not told to answer: %q\n' "$expression" >&2
    exit 3
    ;;
esac
STUB
chmod +x "$work/stub-bin/yq"

printf 'macos:\n  defaults: []\n' >"$work/stubbed.yaml"

stubbed_count_status=0
stubbed_count_output=""
declared_count_with_stubbed_yq() {
  stubbed_count_status=0
  stubbed_count_output="$(
    PATH="$work/stub-bin:$PATH" \
      STUB_YQ_SHAPE_EXPRESSION="$DEFAULTS_RECORDS_SHAPE_EXPRESSION" \
      STUB_YQ_COUNT_EXPRESSION="$DEFAULTS_RECORDS_COUNT_EXPRESSION" \
      STUB_YQ_RULES_EXPRESSION="$DEFAULTS_DATA_FILE_RULES_EXPRESSION" \
      STUB_YQ_SHAPE_ANSWER="$1" STUB_YQ_COUNT_ANSWER="$2" \
      STUB_YQ_RULES_ANSWER="$WHOLE_FILE_RULES_SATISFIED_ANSWER" \
      bash -c 'source "$1"; defaults_records_declared_count "$2"' _ "$LIB" "$work/stubbed.yaml" 2>&1
  )" || stubbed_count_status=$?
}

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
printf '%s' "$output" | grep -qi 'document' ||
  fail "the refusal does not say the file contains more than one document: $output"

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

cat >"$work/empty.yaml" <<'EOF'
macos:
  defaults: []
EOF
status=0
count="$(defaults_records_declared_count "$work/empty.yaml" 2>&1)" || status=$?
[[ $status -eq 0 && $count == 0 ]] ||
  fail "an empty record list must count as 0 and be accepted, got status $status ($count)"

refute_count_accepted "$SMALLEST_REFUSED_RECORD_COUNT" "the smallest eight-digit count"

require_count_accepted "$LARGEST_ACCEPTED_RECORD_COUNT" "the largest seven-digit count"

refute_count_accepted $'1\n---\n0' "a two-document count"

refute_count_accepted '0010' "a count with a leading zero"

refute_count_accepted '18446744073709551616' "a count past the 64-bit range"

refute_count_accepted 'abc' "a non-numeric count"
refute_count_accepted '' "an empty count"
refute_count_accepted ' 1' "a count with leading whitespace"
refute_count_accepted '-1' "a negative count"

require_count_accepted '0' "a count of zero"
require_count_accepted '1' "a count of one"

declared_count_with_stubbed_yq "$HEALTHY_SHAPE_ANSWER" "$LARGEST_ACCEPTED_RECORD_COUNT"
[[ $stubbed_count_status -eq 0 ]] ||
  fail "the stubbed-yq harness is broken: a healthy shape and an in-range count must be accepted, got status $stubbed_count_status ($stubbed_count_output)"
[[ $stubbed_count_output == "$LARGEST_ACCEPTED_RECORD_COUNT" ]] ||
  fail "the stubbed-yq harness is broken: expected the count $LARGEST_ACCEPTED_RECORD_COUNT to come back unchanged, got: $stubbed_count_output"

declared_count_with_stubbed_yq "$HEALTHY_SHAPE_ANSWER" "$SMALLEST_REFUSED_RECORD_COUNT"
[[ $stubbed_count_status -eq 2 ]] ||
  fail "a healthy shape with an oversized count $SMALLEST_REFUSED_RECORD_COUNT must be refused with status 2, so the record count is never used as a loop bound; got status $stubbed_count_status ($stubbed_count_output)"
printf '%s' "$stubbed_count_output" | grep -qF "$SMALLEST_REFUSED_RECORD_COUNT" ||
  fail "the refusal does not carry the offending count, so an operator has nothing to look for: $stubbed_count_output"
printf '%s' "$stubbed_count_output" | grep -qF 'unusable record count' ||
  fail "the oversized count was refused by some other check, so this case does not pin the numeric guard's call site: $stubbed_count_output"

printf 'macos-defaults-count-guard: OK (a multi-document file is refused and told it has more than one document; the digit ceiling refuses %s and accepts %s; the whole function still consults the numeric guard; single and empty files still answer)\n' \
  "$SMALLEST_REFUSED_RECORD_COUNT" "$LARGEST_ACCEPTED_RECORD_COUNT"
