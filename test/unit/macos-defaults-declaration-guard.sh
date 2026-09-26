#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/scripts/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

readonly SUPERSEDED_SHAPE_EXPRESSION='(.macos.defaults // []) | tag'
readonly SUPERSEDED_COUNT_EXPRESSION='(.macos.defaults // []) | length'

require_superseded_read_as_zero_records() {
  local path="$1" superseded_shape superseded_count
  superseded_shape="$(yq eval -r "$SUPERSEDED_SHAPE_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded shape expression against $path"
  superseded_count="$(yq eval -r "$SUPERSEDED_COUNT_EXPRESSION" "$path")" ||
    fail "could not evaluate the superseded count expression against $path"
  [[ $superseded_shape == '!!seq' && $superseded_count == '0' ]] ||
    fail "fixture $path answers shape $superseded_shape and count $superseded_count to the superseded expressions, so it does not reproduce the zero-records fail-open this case exists to pin"
}

refute_declaration_accepted() {
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output"
}

require_names_the_empty_list_spelling() {
  local refusal="$1" description="$2"
  printf '%s' "$refusal" | grep -qF 'defaults: []' ||
    fail "the $description refusal does not name \`defaults: []\` as the way to track no records: $refusal"
}

require_declaration_accepted() {
  local path="$1" expected_count="$2" description="$3" status=0 count
  count="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 0 ]] ||
    fail "$description must be accepted, got status $status ($count)"
  [[ $count == "$expected_count" ]] ||
    fail "$description must count $expected_count record(s), got: $count"
}

require_verdict() {
  local shape_answer="$1" expected_verdict="$2" description="$3" verdict
  verdict="$(records_declaration_verdict "$shape_answer")"
  [[ $verdict == "$expected_verdict" ]] ||
    fail "$description must classify as $expected_verdict, got $verdict (input: $(printf '%q' "$shape_answer"))"
}

require_byte_order_mark_detected() {
  data_file_begins_with_byte_order_mark "$1" ||
    fail "$2 must be detected as beginning with a byte order mark"
}

refute_byte_order_mark_detected() {
  if data_file_begins_with_byte_order_mark "$1"; then
    fail "$2 must NOT be detected as beginning with a byte order mark"
  fi
}

require_unreadable_path_refused() {
  local path="$1" description="$2" status=0 output
  output="$(defaults_records_declared_count "$path" 2>&1)" || status=$?
  [[ $status -eq 2 ]] ||
    fail "$description must be refused with status 2, got $status (output: $output)"
  printf '%s' "$output" | grep -qF -- "$path" ||
    fail "$description was refused without naming the path, so an operator cannot tell which file the tools could not read: $output"
}

require_yq_strips_byte_order_mark() {
  local keys
  keys="$(yq eval -r 'keys | join(",")' "$1")" ||
    fail "could not read the top-level keys of $1"
  case $keys in
    *"$UTF8_BYTE_ORDER_MARK"*)
      fail "$2: yq no longer strips it, so this file is no longer a position where the two readers disagree and the guard's scope needs remeasuring (keys: $(printf '%q' "$keys"))"
      ;;
  esac
}

refute_yq_strips_byte_order_mark() {
  local keys
  keys="$(yq eval -r 'keys | join(",")' "$1")" ||
    fail "could not read the top-level keys of $1"
  case $keys in
    *"$UTF8_BYTE_ORDER_MARK"*) ;;
    *)
      fail "$2: yq stripped it, so a mark in this position is now a reader DIVERGENCE that a byte-0 check does not catch (keys: $(printf '%q' "$keys"))"
      ;;
  esac
}

[[ -f $LIB ]] || fail "missing library: $LIB"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; brew install yq (or run: just setup)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# shellcheck source=/dev/null
source "$LIB" >/dev/null 2>&1

cat >"$work/list.yaml" <<'EOF'
macos:
  defaults:
    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}
    - {domain: com.example.alpha, key: AKey, value: "1", type: bool, tier: enforce}
EOF
require_declaration_accepted "$work/list.yaml" 2 "a real two-record list"

cat >"$work/empty-list.yaml" <<'EOF'
macos:
  defaults: []
EOF
require_declaration_accepted "$work/empty-list.yaml" 0 "an explicitly empty record list"

printf 'other: 1\n' >"$work/absent-macos.yaml"
printf 'macos:\n  killall: []\n' >"$work/absent-defaults.yaml"
printf 'macos:\n  defaults:\n' >"$work/null-defaults.yaml"
: >"$work/empty-file.yaml"

for absent_case in absent-macos absent-defaults null-defaults empty-file; do
  require_superseded_read_as_zero_records "$work/$absent_case.yaml"
  absent_refusal="$(refute_declaration_accepted "$work/$absent_case.yaml" "a file whose records are $absent_case")"
  printf '%s' "$absent_refusal" | grep -q '\.macos\.defaults' ||
    fail "the $absent_case refusal does not name .macos.defaults, so the operator cannot tell what to edit: $absent_refusal"
  require_names_the_empty_list_spelling "$absent_refusal" "$absent_case"
done

printf 'macos:\n  \xef\xbb\xbfdefaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n' \
  >"$work/nested-byte-order-mark.yaml"
require_superseded_read_as_zero_records "$work/nested-byte-order-mark.yaml"
nested_mark_refusal="$(refute_declaration_accepted "$work/nested-byte-order-mark.yaml" "a byte order mark before the defaults key")"
require_names_the_empty_list_spelling "$nested_mark_refusal" "nested byte order mark"

printf '\xef\xbb\xbfmacos:\n  defaults:\n    - {domain: com.example.zebra, key: ZKey, value: "1", type: bool, tier: enforce}\n' \
  >"$work/leading-byte-order-mark.yaml"
leading_shape="$(yq eval -r "$SUPERSEDED_SHAPE_EXPRESSION" "$work/leading-byte-order-mark.yaml")"
leading_count="$(yq eval -r "$SUPERSEDED_COUNT_EXPRESSION" "$work/leading-byte-order-mark.yaml")"
[[ $leading_shape == '!!seq' && $leading_count == '1' ]] ||
  fail "the leading-mark fixture no longer reproduces the divergence: yq answers shape $leading_shape and count $leading_count, so this case is not pinning a file yq reads and the runner template refuses"
leading_mark_refusal="$(refute_declaration_accepted "$work/leading-byte-order-mark.yaml" "a document-start byte order mark")"
printf '%s' "$leading_mark_refusal" | grep -qi 'byte order mark' ||
  fail "the refusal does not name the byte order mark, so the operator cannot see what to remove: $leading_mark_refusal"

printf 'macos: 5\n' >"$work/macos-not-a-mapping.yaml"
require_superseded_read_as_zero_records "$work/macos-not-a-mapping.yaml"
not_a_mapping_refusal="$(refute_declaration_accepted "$work/macos-not-a-mapping.yaml" "a .macos that is not a mapping")"
if printf '%s' "$not_a_mapping_refusal" | grep -qF 'defaults: []'; then
  fail "the refusal tells the operator to write \`defaults: []\`, which will not fix a .macos that is not a mapping: $not_a_mapping_refusal"
fi

require_verdict 'scalar !!null' absent "the answer yq gives for a missing or null node"
require_verdict 'seq !!seq' list "a real sequence, which the absent verdict must not swallow"
require_verdict 'scalar !!seq' other "a scalar wearing a !!seq tag, which is not the absent case"
require_verdict '' other "the empty answer yq gives when .macos is not a mapping"

require_byte_order_mark_detected "$work/leading-byte-order-mark.yaml" "a file that starts with the mark"
refute_byte_order_mark_detected "$work/list.yaml" "a plain ASCII data file"
refute_byte_order_mark_detected "$work/nested-byte-order-mark.yaml" "a file whose mark is not at the start"
refute_byte_order_mark_detected "$work/empty-file.yaml" "an empty file"
printf '\xef\xac\x81rst: 1\n' >"$work/first-byte-collision.yaml"
refute_byte_order_mark_detected "$work/first-byte-collision.yaml" "a file starting with a different 0xEF sequence"
printf '\xef\xbb' >"$work/truncated-mark.yaml"
refute_byte_order_mark_detected "$work/truncated-mark.yaml" "a file holding only the first two bytes of the mark"

mkdir -p "$work/a-directory"
printf 'macos:\n  defaults: []\n' >"$work/unreadable.yaml"
chmod 000 "$work/unreadable.yaml"

refute_byte_order_mark_detected "$work/missing.yaml" "a path that does not exist"
require_unreadable_path_refused "$work/missing.yaml" "a path that does not exist"
refute_byte_order_mark_detected "$work/a-directory" "a path that is a directory"
require_unreadable_path_refused "$work/a-directory" "a path that is a directory"
if [[ $EUID -ne 0 ]]; then
  refute_byte_order_mark_detected "$work/unreadable.yaml" "a file this user cannot read"
  require_unreadable_path_refused "$work/unreadable.yaml" "a file this user cannot read"
fi
chmod 644 "$work/unreadable.yaml"

require_yq_strips_byte_order_mark "$work/leading-byte-order-mark.yaml" \
  "a mark at byte 0"
printf 'macos:\n  defaults: []\n\xef\xbb\xbfother: 1\n' >"$work/mark-before-later-key.yaml"
refute_yq_strips_byte_order_mark "$work/mark-before-later-key.yaml" \
  "a mark at the start of a later line"
printf 'macos:\n  defaults: []\n  \xef\xbb\xbfkillall: []\n' >"$work/mark-after-indent.yaml"
yq eval -r '.macos | keys | join(",")' "$work/mark-after-indent.yaml" |
  grep -qF "$UTF8_BYTE_ORDER_MARK" ||
  fail "yq now strips a mark that follows the indent of a nested key, so a mark in that position is a reader divergence a byte-0 check does not catch"

printf 'macos-defaults-declaration-guard: OK (an explicitly empty list still answers 0; a file that declares no record list is refused and pointed at an explicitly empty one; a byte order mark is refused whether it hides the defaults key or breaks the runner template; an unreadable path answers no-mark and is still refused by name; byte 0 is still the only position the two readers treat differently)\n'
